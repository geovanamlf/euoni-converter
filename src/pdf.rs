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
        self.run_with_limit(input, output, u64::MAX)
    }

    pub fn run_with_limit(
        &self,
        input: &Path,
        output: &Path,
        max_output_size: u64,
    ) -> Result<(), String> {
        let args = Self::build_args(input, output);
        let refs = args.iter().map(std::ffi::OsStr::new).collect::<Vec<_>>();
        let result =
            crate::process::run_limited(self.binary.as_os_str(), &refs, output, max_output_size)?;

        if !result.status.success() {
            return Err(format!("pdftoppm failed: {}", "pdftoppm failed"));
        }

        if !output.exists() {
            return Err("pdftoppm did not create the output file".to_string());
        }

        Ok(())
    }

    pub fn run_all_pages_with_limit(
        &self,
        input: &Path,
        output: &Path,
        format: &str,
        max_output_size: u64,
    ) -> Result<usize, String> {
        let working_dir = output
            .parent()
            .ok_or_else(|| "PDF output has no parent directory".to_string())?
            .join(format!(".pdf-pages-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&working_dir).map_err(|error| error.to_string())?;
        let prefix = working_dir.join("page");
        let format_arg = match format {
            "jpg" => "-jpeg",
            "png" => "-png",
            _ => return Err("unsupported PDF image archive format".to_string()),
        };
        let args = [
            std::ffi::OsStr::new(format_arg),
            input.as_os_str(),
            prefix.as_os_str(),
        ];
        let first_page = working_dir.join(format!("page-1.{format}"));
        let result = crate::process::run_limited(
            self.binary.as_os_str(),
            &args,
            &first_page,
            max_output_size,
        );
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&working_dir);
                return Err(error);
            }
        };
        if !result.status.success() {
            let _ = std::fs::remove_dir_all(&working_dir);
            return Err("pdftoppm failed".to_string());
        }

        let pages = sorted_page_files(&working_dir, format)?;
        if pages.is_empty() {
            let _ = std::fs::remove_dir_all(&working_dir);
            return Err("pdftoppm did not create any page".to_string());
        }
        let total_size = pages.iter().try_fold(0_u64, |total, path| {
            let size = std::fs::metadata(path)
                .map_err(|error| error.to_string())?
                .len();
            total
                .checked_add(size)
                .filter(|size| *size <= max_output_size)
                .ok_or_else(|| "conversion output exceeded the configured limit".to_string())
        });
        if let Err(error) = total_size {
            let _ = std::fs::remove_dir_all(&working_dir);
            return Err(error);
        }

        let archive_result =
            crate::archive::create_zip_from_directory(&working_dir, output, max_output_size);
        let _ = std::fs::remove_dir_all(&working_dir);
        archive_result.map(|_| pages.len())
    }
}

fn sorted_page_files(directory: &Path, format: &str) -> Result<Vec<PathBuf>, String> {
    let extension = if format == "jpg" { "jpg" } else { "png" };
    let mut pages = std::fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    pages.retain(|path| path.extension().and_then(|value| value.to_str()) == Some(extension));
    pages.sort_by_key(|path| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .and_then(|value| value.rsplit('-').next())
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    Ok(pages)
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
        self.run_with_limit(input, output, u64::MAX)
    }

    pub fn run_with_limit(
        &self,
        input: &Path,
        output: &Path,
        max_output_size: u64,
    ) -> Result<(), String> {
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
        let image_result = crate::process::run_limited(
            self.binary.as_os_str(),
            &image_args,
            &normalized,
            max_output_size,
        )?;

        if !image_result.status.success() || !normalized.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(format!("ImageMagick failed: {}", "ImageMagick failed"));
        }

        let pdf_args = [
            normalized.as_os_str(),
            std::ffi::OsStr::new("-o"),
            output.as_os_str(),
        ];
        let pdf_result = crate::process::run_limited(
            self.pdf_binary.as_os_str(),
            &pdf_args,
            output,
            max_output_size,
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
        match pdf_result {
            Ok(result) if result.status.success() && output.exists() => Ok(()),
            Err(_) => {
                let fallback_args = [input.as_os_str(), output.as_os_str()];
                let fallback = crate::process::run_limited(
                    self.binary.as_os_str(),
                    &fallback_args,
                    output,
                    max_output_size,
                )?;
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
        self.run_image_with_limit(input, output, u64::MAX)
    }

    pub fn run_image_with_limit(
        &self,
        input: &Path,
        output: &Path,
        max_output_size: u64,
    ) -> Result<(), String> {
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
            return crate::process::run_limited(
                self.binary.as_os_str(),
                &args,
                output,
                max_output_size,
            )
            .and_then(|result| {
                if result.status.success() && output.exists() {
                    Ok(())
                } else {
                    Err("ImageMagick failed".to_string())
                }
            });
        }

        let args = [std::ffi::OsStr::new(&first_frame), output.as_os_str()];
        let result =
            crate::process::run_limited(self.binary.as_os_str(), &args, output, max_output_size)?;

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
