use std::path::{Path, PathBuf};
use std::process::Command;

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
        let result = Command::new(&self.binary)
            .args(&args)
            .output()
            .map_err(|err| format!("failed to start pdftoppm: {err}"))?;

        if !result.status.success() {
            return Err(format!(
                "pdftoppm failed: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
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
}

impl ImageToPdfRunner {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn build_args(input: &Path, output: &Path) -> Vec<String> {
        vec![
            input.to_string_lossy().to_string(),
            output.to_string_lossy().to_string(),
        ]
    }

    pub fn run(&self, input: &Path, output: &Path) -> Result<(), String> {
        let args = Self::build_args(input, output);
        let result = Command::new(&self.binary)
            .args(&args)
            .output()
            .map_err(|err| format!("failed to start ImageMagick: {err}"))?;

        if !result.status.success() {
            return Err(format!(
                "ImageMagick failed: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
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
