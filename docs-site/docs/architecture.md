---
title: Architecture
sidebar_position: 30
description: System architecture and component overview
---

# Architecture Overview

**System Design and Component Architecture**

This document provides a deep dive into OMG's architecture, component interactions, and the high-performance design decisions that enable sub-10ms package operations.

---

## 🏗️ System Overview

OMG uses a distributed daemon-client model designed for zero-overhead interaction.

```mermaid
graph TD
    User[User / Terminal] --> CLI[omg CLI]
    User --> FastCLI[omg-fast CLI]
    
    subgraph "Client Layer"
        CLI
        FastCLI
    end
    
    CLI -- "Unix IPC (bitcode)" --> Daemon
    FastCLI -- "Direct Read (rkyv)" --> Snapshot[Binary Snapshot File]
    
    subgraph "Daemon Layer (omgd)"
        Daemon[omgd Server]
        Worker[Background Status Worker]
        CacheManager[Cache Manager]
        
        Daemon <--> CacheManager
        Worker --> CacheManager
        Worker --> Snapshot
    end
    
    subgraph "Storage & Caching"
        CacheManager <--> Moka[L1: moka In-Memory]
        CacheManager <--> Redb[L2: redb Persistent]
    end
    
    subgraph "Backends"
        Daemon --> Pacman[libalpm / Arch]
        Daemon --> APT[rust-apt / Debian]
        Daemon --> AUR[AUR / HTTPS]
        Daemon --> Runtimes[Runtime Manager]
    end
```

---

## 🔌 Unix IPC Flow (bitcode)

OMG uses a custom binary protocol over Unix Domain Sockets. This ensures minimal overhead and maximum throughput.

```mermaid
sequenceDiagram
    participant CLI as omg CLI
    participant Srv as omgd Server
    participant Cache as moka Cache
    participant DB as redb Store

    CLI->>CLI: Parse Arguments (clap)
    CLI->>CLI: Serialize Request (bitcode)
    CLI->>Srv: Send Frame (Length-Delimited)
    
    Srv->>Srv: Validate Frame Size
    Srv->>Srv: Deserialize Request (bitcode)
    
    Srv->>Cache: Check Query Cache
    alt Cache Hit
        Cache-->>Srv: Return Cached Result
    else Cache Miss
        Srv->>Srv: Execute Backend Query
        Srv->>Cache: Update Cache
        Srv->>DB: Persist (if required)
    end
    
    Srv->>Srv: Serialize Response (bitcode)
    Srv->>CLI: Send Response Frame
    
    CLI->>CLI: Deserialize & Render
```

### Protocol Characteristics
- **Transport**: Unix Domain Sockets (UDS) for local-only, high-speed communication.
- **Framing**: `LengthDelimitedCodec` ensures atomic message delivery.
- **Serialization**: `bitcode` provides high-speed binary encoding with optional zero-copy support.
- **Concurrency**: `tokio` handles thousands of concurrent IPC requests with minimal context switching.

---

## 💾 Multi-Tier Caching Strategy

OMG employs a tiered approach to data availability, ensuring that common queries never hit the disk or network.

```mermaid
graph LR
    subgraph "Tier 0: Snapshots"
        FS[FastStatus File]
    end
    
    subgraph "Tier 1: In-Memory"
        Moka[moka LRU Cache]
    end
    
    subgraph "Tier 2: Persistent"
        Redb[redb ACID Store]
    end
    
    subgraph "Tier 3: System/Network"
        System[System DBs / APIs]
    end
    
    Query[Query] --> FS
    FS -- "Miss" --> Moka
    Moka -- "Miss" --> Redb
    Redb -- "Miss" --> System
```

### 1. In-Memory (moka)
The **moka** cache handles high-frequency data:
- **Search Results**: Cached for 5 minutes.
- **Package Details**: Cached for 10 minutes.
- **Rate Limit Buckets**: Managed per-connection.

### 2. Persistent (redb)
The **redb** store handles data that must survive reboots:
- **Pre-computed Index**: Cold-start search results.
- **Audit Logs**: Tamper-proof history.
- **Security Scores**: Cached SLSA/PGP validation results.

### 3. Binary Snapshot (rkyv)
A specialized file (`~/.local/share/omg/status.bin`) stores a zero-copy AST of system health. This allows `omg-fast` to provide shell prompt updates in **<500μs**.

---

## 🛡️ Security Architecture

The daemon enforces a multi-stage validation pipeline for every package operation.

```mermaid
flowchart TD
    Start[Download Package] --> Checksum[Verify SHA256]
    Checksum --> PGP[Sequoia-OpenPGP Signature]
    PGP --> SLSA[Sigstore / Rekor SLSA]
    SLSA --> Vuln[ALSA / OSV Vulnerability Scan]
    Vuln --> Policy[Policy Enforcement]
    Policy --> Final[Install / Reject]
    
    Final --> Audit[Log to redb Audit Table]
```

### Components
- **Sequoia-OpenPGP**: Native Rust implementation of OpenPGP for signature verification.
- **Sigstore/Rekor**: Integration with the Sigstore transparency log for SLSA provenance.
- **OSV.dev**: Real-time vulnerability matching against global databases.

---

## 🔧 Runtime Management

OMG manages language runtimes via a unified trait system.

- **Native Managers**: Custom Rust implementations for Node.js, Python, Rust, Go, Ruby, Java, and Bun.
- **mise Fallback**: Seamless integration with the `mise` ecosystem for 100+ additional runtimes.
- **Environment Isolation**: Runtimes are stored in `~/.local/share/omg/versions`, avoiding the need for `sudo`.

---

## 📚 Further Reading

- [IPC Protocol Deep Dive](./ipc.md)
- [Caching System Internals](./cache.md)
- [Security Model & Compliance](./security.md)
