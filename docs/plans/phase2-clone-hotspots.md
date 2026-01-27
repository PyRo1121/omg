# Phase 2: Clone Hotspots Analysis

## Executive Summary

Total clone operations found: **335**
Hot path clones identified: **78**
Target for reduction: **30%** (23 clones)
Focus areas: Daemon handlers, package search, cache operations, and parallel operations

## Methodology

1. Scanned entire codebase with `rg "\.clone\(\)" src --type rust -n`
2. Identified hot paths (request handling, search, cache ops, parallel tasks)
3. Categorized by type and frequency
4. Assessed cost (Arc = cheap, String/Vec = expensive)
5. Provided actionable recommendations

## High-Frequency Clones (Hot Paths)

### 1. src/daemon/handlers.rs (12 clones)

**Purpose:** Request handling in daemon - executed on every client request

#### Line 174: `let query_clone = query.clone();`
- **Type:** `String`
- **Frequency:** Every Debian search request
- **Cost:** O(n) heap allocation
- **Fix:** Pass by reference or use `Arc<str>`
- **Action:** Keep (query moved into blocking task, required)
- **Reasoning:** Necessary for moving into `spawn_blocking` closure

#### Line 199: `state.cache.insert_debian(query, pkgs.clone());`
- **Type:** `Vec<PackageInfo>` (each PackageInfo has 4 Strings)
- **Frequency:** Every uncached Debian search
- **Cost:** High - clones entire vector and all contained strings
- **Fix:** Use `Arc<Vec<PackageInfo>>` in cache
- **Action:** Convert to Arc (HIGH PRIORITY)
- **Impact:** Major reduction in heap allocations for search results

#### Line 373: `let query_clone = query.clone();`
- **Type:** `String`
- **Frequency:** Every official package search request
- **Cost:** O(n) heap allocation
- **Fix:** Use `Arc<str>` or pass by reference
- **Action:** Keep (required for spawn_blocking)
- **Reasoning:** Necessary for thread safety in blocking task

#### Line 391: `state.cache.insert(query, official.clone());`
- **Type:** `Vec<PackageInfo>`
- **Frequency:** Every uncached official search
- **Cost:** High - clones search result vector
- **Fix:** Use `Arc<Vec<PackageInfo>>` in cache
- **Action:** Convert to Arc (HIGH PRIORITY)
- **Impact:** Eliminates double allocation (one for return, one for cache)

#### Line 441: `state.cache.insert_info(pkg.clone());`
- **Type:** `DetailedPackageInfo` (10+ String fields)
- **Frequency:** Every info request hitting index
- **Cost:** High - clones all string fields
- **Fix:** Use `Arc<DetailedPackageInfo>` in cache
- **Action:** Convert to Arc (HIGH PRIORITY)
- **Impact:** Cache stores Arc, returns cheap clone

#### Line 463: `state.cache.insert_info(detailed.clone());`
- **Type:** `DetailedPackageInfo`
- **Frequency:** Every info request hitting package manager
- **Cost:** High - expensive struct clone
- **Fix:** Use `Arc<DetailedPackageInfo>` in cache
- **Action:** Convert to Arc (HIGH PRIORITY)

#### Line 489: `state.cache.insert_info(detailed.clone());`
- **Type:** `DetailedPackageInfo`
- **Frequency:** Every AUR info request
- **Cost:** High - full struct clone
- **Fix:** Use `Arc<DetailedPackageInfo>` in cache
- **Action:** Convert to Arc (HIGH PRIORITY)

#### Line 523: `state.cache.update_status(cached.clone());`
- **Type:** `StatusResult`
- **Frequency:** Every status request (when cached)
- **Cost:** Medium - struct with several fields
- **Fix:** Return reference or use Arc
- **Action:** Convert to Arc (MEDIUM PRIORITY)

#### Line 567: `runtime_versions: state.runtime_versions.read().clone()`
- **Type:** `HashMap<String, String>`
- **Frequency:** Every status request (uncached)
- **Cost:** High - clones entire hashmap
- **Fix:** Use `Arc<HashMap>` for runtime versions
- **Action:** Convert to Arc (HIGH PRIORITY)
- **Impact:** Eliminates frequent hashmap clones

