# SRT-Rust: Secure Reliable Transport Protocol in Rust

A complete rewrite of the SRT (Secure Reliable Transport) protocol library from C/C++ to Rust, with emphasis on **bonded multi-path streaming** for heterogeneous networks.

## Architecture

This project is organized as a Cargo workspace with the following crates:

- **srt-protocol**: Core protocol implementation (packets, handshake, connection state machine)
- **srt-bonding**: Multi-path connection bonding (broadcast, backup, load balancing)
- **srt-crypto**: Encryption with pluggable backends
- **srt-io**: Network I/O and platform abstractions
- **srt**: High-level public API
- **srt-cli**: Command-line tools (sender, receiver, relay)
- **srt-tests**: Integration tests

## Why Rust?

- **Memory safety**: Prevents buffer overflow vulnerabilities in network code
- **Fearless concurrency**: Safe multi-threaded packet processing
- **Zero-cost abstractions**: Performance without sacrificing maintainability
- **Strong type system**: Catches protocol state machine bugs at compile time
- **Modern tooling**: Cargo, built-in testing, excellent ecosystem

## Use Case

This implementation focuses on:
- **One-to-many broadcast**: Single stream → multiple transmission paths (cellular, WiFi, etc.)
- **Many-to-one bonding**: Multiple receivers → single combined stream
- **Quality maximization**: Packet alignment across varying bandwidth connections
- **Production CLI tools**: Real-world deployment with heterogeneous networks

## Installation

### Pre-built Binaries

Download pre-built binaries for your platform from the [Releases](https://github.com/dallasgutauckis/srt-rust/releases) page:

- **Linux x86_64**: For Docker containers and standard Linux servers
- **Linux ARM64**: For ARM-based servers (AWS Graviton, Raspberry Pi, etc.)
- **macOS x86_64**: For Intel Macs
- **macOS ARM64**: For Apple Silicon Macs

```bash
# Download and extract (example for Linux x86_64)
wget https://github.com/dallasgutauckis/srt-rust/releases/download/v0.1.0/srt-linux-x86_64-<version>.tar.gz
tar -xzf srt-linux-x86_64-<version>.tar.gz

# Make binaries executable and move to PATH
chmod +x srt-sender srt-receiver
sudo mv srt-sender srt-receiver /usr/local/bin/
```

### Docker

Pull the official Docker image:

```bash
# Pull latest version
docker pull ghcr.io/YOUR_USERNAME/srt-rust:latest

# Or specific version
docker pull ghcr.io/YOUR_USERNAME/srt-rust:<version>

# Run sender (reading from stdin)
docker run -i --rm -p 9000:9000/udp ghcr.io/YOUR_USERNAME/srt-rust:latest \
  srt-sender --path 0.0.0.0:9000

# Run receiver (outputting to stdout)
docker run --rm -p 9000:9000/udp ghcr.io/YOUR_USERNAME/srt-rust:latest \
  srt-receiver --listen 9000 --output -
```

### From Source

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Build release version
cargo build --release --workspace

# Run binaries
cargo run --bin srt-sender -- --help
cargo run --bin srt-receiver -- --help
```

## Usage

### Basic Streaming Pipeline

**Sender** (reads from stdin, sends to receiver):
```bash
ffmpeg -i input.mp4 -c copy -f mpegts - | \
  srt-sender --path receiver.example.com:9000
```

**Receiver** (receives stream, outputs to UDP):
```bash
srt-receiver --listen 9000 --output udp://127.0.0.1:5000
```

**Play in VLC/OBS**:
```bash
vlc udp://127.0.0.1:5000
# Or in OBS: Media Source → udp://127.0.0.1:5000
```

### Multi-Path Bonding

**Broadcast mode** (send on all paths):
```bash
srt-sender --group broadcast \
  --path 192.168.1.100:9000 \
  --path 10.0.0.100:9000
```

**Backup mode** (use secondary path on failure):
```bash
srt-sender --group backup \
  --path primary.example.com:9000 \
  --path backup.example.com:9000
```

**Load balancing mode** (distribute packets):
```bash
srt-sender --group balancing \
  --path path1.example.com:9000 \
  --path path2.example.com:9000
```

## Development

```bash
# Lint
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --workspace

# Run benchmarks
cargo bench --package srt-protocol

# Generate documentation
cargo doc --workspace --no-deps --open
```

## References

Based on the SRT protocol specification and the reference C/C++ implementation:
- [SRT GitHub Repository](https://github.com/Haivision/srt)
- [SRT Protocol Specification](https://datatracker.ietf.org/doc/html/draft-sharabayko-srt)
