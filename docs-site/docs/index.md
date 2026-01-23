---
title: Introduction
sidebar_position: 1
description: The complete guide to the fastest unified package manager
---

# OMG Documentation

## The Problem Every Developer Knows

You're setting up a new machine. Or onboarding a teammate. Or just trying to get a project running. And suddenly you're juggling:

```bash
pacman -Syu                    # System packages
yay -S some-aur-package        # AUR packages
nvm install 20 && nvm use 20   # Node.js
pyenv install 3.12.0           # Python
rustup default stable          # Rust
rbenv install 3.2.0            # Ruby
sdk install java 21            # Java
```

Seven different tools. Seven different syntaxes. Seven different config files. Seven different ways things can break.

**OMG fixes this.**

---

## One Tool. Everything You Need.

```bash
omg install some-aur-package   # System + AUR packages
omg install ripgrep            # Arch (ALPM) or Debian (APT)
omg use node 20                # Node.js
omg use python 3.12            # Python
omg use rust stable            # Rust
omg use ruby 3.2               # Ruby
omg use java 21                # Java
```

Same syntax. Same tool. Same config. **And it's 22x faster than pacman.**

---

## Why Developers Switch to OMG

### It's Fast. Really Fast.

| Operation | OMG | pacman/yay | How much faster |
|-----------|-----|------------|-----------------|
| Package search | 6ms | 133ms | **22x** |
| Package info | 6.5ms | 138ms | **21x** |
| List installed | 1.2ms | 14ms | **12x** |

**Zero C Dependencies:** OMG uses a pure Rust stack for maximum performance and portability, including `redb` for metadata storage and `zlib-rs`/`ruzstd` for lightning-fast decompression.

On Debian/Ubuntu, it's even more dramatic: **59-483x faster** than apt-cache.

This isn't a benchmark gimmick. OMG uses a persistent daemon with an in-memory package index. Your searches return before your finger leaves the Enter key.

### It Eliminates "Works on My Machine"

```bash
# Capture your entire environment
omg env capture

# Commit omg.lock to your repo
git add omg.lock && git commit -m "Lock environment"

# Teammates sync with one command
omg env sync
```

No more "what version of Node are you running?" No more "did you install the dependencies?" Everyone has the exact same environment.

### It Has Enterprise Security Built In

Most developers bolt security on as an afterthought. OMG has it built in:

- **Vulnerability scanning** — Know about CVEs before they bite you
- **SBOM generation** — CycloneDX 1.5 format for compliance requirements
- **Secret scanning** — Catch leaked API keys and credentials
- **Audit logging** — Tamper-proof logs for compliance
- **PGP verification** — Verify package signatures automatically

### It Runs Your Tasks Intelligently

```bash
omg run dev      # Detects priority: Rust > Node > Python
omg run test     # Ambiguity? Interactive prompt asks your preference
omg run build --all # Run build script in EVERY detected ecosystem
```

One command works across all your projects, now with a smart priority hierarchy and interactive ambiguity resolution.

### Stay Up to Date Effortlessly

```bash
omg self-update  # Updates OMG with a beautiful progress bar
```

Never miss an improvement with the atomic self-updater.

---

## Who Is OMG For?

### Individual Developers

Stop wasting time switching between tools. Stop waiting for slow package searches. Stop debugging environment issues. Just get your work done.

### Teams

Share your exact environment through `omg.lock`. Detect when teammates drift from the baseline. Onboard new developers in minutes instead of hours.

### DevOps & Platform Engineers

Generate SBOMs for compliance. Scan for vulnerabilities in CI. Create reproducible builds. Enforce security policies across your organization.

### Enterprises

Fleet management across thousands of machines. Hierarchical policy enforcement. Self-hosted registries for air-gapped environments. SOC2/ISO27001/FedRAMP compliance evidence export.

---

## What Can OMG Do?

### Package Management
Search, install, update, and remove system packages from official repositories and AUR. Handles dependencies automatically. Shows security grades for every package.

### Runtime Version Management
Install and switch between versions of Node.js, Python, Rust, Go, Ruby, Java, and Bun. Detects `.nvmrc`, `.python-version`, and similar files automatically. Plus 100+ additional runtimes through built-in mise integration.

### Environment Synchronization
Capture your complete environment to a lockfile. Share it with teammates. Detect drift. Sync instantly. Never debug "works on my machine" again.

### Security & Compliance
Scan for vulnerabilities. Generate SBOMs. Detect leaked secrets. Verify package signatures. Maintain tamper-proof audit logs. Export compliance evidence.

### Task Running
Run project tasks with automatic runtime detection. Works with npm, Cargo, Make, Go, Python, and more. One command, any project.

### Container Integration
Generate Dockerfiles from your project. Run dev shells in containers. Build images. Integrate with Docker and Podman.

### Interactive Dashboard
Full-screen TUI showing system status, packages, runtimes, security alerts, and activity. Real-time monitoring at your fingertips.

---

## Getting Started

### Install OMG (30 seconds)

```bash
curl -fsSL https://pyro1121.com/install.sh | bash
```

### Set Up Your Shell (10 seconds)

```bash
# Add to ~/.zshrc (or ~/.bashrc)
eval "$(omg hook zsh)"
```

### Start Using It (immediately)

```bash
omg search firefox          # Search packages
omg install neovim          # Install packages
omg use node 20             # Switch to Node.js 20
omg run dev                 # Run your project
```

**[Full Quick Start Guide →](./quickstart.md)**

---

## Documentation

### Getting Started
- **[Quick Start](./quickstart.md)** — Install and run your first commands
- **[CLI Reference](./cli.md)** — Every command explained
- **[Configuration](./configuration.md)** — Customize OMG for your workflow

### Core Features
- **[Package Management](./packages.md)** — Search, install, update, remove
- **[Runtime Management](./runtimes.md)** — Node, Python, Rust, Go, Ruby, Java, Bun
- **[Shell Integration](./shell-integration.md)** — Hooks, completions, PATH management
- **[Task Runner](./task-runner.md)** — Unified task execution with smart detection
- **[Enterprise & Fleet](./enterprise.md)** — Fleet control and compliance at scale

### Advanced Features
- **[Security & Compliance](./security.md)** — Vulnerability scanning, SBOM, audit logs
- **[Team Collaboration](./team.md)** — Environment lockfiles, drift detection
- **[Container Support](./containers.md)** — Docker/Podman integration
- **[TUI Dashboard](./tui.md)** — Interactive terminal dashboard

### Deep Dives
- **[Architecture](./architecture.md)** — How OMG works under the hood
- **[Daemon Internals](./daemon.md)** — The secret to OMG's speed
- **[Troubleshooting](./troubleshooting.md)** — Common issues and fixes
- **[FAQ](./faq.md)** — Frequently asked questions

---

## The Numbers

A 10-person team doing 50 package operations per day saves **39 minutes per engineer per year** just on package queries. That's **6.5 hours of engineering time** returned to your team annually.

For a 50-person org at average engineering salaries, that's **$2,350-$2,650/year** in reclaimed productivity. And that's before accounting for eliminated environment debugging, faster onboarding, and reduced security incidents.

---

## Support

- **Issues**: [github.com/PyRo1121/omg/issues](https://github.com/PyRo1121/omg/issues)
- **Discussions**: [github.com/PyRo1121/omg/discussions](https://github.com/PyRo1121/omg/discussions)

---

## License

OMG is licensed under **AGPL-3.0-or-later**. Commercial licenses available for organizations that need them.

---

**Ready to stop juggling tools?** [Get started in 5 minutes →](./quickstart.md)
