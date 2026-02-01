# Installation Guide

Complete installation instructions for OMG across all supported platforms.

## Quick Install (Recommended)

### Linux & macOS

```bash
curl -fsSL https://pyro1121.com/install.sh | bash
```

### Windows

```powershell
irm https://pyro1121.com/install.ps1 | iex
```

---

## Platform-Specific Installation

### 🐧 Arch Linux

OMG is available in the AUR (Arch User Repository):

**Prebuilt Binary (Recommended):**
```bash
yay -S omg-bin
```

**Build from Source:**
```bash
yay -S omg
```

**Features:**
- Native `libalpm` integration
- 22x faster than pacman
- Full AUR support
- Daemon-based caching

---

### 🐧 Debian & Ubuntu

**Universal Installer (Recommended):**
```bash
curl -fsSL https://pyro1121.com/install.sh | bash
```

**Manual Installation:**
```bash
# Download latest release
VERSION="0.1.204"
wget "https://github.com/PyRo1121/omg/releases/download/v${VERSION}/omg-v${VERSION}-x86_64-linux-debian.tar.gz"

# Extract
tar -xzf omg-v${VERSION}-x86_64-linux-debian.tar.gz
cd omg-v${VERSION}-x86_64-linux-debian

# Install
sudo cp omg /usr/local/bin/
sudo chmod +x /usr/local/bin/omg

# Verify
omg --version
```

**Features:**
- Native `rust-apt` integration
- 59-483x faster than apt-cache/Nala
- Direct APT database access
- Zero subprocess overhead

---

### 🎩 Fedora & RHEL

**Universal Installer (Recommended):**
```bash
curl -fsSL https://pyro1121.com/install.sh | bash
```

**Features:**
- Pure Rust DNF/RPM implementation
- Direct SQLite database access
- 50-100x faster package queries
- No subprocess calls

---

### 🍎 macOS

**Homebrew (Coming Soon):**
```bash
brew tap pyro1121/omg
brew install omg
```

**Universal Installer:**
```bash
curl -fsSL https://pyro1121.com/install.sh | bash
```

**Supported Architectures:**
- ARM64 (Apple Silicon) - Native
- x86_64 (Intel) - Rosetta 2

**Features:**
- Homebrew integration
- Native macOS binaries
- Optimized for Apple Silicon

---

### 🪟 Windows

OMG provides **three installation methods** for Windows:

#### Option 1: PowerShell Installer (Recommended)

**One-line install:**
```powershell
irm https://pyro1121.com/install.ps1 | iex
```

**With options:**
```powershell
# Disable telemetry
irm https://pyro1121.com/install.ps1 | iex -NoTelemetry

# Skip shell integration
irm https://pyro1121.com/install.ps1 | iex -SkipShell

# Install to custom directory
irm https://pyro1121.com/install.ps1 | iex -InstallDir "C:\Tools\omg"
```

#### Option 2: Scoop Package Manager

```powershell
# Add OMG bucket
scoop bucket add omg https://github.com/PyRo1121/scoop-omg

# Install OMG
scoop install omg

# Update OMG
scoop update omg
```

#### Option 3: Windows Subsystem for Linux (WSL)

```powershell
wsl -- curl -fsSL https://pyro1121.com/install.sh | bash
```

**Features:**
- Pure Rust Scoop integration via libscoop
- 35-73x faster than traditional Scoop
- Zero subprocess calls
- Native Windows binaries

---

### 🦀 Build from Source

**Prerequisites:**
- Rust 1.92+ (`rustup`)
- Platform build tools:
  - Linux: `gcc`, `pkg-config`, `libssl-dev`
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio Build Tools

**Install via Cargo:**
```bash
cargo install omg-cli
```

**Build manually:**
```bash
# Clone repository
git clone https://github.com/PyRo1121/omg.git
cd omg

# Build release binary
cargo build --release

# Install
sudo cp target/release/omg /usr/local/bin/
```

**Platform-specific features:**
```bash
# Arch Linux (with libalpm)
cargo build --release --features arch

# Debian/Ubuntu (with rust-apt)
cargo build --release --features debian

# Fedora/RHEL (pure Rust)
cargo build --release --features fedora

# macOS (Homebrew integration)
cargo build --release --features macos

# Windows (Scoop integration)
cargo build --release --features windows
```

---

## Post-Installation Setup

### 1. Shell Integration

Enable instant version switching for Node.js, Python, etc.

**Bash:**
```bash
echo 'eval "$(omg hook bash)"' >> ~/.bashrc
source ~/.bashrc
```

