---
title: Configuration
sidebar_position: 4
description: Configuration files, paths, and policy settings
---

# Configuration & Policy

**Complete Configuration Guide for OMG**

This guide covers all configuration options, file locations, security policies, and customization options for OMG.

---

## 📍 File Locations

OMG follows the XDG Base Directory Specification with sensible fallbacks.

### Configuration Files

| File | Purpose | Default Path |
|------|---------|--------------|
| **config.toml** | General settings | `~/.config/omg/config.toml` |
| **policy.toml** | Security policy | `~/.config/omg/policy.toml` |

### Data Directory

| Directory | Purpose | Default Path |
| ----------- | --------- | -------------- |
| **Data root** | All OMG data | `~/.local/share/omg/` |
| **Versions** | Runtime installations | `~/.local/share/omg/versions/` |
| **Tools** | Installed CLI tools | `~/.local/share/omg/tools/` |
| **Cache** | Persistent cache (redb) | `~/.local/share/omg/cache.redb` |
| **History** | Transaction history | `~/.local/share/omg/history.json` |
| **Audit** | Audit log | `~/.local/share/omg/audit/audit.jsonl` |

---

## ⚙️ General Configuration (config.toml)

The main configuration file controls telemetry and AUR builds.

Configuration writes from `omg config`, `omg privacy`, and `omg init` share a
`config.lock` beside the config file. A competing write fails with a retry message.
The lock file persists after completion; its presence alone does not mean a writer
is active. External editors do not participate in this coordination.

### Complete Example

```toml
# ~/.config/omg/config.toml

# ═══════════════════════════════════════════════════════════════════════════
# GENERAL SETTINGS
# ═══════════════════════════════════════════════════════════════════════════

# Runtime telemetry is opt-in (default: false)
telemetry_enabled = false

# Data and socket locations are resolved from XDG environment variables.
# They are not configurable in this file.

# ═══════════════════════════════════════════════════════════════════════════
# AUR BUILD SETTINGS
# ═══════════════════════════════════════════════════════════════════════════

[aur]
# Build method: "bubblewrap" (secure, default), "chroot", or "native"
build_method = "bubblewrap"

# Number of parallel AUR builds
build_concurrency = 8

# Require interactive PKGBUILD review before building (default: true)
review_pkgbuild = true

# Use stricter makepkg flags (cleanbuild/verifysource) (default: true)
secure_makepkg = true

# Allow native builds without sandboxing (default: false)
allow_unsafe_builds = false

# Use AUR metadata archive for bulk update checks (default: true)
use_metadata_archive = true

# Metadata archive cache TTL in seconds (default: 300)
metadata_cache_ttl_secs = 300

# MAKEFLAGS for building (passed to makepkg)
# makeflags = "-j8"

# Custom package destination (built packages stored here)
# pkgdest = "/home/user/.cache/omg/pkgdest"

# Custom source destination (sources downloaded here)
# srcdest = "/home/user/.cache/omg/srcdest"

# Cache built packages for faster rebuilds (default: true)
cache_builds = true

# Enable ccache for faster C/C++ builds (default: false)
enable_ccache = false
# ccache_dir = "/home/user/.cache/ccache"

# Enable sccache for faster Rust builds (default: false)
enable_sccache = false
# sccache_dir = "/home/user/.cache/sccache"
```

### Setting Descriptions

#### General Settings

| Setting | Type | Default | Description |
| --------- | ------ | --------- | ------------- |
| `telemetry_enabled` | bool | `false` | Enable runtime telemetry |

#### AUR Settings

| Setting | Type | Default | Description |
| --------- | ------ | --------- | ------------- |
| `build_method` | string | `"bubblewrap"` | Build isolation method (`bubblewrap`, `chroot`, `native`) |
| `build_concurrency` | int | CPU count | Parallel AUR builds |
| `review_pkgbuild` | bool | `true` | Require manual PKGBUILD review |
| `secure_makepkg` | bool | `true` | Use cleanbuild/verifysource |
| `use_metadata_archive` | bool | `true` | Use bulk metadata for fast updates |
| `cache_builds` | bool | `true` | Cache built packages |
| `enable_ccache` | bool | `false` | Use ccache for C/C++ |
| `enable_sccache` | bool | `false` | Use sccache for Rust |

---

## 🛡️ Security Policy (policy.toml)

The security policy controls what packages can be installed and their required security grades.

### Complete Example

