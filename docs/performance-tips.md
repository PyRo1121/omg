# Performance Optimization Guide

This guide covers practical performance tips to get the most speed out of OMG.

---

## Quick Wins

### 1. Enable the Daemon (Default)

The daemon provides 12-40x faster operations through in-memory caching:

```bash
# Start daemon (usually auto-started)
omg daemon

# Verify it's running
omg daemon-status
```

**Impact:** Search: 5-11ms (vs 133ms without daemon)

### 2. Use Parallel AUR Builds

```toml
# ~/.config/omg/config.toml
[aur]
build_concurrency = 16  # Adjust to your CPU cores
```

**Impact:** 50% faster multi-package installations

### 3. Enable Build Caching

```toml
[aur]
cache_builds = true
enable_ccache = true
enable_sccache = true
```

**Impact:**

- ccache: 2-5x faster C/C++ recompilation
- sccache: 2-4x faster Rust recompilation
- cache_builds: Skip unchanged PKGBUILDs

---

## Daemon Optimization

### Startup Performance

The daemon loads package indexes on startup (~27ms for 15K packages).

**Optimization 1: Use systemd to auto-start**

```bash
# Enable systemd service
systemctl --user enable omgd.service
systemctl --user start omgd.service
```

**Optimization 2: Keep daemon running**

Don't restart frequently. The daemon is designed to run continuously and uses <50MB RAM.

### Socket Communication

OMG uses Unix Domain Sockets for near-zero IPC latency.

**Verify socket location:**

```bash
omg config get socket_path
# Should be in $XDG_RUNTIME_DIR or /tmp
```

**Tip:** If socket is on a network filesystem, move it to local disk:

```toml
# ~/.config/omg/config.toml
socket_path = "/tmp/omgd.sock"
```

---

## AUR Build Performance

### Parallel Downloads

OMG parallelizes source downloads by default.

**Check concurrency:**

```bash
omg config get aur.build_concurrency
```

**Optimize for your system:**

- **4-8 cores:** `build_concurrency = 4`
- **16+ cores:** `build_concurrency = 8-16`
- **32+ cores:** `build_concurrency = 16-32`

**RAM consideration:** Each build can use 1-2GB. Limit concurrency if you have <16GB RAM.

### ccache Configuration

**Install ccache:**

```bash
omg install ccache
```

**Enable in config:**

```toml
[aur]
enable_ccache = true
ccache_dir = "/var/cache/ccache"  # Shared cache (optional)
```

**Set cache size:**

```bash
ccache -M 10G  # 10GB cache
```

**Impact:** Rebuilds go from minutes to seconds for C/C++ projects.

### sccache for Rust Packages

**Install sccache:**

```bash
omg install sccache
```

**Enable in config:**

```toml
[aur]
enable_sccache = true
```

**Impact:** 2-4x faster Rust package rebuilds.

### Shared Package Cache

Avoid rebuilding the same package multiple times:

```toml
[aur]
pkgdest = "/var/cache/omg/packages"
srcdest = "/var/cache/omg/sources"
```

**Benefits:**

- Reuse built packages across reinstalls
- Share sources across builds
- Save bandwidth

---

## Network Optimization

### Mirror Selection

Use fast, geographically close mirrors:

```bash
# Arch Linux: Use reflector to find fastest mirrors
sudo reflector --latest 20 --protocol https --sort rate --save /etc/pacman.d/mirrorlist

# Verify mirrors are fast
omg doctor --network
```

### Parallel Downloads

**For pacman (Arch):**

```bash
# /etc/pacman.conf
ParallelDownloads = 5
```

OMG respects pacman's parallel download settings.

### AUR Metadata Archive

Enable bulk metadata fetching:

```toml
[aur]
use_metadata_archive = true
metadata_cache_ttl_secs = 300  # 5 minutes
```

**Impact:** Checking 100+ packages for updates goes from 10s to 1s.

---

## Cache Management

### Cache TTLs

**Metadata cache:** Controls AUR metadata freshness.

```toml
[aur]
metadata_cache_ttl_secs = 300  # 5 minutes (default)
```

**Recommendations:**

- **Development:** 300 (5 min) - fresh data
- **CI/CD:** 600 (10 min) - reduce network calls
- **Production:** 900 (15 min) - stability over freshness

### Status Cache

The daemon caches package status (installed, explicit, etc.):

**Cache location:**

```bash
~/.local/share/omg/daemon/cache.redb
```

**Automatic refresh:** Every 5 minutes in background

**Manual refresh:**

```bash
omg sync  # Forces immediate index rebuild
```

---

## Runtime Version Switching

### PATH Hooks

OMG switches native runtime versions through shell hooks that update `PATH` when the working directory changes. It does not install executable shims or wrapper binaries.

### Native Runtime Resolution

OMG resolves only its native Node, Python, Rust, Go, Ruby, Java, Bun, and Pi installations. Unsupported names fail immediately instead of probing or starting a fallback manager.

---

## CI/CD Performance

### Persistent Daemon

**GitHub Actions:**

```yaml
- name: Start OMG daemon
  run: |
    omg daemon &
    sleep 2  # Wait for startup
    
- name: Install dependencies
  run: omg install firefox vim
```

**Docker:**

```dockerfile
RUN omg daemon &
RUN sleep 2 && omg install build-essentials
```

### Cache Reuse

**Cache OMG data directory:**

```yaml
- uses: actions/cache@v3
  with:
    path: ~/.local/share/omg
    key: omg-${{ runner.os }}-${{ hashFiles('**/omg.lock') }}
```

