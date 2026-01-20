---
title: Caching & Indexing
sidebar_position: 32
description: In-memory and persistent caching strategies
---

# Caching & Indexing

OMG's performance is driven by a sophisticated, three-tiered caching architecture. This multi-layered approach ensures that data is stored in the most efficient location based on its frequency of use and durability requirements.

## 🧠 Tier 1: In-Memory (Hot Cache)

The "Hot" layer uses a high-performance, concurrent memory cache designed for sub-millisecond access.

- **Technology**: Built on a lock-free, concurrent caching engine.
- **Data Types**: Stores recent search results, detailed package metadata, and system status results.
- **Eviction Strategy**: Uses an intelligent Least Recently Used (LRU) policy to stay within memory limits.
- **Latency**: < 0.1ms

| Cache Type | Typical Capacity | TTL (Time-to-Live) |
|------------|------------------|--------------------|
| **Search Queries** | 1,000 entries | 5 Minutes |
| **Package Details**| 1,000 entries | 5 Minutes |
| **System Status**  | 1 entry | 30 Seconds |

---

## 💾 Tier 2: Persistent Storage (Cold Cache)

For data that must survive reboots or daemon restarts, OMG uses an embedded, ACID-compliant database.

- **Technology**: A pure-Rust, transactional atomic database.
- **Durability**: Ensures your transaction history and audit logs are never lost, even in the event of a system crash.
- **Location**: Stored locally in `~/.local/share/omg/cache.redb`.
- **Latency**: < 5ms (disk-dependent)

---

## 🔍 Tier 3: The Optimized Package Index

The core of OMG's search capability is a custom-built package index designed for ultra-fast fuzzy searching across tens of thousands of items.

### High-Speed Indexing
Upon startup, the daemon builds a structured index from your system's package databases (ALPM or APT). This index is kept entirely in memory for the duration of the daemon's life.

### Intelligent Search Algorithm
1. **Prefix Fast Path**: For 1-2 character queries, the engine uses a pre-computed prefix index for instantaneous results.
2. **Parallel Fuzzy Search**: For longer queries, the engine parallelizes the search across all available CPU cores using the **Nucleo** fuzzy matching algorithm.
3. **Keyword Enrichment**: Each entry is indexed by both its name and its description, allowing you to find tools even when you only know what they do, not what they are called.

---

## 🔄 Data Lifecycle Patterns

### Search Request Flow
The system always attempts to serve results from Tier 1 (Memory). If there is a miss, it falls back to Tier 3 (Local Index). If the local results are insufficient, only then does it trigger a Tier 4 (Network) request to the AUR.

### Status Monitoring
System status is generated in the background every 5 minutes and stored in both Tier 1 and Tier 2. This ensures that tools like `omg-fast` always have access to a pre-computed, durable state without needing to query the system live.
