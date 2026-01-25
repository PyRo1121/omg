---
title: Caching & Indexing
sidebar_position: 32
description: In-memory and persistent caching strategies
---

# Caching & Indexing

OMG's industry-leading performance is driven by a sophisticated, three-tiered persistence architecture. By layering memory-mapped data, concurrent in-memory stores, and an ACID-compliant embedded database, OMG ensures that the "hot path" for package operations never hits the disk or network unnecessarily.

---

## ⚡ Tier 1: In-Memory (moka Hot Cache)

The "Hot" layer uses the **moka** caching engine, a high-performance, concurrent cache designed for sub-millisecond access.

- **Technology**: `moka::sync::Cache`
- **Data Types**: 
  - Recent search results (5-minute TTL)
  - Detailed package metadata (10-minute TTL)
  - System status snapshots (30-second TTL)
- **Eviction Strategy**: Intelligent Least Recently Used (LRU) policy.
- **Latency**: < 100μs

---

## 💾 Tier 2: Persistent Store (redb Cold Cache)

For data that must survive reboots or daemon restarts, OMG uses **redb**, an embedded, ACID-compliant database. This ensures system integrity and fast cold starts.

- **Technology**: `redb` (pure-Rust transactional database)
- **Serialization**: `rkyv` (zero-copy binary format)
- **Location**: `~/.local/share/omg/cache.redb`
- **Key Features**:
  - **Cold Start Optimization**: The daemon pre-loads the package index from redb on startup, enabling \<10ms first-run queries.
  - **Audit Integrity**: All package operations are logged with atomic commits.
- **Latency**: < 2ms (NVMe) / < 10ms (SATA)

---

## 🔍 Tier 3: Binary Snapshot (rkyv Fast-Status)

A specialized binary snapshot file is maintained by the daemon to store your system's "vital signs" (update counts, version drift).

- **Technology**: `rkyv` Zero-Copy AST
- **Location**: `~/.local/share/omg/status.bin`
- **Purpose**: Powers the `omg-fast` binary used in shell prompts.
- **Advantage**: Bypasses the entire IPC stack. The client simply memory-maps the file and reads the pre-computed status.
- **Latency**: < 500μs

---

## 🔄 Data Lifecycle Patterns

### Search Request Flow
The system always attempts to serve results from Tier 1 (moka). If there is a miss, it falls back to the local index stored in Tier 2 (redb). If the query involves third-party sources (like the AUR), the daemon triggers a background fetch and populates all three tiers upon completion.

### Status Refresh Worker
A background worker runs every **300 seconds** to refresh the system status:
1.  **Probe**: Checks all runtime versions and system packages.
2.  **Verify**: Performs a quick vulnerability scan.
3.  **Sync**: Updates the moka cache.
4.  **Snap**: Writes a new rkyv Binary Snapshot to disk.
5.  **Commit**: Persists the updated state to the redb database.

---

## 🛠️ Management

You can inspect or clear the cache using the CLI:

```bash
omg cache status    # View hit rates and memory usage
omg cache clear     # Flush all caches (moka and redb)
```
