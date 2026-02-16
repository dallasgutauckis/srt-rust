# Multi-stage build for SRT Rust
FROM rust:1.75-slim as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY srt/ srt/
COPY srt-protocol/ srt-protocol/
COPY srt-bonding/ srt-bonding/
COPY srt-cli/ srt-cli/
COPY srt-crypto/ srt-crypto/
COPY srt-io/ srt-io/
COPY srt-tests/ srt-tests/

# Build release binaries
RUN cargo build --release --bin srt-sender --bin srt-receiver

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binaries from builder
COPY --from=builder /build/target/release/srt-sender /usr/local/bin/
COPY --from=builder /build/target/release/srt-receiver /usr/local/bin/

# Create non-root user
RUN useradd -m -u 1000 srt && \
    chown -R srt:srt /usr/local/bin/srt-*

USER srt

# Expose default ports
EXPOSE 9000/udp
EXPOSE 5000/udp

# Default command shows help
CMD ["srt-sender", "--help"]
