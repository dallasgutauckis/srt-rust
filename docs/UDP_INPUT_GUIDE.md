# UDP Input Guide for srt-sender

The `srt-sender` now supports receiving UDP streams as input, making it easy to integrate with encoders and existing streaming infrastructure.

## Quick Start

### Basic UDP Input

```bash
# Listen for UDP on port 5000, send via multi-path SRT
./srt-sender \
  --input udp://:5000 \
  --path 10.0.1.1:9000 \
  --path 10.0.2.1:9000 \
  --group broadcast
```

### With ffmpeg Encoder

```bash
# Terminal 1: Start srt-sender with UDP input
./srt-sender \
  --input udp://:5000 \
  --path 192.168.1.10:9000 \
  --path 192.168.1.11:9000 \
  --group broadcast \
  --stats 2

# Terminal 2: Encode and send to UDP
ffmpeg -f v4l2 -i /dev/video0 \
  -c:v libx264 -preset ultrafast -tune zerolatency \
  -b:v 2M -f mpegts udp://localhost:5000
```

## Input Formats

### UDP Input

```bash
# Listen on all interfaces, port 5000
--input udp://:5000

# Listen on specific interface
--input udp://0.0.0.0:5000

# IPv6
--input udp://[::]:5000
```

### File Input (still supported)

```bash
# Read from file
--input video.ts

# Read from stdin
--input -

# Pipe from command
cat video.ts | ./srt-sender --input - --path ...
```

### Future: SRT Input

```bash
# Coming soon
--input srt://:5000
```

## Complete Examples

### Example 1: OBS → UDP → Multi-Path SRT

**OBS Settings:**
- Output Mode: Advanced
- Type: Custom Output (FFmpeg)
- FFmpeg Output Type: Output to URL
- File path or URL: `udp://localhost:5000`
- Container Format: `mpegts`
- Video Encoder: `libx264`
- Video Bitrate: `3000k`

**srt-sender:**
```bash
./srt-sender \
  --input udp://:5000 \
  --path 10.0.1.1:9000 \    # 4G Modem 1
  --path 10.0.2.1:9000 \    # 4G Modem 2
  --path 192.168.1.100:9000 \  # WiFi
  --group broadcast \
  --stats 1
```

**Receiver:**
```bash
./srt-receiver \
  --listen 9000 \
  --output - \
  --num-paths 3 | ffplay -
```

---

### Example 2: IP Camera → UDP → Multi-Path SRT

Many IP cameras support RTSP/UDP output:

```bash
# Extract stream from IP camera RTSP
ffmpeg -i rtsp://camera.local:554/stream \
  -c copy -f mpegts udp://localhost:5000 &

# Send via multi-path SRT
./srt-sender \
  --input udp://:5000 \
  --path 10.0.1.1:9000 \
  --path 10.0.2.1:9000 \
  --group broadcast
```

---

### Example 3: Multi-Camera Field Production

**Camera 1 (on site):**
```bash
# Camera 1 encoder
ffmpeg -f v4l2 -i /dev/video0 \
  -c:v libx264 -b:v 2M -f mpegts udp://localhost:5001 &

# Camera 1 multi-path sender
./srt-sender \
  --input udp://:5001 \
  --path producer.example.com:9001 \
  --path producer.example.com:9002 \
  --group broadcast
```

**Camera 2 (on site):**
```bash
# Camera 2 encoder
ffmpeg -f v4l2 -i /dev/video1 \
  -c:v libx264 -b:v 2M -f mpegts udp://localhost:5002 &

# Camera 2 multi-path sender
./srt-sender \
  --input udp://:5002 \
  --path producer.example.com:9003 \
  --path producer.example.com:9004 \
  --group broadcast
```

**Production Server (receiver):**
```bash
# Receive camera 1
./srt-receiver --listen 9001-9002 --output camera1.ts &

# Receive camera 2
./srt-receiver --listen 9003-9004 --output camera2.ts &
```

---

### Example 4: Drone Streaming

```bash
# On drone (Raspberry Pi + 4G modems)
# Camera capture
raspivid -o - -t 0 -w 1280 -h 720 -fps 30 | \
ffmpeg -i - -c:v libx264 -preset ultrafast \
  -b:v 1M -f mpegts udp://localhost:5000 &

# Multi-path transmission
./srt-sender \
  --input udp://:5000 \
  --path 10.0.1.1:9000 \    # 4G Modem 1
  --path 10.0.2.1:9000 \    # 4G Modem 2
  --group broadcast
```

---

### Example 5: Satellite Uplink Backup

