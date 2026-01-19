---
title: Caching & Indexing
sidebar_position: 32
description: In-memory and persistent caching strategies
---

# Caching & Indexing

OMG uses a multi-layered caching strategy to deliver sub-millisecond response times. This covers the in-memory moka cache, persistent redb storage, and the official package index with Nucleo fuzzy search.

## In-Memory Cache (`PackageCache`)

### Architecture Overview

The `PackageCache` uses `moka`, a high-performance, lock-free concurrent caching library for Rust. Moka provides built-in TTL expiration, size-based eviction, and excellent concurrent performance.

### Data Structures

```rust
pub struct PackageCache {
    /// Search results cache: query -> packages
    cache: Cache<String, Vec<PackageInfo>>,
    /// Detailed info cache: pkgname -> info
    detailed_cache: Cache<String, DetailedPackageInfo>,
    /// Negative cache for missing package info
    info_miss_cache: Cache<String, bool>,
    /// Maximum cache size
    max_size: usize,
    /// System status cache (single entry)
    system_status: Cache<String, StatusResult>,
    /// Explicit package list cache (single entry)
    explicit_packages: Cache<String, Vec<String>>,
}
```

### Cache Configuration

Each cache is configured with:
- **max_capacity**: Maximum number of entries
- **time_to_live**: TTL for automatic expiration

```rust
let cache = Cache::builder()
    .max_capacity(max_size as u64)
    .time_to_live(ttl)
    .build();
```

### Default Settings

```rust
impl Default for PackageCache {
    fn default() -> Self {
        // 1000 entries, 5 minute TTL for search/info
        // 30 second TTL for status cache
        Self::new_with_ttls(1000, 300, 30)
    }
}
```

| Cache | Max Size | TTL |
|-------|----------|-----|
| Search results | 1000 queries | 5 minutes |
| Detailed info | 1000 packages | 5 minutes |
| Info miss (negative) | 1000 entries | 5 minutes |
| System status | 1 entry | 30 seconds |
| Explicit packages | 1 entry | 30 seconds |

### Cache Operations

#### Search Results
```rust
// Get cached results
pub fn get(&self, query: &str) -> Option<Vec<PackageInfo>> {
    self.cache.get(query)
}

// Store results
pub fn insert(&self, query: String, packages: Vec<PackageInfo>) {
    self.cache.insert(query, packages);
}
```

#### Package Info
```rust
// Get detailed info
pub fn get_info(&self, name: &str) -> Option<DetailedPackageInfo> {
    self.detailed_cache.get(name)
}

// Check negative cache (known missing)
pub fn is_info_miss(&self, name: &str) -> bool {
    self.info_miss_cache.get(name).unwrap_or(false)
}

// Store info (and clear negative cache)
pub fn insert_info(&self, info: DetailedPackageInfo) {
    let name = info.name.clone();
    self.detailed_cache.insert(name.clone(), info);
    self.info_miss_cache.invalidate(&name);
}

// Record a miss
pub fn insert_info_miss(&self, name: &str) {
    self.info_miss_cache.insert(name.to_string(), true);
}
```

#### System Status
```rust
pub fn get_status(&self) -> Option<StatusResult> {
    self.system_status.get("status")
}

pub fn update_status(&self, result: StatusResult) {
    self.system_status.insert("status".to_string(), result);
}
```

### Cache Statistics

```rust
pub fn stats(&self) -> CacheStats {
    CacheStats {
        size: self.cache.entry_count() as usize,
        max_size: self.max_size,
    }
}
```

### Cache Clearing

```rust
pub fn clear(&self) {
    self.cache.invalidate_all();
    self.detailed_cache.invalidate_all();
    self.info_miss_cache.invalidate_all();
    self.system_status.invalidate_all();
    self.explicit_packages.invalidate_all();
}
```

## Persistent Cache (`PersistentCache`)

### redb Integration

The persistent cache uses redb, a pure Rust embedded database, for durability across daemon restarts:

```rust
pub struct PersistentCache {
    db: Database,
}

const STATUS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("status");
```

