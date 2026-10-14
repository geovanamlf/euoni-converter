use std::path::{Path, PathBuf};

/// Encodes HEIC files through libheif after normalizing arbitrary image inputs.
#[derive(Debug, Clone)]
pub struct HeifRunner {
    convert_binary: PathBuf,
    encoder_binary: PathBuf,
}

impl HeifRunner {
    pub fn new(convert_binary: impl Into<PathBuf>, encoder_binary: impl Into<PathBuf>) -> Self {
        Self {
            convert_binary: convert_binary.into(),
            encoder_binary: encoder_binary.into(),
        }
    }

    pub fn build_encode_args(input: &Path, output: &Path) -> Vec<String> {
        let mut args = vec!["-o".to_string(), output.to_string_lossy().to_string()];
        if output.extension().and_then(|value| value.to_str()) == Some("avif") {
            args.push("--avif".to_string());
        }
        args.push(input.to_string_lossy().to_string());
        args
    }

    pub fn run(&self, input: &Path, output: &Path) -> Result<(), String> {
        let temp_dir = output
            .parent()
            .ok_or_else(|| "HEIC output has no parent directory".to_string())?
            .join(format!(".heic-conversion-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;
        let normalized = temp_dir.join("normalized.png");

        let first_frame = format!("{}[0]", input.to_string_lossy());
        let conversion_args = [std::ffi::OsStr::new(&first_frame), normalized.as_os_str()];
        let conversion = crate::process::run(self.convert_binary.as_os_str(), &conversion_args)?;
        if !conversion.status.success() || !normalized.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(format!("ImageMagick failed: {}", "ImageMagick failed"));
        }

        let encoding_args = Self::build_encode_args(&normalized, output);
        let encoding_args = encoding_args
            .iter()
            .map(std::ffi::OsStr::new)
            .collect::<Vec<_>>();
        let encoding = crate::process::run(self.encoder_binary.as_os_str(), &encoding_args);
        let _ = std::fs::remove_dir_all(&temp_dir);

        match encoding {
            Ok(result) if result.status.success() && output.exists() => Ok(()),
            Ok(_) => Err("heif-enc failed".to_string()),
            Err(err) => Err(err),
        }
    }

    pub fn system() -> Self {
        Self::new("convert", "heif-enc")
    }
}

#[cfg(test)]
mod tests {
    use super::HeifRunner;
    use std::path::Path;

    #[test]
    fn builds_heic_encoder_arguments() {
        let args = HeifRunner::build_encode_args(Path::new("source.png"), Path::new("result.heic"));

        assert_eq!(args, vec!["-o", "result.heic", "source.png"]);
    }
}
