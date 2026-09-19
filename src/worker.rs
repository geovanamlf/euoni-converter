use std::path::Path;

use tokio::sync::mpsc;

use crate::{app::AppState, jobs::Job, jobs::JobStatus};

/// Every format that can participate in a declared conversion.  This list is
/// also used to enumerate the production conversion matrix for tests and UIs.
const CONVERSION_FORMATS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "bmp", "tiff", "gif", "svg", "ico", "heic", "avif", "pdf",
    "docx", "txt", "html", "odt", "rtf", "wav", "mp3", "ogg", "flac", "aac", "m4a", "mp4", "mkv",
    "mov", "avi", "webm",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SupportedConversion {
    pub from: &'static str,
    pub to: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionKind {
    Image(ImageConversion),
    Heic,
    Audio(AudioConversion),
    Video(VideoConversion),
    Pdf(PdfConversion),
    Document(DocumentConversion),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageConversion {
    PngToJpg,
    WebpToJpg,
    JpgToPng,
    WebpToPng,
    PngToWebp,
    JpgToWebp,
    BmpToJpg,
    BmpToPng,
    BmpToWebp,
    TiffToJpg,
    TiffToPng,
    TiffToWebp,
    GifToJpg,
    GifToPng,
    GifToWebp,
    SvgToPng,
    SvgToJpg,
    SvgToWebp,
    IcoToPng,
    IcoToJpg,
    HeicToJpg,
    HeicToPng,
    HeicToWebp,
    AvifToJpg,
    AvifToPng,
    AvifToWebp,
    /// Formats handled by ImageMagick rather than the in-process image codec.
    ImageMagick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioConversion {
    WavToMp3,
    Mp3ToWav,
    OtherToMp3,
    OtherToWav,
    Mp4ToMp3,
    Mp4ToWav,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoConversion {
    ToMp4,
    ToMp3,
    ToWav,
    MkvToMp4,
    MovToMp4,
    AviToMp4,
    WebmToMp4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfConversion {
    PdfToPng,
    PdfToJpg,
    PngToPdf,
    JpgToPdf,
    WebpToPdf,
    BmpToPdf,
    TiffToPdf,
    GifToPdf,
    SvgToPdf,
    HeicToPdf,
    AvifToPdf,
    IcoToPdf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentConversion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPlan {
    pub job: Job,
    pub conversion: ConversionKind,
}

fn conversion_for_formats(from: &str, to: &str) -> Result<ConversionKind, String> {
    match (from, to) {
        ("png", "jpg") => Ok(ConversionKind::Image(ImageConversion::PngToJpg)),
        ("webp", "jpg") => Ok(ConversionKind::Image(ImageConversion::WebpToJpg)),
        ("jpg" | "jpeg", "png") => Ok(ConversionKind::Image(ImageConversion::JpgToPng)),
        ("webp", "png") => Ok(ConversionKind::Image(ImageConversion::WebpToPng)),
        ("png", "webp") => Ok(ConversionKind::Image(ImageConversion::PngToWebp)),
        ("jpg" | "jpeg", "webp") => Ok(ConversionKind::Image(ImageConversion::JpgToWebp)),
        ("bmp", "jpg") => Ok(ConversionKind::Image(ImageConversion::BmpToJpg)),
        ("bmp", "png") => Ok(ConversionKind::Image(ImageConversion::BmpToPng)),
        ("bmp", "webp") => Ok(ConversionKind::Image(ImageConversion::BmpToWebp)),
        ("tiff", "jpg") => Ok(ConversionKind::Image(ImageConversion::TiffToJpg)),
        ("tiff", "png") => Ok(ConversionKind::Image(ImageConversion::TiffToPng)),
        ("tiff", "webp") => Ok(ConversionKind::Image(ImageConversion::TiffToWebp)),
        ("gif", "jpg") => Ok(ConversionKind::Image(ImageConversion::GifToJpg)),
        ("gif", "png") => Ok(ConversionKind::Image(ImageConversion::GifToPng)),
        ("gif", "webp") => Ok(ConversionKind::Image(ImageConversion::GifToWebp)),
        ("svg", "png") => Ok(ConversionKind::Image(ImageConversion::SvgToPng)),
        ("svg", "jpg") => Ok(ConversionKind::Image(ImageConversion::SvgToJpg)),
        ("svg", "webp") => Ok(ConversionKind::Image(ImageConversion::SvgToWebp)),
        ("ico", "png") => Ok(ConversionKind::Image(ImageConversion::IcoToPng)),
        ("ico", "jpg") => Ok(ConversionKind::Image(ImageConversion::IcoToJpg)),
        ("heic", "jpg") => Ok(ConversionKind::Image(ImageConversion::HeicToJpg)),
        ("heic", "png") => Ok(ConversionKind::Image(ImageConversion::HeicToPng)),
        ("heic", "webp") => Ok(ConversionKind::Image(ImageConversion::HeicToWebp)),
        ("avif", "jpg") => Ok(ConversionKind::Image(ImageConversion::AvifToJpg)),
        ("avif", "png") => Ok(ConversionKind::Image(ImageConversion::AvifToPng)),
        ("avif", "webp") => Ok(ConversionKind::Image(ImageConversion::AvifToWebp)),
        // Additional raster/vector combinations supported by ImageMagick.
        ("bmp", "tiff" | "gif" | "ico" | "avif")
        | ("tiff", "bmp" | "gif" | "ico" | "avif")
        | ("gif", "bmp" | "tiff" | "ico" | "avif")
        | ("svg", "bmp" | "tiff" | "gif" | "ico" | "avif")
        | ("ico", "webp" | "bmp" | "tiff" | "gif" | "avif")
        | ("heic", "bmp" | "tiff" | "gif" | "ico" | "avif")
        | ("avif", "bmp" | "tiff" | "gif" | "ico")
        | ("png", "avif")
        | ("jpg" | "jpeg", "avif")
        | ("webp", "avif") => Ok(ConversionKind::Image(ImageConversion::ImageMagick)),
        (
            "bmp" | "tiff" | "gif" | "svg" | "ico" | "avif" | "png" | "jpg" | "jpeg" | "webp",
            "heic",
        ) => Ok(ConversionKind::Heic),
        ("wav", "mp3") => Ok(ConversionKind::Audio(AudioConversion::WavToMp3)),
        ("mp3", "wav") => Ok(ConversionKind::Audio(AudioConversion::Mp3ToWav)),
        ("ogg" | "flac" | "aac" | "m4a", "mp3") => {
            Ok(ConversionKind::Audio(AudioConversion::OtherToMp3))
        }
        ("ogg" | "flac" | "aac" | "m4a", "wav") => {
            Ok(ConversionKind::Audio(AudioConversion::OtherToWav))
        }
        ("mp4", "mp3") => Ok(ConversionKind::Video(VideoConversion::ToMp3)),
        ("mp4", "wav") => Ok(ConversionKind::Video(VideoConversion::ToWav)),
        ("mp4" | "mkv" | "mov" | "avi" | "webm", "mp4") => {
            Ok(ConversionKind::Video(VideoConversion::ToMp4))
        }
        ("mkv" | "mov" | "avi" | "webm", "mp3") => {
            Ok(ConversionKind::Video(VideoConversion::ToMp3))
        }
        ("mkv" | "mov" | "avi" | "webm", "wav") => {
            Ok(ConversionKind::Video(VideoConversion::ToWav))
        }
        ("pdf", "png") => Ok(ConversionKind::Pdf(PdfConversion::PdfToPng)),
        ("pdf", "jpg") => Ok(ConversionKind::Pdf(PdfConversion::PdfToJpg)),
        ("png", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::PngToPdf)),
        ("jpg" | "jpeg", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::JpgToPdf)),
        ("webp", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::WebpToPdf)),
        ("bmp", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::BmpToPdf)),
        ("tiff", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::TiffToPdf)),
        ("gif", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::GifToPdf)),
        ("svg", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::SvgToPdf)),
        ("heic", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::HeicToPdf)),
        ("avif", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::AvifToPdf)),
        ("ico", "pdf") => Ok(ConversionKind::Pdf(PdfConversion::IcoToPdf)),
        ("pdf", "docx" | "txt" | "html" | "odt" | "rtf")
        | ("docx" | "txt" | "html" | "odt" | "rtf", "pdf") => {
            Ok(ConversionKind::Document(DocumentConversion))
        }
        _ => Err("unsupported conversion".to_string()),
    }
}

