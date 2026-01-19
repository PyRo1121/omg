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

OMG ships as three binaries, all built from the same Rust library:

| Binary | Purpose | Size |
|--------|---------|------|
| `omg` | Main CLI interface | ~15MB |
| `omgd` | Persistent daemon | ~15MB |
| `omg-fast` | Ultra-fast status queries | ~5MB |

### omg (CLI)

The main user-facing binary:
- Parses commands via clap derive macros
- Communicates with daemon via Unix socket IPC
- Falls back to direct operations if daemon unavailable
- Spawns tokio runtime for async operations

**Source:** `src/bin/omg.rs`, `src/cli/`

### omgd (Daemon)

The background service:
- Maintains in-memory package index
- Handles IPC requests from CLI
- Runs background refresh workers
- Persists status to binary file and redb

**Source:** `src/bin/omgd.rs`, `src/daemon/`

### omg-fast

Specialized ultra-fast queries:
- Reads binary status file directly
- Sub-millisecond response times
- Used for shell prompts

**Source:** `src/bin/omg-fast.rs`

---

## 🗂️ Library Organization

```
src/
├── bin/
│   ├── omg.rs           # CLI entry point
│   ├── omgd.rs          # Daemon entry point
│   └── omg-fast.rs      # Fast query entry point
├── cli/
│   ├── mod.rs           # CLI module root
│   ├── args.rs          # Command definitions (clap)
│   ├── commands.rs      # Command implementations
│   ├── containers.rs    # Container commands
│   ├── tui/             # TUI dashboard
│   │   ├── mod.rs
│   │   ├── app.rs
│   │   └── ui.rs
│   └── ...
├── core/
│   ├── mod.rs           # Core module root
│   ├── types.rs         # Common types
│   ├── errors.rs        # Error definitions
│   ├── database.rs      # redb wrapper
│   ├── archive.rs       # Archive extraction
│   ├── client.rs        # HTTP client
│   ├── history.rs       # Transaction history
│   ├── security/        # Security features
│   │   ├── mod.rs
│   │   ├── audit.rs
│   │   ├── pgp.rs
│   │   ├── sbom.rs
│   │   ├── secrets.rs
│   │   ├── slsa.rs
│   │   └── vuln.rs
│   └── ...
├── daemon/
│   ├── mod.rs           # Daemon module root
│   ├── server.rs        # Server loop
│   ├── handlers.rs      # Request handlers
│   ├── protocol.rs      # IPC protocol types
│   ├── cache.rs         # Cache management
│   ├── db.rs            # Persistence
│   └── index.rs         # Package index
├── runtimes/
│   ├── mod.rs           # Runtime module root
│   ├── manager.rs       # RuntimeManager trait
│   ├── node.rs          # Node.js manager
│   ├── python.rs        # Python manager
│   ├── rust.rs          # Rust manager
│   ├── go.rs            # Go manager
│   ├── ruby.rs          # Ruby manager
│   ├── java.rs          # Java manager
│   ├── bun.rs           # Bun manager
│   └── mise.rs          # Mise integration
├── package_managers/
│   ├── mod.rs           # Package manager root
│   ├── alpm/            # Arch (libalpm)
│   ├── aur/             # AUR client
│   └── apt/             # Debian (rust-apt)
├── hooks/
│   └── mod.rs           # Shell hooks
├── shims/
│   └── mod.rs           # Shim generation
├── config/
│   └── mod.rs           # Configuration
└── lib.rs               # Library root
```

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

### Three-Tier Caching

| Tier | Technology | TTL | Purpose |
|------|------------|-----|---------|
| L1 | moka (in-memory) | 5 min | Hot request cache |
| L2 | Binary status file | - | Shell prompt queries |
| L3 | redb (persistent) | - | Across daemon restarts |

### moka Cache

High-performance concurrent cache:
- 1000 entry limit (configurable)
- 5-minute TTL
- LRU eviction

**Cached data:**
- Search results
- Package info
- System status
- Explicit package list

### Binary Status File

Fixed-format binary file for ultra-fast reads:
- Location: `$XDG_RUNTIME_DIR/omg.status`
- Format: 4x u32 (total, explicit, orphans, updates)
- Updated every 300 seconds

### redb Persistence

ACID-compliant embedded database:
- Location: `~/.local/share/omg/cache.redb`
- Stores: System status
- Auto-compacts on write

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

## 🔧 Runtime Management

### RuntimeManager Trait

All runtime managers implement:

```rust
#[async_trait]
pub trait RuntimeManager: Send + Sync {
    fn runtime(&self) -> Runtime;
    async fn list_available(&self) -> Result<Vec<String>>;
    fn list_installed(&self) -> Result<Vec<RuntimeVersion>>;
    async fn install(&self, version: &str) -> Result<()>;
    fn uninstall(&self, version: &str) -> Result<()>;
    fn use_version(&self, version: &str) -> Result<()>;
}
```

### Version Storage

```
~/.local/share/omg/versions/
├── node/
│   ├── 18.17.0/
│   │   └── bin/
│   │       ├── node
│   │       ├── npm
│   │       └── npx
│   ├── 20.10.0/
│   │   └── bin/...
│   └── current → 20.10.0
├── python/
│   ├── 3.11.0/
│   ├── 3.12.0/
│   └── current → 3.12.0
└── ...
```

### Resolution Strategy

```
native-then-mise (default):
    1. Check native managers (Node, Python, Go, Rust, Ruby, Java, Bun)
    2. Fall back to mise for unsupported runtimes
    3. Auto-download mise if needed
```

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