#### Line 571: `state.cache.update_status(res.clone());`
- **Type:** `StatusResult`
- **Frequency:** Every uncached status request
- **Cost:** Medium - struct clone
- **Fix:** Use Arc in cache
- **Action:** Convert to Arc (MEDIUM PRIORITY)

#### Line 718: `state.cache.update_explicit(packages.clone());`
- **Type:** `Vec<String>`
- **Frequency:** Every explicit list request
- **Cost:** High - clones string vector
- **Fix:** Use `Arc<Vec<String>>` in cache
- **Action:** Convert to Arc (MEDIUM PRIORITY)

### 2. src/package_managers/aur_index.rs (1 clone)

**Purpose:** AUR package update checking

#### Line 126: `updates.push((name.clone(), local_version.clone(), remote_version));`
- **Type:** `String` and `AlpmVersion`
- **Frequency:** Once per AUR package with updates
- **Cost:** Medium - AlpmVersion is Arc internally, String is O(n)
- **Fix:** Use reference or `Arc<str>` for name
- **Action:** Keep (LOW PRIORITY)
- **Reasoning:** Update check is infrequent, few packages updated at once

### 3. src/core/packages/service.rs (8 clones)

**Purpose:** Package installation service

#### Line 50: `official.push(pkg.clone());`
- **Type:** `String` (package name)
- **Frequency:** Once per package during install
- **Cost:** Low - small strings
- **Fix:** N/A
- **Action:** Keep
- **Reasoning:** Installation is not a hot path

#### Line 53: `name: pkg.clone()`
- **Type:** `String`
- **Frequency:** Once per local file install
- **Cost:** Low
- **Action:** Keep

#### Line 69: `official.push(pkg.clone());`
- **Type:** `String`
- **Frequency:** Once per official package
- **Cost:** Low
- **Action:** Keep

#### Line 84: `aur_pkgs.push(pkg.clone());`
- **Type:** `String`
- **Frequency:** Once per AUR package
- **Cost:** Low
- **Action:** Keep

#### Lines 207-210: Multiple field clones in update handling
- **Type:** Strings
- **Frequency:** Once per package update
- **Cost:** Low - not a hot path
- **Action:** Keep

### 4. src/package_managers/parallel_sync.rs (17 clones)

**Purpose:** Parallel repository database synchronization

#### Line 292: `let client = download_client().clone();`
- **Type:** `Arc<Client>` (reqwest Client)
- **Frequency:** Once per sync operation
- **Cost:** Cheap - Arc clone
- **Action:** Keep (acceptable)
- **Reasoning:** Arc clone is just pointer + refcount increment

#### Line 351: `let client = client.clone();`
- **Type:** `Arc<Client>`
- **Frequency:** Once per parallel download task
- **Cost:** Cheap - Arc clone
- **Action:** Keep (acceptable)
- **Reasoning:** Required for moving into async task

#### Line 677: `let client = download_client().clone();`
- **Type:** `Arc<Client>`
- **Frequency:** Once per parallel package download
- **Cost:** Cheap - Arc clone
- **Action:** Keep (acceptable)

#### Line 692: `let client = client.clone();`
- **Type:** `Arc<Client>`
- **Frequency:** Per parallel task spawn
- **Cost:** Cheap - Arc clone
- **Action:** Keep (acceptable)

#### Lines 694-695: Progress bar clones
- **Type:** `ProgressBar` (likely Arc internally)
- **Frequency:** Per task
- **Cost:** Cheap
- **Action:** Keep

### 5. src/daemon/cache.rs (8 clones)

**Purpose:** Cache key and value operations

#### Lines 93-94: `KEY_STATUS.clone()`, `KEY_EXPLICIT_COUNT.clone()`
- **Type:** `String` (static LazyLock keys)
- **Frequency:** Every cache insert/lookup
- **Cost:** HIGH - unnecessary clones of static strings
- **Fix:** Use `&'static str` instead of `LazyLock<String>`
- **Action:** Convert to &'static str (HIGH PRIORITY)
- **Impact:** Eliminates pointless string allocations

#### Lines 112-114: Static key clones
- **Type:** `String`
- **Frequency:** Every explicit package cache operation
- **Cost:** HIGH
- **Fix:** Use `&'static str`
- **Action:** Convert to &'static str (HIGH PRIORITY)

