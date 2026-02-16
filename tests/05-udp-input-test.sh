#!/bin/bash
# Test 5: UDP Input - Receive UDP stream and send via multi-path bonding
#
# This test demonstrates:
# 1. srt-sender receiving UDP input
# 2. Converting UDP stream to multi-path bonded SRT
# 3. Typical encoder → UDP → srt-sender → multi-path workflow

set -e

TEST_NAME="UDP Input Test"
TEST_DIR="/tmp/srt-test-udp-input"
UDP_PORT=5000
SRT_PORT=19800

echo "========================================="
echo "$TEST_NAME"
echo "========================================="
echo ""

# Cleanup and setup
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"

# Create test data
TEST_SIZE_MB=2
echo "Creating ${TEST_SIZE_MB}MB test stream..."
dd if=/dev/urandom of="$TEST_DIR/test-stream.dat" bs=1M count=$TEST_SIZE_MB 2>/dev/null

INPUT_SIZE=$(stat -f%z "$TEST_DIR/test-stream.dat")
echo "Stream size: $INPUT_SIZE bytes"
echo ""

# Start SRT receiver (will receive the multi-path bonded stream)
echo "Starting SRT receiver on port $SRT_PORT..."
./target/release/srt-receiver \
  --listen $SRT_PORT \
  --output "$TEST_DIR/output.dat" \
  --num-paths 2 \
  --stats 2 > "$TEST_DIR/receiver.log" 2>&1 &
RECV_PID=$!

sleep 2

# Start srt-sender with UDP input (receives UDP, sends via multi-path SRT)
echo "Starting srt-sender with UDP input on port $UDP_PORT..."
./target/release/srt-sender \
  --input udp://:$UDP_PORT \
  --path 127.0.0.1:$SRT_PORT \
  --path 127.0.0.1:$SRT_PORT \
  --group broadcast \
  --stats 2 > "$TEST_DIR/sender.log" 2>&1 &
SENDER_PID=$!

sleep 2

echo ""
echo "UDP input listening on: 0.0.0.0:$UDP_PORT"
echo "Sending stream to: multi-path SRT (2 paths to $SRT_PORT)"
echo ""

# Simulate encoder sending UDP stream
# In production this would be: ffmpeg -i ... -f mpegts udp://localhost:5000
echo "Simulating UDP stream source (sending test data via UDP)..."

# Use netcat or a simple UDP sender
(
    # Send file in chunks via UDP
    CHUNK_SIZE=1316
    while IFS= read -r -n $CHUNK_SIZE -d '' chunk || [ -n "$chunk" ]; do
        echo -n "$chunk" | nc -u -w0 127.0.0.1 $UDP_PORT
        sleep 0.001  # Small delay to avoid overwhelming
    done < "$TEST_DIR/test-stream.dat"

    echo "UDP transmission complete"
) &
UDP_SENDER_PID=$!

# Wait for UDP transmission to complete
wait $UDP_SENDER_PID

echo ""
echo "Waiting for srt-sender and srt-receiver to process..."
sleep 3

# Stop sender and receiver
kill -TERM $SENDER_PID 2>/dev/null || true
sleep 1
kill -TERM $RECV_PID 2>/dev/null || true
sleep 1

# Calculate results
OUTPUT_SIZE=$(stat -f%z "$TEST_DIR/output.dat" 2>/dev/null || echo "0")
if [ "$OUTPUT_SIZE" -eq 0 ]; then
    PERCENT="0"
else
    PERCENT=$(echo "scale=2; 100 * $OUTPUT_SIZE / $INPUT_SIZE" | bc)
fi

echo ""
echo "========================================="
echo "Results"
echo "========================================="
echo "Input (UDP):  $INPUT_SIZE bytes"
echo "Output (SRT): $OUTPUT_SIZE bytes"
echo "Delivery: ${PERCENT}%"
echo ""

# Check if we got acceptable delivery
THRESHOLD=70
if [ "$OUTPUT_SIZE" -ge $((INPUT_SIZE * THRESHOLD / 100)) ]; then
    echo "✅ PASS: UDP → Multi-path SRT workflow working"
    echo ""
    echo "This demonstrates:"
    echo "  • srt-sender receiving UDP input"
    echo "  • Converting UDP stream to bonded SRT"
    echo "  • Multi-path transmission working"
    echo ""
    echo "Production workflow:"
    echo "  ffmpeg → UDP → srt-sender → Multi-path SRT → srt-receiver → Output"
    echo ""
    echo "Example:"
    echo "  # Encoder (ffmpeg)"
    echo "  ffmpeg -i camera.mp4 -c:v libx264 -f mpegts udp://localhost:5000"
    echo ""
    echo "  # Multi-path sender"
    echo "  ./srt-sender --input udp://:5000 \\"
    echo "    --path 10.0.1.1:9000 \\"
    echo "    --path 10.0.2.1:9000 \\"
    echo "    --path 192.168.1.100:9000"

    EXIT_CODE=0
else
    echo "❌ FAIL: Received <${THRESHOLD}% of data"
    echo ""
    echo "Check logs:"
    echo "  • Sender: $TEST_DIR/sender.log"
    echo "  • Receiver: $TEST_DIR/receiver.log"

    EXIT_CODE=1
fi

echo ""
echo "Test files saved in: $TEST_DIR"
echo ""

exit $EXIT_CODE
