---
title: Architecture
sidebar_position: 30
description: System architecture and component overview
---

# Architecture Overview

**System Design and Component Architecture**

This document provides a high-level overview of OMG's architecture, component interactions, and design decisions.

---

## 🏗️ System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              USER                                        │
│                                │                                         │
│                    ┌───────────┴───────────┐                            │
│                    ▼                       ▼                            │
│              ┌──────────┐           ┌──────────────┐                    │
│              │ omg CLI  │           │ omg-fast CLI │                    │
│              └────┬─────┘           └──────┬───────┘                    │
│                   │                        │                            │
│                   │    Unix Socket IPC     │ Direct status read        │
│                   ▼                        ▼                            │
│              ┌────────────────────────────────────┐                     │
│              │           omgd (Daemon)            │                     │
│              │  ┌──────────────────────────────┐  │                     │
│              │  │      In-Memory Caches        │  │                     │
│              │  │  ┌─────────┐ ┌────────────┐  │  │                     │
│              │  │  │  moka   │ │  Index     │  │  │                     │
│              │  │  │  LRU    │ │  (Nucleo)  │  │  │                     │
│              │  │  └─────────┘ └────────────┘  │  │                     │
│              │  └──────────────────────────────┘  │                     │
│              │  ┌──────────────────────────────┐  │                     │
│              │  │        Persistence           │  │                     │
│              │  │  ┌─────────┐ ┌────────────┐  │  │                     │
│              │  │  │  redb   │ │ Binary     │  │  │                     │
│              │  │  │  (ACID) │ │ Status     │  │  │                     │
│              │  │  └─────────┘ └────────────┘  │  │                     │
│              │  └──────────────────────────────┘  │                     │
│              └────────────────┬───────────────────┘                     │
│                               │                                         │
│         ┌─────────────────────┼─────────────────────┐                   │
│         │                     │                     │                   │
│         ▼                     ▼                     ▼                   │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐             │
│  │   libalpm   │      │  rust-apt   │      │  AUR HTTP   │             │
│  │   (Arch)    │      │  (Debian)   │      │   Client    │             │
│  └─────────────┘      └─────────────┘      └─────────────┘             │
│         │                     │                     │                   │
│         ▼                     ▼                     ▼                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Operating System                              │   │
│  │    /var/lib/pacman    /var/lib/dpkg    https://aur.archlinux.org│   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 📦 Binary Components

OMG is distributed as a unified set of three specialized binaries, all statically linked for maximum portability and zero dependencies.

### omg (The CLI)
The primary user interface. It is designed for human interaction, providing rich colored output, progress bars, and interactive TUI elements. It handles argument parsing, security policy enforcement, and communicates with the background daemon via a high-performance Unix socket.

### omgd (The Daemon)
The "brain" of the system. It runs as a lightweight background service that maintains an in-memory index of all system packages and language runtimes. It handles heavy lifting like background vulnerability scanning, metadata indexing, and complex dependency resolution.

### omg-fast (The Prompt Optimizer)
A specialized, ultra-lightweight binary specifically for shell prompts. It skips all network and IPC logic, reading system status directly from a pre-computed binary file to achieve sub-millisecond response times.



---

## 🔄 Data Flow

### Search Request

```
User: omg search firefox
         │
         ▼
    ┌─────────┐
    │ omg CLI │ Parse args, create Request
    └────┬────┘
         │ Unix Socket
         ▼
    ┌─────────┐
    │  omgd   │ Check moka cache
    └────┬────┘
         │ Cache miss
         ▼
    ┌──────────────────────────────────┐
    │ Parallel Query                    │
    │  ┌─────────┐    ┌─────────────┐  │
    │  │ libalpm │    │ AUR HTTP    │  │
    │  │  query  │    │   query     │  │
    │  └────┬────┘    └──────┬──────┘  │
    │       │                │         │
    │       └───────┬────────┘         │
    │               ▼                  │
    │         Merge & Rank            │
    │           (Nucleo)              │
    └──────────────┬───────────────────┘
                   │
                   ▼
             Update moka cache
                   │
                   ▼
              Serialize Response
                   │
                   ▼
              Return to CLI
                   │
                   ▼
              Format & Display
```

### Runtime Switch