```toml
# ~/.config/omg/policy.toml

# ═══════════════════════════════════════════════════════════════════════════
# SECURITY POLICY CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════

# Minimum required security grade for package installation
# Options: "Risk", "Community", "Verified", "Locked"
# 
# Grade hierarchy (lowest to highest):
#   Risk      - Known vulnerabilities present
#   Community - AUR/unsigned packages
#   Verified  - PGP/checksum verified (official repos)
#   Locked    - SLSA Level 3 + PGP verified (core packages)
minimum_grade = "Verified"

# Allow installation of AUR packages
# Set to false to restrict to official repos only
allow_aur = true

# Require PGP signature verification for all packages
# When true, unsigned packages will be rejected
require_pgp = false

# Allowed software licenses for *installed packages* (SPDX identifiers).
# This policy applies to third-party packages, not OMG's own MIT license.
# Leave empty to allow all licenses
# When populated, only packages with these licenses can be installed
allowed_licenses = [
    "AGPL-3.0-or-later",
    "Apache-2.0",
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "GPL-2.0-or-later",
    "GPL-3.0-or-later",
    "LGPL-2.1-or-later",
    "LGPL-3.0-or-later",
    "MPL-2.0",
    "ISC",
    "Unlicense",
    "CC0-1.0",
]

# Explicitly banned packages (will never be installed)
# Useful for blocking packages with known issues
banned_packages = [
    # "example-malicious-package",
    # "deprecated-insecure-tool",
]

# ═══════════════════════════════════════════════════════════════════════════
# ADVANCED POLICY OPTIONS
# ═══════════════════════════════════════════════════════════════════════════

# Block packages with known CVEs above this severity (0.0-10.0)
# max_cve_severity = 7.0

# Require SBOM for installed packages
# require_sbom = false

# Enable SLSA provenance verification
# verify_slsa = true

# Trusted packagers/maintainers
# trusted_maintainers = ["username1", "username2"]
```

### Security Grades Explained

| Grade | Level | Description | Examples |
| ------- | ------- | ------------- | ---------- |
| **Locked** | 3 | SLSA Level 3 + PGP verified | `glibc`, `linux`, `pacman` |
| **Verified** | 2 | PGP/checksum verified | Official repo packages |
| **Community** | 1 | AUR/unsigned sources | AUR packages |
| **Risk** | 0 | Known vulnerabilities | CVE-affected packages |

### Policy Enforcement

When you run `omg install`:

1. **Package grading**: Each package is assigned a security grade
2. **Policy check**: Grade compared against `minimum_grade`
3. **AUR check**: If AUR package and `allow_aur = false`, rejected
4. **PGP check**: If `require_pgp = true` and no signature, rejected
5. **License check**: If `allowed_licenses` is set and license not in list, rejected
6. **Ban check**: If package in `banned_packages`, rejected

### Example Policies

#### Permissive (Default)

```toml
minimum_grade = "Community"
allow_aur = true
require_pgp = false
allowed_licenses = []
banned_packages = []
```

#### Corporate/Secure

```toml
minimum_grade = "Verified"
allow_aur = false
require_pgp = true
allowed_licenses = ["Apache-2.0", "MIT", "BSD-3-Clause"]
banned_packages = ["known-bad-pkg"]
```

#### Paranoid/Air-gapped

```toml
minimum_grade = "Locked"
allow_aur = false
require_pgp = true
allowed_licenses = ["AGPL-3.0-or-later"]
banned_packages = []
```

---

## 🔄 Runtime Management

OMG manages Node, Python, Go, Rust, Ruby, Java, Bun, and Pi natively. There is no runtime-backend selector or implicit fallback manager.

---

## 📁 Version File Support

OMG automatically detects version files in your project:

| File | Runtime | Format |
| ------ | --------- | -------- |
| `.nvmrc` | Node.js | `20.10.0` or `lts/*` |
| `.node-version` | Node.js | `20.10.0` |
| `.bun-version` | Bun | `1.0.25` |
| `.python-version` | Python | `3.12.0` |
| `.ruby-version` | Ruby | `3.3.0` |
| `.go-version` | Go | `1.21.0` |
| `.java-version` | Java | `21` |
| `rust-toolchain.toml` | Rust | TOML format (see below) |
| `.tool-versions` | Multi | asdf format |
| `package.json` | Node/Bun | `engines` or `volta` field |
| `go.mod` | Go | `go 1.21` directive |

### rust-toolchain.toml Format

```toml
[toolchain]
channel = "stable"  # or "nightly", "1.75.0"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu"]
profile = "minimal"  # or "default"
```

### .tool-versions Format

```
node 20.10.0
python 3.12.0
rust stable
go 1.21.0
```

---

## 🌐 Environment Variables

OMG respects these environment variables:

