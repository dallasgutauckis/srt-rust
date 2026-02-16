# SRT CLI Tools - Quick Start Guide

## Overview

The SRT CLI tools provide production-ready multi-path streaming capabilities:
- **`srt-sender`** - Send streams over multiple network paths with bonding
- **`srt-receiver`** - Receive and combine streams from multiple paths

## Features

**Multi-Path Bonding**
- Broadcast mode: Send to all paths, receive from fastest
- Backup mode: Primary/backup with automatic failover
- Load balancing: Distribute packets across paths

**Production Ready**
- Real UDP sockets with proper error handling
- Duplicate packet detection and elimination
- Packet alignment across varying latency paths
- Real-time statistics

---

## Installation

```bash
cd srt-rust
cargo build --release --bin srt-sender
cargo build --release --bin srt-receiver
```

Binaries will be in `target/release/`

---

## Usage Examples

### Example 1: Basic Broadcast Over 2 Paths

**Sender** (send video over 2 cellular connections):
```bash
./srt-sender \
  --input video.ts \
  --group broadcast \
  --path 192.168.1.10:9000 \
  --path 192.168.1.11:9000 \
  --stats 1 \
  --verbose
```

**Receiver** (receive combined stream):
```bash
./srt-receiver \
  --listen 9000 \
  --output output.ts \
  --group broadcast \
  --num-paths 2 \
  --stats 1
```

### Example 2: Broadcast Over 4 Paths (Max Reliability)

**Sender** (4 cellular + WiFi):
```bash
./srt-sender \
  --input - \
  --group broadcast \
  --path 10.0.1.1:9000 \
  --path 10.0.1.2:9000 \
  --path 10.0.1.3:9000 \
  --path 192.168.1.100:9000 \
  --stats 1 < video.ts
```

**Receiver**:
```bash
./srt-receiver \
  --listen 9000 \
  --output - \
  --group broadcast \
  --num-paths 4 | ffplay -
```

### Example 3: Live Streaming Pipeline

**Send camera feed over 3 paths**:
```bash
ffmpeg -f v4l2 -i /dev/video0 -c:v libx264 -f mpegts - | \
  ./srt-sender \
    --input - \
    --group broadcast \
    --path 192.168.1.10:9000 \
    --path 192.168.1.11:9000 \
    --path 192.168.1.12:9000
```

**Receive and play**:
```bash
./srt-receiver \
  --listen 9000 \
  --output - \
  --group broadcast \
  --num-paths 3 | ffplay -
```

---

## Command-Line Reference

### srt-sender

```
SRT multi-path sender

Usage: srt-sender [OPTIONS]

Options:
  -i, --input <INPUT>              Input file (use '-' for stdin) [default: -]
  -g, --group <GROUP>              Bonding mode (broadcast, backup, balancing) [default: broadcast]
  -p, --path <PATH>                Output paths (format: host:port) [can be repeated]
      --fec-overhead <FEC_OVERHEAD> FEC overhead percentage [default: 0]
      --stats <STATS>              Statistics interval in seconds [default: 1]
  -v, --verbose                    Verbose output
  -h, --help                       Print help
```

### srt-receiver

```
SRT multi-path receiver

Usage: srt-receiver [OPTIONS] --listen <LISTEN>

Options:
  -o, --output <OUTPUT>      Output file (use '-' for stdout) [default: -]
  -g, --group <GROUP>        Bonding mode (broadcast, backup, balancing) [default: broadcast]
  -l, --listen <LISTEN>      Listen port
      --num-paths <NUM_PATHS> Expected number of paths [default: 1]
      --stats <STATS>        Statistics interval in seconds [default: 1]
  -v, --verbose              Verbose output
  -h, --help                 Print help
```

---

## How It Works

### Broadcast Mode (Recommended)

**Sender**:
1. Reads data chunks from input
2. Sends same chunk to ALL paths simultaneously
3. Each packet gets a sequence number

**Receiver**:
1. Receives packets from all paths
2. Detects and eliminates duplicates automatically
3. Delivers packets in correct sequence order
4. Uses whichever path delivers first (fastest path wins!)

**Benefits**:
- Maximum reliability (data sent on all paths)
- Automatic path selection (fastest path delivers)
- Seamless handling of path failures
- No data loss if at least one path works

### Backup Mode

**Primary/backup with automatic failover**:
- Sends on primary path only
- Monitors primary health
- Automatically switches to backup if primary fails
- Seamless failover without data loss

### Load Balancing Mode

