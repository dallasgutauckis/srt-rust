# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- SRT input support for srt-sender
- RTMP output support for srt-relay
- Enhanced FEC (Forward Error Correction)
- TLS/encryption support
- Performance optimizations

## [0.1.0] - 2026-02-11

### Added
- **Core SRT Protocol**: Complete implementation of SRT protocol in Rust
- **Multi-path Bonding**: Broadcast mode for redundant transmission
  - Automatic duplicate detection via sequence alignment
  - Support for 2+ simultaneous paths
- **CLI Tools**: Three production-ready command-line tools
  - `srt-sender`: Multi-path transmitter with UDP/file/stdin input
  - `srt-receiver`: Multi-path receiver with duplicate detection
  - `srt-relay`: Multi-format relay with multiple simultaneous outputs
- **UDP Input**: Native UDP input support for encoder integration
  - Compatible with ffmpeg, OBS, IP cameras, hardware encoders
  - URL parsing for `udp://` scheme
- **Multi-Output Relay**: Simultaneous output to multiple destinations
  - Support for UDP (multiple), file (multiple), and stdout
  - Format conversion: SRT↔UDP, UDP↔file, etc.
- **ARM Support**: Full support for ARM architecture
  - Tested on Apple Silicon (aarch64-apple-darwin)
  - Compatible with Raspberry Pi, Jetson, AWS Graviton
- **Comprehensive Testing**: 4 integration tests covering multiple scenarios
  - Perfect conditions test (≥99% delivery)
  - Lossy conditions test (≥70% delivery)
  - Multi-path bonding test (path failure resilience)
  - Streaming simulation test (≥80% delivery)
- **Documentation**: Complete guides for all features
  - TOOLS_OVERVIEW.md - Overview of all three tools
  - RELAY_GUIDE.md - Relay tool usage and examples
  - UDP_INPUT_GUIDE.md - UDP integration guide
  - ARM_DEPLOYMENT.md - ARM deployment guide
  - RELEASING.md - CI/CD and release documentation
- **CI/CD**: GitHub Actions workflows
  - Automated testing on Linux, macOS, Windows
  - Multi-platform binary builds
  - Automated releases with semantic versioning

### Fixed
- Sequence number alignment between sender and receiver
- Duplicate packet detection now working correctly
- Packet serialization for multi-path transmission
- Test suite stability on all platforms

### Technical Details
- **Crates**: srt-protocol, srt-bonding, srt-io, srt-cli
- **Dependencies**: socket2, bytes, crossbeam, tracing
- **Binary sizes**: ~1.7 MB per tool (release mode)
- **Platforms**: Linux, macOS (Intel + ARM), Windows

### Known Limitations
- No encryption support yet (planned for 0.2.0)
- No SRT input support for srt-sender (planned for 0.2.0)
- Backup bonding mode not yet implemented
- No built-in transcoding in relay

## [0.0.1] - 2026-02-01

### Added
- Initial project structure
- Basic packet serialization
- Protocol foundation

---

**Legend**:
- **Added**: New features
- **Changed**: Changes to existing functionality
- **Deprecated**: Features to be removed
- **Removed**: Removed features
- **Fixed**: Bug fixes
- **Security**: Security fixes