pub fn is_supported_conversion(from: &str, to: &str) -> bool {
    conversion_for_formats(from, to).is_ok()
}

/// Returns the complete matrix accepted by the same resolver used at runtime.
/// Adding a supported pair to `conversion_for_formats` therefore makes it
/// discoverable by the matrix suite automatically.
pub fn supported_conversions() -> Vec<SupportedConversion> {
    let mut conversions = CONVERSION_FORMATS
        .iter()
        .flat_map(|from| {
            CONVERSION_FORMATS.iter().filter_map(move |to| {
                is_supported_conversion(from, to).then_some(SupportedConversion { from, to })
            })
        })
        .collect::<Vec<_>>();
    conversions.sort_unstable();
    conversions
}

pub fn plan_conversion(job: &Job) -> Result<JobPlan, String> {
    let conversion = conversion_for_formats(&job.payload.from, &job.payload.to)?;

    Ok(JobPlan {
        job: job.clone(),
        conversion,
    })
}

pub fn execute_image_conversion(
    input_path: &Path,
    output_path: &Path,
    conversion: &ImageConversion,
) -> Result<(), String> {
    if !matches!(
        conversion,
        ImageConversion::PngToJpg
            | ImageConversion::WebpToJpg
            | ImageConversion::JpgToPng
            | ImageConversion::WebpToPng
            | ImageConversion::PngToWebp
            | ImageConversion::JpgToWebp
    ) {
        return crate::pdf::ImageToPdfRunner::system().run_image(input_path, output_path);
    }

    let img =
        image::open(input_path).map_err(|err| format!("failed to open input image: {err}"))?;

    match conversion {
        ImageConversion::PngToJpg
        | ImageConversion::WebpToJpg
        | ImageConversion::JpgToPng
        | ImageConversion::WebpToPng
        | ImageConversion::PngToWebp
        | ImageConversion::JpgToWebp => {
            let rgb = img.to_rgb8();
            match conversion {
                ImageConversion::PngToJpg
                | ImageConversion::WebpToJpg
                | ImageConversion::JpgToWebp => {
                    rgb.save(output_path)
                        .map_err(|err| format!("failed to write output image: {err}"))?;
                }
                ImageConversion::JpgToPng
                | ImageConversion::WebpToPng
                | ImageConversion::PngToWebp => {
                    img.to_rgba8()
                        .save(output_path)
                        .map_err(|err| format!("failed to write output image: {err}"))?;
                }
                _ => unreachable!("ImageMagick conversions return before decoding"),
            }
        }
        _ => unreachable!("ImageMagick conversions return before decoding"),
    }

    Ok(())
}

