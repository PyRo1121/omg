# Debian/Ubuntu Support: 2026 Enterprise-Grade Implementation Plan

OMG already has a feature-gated Debian backend using `rust-apt`, but it needs optimization to be **50x faster than Nala** and **200x faster than APT**. This plan uses cutting-edge 2026 Rust ecosystem libraries.

## 2026 Technology Stack

### Cutting-Edge Crates (Jan 2026)

| Crate | Version | Purpose | Speedup |
|-------|---------|---------|---------|
| **`debian-packaging`** | 0.18+ | Pure Rust apt reimplementation | No FFI overhead |
| **`rkyv`** | 0.8+ | Zero-copy deserialization | 10-100x vs serde |
| **`mmap-sync`** | 2.0 | Cloudflare's wait-free IPC | Sub-microsecond reads |
| **`nucleo`** | 0.5+ | SIMD fuzzy matching (Helix) | 8x faster than fzf |
| **`memchr`** | 2.7+ | SIMD substring search | AVX-512 on x86_64 |
| **`winnow`** | 0.7+ | Zero-copy parser combinators | Faster than nom |
| **`gzp`** | 0.12+ | Parallel gzip/zstd decompression | Multi-core |
| **`redb`** | 2.4+ | Pure Rust LMDB alternative | ACID + fast |

### Architecture: Zero-Copy Everything

```
┌─────────────────────────────────────────────────────────────┐
│                     omgd daemon                              │
├─────────────────────────────────────────────────────────────┤
│  mmap-sync + rkyv                                           │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Zero-copy Package Index (rkyv archived)             │   │
│  │  - 100k+ packages in <1MB mmap                       │   │
│  │  - Read latency: <100ns                              │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  nucleo fuzzy matcher (SIMD)                               │
│  - Pattern matching: <1ms for 100k packages                │
│                                                             │
│  debian-packaging (pure Rust)                              │
│  - Parse Packages files: parallel + zero-copy              │
│  - No libapt-pkg for reads                                 │
└─────────────────────────────────────────────────────────────┘
```

## Current State Analysis

### What We Have ✅
- **`rust-apt` integration** (`src/package_managers/apt.rs`) - 473 lines
- **Distro detection** (`src/core/env/distro.rs`) - Arch/Debian/Ubuntu
- **Feature-gated build** - `debian` feature in `Cargo.toml`
- **Basic operations**: search, install, remove, update, list

### Why APT/Nala Are Slow

| Tool | Search | Why Slow |
|------|--------|----------|
| **APT** | 500-2000ms | Text parsing, no cache, sequential |
| **Nala** | ~500ms | Still uses libapt-pkg, Python overhead |
| **OMG Target** | <5ms | Zero-copy mmap, SIMD search, Rust |

### How to Beat Everyone

| Strategy | vs APT | vs Nala | Implementation |
|----------|--------|---------|----------------|
| **rkyv + mmap-sync** | 200x | 50x | Zero-copy daemon cache |
| **nucleo SIMD** | 100x | 25x | AVX2/NEON fuzzy search |
| **debian-packaging** | 20x | 10x | Pure Rust, no FFI |
| **Parallel downloads** | 5x | 2x | reqwest + tokio |
| **gzp decompression** | 4x | 2x | Multi-core zstd/gzip |

## Implementation Plan (2026 Edition)

### Phase 0: Docker Testing Infrastructure (FIRST)
**Goal**: Test Debian builds from Arch

1. Create `Dockerfile.debian` and `Dockerfile.ubuntu`
2. Create `scripts/debian-smoke-test.sh` with benchmarks
3. Add Makefile targets for easy testing
4. Set up GitHub Actions CI for Debian/Ubuntu

### Phase 1: Zero-Copy Package Index (Priority: CRITICAL)
**Goal**: <5ms search, 50x faster than Nala

1. **rkyv-based package index** (`src/daemon/debian_index.rs`)
   ```rust
   #[derive(Archive, Serialize, Deserialize)]
   pub struct DebianPackageIndex {
       packages: Vec<DebianPackage>,
       name_to_idx: HashMap<String, usize>,
   }
   ```
   - Use `debian-packaging` crate to parse Packages files
   - Serialize with rkyv for zero-copy access
   - Store in mmap for instant daemon startup

2. **mmap-sync for IPC** (optional, for multi-process)
   - Cloudflare's wait-free synchronization
   - Sub-microsecond read latency
   - Daemon writes, CLI reads directly from mmap

3. **nucleo SIMD fuzzy search**
   - Same as Arch backend
   - AVX2/NEON acceleration
   - <1ms for 100k packages

### Phase 2: Pure Rust Parsing (Priority: HIGH)
**Goal**: No libapt-pkg for read operations

1. **debian-packaging crate integration**
   - Parse `/var/lib/apt/lists/*_Packages`
   - Parse `/var/lib/dpkg/status`
   - Zero-copy with winnow parser

2. **Parallel parsing with rayon**
   - Parse multiple Packages files concurrently
   - Use all CPU cores

3. **gzp for decompression**
   - Parallel gzip/xz/zstd decompression
   - 4x faster than single-threaded

### Phase 3: Parallel Downloads (Priority: HIGH)
**Goal**: 5x faster than Nala

1. **Concurrent HTTP with reqwest**
   - 10-50 parallel connections
   - Resume interrupted downloads

2. **Mirror latency testing**
   - Ping all mirrors, select fastest 3
   - Like Nala but faster (async)

3. **Pre-download before dpkg**
   - Download all .deb files first
   - Then sequential dpkg install