```bash
# Primary: Satellite uplink
# Backup: 4G bonding via srt-sender

# Encode stream
ffmpeg -i input.mp4 \
  -c:v libx264 -b:v 5M -f mpegts \
  -tee "udp://satellite-uplink:5000|udp://localhost:5001" &

# Backup path via bonded 4G
./srt-sender \
  --input udp://:5001 \
  --path 10.0.1.1:9000 \
  --path 10.0.2.1:9000 \
  --path 10.0.3.1:9000 \
  --group broadcast
```

## Advantages of UDP Input

### 1. **Encoder Compatibility**
- Works with any encoder supporting UDP output
- No need to modify existing encoder pipelines
- Standard MPEGTS over UDP

### 2. **Flexibility**
- Separate encoding from transmission
- Can restart srt-sender without affecting encoder
- Easy to test and debug

### 3. **Performance**
- Minimal latency
- Encoder and sender can run on different cores/machines
- UDP is lightweight

### 4. **Integration**
```
[Encoder] → UDP → [srt-sender] → Multi-path → [srt-receiver] → [Output]
  ↑                     ↑                           ↑
ffmpeg            Your SRT tool            Your SRT tool
OBS                 (bonding)
vMix
Wirecast
```

## Workflow Patterns

### Pattern 1: Local Encoding
```
Same Machine:
  Encoder (ffmpeg) → UDP:5000 → srt-sender → Multi-path → Remote
```

### Pattern 2: Dedicated Encoder
```
Encoder Machine:
  ffmpeg → UDP → Network

Sender Machine:
  UDP:5000 → srt-sender → Multi-path → Remote
```

### Pattern 3: Cloud Workflow
```
On-Premise:
  Camera → Encoder → UDP → srt-sender → Multi-path →

Cloud:
  → srt-receiver → Processing → CDN
```

## Troubleshooting

### No data received

**Check UDP is arriving:**
```bash
# Listen for UDP packets
nc -u -l 5000

# Send test data
echo "test" | nc -u localhost 5000
```

### Firewall blocking UDP

```bash
# macOS
sudo pfctl -d  # Disable firewall temporarily

# Linux
sudo ufw allow 5000/udp
```

### Port already in use

```bash
# Check what's using the port
lsof -i :5000

# Kill process
kill <PID>

# Or use different port
--input udp://:5001
```

### Packet loss visible

UDP is lossy. The multi-path SRT bonding helps, but:

1. **Increase buffer size** in encoder
2. **Use multiple paths** for redundancy
3. **Monitor network quality**
4. **Consider FEC** (future feature)

## Performance Tips

### 1. Tune UDP buffer

```bash
# Increase OS UDP buffer (Linux)
sudo sysctl -w net.core.rmem_max=26214400
sudo sysctl -w net.core.rmem_default=26214400
```

### 2. Encoder settings

```bash
# Lower latency encoding
ffmpeg -i input \
  -preset ultrafast \
  -tune zerolatency \
  -b:v 2M \
  -f mpegts udp://localhost:5000
```

### 3. Monitor stats

```bash
./srt-sender \
  --input udp://:5000 \
  --path ... \
  --stats 1 \
  --verbose  # See packet-level details
```

## Comparison: UDP vs File Input

| Feature | File Input | UDP Input |
|---------|-----------|-----------|
| **Use case** | Testing, replay | Live streaming |
| **Latency** | N/A | Real-time |
| **Encoder integration** | Requires pipe | Direct |
| **Restart** | Easy | Encoder keeps running |
| **Complexity** | Simple | Requires network setup |

## Next Steps

1. ✅ **UDP input implemented**
2. 🚧 **SRT input** (coming soon)
3. 🚧 **RTMP input** (future)
4. 🚧 **Multi-input** (merge multiple UDP sources)

## Production Deployment

```bash
# Systemd service (Linux)
[Unit]
Description=SRT Multi-Path Sender
After=network.target

[Service]
Type=simple
User=streamer
WorkingDirectory=/opt/srt
ExecStart=/opt/srt/srt-sender \
  --input udp://:5000 \
  --path 10.0.1.1:9000 \
  --path 10.0.2.1:9000 \
  --group broadcast \
  --stats 5
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## FAQ

**Q: Can I use multicast UDP?**
A: Not yet, but it's on the roadmap.

**Q: What's the maximum UDP packet size?**
A: 65,536 bytes (UDP max), but MPEGTS typically uses 188*7 = 1316 bytes.

**Q: Does it support RTP/UDP?**
A: Yes, as long as your encoder sends MPEGTS over RTP/UDP, it works.

**Q: Can I receive from multiple UDP sources?**
A: Not yet. Currently one UDP input per srt-sender instance.

**Q: IPv6 support?**
A: Yes! Use `--input udp://[::]:5000`

---

**Your srt-sender is now a powerful bridge between traditional UDP streaming and modern multi-path bonded SRT!** 🚀
