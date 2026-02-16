#!/bin/bash
# Cross-compile SRT CLI tools for ARM Linux devices (Raspberry Pi, etc.)

set -e

TARGET="aarch64-unknown-linux-gnu"

echo "========================================="
echo "Building SRT CLI for ARM Linux"
echo "========================================="
echo ""
echo "Target: $TARGET"
echo ""

# Install target if not already installed
echo "Ensuring target is installed..."
rustup target add $TARGET

# Install cross-compilation toolchain (if not using Docker)
if ! command -v aarch64-linux-gnu-gcc &> /dev/null; then
    echo ""
    echo "⚠️  ARM cross-compiler not found."
    echo ""
    echo "Install with:"
    echo "  • macOS: brew install filosottile/musl-cross/musl-cross"
    echo "  • Or use Docker: docker run --rm -v $(pwd):/workspace -w /workspace rust:latest ..."
    echo ""
    echo "Attempting build anyway (may fail)..."
    echo ""
fi

# Build
echo "Building for $TARGET..."
cargo build --release --target $TARGET --bin srt-sender --bin srt-receiver

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Build successful!"
    echo ""
    echo "Binaries:"
    ls -lh target/$TARGET/release/srt-{sender,receiver} 2>/dev/null || echo "  (check target/$TARGET/release/)"
    echo ""
    echo "Transfer to your ARM Linux device:"
    echo "  scp target/$TARGET/release/srt-{sender,receiver} pi@raspberrypi.local:~/"
else
    echo ""
    echo "❌ Build failed"
    echo ""
    echo "For easier cross-compilation, use 'cross':"
    echo "  cargo install cross"
    echo "  cross build --release --target $TARGET --bin srt-sender --bin srt-receiver"
fi
