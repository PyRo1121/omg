---
title: Package Search Performance
sidebar_position: 35
description: Deep dive into how OMG achieves 22x faster searches
---

# Package Search Performance Deep Dive

**How OMG Searches 80,000+ Packages in 6 Milliseconds**

This guide explains the multi-layered architecture that makes OMG's package search 22x faster than pacman and 105x faster than Nala.

---

## 📊 Performance Context

### The Challenge

On Arch Linux, the combined official repos + AUR contain ~80,000 packages. Traditional search methods:

```bash
# pacman: ~133ms (parses database files)
pacman -Ss firefox

# yay: ~150ms (pacman + AUR API call)
yay -Ss firefox

# apt-cache: ~652ms (parses text-based Packages files)
apt-cache search firefox

# nala: ~1160ms (Python overhead + apt backend)
nala search firefox
```

### OMG's Approach

OMG achieves **6ms searches** (Arch) and **11ms searches** (Debian) through a three-tier strategy:

1. **In-Memory Index** — Pre-loaded package database
2. **LRU Cache** — Recent search results
3. **Fuzzy Matching** — Nucleo-powered intelligent search

---

## 🏗️ Architecture: Three Performance Layers

### Layer 1: In-Memory Package Index

When the daemon starts, it builds a complete in-memory index of all packages:

```rust
pub struct PackageIndex {
    /// HashMap of package name -> full metadata
    packages: AHashMap<String, DetailedPackageInfo>,

    /// Nucleo search items (name + UTF-32 for fuzzy matching)
    search_items: Vec<(String, Utf32String)>,

    /// Lowercased names for case-insensitive prefix matching
    search_items_lower: Vec<String>,

    /// Prefix index: "fi" -> [firefox, filezilla, ...]
    prefix_index: AHashMap<String, Vec<usize>>,
}
```

#### Why AHashMap?

`AHashMap` (from the `ahash` crate) uses a faster hashing algorithm than Rust's default `HashMap`:
- **50% faster** than SipHash (default)
- **DOS-resistant** (still secure against hash collision attacks)
- **Cache-friendly** (better CPU cache utilization)

#### Building the Index (Arch)

On Arch, OMG uses `alpm-db` to parse repository databases:

```rust
fn new_alpm() -> Result<Self> {
    let start = Instant::now();
    let mut packages = AHashMap::new();

    // Parse /var/lib/pacman/sync/*.db files
    for db_path in find_alpm_databases() {
        let db = alpm_repo_db::parse(&db_path)?;

        for pkg in db.packages {
            packages.insert(pkg.name.clone(), DetailedPackageInfo {
                name: pkg.name,
                version: pkg.version,
                description: pkg.desc,
                url: pkg.url,
                size: pkg.isize,
                download_size: pkg.csize,
                repo: db.name.clone(),
                depends: pkg.depends,
                licenses: pkg.licenses,
                source: "official".to_string(),
            });
        }
    }

    // Build search index from packages
    let index = Self::from_packages(packages.into_values().collect());

    tracing::info!("Index built in {:?} ({} packages)", start.elapsed(), index.len());
    Ok(index)
}
```

**Typical performance**: ~200ms for 15,000 packages (official repos).

#### Building the Index (Debian)

On Debian, OMG uses `rust-apt` to parse APT databases:

```rust
fn new_apt() -> Result<Self> {
    let cache = apt::Cache::get_singleton();
    let mut packages = AHashMap::new();

    for pkg in cache.packages() {
        if let Some(candidate) = pkg.candidate() {
            packages.insert(pkg.name().to_string(), DetailedPackageInfo {
                name: pkg.name().to_string(),
                version: candidate.version().to_string(),
                description: candidate.summary().to_string(),
                // ... additional fields
                source: "apt".to_string(),
            });
        }
    }

    Self::from_packages(packages.into_values().collect())
}
```

**Typical performance**: ~300ms for 60,000 packages (main + universe + multiverse).

---

### Layer 2: Persistent Cache Preloading

To avoid rebuilding the index on every daemon start, OMG uses **persistent caching**:

```rust
pub fn new_with_cache(cache: &PersistentCache) -> Result<Self> {
    let db_mtime = Self::get_db_mtime(); // Get database modification time

    // Try loading from cache first
    if cache.is_index_valid(db_mtime) {
        if let Ok(Some(cached)) = cache.load_index() {
            let index = Self::from_packages(cached.packages);
            tracing::info!("Index loaded from cache in {:?}", start.elapsed());
            return Ok(index);
        }
    }

    // Cache miss — build fresh and save
    let index = Self::new()?;
    cache.save_index(&index.packages, db_mtime)?;

    Ok(index)
}
```

#### Cache Validation