| Variable | Purpose | Default |
| ---------- | --------- | --------- |
| `OMG_SOCKET_PATH` | Override socket path | XDG runtime |
| `OMG_DATA_DIR` | Override data directory | `~/.local/share/omg` |
| `OMG_CONFIG_DIR` | Override config directory | `~/.config/omg` |
| `RUST_LOG` | Logging level filter for CLI/daemon output | `info` |
| `GITHUB_TOKEN` | For `omg env share` | - |
| `XDG_RUNTIME_DIR` | XDG runtime directory | `/run/user/$UID` |
| `XDG_DATA_HOME` | XDG data directory | `~/.local/share` |
| `XDG_CONFIG_HOME` | XDG config directory | `~/.config` |

---

## 🔧 Advanced Configuration

### Systemd Service

Create a systemd user service for the daemon:

```ini
# ~/.config/systemd/user/omgd.service

[Unit]
Description=OMG Package Manager Daemon
After=network.target

[Service]
Type=simple
ExecStart=%h/.local/bin/omgd
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user daemon-reload
systemctl --user enable omgd
systemctl --user start omgd
```

### Shell Hook Customization

The shell hook adds these functions to Zsh:

| Function | Description |
| ---------- | ------------- |
| `omg-ec` | Explicit package count (cached) |
| `omg-tc` | Total package count (cached) |
| `omg-oc` | Orphan count (cached) |
| `omg-uc` | Updates count (cached) |
| `omg-explicit-count` | Fresh explicit count |
| `omg-total-count` | Fresh total count |
| `omg-orphan-count` | Fresh orphan count |
| `omg-updates-count` | Fresh updates count |

Use in your prompt:

```bash
# In .zshrc PROMPT
PROMPT='$(omg-ec) pkgs %~$ '
```

### Custom Mirrors (Future)

```toml
# In config.toml (planned)
[mirrors]
arch = "https://custom-mirror.example.com/archlinux"
aur = "https://aur.archlinux.org"
```

---

## 📋 Configuration Examples

### Minimal Configuration

```toml
# ~/.config/omg/config.toml
# Empty file uses all defaults
```

### Developer Workstation

```toml
# ~/.config/omg/config.toml
[aur]
build_concurrency = 16
enable_ccache = true
cache_builds = true
```

### CI/CD Server

```toml
# ~/.config/omg/config.toml
telemetry_enabled = false

[aur]
build_concurrency = 4
cache_builds = false
```

### Enterprise/Secure

```toml
# ~/.config/omg/policy.toml
minimum_grade = "Verified"
allow_aur = false
require_pgp = true
allowed_licenses = ["Apache-2.0", "MIT", "BSD-3-Clause"]
banned_packages = []
```

---

## 🔍 Troubleshooting Configuration

### Verify Configuration

```bash
# Check config file syntax
omg doctor

# View effective configuration
omg status
```

### Common Issues

| Issue | Solution |
| ------- | ---------- |
| Config not loading | Check file path and TOML syntax |
| Permission denied | Ensure socket/data dirs are writable |
| Policy blocking packages | Lower `minimum_grade` or set `allow_aur = true` |
| Runtime not found | Use one of the documented native runtime names |

### Reset to Defaults

```bash
# Remove config files
rm ~/.config/omg/config.toml
rm ~/.config/omg/policy.toml

# OMG will use defaults
omg status
```

---

## 🎯 Common Configuration Patterns

Real-world configuration examples for different use cases.

### 1. Personal Use (Default)

**Minimal config for single-user development machines:**

```toml
# ~/.config/omg/config.toml
# Most users can skip this - OMG works with zero config!

# Runtime telemetry remains disabled unless explicitly enabled.
telemetry_enabled = false
```

**Policy (optional):**

```toml
# ~/.config/omg/policy.toml
# No strict policies needed for personal use
```

**Best for:** Individual developers, personal laptops, workstations

---

### 2. Team Development

**Shared configuration for consistent team environments:**

```toml
# ~/.config/omg/config.toml
# Share this in your team's dotfiles repo

# Keep runtime telemetry local by default.
telemetry_enabled = false

# Team-friendly AUR settings
[aur]
review_pkgbuild = true  # Require PKGBUILD review
secure_makepkg = true   # Use strict makepkg flags
build_concurrency = 4   # Conservative for shared builders
```

**Policy for team sync:**

```toml
# ~/.config/omg/policy.toml
minimum_grade = "Verified"   # Require verified sources (default is "Community")
allow_aur = false            # Forbid AUR on team machines if desired
banned_packages = []         # Explicitly block problem packages
```

**Usage:**

