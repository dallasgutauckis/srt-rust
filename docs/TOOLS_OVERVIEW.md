# SRT CLI Tools - Complete Overview

Three powerful tools for multi-path bonded streaming with format conversion.

## 🛠️ The Three Tools

### 1. `srt-sender` - Multi-Path Transmitter

**Purpose**: Send streams via multi-path bonded SRT

**Input**: File, stdin, **UDP**
**Output**: Multi-path SRT

```bash
./srt-sender \
  --input udp://:5000 \
  --path 10.0.1.1:9000 \
  --path 10.0.2.1:9000 \
  --path 192.168.1.100:9000 \
  --group broadcast
```

**Use cases**:
- Field camera → Multi-path transmission
- Encoder → Bonded cellular/WiFi
- Live event → Resilient streaming

---

### 2. `srt-receiver` - Multi-Path Receiver

**Purpose**: Receive bonded multi-path SRT streams

**Input**: Multi-path SRT
**Output**: File, stdout

```bash
./srt-receiver \
  --listen 9000 \
  --output output.ts \
  --num-paths 3
```

**Use cases**:
- Receive bonded stream
- Duplicate detection
- Single clean output

---

### 3. `srt-relay` - Multi-Format Relay ⭐ **NEW**

**Purpose**: Receive in one format, output to **multiple destinations** in various formats

**Input**: **SRT**, **UDP**, file, stdin
**Output**: **UDP** (multiple), **file** (multiple), stdout

```bash
./srt-relay \
  --input srt://:9000 \
  --output udp://server1:5000 \
  --output udp://server2:5000 \
  --output file:/tmp/recording.ts \
  --output - \
  --num-paths 2
```

**Use cases**:
- SRT → Multiple UDP destinations
- Recording + live distribution
- One-to-many broadcasting
- Format conversion

## 🔄 How They Work Together

### Complete Workflow

```
┌─────────┐  UDP   ┌────────────┐  Multi-SRT  ┌─────────────┐  SRT   ┌───────────┐
│ Encoder │───────→│srt-sender  │════════════→│srt-receiver │───────→│srt-relay  │
│ (ffmpeg)│  :5000 │ (bonding)  │ 3x paths    │  (bonding)  │  :9000 │ (distrib) │
└─────────┘        └────────────┘             └─────────────┘        └───────────┘
                          ║                                                 ║
                          ║ 4G Modem 1                                      ║
                          ║ 4G Modem 2                           ┌──────────┼──────────┐
                          ║ WiFi                                 ▼          ▼          ▼
                                                            UDP:5000   UDP:5001  File+Monitor
```

## 📋 Quick Reference

| Tool | Input Formats | Output Formats | Multi-Output |
|------|---------------|----------------|--------------|
| **srt-sender** | File, stdin, UDP | Multi-path SRT | No (multi-path) |
| **srt-receiver** | Multi-path SRT | File, stdout | No |
| **srt-relay** | SRT, UDP, file, stdin | UDP, file, stdout | ✅ Yes |

## 🎯 Common Scenarios

### Scenario 1: Field Camera → Production Distribution

```bash
# FIELD (Camera location)
ffmpeg -i camera → UDP:5000

./srt-sender \
  --input udp://:5000 \
  --path production-server:9000 \  # Via cellular
  --path production-server:9001 \  # Via WiFi
  --group broadcast

# PRODUCTION SERVER
./srt-relay \
  --input srt://:9000 \
  --output udp://monitor1:5000 \
  --output udp://monitor2:5000 \
  --output udp://encoder:5000 \
  --output file:/recordings/live.ts \
  --num-paths 2
```

### Scenario 2: Simple Point-to-Point

```bash
# SENDER
./srt-sender --input video.ts --path receiver:9000 --path receiver:9001

# RECEIVER
./srt-receiver --listen 9000 --output received.ts --num-paths 2
```

### Scenario 3: UDP Relay/Forwarding

```bash
# Receive UDP, forward to multiple UDP destinations
./srt-relay \
  --input udp://:5000 \
  --output udp://dest1:6000 \
  --output udp://dest2:6000 \
  --output udp://dest3:6000
```

### Scenario 4: SRT to UDP Conversion

```bash
# Receive multi-path SRT, output single UDP stream
./srt-relay \
  --input srt://:9000 \
  --output udp://destination:5000 \
  --num-paths 3
```

### Scenario 5: Record + Monitor

