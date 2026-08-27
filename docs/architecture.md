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
│              │ Primary  │           │ Speed-Light  │                    │
│              │ CLI      │           │ Optimizer    │                    │
│              └────┬─────┘           └──────┬───────┘                    │
│                   │                        │                            │
│                   │    Private Interface   │ Direct state read          │
│                   ▼                        ▼                            │
│              ┌────────────────────────────────────┐                     │
│              │           System Daemon            │                     │
│              │  ┌──────────────────────────────┐  │                     │
│              │  │      Instant Access Layer    │  │                     │
│              │  │  ┌─────────┐ ┌────────────┐  │  │                     │
│              │  │  │  Active  │ │  Global    │  │  │                     │
│              │  │  │  Cache   │ │  Index     │  │  │                     │
│              │  │  └─────────┘ └────────────┘  │  │                     │
│              │  └──────────────────────────────┘  │                     │
│              │  ┌──────────────────────────────┐  │                     │
│              │  │      Persistence Layer       │  │                     │
│              │  │  ┌─────────┐ ┌────────────┐  │  │                     │
│              │  │  │ Durable │ │ Binary     │  │  │                     │
│              │  │  │ Storage │ │ Status     │  │  │                     │
│              │  │  └─────────┘ └────────────┘  │  │                     │
│              │  └──────────────────────────────┘  │                     │
│              └────────────────┬───────────────────┘                     │
│                               │                                         │
│         ┌─────────────────────┼─────────────────────┐                   │
│         │                     │                     │                   │
│         ▼                     ▼                     ▼                   │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐             │
│  │   Arch      │      │   Debian    │      │   Cloud     │             │
│  │   Handler   │      │   Handler   │      │   Sources   │             │
│  │  (Native)   │      │  (Native)   │      │   (HTTPS)   │             │
│  └─────────────┘      └─────────────┘      └─────────────┘             │
│         │                     │                     │                   │
│         ▼                     ▼                     ▼                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Operating System                              │   │
│  │    Package DBs        Local Files        Remote Registries       │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
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
│              │  │  │ Atomic  │ │ Binary     │  │  │                     │
│              │  │  │  JSON   │ │ Status     │  │  │                     │
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
│  │   Direct    │      │  (Native)   │      │             │             │
│  │   Bindings  │      │             │      │             │             │
│  └─────────────┘      └─────────────┘      └─────────────┘             │
│         │                     │                     │                   │
│         ▼                     ▼                     ▼                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Operating System                              │   │
│  │    /var/lib/pacman    /var/lib/dpkg    <https://aur.archlinux.org│>   │
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
A specialized, ultra-lightweight binary specifically for shell prompts. It achieves sub-millisecond response times by reading a fixed-size binary snapshot of the system's "vital signs" directly from the filesystem. This optimization bypasses the entire IPC and network stack, acting like a shared-memory interface for instant prompt updates.



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

## 🧩 Package Manager Integration

OMG interacts with system package managers using direct library bindings whenever possible to avoid the overhead of spawning subprocesses.

### Arch Linux (libalpm)
The `ArchPackageManager` implementation uses direct FFI bindings to `libalpm` (via the `alpm` crate). This allows OMG to perform package searches, dependency resolution, and transaction management directly within the process memory space, bypassing the `pacman` CLI entirely. This is a key factor in achieving sub-10ms query performance.

---

## 💾 Caching Strategy

OMG uses a multi-tier caching architecture to eliminate the latency typically associated with package managers.

### 1. In-Memory (moka)
The hottest data (recent searches, package details, system status) is kept in a concurrent, high-performance memory cache. This allows multiple CLI instances to share results instantly without hitting the disk.

### 2. Persistent snapshots
The daemon persists its latest status snapshot as versioned JSON using a same-directory temporary file, `fsync`, and atomic rename. Transaction history and the hash-chained audit log are separate owner-only files; package search indexes are rebuilt from native package-manager databases rather than treated as durable authority.

### 3. Binary Status
A specialized binary status file is maintained by the daemon to store your system's "vital signs" (update counts, error status). This is what enables `omg-fast` to power your shell prompt with zero-allocation, zero-IPC reads.

---

## 🔌 IPC Protocol

### Transport

- **Socket:** Unix Domain Socket
- **Framing:** Length-Delimited via `LengthDelimitedCodec`
- **Serialization:** `bitcode` (high-performance binary serialization)

### Message Types

The protocol supports a wide range of structured requests and responses:

*   **Search**: Query for packages with optional limits.
*   **Info**: Retrieve detailed metadata for a specific package.
*   **Status**: Get the current system "vital signs" (package counts, updates).
*   **SecurityAudit**: Trigger a vulnerability scan across installed packages.
*   **Explicit**: List packages installed by the user.
*   **System Controls**: Commands for cache management, pings, and health checks.

### Performance

- **Serialization Latency**: ~10μs
- **Round-trip Time**: ~100μs for cached data, ~1ms for fresh queries.
- **Efficiency**: Persistent connections can issue multiple individually framed requests without a duplicate batch protocol.

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
6. Atomically persist the versioned JSON status snapshot

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
- [CLI Reference](./cli.md)
- [Security & Audit](./security.md)
