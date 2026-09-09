use crate::error::AppError;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub max_upload_size: u64,
    pub max_workers: usize,
    pub job_retention_hours: u64,
    pub data_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            max_upload_size: default_max_upload_size(),
            max_workers: default_max_workers(),
            job_retention_hours: default_job_retention_hours(),
            data_dir: default_data_dir(),
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self, AppError> {
        dotenvy::dotenv().ok();

        let host = std::env::var("HOST").unwrap_or_else(|_| default_host());
        let port = std::env::var("PORT")
            .unwrap_or_else(|_| default_port().to_string())
            .parse::<u16>()
            .map_err(|_| AppError::Config("PORT must be a valid u16".to_string()))?;
        let max_upload_size = std::env::var("MAX_UPLOAD_SIZE")
            .unwrap_or_else(|_| default_max_upload_size().to_string())
            .parse::<u64>()
            .map_err(|_| AppError::Config("MAX_UPLOAD_SIZE must be a valid u64".to_string()))?;
        let max_workers = std::env::var("MAX_WORKERS")
            .unwrap_or_else(|_| default_max_workers().to_string())
            .parse::<usize>()
            .map_err(|_| AppError::Config("MAX_WORKERS must be a valid usize".to_string()))?;
        let job_retention_hours = std::env::var("JOB_RETENTION_HOURS")
            .unwrap_or_else(|_| default_job_retention_hours().to_string())
            .parse::<u64>()
            .map_err(|_| AppError::Config("JOB_RETENTION_HOURS must be a valid u64".to_string()))?;
        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| default_data_dir());

        Ok(Self {
            host,
            port,
            max_upload_size,
            max_workers,
            job_retention_hours,
            data_dir,
        })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_max_upload_size() -> u64 {
    2 * 1024 * 1024 * 1024
}

fn default_max_workers() -> usize {
    2
}

fn default_job_retention_hours() -> u64 {
    24
}

fn default_data_dir() -> String {
    "./data".to_string()
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn settings_defaults_are_reasonable() {
        let settings = Settings::default();

        assert_eq!(settings.host, "0.0.0.0");
        assert_eq!(settings.port, 8080);
        assert_eq!(settings.max_workers, 2);
        assert_eq!(settings.job_retention_hours, 24);
        assert_eq!(settings.data_dir, "./data");
    }
}
