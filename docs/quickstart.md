---
title: Quick Start
sidebar_position: 2
description: Get started with OMG in 5 minutes
---

# Quick Start Guide

Get up and running with OMG in under 5 minutes.

---

## Install OMG

### One-Line Install (Recommended)

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

**Works on all platforms**: Arch Linux, Debian, Ubuntu, Fedora, macOS (ARM64), Windows (WSL).

The installer automatically detects your OS and architecture, downloads the correct pre-built binary, and installs to `~/.local/bin/`. Unknown Linux distributions fall back to the Fedora build (pure Rust, highly portable).

### Platform-Specific Packages

#### Arch Linux (AUR)

```bash
yay -S omg-bin          # Pre-built binary
# or
yay -S omg              # Build from source
```

#### macOS

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

Homebrew packaging is not available yet.

#### Debian/Ubuntu (APT)

```bash
# Coming soon - use one-line installer for now
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

#### Windows Subsystem for Linux

Run inside your WSL distribution:

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

Native Windows is not supported.

### Build from Source

```bash
git clone https://github.com/PyRo1121/omg.git
cd omg
cargo build --release
cp target/release/{omg,omgd,omg-fast} ~/.local/bin/
```

---

## Set Up Your Shell

Add the OMG hook to your shell config. This enables automatic runtime version switching when you enter project directories.

**Zsh** — Add to `~/.zshrc`:

```bash
eval "$(omg hook zsh)"
```

**Bash** — Add to `~/.bashrc`:

```bash
eval "$(omg hook bash)"
```

**Fish** — Add to `~/.config/fish/config.fish`:

```fish
omg hook fish | source
```

Then restart your shell:

```bash
exec $SHELL
```

---

## Verify It Works

```bash
omg --version    # Should print version
omg status       # Shows system overview
omg doctor       # Checks everything is configured correctly
```

---

## 🚀 Your First 5 Minutes with OMG

This walkthrough shows exactly what to expect when you run your first OMG commands, including expected outputs and common mistakes to avoid.

### Step 1: Check Installation (30 seconds)

**Command:**

```bash
omg --version
```

**Expected Output:**

```
omg 0.1.215
```

**If you see:**

- `omg: command not found` → Add `~/.local/bin` to PATH: `export PATH="$HOME/.local/bin:$PATH"`
- Different version → That's fine! Use the version you have.

**Next, check system status:**

```bash
omg status
```

**Expected Output:**

```
OMG Status
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ Daemon running (PID 12345)
✓ Cache loaded (142,345 packages)
✓ Memory: 45 MB
✓ Uptime: 2h 15m

System Information
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
OS: Arch Linux
Installed packages: 1,247
Runtimes: node@20.10.0, python@3.12.0
```

**Common Issues:**

- `Daemon not running` → It's fine! OMG works without daemon (just slower). Start it: `omgd &`
- `Cache not loaded` → Normal on first run. Will populate automatically.

---

### Step 2: Search for Packages (1 minute)

**Command:**

```bash
omg search neovim
```

**Expected Output (appears in ~5-11ms):**

```
Searching packages... (5-11ms)

extra/neovim 0.9.5-1 [Installed]
  Vim-fork focused on extensibility and agility
  
community/neovim-qt 0.2.18-1
  Qt GUI for Neovim
  
aur/neovim-nightly-bin 0.10.0-1 (124 votes)
  Neovim development build (binary)
```

**Try fuzzy search:**

```bash
omg search nvim
```

**Expected Output:**

```
Did you mean: neovim? (y/n) 
```

**Common Mistakes:**

- ❌ `omg search "neovim text editor"` → Too many words. Use: `omg search neovim`
- ❌ Exact matches only → OMG uses fuzzy matching! `nvim`, `neovim`, `neo vim` all work.

---

### Step 3: Install Your First Package (1 minute)

**Command:**

```bash
omg install ripgrep
```

**Expected Output:**

```
Resolving dependencies...
Found in: extra/ripgrep 14.1.0-1

Download: ripgrep-14.1.0-1 (1.2 MB)
[████████████████████████████] 100% (1.2 MB/s)

Installing...
✓ ripgrep 14.1.0-1 installed successfully

Security Grade: A+ ━━━━━━━━━━━━━━━ 100%
✓ No known vulnerabilities
✓ PGP signature verified
✓ Package from official repository
```

**Verify installation:**

```bash
which rg
```

**Expected Output:**

```
/usr/bin/rg
```

**Common Mistakes:**

- ❌ `omg install ripgrep bat fd` without confirmation → OMG will prompt. Use `-y` to skip: `omg install -y ripgrep bat fd`
- ❌ Package not found → Check spelling or search first: `omg search ripgrep`

---

### Step 4: Install a Runtime (2 minutes)

**Command:**

```bash
omg use node 20
```

**Expected Output:**

```
Installing Node.js 20.10.0...

