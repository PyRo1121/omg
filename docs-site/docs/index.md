---
title: Introduction
sidebar_position: 1
description: The complete guide to the fastest unified package manager
---

# OMG Documentation

**The Complete Guide to the Fastest Unified Package Manager**

Welcome to the official OMG documentation. This comprehensive guide covers everything from basic usage to advanced enterprise features, performance tuning, and security hardening.

---

## 🎯 Documentation Overview

OMG is a next-generation package manager that unifies system packages (Arch Linux, Debian/Ubuntu) with language runtime management (Node.js, Python, Go, Rust, Ruby, Java, Bun) into a single, blazing-fast binary. This documentation is organized into progressive sections, from getting started to deep technical internals.

---

## 📖 Table of Contents

### Getting Started
| Guide | Description |
|-------|-------------|
| [Quick Start](./quickstart.md) | Installation and first commands in 5 minutes |
| [CLI Reference](./cli.md) | Complete command reference with examples |
| [Configuration](./configuration.md) | Configuration files, paths, and customization |

### Core Features
| Guide | Description |
|-------|-------------|
| [Package Management](./packages.md) | Search, install, update, remove packages |
| [Runtime Management](./runtimes.md) | Managing Node.js, Python, Go, Rust, Ruby, Java, Bun |
| [Shell Integration](./shell-integration.md) | Hooks, completions, and PATH management |
| [Task Runner](./task-runner.md) | Unified task execution across ecosystems |

### Advanced Features
| Guide | Description |
|-------|-------------|
| [Security & Compliance](./security.md) | Vulnerability scanning, SBOM, secrets, audit logs |
| [Team Collaboration](./team.md) | Environment lockfiles, drift detection, team sync |
| [Container Support](./containers.md) | Docker/Podman integration |
| [TUI Dashboard](./tui.md) | Interactive terminal dashboard |
| [History & Rollback](./history.md) | Transaction history and system rollback |

### Architecture & Internals
| Guide | Description |
|-------|-------------|
| [Architecture Overview](./architecture.md) | System design and component overview |
| [Daemon Internals](./daemon.md) | Background service, IPC, and state management |
| [Caching System](./cache.md) | In-memory and persistent caching |
| [IPC Protocol](./ipc.md) | Binary protocol for CLI-daemon communication |
| [Package Search](./package-search.md) | Search indexing and ranking algorithms |
| [CLI Internals](./cli-internals.md) | CLI implementation details |

### Reference
| Guide | Description |
|-------|-------------|
| [Workflows](./workflows.md) | Common workflows and recipes |
| [Troubleshooting](./troubleshooting.md) | Common issues and solutions |
| [FAQ](./faq.md) | Frequently asked questions |
| [Changelog](./changelog.md) | Version history and release notes |

---

## 🚀 Why OMG?

### Performance That Matters

OMG achieves **22x faster** searches than pacman and **59-483x faster** than apt-cache through:

- **Zero subprocess overhead** — Direct library integration with libalpm and rust-apt
- **Persistent daemon** — In-memory package index with instant lookups
- **Pure Rust implementation** — No Python, no shell scripts, just raw speed
- **Smart caching** — moka (in-memory) + redb (persistent) caching layers

| Operation | OMG | pacman | Speedup |
|-----------|-----|--------|---------|
| Search | 6ms | 133ms | **22x** |
| Info | 6.5ms | 138ms | **21x** |
| Explicit list | 1.2ms | 14ms | **12x** |

### Unified Experience

Stop juggling multiple tools:
- ❌ `pacman` + `yay` + `nvm` + `pyenv` + `rustup` + `rbenv` + `sdkman`
- ✅ Just `omg`

### Enterprise-Grade Security

Built-in security features that would cost thousands in enterprise tools:
- Vulnerability scanning (ALSA + OSV.dev)
- CycloneDX 1.5 SBOM generation
- PGP signature verification (Sequoia-OpenPGP)
- SLSA provenance verification via Sigstore
- Secret scanning with 20+ credential patterns
- Tamper-proof audit logging

---

## 🏗️ Architecture at a Glance

```
┌─────────────────────────────────────────────────────────────────┐
│                         OMG CLI (omg)                           │
│  ┌─────────┬──────────┬──────────┬──────────┬────────────────┐  │
│  │ Package │ Runtime  │ Security │ Task     │ TUI Dashboard  │  │
│  │ Mgmt    │ Mgmt     │ Audit    │ Runner   │                │  │
│  └────┬────┴────┬─────┴────┬─────┴────┬─────┴───────┬────────┘  │
│       │         │          │          │             │           │
│       └─────────┴──────────┴──────────┴─────────────┘           │
│                         │ Unix Socket IPC                       │
└─────────────────────────┼───────────────────────────────────────┘
                          │
┌─────────────────────────┼───────────────────────────────────────┐
│                         ▼                                       │
│                    OMG Daemon (omgd)                            │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ Package Index │ moka Cache │ redb Persistence │ Workers │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌──────────────┬──────────────┬──────────────────────────┐     │
│  │ libalpm      │ rust-apt     │ AUR Client               │     │
│  │ (Arch)       │ (Debian)     │ (HTTP API)               │     │
│  └──────────────┴──────────────┴──────────────────────────┘     │
└──────────────────────────────────────────────────────────────────┘
```

---

## 🎓 Learning Path

### For New Users

1. **[Quick Start](./quickstart.md)** — Install OMG and run your first commands
2. **[CLI Reference](./cli.md)** — Learn all available commands
3. **[Shell Integration](./shell-integration.md)** — Set up shell hooks and completions
4. **[Workflows](./workflows.md)** — Common patterns and recipes

### For Power Users

1. **[Runtime Management](./runtimes.md)** — Master multi-runtime environments
2. **[Task Runner](./task-runner.md)** — Unified task execution
3. **[Team Collaboration](./team.md)** — Share environments with teammates
4. **[TUI Dashboard](./tui.md)** — Real-time system monitoring

### For Enterprise/DevOps

1. **[Security & Compliance](./security.md)** — SBOM, vulnerability scanning, audit logs
2. **[Container Support](./containers.md)** — CI/CD and container integration
3. **[Daemon Internals](./daemon.md)** — Deployment and scaling considerations
4. **[Configuration](./configuration.md)** — Policy enforcement and customization

### For Contributors

1. **[Architecture Overview](./architecture.md)** — System design
2. **[CLI Internals](./cli-internals.md)** — Command implementation
3. **[Daemon Internals](./daemon.md)** — Background service details
4. **[IPC Protocol](./ipc.md)** — Binary protocol specification

---

## 📞 Support & Community

- **GitHub Issues**: [github.com/PyRo1121/omg/issues](https://github.com/PyRo1121/omg/issues)
- **Discussions**: [github.com/PyRo1121/omg/discussions](https://github.com/PyRo1121/omg/discussions)
- **Documentation Source**: [docs/](https://github.com/PyRo1121/omg/tree/main/docs)

---

## 📄 License

OMG is licensed under **AGPL-3.0-or-later**. See the [LICENSE](https://github.com/PyRo1121/omg/blob/main/LICENSE) file for details.

Commercial licenses are available for organizations that cannot comply with AGPL requirements. Contact us for details.

---

**Next Steps**: [Quick Start Guide →](./quickstart.md)
