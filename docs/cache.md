# Caching & Indexing

OMG uses a multi-layered caching strategy to deliver sub-millisecond response times. This covers the in-memory LRU cache, persistent LMDB cache, and the official package index with fuzzy search.

## In-Memory LRU Cache (`PackageCache`)

### Architecture Overview

The `PackageCache` is a high-performance, thread-safe caching structure designed for concurrent read/write operations. It uses a combination of `DashMap` (concurrent hash map) and `RwLock<VecDeque>` to implement an LRU eviction policy with TTL support.

### Data Structures

```rust
pub struct PackageCache {
    /// Search results cache: query -> packages
    cache: DashMap<String, CacheEntry>,
    /// Detailed info cache: pkgname -> info
    detailed_cache: DashMap<String, DetailedEntry>,
    /// LRU order tracking (critical for eviction)
    lru_order: RwLock<VecDeque<String>>,
    /// Maximum cache size (default: 1000 entries)
    max_size: usize,
    /// Cache TTL (default: 5 minutes)
    ttl: Duration,
    /// System status cache with separate TTL logic
    system_status: RwLock<Option<(StatusResult, Instant)>>,
    /// Explicit package list cache
    explicit_packages: RwLock<Option<(Vec<String>, Instant)>>,
}
```

### Cache Entry Types

#### `CacheEntry` - Search Results
```rust
struct CacheEntry {
    packages: Vec<PackageInfo>,
    timestamp: Instant,
}
```
- Stores complete search result vectors
- Timestamp enables TTL-based expiration
- Cloned on retrieval (immutable data structure)

#### `DetailedEntry` - Package Info
```rust
struct DetailedEntry {
    info: DetailedPackageInfo,
    timestamp: Instant,
}
```
- Stores individual package detailed information
- Used by `omg info <package>` commands
- Separate cache from search results for granular control

### LRU Eviction Algorithm

The LRU implementation uses a `VecDeque<String>` to track access order:

1. **Insertion**: New keys are appended to the back of the deque (most recent position)
2. **Access**: The `touch()` method removes the key from its current position and re-appends it to the back
3. **Eviction**: When capacity is reached, `evict_oldest()` pops from the front (oldest position)