```
User: omg use node 20.10.0
         │
         ▼
    ┌─────────┐
    │ omg CLI │ Detect runtime type
    └────┬────┘
         │
         ▼
    Check if installed
         │
    ┌────┴────┐
    │ Yes     │ No
    │         ▼
    │    Download from
    │    nodejs.org/dist
    │         │
    │    Extract to
    │    versions/node/20.10.0
    │         │
    └────┬────┘
         │
         ▼
    Update symlink
    versions/node/current → 20.10.0
         │
         ▼
    Shell hook updates PATH
```

---

## 💾 Caching Strategy

OMG uses a multi-tier caching architecture to eliminate the latency typically associated with package managers.

### 1. In-Memory (moka)
The hottest data (recent searches, package details, system status) is kept in a concurrent, high-performance memory cache. This allows multiple CLI instances to share results instantly without hitting the disk.

### 2. Persistent (redb)
Data that should survive a reboot is stored in `redb`, an ACID-compliant embedded database. This includes your transaction history, audit logs, and pre-computed package indices.

### 3. Binary Status
A specialized 16-byte binary file is maintained by the daemon to store your system's "vital signs" (update counts, error status). This is what enables `omg-fast` to power your shell prompt without any noticeable lag.

---

## 🔌 IPC Protocol

### Transport

- **Socket:** Unix Domain Socket
- **Framing:** Length-Delimited (4-byte prefix)
- **Serialization:** bincode

### Message Types

```rust
pub enum Request {
    Search { id: u32, query: String, limit: usize },
    Info { id: u32, name: String },
    Status { id: u32 },
    Security { id: u32, package: Option<String> },
    CacheClear { id: u32 },
    ExplicitList { id: u32 },
}

pub enum Response {
    Success { id: u32, result: ResponseResult },
    Error { id: u32, message: String },
}
```

### Performance

- Serialization: ~10μs
- Round-trip: ~100μs (cached), ~1ms (fresh)
- Max message size: 16MB

---

## 🔧 Runtime Management Architecture

OMG unifies language runtimes under a single "Runtime Manager" interface. This allows every language—whether it's Node.js, Rust, or Java—to behave identically from a user's perspective.

### Version Storage
All runtimes are stored in your home directory (`~/.local/share/omg/versions`), ensuring you never need `sudo` to switch a Node.js version and your system-wide packages remain untouched.

### Resolution Strategy
By default, OMG uses a "native-then-mise" strategy. It prefers its own highly optimized native managers for common languages but can seamlessly fall back to the `mise` ecosystem for more obscure runtimes, giving you the best of both worlds.

---

## 🛡️ Security Architecture

### Verification Pipeline

```
Package Download
      │
      ▼
┌─────────────────┐
│ Checksum Verify │ SHA256
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  PGP Signature  │ Sequoia-OpenPGP
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ SLSA Provenance │ Sigstore/Rekor
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Vulnerability   │ ALSA + OSV.dev
│    Scan         │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Policy Check    │ policy.toml
└────────┬────────┘
         │
         ▼
    Install / Reject
```

### Audit Log

Hash-chained, tamper-proof logging:
- Location: `~/.local/share/omg/audit/audit.jsonl`
- Format: JSON Lines
- Each entry contains hash of previous entry
- Integrity verifiable with `omg audit verify`

---

## 📊 Background Workers

### Status Refresh Worker

Runs every 300 seconds:
1. Probe all runtime versions
2. Count vulnerabilities
3. Generate system status
4. Update moka cache
5. Write binary status file
6. Persist to redb

### ALSA Scanner (Optional)

When enabled, periodically:
1. Fetch ALSA issues from security.archlinux.org
2. Match against installed packages
3. Update daemon status with CVE count

---

## 🔄 Graceful Shutdown

```
SIGINT/SIGTERM
      │
      ▼
┌─────────────────┐
│ Broadcast       │ Send shutdown signal
│ Channel         │
└────────┬────────┘
         │
    ┌────┴────┬────────────┐
    ▼         ▼            ▼
Client    Background    IPC
Tasks     Workers       Server
    │         │            │
    │ Finish  │ Stop       │ Stop
    │ request │ loop       │ accept
    │         │            │
    └────┬────┴────────────┘
         │
         ▼
┌─────────────────┐
│ Clean up socket │
└─────────────────┘
         │
         ▼
      Exit
```

---

## 📚 Deep Dives

For detailed documentation on specific subsystems:

- [Daemon Internals](./daemon.md)
- [IPC Protocol](./ipc.md)
- [Caching System](./cache.md)
- [Package Search](./package-search.md)
- [Runtime Management](./runtimes.md)
- [CLI Internals](./cli-internals.md)
- [Security & Audit](./security.md)