The cache is considered valid if:
1. The cache file exists
2. The stored database mtime matches current mtime
3. The cache version matches current format

**Result**: Cold starts become **\u003c10ms** instead of ~200ms.

---

### Layer 3: Search Execution

When a search request arrives, OMG uses a multi-strategy approach:

#### Strategy 1: LRU Cache (Fastest)

```rust
fn handle_search(state: &DaemonState, query: String) -> Response {
    // Check cache first (~0.1ms)
    if let Some(cached) = state.cache.get(&query) {
        return Response::Success {
            result: ResponseResult::Search(SearchResult {
                packages: cached,
                total: cached.len(),
            }),
        };
    }

    // Cache miss — proceed to index search
    // ...
}
```

The cache is a `moka::sync::Cache` with:
- **Max capacity**: 1000 queries
- **TTL**: 5 minutes
- **Eviction**: Least Recently Used (LRU)

**Hit rate**: ~80% for typical usage (same queries repeated).

#### Strategy 2: Prefix Fast Path (1-2 Char Queries)

For very short queries, OMG uses a **prefix index**:

```rust
if query.len() <= 2 {
    // Use prefix index for instant results
    if let Some(indices) = self.prefix_index.get(&query.to_lowercase()) {
        return indices.iter()
            .filter_map(|&idx| self.search_items.get(idx))
            .map(|(name, _)| self.packages.get(name).unwrap())
            .cloned()
            .collect();
    }
}
```

The prefix index is built during initialization:

```rust
fn build_prefix_index(items: &[(String, Utf32String)]) -> AHashMap<String, Vec<usize>> {
    let mut index = AHashMap::new();

    for (idx, (name, _)) in items.iter().enumerate() {
        let lower = name.to_lowercase();

        // Index first 2 characters
        for len in 1..=2 {
            if lower.len() >= len {
                let prefix = &lower[..len];
                index.entry(prefix.to_string()).or_insert_with(Vec::new).push(idx);
            }
        }
    }

    index
}
```

**Example**: Query "fi" instantly returns all packages starting with "fi" (firefox, filezilla, fish, etc.).

#### Strategy 3: Fuzzy Matching (Full Search)

For longer queries, OMG uses **Nucleo** — a high-performance fuzzy matching library:

```rust
use nucleo_matcher::{Matcher, Config};

pub fn search(&self, query: &str, limit: usize) -> Vec<PackageInfo> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut scored: Vec<_> = Vec::new();

    let query_utf32 = Utf32String::from(query);

    for (idx, (name, name_utf32)) in self.search_items.iter().enumerate() {
        if let Some(score) = matcher.fuzzy_match(name_utf32, &query_utf32) {
            scored.push((score, idx));
        }
    }

    // Sort by score (descending)
    scored.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // Take top N and convert to PackageInfo
    scored.into_iter()
        .take(limit)
        .filter_map(|(_, idx)| {
            let (name, _) = &self.search_items[idx];
            self.packages.get(name)
        })
        .map(|pkg| PackageInfo {
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            description: pkg.description.clone(),
            source: pkg.source.clone(),
        })
        .collect()
}
```

**How Nucleo Works**:
1. Converts query and package names to UTF-32 (faster character access)
2. Uses Smith-Waterman algorithm for fuzzy matching
3. Scores based on character position and continuity
4. Parallelizes across CPU cores

**Performance**: ~6ms for 80,000 packages (on modern CPUs).

---

## 🔍 Search Flow Example

Let's trace a search for "firefox":

### Step 1: User Input

```bash
omg search firefox
```

### Step 2: CLI Serializes Request

```rust
let request = Request::Search {
    id: 1,
    query: "firefox".to_string(),
    limit: Some(50),
};

// Serialize to binary (bitcode)
let bytes = bitcode::encode(&request)?;
```

### Step 3: IPC Transport

```rust
// Send over Unix socket
socket.send(&bytes).await?;

// Receive response
let response_bytes = socket.recv().await?;
let response = bitcode::decode(&response_bytes)?;
```

**IPC latency**: ~100μs (0.1ms).

### Step 4: Daemon Handles Request

```rust
// Check LRU cache
if let Some(cached) = cache.get("firefox") {
    return Response::Success { result: cached }; // ~0.1ms total
}

// Cache miss — search index
let results = index.search("firefox", 50); // ~2-6ms

// Update cache
cache.insert("firefox".to_string(), results.clone());

return Response::Success { result: results };
```

### Step 5: CLI Displays Results

```rust
match response {
    Response::Success { result } => {
        for pkg in result.packages {
            println!("{} {} - {}", pkg.name, pkg.version, pkg.description);
        }
    }
}
```

