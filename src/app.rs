use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use tokio::sync::mpsc;
use tower_http::services::ServeDir;

use crate::{
    config::Settings,
    jobs::JobStore,
    routes::{
        create_job_handler, download_handler, get_job_handler, get_job_status_handler,
        health_handler, index_handler, list_jobs_handler, system_handler, upload_handler,
    },
    worker::JobQueue,
};

#[derive(Clone, Debug)]
pub struct AppState {
    pub settings: Settings,
    pub jobs: JobStore,
    pub queue: JobQueue,
}

pub fn create_app(settings: Settings) -> Router {
    std::fs::create_dir_all(&settings.data_dir).expect("failed to create data directory");
    let (tx, rx) = mpsc::channel::<String>(settings.max_workers.max(1) * 2);
    let state = AppState {
        settings: settings.clone(),
        jobs: JobStore::open(std::path::Path::new(&settings.data_dir).join("jobs.sqlite"))
            .expect("failed to initialize job database"),
        queue: JobQueue::new(tx),
    };

    let worker_state = state.clone();
    tokio::spawn(async move {
        let mut receiver = rx;
        while let Some(job_id) = receiver.recv().await {
            if let Err(err) = crate::worker::process_queued_job(&worker_state, &job_id).await {
                tracing::error!(job_id = %job_id, error = %err, "queued job failed");
            }
        }
    });

    let recovery_state = state.clone();
    tokio::spawn(async move {
        for job_id in recovery_state.jobs.recover_pending() {
            if let Err(err) = recovery_state.queue.enqueue(job_id.clone()).await {
                tracing::error!(job_id = %job_id, error = %err, "failed to recover queued job");
            }
        }
    });

    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let retention_seconds = cleanup_state
                .settings
                .job_retention_hours
                .saturating_mul(3600);
            let expired_ids = cleanup_state.jobs.expired_job_ids(now, retention_seconds);
            for job_id in expired_ids {
                let path = std::path::Path::new(&cleanup_state.settings.data_dir)
                    .join("jobs")
                    .join(&job_id);
                match tokio::fs::remove_dir_all(path).await {
                    Ok(()) => cleanup_state.jobs.delete(&job_id),
                    Err(err) => {
                        tracing::warn!(job_id = %job_id, error = %err, "failed to remove expired job files");
                    }
                }
            }
        }
    });

    Router::new()
        .route("/", get(index_handler))
        .route("/api/health", get(health_handler))
        .route("/api/system", get(system_handler))
        .route("/api/jobs", post(create_job_handler).get(list_jobs_handler))
        .route("/api/jobs/{id}", get(get_job_handler))
        .route("/api/jobs/{id}/status", get(get_job_status_handler))
        .route("/api/jobs/{id}/download", get(download_handler))
        .route("/api/upload", post(upload_handler))
        .nest_service("/assets", ServeDir::new("web/assets"))
        .layer(DefaultBodyLimit::max(settings.max_upload_size as usize))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::create_app;
    use crate::config::Settings;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::{fs, io::Cursor};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let app = create_app(Settings::default());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn home_page_returns_converter_interface() {
        let app = create_app(Settings::default());

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("Euoni / Conversor"));
        assert!(html.contains("/api/upload"));
    }

    #[tokio::test]
    async fn jobs_endpoint_creates_and_reads_job() {
        let app = create_app(Settings::default());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/jobs")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"input_name":"example.png","from":"png","to":"jpg"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/jobs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn job_status_reports_progress() {
        let app = create_app(Settings::default());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/jobs")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"input_name":"example.png","from":"png","to":"jpg"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/jobs/does-not-exist/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn upload_is_converted_and_can_be_downloaded() {
        let data_dir = std::env::temp_dir().join(format!("euoni-app-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data_dir);
        let settings = Settings {
            data_dir: data_dir.to_string_lossy().to_string(),
            ..Settings::default()
        };
        let app = create_app(settings);

        let image = RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255]));
        let mut image_bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut image_bytes, ImageFormat::Png)
            .unwrap();

        let boundary = "euoni-test-boundary";
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"source.png\"\r\nContent-Type: image/png\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(image_bytes.get_ref());
        body.extend_from_slice(
            format!(
                "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"to\"\r\n\r\njpg\r\n--{boundary}--\r\n"
            )
            .as_bytes(),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/upload")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let upload: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let job_id = upload["job_id"].as_str().unwrap().to_string();

        let mut completed = false;
        for _ in 0..100 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/jobs/{job_id}/status"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status: serde_json::Value = serde_json::from_slice(
                &axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap(),
            )
            .unwrap();
            if status["status"] == "completed" {
                assert_eq!(status["progress"], 100);
                completed = true;
                break;
            }
            tokio::task::yield_now().await;
        }

        assert!(completed, "uploaded job should complete");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/jobs/{job_id}/download"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            !axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .is_empty()
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[tokio::test]
    async fn png_upload_can_be_converted_to_pdf() {
        let data_dir =
            std::env::temp_dir().join(format!("euoni-pdf-app-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data_dir);
        let settings = Settings {
            data_dir: data_dir.to_string_lossy().to_string(),
            ..Settings::default()
        };
        let app = create_app(settings);

        let image = RgbaImage::from_pixel(8, 8, Rgba([0, 120, 255, 255]));
        let mut image_bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut image_bytes, ImageFormat::Png)
            .unwrap();

        let boundary = "euoni-pdf-test-boundary";
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"source.png\"\r\nContent-Type: image/png\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(image_bytes.get_ref());
        body.extend_from_slice(
            format!(
                "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"to\"\r\n\r\npdf\r\n--{boundary}--\r\n"
            )
            .as_bytes(),
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/upload")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let upload: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let job_id = upload["job_id"].as_str().unwrap().to_string();

        let mut completed = false;
        for _ in 0..100 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/jobs/{job_id}/status"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status: serde_json::Value = serde_json::from_slice(
                &axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap(),
            )
            .unwrap();
            if status["status"] == "completed" {
                assert_eq!(status["progress"], 100);
                completed = true;
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(completed, "PDF conversion should complete");

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/jobs/{job_id}/download"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/pdf");
        let pdf = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(pdf.starts_with(b"%PDF"));

        let _ = fs::remove_dir_all(&data_dir);
    }
}
