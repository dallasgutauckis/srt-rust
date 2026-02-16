# srt-relay - Multi-Format Stream Relay Guide

The `srt-relay` tool receives streams in one format and outputs to **multiple destinations** in various formats simultaneously.

## ✨ Key Features

- **Multi-format input**: SRT, UDP, file, stdin
- **Multi-format output**: UDP, file, stdout
- **Multiple outputs**: Send same stream to many destinations at once
- **Format conversion**: SRT → UDP, UDP → file, etc.
- **Broadcasting**: One-to-many streaming

## 🎯 Quick Examples

### Example 1: SRT to Multiple UDP Destinations

```bash
# Receive bonded SRT, send to 3 UDP destinations
./srt-relay \
  --input srt://:9000 \
  --output udp://192.168.1.10:5000 \
  --output udp://192.168.1.11:5000 \
  --output udp://192.168.1.12:5000 \
  --num-paths 2
```

**Use case**: Receive multi-path bonded SRT from field, distribute via UDP to multiple monitors/decoders.

### Example 2: UDP to File + UDP

```bash
# Receive UDP, save to file AND forward to another server
./srt-relay \
  --input udp://:5000 \
  --output file:/tmp/recording.ts \
  --output udp://backup-server:5000
```

**Use case**: Record stream while forwarding for live playback.

### Example 3: File to Multiple UDP Outputs

```bash
# Read file, send to multiple destinations
./srt-relay \
  --input video.ts \
  --output udp://player1:5000 \
  --output udp://player2:5000 \
  --output udp://player3:5000
```

**Use case**: Replay test file to multiple receivers simultaneously.

### Example 4: SRT to File + Stdout

```bash
# Receive SRT, save to file and pipe to ffplay
./srt-relay \
  --input srt://:9000 \
  --output file:/tmp/stream.ts \
  --output - \
  --num-paths 3 | ffplay -
```

**Use case**: Monitor and record incoming bonded SRT stream.

## 📊 Complete Workflow Examples

### Workflow 1: Field Camera → Production Distribution

```
Field:
  Camera → srt-sender → Multi-path SRT → Production server

Production server:
  ./srt-relay \
    --input srt://:9000 \
    --output udp://monitor1:5000 \      # Preview monitor
    --output udp://monitor2:5000 \      # Director monitor
    --output udp://encoder:5000 \       # Main encoder
    --output file:/mnt/recordings/live.ts  # Recording
    --num-paths 4
```

### Workflow 2: Live Event Broadcasting

```
Event venue:
  OBS → srt-sender → Multi-path → Relay server

Relay server:
  ./srt-relay \
    --input srt://:9000 \
    --output udp://cdn1:5000 \
    --output udp://cdn2:5000 \
    --output udp://backup-cdn:5000 \
    --output file:/archive/event-$(date +%Y%m%d).ts \
    --num-paths 2
```

### Workflow 3: Multi-Encoder Distribution

```
# Single SRT input → Multiple encoding formats

# Relay to multiple encoders
./srt-relay \
  --input srt://:9000 \
  --output udp://encoder-1080p:5001 \
  --output udp://encoder-720p:5002 \
  --output udp://encoder-480p:5003 \
  --num-paths 2

# Each encoder produces different quality
# encoder-1080p: ffmpeg -i udp://:5001 ... → 1080p output
# encoder-720p:  ffmpeg -i udp://:5002 ... → 720p output
# encoder-480p:  ffmpeg -i udp://:5003 ... → 480p output
```

### Workflow 4: Redundant Recording

```
# Save to multiple drives for safety
./srt-relay \
  --input srt://:9000 \
  --output file:/mnt/drive1/recording.ts \
  --output file:/mnt/drive2/recording.ts \
  --output file:/mnt/drive3/recording.ts \
  --num-paths 3
```

## 🔧 Input Formats

| Format | Syntax | Example |
|--------|--------|---------|
| **SRT** | `srt://:port` | `--input srt://:9000` |
| **UDP** | `udp://:port` | `--input udp://:5000` |
| **File** | `path` | `--input video.ts` |
| **Stdin** | `-` | `--input -` |

## 📤 Output Formats

| Format | Syntax | Example |
|--------|--------|---------|
| **UDP** | `udp://host:port` | `--output udp://server:5000` |
| **File** | `file:path` or `path` | `--output file:/tmp/rec.ts` |
| **Stdout** | `-` | `--output -` |

## 🎬 Production Scenarios

### Scenario 1: Sports Broadcast

```bash
# Field:
camera → srt-sender → 4G + 5G + WiFi → Stadium server

# Stadium server:
./srt-relay \
  --input srt://:9000 \
  --output udp://commentary-booth:5000 \
  --output udp://replay-system:5001 \
  --output udp://broadcast-truck:5002 \
  --output file:/recordings/game-$(date +%H%M%S).ts \
  --num-paths 3
```

### Scenario 2: News Gathering

```bash
# Reporter in field:
camera → srt-sender → Cellular bonding → Newsroom

# Newsroom:
./srt-relay \
  --input srt://:9000 \
  --output udp://control-room:5000 \    # Live monitoring
  --output udp://editing-station:5001 \ # For editing
  --output file:/archive/field-report.ts \  # Archive
  --num-paths 2
```

### Scenario 3: Concert Streaming

```bash
# Mixing console:
audio/video → srt-sender → Multi-path → Server

# Server:
./srt-relay \
  --input srt://:9000 \
  --output udp://youtube-encoder:5000 \
  --output udp://twitch-encoder:5001 \
  --output udp://facebook-encoder:5002 \
  --output udp://local-screens:5003 \
  --output file:/archive/concert.ts \
  --num-paths 4
```

