use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Storage {
    pub root: PathBuf,
    pub uploads: PathBuf,
    pub jobs: PathBuf,
    pub outputs: PathBuf,
    pub temp: PathBuf,
}

impl Storage {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let root = root.as_ref().to_path_buf();
        let uploads = root.join("uploads");
        let jobs = root.join("jobs");
        let outputs = root.join("outputs");
        let temp = root.join("temp");

        for path in [&uploads, &jobs, &outputs, &temp] {
            fs::create_dir_all(path)?;
        }

        Ok(Self {
            root,
            uploads,
            jobs,
            outputs,
            temp,
        })
    }

    pub fn create_job_dir(&self, job_id: &str) -> Result<PathBuf, std::io::Error> {
        let path = self.jobs.join(job_id);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn ensure_safe_filename(filename: &str) -> Result<String, String> {
        if filename.trim().is_empty() {
            return Err("invalid filename".to_string());
        }

        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err("unsafe filename".to_string());
        }

        let name = Path::new(filename)
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "invalid filename".to_string())?;

        if name == "." || name == ".." {
            return Err("unsafe filename".to_string());
        }

        Ok(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::Storage;

    #[test]
    fn creates_expected_storage_directories() {
        let temp_root = std::env::temp_dir().join(format!(
            "euoni-converter-storage-test-{}",
            std::process::id()
        ));

        let _ = std::fs::remove_dir_all(&temp_root);
        let storage = Storage::new(&temp_root).expect("storage should initialize");

        assert!(storage.uploads.exists());
        assert!(storage.jobs.exists());
        assert!(storage.outputs.exists());
        assert!(storage.temp.exists());

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn rejects_path_traversal_like_filenames() {
        let result = Storage::ensure_safe_filename("../secret.txt");
        assert!(result.is_err());
    }
}
