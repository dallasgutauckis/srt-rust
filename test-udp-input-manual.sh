#!/bin/bash
# Manual UDP Input Test - Simple demonstration

echo "========================================="
echo "UDP Input Test - Manual"
echo "========================================="
echo ""
echo "This test demonstrates UDP input to srt-sender"
echo ""

# Use unique ports to avoid conflicts
UDP_PORT=15000
SRT_PORT=15001

# Create small test file
dd if=/dev/urandom of=/tmp/udp-test.dat bs=1K count=100 2>/dev/null
echo "Created 100KB test file"
echo ""

# Start receiver
echo "Starting SRT receiver..."
./target/release/srt-receiver \
  --listen $SRT_PORT \
  --output /tmp/udp-output.dat \
  --num-paths 2 &
RECV_PID=$!

sleep 1

# Start sender with UDP input
echo "Starting SRT sender with UDP input..."
./target/release/srt-sender \
  --input udp://:$UDP_PORT \
  --path 127.0.0.1:$SRT_PORT \
  --path 127.0.0.1:$SRT_PORT &
SENDER_PID=$!

sleep 2

echo ""
echo "Sending test data via UDP to port $UDP_PORT..."
echo ""

# Send data via UDP using nc (netcat)
cat /tmp/udp-test.dat | nc -u 127.0.0.1 $UDP_PORT &
NC_PID=$!

# Wait for data to be sent
sleep 3

# Check results
echo "Stopping processes..."
kill $SENDER_PID 2>/dev/null || true
sleep 1
kill $RECV_PID 2>/dev/null || true
sleep 1

echo ""
INPUT_SIZE=$(stat -f%z /tmp/udp-test.dat)
OUTPUT_SIZE=$(stat -f%z /tmp/udp-output.dat 2>/dev/null || echo "0")

echo "Input (UDP):  $INPUT_SIZE bytes"
echo "Output (SRT): $OUTPUT_SIZE bytes"

if [ "$OUTPUT_SIZE" -gt 0 ]; then
    PERCENT=$(echo "scale=1; 100 * $OUTPUT_SIZE / $INPUT_SIZE" | bc)
    echo "Delivery: ${PERCENT}%"
    echo ""
    echo "✅ UDP input working!"
else
    echo ""
    echo "⚠️  No data received (may need to adjust timing)"
fi

# Cleanup
rm -f /tmp/udp-test.dat /tmp/udp-output.dat

echo ""
echo "Test complete"
