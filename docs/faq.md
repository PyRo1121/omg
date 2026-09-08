---
title: FAQ
sidebar_position: 51
description: Frequently asked questions
---

# Frequently Asked Questions

**Common Questions About OMG**

---

## 🚀 Getting Started

### What is OMG?

OMG (Oh My God!) is a unified package manager that combines:
- **System packages** (Arch Linux, Debian/Ubuntu)
- **Language runtimes** (Node.js, Python, Go, Rust, Ruby, Java, Bun)
- **AUR support** (Arch Linux)
- **Security auditing** (vulnerability scanning, SBOM)

All in a single Rust binary.

### Why is it called OMG?

The name expands to "Oh My God!". Performance claims belong in the [benchmark evidence](../benchmarks/README.md), with their measured scope.

### What platforms are supported?

| Platform | Status | Notes |
|----------|--------|-------|
| Arch Linux | Alpha | ALPM and AUR backend |
| Manjaro / EndeavourOS | Arch-derived | Validate native dependency compatibility; no blanket parity guarantee |
| Debian/Ubuntu | Alpha | Native APT backend, different security coverage |
| Fedora | Alpha | DNF backend; recorded v0.1.218 smoke failures |
| macOS | Alpha | ARM64 release, Homebrew backend |
| WSL | ✅ Supported | Uses the installed Linux distribution backend |
| Native Windows | ❌ Unsupported | Use WSL |

### How do I install OMG?

```bash
# One-liner
curl -fsSL https://omg.latham.cloud/install.sh | bash

# Or build from source
git clone https://github.com/PyRo1121/omg.git
cd omg && cargo build --release
cp target/release/omg ~/.local/bin/
```

---

## 🔒 Privacy & Telemetry

### Does OMG collect any data?