pub fn execute_audio_conversion(
    input_path: &Path,
    output_path: &Path,
    conversion: &AudioConversion,
) -> Result<(), String> {
    let runner = crate::ffmpeg::FfmpegRunner::default();
    let args = match conversion {
        AudioConversion::WavToMp3 => {
            crate::ffmpeg::FfmpegRunner::build_audio_to_mp3_args(input_path, output_path)
        }
        AudioConversion::Mp3ToWav => {
            crate::ffmpeg::FfmpegRunner::build_mp3_to_wav_args(input_path, output_path)
        }
        AudioConversion::OtherToMp3 => {
            crate::ffmpeg::FfmpegRunner::build_audio_to_mp3_args(input_path, output_path)
        }
        AudioConversion::OtherToWav => {
            crate::ffmpeg::FfmpegRunner::build_audio_to_wav_args(input_path, output_path)
        }
        AudioConversion::Mp4ToMp3 => {
            crate::ffmpeg::FfmpegRunner::build_mp4_to_mp3_args(input_path, output_path)
        }
        AudioConversion::Mp4ToWav => {
            crate::ffmpeg::FfmpegRunner::build_mp4_to_wav_args(input_path, output_path)
        }
    };

    runner.run(&args)
}

pub fn execute_video_conversion(
    input_path: &Path,
    output_path: &Path,
    conversion: &VideoConversion,
) -> Result<(), String> {
    let runner = crate::ffmpeg::FfmpegRunner::default();
    let args = match conversion {
        VideoConversion::ToMp4 | VideoConversion::MkvToMp4 => {
            crate::ffmpeg::FfmpegRunner::build_video_to_mp4_args(input_path, output_path)
        }
        VideoConversion::MovToMp4 => {
            crate::ffmpeg::FfmpegRunner::build_video_to_mp4_args(input_path, output_path)
        }
        VideoConversion::AviToMp4 => {
            crate::ffmpeg::FfmpegRunner::build_video_to_mp4_args(input_path, output_path)
        }
        VideoConversion::WebmToMp4 => {
            crate::ffmpeg::FfmpegRunner::build_video_to_mp4_args(input_path, output_path)
        }
        VideoConversion::ToMp3 => {
            crate::ffmpeg::FfmpegRunner::build_video_to_mp3_args(input_path, output_path)
        }
        VideoConversion::ToWav => {
            crate::ffmpeg::FfmpegRunner::build_video_to_wav_args(input_path, output_path)
        }
    };

    runner.run(&args)
}