**Zsh:**
```bash
echo 'eval "$(omg hook zsh)"' >> ~/.zshrc
source ~/.zshrc
```

**Fish:**
```fish
echo 'omg hook fish | source' >> ~/.config/fish/config.fish
source ~/.config/fish/config.fish
```

**PowerShell (Windows):**
```powershell
echo 'Invoke-Expression (& omg hook powershell)' >> $PROFILE
. $PROFILE
```

### 2. Verify Installation

```bash
# Check version
omg --version

# Run diagnostics
omg doctor

# Test search
omg search vim
```

### 3. Optional: Enable Shell Completions

**Bash:**
```bash
omg completions bash > ~/.local/share/bash-completion/completions/omg
```

**Zsh:**
```bash
omg completions zsh > ~/.zfunc/_omg
```

**Fish:**
```bash
omg completions fish > ~/.config/fish/completions/omg.fish
```

**PowerShell:**
```powershell
omg completions powershell > $PROFILE\..\omg-completion.ps1
```

---

## Installation Options

The universal installer (`install.sh`) supports several environment variables:

```bash
# Disable telemetry
OMG_NO_TELEMETRY=1 curl -fsSL https://pyro1121.com/install.sh | bash

# Skip shell integration
OMG_SKIP_SHELL=1 curl -fsSL https://pyro1121.com/install.sh | bash

# Install specific version
OMG_VERSION=v0.1.204 curl -fsSL https://pyro1121.com/install.sh | bash

# Custom install directory
INSTALL_DIR=~/.omg/bin curl -fsSL https://pyro1121.com/install.sh | bash

# Combine options
OMG_NO_TELEMETRY=1 OMG_SKIP_SHELL=1 OMG_VERSION=v0.1.204 \
  curl -fsSL https://pyro1121.com/install.sh | bash
```

---

## Updating OMG

### Auto-update (Recommended)

```bash
omg self-update
```

### Platform-specific updates

**Arch (AUR):**
```bash
yay -Syu omg-bin
```

**Scoop (Windows):**
```powershell
scoop update omg
```

**Homebrew (macOS):**
```bash
brew upgrade omg
```

**Cargo:**
```bash
cargo install omg-cli --force
```

---

## Uninstallation

### Linux/macOS

**Universal installer:**
```bash
rm -f ~/.local/bin/omg
rm -rf ~/.local/share/omg
rm -rf ~/.config/omg
```

**AUR:**
```bash
yay -R omg-bin
```

**Homebrew:**
```bash
brew uninstall omg
```

### Windows

**Scoop:**
```powershell
scoop uninstall omg
```

**Manual (PowerShell installer):**
```powershell
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\Programs\omg"
Remove-Item -Recurse -Force "$env:APPDATA\omg"
```

---

## Troubleshooting

### Command not found

**Ensure install directory is in PATH:**

```bash
# Linux/macOS
echo $PATH | grep -o "$HOME/.local/bin"

# Add to PATH if missing
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

**Windows:**
```powershell
$env:Path -split ';' | Select-String "omg"
```

### Permission denied

**Linux/macOS:**
```bash
chmod +x ~/.local/bin/omg
```

### Daemon not starting

```bash
# Check daemon status
omg daemon status

# Restart daemon
omg daemon restart

# View logs
journalctl --user -u omgd
```

---

## CI/CD Integration

Use OMG in CI/CD pipelines:

**GitHub Actions:**
```yaml
- name: Install OMG
  run: curl -fsSL https://pyro1121.com/install.sh | bash
  
- name: Use specific Node version
  run: |
    omg use node 20
    omg run build
```

**GitLab CI:**
```yaml
before_script:
  - curl -fsSL https://pyro1121.com/install.sh | bash
  - omg use node 20
```

**Jenkins:**
```groovy
sh 'curl -fsSL https://pyro1121.com/install.sh | bash'
sh 'omg use python 3.12'
```

---

## Next Steps

After installation:

1. **Read the Quick Start**: `omg help`
2. **Search packages**: `omg search <query>`
3. **Install a package**: `omg install <package>`
4. **Use runtimes**: `omg use node 20`
5. **Explore features**: Visit [pyro1121.com/docs](https://pyro1121.com/docs)

---

## Support

- 📚 **Documentation**: https://pyro1121.com/docs
- 💬 **Discussions**: https://github.com/PyRo1121/omg/discussions
- 🐛 **Issues**: https://github.com/PyRo1121/omg/issues
- 📧 **Email**: olen@latham.cloud
