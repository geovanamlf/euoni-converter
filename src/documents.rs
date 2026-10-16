use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct DocumentRunner {
    libreoffice: PathBuf,
    pdftotext: PathBuf,
    pdftohtml: PathBuf,
}

impl DocumentRunner {
    pub fn system() -> Self {
        Self {
            libreoffice: PathBuf::from("libreoffice"),
            pdftotext: PathBuf::from("pdftotext"),
            pdftohtml: PathBuf::from("pdftohtml"),
        }
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
        let from = input
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default();
        let to = output
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default();

        if from == "pdf" {
            if to == "txt" {
                return self.run_command(
                    &self.pdftotext,
                    &[input, output],
                    "pdftotext",
                    max_output_size,
                );
            }
            if to == "html" {
                return self.run_pdf_to_html(input, output, max_output_size);
            }
            if matches!(to, "docx" | "odt" | "rtf") {
                return self.run_pdf_to_editable_document(input, output, to, max_output_size);
            }
        }

        let temp_dir = output
            .parent()
            .ok_or_else(|| "document output has no parent directory".to_string())?
            .join(".document-conversion");
        std::fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;

        self.run_libreoffice(input, output, &temp_dir, to, max_output_size)
    }

    fn run_pdf_to_editable_document(
        &self,
        input: &Path,
        output: &Path,
        to: &str,
        max_output_size: u64,
    ) -> Result<(), String> {
        let temp_dir = output
            .parent()
            .ok_or_else(|| "document output has no parent directory".to_string())?
            .join(".document-conversion");
        std::fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;

        let extracted = temp_dir.join("extracted.txt");
        self.run_command(
            &self.pdftotext,
            &[input, &extracted],
            "pdftotext",
            max_output_size,
        )?;
        let result = self.run_libreoffice(&extracted, output, &temp_dir, to, max_output_size);
        let _ = std::fs::remove_file(&extracted);
        let _ = std::fs::remove_dir(temp_dir);
        result
    }

    fn run_libreoffice(
        &self,
        input: &Path,
        output: &Path,
        temp_dir: &Path,
        to: &str,
        max_output_size: u64,
    ) -> Result<(), String> {
        let args = [
            std::ffi::OsStr::new("--headless"),
            std::ffi::OsStr::new("--convert-to"),
            std::ffi::OsStr::new(to),
            std::ffi::OsStr::new("--outdir"),
            temp_dir.as_os_str(),
            input.as_os_str(),
        ];
        let generated = temp_dir.join(format!(
            "{}.{}",
            input
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("document"),
            to
        ));
        let result = crate::process::run_limited(
            self.libreoffice.as_os_str(),
            &args,
            &generated,
            max_output_size,
        )?;

        if !result.status.success() {
            return Err("LibreOffice failed".to_string());
        }

        if !generated.exists() {
            return Err("LibreOffice did not create the output file".to_string());
        }

        std::fs::rename(&generated, output).map_err(|err| err.to_string())?;
        let _ = std::fs::remove_dir(temp_dir);
        Ok(())
    }

    fn run_pdf_to_html(
        &self,
        input: &Path,
        output: &Path,
        max_output_size: u64,
    ) -> Result<(), String> {
        let args = [
            std::ffi::OsStr::new("-s"),
            std::ffi::OsStr::new("-noframes"),
            input.as_os_str(),
            output.as_os_str(),
        ];
        let result = crate::process::run_limited(
            self.pdftohtml.as_os_str(),
            &args,
            output,
            max_output_size,
        )?;
        if !result.status.success() {
            return Err("pdftohtml failed".to_string());
        }
        if !output.exists() {
            return Err("pdftohtml did not create the output file".to_string());
        }
        Ok(())
    }

    fn run_command(
        &self,
        command: &Path,
        args: &[&Path],
        name: &str,
        max_output_size: u64,
    ) -> Result<(), String> {
        let expected_output = args[1];
        let converted_args = args.iter().map(|path| path.as_os_str()).collect::<Vec<_>>();
        let result = crate::process::run_limited(
            command.as_os_str(),
            &converted_args,
            expected_output,
            max_output_size,
        )?;
        if !result.status.success() {
            return Err(format!("{name} failed"));
        }
        if !expected_output.exists() {
            return Err(format!("{name} did not create the output file"));
        }
        Ok(())
    }
}
