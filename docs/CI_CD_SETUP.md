# CI/CD Setup Complete ✅

GitHub Actions workflows and release automation are now fully configured for SRT Rust.

## What's Been Set Up

### 1. Continuous Integration (`.github/workflows/ci.yml`)

**Triggers**: Push or PR to `master`, `main`, or `develop`

**Tests on**:
- ✅ Linux (ubuntu-latest)
- ✅ macOS (macos-latest)  
- ✅ Windows (windows-latest)

**Runs**:
- ✅ Unit tests (`cargo test --workspace`)
- ✅ Integration tests (4 end-to-end scenarios)
- ✅ Code formatting (`cargo fmt --check`)
- ✅ Linting (`cargo clippy`)
- ✅ Documentation build (`cargo doc`)
- ✅ Code coverage (Tarpaulin → Codecov)
- ✅ Security audit (`cargo audit`)
- ✅ Benchmarks (`cargo bench`)
- ✅ CLI tool builds (srt-sender, srt-receiver, srt-relay)

**Build artifacts**: Binaries uploaded for each platform

---

### 2. Release Automation (`.github/workflows/release.yml`)

**Triggers**: Push a version tag (e.g., `v0.1.0`)

**Builds binaries for**:
- ✅ Linux x86_64 (`x86_64-unknown-linux-gnu`)
- ✅ Linux ARM64 (`aarch64-unknown-linux-gnu`) - Raspberry Pi, AWS Graviton
- ✅ macOS Intel (`x86_64-apple-darwin`)
- ✅ macOS Apple Silicon (`aarch64-apple-darwin`)
- ✅ Windows x86_64 (`x86_64-pc-windows-msvc`)

**Automatically**:
1. Creates GitHub release with changelog
2. Builds release binaries (~1.7 MB each)
3. Packages into archives (tar.gz for Unix, zip for Windows)
4. Uploads to GitHub Releases
5. Optionally publishes to crates.io

---

### 3. Release Helper Script (`scripts/release.sh`)

**Usage**:
```bash
# Patch release: 0.1.0 → 0.1.1
./scripts/release.sh patch

# Minor release: 0.1.0 → 0.2.0
./scripts/release.sh minor

# Major release: 0.1.0 → 1.0.0
./scripts/release.sh major

# Dry run (preview changes)
./scripts/release.sh patch --dry-run
```

**What it does**:
1. ✅ Updates version in all `Cargo.toml` files
2. ✅ Updates `Cargo.lock`
3. ✅ Runs test suite
4. ✅ Creates git commit: `chore: bump version to vX.Y.Z`
5. ✅ Creates git tag: `vX.Y.Z`
6. ✅ Prompts for confirmation
7. ✅ Shows next steps

---

## Quick Start Guide

### Making Your First Release

**Current version**: `0.1.0`

1. **Create release** (choose one):
   ```bash
   # Automated (recommended)
   ./scripts/release.sh patch
   
   # Or manual
   # Edit version in Cargo.toml files
   # git commit -m "chore: bump version to v0.1.1"
   # git tag v0.1.1
   ```

2. **Push to GitHub**:
   ```bash
   git push origin master
   git push origin v0.1.1
   ```

3. **Watch GitHub Actions**:
   - Go to: https://github.com/YOUR_USERNAME/srt-rust/actions
   - Monitor release workflow (takes 5-10 minutes)

4. **Check release**:
   - Go to: https://github.com/YOUR_USERNAME/srt-rust/releases
   - Verify binaries are attached
   - Download and test

---

## What Happens When You Push a Tag

```
git push origin v0.1.1
    ↓
GitHub detects tag
    ↓
Release workflow starts
    ↓
╔═══════════════════════════════════════╗
║  1. Create GitHub Release             ║
║     - Extract changelog               ║
║     - Generate release notes          ║
╚═══════════════════════════════════════╝
    ↓
╔═══════════════════════════════════════╗
║  2. Build Binaries (parallel)         ║
║     ├─ Linux x86_64                   ║
║     ├─ Linux ARM64                    ║
║     ├─ macOS Intel                    ║
║     ├─ macOS Apple Silicon            ║
║     └─ Windows x86_64                 ║
╚═══════════════════════════════════════╝
    ↓
╔═══════════════════════════════════════╗
║  3. Upload Release Assets             ║
║     - srt-rust-v0.1.1-<target>.tar.gz ║
║     - Contains all 3 CLI tools        ║
╚═══════════════════════════════════════╝
    ↓
╔═══════════════════════════════════════╗
║  4. Publish to crates.io (optional)   ║
║     - srt-protocol                    ║
║     - srt-io                          ║
║     - srt-bonding                     ║
║     - srt                             ║
╚═══════════════════════════════════════╝
    ↓
✅ Release v0.1.1 complete!
```

---

## File Structure

