# Euoni Converter

Euoni Converter is a self-hosted local media conversion service built in Rust. It processes queued media conversions locally and protects each job with a one-time capability token.

## Requirements

The easiest way to run the project is with Docker and Docker Compose.

For running without Docker, install Rust, FFmpeg, Poppler (`pdftoppm`), ImageMagick and libheif's `heif-enc` utility (for HEIC output).

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

The Docker volume `euoni-data` stores the jobs database and converted files. The container runs as the unprivileged `euoni` user; the mounted `/data` volume must be writable by that user. The container filesystem is read-only and `/tmp` is a temporary filesystem used by conversion tools.

The upload limit defaults to 512 MiB, the output limit to 1 GiB, and each external conversion has a 300-second timeout. External runners are interrupted when their observable output exceeds the configured output limit, and the final artifact is validated again. These values can be changed through `.env`, but increasing them increases resource and denial-of-service risk.

## Run on a home server

The default configuration binds only to `127.0.0.1`, which is appropriate for a local installation. To access the application from other devices on a home network, edit `.env` on the server and consciously change both the Docker publish address and the application bind address:

```env
EUONI_BIND_ADDRESS=192.168.3.38
HOST=0.0.0.0
```

Replace the address with the server's own LAN IP. Then start or recreate the container:

```bash
docker compose up -d --build
```

Other devices on the same network can use:

```text
http://192.168.3.38:8080
```

LAN exposure is not authentication by itself. Every upload response contains a secret job token. The frontend keeps that token only in memory and sends it in the `X-Euoni-Job-Token` header for status and download requests. Do not put job tokens in URLs, logs, screenshots, or persistent browser storage. A page reload loses access to jobs created before the reload; this is intentional.

`/api/health` and `/api/system` expose only a minimal service status. Job details and downloads require the token returned when the job is created. Legacy jobs created before token support have no credential and are deliberately inaccessible; they expire through the normal retention cleanup.

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

- Images: PNG, JPG, JPEG, WEBP, BMP, TIFF, GIF, SVG, ICO, HEIC and AVIF between the supported image formats and PDF.
- Audio: WAV, MP3, OGG, FLAC, AAC and M4A to MP3 or WAV.
- Video: MP4, MKV, MOV, AVI and WEBM to MP4, MP3 or WAV.
- PDF: PDF to PNG/JPG and images to PDF.
- Documents: PDF to DOCX, TXT, HTML, ODT and RTF; DOCX, TXT, HTML, ODT and RTF to PDF. Office conversions require LibreOffice and PDF text/HTML extraction uses Poppler. Complex PDF layouts may not be preserved.

The web interface only displays valid output formats for the selected input.

When exposed beyond loopback, place the service behind a trusted network boundary or reverse proxy with TLS and additional access controls. The per-job token protects individual results; it is not a user account system.
