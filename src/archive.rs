use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use zip::{ZipWriter, write::SimpleFileOptions};

pub fn create_zip_from_directory(
    source_dir: &Path,
    output: &Path,
    max_size: u64,
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files(source_dir, source_dir, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err("conversion produced no files".to_string());
    }

    let file = File::create(output).map_err(|error| error.to_string())?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = [0_u8; 64 * 1024];

    for path in &files {
        let relative = path
            .strip_prefix(source_dir)
            .map_err(|error| format!("invalid archive path: {error}"))?;
        let name = relative
            .to_str()
            .ok_or_else(|| "archive path is not valid UTF-8".to_string())?
            .replace('\\', "/");
        archive
            .start_file(name, options)
            .map_err(|error| error.to_string())?;
        let mut input = File::open(path).map_err(|error| error.to_string())?;
        loop {
            let read = input.read(&mut buffer).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            archive
                .write_all(&buffer[..read])
                .map_err(|error| error.to_string())?;
            if output
                .metadata()
                .map(|metadata| metadata.len() > max_size)
                .unwrap_or(false)
            {
                return Err("conversion output exceeded the configured limit".to_string());
            }
        }
    }

    archive.finish().map_err(|error| error.to_string())?;
    if output.metadata().map_err(|error| error.to_string())?.len() > max_size {
        return Err("conversion output exceeded the configured limit".to_string());
    }
    Ok(files)
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            if path.strip_prefix(root).is_err() {
                return Err("archive file escaped working directory".to_string());
            }
            files.push(path);
        } else {
            return Err("archive contains a non-regular file".to_string());
        }
    }
    Ok(())
}
