# SRT CLI End-to-End Tests

Comprehensive test suite for validating SRT multi-path bonding functionality under various network conditions.

## Quick Start

Run all tests:
```bash
cd srt-rust
./tests/run-all-tests.sh
```

Run individual tests:
```bash
./tests/01-perfect-conditions.sh
./tests/02-lossy-conditions.sh
./tests/03-multipath-bonding.sh
./tests/04-streaming-simulation.sh
```

## Test Descriptions

### 1. Perfect Conditions Test (`01-perfect-conditions.sh`)

**Purpose**: Verify 100% packet delivery under ideal conditions.

**Scenario**:
- Local loopback (127.0.0.1)
- No packet loss
- Dual-path broadcast bonding
- 5 MB test file

**Pass Criteria**: ≥99% of data received

**What This Tests**:
- Basic sender/receiver functionality
- Packet serialization/deserialization
- Duplicate detection (same data sent on 2 paths, received once)
- In-order packet delivery
- Multi-path bonding basics

**Expected Result**: ~100% delivery (minor buffer flush tolerance)

### 2. Lossy Conditions Test (`02-lossy-conditions.sh`)

**Purpose**: Verify acceptable delivery despite packet loss.

**Scenario**:
- Simulated packet loss (~20%) via receiver pause/resume
- 5 MB test file
- Broadcast bonding for redundancy

**Pass Criteria**: ≥70% of data received

**What This Tests**:
- Resilience to packet loss
- Graceful degradation under poor conditions
- Broadcast redundancy benefits
- Acceptable for streaming use cases

**Expected Result**: 70-90% delivery (depending on loss simulation timing)

**Why This Matters**: Real-world networks (especially cellular) experience packet loss. The system should degrade gracefully.

### 3. Multi-Path Bonding Test (`03-multipath-bonding.sh`)

**Purpose**: Verify true multi-path transmission over distinct paths.

**Scenario**:
- 3 separate receivers on different ports (simulating cellular1, cellular2, WiFi)
- Broadcast to all 3 paths
- Intentional path failure (kill one receiver)
- 3 MB test file

**Pass Criteria**: ≥2 paths receive significant data

**What This Tests**:
- Multi-path transmission
- Independent stream delivery per path
- Path failure resilience
- Broadcast bonding to multiple destinations

**Expected Result**: All 3 paths receive data initially, 2 continue after simulated failure

**Real-World Equivalent**:
- Path 1 = Cellular modem 1 → receiver
- Path 2 = Cellular modem 2 → receiver
- Path 3 = WiFi → receiver

In production, a single receiver would bond all paths and eliminate duplicates automatically.

### 4. Streaming Simulation Test (`04-streaming-simulation.sh`)

**Purpose**: Verify suitability for live video streaming (MPEGTS).

**Scenario**:
- MPEGTS-like workload (188-byte TS packets, 7 per SRT packet)
- 5 Mbps bitrate simulation
- 10-second stream duration
- Dual-path broadcast bonding

**Pass Criteria**: ≥80% of stream received

**What This Tests**:
- Sustained throughput for streaming
- MPEGTS-compatible delivery
- Real-time transmission capability
- Streaming quality tiers (Excellent/Good/Fair)

**Expected Result**:
- **95-100%**: Excellent (broadcast quality)
- **90-95%**: Very Good (high quality streaming)
- **85-90%**: Good (acceptable for streaming)
- **80-85%**: Fair (usable with degradation)

**Why MPEGTS?**: MPEG Transport Stream is designed for lossy networks. Video decoders can conceal lost frames, making 10-20% packet loss acceptable for live streaming.

## Test Artifacts

All tests save artifacts to `/tmp/srt-test-*/`:
- `input.dat` / `input.ts` - Original test data
- `output.dat` / `output.ts` - Received data
- `sender.log` - Sender detailed logs
- `receiver.log` / `receiver1.log`, etc. - Receiver detailed logs

Inspect these files for debugging if tests fail.

## Interpreting Packet Loss

### Why Some Loss is Expected

Even with `kill -TERM` (graceful shutdown), some loss occurs because:

1. **Buffer Flush Timing**: The receiver flushes every 50 packets. Final packets may not be flushed before shutdown.
2. **In-Flight Packets**: Packets in UDP buffers when receiver stops are lost.
3. **Test Limitations**: These are simulations, not perfect network conditions.

### Acceptable Loss Rates

| Use Case | Acceptable Loss | Quality |
|----------|----------------|---------|
| File transfer | <1% | Perfect |
| Live streaming (MPEGTS) | 5-15% | Good |
| Real-time video | 10-20% | Acceptable |
| Voice (opus) | 20-30% | Usable with concealment |

The tests validate that the system performs within these bounds.

## Real-World Example

```bash
# Field camera (4 cellular modems + WiFi)
ffmpeg -f v4l2 -i /dev/video0 -c:v libx264 -f mpegts - | \
  ./srt-sender \
    --input - \
    --path 10.0.1.1:9000 \  # Cellular 1
    --path 10.0.1.2:9000 \  # Cellular 2
    --path 10.0.1.3:9000 \  # Cellular 3
    --path 10.0.1.4:9000 \  # Cellular 4
    --path 192.168.1.100:9000  # WiFi backup

# Control room (receive and play)
./srt-receiver \
  --listen 9000 \
  --output - \
  --num-paths 5 | ffplay -
```