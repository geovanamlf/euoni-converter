use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JobPayload {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: String,
    pub created_at: u64,
    pub status: JobStatus,
    pub input_name: String,
    pub output_name: Option<String>,
    pub payload: JobPayload,
    pub progress: u8,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JobStore {
    connection: Arc<Mutex<Connection>>,
}

impl JobStore {
    pub fn new() -> Self {
        Self::open_in_memory().expect("failed to initialize in-memory job database")
    }

    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    fn open_in_memory() -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> rusqlite::Result<Self> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                input_name TEXT NOT NULL,
                output_name TEXT,
                source_format TEXT NOT NULL,
                target_format TEXT NOT NULL,
                progress INTEGER NOT NULL,
                error TEXT
            );",
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn create_job(&self, input_name: &str, from: &str, to: &str) -> Job {
        let id = Uuid::new_v4().simple().to_string();
        let job = Job {
            id: id.clone(),
            created_at: current_unix_time(),
            status: JobStatus::Queued,
            input_name: input_name.to_string(),
            output_name: None,
            payload: JobPayload {
                from: from.to_string(),
                to: to.to_string(),
            },
            progress: 0,
            error: None,
        };

        self.connection
            .lock()
            .expect("job database lock poisoned")
            .execute(
                "INSERT INTO jobs (id, created_at, status, input_name, output_name, source_format, target_format, progress, error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    &job.id, job.created_at, status_name(&job.status), &job.input_name,
                    job.output_name.as_deref(), &job.payload.from, &job.payload.to,
                    job.progress, job.error.as_deref()
                ],
            )
            .expect("failed to insert job");

        self.get(&job.id).expect("inserted job should be readable")
    }

    pub fn get(&self, id: &str) -> Option<Job> {
        self.connection
            .lock()
            .expect("job database lock poisoned")
            .query_row(
                "SELECT id, created_at, status, input_name, output_name, source_format, target_format, progress, error FROM jobs WHERE id = ?1",
                params![id], row_to_job,
            )
            .optional()
            .expect("failed to read job")
    }

    pub fn list(&self) -> Vec<Job> {
        let connection = self.connection.lock().expect("job database lock poisoned");
        let mut statement = connection
            .prepare("SELECT id, created_at, status, input_name, output_name, source_format, target_format, progress, error FROM jobs ORDER BY created_at")
            .expect("failed to prepare job list query");
        statement
            .query_map([], row_to_job)
            .expect("failed to list jobs")
            .map(|job| job.expect("failed to decode job"))
            .collect()
    }

    pub fn update_status(&self, id: &str, status: JobStatus) -> Option<Job> {
        self.connection
            .lock()
            .expect("job database lock poisoned")
            .execute(
                "UPDATE jobs SET status = ?1 WHERE id = ?2",
                params![status_name(&status), id],
            )
            .expect("failed to update job status");
        self.get(id)
    }

    pub fn recover_pending(&self) -> Vec<String> {
        let connection = self.connection.lock().expect("job database lock poisoned");
        connection
            .execute(
                "UPDATE jobs SET status = 'queued' WHERE status = 'processing'",
                [],
            )
            .expect("failed to recover processing jobs");
        let mut statement = connection
            .prepare("SELECT id FROM jobs WHERE status = 'queued' ORDER BY created_at")
            .expect("failed to prepare pending job query");
        statement
            .query_map([], |row| row.get(0))
            .expect("failed to list pending jobs")
            .map(|id| id.expect("failed to decode pending job id"))
            .collect()
    }

    pub fn set_progress(&self, id: &str, progress: u8) -> Option<Job> {
        self.connection
            .lock()
            .expect("job database lock poisoned")
            .execute(
                "UPDATE jobs SET progress = ?1 WHERE id = ?2",
                params![progress, id],
            )
            .expect("failed to update job progress");
        self.get(id)
    }

    pub fn set_output(&self, id: &str, output_name: &str) -> Option<Job> {
        self.connection
            .lock()
            .expect("job database lock poisoned")
            .execute(
                "UPDATE jobs SET output_name = ?1 WHERE id = ?2",
                params![output_name, id],
            )
            .expect("failed to update job output");
        self.get(id)
    }

    pub fn set_error(&self, id: &str, error: &str) -> Option<Job> {
        self.connection
            .lock()
            .expect("job database lock poisoned")
            .execute(
                "UPDATE jobs SET error = ?1, status = 'failed' WHERE id = ?2",
                params![error, id],
            )
            .expect("failed to update job error");
        self.get(id)
    }

    pub fn remove_expired(&self, now: u64, retention_seconds: u64) -> Vec<String> {
        let cutoff = now.saturating_sub(retention_seconds);
        let connection = self.connection.lock().expect("job database lock poisoned");
        let mut statement = connection.prepare("SELECT id FROM jobs WHERE created_at <= ?1 AND status IN ('completed', 'failed', 'cancelled')").expect("failed to prepare expired query");
        let ids: Vec<String> = statement
            .query_map(params![cutoff], |row| row.get(0))
            .expect("failed to find expired jobs")
            .map(|id| id.expect("failed to decode expired id"))
            .collect();
        for id in &ids {
            connection
                .execute("DELETE FROM jobs WHERE id = ?1", params![id])
                .expect("failed to delete expired job");
        }
        ids
    }
}

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    let status: String = row.get(2)?;
    Ok(Job {
        id: row.get(0)?,
        created_at: row.get(1)?,
        status: parse_status(&status),
        input_name: row.get(3)?,
        output_name: row.get(4)?,
        payload: JobPayload {
            from: row.get(5)?,
            to: row.get(6)?,
        },
        progress: row.get(7)?,
        error: row.get(8)?,
    })
}

