use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    Image,
    Audio,
    Video,
    Pdf,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct UploadedFile {
    pub original_name: String,
    pub safe_name: String,
    pub saved_path: PathBuf,
    pub size_bytes: u64,
    pub kind: FileKind,
}

impl UploadedFile {
    pub fn detect_kind(file_path: &Path) -> FileKind {
        let bytes = std::fs::read(file_path).unwrap_or_default();

        if bytes.is_empty() {
            return FileKind::Unknown;
        }

        if bytes.starts_with(b"%PDF") {
            return FileKind::Pdf;
        }

        let kind = infer::get(&bytes);
        match kind.map(|info| info.matcher_type()) {
            Some(infer::MatcherType::Image) => FileKind::Image,
            Some(infer::MatcherType::Video) => FileKind::Video,
            Some(infer::MatcherType::Audio) => FileKind::Audio,
            _ => FileKind::Unknown,
        }
    }
}

pub fn save_uploaded_file(
    bytes: &[u8],
    destination: &Path,
    original_name: &str,
    max_size: u64,
) -> Result<UploadedFile, String> {
    let safe_name = crate::storage::Storage::ensure_safe_filename(original_name)
        .map_err(|err| err.to_string())?;

    if bytes.len() as u64 > max_size {
        return Err("file too large".to_string());
    }

    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let mut file = File::create(destination).map_err(|err| err.to_string())?;
    file.write_all(bytes).map_err(|err| err.to_string())?;

    let kind = UploadedFile::detect_kind(destination);

    Ok(UploadedFile {
        original_name: original_name.to_string(),
        safe_name,
        saved_path: destination.to_path_buf(),
        size_bytes: bytes.len() as u64,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::{FileKind, save_uploaded_file};
    use std::fs;

    #[test]
    fn saves_a_file_and_detects_kind() {
        let temp_dir =
            std::env::temp_dir().join(format!("euoni-upload-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let destination = temp_dir.join("sample.png");
        let png_bytes = [137, 80, 78, 71, 13, 10, 26, 10];
        let result = save_uploaded_file(&png_bytes, &destination, "sample.png", 10_000).unwrap();

        assert_eq!(result.safe_name, "sample.png");
        assert!(destination.exists());
        assert_eq!(result.kind, FileKind::Image);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
