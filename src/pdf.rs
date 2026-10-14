use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct PdfRunner {
    binary: PathBuf,
}

impl PdfRunner {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn build_args(input: &Path, output: &Path) -> Vec<String> {
        let prefix = output.with_extension("");
        let format = match output.extension().and_then(|extension| extension.to_str()) {
            Some("jpg" | "jpeg") => "-jpeg",
            _ => "-png",
        };
        vec![
            "-singlefile".to_string(),
            format.to_string(),
            input.to_string_lossy().to_string(),
            prefix.to_string_lossy().to_string(),
        ]
    }

    pub fn run(&self, input: &Path, output: &Path) -> Result<(), String> {
        let args = Self::build_args(input, output);
        let refs = args.iter().map(std::ffi::OsStr::new).collect::<Vec<_>>();
        let result = crate::process::run(self.binary.as_os_str(), &refs)?;

        if !result.status.success() {
            return Err(format!("pdftoppm failed: {}", "pdftoppm failed"));
        }

        if !output.exists() {
            return Err("pdftoppm did not create the output file".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImageToPdfRunner {
    binary: PathBuf,
    pdf_binary: PathBuf,
}

impl ImageToPdfRunner {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            pdf_binary: PathBuf::from("img2pdf"),
        }
    }

    pub fn build_args(input: &Path, output: &Path) -> Vec<String> {
        vec![
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        ]
    }

    pub fn run(&self, input: &Path, output: &Path) -> Result<(), String> {
        let temp_dir = output
            .parent()
            .ok_or_else(|| "PDF output has no parent directory".to_string())?
            .join(format!(".image-conversion-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;
        let normalized = temp_dir.join("normalized.png");
        let first_frame = format!("{}[0]", input.to_string_lossy());
        let image_args = [
            std::ffi::OsStr::new(&first_frame),
            std::ffi::OsStr::new("-depth"),
            std::ffi::OsStr::new("8"),
            normalized.as_os_str(),
        ];
        let image_result = crate::process::run(self.binary.as_os_str(), &image_args)?;

        if !image_result.status.success() || !normalized.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(format!("ImageMagick failed: {}", "ImageMagick failed"));
        }

        let pdf_args = [
            normalized.as_os_str(),
            std::ffi::OsStr::new("-o"),
            output.as_os_str(),
        ];
        let pdf_result = crate::process::run(self.pdf_binary.as_os_str(), &pdf_args);
        let _ = std::fs::remove_dir_all(&temp_dir);
        match pdf_result {
            Ok(result) if result.status.success() && output.exists() => Ok(()),
            Err(_) => {
                let fallback_args = [input.as_os_str(), output.as_os_str()];
                let fallback = crate::process::run(self.binary.as_os_str(), &fallback_args)?;
                if fallback.status.success() && output.exists() {
                    Ok(())
                } else {
                    Err(format!(
                        "PDF conversion failed: {}",
                        "PDF conversion failed"
                    ))
                }
            }
            Ok(_) => Err("img2pdf failed".to_string()),
        }
    }

    pub fn run_image(&self, input: &Path, output: &Path) -> Result<(), String> {
        let first_frame = format!("{}[0]", input.to_string_lossy());
        if output.extension().and_then(|extension| extension.to_str()) == Some("ico") {
            let args = [
                std::ffi::OsStr::new(&first_frame),
                std::ffi::OsStr::new("-resize"),
                std::ffi::OsStr::new("256x256>"),
                std::ffi::OsStr::new("-define"),
                std::ffi::OsStr::new("icon:auto-resize=256,128,64,48,32,16"),
                output.as_os_str(),
            ];
            return crate::process::run(self.binary.as_os_str(), &args).and_then(|result| {
                if result.status.success() && output.exists() {
                    Ok(())
                } else {
                    Err("ImageMagick failed".to_string())
                }
            });
        }

        let args = [std::ffi::OsStr::new(&first_frame), output.as_os_str()];
        let result = crate::process::run(self.binary.as_os_str(), &args)?;

        if !result.status.success() {
            return Err(format!("ImageMagick failed: {}", "ImageMagick failed"));
        }

        if !output.exists() {
            return Err("ImageMagick did not create the output file".to_string());
        }

        Ok(())
    }

    pub fn system() -> Self {
        Self::new("convert")
    }
}

impl PdfRunner {
    pub fn system() -> Self {
        Self::new("pdftoppm")
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageToPdfRunner, PdfRunner};
    use std::path::Path;

    #[test]
    fn builds_single_page_png_arguments() {
        let args = PdfRunner::build_args(Path::new("source.pdf"), Path::new("converted.png"));

        assert_eq!(args, vec!["-singlefile", "-png", "source.pdf", "converted"]);
    }

    #[test]
    fn builds_single_page_jpg_arguments() {
        let args = PdfRunner::build_args(Path::new("source.pdf"), Path::new("converted.jpg"));

        assert_eq!(
            args,
            vec!["-singlefile", "-jpeg", "source.pdf", "converted"]
        );
    }

    #[test]
    fn builds_image_to_pdf_arguments() {
        let args =
            ImageToPdfRunner::build_args(Path::new("source.png"), Path::new("converted.pdf"));

        assert_eq!(args, vec!["source.png", "converted.pdf"]);
    }
}
