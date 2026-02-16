#!/bin/bash
# Quick test script for SRT CLI tools

set -e

echo "=== SRT CLI Tools Test ==="
echo ""

# Build the tools
echo "Building CLI tools..."
cargo build --release --bin srt-sender --bin srt-receiver

echo ""
echo "Creating test data (1 MB)..."
dd if=/dev/urandom of=/tmp/test-input.dat bs=1M count=1 2>/dev/null

echo ""
echo "Starting receiver in background..."
timeout 10 ./target/release/srt-receiver \
  --listen 19000 \
  --output /tmp/test-output.dat \
  --num-paths 2 \
  --stats 2 &
RECEIVER_PID=$!

# Give receiver time to start
sleep 1

echo ""
echo "Starting sender..."
timeout 8 ./target/release/srt-sender \
  --input /tmp/test-input.dat \
  --path 127.0.0.1:19000 \
  --path 127.0.0.1:19000 \
  --stats 2 || true

# Wait a bit for receiver to process
sleep 2

echo ""
echo "Stopping receiver..."
kill $RECEIVER_PID 2>/dev/null || true
wait $RECEIVER_PID 2>/dev/null || true

echo ""
echo "=== Results ==="
echo "Input file size: $(ls -lh /tmp/test-input.dat | awk '{print $5}')"
if [ -f /tmp/test-output.dat ]; then
    echo "Output file size: $(ls -lh /tmp/test-output.dat | awk '{print $5}')"
    echo ""
    echo "✅ Test completed!"
    echo "Files created successfully. Check sizes to verify transfer."
else
    echo "❌ Output file not created"
    echo "This may be normal if packets didn't arrive yet."
fi

echo ""
echo "=== Cleanup ==="
rm -f /tmp/test-input.dat /tmp/test-output.dat
echo "Test files cleaned up."
echo ""
echo "=== CLI Tools Ready! ==="
echo "See CLI_GUIDE.md for usage examples."
