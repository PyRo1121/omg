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

| Runtime | Management Features | Supported Version Files | Binary Access |
|---------|---------------------|--------------------------|---------------|
| **Node.js** | Official dist integration, SHA256 verification | `.nvmrc`, `.node-version` | `node`, `npm`, `npx` |
| **Python** | Standalone build support, GPG verification | `.python-version` | `python3`, `pip` |
| **Go** | Official binary distribution support | `.go-version` | `go` |
| **Rust** | Direct toolchain management (No rustup needed) | `.rust-version`, `rust-toolchain.toml` | `rustc`, `cargo` |
| **Ruby** | Ruby-builder integration | `.ruby-version` | `ruby`, `gem` |
| **Java** | Adoptium (Eclipse Temurin) support | `.java-version` | `java`, `javac` |
| **Bun** | Official release management | `.bun-version` | `bun`, `bunx` |

### Extended Universe (Built-in Mise)
For everything else, OMG includes a **built-in mise manager**. If you request a runtime that isn't natively handled, OMG automatically leverages the `mise` ecosystem to provide support for over **100 additional languages and tools**, including Deno, Elixir, Zig, PHP, and more.

---

## 🛠️ How It Works

### Version Switching
OMG achieves sub-millisecond version switching by using atomic symlink updates in your local data directory. When you run `omg use <version>`, the system instantly updates a `current` symlink to point to the desired installation.

### Project Isolation
OMG automatically detects version files (`.nvmrc`, `.python-version`, etc.) as you navigate between directories. It walks up your file system to find the closest project root and ensures your environment is always in sync with your source code.

### Sandboxed Installations
All runtimes are installed in a user-local directory (`~/.local/share/omg/versions`). This provides several benefits:
- **No Sudo Required**: Install and switch versions without ever needing administrative privileges.
- **Isolation**: Different versions cannot interfere with each other or your system's global packages.
- **Easy Cleanup**: Removing a runtime is as simple as deleting its specific version directory.

---

## 🔒 Security and Integrity

Safety is a first-class citizen in OMG's runtime management:
- **Cryptographic Verification**: Every download is verified against official SHASUM256 or GPG signatures.
- **Secure Transport**: All downloads are performed over HTTPS with certificate validation.
- **Isolated Build Paths**: For runtimes that require compilation, OMG uses isolated build directories to prevent environment pollution.

---

## 📊 Performance Profile

OMG is designed to be the fastest runtime manager in the industry:
- **Switching Latency**: ~100μs (single atomic symlink update)
- **Detection Lag**: Zero (uses an optimized, zero-allocation directory probe)
- **Memory Footprint**: <1MB baseline per runtime
- **Sync Downloads**: High-speed, parallel downloading of multiple runtimes
