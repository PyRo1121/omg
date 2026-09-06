# Installation Guide

Complete installation instructions for OMG across all supported platforms.

Downloaded releases require GitHub CLI (`gh`) for build-provenance verification.
If `gh` is missing, installation stops. An explicit opt-out is documented under
[Installation Options](#installation-options).

## Quick Install (Recommended)

### Linux & macOS

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

### Windows Subsystem for Linux

Run the Linux installer inside your WSL distribution:

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

Native Windows is not supported.

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
- 12-24x faster than pacman
- Full AUR support
- Daemon-based caching

---

### 🐧 Debian & Ubuntu

**Universal Installer (Recommended):**

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

**Manual Installation:**

```bash
# Download latest release
VERSION="0.1.215"
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
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

**Features:**

- Pure Rust DNF/RPM implementation
- Direct SQLite database access
- 50-100x faster package queries
- No subprocess calls

---

### 🍎 macOS

Homebrew packaging is not available yet. Use the universal installer:

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

**Supported Architectures:**

- ARM64 (Apple Silicon) - Native
- x86_64 (Intel) - Rosetta 2

**Features:**

- Homebrew integration
- Native macOS binaries
- Optimized for Apple Silicon

---

### 🪟 Windows Subsystem for Linux

Run the universal Linux installer from inside the WSL distribution:

```bash
curl -fsSL https://omg.latham.cloud/install.sh | bash
```

OMG detects and uses the package backend for the installed Linux distribution. Native Windows, PowerShell, Scoop, and Winget are not supported.

---

### 🦀 Build from Source

**Prerequisites:**

- Rust 1.93+ (`rustup`)
- Platform build tools:
  - Linux: `gcc`, `pkg-config`, `libssl-dev`
  - Debian/Ubuntu builds (`--features debian`): `libapt-pkg-dev`, `clang`, `cmake`
  - macOS: Xcode Command Line Tools

**Install via Cargo** (requires the `omg` crate on [crates.io](https://crates.io/crates/omg); if unavailable, build from git):

```bash
cargo install omg --git https://github.com/PyRo1121/omg --locked
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

To explicitly accept checksum-only verification when `gh` is missing, pass the
opt-out to the shell running the installer. This cannot bypass a failed
attestation check. Checksum and provenance refusals also stop the local-source
fallback.

```bash
curl -fsSL https://omg.latham.cloud/install.sh | OMG_INSTALL_ALLOW_UNVERIFIED_PROVENANCE=1 bash
```

The opt-out accepts only `1`, `true`, or `yes`; other values leave verification
required. Use it only when you accept the missing provenance verification.

The universal installer (`install.sh`) supports several environment variables:

```bash
# Disable telemetry (variable must reach the installer's bash, not curl)
curl -fsSL https://omg.latham.cloud/install.sh | OMG_NO_TELEMETRY=1 bash

# Skip shell integration
curl -fsSL https://omg.latham.cloud/install.sh | OMG_SKIP_SHELL=1 bash

# Install specific version
curl -fsSL https://omg.latham.cloud/install.sh | OMG_VERSION=v0.1.215 bash

# Custom install directory
curl -fsSL https://omg.latham.cloud/install.sh | INSTALL_DIR="$HOME/.omg/bin" bash

# Combine options
curl -fsSL https://omg.latham.cloud/install.sh |
  OMG_VERSION=v0.1.215 OMG_NO_TELEMETRY=1 OMG_SKIP_SHELL=1 bash
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

**Homebrew (macOS):** not packaged yet — use `omg self-update` or reinstall from [releases](https://github.com/PyRo1121/omg/releases).

**Cargo:**

```bash
cargo install omg --git https://github.com/PyRo1121/omg --locked --force
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

**Homebrew:** not packaged yet — remove the binary manually (see Linux/macOS above).

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

### Permission denied

**Linux/macOS:**

```bash
chmod +x ~/.local/bin/omg
```

### Daemon not starting

```bash
# Check daemon status
omg daemon-status

# Run the daemon in the foreground to see errors
omgd
```

If you created the optional systemd user service from
[Configuration](./configuration.md), you can also inspect it with:

```bash
journalctl --user -u omgd
```

---

## CI/CD Integration

Use OMG in CI/CD pipelines:

**GitHub Actions:**

```yaml
- name: Install OMG
  run: curl -fsSL https://omg.latham.cloud/install.sh | bash
  
- name: Use specific Node version
  run: |
    omg use node 20
    omg run build
```

**GitLab CI:**

```yaml
before_script:
  - curl -fsSL https://omg.latham.cloud/install.sh | bash
  - omg use node 20
```

**Jenkins:**

```groovy
sh 'curl -fsSL https://omg.latham.cloud/install.sh | bash'
sh 'omg use python 3.12'
```

---

## Next Steps

After installation:

1. **Read the Quick Start**: `omg help`
2. **Search packages**: `omg search <query>`
3. **Install a package**: `omg install <package>`
4. **Use runtimes**: `omg use node 20`
5. **Explore features**: Visit [GitHub docs](https://github.com/PyRo1121/omg/tree/main/docs)

---

## Support

- 📚 **Documentation**: <https://github.com/PyRo1121/omg/tree/main/docs>
- 💬 **Discussions**: <https://github.com/PyRo1121/omg/discussions>
- 🐛 **Issues**: <https://github.com/PyRo1121/omg/issues>
- 📧 **Email**: <olen@latham.cloud>

### Release verification prerequisite

Install GitHub CLI (`gh`) before running the installer. Release archives must
pass both checksum validation and attestation verification for the selected tag
and OMG release workflow. Missing or rejected provenance stops installation.
For an explicit source build, review a trusted checkout and run
`bash ./install.sh --from-source`. See [Security boundaries](../SECURITY.md#security-boundaries-and-retained-trust)
for bootstrap and upstream publisher trust limits.