pub fn execute_pdf_conversion(
    input_path: &Path,
    output_path: &Path,
    conversion: &PdfConversion,
) -> Result<(), String> {
    match conversion {
        PdfConversion::PdfToPng | PdfConversion::PdfToJpg => {
            crate::pdf::PdfRunner::system().run(input_path, output_path)
        }
        PdfConversion::PngToPdf
        | PdfConversion::JpgToPdf
        | PdfConversion::WebpToPdf
        | PdfConversion::BmpToPdf
        | PdfConversion::TiffToPdf
        | PdfConversion::GifToPdf
        | PdfConversion::SvgToPdf
        | PdfConversion::HeicToPdf
        | PdfConversion::AvifToPdf
        | PdfConversion::IcoToPdf => {
            crate::pdf::ImageToPdfRunner::system().run(input_path, output_path)
        }
    }
}

pub fn execute_job(job: &Job, input_path: &Path, output_path: &Path) -> Result<(), String> {
    let plan = plan_conversion(job)?;

    match plan.conversion {
        ConversionKind::Image(ref conversion) => {
            execute_image_conversion(input_path, output_path, conversion)
        }
        ConversionKind::Heic => crate::heif::HeifRunner::system().run(input_path, output_path),
        ConversionKind::Audio(ref conversion) => {
            execute_audio_conversion(input_path, output_path, conversion)
        }
        ConversionKind::Video(ref conversion) => {
            execute_video_conversion(input_path, output_path, conversion)
        }
        ConversionKind::Pdf(ref conversion) => {
            execute_pdf_conversion(input_path, output_path, conversion)
        }
        ConversionKind::Document(_) => {
            crate::documents::DocumentRunner::system().run(input_path, output_path)
        }
    }
}

#[derive(Clone, Debug)]
pub struct JobQueue {
    tx: mpsc::Sender<String>,
}

impl JobQueue {
    pub fn new(tx: mpsc::Sender<String>) -> Self {
        Self { tx }
    }

