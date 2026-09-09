use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegRunner {
    pub binary: String,
}

impl Default for FfmpegRunner {
    fn default() -> Self {
        Self {
            binary: "ffmpeg".to_string(),
        }
    }
}

impl FfmpegRunner {
    pub fn build_audio_to_mp3_args(input: &Path, output: &Path) -> Vec<String> {
        vec![
            "-y".to_string(),
            "-i".to_string(),
            input.to_string_lossy().to_string(),
            "-vn".to_string(),
            "-ar".to_string(),
            "44100".to_string(),
            "-acodec".to_string(),
            "libmp3lame".to_string(),
            output.to_string_lossy().to_string(),
        ]
    }

    pub fn build_mp3_to_wav_args(input: &Path, output: &Path) -> Vec<String> {
        vec![
            "-y".to_string(),
            "-i".to_string(),
            input.to_string_lossy().to_string(),
            "-vn".to_string(),
            "-acodec".to_string(),
            "pcm_s16le".to_string(),
            output.to_string_lossy().to_string(),
        ]
    }

    pub fn build_mp4_to_mp3_args(input: &Path, output: &Path) -> Vec<String> {
        vec![
            "-y".to_string(),
            "-i".to_string(),
            input.to_string_lossy().to_string(),
            "-vn".to_string(),
            "-acodec".to_string(),
            "libmp3lame".to_string(),
            output.to_string_lossy().to_string(),
        ]
    }

    pub fn build_mp4_to_wav_args(input: &Path, output: &Path) -> Vec<String> {
        vec![
            "-y".to_string(),
            "-i".to_string(),
            input.to_string_lossy().to_string(),
            "-vn".to_string(),
            "-acodec".to_string(),
            "pcm_s16le".to_string(),
            output.to_string_lossy().to_string(),
        ]
    }

    pub fn build_audio_to_wav_args(input: &Path, output: &Path) -> Vec<String> {
        Self::build_mp3_to_wav_args(input, output)
    }

    pub fn build_video_to_mp3_args(input: &Path, output: &Path) -> Vec<String> {
        Self::build_mp4_to_mp3_args(input, output)
    }

    pub fn build_video_to_wav_args(input: &Path, output: &Path) -> Vec<String> {
        Self::build_mp4_to_wav_args(input, output)
    }

    pub fn build_video_to_mp4_args(input: &Path, output: &Path) -> Vec<String> {
        vec![
            "-y".to_string(),
            "-i".to_string(),
            input.to_string_lossy().to_string(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "fast".to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
            output.to_string_lossy().to_string(),
        ]
    }

    pub fn run(&self, args: &[String]) -> Result<(), String> {
        let output = Command::new(&self.binary)
            .args(args)
            .output()
            .map_err(|err| format!("failed to execute ffmpeg: {err}"))?;

        if !output.status.success() {
            let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if details.is_empty() {
                "ffmpeg exited with non-zero status".to_string()
            } else {
                format!("ffmpeg failed: {details}")
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FfmpegRunner;
    use std::path::Path;

    #[test]
    fn builds_mp3_conversion_arguments() {
        let _runner = FfmpegRunner::default();
        let args =
            FfmpegRunner::build_audio_to_mp3_args(Path::new("input.wav"), Path::new("output.mp3"));

        assert!(args.iter().any(|arg| arg == "-i"));
        assert!(args.iter().any(|arg| arg == "libmp3lame"));
        assert!(args.iter().any(|arg| arg == "output.mp3"));
    }

    #[test]
    fn builds_mp4_conversion_arguments() {
        let _runner = FfmpegRunner::default();
        let args =
            FfmpegRunner::build_video_to_mp4_args(Path::new("input.mkv"), Path::new("output.mp4"));

        assert!(args.iter().any(|arg| arg == "-c:v"));
        assert!(args.iter().any(|arg| arg == "libx264"));
        assert!(args.iter().any(|arg| arg == "output.mp4"));
    }

    #[test]
    fn builds_mp4_to_mp3_arguments() {
        let args =
            FfmpegRunner::build_mp4_to_mp3_args(Path::new("input.mp4"), Path::new("output.mp3"));

        assert!(args.iter().any(|arg| arg == "-vn"));
        assert!(args.iter().any(|arg| arg == "libmp3lame"));
        assert!(args.iter().any(|arg| arg == "output.mp3"));
    }
}
