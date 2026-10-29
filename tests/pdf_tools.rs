use euoni_converter::pdf_tools::{
    CompressionLevel, PageRange, PdfManifest, PdfOperation, WatermarkPosition, execute,
};
use image::{Rgba, RgbaImage};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
use zip::ZipArchive;

const FIXTURE: &str = "test-files/PDF-PDF.pdf";

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "euoni-pdf-tool-{name}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn setup(name: &str, operation: PdfOperation, files: &[(&str, &Path)]) -> (PathBuf, PathBuf) {
    let dir = temp_dir(name);
    for (file_name, source) in files {
        fs::copy(source, dir.join(file_name)).unwrap();
    }
    let manifest = dir.join("operation.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&PdfManifest {
            operation,
            files: files.iter().map(|(name, _)| (*name).to_string()).collect(),
            watermark_image: None,
        })
        .unwrap(),
    )
    .unwrap();
    (dir.clone(), manifest)
}

fn pages(path: &Path) -> u32 {
    let output = std::process::Command::new("pdfinfo")
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Pages:")?.trim().parse().ok())
        .unwrap()
}

#[test]
fn structural_operations_produce_expected_page_counts() {
    let fixture = Path::new(FIXTURE);
    let (dir, manifest) = setup(
        "merge",
        PdfOperation::Merge,
        &[("a.pdf", fixture), ("b.pdf", fixture)],
    );
    let merged = dir.join("merged.pdf");
    execute(&manifest, &dir, &merged, 64 * 1024 * 1024).unwrap();
    assert_eq!(pages(&merged), pages(fixture) * 2);
    fs::remove_dir_all(dir).unwrap();

    let (dir, manifest) = setup(
        "reorder",
        PdfOperation::Reorder { pages: vec![1, 1] },
        &[("source.pdf", fixture)],
    );
    let reordered = dir.join("reordered.pdf");
    execute(&manifest, &dir, &reordered, 64 * 1024 * 1024).unwrap();
    assert_eq!(pages(&reordered), 2);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn split_creates_valid_pdf_entries_and_other_operations_are_valid() {
    let fixture = Path::new(FIXTURE);
    let (dir, manifest) = setup(
        "split",
        PdfOperation::Split {
            ranges: vec![PageRange { start: 1, end: 1 }],
        },
        &[("source.pdf", fixture)],
    );
    let archive_path = dir.join("parts.zip");
    execute(&manifest, &dir, &archive_path, 64 * 1024 * 1024).unwrap();
    let archive_file = fs::File::open(&archive_path).unwrap();
    let mut archive = ZipArchive::new(archive_file).unwrap();
    let mut entry = archive.by_name("part-1.pdf").unwrap();
    assert!(entry.read_to_end(&mut Vec::new()).is_ok());
    fs::remove_dir_all(dir).unwrap();

    for (name, operation) in [
        (
            "compress",
            PdfOperation::Compress {
                level: CompressionLevel::Balanced,
            },
        ),
        (
            "rotate",
            PdfOperation::Rotate {
                pages: None,
                angle: 90,
            },
        ),
        (
            "watermark",
            PdfOperation::Watermark {
                text: Some("Euoni".to_string()),
                image: None,
                position: WatermarkPosition::Center,
                opacity: 0.35,
                size: 24,
                scale: 0.25,
                pages: None,
            },
        ),
    ] {
        let (dir, manifest) = setup(name, operation, &[("source.pdf", fixture)]);
        let output = dir.join("output.pdf");
        execute(&manifest, &dir, &output, 64 * 1024 * 1024).unwrap();
        assert_eq!(pages(&output), pages(fixture));
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn image_watermark_produces_a_valid_pdf() {
    let fixture = Path::new(FIXTURE);
    let dir = temp_dir("image-watermark");
    fs::copy(fixture, dir.join("source.pdf")).unwrap();
    let image = RgbaImage::from_pixel(24, 24, Rgba([255, 0, 0, 180]));
    let image_path = dir.join("mark.png");
    image.save(&image_path).unwrap();
    let manifest = dir.join("operation.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&PdfManifest {
            operation: PdfOperation::Watermark {
                text: None,
                image: Some("mark.png".to_string()),
                position: WatermarkPosition::BottomRight,
                opacity: 0.5,
                size: 24,
                scale: 0.25,
                pages: None,
            },
            files: vec!["source.pdf".to_string()],
            watermark_image: Some("mark.png".to_string()),
        })
        .unwrap(),
    )
    .unwrap();
    let output = dir.join("output.pdf");
    execute(&manifest, &dir, &output, 64 * 1024 * 1024).unwrap();
    assert_eq!(pages(&output), pages(fixture));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn image_watermark_preserves_searchable_text_and_page_structure() {
    let fixture = Path::new(FIXTURE);
    let dir = temp_dir("image-watermark-fidelity");
    let source = dir.join("source.pdf");
    fs::copy(fixture, &source).unwrap();
    let source_text = Command::new("pdftotext")
        .arg(&source)
        .arg("-")
        .output()
        .unwrap();
    assert!(source_text.status.success());
    let image = RgbaImage::from_pixel(24, 24, Rgba([0, 0, 255, 96]));
    let image_path = dir.join("mark.png");
    image.save(&image_path).unwrap();
    let manifest = dir.join("operation.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&PdfManifest {
            operation: PdfOperation::Watermark {
                text: None,
                image: Some("mark.png".to_string()),
                position: WatermarkPosition::TopLeft,
                opacity: 0.5,
                size: 24,
                scale: 0.5,
                pages: None,
            },
            files: vec!["source.pdf".to_string()],
            watermark_image: Some("mark.png".to_string()),
        })
        .unwrap(),
    )
    .unwrap();
    let output = dir.join("output.pdf");
    execute(&manifest, &dir, &output, 64 * 1024 * 1024).unwrap();
    let output_text = Command::new("pdftotext")
        .arg(&output)
        .arg("-")
        .output()
        .unwrap();
    assert!(output_text.status.success());
    assert_eq!(output_text.stdout, source_text.stdout);
    assert_eq!(pages(&output), pages(&source));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn compression_levels_are_executed_for_text_image_and_mixed_pdfs() {
    let dir = temp_dir("compression-levels");
    let text = dir.join("text.pdf");
    let image = dir.join("image.pdf");
    let mixed = dir.join("mixed.pdf");
    let text_status = Command::new("convert")
        .args([
            "-size",
            "1200x900",
            "xc:white",
            "-fill",
            "black",
            "-pointsize",
            "36",
            "-gravity",
            "center",
            "-annotate",
            "0",
            "Euoni PDF text validation",
        ])
        .arg(&text)
        .status()
        .unwrap();
    assert!(text_status.success());
    let image_status = Command::new("convert")
        .args(["-size", "1200x900", "gradient:navy-yellow"])
        .arg(&image)
        .status()
        .unwrap();
    assert!(image_status.success());
    let mixed_status = Command::new("pdfunite")
        .args([text.as_os_str(), image.as_os_str()])
        .arg(&mixed)
        .status()
        .unwrap();
    assert!(mixed_status.success());

    let mut sizes = Vec::new();
    for (name, source) in [("text", &text), ("image", &image), ("mixed", &mixed)] {
        for level in [
            CompressionLevel::Lossless,
            CompressionLevel::Balanced,
            CompressionLevel::Aggressive,
        ] {
            let output = dir.join(format!(
                "{name}-{}.pdf",
                serde_json::to_string(&level).unwrap()
            ));
            let manifest = dir.join(format!("{name}-operation.json"));
            fs::write(
                &manifest,
                serde_json::to_vec(&PdfManifest {
                    operation: PdfOperation::Compress { level },
                    files: vec![source.file_name().unwrap().to_string_lossy().to_string()],
                    watermark_image: None,
                })
                .unwrap(),
            )
            .unwrap();
            execute(&manifest, &dir, &output, 128 * 1024 * 1024).unwrap();
            assert!(pages(&output) >= 1);
            sizes.push(fs::metadata(output).unwrap().len());
        }
    }
    eprintln!("compression output sizes: {sizes:?}");
    assert!(sizes.iter().all(|size| *size > 0));
    assert!(sizes.windows(2).any(|pair| pair[0] != pair[1]));
    fs::remove_dir_all(dir).unwrap();
}
