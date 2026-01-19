# Package Management

**Complete Guide to Searching, Installing, and Managing Packages**

OMG provides unified package management for official repositories and AUR on Arch Linux, with experimental Debian/Ubuntu support.

---

## 🎯 Overview

OMG's package management features:

- **22x faster searches** than pacman (6ms vs 133ms)
- **Unified AUR integration** — no separate AUR helper needed
- **Security grading** — packages rated before installation
- **Policy enforcement** — organization-wide installation rules
- **Transaction history** — full audit trail with rollback

---

## 🔍 Package Search

### Basic Search

```bash
# Search all repositories (official + AUR)
omg search vim

# Limit results
omg search vim --limit 10

# Search AUR only
omg search -a visual-studio-code

# Search official only
omg search -o firefox
```

### Interactive Search

```bash
# Interactive mode — select packages to install
omg search browser -i
```

Interactive mode provides:
- Fuzzy matching with arrow key navigation
- Multi-select with space bar
- Package descriptions inline
- Security grades displayed

### Search Performance

| Mode | Latency | Notes |
|------|---------|-------|
| With daemon (cached) | ~6ms | Instant feel |
| With daemon (fresh) | ~200ms | Still fast |
| Without daemon | ~500ms | Direct libalpm |

### Fuzzy Matching

OMG uses Nucleo for intelligent fuzzy matching:

```bash
omg search frfx
# Finds: firefox, firefox-developer-edition

omg search vsc
# Finds: visual-studio-code-bin, vscodium-bin
```

---

## 📦 Package Installation

### Install Packages

```bash
# Install single package
omg install firefox

# Install multiple packages
omg install firefox chromium brave-bin

# Install AUR package (auto-detected)
omg install visual-studio-code-bin

# Install as dependency
omg install --asdeps libfoo
```

### Installation Flow

1. **Security Grade Assignment**
   - LOCKED: Core packages with SLSA + PGP
   - VERIFIED: Official packages with PGP signatures
   - COMMUNITY: AUR packages
   - RISK: Known vulnerabilities

2. **Policy Enforcement**
   - Check against `policy.toml`
   - May block based on grade, AUR, or license

3. **Dependency Resolution**
   - Official packages via libalpm
   - AUR dependencies built first

4. **Download & Verify**
   - PGP signatures verified
   - Checksums validated

5. **Installation**
   - Official: pacman integration
   - AUR: makepkg + pacman -U

6. **History Recording**
   - Transaction logged to history.json

### AUR Build Options

Configure in `~/.config/omg/config.toml`:

```toml
[aur]
# Parallel build jobs
build_concurrency = 8

# makepkg flags
makeflags = "-j8"

# Cache built packages
cache_builds = true

# Use ccache for C/C++
enable_ccache = true

# Use sccache for Rust
enable_sccache = false
```

---

## 🗑️ Package Removal

### Remove Packages

```bash
# Remove single package
omg remove firefox

# Remove with orphaned dependencies
omg remove firefox -r

# Remove multiple packages
omg remove pkg1 pkg2 pkg3
```

### Safety Features

- Confirms before removing packages
- Won't remove system dependencies
- Warns about dependent packages

---

## 🔄 System Updates

### Update Packages

```bash
# Update everything (official + AUR)
omg update

# Check for updates without installing
omg update --check
```

### Update Flow

1. **Database Sync** — Fresh package lists
2. **Official Updates** — Via pacman
3. **AUR Updates** — Parallel builds
4. **History Recording** — All changes logged

### Selective Updates

```bash
# Update specific package
omg install firefox  # Re-installing updates if newer

# Update official only (traditional pacman)
sudo pacman -Syu
```

---

## ℹ️ Package Information

### Get Package Details

```bash
omg info firefox
```

**Output includes:**
- Name and version
- Description
- Repository (official/AUR)
- Dependencies and optional dependencies
- Installed files count
- Security grade
- Installation status

### Performance

| Mode | Latency |
|------|---------|
| With daemon (cached) | ~6.5ms |
| Without daemon | ~150ms |

---

## 📋 Package Listings

### Explicitly Installed Packages

