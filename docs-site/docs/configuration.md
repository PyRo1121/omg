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
|-----------|---------|--------------|
| **Data root** | All OMG data | `~/.local/share/omg/` |
| **Versions** | Runtime installations | `~/.local/share/omg/versions/` |
| **Tools** | Installed CLI tools | `~/.local/share/omg/tools/` |
| **Mise** | Bundled mise binary | `~/.local/share/omg/mise/` |
| **Cache** | Persistent cache (redb) | `~/.local/share/omg/cache.redb` |
| **History** | Transaction history | `~/.local/share/omg/history.json` |
| **Audit** | Audit log | `~/.local/share/omg/audit/audit.jsonl` |

### Runtime Files

| File | Purpose | Default Path |
|------|---------|--------------|
| **Socket** | Daemon IPC | `$XDG_RUNTIME_DIR/omg.sock` or `/tmp/omg.sock` |
| **Status** | Fast status cache | `$XDG_RUNTIME_DIR/omg.status` |
| **PID** | Daemon PID | `$XDG_RUNTIME_DIR/omgd.pid` |

### Project Files

| File | Purpose |
|------|---------|
| **omg.lock** | Environment lockfile (in project root) |

---

## ⚙️ General Configuration (config.toml)

The main configuration file controls daemon behavior, runtime settings, and feature toggles.

### Complete Example

```toml
# ~/.config/omg/config.toml

# ═══════════════════════════════════════════════════════════════════════════
# GENERAL SETTINGS
# ═══════════════════════════════════════════════════════════════════════════

# Enable shim system for IDE compatibility (default: false)
# Shims are slower than PATH modification but work with all IDEs
shims_enabled = false

# Override data directory (default: ~/.local/share/omg)
# Uncomment to customize:
# data_dir = "/custom/path/omg"

# Override socket path (default: $XDG_RUNTIME_DIR/omg.sock)
# Uncomment to customize:
# socket_path = "/run/user/1000/omg.sock"

# Default shell for hooks and completions
# Options: "zsh", "bash", "fish"
default_shell = "zsh"

# Automatically check for OMG updates (default: false)
auto_update = false

# Runtime backend preference
# Options: "native", "mise", "native-then-mise" (default)
runtime_backend = "native-then-mise"

# ═══════════════════════════════════════════════════════════════════════════
# AUR BUILD SETTINGS
# ═══════════════════════════════════════════════════════════════════════════

[aur]
# Number of parallel AUR builds
build_concurrency = 8

# MAKEFLAGS for building (passed to makepkg)
makeflags = "-j8"

# Custom package destination (built packages stored here)
pkgdest = "/home/user/.cache/omg/pkgdest"

# Custom source destination (sources downloaded here)
srcdest = "/home/user/.cache/omg/srcdest"

# Cache built packages for faster rebuilds
cache_builds = true

# Enable ccache for faster C/C++ builds
enable_ccache = false
ccache_dir = "/home/user/.cache/ccache"

# Enable sccache for faster Rust builds
enable_sccache = false
sccache_dir = "/home/user/.cache/sccache"

# Build method: "standard" (makepkg) or "containerized" (isolated)
build_method = "standard"

# ═══════════════════════════════════════════════════════════════════════════
# DAEMON SETTINGS
# ═══════════════════════════════════════════════════════════════════════════

[daemon]
# Background refresh interval (seconds)
refresh_interval = 300

# Maximum cached search results
max_cache_entries = 1000

# Cache TTL for search results (seconds)
cache_ttl = 300

# ═══════════════════════════════════════════════════════════════════════════
# NETWORK SETTINGS
# ═══════════════════════════════════════════════════════════════════════════

[network]
# Request timeout for AUR/runtime APIs (seconds)
timeout = 30

# Enable parallel downloads
parallel_downloads = true

# Maximum concurrent downloads
max_parallel = 5

# Custom user agent
# user_agent = "omg/0.1.0"
```

### Setting Descriptions

#### General Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `shims_enabled` | bool | `false` | Use shims instead of PATH modification |
| `data_dir` | string | `~/.local/share/omg` | Override data directory |
| `socket_path` | string | XDG runtime | Override socket path |
| `default_shell` | string | `"zsh"` | Default shell for hooks |
| `auto_update` | bool | `false` | Auto-check for OMG updates |
| `runtime_backend` | string | `"native-then-mise"` | Runtime resolution strategy |