### Scenario 4: Remote Production

```bash
# Camera operators (multiple locations):
cam1 → srt-sender → Multi-path → Production server
cam2 → srt-sender → Multi-path → Production server
cam3 → srt-sender → Multi-path → Production server

# Production server (relay each camera):
./srt-relay --input srt://:9001 \
  --output udp://switcher:5001 \
  --output file:/recordings/cam1.ts --num-paths 2 &

./srt-relay --input srt://:9002 \
  --output udp://switcher:5002 \
  --output file:/recordings/cam2.ts --num-paths 2 &

./srt-relay --input srt://:9003 \
  --output udp://switcher:5003 \
  --output file:/recordings/cam3.ts --num-paths 2 &

# Switcher selects which camera is live
```

## 🔄 Format Conversion Matrix

| Input → Output | Use Case |
|----------------|----------|
| **SRT → UDP** | Distribute bonded stream to UDP receivers |
| **SRT → File** | Record bonded stream |
| **SRT → Stdout** | Pipe to ffmpeg/ffplay |
| **UDP → SRT** | (Use srt-sender instead) |
| **UDP → File** | Record UDP stream |
| **UDP → UDP** | Relay/forward UDP stream |
| **File → UDP** | Replay file to network |
| **File → Multiple UDP** | Distribute file to many receivers |

## 📈 Performance Tips

### 1. UDP Output Buffer Size

For many UDP outputs, increase buffer:
```bash
# Linux
sudo sysctl -w net.core.wmem_max=26214400
```

### 2. File I/O

Use fast storage for file outputs:
```bash
--output file:/mnt/nvme/recording.ts  # SSD/NVMe preferred
```

### 3. Multiple Outputs

Each output adds overhead. Monitor CPU:
```bash
./srt-relay ... --stats 1 --verbose
```

### 4. Network Bandwidth

Calculate total bandwidth:
```bash
# If stream is 5 Mbps and you have 4 UDP outputs:
# Total bandwidth = 5 Mbps × 4 = 20 Mbps
```

## 🐛 Troubleshooting

### No output received

**Check relay is running:**
```bash
./srt-relay --input srt://:9000 --output udp://dest:5000 --verbose
```

**Check destination is listening:**
```bash
# On destination machine
nc -u -l 5000
```

### High CPU usage

**Reduce output count** or **use faster storage** for file outputs.

### UDP packets dropped

**Increase UDP buffer:**
```bash
sudo sysctl -w net.core.wmem_max=26214400
```

**Or reduce number of UDP outputs.**

## 🔗 Integration Examples

### With ffmpeg

```bash
# Receive SRT, transcode, output multiple formats
./srt-relay --input srt://:9000 --output - --num-paths 2 | \
  ffmpeg -i - \
    -c:v libx264 -b:v 2M -f mpegts udp://server1:5000 \
    -c:v libx264 -b:v 1M -f mpegts udp://server2:5000
```

### With OBS

```bash
# Send relay output to OBS via UDP
./srt-relay --input srt://:9000 --output udp://localhost:5000

# In OBS: Media Source → udp://localhost:5000
```

### With VLC

```bash
# Relay to VLC for monitoring
./srt-relay --input srt://:9000 --output udp://localhost:5000

vlc udp://@:5000
```

## 📊 Monitoring

### Real-time Statistics

```bash
./srt-relay \
  --input srt://:9000 \
  --output udp://dest:5000 \
  --stats 1 \
  --verbose
```

Shows:
- Packets relayed
- Data volume
- Throughput (Mbps)
- SRT paths detected (for SRT input)

### Logging

```bash
./srt-relay ... 2>&1 | tee relay.log
```

## 🎯 Common Use Cases Summary

| Use Case | Input | Outputs |
|----------|-------|---------|
| **Distribution** | SRT | Multiple UDP destinations |
| **Recording** | SRT/UDP | File + UDP passthrough |
| **Monitoring** | SRT | File + Stdout (ffplay) |
| **Backup** | Any | Multiple files (redundancy) |
| **Multi-encoder** | SRT | Multiple UDP (different encoders) |
| **Broadcast** | SRT | UDP to multiple sites |
| **Archive + Live** | SRT | File + UDP for live playback |

## 🚀 Production Deployment

### Systemd Service

```ini
[Unit]
Description=SRT Relay
After=network.target

[Service]
Type=simple
User=streamer
WorkingDirectory=/opt/srt
ExecStart=/opt/srt/srt-relay \
  --input srt://:9000 \
  --output udp://monitor:5000 \
  --output udp://encoder:5001 \
  --output file:/mnt/recordings/live-%%Y%%m%%d-%%H%%M%%S.ts \
  --num-paths 3 \
  --stats 5
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## 🔮 Future Enhancements

- [ ] **Multi-input merging** (receive from multiple sources)
- [ ] **HLS output** (write HLS playlist)
- [ ] **RTMP output** (stream to RTMP servers)
- [ ] **Filtering** (only relay certain streams)
- [ ] **Transcoding** (built-in format conversion)

## 💡 Pro Tips

1. **Record + distribute**: Always use `--output file:...` for critical streams
2. **Monitor**: Add `--output -` and pipe to ffplay for visual monitoring
3. **Redundancy**: Use multiple file outputs to different drives
4. **Testing**: Start with `--output -` to verify stream before adding destinations
5. **Stats**: Use `--stats 1 --verbose` during setup, disable in production

---

**The srt-relay is your Swiss Army knife for stream distribution and format conversion!** 🎉