```
srt-rust/
├── .github/
│   └── workflows/
│       ├── ci.yml              # ✅ Continuous Integration
│       └── release.yml         # ✅ Release Automation
├── scripts/
│   └── release.sh              # ✅ Release Helper Script
├── CHANGELOG.md                # ✅ Version History
├── RELEASING.md                # ✅ Full Documentation
└── CI_CD_SETUP.md             # ✅ This File
```

---

## Testing the CI/CD Setup

### Test CI Workflow

```bash
# Make a small change
echo "# Test" >> README.md
git add README.md
git commit -m "test: CI workflow"
git push origin master

# Check: https://github.com/YOUR_USERNAME/srt-rust/actions
# Should see CI workflow running
```

### Test Release Workflow (Dry Run)

```bash
# Preview what release script will do
./scripts/release.sh patch --dry-run

# Output:
# Current version: 0.1.0
# New version: 0.1.1
# Dry run - no changes will be made
```

### Test Release Workflow (Real)

```bash
# Create a test release
./scripts/release.sh patch
# Review changes: git show
git push origin master
git push origin v0.1.1

# Monitor: https://github.com/YOUR_USERNAME/srt-rust/actions
# Check release: https://github.com/YOUR_USERNAME/srt-rust/releases
```

---

## Release Asset Format

Each release includes 5 archives:

```
srt-rust-v0.1.1-x86_64-unknown-linux-gnu.tar.gz      # 1.7 MB
srt-rust-v0.1.1-aarch64-unknown-linux-gnu.tar.gz     # 1.7 MB
srt-rust-v0.1.1-x86_64-apple-darwin.tar.gz           # 1.7 MB
srt-rust-v0.1.1-aarch64-apple-darwin.tar.gz          # 1.7 MB
srt-rust-v0.1.1-x86_64-pc-windows-msvc.zip           # 1.7 MB
```

Each archive contains:
```
srt-sender      # or .exe on Windows
srt-receiver    # or .exe on Windows
srt-relay       # or .exe on Windows
```

---

## CI Status Badges

Add these to your README.md:

```markdown
[![CI](https://github.com/YOUR_USERNAME/srt-rust/workflows/CI/badge.svg)](https://github.com/YOUR_USERNAME/srt-rust/actions)
[![Release](https://github.com/YOUR_USERNAME/srt-rust/workflows/Release/badge.svg)](https://github.com/YOUR_USERNAME/srt-rust/actions)
[![Latest Release](https://img.shields.io/github/v/release/YOUR_USERNAME/srt-rust)](https://github.com/YOUR_USERNAME/srt-rust/releases)
```

---

## Publishing to crates.io (Optional)

If you want to publish to crates.io:

1. **Get token**: https://crates.io/settings/tokens
2. **Add to GitHub Secrets**:
   - Go to: https://github.com/YOUR_USERNAME/srt-rust/settings/secrets/actions
   - Add: `CARGO_REGISTRY_TOKEN`
3. **Push tag**: The release workflow will automatically publish

---

## Versioning Strategy

We follow [Semantic Versioning](https://semver.org/):

| Version | Type | Example |
|---------|------|---------|
| **MAJOR** | Breaking changes | 1.0.0 → 2.0.0 |
| **MINOR** | New features | 0.1.0 → 0.2.0 |
| **PATCH** | Bug fixes | 0.1.0 → 0.1.1 |

**Current**: `0.1.0` (initial production release)

**Planned**:
- `0.1.1` - Bug fixes
- `0.2.0` - SRT input support
- `0.3.0` - Encryption support
- `1.0.0` - Stable API, production ready

---

## Monitoring

**CI/CD Dashboard**: https://github.com/YOUR_USERNAME/srt-rust/actions

**Recent Releases**: https://github.com/YOUR_USERNAME/srt-rust/releases

**Code Coverage**: https://codecov.io/gh/YOUR_USERNAME/srt-rust (if enabled)

---

## Troubleshooting

### CI workflow fails

**Check locally**:
```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
./tests/run-all-tests.sh
```

### Release workflow fails

**Common issues**:
- Tag format wrong (use `v0.1.0`, not `0.1.0`)
- Tests failing
- Cross-compilation issues

**Debug**:
- Check GitHub Actions logs
- Run local build: `cargo build --release --bins`

### Script won't run

```bash
chmod +x scripts/release.sh
```

---

## Next Steps

1. ✅ **Test CI**: Make a commit, watch it run
2. ✅ **Test Release Script**: Run `./scripts/release.sh patch --dry-run`
3. ✅ **Update README**: Add CI badges
4. ✅ **Make Release**: When ready, push a tag
5. ✅ **Verify**: Download and test release binaries

---

## Documentation

- **Full Guide**: `RELEASING.md`
- **Changelog**: `CHANGELOG.md`
- **This Guide**: `CI_CD_SETUP.md`

---

**Your CI/CD pipeline is ready! 🚀**

Every push is tested. Every tag creates a release. All platforms covered.