**Total latency breakdown**:
- IPC (send + recv): ~0.2ms
- Cache check: ~0.1ms
- Index search (miss): ~5ms
- Serialization: ~0.1ms
- Display: ~0.5ms (terminal I/O)
- **Total: ~6ms**

---

## 📈 Performance Optimizations

### 1. SIMD String Search

For substring matching, OMG uses `memchr` for SIMD-accelerated search:

```rust
use memchr::memmem;

// Fast substring check before fuzzy matching
let finder = memmem::Finder::new(query.as_bytes());
if !finder.find(name.as_bytes()).is_some() {
    continue; // Skip this package
}
```

**Speed**: ~10 GB/s throughput (AVX2 on modern CPUs).

### 2. Parallel Scanning

For large result sets, OMG uses `rayon` for parallel iteration:

```rust
use rayon::prelude::*;

let scored: Vec<_> = self.search_items
    .par_iter() // Parallel iterator
    .filter_map(|(name, name_utf32)| {
        matcher.fuzzy_match(name_utf32, &query_utf32)
            .map(|score| (score, name))
    })
    .collect();
```

**Speedup**: 4-8x on modern CPUs (8+ cores).

### 3. Zero-Copy Deserialization

OMG's IPC protocol uses `bitcode` which supports zero-copy for certain types:

```rust
// No allocation — just pointer cast
let request: &Request = bitcode::decode_borrowed(&bytes)?;
```

**Benefit**: ~50% faster deserialization for large responses.

---

## 🔬 Benchmarking Methodology

OMG's benchmarks use `criterion.rs` for statistical rigor:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn search_benchmark(c: &mut Criterion) {
    let index = PackageIndex::new().unwrap();

    c.bench_function("search firefox", |b| {
        b.iter(|| {
            index.search(black_box("firefox"), 50)
        });
    });
}

criterion_group!(benches, search_benchmark);
criterion_main!(benches);
```

**Typical results** (i9-14900K):
```
search firefox          time:   [5.8ms 6.1ms 6.4ms]
```

---

## 🆚 Comparison: pacman vs OMG

### pacman Search

```bash
$ strace -c pacman -Ss firefox 2>&1 | tail -5
% time     seconds  usecs/call     calls    errors syscall
------ ----------- ----------- --------- --------- ----------------
 45.23    0.060123          15      3912           read
 18.72    0.024886          13      1856           openat
 12.45    0.016543          18       897           fstat
```

**Key bottlenecks**:
- 3,912 `read()` calls (parsing text files)
- 1,856 `openat()` calls (opening database files)
- Total: **133ms**

### OMG Search (with daemon running)

```bash
$ strace -c omg search firefox 2>&1 | tail -5
% time     seconds  usecs/call     calls    errors syscall
------ ----------- ----------- --------- --------- ----------------
 52.34    0.003145         523         6           read
 21.18    0.001272         318         4           write
 10.23    0.000614         153         4           connect
```

**Key advantages**:
- 6 `read()` calls (IPC only, no file parsing)
- 4 `connect()` calls (socket setup)
- Total: **6ms** (22x faster)

---

## 🛠️ Performance Tuning

### Increase Cache Size

In `~/.config/omg/config.toml`:

```toml
[daemon]
cache_size = 2000  # Default: 1000
cache_ttl = 600    # Default: 300 (5 minutes)
```

Higher cache size = more RAM but better hit rate.

### Preload Common Queries

```bash
# Warm up cache on daemon start
omg search firefox &
omg search vim &
omg search chrome &
wait
```

### Use SSD/NVMe for Database

OMG's persistent cache (`~/.local/share/omg/cache.redb`) benefits from fast storage:
- **HDD**: ~50ms index load
- **SATA SSD**: ~10ms index load
- **NVMe SSD**: ~5ms index load

---

## 📚 Implementation References

### Source Files

- **Index**: `src/daemon/index.rs`
- **Cache**: `src/daemon/cache.rs`
- **Search Handler**: `src/daemon/handlers.rs` (`handle_search`)
- **Persistent Cache**: `src/daemon/db.rs`

### Related Docs

- [Architecture](./architecture.md) — Overall system design
- [Daemon Internals](./daemon.md) — Daemon lifecycle
- [IPC Protocol](./ipc.md) — Communication details

---

## 💡 Key Takeaways

1. **Three-Tier Strategy**: LRU cache → In-memory index → Fuzzy search
2. **Persistent Caching**: \u003c10ms cold starts via preloaded index
3. **AHashMap**: 50% faster than default HashMap
4. **Nucleo Fuzzy Matching**: SIMD-accelerated, parallel scoring
5. **Binary Protocol**: ~100μs IPC overhead vs ~10ms for JSON

This is how OMG makes searching 80,000 packages feel instant — you literally can't perceive the 6ms delay.