```bash
# Lock environment for team
omg env capture
git add omg.lock

# Team members verify their machine matches the lock
omg env check

# Or restore directly from a shared Gist:
# omg env sync <gist-url>
```

**Best for:** Small to medium development teams (2-20 people)

---

### 3. CI/CD Pipelines

**Optimized for automation and reproducibility:**

```toml
# ~/.config/omg/config.toml
# Use in CI Docker images or runner VMs

# Minimal AUR builds (avoid in CI when possible)
[aur]
build_concurrency = 8
allow_unsafe_builds = false  # Force secure builds
review_pkgbuild = false      # Can't review in CI

# Disable telemetry in CI
telemetry_enabled = false
```

**Policy for CI:**

```toml
# ~/.config/omg/policy.toml
# Strict security in CI
minimum_grade = "Verified"
require_pgp = true
allow_aur = false
```

**GitHub Actions Example:**

```yaml
# .github/workflows/ci.yml
- name: Install OMG
  run: curl -fsSL https://omg.latham.cloud/install.sh | bash

- name: Lock environment
  run: omg env check  # Verify omg.lock matches

- name: Install dependencies
  run: omg install
```

**Best for:** CI/CD pipelines, Docker images, automated builds

---

### 4. Low-Resource Systems

**Minimize memory and CPU usage:**

```toml
# ~/.config/omg/config.toml
# For VPS, Raspberry Pi, or resource-constrained systems

# Conservative parallelism
[aur]
build_concurrency = 1  # Single-threaded builds
use_metadata_archive = false  # Save memory
```

Note: there is no config switch to disable the daemon — simply do not start
`omgd`; the CLI falls back to direct package-manager queries without it.

**Best for:** VPS, Raspberry Pi, low-RAM systems (<2GB), embedded devices

---

### 5. Maximum Performance

**Optimized for speed on high-end machines:**

```toml
# ~/.config/omg/config.toml
# For workstations with 16+ cores, 32GB+ RAM

[aur]
build_concurrency = 16  # Match CPU cores
use_metadata_archive = true
metadata_cache_ttl_secs = 3600  # Cache longer
enable_ccache = true    # Faster C/C++ rebuilds
enable_sccache = true   # Faster Rust rebuilds
```

Note: the daemon runs automatically when started and keeps its cache sized
internally; there are no `[node]`/`[python]` mirror overrides in config today.

**Best for:** High-end workstations, build servers, performance-critical workflows

---

### 6. Enterprise Security

**Strict policies for compliance and security:**

```toml
# ~/.config/omg/config.toml
# Corporate/enterprise environments

# Strict AUR builds
[aur]
build_method = "bubblewrap"  # Sandboxed builds only
review_pkgbuild = true        # Manual review required
allow_unsafe_builds = false   # No native builds
secure_makepkg = true
```

**Policy for compliance:**

```toml
# ~/.config/omg/policy.toml
# Strict enterprise security policy
minimum_grade = "Verified"
require_pgp = true
allow_aur = false
allowed_licenses = ["Apache-2.0", "MIT", "BSD-3-Clause"]
banned_packages = [
  "untrusted-package",
]
```

**Best for:** Enterprise, regulated industries (healthcare, finance), security-critical environments

---

## 📊 Configuration Comparison

Quick reference for choosing the right configuration:

| Feature | Personal | Team | CI/CD | Low-Resource | Performance | Enterprise |
| --------- | ---------- | ------ | ------- | -------------- | ------------- | ------------ |
| **Daemon** | ✅ Yes | ✅ Yes | ❌ No | ❌ No | ✅ Yes | ✅ Yes |
| **Auto-update** | ✅ Yes | ❌ No | ❌ No | ❌ No | ✅ Yes | ❌ No |
| **Parallel builds** | 8 | 4 | 8 | 1 | 16 | 4 |
| **Security scanning** | Optional | ✅ Yes | ✅ Yes | ❌ No | Optional | ✅ Required |
| **Audit logging** | ❌ No | ✅ Yes | ✅ Yes | ❌ No | Optional | ✅ Required |
| **PKGBUILD review** | ❌ No | ✅ Yes | ❌ No | ❌ No | ❌ No | ✅ Required |
| **Lock files** | Optional | ✅ Yes | ✅ Yes | Optional | Optional | ✅ Required |
| **Memory usage** | ~50MB | ~50MB | ~30MB | ~10MB | ~100MB | ~50MB |

---

## 📚 See Also

- [Security & Compliance](./security.md) — Detailed security policy documentation
- [Daemon Internals](./daemon.md) — Advanced daemon configuration
- [Runtime Management](./runtimes.md) — Version file formats