Installer telemetry requires consent and defaults to no. Runtime telemetry is opt-in. When enabled, it collects canonical command names, timings, success status, backend, session data, and a hashed machine identifier. See [privacy and telemetry](./security.md#privacy-and-telemetry) for scope and opt-out controls.

### How do I opt out of telemetry?

**During installation:**
```bash
OMG_NO_TELEMETRY=1 bash omg-install.sh
```

**After installation:**
```bash
# Add to your shell config
export OMG_TELEMETRY=0
```

### Where is data sent?

Telemetry uses the OMG service. Package backends, download providers, advisory APIs, and dashboard features also contact their respective services. Disabling telemetry does not disable those functional requests. Review the [telemetry client](../src/core/telemetry_client.rs) rather than assuming all outgoing data is telemetry.

---

## ⚡ Performance

### How is OMG so fast?

The daemon caches package indexes in memory, and some backends use direct library or database reads. Other paths invoke native tools or contact network services. The binary IPC protocol does not eliminate those costs.

### What are the actual performance numbers?

See [raw benchmark records and methodology](../benchmarks/README.md). Measurements depend on the artifact, host, backend, cache state, query, and enabled sources. They do not establish universal speedups.

### Does OMG need the daemon to be fast?

Most package queries have direct fallback paths. The daemon can reduce repeated query work. Vulnerability scans, metrics, and Unix SOC 2 export require it. See [daemon requirements](./daemon.md).

---

## 📦 Package Management

### Does OMG replace pacman?

No. OMG uses pacman/libalpm under the hood. It's a faster interface, not a replacement.

### Does OMG replace yay/paru?

OMG includes AUR workflows, but does not promise every yay or paru option. Keep your existing tools until you have verified the operations you depend on.

### Can I use OMG and yay together?

Yes, they can coexist. They both use the same pacman databases.

### How does AUR building work?

OMG handles AUR builds with:
- Parallel builds (configurable concurrency)
- ccache/sccache support
- Build caching
- PGP verification

Configure in `~/.config/omg/config.toml`:
```toml
[aur]
build_concurrency = 8
enable_ccache = true
```

---

## 🔧 Runtime Management

### What runtimes are supported?

OMG provides managers for:
- Node.js
- Python
- Go
- Rust
- Ruby
- Java
- Bun
- Pi coding agent

Unsupported runtime names fail explicitly; OMG does not download a fallback runtime manager.

### Does OMG replace nvm/pyenv/rustup?

OMG selects supported runtimes, but providers may delegate to tools such as rustup or ruby-build. It does not replace every provider feature. Avoid conflicting shell hooks.

### How does version detection work?

OMG checks for version files when you change directories:
- `.nvmrc`, `.node-version` (Node.js)
- `.python-version` (Python)
- `rust-toolchain.toml` (Rust)
- `.tool-versions` (Multiple)

The shell hook automatically updates PATH.

### What happens for unsupported runtimes?

OMG rejects unsupported names with a list of native runtimes. It never downloads or invokes another runtime manager as an implicit fallback.

---

## 🛡️ Security

### What security features does OMG have?

- **Vulnerability scanning** (ALSA + OSV.dev)
- **SBOM generation** (CycloneDX 1.5)
- **PGP verification** (Sequoia-OpenPGP)
- **Artifact signatures** through supported Rekor entries, with an exact expected identity; no SLSA build-level verification
- **Secret scanning** (20+ credential patterns)
- **Audit logging** (hash-chained; user-owned logs are not authenticated)
- **Policy enforcement** (grade-based blocking)

### What are security grades?

| Grade | Meaning |
|-------|---------|
| LOCKED | Policy enum value; not conferred by core package names or the current SLSA verifier |
| VERIFIED | Official repository source classification, not an independent verification receipt |
| COMMUNITY | AUR packages |
| RISK | Packages with known CVEs |

### Is OMG safe to use?

OMG is alpha software and executes package or project code in several workflows. Use a recoverable machine, review community packages, and retain your native package tools. Signatures and local audit chains do not prove safety. Read [the security boundaries](../SECURITY.md#security-boundaries-and-retained-trust) before relying on them.

---

## 🐚 Shell Integration

### Which shells are supported?

- **Zsh** (recommended)
- **Bash**
- **Fish**

### Why do I need a shell hook?

The hook:
- Updates PATH when you change directories
- Detects version files automatically
- Provides fast package count functions for prompts

### Will the hook slow down my shell?

No. The hook is highly optimized:
- Sub-millisecond execution
- Uses cached status from daemon
- Minimal work on each prompt

---

## 👥 Team Features

### How do I share my environment with teammates?

```bash
# Capture environment
omg env capture

# Share via Gist
export GITHUB_TOKEN=your_token
omg env share

# Teammate syncs
omg env sync https://gist.github.com/...
```

### What is omg.lock?

It's an environment lockfile containing:
- Runtime versions
- Explicit packages
- Environment fingerprint

Commit it to version control for reproducible environments.

### How does drift detection work?

`omg env check` compares your local environment against `omg.lock` and reports differences.

---

## 🔄 History & Rollback

### Does OMG track what I install?

Yes. All transactions (install/remove/update) are logged to `~/.local/share/omg/history.json`.

### Can I undo an installation?

Yes:
```bash
# Interactive rollback
omg rollback

# Or specify transaction ID
omg rollback abc123
```

### What are the rollback limitations?

- Official packages only (AUR rollback planned)
- Requires old package versions in cache
- May need manual dependency resolution
- `HoldPkg` and `IgnorePkg` entries in `pacman.conf` are enforced: held packages cannot be removed and ignored packages are excluded from updates

---

## 🖥️ TUI Dashboard

### What is `omg dash`?

An interactive terminal dashboard showing:
- Package counts
- Update status
- Active runtimes
- CVE counts
- Recent activity

### What are the keyboard controls?

| Key | Action |
|-----|--------|
| `q` | Quit |
| `r` | Refresh |
| `Tab` | Switch views |

---

## 🐳 Containers

### Does OMG support Docker?

Yes. OMG provides container commands:
```bash
omg container shell  # Dev shell
omg container build  # Build image
omg container init   # Generate Dockerfile
```

### Does OMG prefer Docker or Podman?

OMG prefers Podman for rootless security, but supports both.

---

## 🔧 Troubleshooting

### OMG is slow

```bash
# Ensure daemon is running
omg daemon

# Check status
omg status
```

### "Daemon not running"

```bash
# Start daemon
omg daemon

# If socket exists but daemon is dead
rm $XDG_RUNTIME_DIR/omg.sock
omg daemon
```

### Shell hook not working

```bash
# Verify installation
grep "omg hook" ~/.zshrc

# Restart shell completely
exec zsh
```

### See the [Troubleshooting Guide](./troubleshooting.md) for more.

---

## License

OMG is free and open source under the [MIT License](../LICENSE).

---

## 🤝 Contributing

### How can I contribute?

- Report bugs on GitHub Issues
- Submit PRs for features/fixes
- Improve documentation
- Share OMG with others

### Where is the source code?

[github.com/PyRo1121/omg](https://github.com/PyRo1121/omg)

---

## 📚 More Questions?

- Check the [Troubleshooting Guide](./troubleshooting.md)
- Read the [CLI Reference](./cli.md)
- Open a [GitHub Issue](https://github.com/PyRo1121/omg/issues)
