---
title: Runtime Management
sidebar_position: 11
description: Managing Node.js, Python, Go, Rust, Ruby, Java, and Bun
---

# Runtime Management

OMG provides a unified, high-performance interface for managing multiple programming language runtimes. It is designed to be a faster, more reliable alternative to traditional managers like `nvm`, `pyenv`, or `rustup`, offering sub-millisecond version switching and zero-configuration setups.

## 🚀 Supported Runtimes

### Native Runtimes
OMG features native, pure Rust implementations for the most popular language ecosystems. These implementations are optimized for speed and require no external dependencies.

| Runtime | Auto-detect File | Install Command | Switch Command | Binaries |
|---------|------------------|-----------------|----------------|----------|
| **Node.js** | `.nvmrc`, `.node-version`, `package.json#engines` | `omg use node 20` | `omg use node 18` | `node`, `npm`, `npx` |
| **Python** | `.python-version`, `pyproject.toml`, `runtime.txt` | `omg use python 3.12` | `omg use python 3.11` | `python3`, `pip` |
| **Go** | `.go-version`, `go.mod` | `omg use go 1.21` | `omg use go 1.20` | `go` |
| **Rust** | `rust-toolchain.toml`, `rust-toolchain`, `.rust-version` | `omg use rust stable` | `omg use rust nightly` | `rustc`, `cargo` |
| **Ruby** | `.ruby-version`, `Gemfile` | `omg use ruby 3.2` | `omg use ruby 3.1` | `ruby`, `gem` |
| **Java** | `.java-version`, `pom.xml` | `omg use java 21` | `omg use java 17` | `java`, `javac` |
| **Bun** | `.bun-version` | `omg use bun latest` | `omg use bun 1.0` | `bun`, `bunx` |

### Extended Universe (Built-in Mise)
For everything else, OMG includes a **built-in mise manager**. If you request a runtime that isn't natively handled, OMG automatically leverages the `mise` ecosystem to provide support for over **100 additional languages and tools**, including Deno, Elixir, Zig, PHP, and more.

```bash
# Install any tool via mise
omg tool install bat
omg tool install ripgrep
omg tool install terraform

# List available tools
omg tool list --available

# Update all tools
omg tool update
```

---

## 📚 Quick Examples

### Node.js

```bash
# Install Node.js 20
omg use node 20

# Auto-detect from .nvmrc
echo "20.10.0" > .nvmrc
cd .  # Shell hook auto-switches

# List installed versions
omg list node

# Show current version
omg which node

# Install specific version
omg use node 20.10.0

# Use LTS version
omg use node lts
```

### Python

```bash
# Install Python 3.12
omg use python 3.12

# Auto-detect from .python-version
echo "3.12.1" > .python-version
cd .  # Auto-switch

# Use in virtual environment
omg use python 3.12
python -m venv .venv
source .venv/bin/activate

# Install specific patch version
omg use python 3.12.1
```

### Go

```bash
# Install Go 1.21
omg use go 1.21

# Auto-detect from go.mod
echo "module myapp" > go.mod
echo "go 1.21" >> go.mod
cd .  # Auto-switch

# List available versions
omg list go --available
```

### Rust

```bash
# Install Rust stable
omg use rust stable

# Use nightly for a project
echo "nightly" > rust-toolchain
cd .  # Auto-switch to nightly

# Use specific version
omg use rust 1.75.0

# Install toolchain with components
omg use rust stable --components clippy,rustfmt
```

### Multiple Runtimes (Typical Project)

```bash
# Install all runtimes for a full-stack project
omg use node 20
omg use python 3.12
omg use rust stable

# Captured in omg.lock for team sync
omg env capture

# Team member syncs instantly
omg env sync
```

---

## 🛠️ How Runtime Switching Works

### 1. Shell Hook Detects Directory Change

```bash
cd /my/project  # Shell hook runs on directory change
```

### 2. OMG Reads Version Files

OMG scans for version files in priority order:

**Node.js Priority:**
1. `.nvmrc`
2. `package.json#engines.node`
3. `.node-version`

**Python Priority:**
1. `.python-version`
2. `pyproject.toml`
3. `runtime.txt`

**Rust Priority:**
1. `rust-toolchain.toml`
2. `rust-toolchain`
3. `.rust-version`

### 3. Updates PATH Instantly

```bash
# Before: /usr/bin/node → 16.0.0
# After:  ~/.local/share/omg/versions/node/20.10.0/bin/node
```

**No subprocess overhead** - Direct PATH manipulation in your shell.

### 4. Works in Subshells

```bash
# Version persists in subshells
(node --version)  # Uses correct version

# Tmux/screen sessions inherit version
tmux new-session "node server.js"
```

---

## 🎯 Auto-Detection Priority

When multiple version files exist in the same directory:

### Node.js
1. `.nvmrc` (highest priority)
2. `package.json#engines.node`
3. `.node-version`

### Python
1. `.python-version` (highest priority)
2. `pyproject.toml`
3. `runtime.txt`

### Override Auto-Detection

```bash
# Force specific version (ignores version files)
omg use node 18 --force

# Show which version file is active
omg which node --verbose
```

---

## 🔌 mise Integration

OMG bundles mise for 100+ additional runtimes and tools that aren't natively implemented.

```bash
# Install development tools
omg tool install bat
omg tool install ripgrep
omg tool install fd
omg tool install terraform
omg tool install elixir

# List all available tools
omg tool list --available

# Update all installed tools
omg tool update

# Remove a tool
omg tool uninstall bat
```

**Supported via mise:**
- Deno, Elixir, Zig, PHP, Lua
- Terraform, kubectl, helm
- And 90+ more languages/tools