### Features

- **Pure Rust**: No C dependencies
- **ACID Transactions**: Full transactional support
- **Automatic Sizing**: No manual configuration needed
- **Crash Safety**: Data survives unexpected shutdowns

### Database Location

```
~/.local/share/omg/cache.redb
```

### Operations

```rust
impl PersistentCache {
    pub fn new(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        let db_path = path.join("cache.redb");
        let db = Database::create(&db_path)?;
        Ok(Self { db })
    }

    pub fn get_status(&self) -> Result<Option<StatusResult>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(STATUS_TABLE)?;
        
        match table.get("current")? {
            Some(guard) => {
                let status: StatusResult = serde_json::from_slice(guard.value())?;
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    pub fn set_status(&self, status: &StatusResult) -> Result<()> {
        let data = serde_json::to_vec(status)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(STATUS_TABLE)?;
            table.insert("current", data.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }
}
```

### What's Persisted

Currently, only system status is persisted:
- Total package count
- Explicit package count
- Orphan count
- Updates available
- Security vulnerabilities count
- Runtime versions

This allows fast daemon startup without re-querying everything.

## Official Package Index (`PackageIndex`)

### Data Architecture

The package index maintains several synchronized data structures for optimal query performance:

```rust
pub struct PackageIndex {
    /// Maps package name to detailed info (using ahash for speed)
    packages: AHashMap<String, DetailedPackageInfo>,
    /// Search items with UTF-32 strings for Nucleo
    search_items: Vec<(String, Utf32String)>,
    /// Lowercased versions for case-insensitive search
    search_items_lower: Vec<Utf32String>,
    /// Prefix index for 1-2 character fast path
    prefix_index: AHashMap<String, Vec<usize>>,
    /// Reader-writer lock for thread safety
    lock: RwLock<()>,
}
```

### Index Building

On daemon startup, the index is built from system databases:

#### Arch Linux (ALPM)
```rust
for db_name in ["core", "extra", "multilib"] {
    let db = alpm.register_syncdb(db_name, SigLevel::USE_DEFAULT)?;
    for pkg in db.pkgs() {
        // Extract: name, version, description, URL, size,
        // dependencies, licenses, etc.
        packages.insert(info.name.clone(), info);
    }
}
```

#### Debian/Ubuntu (APT)
```rust
let cache = Cache::new(&[])?;
for pkg in cache.packages(&PackageSort::default()) {
    // Extract from candidate or installed version
    packages.insert(info.name.clone(), info);
}
```

### Search Algorithm

#### Fast Path: Prefix Matching (1-2 characters)

For short queries, the prefix index provides instant results:

```rust
if query_lower.len() < 3 {
    let matches = self.prefix_index
        .get(&query_lower)
        .into_iter()
        .flatten()
        .take(limit)
        .filter_map(|idx| {
            let name = &self.search_items[*idx].0;
            self.packages.get(name).map(to_package_info)
        })
        .collect();
    
    if !matches.is_empty() {
        return matches;
    }
}
```

#### Fuzzy Matching (3+ characters)

Uses Rayon for parallel processing and Nucleo for high-performance fuzzy matching:

```rust
// 1. Create thread-local matchers for parallelism
let mut matches: Vec<(u16, usize)> = self.search_items_lower
    .par_iter()
    .enumerate()
    .map_init(
        || Matcher::new(Config::DEFAULT),  // Per-thread matcher
        |matcher, (idx, search_str)| {
            matcher.fuzzy_match(query_slice, search_str.slice(..))
                .map(|score| (score, idx))
        },
    )
    .flatten()
    .collect();

// 2. Optimized partial sort for large result sets
if matches.len() > limit {
    matches.select_nth_unstable_by(limit, |a, b| b.0.cmp(&a.0));
    matches.truncate(limit);
}

// 3. Sort by score (descending)
matches.sort_unstable_by(|a, b| b.0.cmp(&a.0));

// 4. Map indices back to package info
results = matches.iter()
    .filter_map(|(_, idx)| self.packages.get(&self.search_items[*idx].0))
    .map(to_package_info)
    .collect();
```

