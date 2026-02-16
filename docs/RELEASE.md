# Release Guide

## Creating a Release

1. **Update version** in all `Cargo.toml` files
2. **Update CHANGELOG.md** with release notes
3. **Commit changes**:
   ```bash
   git add .
   git commit -m "Release v0.1.0"
   ```

4. **Create and push tag**:
   ```bash
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin main --tags
   ```

5. **GitHub Actions will automatically**:
   - Build binaries for all platforms (Linux x86_64, Linux ARM64, macOS x86_64, macOS ARM64)
   - Create GitHub release
   - Upload release artifacts
   - Build and push Docker images to GitHub Container Registry

## Platform Targets

### Linux x86_64 (`x86_64-unknown-linux-gnu`)
- **Use case**: Docker containers, standard Linux servers
- **Build**: Native on Ubuntu
- **Binary**: `srt-linux-x86_64-VERSION.tar.gz`

### Linux ARM64 (`aarch64-unknown-linux-gnu`)
- **Use case**: ARM servers (AWS Graviton, Raspberry Pi 4+, etc.)
- **Build**: Cross-compilation using `cross`
- **Binary**: `srt-linux-arm64-VERSION.tar.gz`

### macOS x86_64 (`x86_64-apple-darwin`)
- **Use case**: Intel Macs
- **Build**: Native on macOS
- **Binary**: `srt-macos-x86_64-VERSION.tar.gz`

### macOS ARM64 (`aarch64-apple-darwin`)
- **Use case**: Apple Silicon Macs (M1, M2, M3, M4)
- **Build**: Cross-compilation on macOS
- **Binary**: `srt-macos-arm64-VERSION.tar.gz`

## Docker Images

Docker images are built for both `linux/amd64` and `linux/arm64` platforms.

**Tags**:
- `ghcr.io/YOUR_USERNAME/srt-rust:VERSION` - Specific version
- `ghcr.io/YOUR_USERNAME/srt-rust:latest` - Latest release

**Using the image**:
```bash
# Pull
docker pull ghcr.io/YOUR_USERNAME/srt-rust:latest

# Run sender
docker run -i --rm ghcr.io/YOUR_USERNAME/srt-rust:latest \
  srt-sender --path receiver.example.com:9000

# Run receiver
docker run --rm -p 9000:9000/udp ghcr.io/YOUR_USERNAME/srt-rust:latest \
  srt-receiver --listen 9000 --output -
```

## Manual Building

### Cross-compile for ARM on macOS:
```bash
# Install target
rustup target add aarch64-apple-darwin

# Build
cargo build --release --target aarch64-apple-darwin
```

### Cross-compile for ARM Linux:
```bash
# Install cross
cargo install cross

# Build
cross build --release --target aarch64-unknown-linux-gnu
```

### Build Docker image locally:
```bash
# Single platform
docker build -t srt-rust:local .

# Multi-platform (requires buildx)
docker buildx build --platform linux/amd64,linux/arm64 -t srt-rust:local .
```

## Testing Releases

Before tagging, test on all target platforms:

```bash
# Linux x86_64
cargo test --target x86_64-unknown-linux-gnu

# Linux ARM64 (requires cross)
cross test --target aarch64-unknown-linux-gnu

# macOS x86_64
cargo test --target x86_64-apple-darwin

# macOS ARM64
cargo test --target aarch64-apple-darwin
```

## Troubleshooting

### Cross-compilation fails for ARM
- Ensure `cross` is installed: `cargo install cross`
- Check Docker is running (cross uses Docker internally)
- Update cross: `cargo install --force cross`

### Docker build fails
- Check Dockerfile syntax
- Verify all workspace crates are included
- Test build locally: `docker build .`

### GitHub Actions fails
- Check workflow syntax
- Verify GitHub token permissions
- Check runner platform availability