**Distributes packets across paths**:
- Different packets go to different paths
- Weighted by path bandwidth/RTT
- More efficient bandwidth utilization
- Receiver combines all packets

---

## Network Setup Tips

### 1. Multiple Cellular Modems

```bash
# Each modem gets its own IP/interface
# Path 1: Cellular modem 1 → 10.0.1.1
# Path 2: Cellular modem 2 → 10.0.1.2
# Path 3: Cellular modem 3 → 10.0.1.3

./srt-sender \
  --path 10.0.1.1:9000 \
  --path 10.0.1.2:9000 \
  --path 10.0.1.3:9000
```

### 2. Cellular + WiFi

```bash
# Mix different network types
./srt-sender \
  --path 10.0.1.1:9000    # Cellular
  --path 192.168.1.10:9000 # WiFi
```

### 3. Multi-WAN Setup

```bash
# Different ISPs/WAN connections
./srt-sender \
  --path 203.0.113.10:9000  # ISP 1
  --path 198.51.100.20:9000 # ISP 2
```

---

## Performance Tuning

### Buffer Sizes

The tools use optimized buffer sizes by default (1316 byte chunks for SRT).

### Statistics

Enable stats to monitor performance:
```bash
--stats 1  # Print stats every 1 second
```

Output shows:
- Number of paths/members
- Packets buffered vs ready
- Throughput (Mbps)
- Packet count

### Verbosity

Use `--verbose` for detailed debugging:
```bash
./srt-sender --verbose  # Shows packet-level details
```

---

## Testing Locally

Test on the same machine using localhost:

**Terminal 1 (receiver)**:
```bash
./srt-receiver --listen 9000 --output received.dat --num-paths 2
```

**Terminal 2 (sender)**:
```bash
dd if=/dev/urandom bs=1M count=10 | \
  ./srt-sender \
    --input - \
    --path 127.0.0.1:9000 \
    --path 127.0.0.1:9000
```

**Verify**:
```bash
# Check file size
ls -lh received.dat

# Should be ~10 MB
```

---

## Real-World Example: Multi-Camera Live Stream

**Field camera with 4 cellular modems + WiFi backup**:

```bash
#!/bin/bash
# capture-and-send.sh

ffmpeg -f v4l2 -i /dev/video0 \
       -c:v libx264 -preset ultrafast -tune zerolatency \
       -b:v 2M -f mpegts - | \
  ./srt-sender \
    --input - \
    --group broadcast \
    --path 10.0.1.1:9000 \
    --path 10.0.1.2:9000 \
    --path 10.0.1.3:9000 \
    --path 10.0.1.4:9000 \
    --path 192.168.1.100:9000 \
    --stats 2 \
    --verbose 2>&1 | tee sender.log
```

**Control room receiver**:

```bash
#!/bin/bash
# receive-and-broadcast.sh

./srt-receiver \
  --listen 9000 \
  --output - \
  --group broadcast \
  --num-paths 5 \
  --stats 2 \
  2>&1 | tee receiver.log | \
  ffmpeg -i - -c copy -f rtmp rtmp://streaming-server/live/camera1
```

---

## Troubleshooting

### "No active members" error

**Cause**: Sender has no active paths
**Fix**: Check network connectivity to all paths

### Receiver shows 0 packets

**Cause**: Firewall blocking, wrong port
**Fix**:
```bash
# Check firewall
sudo ufw allow 9000/udp

# Verify receiver is listening
netstat -an | grep 9000
```

### High latency / buffering

**Cause**: One path much slower than others
**Fix**: Remove slow path or use backup mode

---

## Next Steps

- CLI tools are working!
- Try local test first
- Test over real networks with multiple paths
- Monitor statistics to verify bonding is working
- Deploy in production!

---

## Current Limitations

1. **No encryption** - Data sent in clear text (Phase 7 - can add later)
2. **No FEC** - Relies on retransmission (Phase 4 - can add later)
3. **Manual path specification** - Paths must be configured (auto-discovery could be added)

These are all planned features that can be added incrementally!

---

## Success Criteria

You have working multi-path bonding if:
- Sender reports "Transmission complete" with packet count
- Receiver reports packets received with Mbps stats
- Output file matches input file size (for file transfers)
- Stream plays smoothly (for live video)
- Removing one path doesn't interrupt stream (broadcast mode)

---

**Questions? Issues?**
- Check logs with `--verbose`
- Review firewall/network config
- Test locally first before multi-machine setup

**Enjoy your resilient multi-path streaming!** 🎉
