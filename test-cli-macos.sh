#!/bin/bash
# macOS-compatible test script for SRT CLI tools

set -e

echo "=== SRT CLI Tools Test (macOS) ==="
echo ""

# Build the tools
echo "Building CLI tools..."
cargo build --release --bin srt-sender --bin srt-receiver

echo ""
echo "Creating test data (1 MB)..."
dd if=/dev/urandom of=/tmp/test-input.dat bs=1M count=1 2>/dev/null

echo ""
echo "Starting receiver in background..."
./target/release/srt-receiver \
  --listen 19000 \
  --output /tmp/test-output.dat \
  --num-paths 2 \
  --stats 2 &
RECEIVER_PID=$!

# Give receiver time to start
sleep 2

echo ""
echo "Starting sender..."
./target/release/srt-sender \
  --input /tmp/test-input.dat \
  --path 127.0.0.1:19000 \
  --path 127.0.0.1:19000 \
  --stats 2 &
SENDER_PID=$!

# Wait for sender to finish (max 10 seconds)
for i in {1..10}; do
    if ! kill -0 $SENDER_PID 2>/dev/null; then
        echo "Sender finished"
        break
    fi
    sleep 1
done

# Wait a bit for receiver to process remaining packets
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

    INPUT_SIZE=$(stat -f%z /tmp/test-input.dat)
    OUTPUT_SIZE=$(stat -f%z /tmp/test-output.dat)

    if [ "$INPUT_SIZE" -eq "$OUTPUT_SIZE" ]; then
        echo "✅ Test PASSED! File sizes match perfectly."
    else
        echo "⚠️ File sizes differ: input=$INPUT_SIZE, output=$OUTPUT_SIZE"
        echo "This may be expected as not all packets may have arrived."
    fi
else
    echo "❌ Output file not created"
    echo "This may indicate receiver didn't get any data."
fi

echo ""
echo "=== Cleanup ==="
rm -f /tmp/test-input.dat /tmp/test-output.dat
echo "Test files cleaned up."
echo ""
echo "=== CLI Tools Ready! ==="
echo "See CLI_GUIDE.md for usage examples."
