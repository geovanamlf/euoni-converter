use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PdfOperation {
    Merge,
    Split {
        ranges: Vec<PageRange>,
    },
    Reorder {
        pages: Vec<u32>,
    },
    Rotate {
        pages: Option<Vec<u32>>,
        angle: i32,
    },
    Compress {
        level: CompressionLevel,
    },
    Watermark {
        text: Option<String>,
        image: Option<String>,
        position: WatermarkPosition,
        opacity: f32,
        size: u16,
        scale: f32,
        pages: Option<Vec<u32>>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionLevel {
    Lossless,
    Balanced,
    Aggressive,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkPosition {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PdfManifest {
    pub operation: PdfOperation,
    pub files: Vec<String>,
    #[serde(default)]
    pub watermark_image: Option<String>,
}

pub fn execute(
    manifest_path: &Path,
    job_dir: &Path,
    output: &Path,
    max_output_size: u64,
) -> Result<(), String> {
    let manifest: PdfManifest = serde_json::from_slice(
        &fs::read(manifest_path)
            .map_err(|error| format!("failed to read PDF operation: {error}"))?,
    )
    .map_err(|error| format!("invalid PDF operation: {error}"))?;
    let inputs = manifest
        .files
        .iter()
        .map(|name| {
            crate::storage::Storage::ensure_safe_filename(name)
                .map(|safe| job_dir.join(safe))
                .map_err(|error| format!("invalid PDF input filename: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if inputs.is_empty() || inputs.len() > 32 {
        return Err("a PDF operation requires between 1 and 32 files".to_string());
    }
    for input in &inputs {
        validate_pdf(input)?;
    }

    match manifest.operation {
        PdfOperation::Merge => {
            if inputs.len() < 2 {
                return Err("merge requires at least two PDF files".to_string());
            }
            merge(&inputs, output, max_output_size)
        }
        PdfOperation::Split { ranges } => split(&inputs[0], &ranges, output, max_output_size),
        PdfOperation::Reorder { pages } => reorder(&inputs[0], &pages, output, max_output_size),
        PdfOperation::Rotate { pages, angle } => {
            rotate(&inputs[0], pages.as_deref(), angle, output, max_output_size)
        }
        PdfOperation::Compress { level } => compress(&inputs[0], level, output, max_output_size),
        PdfOperation::Watermark {
            text,
            image,
            position,
            opacity,
            size,
            scale,
            pages,
        } => {
            let image_path = manifest
                .watermark_image
                .as_deref()
                .map(|name| job_dir.join(name));
            if image.as_deref() != manifest.watermark_image.as_deref() {
                return Err("watermark image manifest is inconsistent".to_string());
            }
            watermark(
                &inputs[0],
                text.as_deref(),
                image_path.as_deref(),
                position,
                opacity,
                size,
                scale,
                pages.as_deref(),
                output,
                max_output_size,
            )
        }
    }
}

fn validate_pdf(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "PDF input is missing".to_string())?;
    if !metadata.is_file() {
        return Err("PDF input is not a regular file".to_string());
    }
    let output = crate::process::run_capture(std::ffi::OsStr::new("pdfinfo"), &[path.as_os_str()])?;
    if !output.status.success() {
        return Err("invalid or encrypted PDF".to_string());
    }
    Ok(())
}

fn page_count(path: &Path) -> Result<u32, String> {
    let output = crate::process::run_capture(std::ffi::OsStr::new("pdfinfo"), &[path.as_os_str()])?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .find_map(|line| line.strip_prefix("Pages:")?.trim().parse().ok())
        .ok_or_else(|| "unable to determine PDF page count".to_string())
}

fn run_gs(args: &[&std::ffi::OsStr], output: &Path, max: u64) -> Result<(), String> {
    let result = crate::process::run_limited(std::ffi::OsStr::new("gs"), args, output, max)?;
    if result.status.success() && output.is_file() {
        validate_pdf(output)
    } else {
        Err("Ghostscript failed to create a valid PDF".to_string())
    }
}

fn merge(inputs: &[PathBuf], output: &Path, max: u64) -> Result<(), String> {
    let mut args = vec![
        std::ffi::OsStr::new("-q"),
        std::ffi::OsStr::new("-dBATCH"),
        std::ffi::OsStr::new("-dNOPAUSE"),
        std::ffi::OsStr::new("-sDEVICE=pdfwrite"),
        std::ffi::OsStr::new("-dCompatibilityLevel=1.7"),
    ];
    let output_arg = format!("-sOutputFile={}", output.display());
    args.push(std::ffi::OsStr::new(&output_arg));
    args.extend(inputs.iter().map(|path| path.as_os_str()));
    run_gs(&args, output, max)
}

fn extract_page(input: &Path, page: u32, output: &Path, max: u64) -> Result<(), String> {
    let first = format!("-dFirstPage={page}");
    let last = format!("-dLastPage={page}");
    let output_arg = format!("-sOutputFile={}", output.display());
    run_gs(
        &[
            std::ffi::OsStr::new("-q"),
            std::ffi::OsStr::new("-dBATCH"),
            std::ffi::OsStr::new("-dNOPAUSE"),
            std::ffi::OsStr::new("-sDEVICE=pdfwrite"),
            std::ffi::OsStr::new(&first),
            std::ffi::OsStr::new(&last),
            std::ffi::OsStr::new(&output_arg),
            input.as_os_str(),
        ],
        output,
        max,
    )
}

fn split(input: &Path, ranges: &[PageRange], output: &Path, max: u64) -> Result<(), String> {
    let count = page_count(input)?;
    if ranges.is_empty() || ranges.len() > 32 {
        return Err("split requires between 1 and 32 ranges".to_string());
    }
    let temp = output
        .parent()
        .ok_or_else(|| "output has no parent directory".to_string())?
        .join(format!(".pdf-split-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp).map_err(|error| error.to_string())?;
    let result = (|| {
        let mut ranges_output = Vec::new();
        for (index, range) in ranges.iter().enumerate() {
            if range.start == 0 || range.end < range.start || range.end > count {
                return Err("split range is outside the document".to_string());
            }
            for page in range.start..=range.end {
                let page_path = temp.join(format!("page-{page}.pdf"));
                extract_page(input, page, &page_path, max)?;
            }
            let range_output = temp.join(format!("range-{}.pdf", index + 1));
            let pages = (range.start..=range.end)
                .map(|page| temp.join(format!("page-{page}.pdf")))
                .collect::<Vec<_>>();
            merge(&pages, &range_output, max)?;
            ranges_output.push(range_output);
        }
        let zip_file = File::create(output).map_err(|error| error.to_string())?;
        let mut archive = zip::ZipWriter::new(zip_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (index, _range) in ranges.iter().enumerate() {
            let name = format!("part-{}.pdf", index + 1);
            archive
                .start_file(name, options)
                .map_err(|error| error.to_string())?;
            let bytes = fs::read(&ranges_output[index]).map_err(|error| error.to_string())?;
            use std::io::Write;
            archive
                .write_all(&bytes)
                .map_err(|error| error.to_string())?;
        }
        archive.finish().map_err(|error| error.to_string())?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

fn reorder(input: &Path, pages: &[u32], output: &Path, max: u64) -> Result<(), String> {
    let count = page_count(input)?;
    if pages.is_empty() || pages.len() > 256 || pages.iter().any(|page| *page == 0 || *page > count)
    {
        return Err("reorder contains an invalid page sequence".to_string());
    }
    let temp = output
        .parent()
        .ok_or_else(|| "output has no parent directory".to_string())?
        .join(format!(".pdf-reorder-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp).map_err(|error| error.to_string())?;
    let result = (|| {
        let mut extracted = Vec::new();
        for (index, page) in pages.iter().enumerate() {
            let path = temp.join(format!("page-{index}.pdf"));
            extract_page(input, *page, &path, max)?;
            extracted.push(path);
        }
        merge(&extracted, output, max)
    })();
    let _ = fs::remove_dir_all(temp);
    result
}

fn rotate(
    input: &Path,
    pages: Option<&[u32]>,
    angle: i32,
    output: &Path,
    max: u64,
) -> Result<(), String> {
    if !matches!(angle, 90 | 180 | 270) {
        return Err("rotation angle must be 90, 180, or 270".to_string());
    }
    if let Some(pages) = pages {
        let count = page_count(input)?;
        if pages.is_empty() || pages.iter().any(|page| *page == 0 || *page > count) {
            return Err("rotation contains an invalid page".to_string());
        }
        let temp = output
            .parent()
            .ok_or_else(|| "output has no parent directory".to_string())?
            .join(format!(".pdf-rotate-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp).map_err(|error| error.to_string())?;
        let result = (|| {
            let selected = pages
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();
            let mut extracted = Vec::new();
            for page in 1..=count {
                let source = temp.join(format!("source-{page}.pdf"));
                let rotated = temp.join(format!("rotated-{page}.pdf"));
                extract_page(input, page, &source, max)?;
                if selected.contains(&page) {
                    rotate_file(&source, &rotated, angle, max)?;
                    extracted.push(rotated);
                } else {
                    extracted.push(source);
                }
            }
            merge(&extracted, output, max)
        })();
        let _ = fs::remove_dir_all(temp);
        return result;
    }
    rotate_file(input, output, angle, max)
}

fn rotate_file(input: &Path, output: &Path, angle: i32, max: u64) -> Result<(), String> {
    let orientation = format!("<</PageOrientation {angle}>> setpagedevice");
    let output_arg = format!("-sOutputFile={}", output.display());
    run_gs(
        &[
            std::ffi::OsStr::new("-q"),
            std::ffi::OsStr::new("-dBATCH"),
            std::ffi::OsStr::new("-dNOPAUSE"),
            std::ffi::OsStr::new("-sDEVICE=pdfwrite"),
            std::ffi::OsStr::new(&output_arg),
            std::ffi::OsStr::new("-c"),
            std::ffi::OsStr::new(&orientation),
            input.as_os_str(),
        ],
        output,
        max,
    )
}

fn compress(input: &Path, level: CompressionLevel, output: &Path, max: u64) -> Result<(), String> {
    let setting = match level {
        CompressionLevel::Lossless => "-dPDFSETTINGS=/default",
        CompressionLevel::Balanced => "-dPDFSETTINGS=/ebook",
        CompressionLevel::Aggressive => "-dPDFSETTINGS=/screen",
    };
    let output_arg = format!("-sOutputFile={}", output.display());
    run_gs(
        &[
            std::ffi::OsStr::new("-q"),
            std::ffi::OsStr::new("-dBATCH"),
            std::ffi::OsStr::new("-dNOPAUSE"),
            std::ffi::OsStr::new("-sDEVICE=pdfwrite"),
            std::ffi::OsStr::new(setting),
            std::ffi::OsStr::new(&output_arg),
            input.as_os_str(),
        ],
        output,
        max,
    )
}

#[allow(clippy::too_many_arguments)]
fn watermark(
    input: &Path,
    text: Option<&str>,
    image: Option<&Path>,
    position: WatermarkPosition,
    opacity: f32,
    size: u16,
    scale: f32,
    pages: Option<&[u32]>,
    output: &Path,
    max: u64,
) -> Result<(), String> {
    if text.map(str::trim).unwrap_or("").is_empty() == image.is_none()
        || text.is_some_and(|value| value.len() > 512)
        || !(0.05..=1.0).contains(&opacity)
    {
        return Err("watermark text or opacity is invalid".to_string());
    }
    if !(8..=144).contains(&size) || !(0.05..=1.0).contains(&scale) {
        return Err("watermark size must be between 8 and 144".to_string());
    }
    if let Some(image) = image {
        validate_png(image)?;
        return watermark_image_structural(
            input, image, position, opacity, scale, pages, output, max,
        );
    }
    if let Some(pages) = pages {
        let count = page_count(input)?;
        if pages.is_empty() || pages.iter().any(|page| *page == 0 || *page > count) {
            return Err("watermark contains an invalid page".to_string());
        }
    }
    let text = text.ok_or_else(|| "watermark text is required".to_string())?;
    let escaped = text
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)");
    let (x, y) = match position {
        WatermarkPosition::Center => ("300", "400"),
        WatermarkPosition::TopLeft => ("36", "800"),
        WatermarkPosition::TopRight => ("450", "800"),
        WatermarkPosition::BottomLeft => ("36", "36"),
        WatermarkPosition::BottomRight => ("450", "36"),
    };
    let script = format!(
        "/Helvetica findfont {size} scalefont setfont {opacity} setgray {x} {y} moveto ({escaped}) show"
    );
    let output_arg = format!("-sOutputFile={}", output.display());
    run_gs(
        &[
            std::ffi::OsStr::new("-q"),
            std::ffi::OsStr::new("-dBATCH"),
            std::ffi::OsStr::new("-dNOPAUSE"),
            std::ffi::OsStr::new("-sDEVICE=pdfwrite"),
            std::ffi::OsStr::new(&output_arg),
            std::ffi::OsStr::new("-c"),
            std::ffi::OsStr::new(&script),
            input.as_os_str(),
        ],
        output,
        max,
    )
}

fn validate_png(path: &Path) -> Result<(), String> {
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("png"))
    {
        return Err("watermark image must use the PNG extension".to_string());
    }
    let image = image::open(path).map_err(|_| "watermark image must be a valid PNG".to_string())?;
    if image.width() == 0 || image.height() == 0 || image.width() > 4096 || image.height() > 4096 {
        return Err("watermark image dimensions are invalid".to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn watermark_image_structural(
    input: &Path,
    image: &Path,
    position: WatermarkPosition,
    opacity: f32,
    scale: f32,
    pages: Option<&[u32]>,
    output: &Path,
    max: u64,
) -> Result<(), String> {
    let count = page_count(input)?;
    if let Some(pages) = pages
        && (pages.is_empty() || pages.iter().any(|page| *page == 0 || *page > count))
    {
        return Err("watermark contains an invalid page".to_string());
    }
    let temp = output
        .parent()
        .ok_or_else(|| "output has no parent directory".to_string())?
        .join(format!(".pdf-watermark-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp).map_err(|error| error.to_string())?;
    let result = (|| {
        let selected = pages.map(|values| {
            values
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
        });
        let gravity = match position {
            WatermarkPosition::Center => "center",
            WatermarkPosition::TopLeft => "northwest",
            WatermarkPosition::TopRight => "northeast",
            WatermarkPosition::BottomLeft => "southwest",
            WatermarkPosition::BottomRight => "southeast",
        };
        let opacity_arg = format!("{}%", (opacity * 100.0).round() as u16);
        let scale_arg = format!("{}%", (scale * 100.0).round() as u16);
        let prepared = temp.join("prepared-watermark.png");
        let prepare = crate::process::run_limited(
            std::ffi::OsStr::new("convert"),
            &[
                image.as_os_str(),
                std::ffi::OsStr::new("-resize"),
                std::ffi::OsStr::new(&scale_arg),
                std::ffi::OsStr::new("-channel"),
                std::ffi::OsStr::new("A"),
                std::ffi::OsStr::new("-evaluate"),
                std::ffi::OsStr::new("multiply"),
                std::ffi::OsStr::new(&opacity_arg),
                std::ffi::OsStr::new("+channel"),
                prepared.as_os_str(),
            ],
            &prepared,
            max,
        )?;
        if !prepare.status.success() || !prepared.is_file() {
            return Err("failed to prepare image watermark".to_string());
        }

        let mut current = temp.join("current.pdf");
        fs::copy(input, &current).map_err(|error| error.to_string())?;
        for page in 1..=count {
            if selected
                .as_ref()
                .is_some_and(|values| !values.contains(&page))
            {
                continue;
            }
            let (width, height) = page_size(input, page)?;
            let width_px = width.round().max(1.0) as u32;
            let height_px = height.round().max(1.0) as u32;
            let overlay_png = temp.join(format!("overlay-{page}.png"));
            let overlay_pdf = temp.join(format!("overlay-{page}.pdf"));
            let overlay = crate::process::run_limited(
                std::ffi::OsStr::new("convert"),
                &[
                    std::ffi::OsStr::new("-size"),
                    std::ffi::OsStr::new(&format!("{width_px}x{height_px}")),
                    std::ffi::OsStr::new("xc:none"),
                    prepared.as_os_str(),
                    std::ffi::OsStr::new("-gravity"),
                    std::ffi::OsStr::new(gravity),
                    std::ffi::OsStr::new("-geometry"),
                    std::ffi::OsStr::new("+24+24"),
                    std::ffi::OsStr::new("-composite"),
                    overlay_png.as_os_str(),
                ],
                &overlay_png,
                max,
            )?;
            if !overlay.status.success() || !overlay_png.is_file() {
                return Err("failed to create structural watermark overlay".to_string());
            }
            let page_size_arg = format!("{width}ptx{height}pt");
            let image_size_arg = format!("{width}ptx{height}pt");
            let overlay_pdf_result = crate::process::run_limited(
                std::ffi::OsStr::new("img2pdf"),
                &[
                    std::ffi::OsStr::new("--pagesize"),
                    std::ffi::OsStr::new(&page_size_arg),
                    std::ffi::OsStr::new("--imgsize"),
                    std::ffi::OsStr::new(&image_size_arg),
                    std::ffi::OsStr::new("-o"),
                    overlay_pdf.as_os_str(),
                    overlay_png.as_os_str(),
                ],
                &overlay_pdf,
                max,
            )?;
            if !overlay_pdf_result.status.success() || !overlay_pdf.is_file() {
                return Err("failed to encode structural watermark overlay".to_string());
            }
            let next = temp.join(format!("current-{page}.pdf"));
            let page_arg = page.to_string();
            let qpdf = crate::process::run_limited(
                std::ffi::OsStr::new("qpdf"),
                &[
                    current.as_os_str(),
                    std::ffi::OsStr::new("--overlay"),
                    overlay_pdf.as_os_str(),
                    std::ffi::OsStr::new("--from=1"),
                    std::ffi::OsStr::new(&format!("--to={page_arg}")),
                    std::ffi::OsStr::new("--"),
                    next.as_os_str(),
                ],
                &next,
                max,
            )?;
            if !qpdf.status.success() || !next.is_file() {
                return Err("failed to apply structural watermark overlay".to_string());
            }
            current = next;
        }
        fs::copy(&current, output).map_err(|error| error.to_string())?;
        validate_pdf(output)
    })();
    let _ = fs::remove_dir_all(&temp);
    result
}

fn page_size(path: &Path, page: u32) -> Result<(f64, f64), String> {
    let first = page.to_string();
    let last = page.to_string();
    let output = crate::process::run_capture(
        std::ffi::OsStr::new("pdfinfo"),
        &[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new(&first),
            std::ffi::OsStr::new("-l"),
            std::ffi::OsStr::new(&last),
            path.as_os_str(),
        ],
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| line.starts_with("Page size:") || line.starts_with("Page "))
        .ok_or_else(|| "unable to determine PDF page size".to_string())?;
    let mut tokens = line.split_whitespace();
    if tokens.find(|value| *value == "size:").is_none() {
        return Err("invalid PDF page size".to_string());
    }
    let width = tokens
        .next()
        .ok_or_else(|| "invalid PDF page size".to_string())?
        .parse::<f64>()
        .map_err(|_| "invalid PDF page size".to_string())?;
    let _separator = tokens
        .next()
        .ok_or_else(|| "invalid PDF page size".to_string())?;
    let height = tokens
        .next()
        .ok_or_else(|| "invalid PDF page size".to_string())?
        .parse::<f64>()
        .map_err(|_| "invalid PDF page size".to_string())?;
    let values = [width, height];
    if values.len() != 2
        || values
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err("invalid PDF page size".to_string());
    }
    Ok((values[0], values[1]))
}