### Search String Construction

Each package's searchable text combines name and description:

```rust
let search_str = format!("{} {}", info.name, info.description);
let search_lower = search_str.to_lowercase();
```

This allows users to find packages by either name or description keywords.

### Performance Characteristics

| Operation | Time Complexity | Typical Latency |
|-----------|-----------------|-----------------|
| Get by name | O(1) | < 1μs |
| Prefix search | O(1) lookup + O(k) iteration | < 100μs |
| Fuzzy search | O(n) parallel | < 1ms |
| Index build | O(n) | ~1-2s on startup |

Where n = number of packages (~15,000 for Arch), k = matches

### Memory Usage

- **Package data**: ~15MB for full Arch repository
- **Search strings**: ~10MB (UTF-32 for Nucleo)
- **Prefix index**: ~2MB
- **Total**: ~25-30MB

## Cache Interaction Patterns

### Search Request Flow

```
Client Request
     │
     ▼
┌─────────────────┐
│ Check moka      │ ─── Hit ───▶ Return cached
│ search cache    │
└────────┬────────┘
         │ Miss
         ▼
┌─────────────────┐
│ Search          │
│ PackageIndex    │
└────────┬────────┘
         │ < 5 results
         ▼
┌─────────────────┐
│ Query AUR       │
│ (network)       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Combine results │
│ Store in cache  │
└────────┬────────┘
         │
         ▼
    Return
```

### Info Request Flow

```
Client Request
     │
     ▼
┌─────────────────┐
│ Check moka      │ ─── Hit ───▶ Return cached
│ detailed_cache  │
└────────┬────────┘
         │ Miss
         ▼
┌─────────────────┐
│ Check negative  │ ─── Known miss ───▶ Return error
│ info_miss_cache │
└────────┬────────┘
         │ Unknown
         ▼
┌─────────────────┐
│ Lookup in       │ ─── Found ───▶ Cache & return
│ PackageIndex    │
└────────┬────────┘
         │ Not found
         ▼
┌─────────────────┐
│ Query AUR       │ ─── Found ───▶ Cache & return
│ (network)       │
└────────┬────────┘
         │ Not found
         ▼
┌─────────────────┐
│ Record miss     │
│ Return error    │
└─────────────────┘
```

### Status Request Flow

```
Client Request
     │
     ▼
┌─────────────────┐
│ Check redb      │ ─── Found ───▶ Return persisted
│ persistent DB   │
└────────┬────────┘
         │ Not found
         ▼
┌─────────────────┐
│ Check moka      │ ─── Hit ───▶ Return cached
│ system_status   │
└────────┬────────┘
         │ Miss
         ▼
┌─────────────────┐
│ Generate fresh  │
│ status via ALPM │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Store in both   │
│ redb + moka     │
└────────┬────────┘
         │
         ▼
    Return
```

## Tuning Considerations

### Cache Size

- **Larger cache**: More memory, fewer AUR calls
- **Smaller cache**: Less memory, more cache misses
- **Default (1000)**: Good balance for most systems

### TTL Duration

- **Shorter TTL**: Fresher data, more regeneration
- **Longer TTL**: Staler data, better performance
- **Search TTL (5 min)**: Package data rarely changes
- **Status TTL (30 sec)**: Quick updates for dashboards

### Monitoring

Available via daemon IPC:
```bash
# Cache stats (requires daemon)
omg daemon  # If not running
# Use Request::CacheStats
```

Returns:
- Current entry count
- Maximum size

## Source Files

| File | Purpose |
|------|---------|
| [daemon/cache.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/daemon/cache.rs) | PackageCache implementation with moka |
| [daemon/db.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/daemon/db.rs) | PersistentCache with redb |
| [daemon/index.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/daemon/index.rs) | PackageIndex with Nucleo search |
| [daemon/handlers.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/daemon/handlers.rs) | Cache interaction in request handlers |
