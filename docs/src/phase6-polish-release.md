# Phase 6: Polish & Release

This phase focuses on preparing uterx for production release with comprehensive tooling, documentation, and quality assurance.

## Overview

**Status**: ⏳ Planned

**Goal**: Production-ready release with CI/CD, benchmarks, documentation, and crash reporting.

## Tasks

### 1. Cross-platform CI (GitHub Actions)

Set up continuous integration for both Windows and Linux platforms.

#### GitHub Actions Workflow

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
        rust: [stable]

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          profile: minimal
          toolchain: ${{ matrix.rust }}
          components: rustfmt, clippy

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-cargo-build-target-${{ hashFiles('**/Cargo.lock') }}

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Run tests
        run: cargo test --all-features

      - name: Build release
        run: cargo build --release

  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          profile: minimal
          toolchain: stable

      - name: Run benchmarks
        run: cargo bench --all
```

#### CI Requirements

- ✅ Formatting check with `cargo fmt`
- ✅ Linting with `cargo clippy`
- ✅ Unit tests across all crates
- ✅ Integration tests
- ✅ Release build verification
- ✅ Benchmark execution
- ✅ Documentation build verification

### 2. Benchmarks with Criterion

Implement comprehensive benchmarks to measure and track performance.

#### Benchmark Categories

**Terminal Emulation**
- VTE parsing throughput
- Cell grid operations
- Scrollback buffer operations

**Rendering**
- Frame geometry generation
- Damage tracking overhead
- Glyph atlas lookups
- Ligature rendering

**Multiplexer**
- Pane split/merge operations
- Layout computation
- Session save/restore

**Plugin System**
- Plugin load/start time
- Host API call overhead
- Permission check performance

#### Example Benchmark

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use uterx_core::Grid;

fn bench_grid_write_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_write_char");
    for size in [80x24, 120x40, 200x60].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut grid = Grid::new(size.width, size.height);
            b.iter(|| {
                for row in 0..size.height {
                    for col in 0..size.width {
                        grid.write_char(col, row, black_box('X'));
                    }
                }
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_grid_write_char);
criterion_main!(benches);
```

#### Benchmark Goals

- VTE parsing: > 10 MB/s
- Frame generation: < 1ms per frame
- Plugin load: < 100ms
- Session save: < 50ms

### 3. mdBook Documentation

Comprehensive documentation for users and developers.

#### Documentation Structure

```
docs/
├── book.toml              # mdBook configuration
├── SUMMARY.md             # Table of contents
└── src/
    ├── introduction.md    # Project overview
    ├── getting-started.md # Installation and usage
    ├── architecture.md    # Architecture deep dive
    ├── phases.md          # Development phases overview
    ├── phase1-core-terminal.md
    ├── phase2-multiplexer.md
    ├── phase3-gpu-rendering.md
    ├── phase4-plugin-system.md
    ├── phase5-example-plugins.md
    ├── phase6-polish-release.md  # This file
    ├── phase7-innovative-features.md
    ├── api-reference.md   # API documentation
    ├── configuration.md   # Configuration reference
    ├── plugins.md         # Plugin development guide
    ├── contributing.md    # Contribution guidelines
    └── license.md         # License information
```

#### Documentation Goals

- User-facing documentation for installation and usage
- Developer documentation for architecture and API
- Plugin development guide
- Contribution guidelines
- Inline code documentation with rustdoc

### 4. Crash Reporting

Implement crash reporting to collect and analyze errors.

#### Crash Reporting Strategy

**Local Crash Logs**
- Log crashes to `~/.uterx/crashes/`
- Include stack trace, system info, and configuration
- Timestamped crash reports

**Error Handling**
- Panic hooks for unhandled panics
- Error logging with backtraces
- Graceful degradation where possible

#### Example Panic Hook

```rust
use std::panic;
use std::path::PathBuf;
use std::fs;
use std::io::Write;

fn setup_panic_handler() {
    panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        let crash_dir = dirs::home_dir()
            .unwrap()
            .join(".uterx")
            .join("crashes");

        fs::create_dir_all(&crash_dir).ok();

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let crash_file = crash_dir.join(format!("crash_{}.log", timestamp));

        let mut file = fs::File::create(crash_file).ok();

        if let Some(ref mut f) = file {
            writeln!(f, "uterx Crash Report").ok();
            writeln!(f, "=================").ok();
            writeln!(f, "Time: {}", timestamp).ok();
            writeln!(f, "Version: {}", env!("CARGO_PKG_VERSION")).ok();
            writeln!(f, "OS: {}", std::env::consts::OS).ok();
            writeln!(f, "Arch: {}", std::env::consts::ARCH).ok();
            writeln!(f).ok();
            writeln!(f, "Panic: {}", panic_info).ok();
            writeln!(f).ok();
            writeln!(f, "Backtrace:\n{:?}", backtrace).ok();
        }

        eprintln!("uterx crashed! Crash report saved to: {:?}", file);
    }));
}
```