    pub async fn enqueue(&self, job_id: String) -> Result<(), String> {
        self.tx
            .send(job_id)
            .await
            .map_err(|_| "queue send failed".to_string())
    }
}

pub async fn process_queued_job(state: &AppState, job_id: &str) -> Result<(), String> {
    let Some(job) = state.jobs.get(job_id) else {
        return Err("job not found".to_string());
    };

    state.jobs.update_status(job_id, JobStatus::Processing);
    state.jobs.set_progress(job_id, 10);

    let input_path = Path::new(&state.settings.data_dir)
        .join("jobs")
        .join(job_id)
        .join(&job.input_name);
    let output_name = format!("converted.{}", job.payload.to);
    let output_path = Path::new(&state.settings.data_dir)
        .join("jobs")
        .join(job_id)
        .join(&output_name);

    match execute_job(&job, &input_path, &output_path) {
        Ok(_) => {
            state.jobs.set_output(job_id, &output_name);
            state.jobs.set_progress(job_id, 100);
            state.jobs.update_status(job_id, JobStatus::Completed);
            Ok(())
        }
        Err(err) => {
            state.jobs.set_error(job_id, &err);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioConversion, ConversionKind, ImageConversion, conversion_for_formats,
        execute_audio_conversion, execute_image_conversion, execute_job, is_supported_conversion,
        plan_conversion,
    };
    use crate::jobs::{Job, JobPayload, JobStatus};
    use std::fs;
    use std::process::Command;

    #[test]
    fn plans_supported_png_to_jpg_conversion() {
        let job = Job {
            id: "job-1".to_string(),
            created_at: 0,
            status: JobStatus::Queued,
            input_name: "image.png".to_string(),
            output_name: None,
            payload: JobPayload {
                from: "png".to_string(),
                to: "jpg".to_string(),
            },
            progress: 0,
            error: None,
        };

        let plan = plan_conversion(&job).expect("conversion should be supported");
        assert_eq!(
            plan.conversion,
            ConversionKind::Image(ImageConversion::PngToJpg)
        );
    }

    #[test]
    fn rejects_unsupported_conversion() {
        let job = Job {
            id: "job-2".to_string(),
            created_at: 0,
            status: JobStatus::Queued,
            input_name: "image.mp4".to_string(),
            output_name: None,
            payload: JobPayload {
                from: "gif".to_string(),
                to: "wav".to_string(),
            },
            progress: 0,
            error: None,
        };

        assert!(plan_conversion(&job).is_err());
    }

    #[test]
    fn supports_the_declared_conversion_matrix() {
        let image_conversions = [
            ("png", "jpg"),
            ("png", "webp"),
            ("png", "pdf"),
            ("png", "avif"),
            ("png", "heic"),
            ("jpg", "png"),
            ("jpg", "webp"),
            ("jpg", "pdf"),
            ("jpg", "avif"),
            ("jpg", "heic"),
            ("jpeg", "png"),
            ("jpeg", "webp"),
            ("jpeg", "pdf"),
            ("jpeg", "avif"),
            ("jpeg", "heic"),
            ("webp", "jpg"),
            ("webp", "png"),
            ("webp", "pdf"),
            ("webp", "avif"),
            ("webp", "heic"),
            ("bmp", "jpg"),
            ("bmp", "png"),
            ("bmp", "webp"),
            ("bmp", "pdf"),
            ("bmp", "tiff"),
            ("bmp", "gif"),
            ("bmp", "ico"),
            ("bmp", "heic"),
            ("bmp", "avif"),
            ("tiff", "jpg"),
            ("tiff", "png"),
            ("tiff", "webp"),
            ("tiff", "pdf"),
            ("tiff", "bmp"),
            ("tiff", "gif"),
            ("tiff", "ico"),
            ("tiff", "heic"),
            ("tiff", "avif"),
            ("gif", "jpg"),
            ("gif", "png"),
            ("gif", "webp"),
            ("gif", "pdf"),
            ("gif", "bmp"),
            ("gif", "tiff"),
            ("gif", "ico"),
            ("gif", "heic"),
            ("gif", "avif"),
            ("svg", "png"),
            ("svg", "jpg"),
            ("svg", "webp"),
            ("svg", "pdf"),
            ("svg", "bmp"),
            ("svg", "tiff"),
            ("svg", "gif"),
            ("svg", "ico"),
            ("svg", "heic"),
            ("svg", "avif"),
            ("ico", "png"),
            ("ico", "jpg"),
            ("ico", "webp"),
            ("ico", "bmp"),
            ("ico", "tiff"),
            ("ico", "gif"),
            ("ico", "pdf"),
            ("ico", "heic"),
            ("ico", "avif"),
            ("heic", "jpg"),
            ("heic", "png"),
            ("heic", "webp"),
            ("heic", "pdf"),
            ("heic", "bmp"),
            ("heic", "tiff"),
            ("heic", "gif"),
            ("heic", "ico"),
            ("heic", "avif"),
            ("avif", "jpg"),
            ("avif", "png"),
            ("avif", "webp"),
            ("avif", "pdf"),
            ("avif", "bmp"),
            ("avif", "tiff"),
            ("avif", "gif"),
            ("avif", "ico"),
            ("avif", "heic"),
            ("pdf", "png"),
            ("pdf", "jpg"),
        ];
        let audio_conversions = [
            ("wav", "mp3"),
            ("mp3", "wav"),
            ("ogg", "mp3"),
            ("ogg", "wav"),
            ("flac", "mp3"),
            ("flac", "wav"),
            ("aac", "mp3"),
            ("aac", "wav"),
            ("m4a", "mp3"),
            ("m4a", "wav"),
        ];
        let video_conversions = [
            ("mp4", "mp4"),
            ("mp4", "mp3"),
            ("mp4", "wav"),
            ("mkv", "mp4"),
            ("mkv", "mp3"),
            ("mkv", "wav"),
            ("mov", "mp4"),
            ("mov", "mp3"),
            ("mov", "wav"),
            ("avi", "mp4"),
            ("avi", "mp3"),
            ("avi", "wav"),
            ("webm", "mp4"),
            ("webm", "mp3"),
            ("webm", "wav"),
        ];
        let document_conversions = [
            ("pdf", "docx"),
            ("pdf", "txt"),
            ("pdf", "html"),
            ("pdf", "odt"),
            ("pdf", "rtf"),
            ("docx", "pdf"),
            ("txt", "pdf"),
            ("html", "pdf"),
            ("odt", "pdf"),
            ("rtf", "pdf"),
        ];

        for (source, target) in image_conversions
            .into_iter()
            .chain(audio_conversions)
            .chain(video_conversions)
            .chain(document_conversions)
        {
            assert!(
                is_supported_conversion(source, target),
                "{source} -> {target}"
            );
        }

        for (source, target) in [("txt", "jpg"), ("pdf", "mp4"), ("mp3", "pdf")] {
            assert!(
                !is_supported_conversion(source, target),
                "{source} -> {target}"
            );
        }
    }

    #[test]
    fn dispatches_modern_image_outputs_to_the_right_runner() {
        assert_eq!(
            conversion_for_formats("png", "avif"),
            Ok(ConversionKind::Image(ImageConversion::ImageMagick))
        );
        assert_eq!(
            conversion_for_formats("png", "heic"),
            Ok(ConversionKind::Heic)
        );
    }

    #[test]
    fn executes_png_to_jpg_conversion() {
        let tmpdir = std::env::temp_dir().join(format!("euoni-worker-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmpdir);
        fs::create_dir_all(&tmpdir).unwrap();

        let source = tmpdir.join("source.png");
        let output = tmpdir.join("result.jpg");

        let image = image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 255]));
        image.save(&source).unwrap();

        let conversion = ImageConversion::PngToJpg;
        execute_image_conversion(&source, &output, &conversion).unwrap();

        assert!(output.exists());
        let decoded = image::open(&output).unwrap();
        assert_eq!(decoded.color().channel_count(), 3);

        let _ = fs::remove_dir_all(&tmpdir);
    }

    #[test]
    fn executes_wav_to_mp3_conversion() {
        let tmpdir =
            std::env::temp_dir().join(format!("euoni-worker-audio-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmpdir);
        fs::create_dir_all(&tmpdir).unwrap();

        let source = tmpdir.join("source.wav");
        let output = tmpdir.join("result.mp3");

        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=1000:duration=1",
                "-c:a",
                "pcm_s16le",
                &source.to_string_lossy(),
            ])
            .status()
            .unwrap();

        assert!(status.success());

        let conversion = AudioConversion::WavToMp3;
        execute_audio_conversion(&source, &output, &conversion).unwrap();

        assert!(output.exists());
        assert!(output.metadata().unwrap().len() > 0);

        let _ = fs::remove_dir_all(&tmpdir);
    }

    #[test]
    fn executes_representative_pdf_audio_and_video_jobs() {
        let tmpdir =
            std::env::temp_dir().join(format!("euoni-worker-matrix-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmpdir);
        fs::create_dir_all(&tmpdir).unwrap();

        let image = tmpdir.join("source.png");
        image::RgbaImage::from_pixel(32, 32, image::Rgba([20, 120, 220, 255]))
            .save(&image)
            .unwrap();
        let source_pdf = tmpdir.join("source.pdf");
        Command::new("convert")
            .args([image.to_str().unwrap(), source_pdf.to_str().unwrap()])
            .status()
            .unwrap();

        let pdf_job = Job {
            id: "pdf-job".to_string(),
            created_at: 0,
            status: JobStatus::Queued,
            input_name: "source.pdf".to_string(),
            output_name: None,
            payload: JobPayload {
                from: "pdf".to_string(),
                to: "jpg".to_string(),
            },
            progress: 0,
            error: None,
        };
        let pdf_output = tmpdir.join("pdf-result.jpg");
        execute_job(&pdf_job, &source_pdf, &pdf_output).unwrap();

        let source_wav = tmpdir.join("source.wav");
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                source_wav.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        let audio_job = Job {
            id: "audio-job".to_string(),
            created_at: 0,
            status: JobStatus::Queued,
            input_name: "source.wav".to_string(),
            output_name: None,
            payload: JobPayload {
                from: "wav".to_string(),
                to: "mp3".to_string(),
            },
            progress: 0,
            error: None,
        };
        let audio_output = tmpdir.join("audio-result.mp3");
        execute_job(&audio_job, &source_wav, &audio_output).unwrap();

        let source_mp4 = tmpdir.join("source.mp4");
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=32x32:rate=5:duration=1",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-shortest",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                source_mp4.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        let video_job = Job {
            id: "video-job".to_string(),
            created_at: 0,
            status: JobStatus::Queued,
            input_name: "source.mp4".to_string(),
            output_name: None,
            payload: JobPayload {
                from: "mp4".to_string(),
                to: "wav".to_string(),
            },
            progress: 0,
            error: None,
        };
        let video_output = tmpdir.join("video-result.wav");
        execute_job(&video_job, &source_mp4, &video_output).unwrap();

        assert!(pdf_output.metadata().unwrap().len() > 0);
        assert!(audio_output.metadata().unwrap().len() > 0);
        assert!(video_output.metadata().unwrap().len() > 0);
        let _ = fs::remove_dir_all(&tmpdir);
    }

    #[tokio::test]
    async fn queue_accepts_job_ids() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(4);
        let queue = super::JobQueue::new(tx);

        queue.enqueue("job-abc".to_string()).await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received, "job-abc");
    }
}
