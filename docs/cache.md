---
title: Caching & Indexing
sidebar_position: 32
description: In-memory and persistent caching strategies
---

# Caching & Indexing

OMG's performance is driven by a sophisticated, three-tiered persistence architecture. This multi-layered approach ensures that data is stored in the most efficient location based on its frequency of use and durability requirements.

## 🧠 Tier 1: In-Memory (Hot Cache)

The "Hot" layer uses a high-performance, concurrent memory cache designed for sub-millisecond access.

- **Technology**: Built on a lock-free, concurrent caching engine.
- **Data Types**: Stores recent search results, detailed package metadata, and system status results.
- **Eviction Strategy**: Uses an intelligent Least Recently Used (LRU) policy to stay within memory limits.
- **Latency**: < 0.1ms

---

## 💾 Tier 2: Persistent Snapshot (Cold Cache)

For data that must survive reboots or daemon restarts, OMG keeps a versioned JSON status snapshot. The daemon writes it through a same-directory temporary file, `fsync`, and atomic rename, so a crash can never leave a truncated file behind.

- **Technology**: Versioned JSON snapshot (`status-cache.json`).
- **Durability**: Atomic replacement plus owner-only file mode.
- **Location**: Stored locally in `~/.local/share/omg/` (`OMG_DAEMON_DATA_DIR` overrides it).
- **Latency**: < 5ms (disk-dependent)

---

## 🔍 Tier 3: Binary Snapshot Layer

A specialized binary snapshot file is maintained by the daemon to store your system's "vital signs" (update counts, error status). This is what enables `omg ec|tc|oc|uc` to power your shell prompt with zero-allocation, zero-IPC reads, achieving instantaneous updates.

---

## 🔄 Data Lifecycle Patterns

### Search Request Flow
The system always attempts to serve results from Tier 1 (Memory). If there is a miss, it falls back to the daemon's local package index. If the local results are insufficient, only then does it make a network request to the AUR.

### Status Monitoring
System status is generated in the background every 5 minutes and stored in both Tier 1 and Tier 2. This ensures that prompt counters (`omg ec|tc|oc|uc`) always have access to a pre-computed, durable state without needing to query the system live.