**Impact:** Skip re-downloading packages and runtimes.

### Parallel Installs

Install independent packages in parallel:

```bash
# Sequential (slow)
omg install gcc clang rustup

# Parallel (fast)
omg install gcc &
omg install clang &
omg install rustup &
wait
```

---

## Filesystem Optimization

### Use Fast Storage

OMG benefits from fast disk I/O:

**Benchmark your disk:**

```bash
dd if=/dev/zero of=/tmp/testfile bs=1M count=1024 conv=fdatasync
```

**Recommendations:**

- **NVMe SSD:** Ideal (5000+ MB/s)
- **SATA SSD:** Good (500+ MB/s)
- **HDD:** Acceptable but slower (100+ MB/s)

### Mount Options

**For /tmp (daemon socket):**

```bash
# /etc/fstab
tmpfs /tmp tmpfs defaults,noatime,mode=1777 0 0
```

**For build directories:**

```bash
# Use tmpfs for builds (fast but RAM-limited)
export BUILDDIR=/tmp/makepkg
```

---

## Monitoring Performance

### Built-in Metrics

```bash
# Show operation times
omg stats

# Prometheus-style metrics
omg metrics
```

### Daemon Status

```bash
omg daemon-status
```

**Look for:**

- Cache hit rate (aim for >80%)
- Response times (<10ms for search/info)
- Memory usage (<100MB)

### Doctor Check

```bash
omg doctor --network --eol
```

**Checks:**

- Mirror connectivity
- Network latency
- Package database freshness

---

## Troubleshooting Slow Performance

### Daemon Not Running

**Symptom:** Commands take 100ms+ instead of <10ms

**Fix:**

```bash
omg daemon
```

### Cache Corruption

**Symptom:** Errors or slow index rebuilds

**Fix:**

```bash
rm -rf ~/.local/share/omg/daemon/cache.redb
omg sync  # Rebuild
```

### Slow Mirrors

**Symptom:** Package downloads are slow

**Fix:**

```bash
omg doctor --network
# Follow recommendations to switch mirrors
```

### High Memory Usage

**Symptom:** Daemon using >200MB RAM

**Possible causes:**

- Large AUR index (80K+ packages)
- Many cached search results

**Fix:**

```bash
# Restart daemon to clear memory
pkill omgd
omg daemon
```

### Slow AUR Builds

**Symptom:** Builds taking longer than expected

**Checklist:**

1. Enable ccache/sccache
2. Increase build_concurrency
3. Use tmpfs for build directory
4. Check available RAM (each build needs 1-2GB)

---

## Advanced Tuning

### Custom MAKEFLAGS

```toml
[aur]
makeflags = "-j16"  # Limit parallel make jobs
```

**When to use:** If builds are failing due to memory exhaustion.

### Build Method

```toml
[aur]
build_method = "native"  # Fastest
# build_method = "bubblewrap"  # Secure but slower
# build_method = "chroot"  # Most secure, slowest
```

**Performance:**

- `native`: Fastest (no sandbox overhead)
- `bubblewrap`: -10% performance
- `chroot`: -30% performance

---

## Performance Baseline

### Expected Response Times

| Operation | Target | Acceptable | Slow |
| ----------- | -------- | ------------ | ------ |
| Search | <10ms | <50ms | >100ms |
| Info | <10ms | <50ms | >100ms |
| Status | <10ms | <50ms | >100ms |
| Runtime switch | <5ms | <50ms | >200ms |
| Install (single) | <5s | <30s | >60s |

### How to Benchmark

```bash
# Use hyperfine for accurate benchmarks
hyperfine --warmup 3 "omg search firefox --no-aur"

# Compare with pacman
hyperfine --warmup 3 "pacman -Ss firefox"
```

---

## Real-World Performance

### Development Workstation

**Setup:**

- 16-core CPU
- 32GB RAM
- NVMe SSD
- ccache enabled

**Results:**

- Search: 5-7ms
- Install (cached): 2-4s
- AUR rebuild (with ccache): 10-30s
- Runtime switch: 1-2ms

### CI/CD Pipeline

**Setup:**

- GitHub Actions (2-core)
- Standard runner
- Cached OMG data

**Results:**

- Search: 10-15ms
- Install (cached): 5-10s
- Full environment setup: <30s

### Production Server

**Setup:**

- 64-core CPU
- 128GB RAM
- High-performance storage

**Results:**

- Search: 3-5ms
- Bulk install (16 parallel): <60s
- Update check (100+ packages): <3s

---

## Summary

**Top 5 Performance Tips:**

1. **Keep daemon running** - 12-40x faster operations
2. **Enable ccache/sccache** - 2-5x faster rebuilds
3. **Increase build_concurrency** - 50% faster multi-package installs
4. **Use fast mirrors** - Check with `omg doctor --network`
5. **Cache in CI/CD** - Reuse data directory between runs

**Impact:** With all optimizations, OMG delivers:

- **Package search:** 5-11ms (12-24x faster than pacman)
- **Info queries:** 3-6ms (21-38x faster than pacman)
- **AUR builds:** 50% faster than yay
- **Runtime switching:** <2ms (50-100x faster than nvm/pyenv)

---

## Additional Resources

- [Benchmark Results](../BENCHMARK-RESULTS.md) - Detailed performance analysis
- [Daemon Architecture](daemon.md) - How the daemon achieves low latency
- [Performance Tips](./performance-tips.md) - General optimization guidance
- [Configuration Guide](configuration.md) - All config options explained
