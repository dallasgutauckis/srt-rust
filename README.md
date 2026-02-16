# SRT-Rust: Secure Reliable Transport Protocol in Rust

A complete rewrite of the SRT (Secure Reliable Transport) protocol library from C/C++ to Rust, with emphasis on **bonded multi-path streaming** for heterogeneous networks.

## Project Status

🚧 **In Active Development** - Phase 1: Foundation (Months 1-2)

### Current Progress
- [x] Cargo workspace structure
- [ ] Packet structures and serialization
- [ ] Sequence number handling
- [x] CI/CD pipeline
- [x] Unit tests for packet serialization

## Architecture

This project is organized as a Cargo workspace with the following crates:

- **srt-protocol**: Core protocol implementation (packets, handshake, connection state machine)
- **srt-bonding**: Multi-path connection bonding (broadcast, backup, load balancing)
- **srt-crypto**: Encryption with pluggable backends
- **srt-io**: Network I/O and platform abstractions
- **srt**: High-level public API
- **srt-cli**: Command-line tools (sender, receiver, relay)
- **srt-tests**: Integration tests

## Why Rust?

- **Memory safety**: Prevents buffer overflow vulnerabilities in network code
- **Fearless concurrency**: Safe multi-threaded packet processing
- **Zero-cost abstractions**: Performance without sacrificing maintainability
- **Strong type system**: Catches protocol state machine bugs at compile time
- **Modern tooling**: Cargo, built-in testing, excellent ecosystem

## Use Case

This implementation focuses on:
- **One-to-many broadcast**: Single stream → multiple transmission paths (cellular, WiFi, etc.)
- **Many-to-one bonding**: Multiple receivers → single combined stream
- **Quality maximization**: Packet alignment across varying bandwidth connections
- **Production CLI tools**: Real-world deployment with heterogeneous networks

## Building

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Build release version
cargo build --release --workspace

# Run specific binary (once implemented)
cargo run --bin srt-sender -- --help
```

## Development

```bash
# Lint
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --workspace

# Run benchmarks
cargo bench --package srt-protocol

# Generate documentation
cargo doc --workspace --no-deps --open
```

## Timeline

- **Phase 1** (Months 1-2): Foundation - Packet structures, sequence numbers
- **Phase 2** (Months 2-4): Core Protocol - Single connection send/receive
- **Phase 3** (Months 4-7): Connection Bonding - Multi-path streaming (CRITICAL)
- **Phase 4** (Months 7-8): Forward Error Correction
- **Phase 5** (Months 8-9): CLI Applications
- **Phase 6** (Months 9-10): Performance & Optimization
- **Phase 7** (Months 10-11): Encryption
- **Phase 8** (Months 11-12): Testing & Stabilization

## License

MIT OR Apache-2.0

## References

Based on the SRT protocol specification and the reference C/C++ implementation:
- [SRT GitHub Repository](https://github.com/Haivision/srt)
- [SRT Protocol Specification](https://datatracker.ietf.org/doc/html/draft-sharabayko-srt)