#### AUR Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `build_concurrency` | int | `8` | Parallel AUR builds |
| `makeflags` | string | `"-j8"` | makepkg flags |
| `pkgdest` | string | | Custom package output |
| `srcdest` | string | | Custom source directory |
| `cache_builds` | bool | `true` | Cache built packages |
| `enable_ccache` | bool | `false` | Use ccache |
| `enable_sccache` | bool | `false` | Use sccache |
| `build_method` | string | `"standard"` | Build isolation method |

#### Daemon Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `refresh_interval` | int | `300` | Status refresh interval (seconds) |
| `max_cache_entries` | int | `1000` | Maximum cache entries |
| `cache_ttl` | int | `300` | Cache TTL (seconds) |

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

# Allowed software licenses (SPDX identifiers)
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
|-------|-------|-------------|----------|
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

## 🔄 Runtime Backend Configuration

OMG supports three runtime backends:

### native

Uses OMG's built-in pure Rust runtime managers.

```toml
runtime_backend = "native"
```

**Supported runtimes**: Node, Python, Go, Rust, Ruby, Java, Bun

### mise

Uses the bundled mise tool for all runtimes.

```toml
runtime_backend = "mise"
```

**Supported runtimes**: 100+ runtimes

### native-then-mise (Default)

Prefers native managers, falls back to mise for unsupported runtimes.

```toml
runtime_backend = "native-then-mise"
```

**Best of both worlds**: Fast native managers + wide mise compatibility

---

## 📁 Version File Support

OMG automatically detects version files in your project:

| File | Runtime | Format |
|------|---------|--------|
| `.nvmrc` | Node.js | `20.10.0` or `lts/*` |
| `.node-version` | Node.js | `20.10.0` |
| `.bun-version` | Bun | `1.0.25` |
| `.python-version` | Python | `3.12.0` |
| `.ruby-version` | Ruby | `3.3.0` |
| `.go-version` | Go | `1.21.0` |
| `.java-version` | Java | `21` |
| `rust-toolchain.toml` | Rust | TOML format (see below) |
| `.tool-versions` | Multi | asdf format |
| `.mise.toml` | Multi | Mise format |
| `.mise.local.toml` | Multi | Local overrides |
| `mise.toml` | Multi | Project root |
| `package.json` | Node/Bun | `engines` or `volta` field |
| `go.mod` | Go | `go 1.21` directive |

### rust-toolchain.toml Format

```toml
[toolchain]
channel = "stable"  # or "nightly", "1.75.0"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu"]
profile = "minimal"  # or "default", "complete"
```

### .tool-versions Format

```
node 20.10.0
python 3.12.0
rust stable
go 1.21.0
```

### .mise.toml Format

```toml
[tools]
node = "20.10.0"
python = "3.12.0"
rust = "stable"
deno = "1.40.0"
```

---

## 🌐 Environment Variables

OMG respects these environment variables:

| Variable | Purpose | Default |
|----------|---------|---------|
| `OMG_SOCKET_PATH` | Override socket path | XDG runtime |
| `OMG_DATA_DIR` | Override data directory | `~/.local/share/omg` |
| `OMG_CONFIG_DIR` | Override config directory | `~/.config/omg` |
| `OMG_LOG_LEVEL` | Logging level | `info` |
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
ExecStart=%h/.local/bin/omgd --foreground
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
|----------|-------------|
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
default_shell = "zsh"
runtime_backend = "native-then-mise"

[aur]
build_concurrency = 16
enable_ccache = true
cache_builds = true
```

### CI/CD Server

```toml
# ~/.config/omg/config.toml
auto_update = false
runtime_backend = "native"

[daemon]
refresh_interval = 60
cache_ttl = 60

[aur]
build_concurrency = 4
cache_builds = false
```

### Enterprise/Secure

```toml
# ~/.config/omg/config.toml
[daemon]
max_cache_entries = 5000
cache_ttl = 600
```

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
|-------|----------|
| Config not loading | Check file path and TOML syntax |
| Permission denied | Ensure socket/data dirs are writable |
| Policy blocking packages | Lower `minimum_grade` or set `allow_aur = true` |
| Runtime not found | Check `runtime_backend` setting |

### Reset to Defaults

```bash
# Remove config files
rm ~/.config/omg/config.toml
rm ~/.config/omg/policy.toml

# OMG will use defaults
omg status
```

---

## 📚 See Also

- [Security & Compliance](./security.md) — Detailed security policy documentation
- [Daemon Internals](./daemon.md) — Advanced daemon configuration
- [Runtime Management](./runtimes.md) — Version file formats