```bash
# List all explicit packages
omg explicit

# Count only
omg explicit --count
```

### System Status

```bash
omg status
```

Shows:
- Total packages
- Explicit packages
- Orphaned packages
- Updates available
- Vulnerabilities

---

## 🧹 Cleanup

### Clean Caches

```bash
# Remove orphaned packages
omg clean --orphans

# Clear package cache
omg clean --cache

# Clear AUR build cache
omg clean --aur

# Full cleanup
omg clean --all
```

### Sync Databases

```bash
omg sync
```

---

## 🔐 Security Features

### Security Grades

Every package is assigned a security grade:

| Grade | Meaning | Examples |
|-------|---------|----------|
| **LOCKED** | SLSA Level 3 + PGP | glibc, linux, pacman |
| **VERIFIED** | PGP signature verified | Official repo packages |
| **COMMUNITY** | AUR/unsigned | AUR packages |
| **RISK** | Known vulnerabilities | CVE-affected packages |

### Policy Enforcement

Create `~/.config/omg/policy.toml`:

```toml
# Minimum grade required
minimum_grade = "Verified"

# Allow AUR packages
allow_aur = true

# Require PGP signatures
require_pgp = false

# Allowed licenses (SPDX)
allowed_licenses = ["Apache-2.0", "MIT"]

# Banned packages
banned_packages = ["some-bad-pkg"]
```

### Vulnerability Checking

OMG checks installed packages against:
- **Arch Linux Security Advisory (ALSA)**
- **OSV.dev global database**

Run audit:
```bash
omg audit
```

---

## 📜 Transaction History

### View History

```bash
# Recent transactions
omg history

# Last 5 transactions
omg history --limit 5
```

### Rollback

```bash
# Interactive rollback
omg rollback

# Rollback specific transaction
omg rollback <transaction-id>
```

**Rollback Limitations:**
- Official packages only (AUR rollback planned)
- Requires old packages in cache
- May have dependency conflicts

---

## 🌐 Mirror Management

### Pacman Mirrors

OMG uses system pacman mirrors. Configure in `/etc/pacman.d/mirrorlist`.

### AUR Source

Default AUR endpoint: `https://aur.archlinux.org`

---

## 💨 Performance Tips

### 1. Use the Daemon

```bash
# Start daemon for cache
omg daemon

# Verify it's running
omg status
```

### 2. Use omg-fast for Scripts

```bash
# Ultra-fast package count
omg-fast ec  # Explicit count
omg-fast tc  # Total count
```

### 3. Batch Operations

```bash
# Install multiple at once
omg install pkg1 pkg2 pkg3

# Rather than individual commands
omg install pkg1
omg install pkg2
omg install pkg3
```

---

## 🐧 Platform Support

### Arch Linux (Full Support)

- Official repositories via libalpm
- AUR with full build support
- All features available

### Debian/Ubuntu (Experimental)

Build with Debian feature:
```bash
cargo build --release --features debian
```

Requires `libapt-pkg-dev`:
```bash
sudo apt install libapt-pkg-dev
```

Supported commands:
- `omg search`
- `omg info`
- `omg install`
- `omg remove`
- `omg update`
- `omg explicit`

**Note:** No AUR equivalent on Debian.

---

## 🔧 Troubleshooting

### Search Returns Nothing

```bash
# Sync databases
omg sync

# Restart daemon
pkill omgd && omg daemon

# Try direct
pacman -Ss <query>
```

### AUR Build Fails

```bash
# Check base-devel
pacman -Q base-devel

# Clear cache and retry
omg clean --aur
omg install <package>

# Check logs
cat ~/.cache/omg/logs/*.log
```

### Permission Denied

```bash
# AUR builds shouldn't need sudo
# Official installs prompt for sudo

# If socket issues
ls -la $XDG_RUNTIME_DIR/omg.sock
```

---

## 📚 See Also

- [CLI Reference](./cli.md) — All package commands
- [Security & Compliance](./security.md) — Security grading details
- [Configuration](./configuration.md) — Policy configuration
- [History & Rollback](./history.md) — Transaction management
