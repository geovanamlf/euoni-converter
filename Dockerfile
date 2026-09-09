FROM rust:1.95-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && printf 'fn main() {}\n' > src/main.rs
RUN cargo build --release

COPY src ./src
COPY web ./web
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg poppler-utils imagemagick ca-certificates \
    && sed -i 's/rights="none" pattern="PDF"/rights="read|write" pattern="PDF"/' /etc/ImageMagick-6/policy.xml \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/euoni-converter /usr/local/bin/euoni-converter

ENV HOST=0.0.0.0 \
    PORT=8080 \
    DATA_DIR=/data \
    MAX_WORKERS=2 \
    JOB_RETENTION_HOURS=24 \
    MAX_UPLOAD_SIZE=2147483648

RUN mkdir -p /data
VOLUME ["/data"]
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/euoni-converter"]
