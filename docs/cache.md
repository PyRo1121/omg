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

## 💾 Tier 2: Persistent Storage (Cold Cache)

For data that must survive reboots or daemon restarts, OMG uses an embedded, ACID-compliant database. This ensures that even in the event of a system crash or power loss, your transaction history and security logs remain uncorrupted.

- **Technology**: A pure-Rust, transactional atomic database.
- **Durability**: Guaranteed data integrity through atomic commits.
- **Location**: Stored locally in `~/.local/share/omg/cache.redb`.
- **Latency**: < 5ms (disk-dependent)

---

## 🔍 Tier 3: Binary Snapshot Layer

A specialized binary snapshot file is maintained by the daemon to store your system's "vital signs" (update counts, error status). This is what enables `omg-fast` to power your shell prompt with zero-allocation, zero-IPC reads, achieving instantaneous updates.

---

## 🔄 Data Lifecycle Patterns

### Search Request Flow
The system always attempts to serve results from Tier 1 (Memory). If there is a miss, it falls back to Tier 3 (Local Index). If the local results are insufficient, only then does it trigger a Tier 4 (Network) request to the AUR.

### Status Monitoring
System status is generated in the background every 5 minutes and stored in both Tier 1 and Tier 2. This ensures that tools like `omg-fast` always have access to a pre-computed, durable state without needing to query the system live.
