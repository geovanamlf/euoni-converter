use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

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
        let mut file = match File::open(file_path) {
            Ok(file) => file,
            Err(_) => return FileKind::Unknown,
        };
        let mut bytes = vec![0; 64 * 1024];
        let length = match file.read(&mut bytes) {
            Ok(length) => length,
            Err(_) => return FileKind::Unknown,
        };
        bytes.truncate(length);

        if bytes.is_empty() {
            return FileKind::Unknown;
        }

        if bytes.starts_with(b"%PDF") {
            return FileKind::Pdf;
        }

        if bytes
            .windows(4)
            .any(|window| window.eq_ignore_ascii_case(b"<svg"))
        {
            return FileKind::Image;
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

pub async fn save_multipart_file(
    mut field: axum::extract::multipart::Field<'_>,
    destination: &Path,
    original_name: &str,
    max_size: u64,
) -> Result<UploadedFile, String> {
    let safe_name = crate::storage::Storage::ensure_safe_filename(original_name)
        .map_err(|err| err.to_string())?;
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|err| err.to_string())?;
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|err| err.to_string())?;
    let mut size_bytes = 0u64;

    while let Some(chunk) = field.chunk().await.map_err(|err| err.to_string())? {
        size_bytes = size_bytes
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| "file too large".to_string())?;
        if size_bytes > max_size {
            let _ = tokio::fs::remove_file(destination).await;
            return Err("file too large".to_string());
        }
        file.write_all(&chunk)
            .await
            .map_err(|err| err.to_string())?;
    }
    file.flush().await.map_err(|err| err.to_string())?;
    let kind = UploadedFile::detect_kind(destination);

    Ok(UploadedFile {
        original_name: original_name.to_string(),
        safe_name,
        saved_path: destination.to_path_buf(),
        size_bytes,
        kind,
    })
}

pub fn validate_uploaded_format(path: &Path, extension: &str) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|_| "invalid uploaded file".to_string())?;
    if metadata.len() == 0 {
        return Err("empty file".to_string());
    }

    let extension = extension.to_ascii_lowercase();
    let kind = UploadedFile::detect_kind(path);
    let image_extensions = [
        "png", "jpg", "jpeg", "webp", "bmp", "tiff", "tif", "gif", "svg", "ico", "heic", "avif",
    ];
    let document_extensions = ["docx", "txt", "html", "odt", "rtf"];

    if extension == "pdf" {
        return (kind == FileKind::Pdf)
            .then_some(())
            .ok_or_else(|| "file content does not match extension".to_string());
    }
    if image_extensions.contains(&extension.as_str()) {
        return (kind == FileKind::Image)
            .then_some(())
            .ok_or_else(|| "file content does not match extension".to_string());
    }
    if document_extensions.contains(&extension.as_str()) {
        let mut file = File::open(path).map_err(|_| "invalid uploaded file".to_string())?;
        let mut prefix = [0; 8];
        let read = file
            .read(&mut prefix)
            .map_err(|_| "invalid uploaded file".to_string())?;
        let valid = match extension.as_str() {
            "txt" | "html" => read > 0,
            "rtf" => prefix[..read].starts_with(b"{\\rtf"),
            "docx" | "odt" => prefix[..read].starts_with(b"PK"),
            _ => false,
        };
        return valid
            .then_some(())
            .ok_or_else(|| "file content does not match extension".to_string());
    }
    Ok(())
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
