# Development Guide

## Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

### Installing Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Verify installation:

```bash
rustc --version
cargo --version
```

## Building

### Build all crates

```bash
cd srt-rust
cargo build --workspace
```

### Build with optimizations

```bash
cargo build --release --workspace
```

### Build specific crate

```bash
cargo build --package srt-protocol
```

## Testing

### Run all tests

```bash
cargo test --workspace
```

### Run tests for specific crate

```bash
cargo test --package srt-protocol
```

### Run property-based tests with more cases

```bash
PROPTEST_CASES=100000 cargo test --package srt-tests
```

### Run tests with output

```bash
cargo test --workspace -- --nocapture
```

## Linting and Formatting

### Format code

```bash
cargo fmt --all
```

### Check formatting without making changes

```bash
cargo fmt --all -- --check
```

### Run clippy lints

```bash
cargo clippy --workspace --all-targets
```

### Run clippy with warnings as errors

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

## Benchmarking

### Run all benchmarks

```bash
cargo bench --workspace
```

### Run benchmarks for specific crate

```bash
cargo bench --package srt-protocol
```

### Generate benchmark report

```bash
cargo bench --package srt-protocol
# Open target/criterion/report/index.html
```

## Documentation

### Build documentation

```bash
cargo doc --workspace --no-deps
```

### Build and open documentation

```bash
cargo doc --workspace --no-deps --open
```

### Check documentation links

```bash
cargo doc --workspace --no-deps
# Check for broken links
```

## Code Coverage

### Install tarpaulin

```bash
cargo install cargo-tarpaulin
```

### Generate coverage report

```bash
cargo tarpaulin --workspace --out Html --timeout 300
# Open tarpaulin-report.html
```

## Profiling

### Install flamegraph

```bash
cargo install flamegraph
```

### Generate flamegraph

```bash
cargo flamegraph --bench packet_bench -- --bench
```

## Running CLI Tools (When Implemented)

### Sender

```bash
cargo run --bin srt-sender -- --help
```

### Receiver

```bash
cargo run --bin srt-receiver -- --help
```

### Relay

```bash
cargo run --bin srt-relay -- --help
```

## Development Workflow

### 1. Create a new feature

```bash
git checkout -b feature/my-feature
```

### 2. Make changes and test

```bash
# Edit files
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all
```

### 3. Run benchmarks if performance-critical

```bash
cargo bench --package srt-protocol
```

### 4. Update documentation

```bash
cargo doc --workspace --no-deps --open
```

### 5. Commit and push

```bash
git add .
git commit -m "Add feature X"
git push origin feature/my-feature
```

## Debugging

### Enable debug logging

```bash
RUST_LOG=debug cargo test --package srt-protocol -- --nocapture
```

### Run with backtrace

```bash
RUST_BACKTRACE=1 cargo test
```

### Use rust-gdb or rust-lldb

```bash
rust-gdb target/debug/srt-sender
```

## Common Tasks

### Add a new dependency

Edit the appropriate `Cargo.toml`:

```toml
[dependencies]
new-crate = "1.0"
```

Then run:

```bash
cargo build
```

### Update dependencies

```bash
cargo update
```

### Check for outdated dependencies

```bash
cargo install cargo-outdated
cargo outdated
```

### Find unused dependencies

```bash
cargo install cargo-udeps
cargo +nightly udeps
```

## Performance Tips

### Release builds are much faster

Always use `--release` for performance testing:

```bash
cargo build --release
cargo bench
```

### Profile-guided optimization

For maximum performance, consider PGO:

```bash
# Generate profile data
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release

# Run typical workload
./target/release/srt-sender ...

# Build with profile data
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" cargo build --release
```

## Troubleshooting

### Clean build artifacts

```bash
cargo clean
```

### Check project structure

```bash
cargo tree
```

### Verify workspace members

```bash
cargo metadata --format-version=1 | jq '.workspace_members'
```

## IDE Setup

### VS Code

Install recommended extensions:
- rust-analyzer
- CodeLLDB (for debugging)
- crates (for dependency management)

### IntelliJ IDEA / CLion

Install the Rust plugin from JetBrains.

### Vim/Neovim

Use rust-analyzer with your LSP client of choice.

## Resources

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [The Cargo Book](https://doc.rust-lang.org/cargo/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
