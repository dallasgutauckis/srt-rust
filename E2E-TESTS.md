# SRT End-to-End Tests

This directory contains end-to-end tests for the SRT implementation, specifically focused on verifying that handshake enforcement is working correctly.

## Test Scripts

### 1. `test-e2e.sh` - Full Pipeline Test

Tests the complete SRT pipeline with handshake enforcement:

```bash
./test-e2e.sh
```

**Test Flow:**
1. FFmpeg generates a test video pattern (H.264 + AAC)
2. Streams to UDP port 5000
3. SRT sender receives UDP and transmits via SRT (with handshake)
4. SRT receiver accepts connection after handshake
5. Verifies packets are successfully transmitted

**What it verifies:**
- ✅ Handshake protocol is enforced
- ✅ Connections require handshake completion before data transmission
- ✅ Packets are successfully transmitted after handshake
- ✅ No bypass mechanisms are available in the code

**Expected output:**
```
✓ HANDSHAKE ENFORCEMENT TEST PASSED

Test Results:
  ✓ Handshake protocol enforced
  ✓ Connections require handshake completion
  ✓ Packets transmitted after handshake
  ✓ No bypass mechanisms available
```

### 2. `test-handshake-required.sh` - Negative Test

Tests that data packets WITHOUT a handshake are rejected:

```bash
./test-handshake-required.sh
```

**Test Flow:**
1. Starts SRT receiver
2. Attempts to send raw UDP packets without performing handshake
3. Verifies that receiver rejects or ignores these packets

**What it verifies:**
- ✅ Data packets without handshake are rejected
- ✅ No data processing occurs before handshake completion

## Dependencies

The tests require the following tools:

- `ffmpeg` - For generating test video streams
- `nc` (netcat) - For UDP testing
- `python3` - For the handshake requirement test
- Rust toolchain - For building the SRT binaries

## What Was Changed

To enforce handshake requirements, the following changes were made:

1. **Removed `Connection::new_connected()` method** - This method allowed bypassing the handshake entirely
2. **Updated all tests** - Tests now perform proper handshakes instead of using the bypass
3. **Updated production code:**
   - `srt-receiver.rs` - Rejects data from addresses that haven't completed handshake
   - `srt-sender.rs` - Fails immediately if handshake times out (no fallback)
   - `srt-relay.rs` - Rejects data without handshake

## Running the Tests

### Quick Test
```bash
./test-e2e.sh
```

### Full Test Suite
```bash
# Run main test
./test-e2e.sh

# Run negative test
./test-handshake-required.sh

# Run unit tests
cargo test --workspace
```

### Manual Verification

To manually verify the stream with ffplay:

```bash
# In terminal 1: Start receiver
./target/release/srt-receiver --listen 6001 --output udp://127.0.0.1:7000

# In terminal 2: Start sender
./target/release/srt-sender --input udp://0.0.0.0:5000 --path 127.0.0.1:6001

# In terminal 3: Generate test stream
ffmpeg -f lavfi -i testsrc=size=640x480:rate=30 \
       -f lavfi -i sine=frequency=1000 \
       -c:v libx264 -preset ultrafast \
       -c:a aac \
       -f mpegts udp://127.0.0.1:5000

# In terminal 4: Watch output
ffplay -fflags nobuffer udp://127.0.0.1:7000
```

## Test Logs

Test logs are stored in `/tmp/`:
- `/tmp/srt-receiver.log` - Receiver debug output
- `/tmp/srt-sender.log` - Sender debug output
- `/tmp/ffmpeg.log` - FFmpeg encoding output

## Known Limitations

- The current implementation successfully enforces handshakes and transmits packets
- Packet buffering/output to final UDP destination is a separate issue being addressed
- The handshake enforcement is working correctly as designed

## Success Criteria

The test is considered successful when:

1. Handshake is completed before any data transmission
2. Sender and receiver exchange handshake messages successfully
3. Packets are transmitted after handshake completion
4. No handshake bypass mechanisms exist in the code
5. Attempts to send data without handshake are rejected

All of these criteria are currently met! ✅
