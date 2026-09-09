# Euoni Converter

Euoni Converter is a self-hosted local media conversion service built in Rust. It currently supports queued image conversions, FFmpeg-backed audio/video conversions, and the first PDF operation: rendering a PDF page to PNG.

## Project goals

- Keep the server simple and easy to run on a home network.
- Support local browser access through the LAN.
- Favor modularity before adding complex conversion features.
- Use mature tooling such as Axum, Tokio, FFmpeg, and Poppler instead of custom codec implementations.

## Current scope

- Rust binary project with a modular layout.
- Configuration via environment variables and `.env` in development.
- Axum HTTP server with health and system endpoints.
- Basic startup validation.
- SQLite-persisted job store and worker queue.
- Multipart uploads with safe filenames and job status tracking.
- Image conversions: PNG/JPG/JPEG/WEBP between the supported image targets, including PDF output.
- Audio conversions: WAV/MP3/OGG/FLAC/AAC/M4A to MP3 or WAV.
- Video conversions: MP4/MKV/MOV/AVI/WEBM to MP4, MP3, or WAV.
- PDF conversions: PDF to PNG/JPG and PNG/JPG to PDF.
- The browser only shows valid targets for the selected source; unsupported pairs are rejected before a job is created.
- Browser interface served at `/` for upload, progress, and download.

## Directory structure

```text
src/
├── app.rs
├── config.rs
├── error.rs
├── lib.rs
├── main.rs
├── pdf.rs
├── ffmpeg.rs
├── jobs.rs
├── storage.rs
├── upload.rs
├── worker.rs
├── routes/
│   ├── health.rs
│   ├── mod.rs
│   ├── system.rs
│   ├── jobs.rs
│   ├── upload.rs
│   └── download.rs
```

## Configuration

Use a local `.env` file during development. Example:

```env
HOST=0.0.0.0
PORT=8080
EUONI_BIND_ADDRESS=127.0.0.1
MAX_UPLOAD_SIZE=2147483648
MAX_WORKERS=2
JOB_RETENTION_HOURS=24
DATA_DIR=./data
```

The repository includes `.env.example` for reference. Do not commit the real `.env` file.

For a local clone, copy `.env.example` to `.env`; the interface will be available at `http://localhost:8080`. To expose the service only on a home server LAN address, set `EUONI_BIND_ADDRESS` in `.env` to that address, for example `192.168.3.38`.

## Run locally

```bash
cargo run
```

The application listens by default on `0.0.0.0:8080`.

PDF conversion requires `pdftoppm` and ImageMagick's `convert` to be installed and available in `PATH`.

## Run with Docker

```bash
cp .env.example .env
docker compose up --build
```

The service is available at `http://localhost:8080`. The Compose volume `euoni-data` persists the SQLite database and converted files across container restarts.

To run the same clone on a home server for other devices on the LAN, set `EUONI_BIND_ADDRESS` in `.env` to the server's LAN IP before starting Compose.

On the home server, bind to its IPv4 LAN address and allow TCP port 8080 only from the local subnet in UFW; do not forward this port on the router.

## Endpoints

- `GET /api/health`
- `GET /api/system`
- `POST /api/upload`
- `POST /api/jobs`
- `GET /api/jobs/{id}`
- `GET /api/jobs/{id}/status`
- `GET /api/jobs/{id}/download`

## Test

```bash
cargo test
```

## Notes

- Jobs are persisted in SQLite at `DATA_DIR/jobs.sqlite` and survive server restarts.
