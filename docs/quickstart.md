# Quick Start Guide

**Get up and running with OMG in 5 minutes**

This guide will walk you through installing OMG, setting up shell integration, and running your first commands.

---

## 📋 Prerequisites

Before installing OMG, ensure you have:

| Requirement | Details |
|-------------|---------|
| **Operating System** | Arch Linux, Manjaro, EndeavourOS, or Debian/Ubuntu 22.04+ |
| **Rust** | 1.92+ (for building from source) |
| **Base packages** | `git`, `curl`, `tar`, `sudo` |
| **For Arch** | Access to libalpm (comes with pacman) |
| **For Debian** | `libapt-pkg-dev` (for building with debian feature) |

---

## 🚀 Installation

### Option 1: One-Line Installer (Recommended)

The easiest way to install OMG:

```bash
curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | bash
```

**To disable telemetry** (anonymous usage data):

```bash
curl -fsSL https://raw.githubusercontent.com/PyRo1121/omg/main/install.sh | OMG_NO_TELEMETRY=1 bash
```

This script will:
1. Download or build the release binary
2. Install to `~/.local/bin/`
3. Ask about telemetry preferences
4. Configure shell integration
5. Install shell completions

### Option 2: Build from Source

For complete control over the build:

```bash
# Clone the repository
git clone https://github.com/PyRo1121/omg.git
cd omg

# Build release binary (with Arch Linux support)
cargo build --release

# Build with Debian/Ubuntu support (requires libapt-pkg-dev)
# cargo build --release --features debian

# Install binaries
cp target/release/omg ~/.local/bin/
cp target/release/omgd ~/.local/bin/
cp target/release/omg-fast ~/.local/bin/

# Ensure ~/.local/bin is in your PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

### Option 3: AUR (Arch Linux)

```bash
# Using yay
yay -S omg-bin

# Or using paru
paru -S omg-bin
```

---

## 🔧 Initial Setup

### 1. Start the Daemon

The daemon provides ultra-fast cached responses:

```bash
# Start in background (default)
omg daemon

# Or run in foreground for debugging
omgd --foreground
```

> **Tip**: Add `omg daemon` to your system startup for persistent caching.

### 2. Configure Shell Integration

Add the shell hook to your shell configuration:

**Zsh** (`~/.zshrc`):
```bash
eval "$(omg hook zsh)"
```

**Bash** (`~/.bashrc`):
```bash
eval "$(omg hook bash)"
```

**Fish** (`~/.config/fish/config.fish`):
```fish
omg hook fish | source
```

Restart your shell or source the config:
```bash
source ~/.zshrc  # or ~/.bashrc
```

### 3. Install Completions

Install shell completions for an enhanced experience:

```bash
# For Zsh
omg completions zsh > ~/.zsh/completions/_omg

# For Bash
omg completions bash > /etc/bash_completion.d/omg

# For Fish
omg completions fish > ~/.config/fish/completions/omg.fish
```

### 4. Verify Installation

```bash
# Check OMG version
omg --version

# Check system status
omg status

# Run diagnostics
omg doctor
```

---

## 🎯 Your First Commands

### Package Management

```bash
# Search for packages (official repos + AUR)
omg search vim

# Interactive search (select packages to install)
omg search vim -i

# Install a package
omg install neovim

# Get package info
omg info firefox

# Update all packages
omg update

# List explicitly installed packages
omg explicit

# Remove a package
omg remove package-name

# Remove with dependencies
omg remove package-name -r
```

### Runtime Management

```bash
# List available Node.js versions
omg list node --available

# Install and use Node.js 20
omg use node 20.10.0

# Check which version is active
omg which node

# Install Python 3.12
omg use python 3.12.0

# Use Rust stable
omg use rust stable

# Install Deno (via built-in mise)
omg use deno 1.40.0
```

### Task Runner

```bash
# Run project tasks (auto-detects package.json, Cargo.toml, etc.)
omg run dev

# Run with arguments
omg run test -- --watch

# List available tasks
omg run --list
```

### System Health

```bash
# Full system status
omg status

# Run diagnostics
omg doctor

# Security audit
omg audit

# View usage statistics
omg stats
```

---

## 🖥️ Interactive Dashboard

Launch the real-time TUI dashboard:

```bash
omg dash
```

| Key | Action |
|-----|--------|
| `q` | Quit |
| `r` | Refresh |
| `Tab` | Switch views |

---

## 📁 Directory Structure

After installation, OMG uses these directories:

| Path | Purpose |
|------|---------|
| `~/.local/bin/omg` | Main binary |
| `~/.local/bin/omgd` | Daemon binary |
| `~/.local/share/omg/` | Data directory (runtimes, cache) |
| `~/.local/share/omg/versions/` | Installed runtime versions |
| `~/.config/omg/config.toml` | Configuration file |
| `~/.config/omg/policy.toml` | Security policy |
| `$XDG_RUNTIME_DIR/omg.sock` | Daemon socket |

---

## ⚡ Performance Tips

### Enable the Daemon

The daemon provides 10-100x faster responses for repeated queries:

```bash
# Start daemon (runs in background)
omg daemon

# Add to systemd for persistence
# See configuration docs for systemd service file
```

### Use Ultra-Fast Queries

For shell prompts and scripts, use the ultra-fast binary:

```bash
# Get package counts in sub-millisecond
omg-fast ec    # Explicit count
omg-fast tc    # Total count
omg-fast uc    # Updates count
omg-fast oc    # Orphan count
```

### Shell Prompt Integration

Add package counts to your prompt (sub-microsecond with caching):

```bash
# In your shell prompt (Zsh)
PROMPT='$(omg-ec) packages %~$ '
```

---

## 🔐 Security Quick Start

### Run a Security Audit

```bash
# Scan for vulnerabilities
omg audit

# Generate SBOM
omg audit sbom

# Check for leaked secrets
omg audit secrets

# View audit log
omg audit log
```

### Configure Security Policy

Create `~/.config/omg/policy.toml`:

```toml
minimum_grade = "Verified"
allow_aur = true
require_pgp = false
banned_packages = []
```

---

## 🤝 Team Collaboration

### Share Your Environment

```bash
# Capture current environment to omg.lock
omg env capture

# Share via GitHub Gist
export GITHUB_TOKEN=your_token
omg env share

# Teammate syncs your environment
omg env sync https://gist.github.com/...
```

### Check for Drift

```bash
omg env check
```

---

## 🐛 Troubleshooting

### Daemon Won't Start

```bash
# Check if socket exists
ls -la $XDG_RUNTIME_DIR/omg.sock

# Remove stale socket
rm $XDG_RUNTIME_DIR/omg.sock

# Start in foreground to see errors
omgd --foreground
```

### Shell Hook Not Working

```bash
# Verify hook is installed
grep "omg hook" ~/.zshrc

# Test hook output
omg hook zsh

# Restart shell
exec zsh
```

### Slow Performance

```bash
# Ensure daemon is running
omg status

# Check cache health
omg doctor

# Clear and rebuild cache
omg clean
```

---

## 📚 Next Steps

Now that you're set up, explore these guides:

1. **[CLI Reference](./cli.md)** — Complete command documentation
2. **[Runtime Management](./runtimes.md)** — Multi-language environment setup
3. **[Security & Compliance](./security.md)** — Enterprise security features
4. **[Configuration](./configuration.md)** — Customize OMG for your workflow

---

**Need Help?** Run `omg doctor` for diagnostics or visit the [Troubleshooting Guide](./troubleshooting.md).
