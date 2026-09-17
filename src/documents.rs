use std::path::{Path, PathBuf};
use std::process::Command;

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
                return self.run_command(&self.pdftotext, &[input, output], "pdftotext");
            }
            if to == "html" {
                return self.run_pdf_to_html(input, output);
            }
            if matches!(to, "docx" | "odt" | "rtf") {
                return self.run_pdf_to_editable_document(input, output, to);
            }
        }

        let temp_dir = output
            .parent()
            .ok_or_else(|| "document output has no parent directory".to_string())?
            .join(".document-conversion");
        std::fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;

        self.run_libreoffice(input, output, &temp_dir, to)
    }

    fn run_pdf_to_editable_document(
        &self,
        input: &Path,
        output: &Path,
        to: &str,
    ) -> Result<(), String> {
        let temp_dir = output
            .parent()
            .ok_or_else(|| "document output has no parent directory".to_string())?
            .join(".document-conversion");
        std::fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;

        let extracted = temp_dir.join("extracted.txt");
        self.run_command(&self.pdftotext, &[input, &extracted], "pdftotext")?;
        let result = self.run_libreoffice(&extracted, output, &temp_dir, to);
        let _ = std::fs::remove_file(&extracted);
        let _ = std::fs::remove_dir(&temp_dir);
        result
    }

    fn run_libreoffice(
        &self,
        input: &Path,
        output: &Path,
        temp_dir: &Path,
        to: &str,
    ) -> Result<(), String> {
        let result = Command::new(&self.libreoffice)
            .args(["--headless", "--convert-to", to, "--outdir"])
            .arg(&temp_dir)
            .arg(input)
            .output()
            .map_err(|err| format!("failed to start LibreOffice: {err}"))?;

        if !result.status.success() {
            return Err(format!(
                "LibreOffice failed: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
        }

        let generated = temp_dir.join(format!(
            "{}.{}",
            input
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("document"),
            to
        ));
        if !generated.exists() {
            return Err(format!(
                "LibreOffice did not create the output file: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
        }

        std::fs::rename(&generated, output).map_err(|err| err.to_string())?;
        let _ = std::fs::remove_dir(&temp_dir);
        Ok(())
    }

    fn run_pdf_to_html(&self, input: &Path, output: &Path) -> Result<(), String> {
        let result = Command::new(&self.pdftohtml)
            .args(["-s", "-noframes"])
            .arg(input)
            .arg(output)
            .output()
            .map_err(|err| format!("failed to start pdftohtml: {err}"))?;
        if !result.status.success() {
            return Err(format!(
                "pdftohtml failed: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
        }
        if !output.exists() {
            return Err("pdftohtml did not create the output file".to_string());
        }
        Ok(())
    }

    fn run_command(&self, command: &Path, args: &[&Path], name: &str) -> Result<(), String> {
        let result = Command::new(command)
            .args(args)
            .output()
            .map_err(|err| format!("failed to start {name}: {err}"))?;
        if !result.status.success() {
            return Err(format!(
                "{name} failed: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
        }
        if !args[1].exists() {
            return Err(format!("{name} did not create the output file"));
        }
        Ok(())
    }
}
