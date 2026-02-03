# Cargo Build Profiles

Quick reference for choosing the optimal build profile.

## Available Profiles

| Profile | Build Time | Binary Size | Runtime Speed | Use Case |
|---------|------------|-------------|---------------|----------|
| `dev` (default) | ~14s | 456M (debug) | Slow | Development iteration |
| `release-fast` | ~52s | 20M | -5% | Fast iteration on optimized builds |
| `release` | ~46s | 16M | Fastest | Production deployment |
| `pgo-instrument` | ~30s | 20M | -10% | PGO Phase 1: collect profile data |
| `release-pgo` | ~90s | 18M | +8-15% | PGO Phase 2: optimized build |
| `release-size` | ~52s | <16M | -10% | Size-constrained environments |
| `bench` | ~46s | 16M | Fastest | Benchmarking/profiling |

## Usage

```bash
# Development (fastest builds)
cargo build
cargo run

# Fast optimized builds (recommended for local testing)
cargo build --profile release-fast
./target/release-fast/omg

# Production release (maximum performance)
cargo build --release
./target/release/omg

# Size-optimized (embedded/constrained environments)
cargo build --profile release-size
./target/release-size/omg

# Benchmarking (with debug symbols for profiling)
cargo build --profile bench
cargo bench
```

## Profile Details

### `dev` (Development)
- **Optimization:** None (opt-level=0)
- **LTO:** Disabled
- **Incremental:** Enabled
- **Debug Info:** Full
- **Best for:** Rapid development, debugging

### `release-fast`
- **Optimization:** Good (opt-level=2)
- **LTO:** Thin (80% of fat LTO benefits)
- **Codegen Units:** 16 (parallel compilation)
- **Incremental:** Enabled
- **Best for:** Local testing of optimized builds

### `release` (Production)
- **Optimization:** Maximum (opt-level=3)
- **LTO:** Fat (full cross-crate inlining)
- **Codegen Units:** 1 (maximum optimization)
- **Incremental:** Disabled (LTO incompatible)
- **Best for:** Production deployments, CI/CD

### `pgo-instrument` (PGO Phase 1)
- **Optimization:** Good (opt-level=2)
- **LTO:** Off (avoids MIR inliner crashes during instrumentation)
- **Codegen Units:** 16 (maximum parallelism)
- **Best for:** Building instrumented binary to collect profile data
- **Note:** Doesn't need to be fast, just needs to run workloads

### `release-pgo` (PGO Phase 2)
- **Optimization:** Maximum (opt-level=3)
- **LTO:** Thin (safe when using profile data)
- **Codegen Units:** 8 (balanced)
- **Best for:** Building optimized binary with profile data
- **Note:** Separate from instrumentation to avoid rustc bugs

### `release-size`
- **Optimization:** Size (opt-level="z")
- **LTO:** Fat
- **Strip:** Symbols (aggressive)
- **Best for:** Docker images, embedded systems

### `bench`
- **Optimization:** Maximum (inherits from release)
- **Debug Info:** Enabled
- **Best for:** Performance profiling, flamegraphs

## Build Time Comparison

```
dev:          ~14s  (baseline)
release-fast: ~42s  (3x slower than dev, 9% faster than release)
release:      ~46s  (production standard)
release-size: ~52s  (13% slower for size optimization)
```

## Advanced Optimizations

### CPU-Specific Builds (5-10% faster, NOT portable)

Enable CPU-specific instructions (AVX2, BMI2, etc.):

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

**Warning:** Binary will ONLY run on CPUs with same features as build machine.

Alternatively, uncomment in `.cargo/config.toml`:

```toml
[build]
rustflags = ["-C", "target-cpu=native"]
```

### Profile-Guided Optimization (8-15% faster)

**CRITICAL:** PGO requires **TWO separate profiles** to avoid rustc crashes!

Use `build-pgo.sh` for automated two-phase workflow:

```bash
./build-pgo.sh
```

Or manually:

```bash
# Phase 1: Build instrumented binary (lightweight profile)
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" \
    cargo build --profile pgo-instrument

# Phase 2: Run realistic workloads
./target/pgo-instrument/omgd &
./target/pgo-instrument/omg search vim firefox git python
./target/pgo-instrument/omg info vim
./target/pgo-instrument/omg status
killall omgd

# Phase 3: Build optimized binary with profile data
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data -Cllvm-args=-pgo-warn-missing-function" \
    cargo build --profile release-pgo
```

**Why Two Separate Profiles?**

| Issue | Solution |
|-------|----------|
| **MIR inliner stack overflow** during instrumentation | Use `pgo-instrument` with `lto=false`, `opt-level=2` |
| **GCC internal error** in aws-lc-sys with fat LTO + PGO | Use `release-pgo` with `lto="thin"` |
| **rustc SIGSEGV** in LLVM inliner with aggressive optimization | Separate instrumentation from optimization |

The profile data collected at opt-level=2 is still valid for optimizing at opt-level=3—it captures branch frequencies and call patterns, not optimization artifacts.

### Minimal Binary Size (<14MB)

Remove PGP verification to save 1.2MB:

```bash
cargo build --profile release-size --no-default-features --features arch
```

**Tradeoff:** Loses PGP signature verification for AUR packages.

## Linker Optimization

For faster linking, install mold or lld:

```bash
# Install mold (10x faster linking)
sudo apt install mold

# Or use lld (5x faster linking)
sudo apt install lld

# Configure in .cargo/config.toml
```

## Serialization Architecture

OMG uses different serialization strategies optimized for each use case:

### IPC Protocol (Client ↔ Daemon)
- **Library:** `bitcode` (fastest non-zero-copy serializer)
- **Why:** Simple API, small messages (<1KB), already in memory
- **Performance:** <1ms deserialization, sufficient for IPC

### Persistent Cache (Daemon ↔ Disk)
- **Library:** `rkyv` (zero-copy deserialization)
- **Why:** Large data (package index), benefits from zero-copy reads
- **Performance:** 2-4x faster than bitcode for large data

**Decision:** Zero-copy IPC not worth complexity tradeoff for small messages.

## Recommendations

- **Daily development:** Use `dev` profile (default)
- **Pre-release testing:** Use `release-fast` profile
- **Production builds:** Use `release` profile
- **CI/CD:** Use `release` profile
- **Docker images:** Use `release-size` profile
- **Maximum performance:** PGO + `target-cpu=native` (local only)