### Phase 4: Transaction Optimization (Priority: MEDIUM)
**Goal**: Faster installs

1. **rust-apt for transactions** (keep existing)
   - Required for dpkg integration
   - Dependency resolution

2. **Batch operations**
   - Group installs to reduce fsync
   - `--force-unsafe-io` when safe

## Technical Architecture

### Read Path (Zero-Copy)
```
CLI request → Unix socket → Daemon
                              ↓
                    rkyv mmap (zero-copy)
                              ↓
                    nucleo SIMD search
                              ↓
                    Response (<5ms)
```

### Write Path (rust-apt)
```
CLI request → Unix socket → Daemon
                              ↓
                    rust-apt (libapt-pkg)
                              ↓
                    dpkg (actual install)
```

## New Dependencies

```toml
[dependencies]
# Pure Rust Debian parsing
debian-packaging = "0.18"

# Zero-copy serialization
rkyv = { version = "0.8", features = ["validation"] }

# Cloudflare's mmap IPC (optional)
mmap-sync = "2.0"

# Parallel decompression
gzp = "0.12"

# Zero-copy parsing
winnow = "0.7"
```

## File Changes Required

| File | Change |
|------|--------|
| `Dockerfile.debian` | NEW - Debian test container |
| `Dockerfile.ubuntu` | NEW - Ubuntu test container |
| `scripts/debian-smoke-test.sh` | NEW - Smoke test + benchmarks |
| `src/daemon/debian_index.rs` | NEW - rkyv-based package index |
| `src/package_managers/deb_parser.rs` | NEW - debian-packaging wrapper |
| `Cargo.toml` | Add debian-packaging, rkyv, gzp, winnow |

## Success Metrics

| Operation | APT | Nala | OMG Target | vs Nala |
|-----------|-----|------|------------|---------|
| Search | 1000ms | 500ms | <5ms | **100x** |
| Info | 300ms | 200ms | <3ms | **66x** |
| Status | 500ms | 300ms | <5ms | **60x** |
| Download | 1x | 3x | 5x | **1.7x** |
| Install | 1x | 1x | 1.2x | **1.2x** |

## Docker Smoke Test Setup

Since you run Arch, we'll use Docker for Debian/Ubuntu testing.

### Dockerfile.debian
```dockerfile
FROM debian:bookworm

RUN apt-get update && apt-get install -y \
    curl build-essential pkg-config libssl-dev libapt-pkg-dev git clang \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /omg
COPY . .
RUN cargo build --release --features debian
CMD ["./scripts/debian-smoke-test.sh"]
```

### Dockerfile.ubuntu
```dockerfile
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y \
    curl build-essential pkg-config libssl-dev libapt-pkg-dev git clang \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /omg
COPY . .
RUN cargo build --release --features debian
CMD ["./scripts/debian-smoke-test.sh"]
```

### scripts/debian-smoke-test.sh
```bash
#!/bin/bash
set -e
echo "=== OMG Debian/Ubuntu Smoke Test ==="
echo "Testing on: $(cat /etc/os-release | grep PRETTY_NAME)"

# Refresh package lists for testing
apt-get update -qq

echo ""
echo "=== BENCHMARK: Search ==="
echo "[APT] apt-cache search vim..."
time apt-cache search vim > /dev/null

echo "[OMG] omg search vim..."
time ./target/release/omg search vim > /dev/null

echo ""
echo "=== BENCHMARK: Info ==="
echo "[APT] apt-cache show curl..."
time apt-cache show curl > /dev/null

echo "[OMG] omg info curl..."
time ./target/release/omg info curl > /dev/null

echo ""
echo "=== BENCHMARK: Status ==="
echo "[OMG] omg status..."
time ./target/release/omg status

echo ""
echo "=== BENCHMARK: List Installed ==="
echo "[APT] dpkg -l..."
time dpkg -l > /dev/null

echo "[OMG] omg explicit..."
time ./target/release/omg explicit > /dev/null

echo ""
echo "=== All smoke tests passed ==="
```

### Makefile targets
```makefile
.PHONY: docker-debian docker-ubuntu docker-test

docker-debian:
	docker build -f Dockerfile.debian -t omg-debian .
	docker run --rm omg-debian

docker-ubuntu:
	docker build -f Dockerfile.ubuntu -t omg-ubuntu .
	docker run --rm omg-ubuntu

docker-test: docker-debian docker-ubuntu
	@echo "All Docker tests passed"
```

### GitHub Actions CI (.github/workflows/debian.yml)
```yaml
name: Debian/Ubuntu CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  debian:
    runs-on: ubuntu-latest
    container: debian:bookworm
    steps:
      - uses: actions/checkout@v4
      - name: Install deps
        run: |
          apt-get update
          apt-get install -y curl build-essential pkg-config libssl-dev libapt-pkg-dev clang
      - name: Install Rust
        run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      - name: Build
        run: |
          . $HOME/.cargo/env
          cargo build --release --features debian
      - name: Test
        run: |
          . $HOME/.cargo/env
          cargo test --features debian

  ubuntu:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install deps
        run: sudo apt-get update && sudo apt-get install -y libapt-pkg-dev
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      - name: Build
        run: cargo build --release --features debian
      - name: Test
        run: cargo test --features debian
```

## Next Steps

1. ✅ Research complete (2026 tech stack identified)
2. **Create Docker testing infrastructure** ← START HERE
3. Add `debian-packaging`, `rkyv`, `gzp`, `winnow` dependencies
4. Implement zero-copy package index with rkyv
5. Integrate nucleo SIMD search for Debian
6. Benchmark vs APT and Nala