Downloading: node-v20.10.0-linux-x64.tar.xz
[████████████████████████████] 100% (28.5 MB)

Verifying checksum... ✓
Extracting... ✓
Installing to ~/.local/share/omg/versions/node/20.10.0... ✓

✓ Node.js 20.10.0 installed successfully
✓ Set as active version

Active: node v20.10.0, npm v10.2.3
```

**Verify it works:**

```bash
node --version
npm --version
```

**Expected Output:**

```
v20.10.0
10.2.3
```

**Try switching versions:**

```bash
omg use node 18
```

**Expected Output (much faster - ~10ms):**

```
Installing Node.js 18.19.0...
[Same installation process]

✓ Switched to Node.js 18.19.0
```

**Common Mistakes:**

- ❌ `node: command not found` after install → Shell hook not loaded. Run: `eval "$(omg hook zsh)"` or restart shell.
- ❌ Old version still active → Check hook: `type omg` should show it's a function, not a binary.

---

### Step 5: Auto-Detect Project Versions (1 minute)

**Create a test project:**

```bash
mkdir test-project
cd test-project
echo "20.10.0" > .nvmrc
```

**Now just enter the directory:**

```bash
cd .
```

**Expected Output:**

```
✓ Detected .nvmrc → Switched to Node.js 20.10.0
```

**Verify:**

```bash
node --version
```

**Expected Output:**

```
v20.10.0
```

**Create Python project:**

```bash
echo "3.12.0" > .python-version
cd .
```

**Expected Output:**

```
✓ Detected .python-version → Switched to Python 3.12.0
```

**Common Mistakes:**

- ❌ Auto-detect not working → Shell hook not installed. Add `eval "$(omg hook zsh)"` to `~/.zshrc`
- ❌ Still using old version → File format wrong. `.nvmrc` should contain just the version: `20.10.0`, not `node 20.10.0`

---

### Step 6: Lock Your Environment (30 seconds)

**Command:**

```bash
omg env capture
```

**Expected Output:**

```
Capturing environment...

Detected:
- Node.js: 20.10.0
- Python: 3.12.0
- Packages: ripgrep 14.1.0-1

Saved to: omg.lock
```

**Check the file:**

```bash
cat omg.lock
```

**Expected Output:**

```json
{
  "version": "1.0",
  "runtimes": {
    "node": "20.10.0",
    "python": "3.12.0"
  },
  "packages": [
    {
      "name": "ripgrep",
      "version": "14.1.0-1",
      "source": "extra"
    }
  ],
  "captured_at": "2024-02-01T12:30:00Z"
}
```

**Share with team:**

```bash
git add omg.lock
git commit -m "Lock OMG environment"
```

**Teammate verifies:**

```bash
git pull
omg env check
```

**Expected Output:**

```
Checking environment against omg.lock...

✓ No drift detected — your environment matches omg.lock
```

To install the pinned runtimes and packages, teammates run `omg use <runtime> <version>`
and `omg install <package>`. To restore a full shared environment from a Gist instead,
use `omg env sync https://gist.github.com/user/abc123`.

---

## ✅ Success Checklist

After your first 5 minutes, you should have:

- [x] Installed OMG
- [x] Set up shell integration
- [x] Searched for packages (< 10ms searches!)
- [x] Installed a package with security grading
- [x] Installed Node.js 20
- [x] Auto-detected project version from `.nvmrc`
- [x] Locked your environment to `omg.lock`

**Next steps:**

- Explore `omg dash` for interactive TUI
- Try `omg run dev` in your projects
- Read [CLI Reference](./cli.md) for all commands
- Set up [Team Sync](./team.md) for collaboration

---

## 🐛 Common First-Time Mistakes

### 1. Shell Hook Not Working

**Symptom:** Runtime versions don't auto-switch when entering directories.

**Fix:**

```bash
# Check if hook is loaded
type omg  # Should show "omg is a function"

# If not, add to shell config:
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
exec $SHELL
```

### 2. PATH Issues

**Symptom:** `omg: command not found`

**Fix:**

```bash
# Add to PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
exec $SHELL
```

### 3. Daemon Not Starting

**Symptom:** Searches are slow (> 100ms)

**Fix:**

```bash
# Start daemon manually
omgd &

# Or add to shell startup:
echo 'omgd 2>/dev/null &' >> ~/.zshrc
```

### 4. Version File Format Wrong

**Symptom:** `.nvmrc` not detected

**Wrong:**

```
node 20.10.0
```

**Correct:**

```
20.10.0
```

### 5. Multiple Runtime Managers Conflicting

**Symptom:** `nvm` or `pyenv` overriding OMG versions

**Fix:**

