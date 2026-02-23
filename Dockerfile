# ─── Stage 1: Build ─────────────────────────────────────────
FROM rust:1.83-bookworm AS builder

WORKDIR /app
COPY . .

RUN cargo build --release

# ─── Stage 2: Runtime ────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -s /bin/sh zenclaw

COPY --from=builder /app/target/release/zenclaw /usr/local/bin/zenclaw

USER zenclaw
WORKDIR /home/zenclaw

# Default data directories
RUN mkdir -p /home/zenclaw/.config/zenclaw /home/zenclaw/.local/share/zenclaw

EXPOSE 3000

# Default: start API server
ENTRYPOINT ["zenclaw"]
CMD ["serve", "--host", "0.0.0.0", "--port", "3000"]
