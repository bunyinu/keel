# Build stage
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
RUN cargo build --release --bin keel-server

# Runtime — run as root so Render persistent disk at /data is always writable.
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/keel-server /usr/local/bin/keel-server
COPY --from=builder /app/web /app/web
RUN mkdir -p /data
ENV KEEL_DB_PATH=/data/keel.db
ENV PORT=8080
EXPOSE 8080
CMD ["/usr/local/bin/keel-server"]
