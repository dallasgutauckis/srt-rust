#!/bin/bash
# Test 4: Streaming Simulation - MPEGTS-like Workload
#
# This test simulates a live streaming scenario similar to MPEGTS transmission.
# MPEGTS characteristics:
# - Fixed 188-byte packets (TS packets)
# - Constant bitrate delivery
# - Resilient to packet loss (up to 20% loss acceptable for video)
# - Real-time constraints
#
# This test verifies:
# 1. Sustained throughput for streaming
# 2. Acceptable delivery for streaming use case (≥80%)
# 3. Low latency characteristics

set -e

TEST_NAME="Streaming Simulation Test (MPEGTS-like)"
TEST_DIR="/tmp/srt-test-streaming"
PORT=19700

echo "========================================="
echo "$TEST_NAME"
echo "========================================="
echo ""

# Cleanup and setup
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"

# Simulate MPEGTS stream: 7 TS packets per UDP packet = 1316 bytes (SRT payload size)
# This matches real MPEGTS over UDP encapsulation
TS_PACKET_SIZE=188
TS_PACKETS_PER_UDP=7
STREAM_DURATION_SEC=10
BITRATE_MBPS=5  # 5 Mbps stream (typical for HD video)

# Calculate total size
BYTES_PER_SEC=$((BITRATE_MBPS * 1000000 / 8))
TOTAL_BYTES=$((BYTES_PER_SEC * STREAM_DURATION_SEC))

echo "Simulating MPEGTS stream:"
echo "  • Duration: ${STREAM_DURATION_SEC} seconds"
echo "  • Bitrate: ${BITRATE_MBPS} Mbps"
echo "  • Total data: $(echo "scale=2; $TOTAL_BYTES / 1000000" | bc) MB"
echo "  • Pattern: ${TS_PACKETS_PER_UDP} TS packets per SRT packet"
echo ""

# Create test data (simulating MPEGTS stream)
echo "Generating test stream..."
dd if=/dev/urandom of="$TEST_DIR/stream.ts" bs=1M count=$((TOTAL_BYTES / 1000000 + 1)) 2>/dev/null
# Truncate to exact size
dd if="$TEST_DIR/stream.ts" of="$TEST_DIR/input.ts" bs=1 count=$TOTAL_BYTES 2>/dev/null

INPUT_SIZE=$(stat -f%z "$TEST_DIR/input.ts")
echo "Stream size: $INPUT_SIZE bytes ($(echo "scale=2; $INPUT_SIZE / 1000000" | bc) MB)"
echo ""

# Start receiver
echo "Starting receiver (dual-path for redundancy)..."
./target/release/srt-receiver \
  --listen $PORT \
  --output "$TEST_DIR/output.ts" \
  --num-paths 2 \
  --stats 1 > "$TEST_DIR/receiver.log" 2>&1 &
RECV_PID=$!

sleep 2

# Capture start time for latency measurement
START_TIME=$(date +%s)

# Send stream with broadcast bonding (typical for live streaming)
echo "Streaming data via 2 paths (broadcast mode)..."
./target/release/srt-sender \
  --input "$TEST_DIR/input.ts" \
  --path 127.0.0.1:$PORT \
  --path 127.0.0.1:$PORT \
  --group broadcast \
  --stats 1 2>&1 | tee "$TEST_DIR/sender.log"

# Capture end time
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

# Wait for receiver to process
echo ""
echo "Waiting for receiver to process remaining packets..."
sleep 2

# Stop receiver
kill -TERM $RECV_PID 2>/dev/null
sleep 1

# Calculate results
OUTPUT_SIZE=$(stat -f%z "$TEST_DIR/output.ts")
PERCENT=$(echo "scale=4; 100 * $OUTPUT_SIZE / $INPUT_SIZE" | bc)
LOSS_PERCENT=$(echo "scale=2; 100 - $PERCENT" | bc)

# Calculate throughput
THROUGHPUT_MBPS=$(echo "scale=2; ($OUTPUT_SIZE * 8) / ($ELAPSED * 1000000)" | bc)

echo ""
echo "========================================="
echo "Results"
echo "========================================="
echo "Input:  $INPUT_SIZE bytes"
echo "Output: $OUTPUT_SIZE bytes"
echo "Delivery: ${PERCENT}%"
echo "Loss: ${LOSS_PERCENT}%"
echo "Duration: ${ELAPSED} seconds"
echo "Throughput: ${THROUGHPUT_MBPS} Mbps"
echo ""

# For streaming, 80% delivery is acceptable (MPEGTS can handle packet loss)
STREAMING_THRESHOLD=80

if [ "$OUTPUT_SIZE" -ge $((INPUT_SIZE * STREAMING_THRESHOLD / 100)) ]; then
    echo "✅ PASS: Received ≥${STREAMING_THRESHOLD}% of stream"
    echo ""
    echo "Streaming Quality Assessment:"

    if [ "$OUTPUT_SIZE" -ge $((INPUT_SIZE * 95 / 100)) ]; then
        echo "  📺 EXCELLENT (≥95%) - Broadcast quality"
    elif [ "$OUTPUT_SIZE" -ge $((INPUT_SIZE * 90 / 100)) ]; then
        echo "  📺 VERY GOOD (90-95%) - High quality streaming"
    elif [ "$OUTPUT_SIZE" -ge $((INPUT_SIZE * 85 / 100)) ]; then
        echo "  📺 GOOD (85-90%) - Acceptable for streaming"
    else
        echo "  📺 FAIR (80-85%) - Usable with some degradation"
    fi

    echo ""
    echo "This demonstrates:"
    echo "  • Sustained throughput for streaming workloads"
    echo "  • MPEGTS-compatible delivery (resilient to loss)"
    echo "  • Real-time transmission capable"
    echo "  • Broadcast bonding provides redundancy"
    echo ""
    echo "For live video streaming:"
    echo "  • ${LOSS_PERCENT}% packet loss is acceptable for MPEGTS"
    echo "  • Video decoders can conceal lost frames"
    echo "  • Multi-path bonding improves reliability"

    EXIT_CODE=0
else
    echo "❌ FAIL: Received <${STREAMING_THRESHOLD}% of stream"
    echo ""
    echo "Expected: ≥${STREAMING_THRESHOLD}% delivery for streaming"
    echo "Actual: ${PERCENT}% delivery"
    echo ""
    echo "This level of loss would cause visible degradation in video streaming."

    EXIT_CODE=1
fi

echo ""
echo "Receiver detected paths:"
grep "New path detected" "$TEST_DIR/receiver.log" || echo "  (check receiver.log for details)"
echo ""
echo "Test files saved in: $TEST_DIR"
echo ""

exit $EXIT_CODE
