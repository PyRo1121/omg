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

Because of the reaction we want you to have when you see how fast it is. 12-24x faster than pacman for searches.

### What platforms are supported?

| Platform | Status | Notes |
|----------|--------|-------|
| Arch Linux | ✅ Full | All features |
| Manjaro | ✅ Full | Same as Arch |
| EndeavourOS | ✅ Full | Same as Arch |
| Debian/Ubuntu | 🔶 Experimental | No AUR equivalent |
| Fedora/RHEL | 🔜 Planned | Coming soon |
| macOS | 🔜 Planned | Homebrew integration |
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

By default, OMG collects **anonymous** usage data to improve the product:
- One-time install ping (random UUID, version, platform)
- Command usage statistics
- Error reports

**No personal data, package names, or file contents are ever collected.**

### How do I opt out of telemetry?

**During installation:**
```bash
curl -fsSL https://... | OMG_NO_TELEMETRY=1 bash
```

**After installation:**
```bash
# Add to your shell config
export OMG_TELEMETRY=0
```

### Where is data sent?

Data is sent to `omg-api.latham.cloud`. The telemetry endpoint only accepts:
- Install counts (for GitHub badge)
- Anonymous command usage patterns
- Error reports with stack traces (no user data)

---

## ⚡ Performance

### How is OMG so fast?

1. **No subprocess overhead** — Direct library integration (libalpm, rust-apt)
2. **Persistent daemon** — In-memory package index with moka caching
3. **Pure Rust** — No Python, no shell scripts
4. **Binary protocol** — Bincode over Unix sockets for IPC

### What are the actual performance numbers?

| Operation | OMG | pacman | Speedup |
|-----------|-----|--------|---------|
| Search | 5-11ms | 133ms | **12-24x** |
| Info | 3-6ms | 138ms | **21-38x** |
| Explicit list | <2ms | 14ms | **7-14x** |

### Does OMG need the daemon to be fast?

The daemon provides maximum speed, but OMG works without it:
- **With daemon**: 5-11ms searches (cached)
- **Without daemon**: 50-200ms searches (direct libalpm)

---

## 📦 Package Management

### Does OMG replace pacman?

No. OMG uses pacman/libalpm under the hood. It's a faster interface, not a replacement.

### Does OMG replace yay/paru?

Yes! OMG has built-in AUR support. You don't need a separate AUR helper.

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

**Native (Pure Rust implementations):**
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

Yes, OMG can manage these runtimes directly. However, they can coexist if needed.

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
- **SLSA provenance** (Sigstore/Rekor)
- **Secret scanning** (20+ credential patterns)
- **Audit logging** (hash-chained; user-owned logs are not authenticated)
- **Policy enforcement** (grade-based blocking)

### What are security grades?

| Grade | Meaning |
|-------|---------|
| LOCKED | Core packages with SLSA + PGP |
| VERIFIED | Official repo packages (PGP verified) |
| COMMUNITY | AUR packages |
| RISK | Packages with known CVEs |

### Is OMG safe to use?

Yes. OMG:
- Verifies PGP signatures on official packages
- Runs without root (except for system package installs via sudo)
- Uses HTTPS for all network requests
- Maintains hash-chained local audit logs

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
