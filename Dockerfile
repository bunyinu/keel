# Build stage
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY web ./web
RUN cargo build --release --bin keel-server

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
RUN useradd -m -u 1000 keel
WORKDIR /app
COPY --from=builder /app/target/release/keel-server /usr/local/bin/keel-server
RUN mkdir -p /data && chown keel:keel /data
USER keel
ENV KEEL_DB_PATH=/data/keel.db
ENV PORT=8080
EXPOSE 8080
CMD ["keel-server"]