This approach provides O(1) insertion and O(n) eviction (where n is typically small due to the deque's localized operations).

### TTL Management

TTL is checked on every read operation with lazy expiration:
- Search cache entries expire after 5 minutes (300 seconds)
- System status has independent TTL checking
- Expired entries are removed on access (not proactively)

```rust
if entry.timestamp.elapsed() > self.ttl {
    drop(entry);
    self.cache.remove(query);
    return None;
}
```

### Concurrency Model

The cache uses multiple synchronization primitives:

- **`DashMap`**: Lock-free concurrent hashmap for cache storage
- **`RwLock<VecDeque>`**: Protects LRU order tracking (write-heavy during eviction)
- **`AtomicU64`**: For request ID generation in IPC client

This design allows unlimited concurrent readers while ensuring consistency during writes.

### Cache Statistics

The cache provides runtime statistics:
```rust
pub fn stats(&self) -> CacheStats {
    CacheStats {
        entries: self.cache.len(),
        detailed_entries: self.detailed_cache.len(),
        max_size: self.max_size,
    }
}
```

## Persistent Cache (`PersistentCache`)

### LMDB Integration

The persistent cache uses LMDB (Lightning Memory-Mapped Database) for durability across daemon restarts:

```rust
pub struct PersistentCache {
    env: Env,  // LMDB environment
    status_db: Database<Str, SerdeJson<StatusResult>>,
}
```

### Configuration

- **Map size**: 10MB (sufficient for status metadata)
- **Database**: Single named database "status"
- **Serialization**: `bincode` for compact binary storage
- **Transactions**: ACID compliance with read/write transactions

### Operations

```rust
pub fn get_status(&self) -> Result<Option<StatusResult>>
pub fn set_status(&self, status: &StatusResult) -> Result<()>
```

The persistent cache stores only system status results, as other data is cheap to regenerate from libalpm.

## Official Package Index (`PackageIndex`)

### Data Architecture

The package index maintains two synchronized data structures:

```rust
pub struct PackageIndex {
    /// Maps package name to detailed info (using ahash for speed)
    packages: AHashMap<String, DetailedPackageInfo>,
    /// Search items for Nucleo (pre-computed UTF-32 strings)
    search_items: Vec<(String, Utf32String)>,
    /// Reader-writer lock for package lookups
    lock: RwLock<()>,
}
```

### Index Building Process

On startup, the index:

1. **Initialize ALPM**: Connects to libalpm at `/var/lib/pacman`
2. **Register Databases**: Loads `core`, `extra`, and `multilib` repositories
3. **Extract Metadata**: For each package, collects:
   - Name, version, description, URL
   - Installed size and download size
   - Repository source
   - Dependencies (depends, makedepends, optdepends, checkdepends)
   - Conflicts and provides
4. **Generate Search Strings**: Concatenates name and description for fuzzy matching
5. **UTF-32 Conversion**: Pre-converts to UTF-32 for Nucleo matcher efficiency

### Search Algorithm

#### Fast Path: Prefix Matching (1-2 character queries)
```rust
if query.len() < 3 {
    let matches: Vec<_> = self
        .search_items
        .iter()
        .filter(|(name, _)| name.starts_with(query))
        .take(limit)
        // ... map to PackageInfo
}
```

This optimization handles the common case of users typing short prefixes like 'f' or 'fi'.

#### Fuzzy Matching (3+ character queries)

Uses Rayon for parallel processing and Nucleo for high-performance fuzzy matching:

1. **Parallel Preparation**: Convert query to UTF-32 slice
2. **Parallel Matching**: Rayon processes search items in parallel
   ```rust
   let matches: Vec<_> = self
       .search_items
       .par_iter()  // Parallel iterator
       .enumerate()
       .filter_map(|(idx, (_, search_str))| {
           Matcher::new(Config::DEFAULT)
               .fuzzy_match(query_slice, search_str.slice(..))
               .map(|score| (score, idx))
       })
       .collect();
   ```
3. **Optimized Sorting**: Uses `select_nth_unstable_by` for O(n) partial sort when results exceed limit
4. **Final Mapping**: Maps matched indices back to package info

### Performance Characteristics

- **Index size**: ~15MB for full Arch repository
- **Search latency**: <1ms for typical queries
- **Memory usage**: AHashMap for O(1) lookups, minimal allocations
- **Concurrency**: RwLock allows multiple concurrent readers

### Search String Construction

```rust
let search_str = format!("{} {}", info.name, info.description);
```

This strategy allows users to find packages by either name or description keywords.

## Cache Interaction Patterns

### Search Request Flow

1. **Cache Check**: `PackageCache.get(query)` with TTL validation
2. **Index Search**: If cache miss, search `PackageIndex`
3. **AUR Fallback**: Query AUR if official results insufficient
4. **Cache Storage**: Store results in `PackageCache` with current timestamp
5. **LRU Update**: Move query to most recent position in LRU deque

### Info Request Flow

1. **Detailed Cache Check**: `PackageCache.get_info(name)`
2. **Index Lookup**: If miss, use `PackageIndex.packages.get(name)`
3. **Cache Storage**: Store in detailed cache for future requests

### System Status Flow

1. **Persistent Cache**: Check LMDB for stored status
2. **TTL Validation**: Verify if status is still fresh
3. **Live Generation**: If expired, generate new system status
4. **Dual Storage**: Update both persistent cache and in-memory cache

## Design Rationale

### Memory vs. Computation Trade-offs

- **Pre-computation**: UTF-32 strings and package metadata are pre-computed
- **Lazy Loading**: AUR results are fetched on-demand
- **Selective Persistence**: Only system status persists across restarts

### Concurrency Strategy

- **Lock-free Reads**: DashMap enables unlimited concurrent readers
- **Minimal Lock Contention**: LRU tracking uses separate lock from cache storage
- **Parallel Processing**: Rayon parallelizes fuzzy matching

### Performance Targets

- **Cache Hit**: <0.1ms (memory access)
- **Index Search**: <1ms (in-memory fuzzy matching)
- **Cache Miss with AUR**: <100ms (network bound)
- **Daemon Restart**: <5s (index rebuild from libalpm)

## Cache Configuration

### Default Values

```rust
impl Default for PackageCache {
    fn default() -> Self {
        // 1000 entries, 5 minute TTL
        Self::new(1000, 300)
    }
}
```

### Tuning Considerations

- **Cache Size**: Larger values increase memory usage but reduce AUR calls
- **TTL Duration**: Shorter TTL provides fresher data but more cache misses
- **LMDB Size**: Must accommodate system status with room for growth

## Monitoring and Debugging

### Cache Statistics

Monitor cache effectiveness through:
- Hit/miss ratios (not directly tracked, can be added)
- Eviction frequency
- Memory usage patterns

### Common Issues

1. **Memory Leaks**: Ensure LRU eviction is working correctly
2. **Stale Data**: Verify TTL expiration is functioning
3. **Lock Contention**: Monitor RwLock contention in high-load scenarios

## Future Optimizations

### Potential Enhancements

1. **Adaptive TTL**: Dynamic TTL based on query frequency
2. **Compression**: Compress cached package data to increase capacity
3. **Sharding**: Partition cache by query pattern for reduced contention
4. **Persistent Search Cache**: Extend LMDB to store frequent search results
5. **Predictive Prefetching**: Cache related packages based on access patterns

### Scaling Considerations

- **Multi-repo Support**: Index structure supports additional repositories
- **Distributed Cache**: Could extend to Redis for multi-node scenarios
- **Snapshot Support**: LMDB allows for consistent snapshots

Source: `src/daemon/index.rs`.
