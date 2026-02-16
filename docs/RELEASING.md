# Release Process

This document describes the CI/CD setup and release process for SRT Rust.

## Overview

The project uses GitHub Actions for:
- **Continuous Integration (CI)**: Run tests on every push/PR
- **Release Automation**: Build binaries and publish releases
- **Code Quality**: Linting, formatting, coverage, security audits

## CI Workflow

**Trigger**: Push or PR to `master`, `main`, or `develop` branches

**Jobs**:
1. **Test Suite** - Run tests on Linux, macOS, Windows with stable and beta Rust
2. **Clippy** - Rust linter for common mistakes
3. **Rustfmt** - Code formatting check
4. **Benchmarks** - Performance benchmarks
5. **Documentation** - Build documentation
6. **Coverage** - Code coverage with Tarpaulin
7. **CLI Tools** - Build and test all three CLI tools
8. **Integration Tests** - Run end-to-end test suite

**File**: `.github/workflows/ci.yml`

## Release Workflow

**Trigger**: Push a version tag (e.g., `v0.1.0`)

**Process**:
1. Create GitHub release with changelog
2. Build binaries for all platforms:
   - Linux x86_64
   - Linux ARM64 (Raspberry Pi, AWS Graviton)
   - macOS Intel
   - macOS Apple Silicon
   - Windows x86_64
3. Upload binaries as release assets
4. Optionally publish to crates.io

**File**: `.github/workflows/release.yml`

## Making a Release

### Automated Method (Recommended)

Use the release script:

```bash
# Patch release (0.1.0 → 0.1.1)
./scripts/release.sh patch

# Minor release (0.1.1 → 0.2.0)
./scripts/release.sh minor

# Major release (0.2.0 → 1.0.0)
./scripts/release.sh major

# Dry run (no changes)
./scripts/release.sh patch --dry-run
```

The script will:
1. Update version in all `Cargo.toml` files
2. Update `Cargo.lock`
3. Run tests
4. Create a git commit: `chore: bump version to vX.Y.Z`
5. Create a git tag: `vX.Y.Z`

Then push:
```bash
git push origin master
git push origin vX.Y.Z
```

### Manual Method

1. **Update versions** in all `Cargo.toml` files:
   ```bash
   # Update version = "0.1.0" to version = "0.1.1"
   find . -name "Cargo.toml" -exec sed -i 's/version = "0.1.0"/version = "0.1.1"/' {} \;
   ```

2. **Update Cargo.lock**:
   ```bash
   cargo update --workspace
   ```

3. **Run tests**:
   ```bash
   cargo test --workspace
   ./tests/run-all-tests.sh
   ```

4. **Update CHANGELOG.md** (optional but recommended):
   ```markdown
   ## [0.1.1] - 2026-02-11
   
   ### Added
   - New feature X
   
   ### Fixed
   - Bug Y
   
   ### Changed
   - Improved Z
   ```

5. **Commit and tag**:
   ```bash
   git add .
   git commit -m "chore: bump version to v0.1.1"
   git tag -a v0.1.1 -m "Release v0.1.1"
   ```

6. **Push**:
   ```bash
   git push origin master
   git push origin v0.1.1
   ```

## After Pushing the Tag

GitHub Actions will automatically:

1. ✅ Create a GitHub release
2. ✅ Build binaries for all platforms (5-10 minutes)
3. ✅ Upload binaries as release assets
4. ✅ Generate release notes from commits

**Check progress**: https://github.com/YOUR_USERNAME/srt-rust/actions

## Release Assets

Each release includes pre-built binaries:

```
srt-rust-v0.1.0-x86_64-unknown-linux-gnu.tar.gz      # Linux x86_64
srt-rust-v0.1.0-aarch64-unknown-linux-gnu.tar.gz     # Linux ARM64
srt-rust-v0.1.0-x86_64-apple-darwin.tar.gz           # macOS Intel
srt-rust-v0.1.0-aarch64-apple-darwin.tar.gz          # macOS Apple Silicon
srt-rust-v0.1.0-x86_64-pc-windows-msvc.zip           # Windows
```

