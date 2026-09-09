# Euoni Converter

Euoni Converter is a self-hosted local media conversion service built in Rust. It currently supports queued image conversions, FFmpeg-backed audio/video conversions, and the first PDF operation: rendering a PDF page to PNG.

## Requirements

The easiest way to run the project is with Docker and Docker Compose.

For running without Docker, install Rust, FFmpeg, Poppler (`pdftoppm`) and ImageMagick.

## Run with Docker

Clone the repository:

```bash
git clone git@github.com:geovanamlf/euoni-converter.git
cd euoni-converter
```

Create the local configuration file and start the application:

```bash
cp .env.example .env
docker compose up --build
```

Open `http://localhost:8080` in the browser.

```bash
docker compose up -d --build
```

See the logs with:

```bash
docker compose logs -f
```

Stop the application with:

```bash
docker compose down
```

The Docker volume `euoni-data` stores the jobs database and converted files.

## Run on a home server

The default configuration binds only to `127.0.0.1`, which is appropriate for a local installation. To access the application from other devices on a home network, edit `.env` on the server:

```env
EUONI_BIND_ADDRESS=192.168.3.38
```

Replace the address with the server's own LAN IP. Then start or recreate the container:

```bash
docker compose up -d --build
```

Other devices on the same network can use:

```text
http://192.168.3.38:8080
```

## Run without Docker

```bash
cp .env.example .env
cargo run
```

This mode requires Rust and the external conversion tools installed on the host.

Run the tests with:

```bash
cargo test
```

## Supported conversions

- Images: PNG, JPG, JPEG and WEBP between the supported image formats and PDF.
- Audio: WAV, MP3, OGG, FLAC, AAC and M4A to MP3 or WAV.
- Video: MP4, MKV, MOV, AVI and WEBM to MP4, MP3 or WAV.
- PDF: PDF to PNG/JPG and images to PDF.

The web interface only displays valid output formats for the selected input.
