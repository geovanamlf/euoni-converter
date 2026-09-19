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
        let image_result = Command::new(&self.binary)
            .args([
                first_frame.as_str(),
                "-depth",
                "8",
                normalized.to_string_lossy().as_ref(),
            ])
            .output()
            .map_err(|err| format!("failed to start ImageMagick: {err}"))?;

        if !image_result.status.success() || !normalized.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(format!(
                "ImageMagick failed: {}",
                String::from_utf8_lossy(&image_result.stderr).trim()
            ));
        }

        let pdf_result = Command::new(&self.pdf_binary)
            .arg(&normalized)
            .arg("-o")
            .arg(output)
            .output();
        let _ = std::fs::remove_dir_all(&temp_dir);
        match pdf_result {
            Ok(result) if result.status.success() && output.exists() => Ok(()),
            Err(_) => {
                let fallback = Command::new(&self.binary)
                    .arg(input)
                    .arg(output)
                    .output()
                    .map_err(|err| format!("failed to start PDF converter: {err}"))?;
                if fallback.status.success() && output.exists() {
                    Ok(())
                } else {
                    Err(format!(
                        "PDF conversion failed: {}",
                        String::from_utf8_lossy(&fallback.stderr).trim()
                    ))
                }
            }
            Ok(result) => Err(format!(
                "img2pdf failed: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            )),
        }
    }

    pub fn run_image(&self, input: &Path, output: &Path) -> Result<(), String> {
        let first_frame = format!("{}[0]", input.to_string_lossy());
        let mut command = Command::new(&self.binary);
        command.arg(first_frame);

        if output.extension().and_then(|extension| extension.to_str()) == Some("ico") {
            command.args([
                "-resize",
                "256x256>",
                "-define",
                "icon:auto-resize=256,128,64,48,32,16",
            ]);
        }

        let result = command
            .arg(output)
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
