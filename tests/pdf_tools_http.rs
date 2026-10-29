use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use euoni_converter::app::create_app;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::Value;
use std::{io::Cursor, path::PathBuf, time::Duration};
use tower::ServiceExt;

const PDF: &[u8] = include_bytes!("../test-files/PDF-PDF.pdf");

fn data_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "euoni-http-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn png_fixture() -> Vec<u8> {
    let image = RgbaImage::from_pixel(24, 24, Rgba([255, 0, 0, 180]));
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn multipart(
    operation: &str,
    files: &[(&str, &[u8])],
    watermark: Option<&[u8]>,
) -> (String, Vec<u8>) {
    let boundary = format!("euoni-http-{}", uuid::Uuid::new_v4());
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"operation\"\r\nContent-Type: application/json\r\n\r\n{operation}\r\n"
        )
        .as_bytes(),
    );
    for (name, bytes) in files {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/pdf\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    if let Some(bytes) = watermark {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"watermark_image\"; filename=\"mark.png\"\r\nContent-Type: image/png\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn operation(
    app: &axum::Router,
    json: &str,
    files: &[(&str, &[u8])],
    watermark: Option<&[u8]>,
) -> (String, String, Vec<u8>) {
    let (content_type, body) = multipart(json, files, watermark);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/pdf/operations")
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (
        json["job_id"].as_str().unwrap().to_string(),
        json["token"].as_str().unwrap().to_string(),
        json.to_string().into_bytes(),
    )
}

async fn completed(app: &axum::Router, id: &str, token: &str) -> Value {
    for _ in 0..100 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/jobs/{id}/status"))
                    .header("x-euoni-job-token", token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        if status["status"] == "completed" || status["status"] == "failed" {
            return status;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("PDF job did not finish");
}

async fn download(app: &axum::Router, id: &str, token: &str) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/jobs/{id}/download"))
                .header("x-euoni-job-token", token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, bytes)
}

#[tokio::test]
async fn all_six_pdf_operations_work_over_authenticated_http() {
    let data = data_dir();
    let app = create_app(euoni_converter::config::Settings {
        data_dir: data.to_string_lossy().to_string(),
        ..Default::default()
    });
    let png = png_fixture();
    let cases = [
        (
            r#"{"type":"merge"}"#,
            vec![("a.pdf", PDF), ("b.pdf", PDF)],
            None,
        ),
        (
            r#"{"type":"split","ranges":[{"start":1,"end":1}]}"#,
            vec![("source.pdf", PDF)],
            None,
        ),
        (
            r#"{"type":"reorder","pages":[1,1]}"#,
            vec![("source.pdf", PDF)],
            None,
        ),
        (
            r#"{"type":"rotate","angle":90}"#,
            vec![("source.pdf", PDF)],
            None,
        ),
        (
            r#"{"type":"compress","level":"balanced"}"#,
            vec![("source.pdf", PDF)],
            None,
        ),
        (
            r#"{"type":"watermark","text":"EUONI","position":"center","opacity":0.4,"size":24,"scale":0.25}"#,
            vec![("source.pdf", PDF)],
            None,
        ),
        (
            r#"{"type":"watermark","image":"mark.png","position":"bottom_right","opacity":0.5,"size":24,"scale":0.25}"#,
            vec![("source.pdf", PDF)],
            Some(png.as_slice()),
        ),
    ];

    for (json, files, watermark) in cases {
        let (id, token, _) = operation(&app, json, &files, watermark).await;
        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/jobs/{id}/status"))
                    .header("x-euoni-job-token", "wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::NOT_FOUND);
        let status = completed(&app, &id, &token).await;
        assert_eq!(status["status"], "completed", "{json}: {status}");
        let (download_status, bytes) = download(&app, &id, &token).await;
        assert_eq!(download_status, StatusCode::OK);
        assert!(
            bytes.starts_with(b"%PDF") || bytes.starts_with(b"PK"),
            "{json}"
        );
        let unauthorized_download = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/jobs/{id}/download"))
                    .header("x-euoni-job-token", "wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized_download.status(), StatusCode::NOT_FOUND);
    }
    std::fs::remove_dir_all(data).unwrap();
}

#[tokio::test]
async fn pdf_http_upload_limits_and_invalid_operations_are_rejected() {
    let data = data_dir();
    let app = create_app(euoni_converter::config::Settings {
        data_dir: data.to_string_lossy().to_string(),
        max_upload_size: 128,
        ..Default::default()
    });
    let (content_type, body) = multipart(
        r#"{"type":"merge"}"#,
        &[("a.pdf", PDF), ("b.pdf", PDF)],
        None,
    );
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/pdf/operations")
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let remaining = std::fs::read_dir(data.join("temp")).unwrap().count();
    assert_eq!(remaining, 0, "rejected upload left temporary files");
    std::fs::remove_dir_all(data).unwrap();
}