```bash
# Remove old hooks from shell config
# Comment out or remove these lines:
# export NVM_DIR="$HOME/.nvm"
# [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"

# OMG can coexist, but shell hook order matters
# Put OMG hook AFTER other managers to take precedence
```

---

## Your First 60 Seconds with OMG

### Search for a Package

```bash
omg search neovim
```

Notice how fast that was? With the daemon running, searches return in ~5-11ms.

### Install Something

```bash
omg install neovim
```

OMG automatically detects whether a package is in the official repos or AUR and handles it appropriately.

### Switch Node.js Versions

```bash
omg use node 20
```

If Node.js 20 isn't installed, OMG downloads and installs it automatically. Then it sets it as your active version.

### Check What's Active

```bash
omg which node     # Shows active Node.js version
omg list node      # Shows all installed Node.js versions
```

### Run a Project

Navigate to any project directory and:

```bash
omg run dev
```

OMG detects your project type (package.json, Cargo.toml, Makefile, etc.) and runs the appropriate command with the correct runtime version.

---

## Common Workflows

### Managing System Packages

```bash
omg search <query>        # Find packages
omg search <query> -d     # Detailed results (votes, popularity)
omg install <packages>    # Install packages
omg remove <package>      # Remove a package
omg remove <package> -r   # Remove with unused dependencies
omg update                # Update all packages
omg update --check        # Check for updates without installing
```

### Managing Language Runtimes

```bash
# Node.js
omg use node 20           # Install and use Node.js 20
omg use node lts          # Use latest LTS version

# Python
omg use python 3.12       # Install and use Python 3.12

# Rust
omg use rust stable       # Use stable Rust
omg use rust nightly      # Use nightly Rust

# Others
omg use go 1.22           # Go
omg use ruby 3.3          # Ruby
omg use java 21           # Java
omg use bun 1.0           # Bun

# List what's installed
omg list                  # All runtimes
omg list node             # Just Node.js versions
omg list node --available # Versions available for download
```

### Sharing Your Environment

```bash
# Capture your current environment to a lockfile
omg env capture

# Check if your environment matches the lockfile
omg env check

# Share your environment (uploads to GitHub Gist)
export GITHUB_TOKEN=your_token
omg env share

# Sync someone else's environment
omg env sync https://gist.github.com/user/abc123
```

### Running Security Checks

```bash
omg audit                 # Scan for vulnerabilities
omg audit sbom            # Generate software bill of materials
omg audit secrets         # Scan for leaked credentials
```

---

## Enable the Daemon (Recommended)

The daemon keeps a package index in memory, making searches 12-24x faster. Start it with:

```bash
omg daemon
```

The daemon runs in the background. To have it start automatically, you can:

1. **Add to shell init** — The shell hook can start it automatically
2. **Use systemd** — Create a user service (see [Configuration](./configuration.md))
3. **Start manually** — Run `omg daemon` when you need it

Without the daemon, OMG still works — it just falls back to direct package manager queries.

---

## Project Setup

When you enter a project directory, OMG automatically detects version files and switches runtimes:

| File | Runtime |
| ------ | --------- |
| `.nvmrc` or `.node-version` | Node.js |
| `.python-version` | Python |
| `.ruby-version` | Ruby |
| `.go-version` | Go |
| `.java-version` | Java |
| `rust-toolchain.toml` | Rust |
| `.tool-versions` | Multiple runtimes (asdf format) |

Create a version file in your project:

```bash
echo "20.10.0" > .nvmrc
```

Now whenever you `cd` into this directory, OMG automatically switches to Node.js 20.10.0.

---

## Interactive Dashboard

Launch the full-screen dashboard:

```bash
omg dash
```

Navigate with:

- `Tab` — Switch between views
- `r` — Refresh
- `q` — Quit

The dashboard shows packages, runtimes, security alerts, and system activity in real time.

---

## Getting Help

```bash
omg --help              # General help
omg <command> --help    # Help for specific command
omg doctor              # Diagnose issues
```

---

## What's Next?

Now that you're set up:

- **[CLI Reference](./cli.md)** — Every command explained in detail
- **[Runtime Management](./runtimes.md)** — Deep dive into managing language runtimes
- **[Team Collaboration](./team.md)** — Share environments with your team
- **[Security](./security.md)** — Vulnerability scanning and compliance

---

## Quick Reference

| Task | Command |
| ------ | --------- |
| Search packages | `omg search <query>` |
| Install package | `omg install <package>` |
| Remove package | `omg remove <package>` |
| Update all | `omg update` |
| Use runtime | `omg use <runtime> <version>` |
| List runtimes | `omg list` |
| Run task | `omg run <task>` |
| System status | `omg status` |
| Security scan | `omg audit` |
| Dashboard | `omg dash` |

---

**Having trouble?** Run `omg doctor` or check the [Troubleshooting Guide](./troubleshooting.md).
