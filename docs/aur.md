# AUR Support

OMG supports the Arch User Repository (AUR) with the same install, search, and update commands used for official packages.

## Overview

OMG treats AUR packages as first-class citizens, with no distinction between official repository packages and AUR packages in the CLI. Just use `omg install` for everything.

**Key advantages over yay/paru:**

- **One command surface** for official and AUR packages
- **No sudo timeouts** during long builds
- **Parallel source downloads** for multi-source packages
- **Smart dependency resolution** that skips unnecessary API calls
- **Real-time progress tracking** for downloads and installations

## Performance Features

### 1. Parallel Source Downloads

When building AUR packages with multiple sources, OMG downloads them all concurrently instead of sequentially.

**Impact**: Total download time approaches the slowest single source instead of the sum.

### 2. Smart Dependency Resolution

Before querying the AUR API for dependencies, OMG filters out packages already installed on your system.

**Impact**: Eliminates unnecessary network calls for packages with many deps.

### 3. Sudoloop Mechanism

OMG automatically maintains sudo authentication throughout the entire build process via a background refresh thread.

**Impact**: No more password prompts mid-build. Critical for packages with long compilation times (e.g., `chromium`, `linux`).

### 4. Metadata Archive

Bulk update checks use the AUR metadata archive instead of one request per package.

**Impact**: Fewer API calls and faster update discovery across many AUR packages.

### 5. Streamlined Build Process

Unnecessary intermediate cleanup steps have been removed, and cleanup only occurs when actually needed.

**Impact**: Reduces overhead and I/O operations during builds.

### 6. Real-time Progress Tracking

Live progress indicators show exactly what's downloading, building, and installing.

**Impact**: Better user experience and visibility into what OMG is doing.

## Configuration

Add these to your `~/.config/omg/config.toml`:

```toml
[aur]
# Build isolation (default: "bubblewrap"); "chroot" or "native"
build_method = "bubblewrap"

# Maximum concurrent AUR builds (default: CPU count)
build_concurrency = 4

# Require interactive PKGBUILD review before building (default: true)
review_pkgbuild = true

# Reuse build outputs when the PKGBUILD hash matches (default: true)
cache_builds = true
```

## Usage Examples

```bash
# Install an AUR package (just like pacman/yay)
omg install yay

# Install with dependencies
omg install spotify

# Search AUR and official repos together
omg search chrome

# Update all packages (official + AUR)
omg update

# Remove an AUR package
omg remove spotify
```

## Benchmarks

Reproducible measurements live in `benchmarks/records/` (search, info, status, and explicit operations). AUR install times depend on network speed, CPU, and package complexity, so they are not published as fixed numbers.

## How It Works

When you run `omg install <aur-package>`:

1. **Query AUR API** for package metadata
2. **Smart dependency check**: Filter already-installed deps from the list
3. **Clone git repo** to build directory
4. **Start sudoloop** in background to maintain authentication
5. **Parse PKGBUILD** using cached regex patterns
6. **Download sources in parallel** via tokio async runtime
7. **Build package** with makepkg
8. **Install with pacman** using maintained sudo session
9. **Cleanup** build artifacts (unless `keep_build = true`)

## Troubleshooting

### Sudo timeout during builds

OMG refreshes sudo authentication automatically during builds. If a prompt still appears, run `omg doctor` to diagnose privilege issues.

### Slow builds

Raise the concurrent build limit:

```toml
[aur]
build_concurrency = 8  # Build up to 8 packages concurrently
```

### Build failures

Point `pkgdest` and `srcdest` at known locations so outputs survive for inspection:

```toml
[aur]
pkgdest = "/var/cache/omg/packages"
srcdest = "/var/cache/omg/sources"
```

## Comparison with yay/paru

| Feature | OMG | yay | paru |
| --------- | ----- | ----- | ------ |
| Parallel downloads | ✅ Yes | ❌ No | ❌ No |
| Smart dep resolution | ✅ Yes | ❌ No | ❌ No |
| Sudoloop | ✅ Yes | ❌ No | ⚠️ Partial |
| Progress tracking | ✅ Real-time | ⚠️ Basic | ⚠️ Basic |
| Build cache reuse | ✅ Yes | ❌ No | ❌ No |

## Technical Details

### Parallel Download Implementation

OMG uses Rust's async runtime (tokio) to spawn concurrent download tasks:

```rust
// Pseudo-code representation
let handles: Vec<_> = sources
    .iter()
    .map(|src| tokio::spawn(download(src)))
    .collect();

for handle in handles {
    handle.await?;
}
```

### Sudoloop Architecture

A dedicated background thread spawns `sudo -v` every N seconds:

```rust
thread::spawn(move || {
    loop {
        Command::new("sudo").arg("-v").output().ok();
        thread::sleep(Duration::from_secs(refresh_interval));
    }
});
```

### Smart Dependency Filtering

Before querying AUR:

```rust
let installed = get_installed_packages();
let deps_to_fetch: Vec<_> = deps
    .iter()
    .filter(|d| !installed.contains(d))
    .collect();
```

This simple optimization can eliminate 50%+ of AUR API calls for packages with many common dependencies.

## Contributing

Found a way to make AUR support even faster? Open an issue or PR!

See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.
