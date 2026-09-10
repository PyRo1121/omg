---
title: Caching & Indexing
sidebar_position: 32
description: In-memory and persistent caching strategies
---

# Caching & Indexing

OMG uses in-memory caches, a persistent status snapshot, and a binary snapshot for prompt counters. These stores serve different requests; they are not a single fallback chain for every command.

## 🧠 Tier 1: In-Memory (Hot Cache)

The "Hot" layer uses a high-performance, concurrent memory cache designed for sub-millisecond access.

- **Technology**: Built on a lock-free, concurrent caching engine.
- **Data Types**: Stores recent search results, detailed package metadata, and system status results.
- **Eviction Strategy**: Uses an intelligent Least Recently Used (LRU) policy to stay within memory limits.
- **Latency**: Depends on the request and cache state; measure the complete command.

---

## 💾 Tier 2: Persistent Snapshot (Cold Cache)

For data that must survive reboots or daemon restarts, OMG keeps a versioned JSON status snapshot. The daemon writes it through a same-directory temporary file, `fsync`, and atomic rename, so a crash can never leave a truncated file behind.

- **Technology**: Versioned JSON snapshot (`status-cache.json`).
- **Durability**: Atomic replacement plus owner-only file mode.
- **Location**: Stored locally in `~/.local/share/omg/` (`OMG_DAEMON_DATA_DIR` overrides it).
- **Latency**: Depends on storage and snapshot size.

---

## 🔍 Tier 3: Binary Snapshot Layer

The daemon maintains a binary snapshot for package counters. The `omg ec`, `omg tc`, `omg oc`, and `omg uc` commands can read it without a daemon request. A snapshot can be stale; a prompt counter is not a live transaction-state guarantee.

---

## 🔄 Data Lifecycle Patterns

### Search Request Flow
Official queries can use daemon caches or a direct backend fallback. On Arch, official and AUR searches run concurrently unless `--no-aur` is set. AUR lookup does not wait for insufficient local results. See [search behavior](./package-search.md).

### Status Monitoring
System status is generated in the background every 5 minutes and stored in Tier 1 and Tier 2 (`status-cache.json`). The daemon also writes the 32-byte atomic Tier 3 binary snapshot (`omg.status` next to the socket), allowing prompt counters (`omg ec|tc|oc|uc`) to read package totals in microseconds with zero IPC.