Each archive contains:
- `srt-sender`
- `srt-receiver`
- `srt-relay`

## Publishing to crates.io (Optional)

To publish crates to crates.io:

1. **Set up token**:
   ```bash
   # Get token from https://crates.io/settings/tokens
   # Add to GitHub secrets as CARGO_REGISTRY_TOKEN
   ```

2. **Enable in release workflow**:
   - The `publish-crates` job will automatically run on tag push
   - It publishes crates in dependency order

3. **Manual publish**:
   ```bash
   cargo publish --package srt-protocol
   cargo publish --package srt-io
   cargo publish --package srt-bonding
   cargo publish --package srt
   ```

## Versioning Strategy

We follow [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** (X.0.0): Breaking API changes
- **MINOR** (0.X.0): New features, backward compatible
- **PATCH** (0.0.X): Bug fixes, backward compatible

### Pre-1.0 Releases

Before 1.0.0, we use:
- **0.X.0** for breaking changes or major features
- **0.X.Y** for bug fixes and minor features

### Examples

```
0.1.0 → 0.1.1   # Bug fix
0.1.1 → 0.2.0   # New feature (UDP input)
0.2.0 → 0.3.0   # Another feature (relay tool)
0.3.0 → 1.0.0   # Stable API, production ready
```

## Changelog Format

Keep `CHANGELOG.md` updated with each release:

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Feature in progress

## [0.1.0] - 2026-02-11

### Added
- Multi-path bonded SRT transmission
- UDP input support
- Multi-format relay tool
- Comprehensive test suite
- ARM support

### Fixed
- Sequence alignment issues
- Duplicate packet detection

## [0.0.1] - 2026-02-01

### Added
- Initial release
- Basic SRT protocol implementation
```

## Troubleshooting

### Release workflow fails

**Check**:
1. All tests pass locally: `cargo test --workspace`
2. All platforms build: `cargo build --target <target> --release`
3. Tag format is correct: `v0.1.0` (not `0.1.0` or `version-0.1.0`)

### Binary sizes too large

**Current sizes**: ~1.7 MB per binary (release mode)

**To reduce**:
```toml
# In Cargo.toml
[profile.release]
opt-level = 'z'     # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit
strip = true        # Strip symbols
```

### Cross-compilation issues

**Linux ARM**: Requires `gcc-aarch64-linux-gnu` package

**macOS cross-compile**: Build on macOS with multiple targets:
```bash
rustup target add aarch64-apple-darwin
cargo build --target aarch64-apple-darwin --release
```

## CI/CD Best Practices

1. **Always run tests locally** before pushing
2. **Use feature branches** for development
3. **Squash commits** before merging to main
4. **Write meaningful commit messages**
5. **Update CHANGELOG.md** with each PR
6. **Test releases** in a staging environment first
7. **Monitor GitHub Actions** after pushing tags

## GitHub Secrets

Required secrets for full automation:

| Secret | Purpose | Required |
|--------|---------|----------|
| `GITHUB_TOKEN` | Auto-created | ✅ Yes |
| `CARGO_REGISTRY_TOKEN` | Publish to crates.io | ❌ Optional |
| `CODECOV_TOKEN` | Code coverage | ❌ Optional |

Set secrets at: `https://github.com/YOUR_USERNAME/srt-rust/settings/secrets/actions`

## Monitoring Releases

**View releases**: https://github.com/YOUR_USERNAME/srt-rust/releases

**View actions**: https://github.com/YOUR_USERNAME/srt-rust/actions

**View builds**: Click on any workflow run to see logs

## Release Checklist

Before releasing:

- [ ] All tests pass: `cargo test --workspace`
- [ ] Integration tests pass: `./tests/run-all-tests.sh`
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version bumped in all Cargo.toml files
- [ ] Commit message follows convention
- [ ] Git tag created with `v` prefix

After releasing:

- [ ] GitHub release created successfully
- [ ] All platform binaries uploaded
- [ ] Release notes accurate
- [ ] Download and test binaries
- [ ] Announce release (optional)

---

**Questions?** Open an issue or check the GitHub Actions logs for details.
