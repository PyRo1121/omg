---
title: Package Search
sidebar_position: 34
description: Search indexing and ranking algorithms
---

# Package Search Flow

OMG's search engine is built for extreme speed and relevance. It combines a high-performance local index of official packages with an intelligent, conditional fallback to the AUR (Arch User Repository).

## 🔍 The Search Pipeline

When you type `omg search <query>`, the system executes a multi-stage pipeline to find the best results in less than a blink.

### 1. Instant Cache Check
First, the daemon checks its internal high-speed memory cache.
- **Latency**: &lt;0.1ms
- **Hit Rate**: Over 80% for common queries
- **Freshness**: Cache is automatically invalidated every 5 minutes to ensure results stay current.

### 2. Official Index Lookup
If the cache misses, OMG queries its optimized in-memory index of all official repository packages.
- **Search Strategy**: Uses a hybrid approach combining prefix matching and SIMD-accelerated substring search.
- **Performance**: High-speed substring matching allows the engine to scan 15,000+ packages in less than 1ms.
- **Ranking**: Instant results are returned as soon as matches are found, prioritizing binary name matches.

### 3. Conditional AUR Fallback
To save bandwidth and time, OMG only hits the network for AUR results if the official repositories don't provide a strong match.
- **Trigger**: Activated only if official results are sparse (e.g., fewer than 5 matches).
- **Latency**: 50–200ms (depending on network conditions).
- **Architecture**: Communicates via the official AUR RPC interface with built-in rate limiting.

### 4. Result Aggregation
Finally, results from all sources are merged, ranked, and presented in a unified list. Official packages are always prioritized over community-maintained (AUR) versions for maximum security.

---

## 📊 Performance at a Glance

| Stage | Data Source | Latency | Benefit |
|-------|-------------|---------|---------|
| **Cache** | System RAM | &lt;0.1ms | Instant repeated queries |
| **Official Index** | Local SSD/RAM | &lt;1ms | Blazing fast primary search |
| **AUR RPC** | Network | 50-200ms | Millions of packages available |

---

## 🛠️ Technical Highlights

### Zero-Allocation Probing
OMG uses a highly optimized probe mechanism to check system status and package availability. By avoiding unnecessary memory allocations, the engine remains responsive even under heavy load.

### Parallel Security Audits
When performing an audit (`omg audit`), the engine divides your installed packages into chunks and scans them in parallel across all CPU cores. High-severity vulnerabilities are prioritized and highlighted instantly.

### Intelligent Error Recovery
The search engine is designed for resilience:
- **Network Outage**: If the AUR is unreachable, the system continues to serve official results seamlessly.
- **Index Corruption**: The engine can automatically rebuild its local index from system databases if corruption is detected.
- **Rate Limiting**: Gracefully handles upstream API limits to prevent IP blocking.
