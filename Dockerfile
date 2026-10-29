
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
    && apt-get install -y --no-install-recommends ffmpeg poppler-utils ghostscript qpdf imagemagick img2pdf libheif-examples libx265-199 zip unzip librsvg2-bin libreoffice ca-certificates gosu \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/euoni-converter /usr/local/bin/euoni-converter

ENV HOST=0.0.0.0 \
    PORT=8080 \
    DATA_DIR=/data \
    MAX_WORKERS=2 \
    JOB_RETENTION_HOURS=24 \
    MAX_UPLOAD_SIZE=536870912 \
    MAX_OUTPUT_SIZE=1073741824 \
    CONVERSION_TIMEOUT_SECONDS=300

RUN groupadd --system euoni \
    && useradd --system --gid euoni --home-dir /tmp/euoni-home --no-create-home euoni \
    && mkdir -p /data /tmp/euoni-home \
    && chown -R euoni:euoni /data /tmp/euoni-home \
    && chmod 0750 /data /tmp/euoni-home

WORKDIR /app

COPY --from=builder /app/web ./web
COPY imagemagick-policy.xml /tmp/euoni-security-policy.xml
RUN sed '/<\/policymap>/e sed "1d;\$d" /tmp/euoni-security-policy.xml' \
        /etc/ImageMagick-6/policy.xml > /tmp/policy.xml \
    && cat /tmp/policy.xml > /etc/ImageMagick-6/policy.xml \
    && rm -f /tmp/policy.xml /tmp/euoni-security-policy.xml

COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod 0755 /usr/local/bin/docker-entrypoint.sh

VOLUME ["/data"]
EXPOSE 8080

ENV HOME=/tmp
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
