---
id: index
title: OMG Documentation
sidebar_label: Introduction
sidebar_position: 1
slug: /
description: The complete guide to the fastest unified package manager for Arch Linux and all language runtimes
---

# OMG Documentation

**The Complete Guide to the Fastest Unified Package Manager**

Welcome to the official OMG documentation. This comprehensive guide covers everything from basic usage to advanced enterprise features, performance tuning, and security hardening.

---

## 🎯 Documentation Overview

OMG is a next-generation package manager that unifies system packages (Arch Linux, Debian/Ubuntu) with language runtime management (Node.js, Python, Go, Rust, Ruby, Java, Bun) into a single, blazing-fast binary.

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

1. **[CLI Reference](./cli)** — Learn all available commands
2. **[Configuration](./configuration)** — Configure OMG for your workflow
3. **[Migration Guides](./migration/from-yay)** — Coming from yay, nvm, or pyenv?

### For Power Users

1. **[Runtime Management](./runtimes)** — Master multi-runtime environments
2. **[Workflows](./workflows)** — Common patterns and recipes
3. **[TUI Dashboard](./tui)** — Real-time system monitoring

### For Enterprise/DevOps

1. **[Security & Compliance](./security)** — SBOM, vulnerability scanning, audit logs
2. **[Daemon Internals](./daemon)** — Deployment and scaling considerations
3. **[Architecture Overview](./architecture)** — System design

---

## 📞 Support & Community

- **GitHub Issues**: [github.com/PyRo1121/omg/issues](https://github.com/PyRo1121/omg/issues)
- **Discussions**: [github.com/PyRo1121/omg/discussions](https://github.com/PyRo1121/omg/discussions)

---

## 📄 License

OMG is licensed under **AGPL-3.0-or-later**. Commercial licenses are available for organizations that cannot comply with AGPL requirements.