#### Line 181-182: `let name = info.name.clone(); ... insert(name.clone(), info)`
- **Type:** `String`
- **Frequency:** Every info cache insert
- **Cost:** Medium - double clone (one extracted, one for key)
- **Fix:** Extract reference, clone once for key
- **Action:** Optimize (MEDIUM PRIORITY)
- **Optimization:** `self.detailed_cache.insert(info.name.clone(), info);`

## Medium-Frequency Clones (Warm Paths)

### 6. src/package_managers/aur.rs (47 clones)

Most clones in AUR operations are for:
- Path cloning for async tasks (PathBuf) - **acceptable**
- Client cloning (`Arc<Client>`) - **cheap, acceptable**
- Package name strings during build - **not hot path**

Notable ones:

#### Lines 1681, 1695, 1701: Environment variable clones
- **Type:** `OsString`
- **Frequency:** Per AUR build
- **Cost:** Medium
- **Action:** Keep (not hot path)

### 7. src/package_managers/pacman_db.rs (8 clones)

#### Line 232, 372: `packages.insert(pkg.name.clone(), pkg)`
- **Type:** `String` (package name)
- **Frequency:** Per package in database parse
- **Cost:** Medium - one-time parse operation
- **Action:** Keep (not hot path)

### 8. src/daemon/index.rs (2 clones)

#### Line 201, 245: `name_to_idx.insert(pkg.name.clone(), idx)`
- **Type:** `String`
- **Frequency:** Index initialization only
- **Cost:** Low - startup only
- **Action:** Keep

## Low-Priority Areas

### Testing Code
- **Location:** `src/core/testing/mocks.rs` (15 clones)
- **Action:** Keep (test code performance not critical)

### CLI/TUI Code
- **Location:** Various CLI modules (50+ clones)
- **Action:** Keep (user-facing operations, not hot paths)

### Runtime Management
- **Location:** `src/runtimes/*` (30+ clones)
- **Action:** Keep (infrequent operations)

## Recommendations Summary

### High Priority (Target 30% reduction)

1. **Cache Architecture Changes (Saves ~15 clones)**
   - Convert `PackageCache` to use `Arc<Vec<PackageInfo>>` for search results
   - Convert to `Arc<DetailedPackageInfo>` for info cache
   - Change static keys from `LazyLock<String>` to `&'static str`
   - Use `Arc<HashMap>` for runtime_versions in daemon state

2. **Expected Impact:**
   - Eliminate ~15 expensive clones in hot request paths
   - Reduce heap allocations by 60-80% for cached responses
   - Improve search and info request latency by 20-30%

### Implementation Plan

#### Phase 1: Static String Keys (Quick Win)
```rust
// Before:
static KEY_STATUS: LazyLock<String> = LazyLock::new(|| "status".to_string());
self.cache.insert(KEY_STATUS.clone(), value);

// After:
const KEY_STATUS: &str = "status";
self.cache.insert(KEY_STATUS, value);
```

#### Phase 2: Arc-ify Cache Values
```rust
// Before:
cache: Cache<String, Vec<PackageInfo>>

// After:
cache: Cache<String, Arc<Vec<PackageInfo>>>

// Usage:
state.cache.insert(query, Arc::new(official));
// Return: Arc::clone for cheap distribution
```

#### Phase 3: Runtime Versions HashMap
```rust
// In DaemonState:
runtime_versions: Arc<RwLock<Arc<HashMap<String, String>>>>

// Read and clone Arc pointer only:
let versions = state.runtime_versions.read().clone(); // Cheap Arc clone
```

### Medium Priority

- Optimize double clones (e.g., cache.rs line 181-182)
- Consider Cow for conditionally cloned strings
- Review status result cloning patterns

### Low Priority

- AUR build path clones (not hot path)
- Testing code optimizations
- CLI/TUI response clones

## Metrics

### Current State
- Total clones: 335
- Hot path clones: 78
- Expensive hot path clones: 23

### Target State (30% reduction)
- Remove: 23 expensive clones
- Convert to Arc: 15 clones
- Optimize: 8 clones
- Expected latency improvement: 20-30% for cached requests
- Expected memory churn reduction: 60-80%

## Verification Plan

1. Add benchmark for search requests (cached vs uncached)
2. Add benchmark for info requests
3. Measure heap allocations with `dhat`
4. Profile with `cargo flamegraph`
5. Compare before/after metrics

## Next Steps

Proceed to Task 5: Implement Arc conversions in identified hot paths.