### 5. Open-source Release

Prepare for public release.

#### Release Checklist

**Code Quality**
- [ ] All clippy warnings resolved
- [ ] Code formatted consistently
- [ ] Documentation complete
- [ ] Tests passing (90%+ coverage)

**Project Files**
- [ ] LICENSE file (choose appropriate license)
- [ ] CONTRIBUTING.md
- [ ] CHANGELOG.md
- [ ] SECURITY.md
- [ ] README.md updated

**Release Assets**
- [ ] Windows binary (x64)
- [ ] Linux binary (x64)
- [ ] Source tarball
- [ ] Release notes

**Infrastructure**
- [ ] GitHub repository public
- [ ] CI/CD configured
- [ ] Issue templates
- [ ] Pull request template
- [ ] Discussion forums enabled

#### Release Notes Template

```markdown
# uterx v0.1.0

## What's New

This is the first stable release of uterx, a "Terminal Desktop" application combining the performance of Ghostty with the multiplexing capabilities of Zellij.

### Features

- **Terminal Emulation**: Full VTE/ANSI parsing with Unicode 17 support
- **GPU Rendering**: wgpu-based rendering at 60fps
- **Multiplexer**: Panes, tabs, sessions with drag-and-drop
- **Plugin System**: WASM-based plugins with sandboxed execution
- **Built-in Tools**: File browser, text editor, AI assistant

### Installation

```bash
cargo install uterx
```

### Documentation

Full documentation available at: https://docs.uterx.dev

### Known Issues

[List any known issues]

### Contributors

[List contributors]
```

## Quality Metrics

### Code Coverage

Target: 90%+ code coverage across all crates.

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --workspace
```

### Performance Benchmarks

Track performance metrics over time:

| Metric | Target | Current |
|--------|--------|---------|
| VTE parsing throughput | > 10 MB/s | TBD |
| Frame generation time | < 1ms | TBD |
| Plugin load time | < 100ms | TBD |
| Session save time | < 50ms | TBD |
| Memory usage (idle) | < 50MB | TBD |

### Linting

Zero clippy warnings in release builds:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## Testing Strategy

### Unit Tests

Each crate should have comprehensive unit tests:

```bash
cargo test --workspace
```

### Integration Tests

Cross-crate integration tests:

```bash
cargo test --test '*'
```

### Manual Testing

- Smoke test: Basic terminal operations
- Regression test: Test all reported bugs
- Performance test: Benchmark on various hardware
- Platform test: Test on Windows and Linux

## Documentation Tasks

### User Documentation

- [ ] Installation guide
- [ ] Getting started tutorial
- [ ] Keybindings reference
- [ ] Configuration reference
- [ ] Plugin user guide
- [ ] Troubleshooting guide

### Developer Documentation

- [ ] Architecture overview
- [ ] API reference (rustdoc)
- [ ] Plugin development guide
- [ ] Contribution guidelines
- [ ] Code of conduct

## Release Timeline

| Milestone | Target Date | Status |
|-----------|-------------|--------|
| CI setup | Week 1 | ⏳ |
| Benchmarks | Week 2 | ⏳ |
| Documentation | Week 3 | ⏳ |
| Crash reporting | Week 3 | ⏳ |
| Release prep | Week 4 | ⏳ |
| v0.1.0 release | End of Week 4 | ⏳ |

## Success Criteria

Phase 6 is considered complete when:

- ✅ CI passes on all commits to main branch
- ✅ Benchmarks run and results are tracked
- ✅ Documentation is complete and builds successfully
- ✅ Crash reporting is implemented
- ✅ First stable release (v0.1.0) is published

## Related Documentation

- [Getting Started](./getting-started.md) - User guide
- [Contributing](./contributing.md) - Contribution guidelines
- [Architecture](./architecture.md) - Technical details