See [Tool Runner docs](task-runner.md) for details.

---

## 🚀 Migration from Other Tools

### From nvm (Node Version Manager)

```bash
# Automatic migration
omg migrate from-nvm

# What gets imported:
# - All installed Node.js versions
# - Current/default version
# - Global npm packages

# Manual migration
nvm list  # Note your versions
omg use node 20
omg use node 18
npm install -g $(npm list -g --depth=0 --json | jq -r '.dependencies | keys[]')
```

### From pyenv (Python Version Manager)

```bash
# Automatic migration
omg migrate from-pyenv

# What gets imported:
# - All installed Python versions
# - Global/system version
# - Virtual environments (symlinked)

# Manual migration
pyenv versions  # Note your versions
omg use python 3.12
omg use python 3.11
```

### From rustup (Rust Toolchain Manager)

```bash
# Automatic migration
omg migrate from-rustup

# What gets imported:
# - Installed toolchains (stable, nightly, beta)
# - Default toolchain
# - Installed components (clippy, rustfmt, etc.)

# Manual migration
rustup show  # Note your toolchains
omg use rust stable
omg use rust nightly
```

**Migration is non-destructive** - Your existing tools remain installed until you remove them.

---

## 📊 Performance Comparison

| Operation | OMG | nvm | pyenv | rustup |
|-----------|-----|-----|-------|--------|
| **Version switch** | <10ms | 100-200ms | 150-300ms | 50-100ms |
| **Auto-detect** | <5ms | 50-100ms | 100-200ms | 30-60ms |
| **Install** | 10-60s | Similar | Similar | Similar |
| **Shell startup** | <1ms | 20-50ms | 30-70ms | 10-20ms |

**Why is OMG so fast?**

1. **Direct PATH manipulation** - No subprocess overhead
2. **Daemon caches version files** - Zero filesystem traversal on repeat
3. **Zero shell overhead** - Pure Rust implementation
4. **Atomic symlink updates** - Single syscall for version switch
5. **Pre-compiled binaries** - No runtime compilation

---

## 🔒 Security and Integrity

Safety is a first-class citizen in OMG's runtime management:

### Cryptographic Verification

**Every download is verified:**
- **Node.js**: SHA256 checksums from `SHASUMS256.txt`
- **Python**: GPG signatures from python.org
- **Rust**: SHA256 from rust-lang.org manifests
- **Go**: Official binary checksums
- **Java**: Adoptium checksums

### Secure Transport

- All downloads over **HTTPS** with certificate validation
- TLS 1.3 preferred
- Certificate pinning for critical sources

### Sandboxed Installations

All runtimes installed in user-local directory:
```
~/.local/share/omg/versions/
├── node/
│   ├── 20.10.0/
│   └── 18.19.0/
├── python/
│   ├── 3.12.1/
│   └── 3.11.7/
└── rust/
    ├── stable/
    └── nightly/
```

**Benefits:**
- **No Sudo Required** - Never need administrative privileges
- **Isolation** - Versions cannot interfere with each other
- **Easy Cleanup** - `rm -rf ~/.local/share/omg/versions/node/20.10.0`

### Isolated Build Paths

For runtimes requiring compilation (Python, Ruby):
- Builds happen in temporary directories
- No environment pollution
- Clean failure recovery

---

## 🛠️ Troubleshooting

### Version Not Switching

**Symptom:** Running `omg use node 20` but `node --version` shows old version.

**Check shell hook:**
```bash
type omg  # Should show it's a function, not a binary
```

**Fix: Re-source shell config:**
```bash
exec $SHELL  # Restart shell
# OR
source ~/.zshrc  # For zsh
source ~/.bashrc  # For bash
```

---

### Auto-Detect Not Working

**Symptom:** `.nvmrc` exists but version doesn't auto-switch.

**Check version file:**
```bash
cat .nvmrc  # Should contain version like "20.10.0" or "20"
```

**Force specific version:**
```bash
omg use node 20 --force
```

**Verify shell hook is installed:**
```bash
grep "omg hook" ~/.zshrc  # Should exist
```

---

### Installation Failed

**Symptom:** `omg use node 20` fails with network error.

**Check network connectivity:**
```bash
curl -I https://nodejs.org/dist/
```

**Try different mirror:**
```bash
omg config set node.mirror "https://npmmirror.com/mirrors/node"
```

**Check disk space:**
```bash
df -h ~/.local/share/omg
```

---

### Binary Not Found After Install

**Symptom:** `node: command not found` after `omg use node 20`.

**Check PATH:**
```bash
echo $PATH | grep omg  # Should contain omg shims directory
```

**Verify installation:**
```bash
omg which node
ls -la ~/.local/share/omg/versions/node/20.*/bin/node
```

**Fix: Restart shell or re-source config:**
```bash
exec $SHELL
```

---

### Conflicting Global Packages

**Symptom:** Global packages installed with `npm install -g` disappear after version switch.

**Explanation:** Global packages are version-specific in OMG (by design for isolation).

**Solution 1: Install per version:**
```bash
omg use node 20
npm install -g typescript

omg use node 18
npm install -g typescript  # Separate install for Node 18
```

**Solution 2: Use project-local packages:**
```bash
npm install --save-dev typescript  # Better practice
npx tsc  # Run without global install
```

---

## 🔗 See Also

- [Shell Integration](shell-integration.md) - Shell hook setup and configuration
- [Configuration](configuration.md) - Runtime-specific settings
- [Team Sync](team.md) - Lock files and environment sharing
- [Security](security.md) - Verification and integrity checks
- [Troubleshooting](troubleshooting.md) - Common issues and solutions