fn status_name(status: &JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Processing => "processing",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

fn parse_status(status: &str) -> JobStatus {
    match status {
        "processing" => JobStatus::Processing,
        "completed" => JobStatus::Completed,
        "failed" => JobStatus::Failed,
        "cancelled" => JobStatus::Cancelled,
        _ => JobStatus::Queued,
    }
}

fn current_unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{JobStatus, JobStore};

    #[test]
    fn creates_job_in_queued_state() {
        let store = JobStore::new();
        let job = store.create_job("example.png", "png", "jpg");

        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.payload.from, "png");
        assert_eq!(job.payload.to, "jpg");
        assert_eq!(job.progress, 0);
        assert!(store.get(&job.id).is_some());
    }

    #[test]
    fn updates_progress_and_status() {
        let store = JobStore::new();
        let job = store.create_job("example.png", "png", "jpg");

        store.set_progress(&job.id, 42);
        store.update_status(&job.id, JobStatus::Processing);

        let saved = store.get(&job.id).expect("job should exist");
        assert_eq!(saved.progress, 42);
        assert_eq!(saved.status, JobStatus::Processing);
    }

    #[test]
    fn recovers_processing_jobs_as_queued() {
        let store = JobStore::new();
        let job = store.create_job("pending.png", "png", "jpg");
        store.update_status(&job.id, JobStatus::Processing);

        assert_eq!(store.recover_pending(), vec![job.id.clone()]);
        assert_eq!(store.get(&job.id).unwrap().status, JobStatus::Queued);
    }

    #[test]
    fn persists_jobs_when_reopened() {
        let path = std::env::temp_dir().join(format!("euoni-jobs-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let job = {
            let store = JobStore::open(&path).unwrap();
            store.create_job("saved.png", "png", "jpg")
        };

        let reopened = JobStore::open(&path).unwrap();
        assert_eq!(reopened.get(&job.id), Some(job));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn removes_only_expired_terminal_jobs() {
        let store = JobStore::new();
        let completed = store.create_job("done.png", "png", "jpg");
        store.update_status(&completed.id, JobStatus::Completed);
        let active = store.create_job("active.png", "png", "jpg");

        let expired = store.remove_expired(completed.created_at + 3601, 3600);

        assert_eq!(expired, vec![completed.id.clone()]);
        assert!(store.get(&completed.id).is_none());
        assert!(store.get(&active.id).is_some());
    }
}
