use euoni_converter::{
    jobs::{Job, JobPayload, JobStatus},
    worker::execute_job,
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
use zip::ZipArchive;

const PDF_FIXTURE: &str = "test-files/PDF-PDF.pdf";

fn job(target: &str) -> Job {
    Job {
        id: "pdf-archive-test".to_string(),
        created_at: 0,
        status: JobStatus::Queued,
        input_name: "source.pdf".to_string(),
        output_name: None,
        payload: JobPayload {
            from: "pdf".to_string(),
            to: target.to_string(),
        },
        progress: 0,
        error: None,
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "euoni-pdf-{name}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn make_two_page_pdf(directory: &Path) -> PathBuf {
    let output = directory.join("two-pages.pdf");
    let status = Command::new("pdfunite")
        .args([PDF_FIXTURE, PDF_FIXTURE])
        .arg(&output)
        .status()
        .expect("pdfunite must be installed for PDF archive tests");
    assert!(status.success(), "pdfunite failed");
    output
}

fn archive_entries(path: &Path) -> Vec<String> {
    let file = fs::File::open(path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut names = Vec::new();
    for index in 0..archive.len() {
        names.push(archive.by_index(index).unwrap().name().to_string());
    }
    names
}

#[test]
fn pdf_to_png_zip_contains_one_numbered_page_for_one_page_pdf() {
    let directory = temp_dir("single-page");
    let input = Path::new(PDF_FIXTURE);
    let output = directory.join("converted.png.zip");

    execute_job(&job("png-zip"), input, &output).unwrap();

    assert_eq!(archive_entries(&output), vec!["page-1.png"]);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn pdf_to_jpg_zip_contains_all_numbered_pages() {
    let directory = temp_dir("multi-page");
    let input = make_two_page_pdf(&directory);
    let output = directory.join("converted.jpg.zip");

    execute_job(&job("jpg-zip"), &input, &output).unwrap();

    assert_eq!(archive_entries(&output), vec!["page-1.jpg", "page-2.jpg"]);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn pdf_to_html_zip_contains_referenced_resources_inside_the_archive() {
    let directory = temp_dir("html");
    let output = directory.join("converted.html.zip");

    execute_job(&job("html-zip"), Path::new(PDF_FIXTURE), &output).unwrap();

    let file = fs::File::open(&output).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut html = String::new();
    archive
        .by_name("index.html")
        .unwrap()
        .read_to_string(&mut html)
        .unwrap();
    for reference in referenced_files(&html) {
        assert!(
            archive.by_name(&reference).is_ok(),
            "HTML reference {reference} missing from archive"
        );
    }
    assert!(!archive.is_empty());
    let _ = fs::remove_dir_all(directory);
}

fn referenced_files(html: &str) -> Vec<String> {
    let mut references = Vec::new();
    for attribute in ["src=\"", "href=\"", "src='", "href='"] {
        let quote = attribute.as_bytes()[attribute.len() - 1] as char;
        let mut rest = html;
        while let Some(index) = rest.find(attribute) {
            let raw = &rest[index + attribute.len()..];
            let Some(end) = raw.find(quote) else {
                break;
            };
            let value = raw[..end].strip_prefix("./").unwrap_or(&raw[..end]);
            if !value.starts_with('#') && !value.contains("://") && !value.starts_with("data:") {
                references.push(value.to_string());
            }
            rest = &raw[end + 1..];
        }
    }
    references
}