```bash
# Save to file while monitoring in ffplay
./srt-relay \
  --input srt://:9000 \
  --output file:/tmp/recording.ts \
  --output - \
  --num-paths 2 | ffplay -
```

## 🎬 Full Production Example

### Multi-Camera Live Event

**Field Camera 1:**
```bash
ffmpeg -i /dev/video0 -c:v libx264 -f mpegts udp://localhost:5001 &

./srt-sender --input udp://:5001 \
  --path server:9001 --path server:9002 \
  --group broadcast
```

**Field Camera 2:**
```bash
ffmpeg -i /dev/video1 -c:v libx264 -f mpegts udp://localhost:5002 &

./srt-sender --input udp://:5002 \
  --path server:9003 --path server:9004 \
  --group broadcast
```

**Production Server (Camera 1):**
```bash
./srt-relay --input srt://:9001 \
  --output udp://switcher:6001 \
  --output file:/recordings/cam1.ts \
  --num-paths 2 &
```

**Production Server (Camera 2):**
```bash
./srt-relay --input srt://:9003 \
  --output udp://switcher:6002 \
  --output file:/recordings/cam2.ts \
  --num-paths 2 &
```

**Video Switcher:**
```bash
# Receives both cameras on UDP
# Selects which camera is live
# Outputs final program
```

## 🔧 Tool Selection Guide

**Use `srt-sender` when:**
- You need multi-path bonding (cellular, WiFi, etc.)
- You have unreliable network paths
- You want redundancy/resilience
- Input is file, stdin, or UDP

**Use `srt-receiver` when:**
- You're receiving a bonded SRT stream
- You need duplicate detection
- You want a single clean output
- Output is file or stdout

**Use `srt-relay` when:**
- You need to convert formats
- You want multiple outputs from one input
- You're distributing to many destinations
- You need recording + live output
- You want one-to-many broadcasting

## 🚀 Workflows

### Workflow 1: Simple Bonding
```
Sender → Multi-path SRT → Receiver
```
Tools: `srt-sender` + `srt-receiver`

### Workflow 2: Bonding + Distribution
```
Encoder → srt-sender → Multi-path → srt-relay → Multiple destinations
```
Tools: `srt-sender` + `srt-relay`

### Workflow 3: UDP Distribution
```
UDP source → srt-relay → Multiple UDP destinations
```
Tools: `srt-relay` only

### Workflow 4: Full Pipeline
```
Camera → ffmpeg → srt-sender → Multi-path → srt-receiver → srt-relay → Dist
```
Tools: All three

## 📊 Feature Matrix

| Feature | srt-sender | srt-receiver | srt-relay |
|---------|-----------|--------------|-----------|
| Multi-path bonding | ✅ Output | ✅ Input | ✅ Input |
| UDP input | ✅ | ❌ | ✅ |
| UDP output | ❌ | ❌ | ✅ |
| SRT input | ❌ | ✅ | ✅ |
| SRT output | ✅ | ❌ | ❌ |
| File input | ✅ | ❌ | ✅ |
| File output | ❌ | ✅ | ✅ |
| Multiple outputs | ❌ | ❌ | ✅ |
| Broadcast bonding | ✅ | ✅ | ✅ |
| Backup bonding | ✅ | ✅ | ⏳ |
| ARM support | ✅ | ✅ | ✅ |

## 🎓 Learning Path

1. **Start with**: `srt-sender` + `srt-receiver` for basic bonding
2. **Add**: UDP input to `srt-sender` for encoder integration
3. **Advanced**: Use `srt-relay` for distribution and format conversion
4. **Production**: Combine all three for complete workflows

## 📚 Documentation

- **srt-sender**: See `CLI_GUIDE.md` and `UDP_INPUT_GUIDE.md`
- **srt-receiver**: See `CLI_GUIDE.md`
- **srt-relay**: See `RELAY_GUIDE.md`
- **Testing**: See `tests/README.md`
- **ARM deployment**: See `ARM_DEPLOYMENT.md`

## ✅ All Tools Ready

✅ **srt-sender** - Production ready
✅ **srt-receiver** - Production ready
✅ **srt-relay** - Production ready

✅ **UDP input** - Implemented
✅ **Multi-output** - Implemented
✅ **ARM support** - Tested on Apple Silicon
✅ **Test suite** - 4/4 tests passing

---

**You now have a complete toolkit for resilient, multi-path streaming with format conversion!** 🎉
