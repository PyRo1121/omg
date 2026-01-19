---
title: Package Search Flow
sidebar_label: Package Search
sidebar_position: 23
description: Search indexing and ranking algorithms
---

# Package Search Flow

OMG's search system combines instant official package indexing with conditional AUR fallback to deliver sub-millisecond response times for common packages while still providing comprehensive results.

## Search Request Flow

### Request Routing

All search requests enter through `handle_search`:

```rust
async fn handle_search(
    state: Arc<DaemonState>,
    id: RequestId,
    query: String,
    limit: Option<usize>,
) -> Response {
    let limit = limit.unwrap_or(50);
    // ... search logic
}
```

### Step 1: Cache Check (Sub-millisecond)

The first stop is always the in-memory moka cache:

```rust
if let Some(cached) = state.cache.get(&query) {
    let packages: Vec<_> = cached.into_iter().take(limit).collect();
    let total = packages.len();
    return Response::Success {
        id,
        result: ResponseResult::Search(SearchResult { packages, total }),
    };
}
```

Cache characteristics:
- **Hit Rate**: ~80% for repeated queries
- **Latency**: &lt;0.1ms (memory access)
- **TTL**: 5 minutes for freshness
- **Capacity**: 1000 cached queries

### Step 2: Official Index Search (Sub-millisecond)

On cache miss, the daemon searches the official package index:

```rust
let official = state.index.search(&query, limit);
```

Index search process:
1. **Query Analysis**: Determine if fuzzy or prefix match
2. **Parallel Processing**: Rayon parallelizes across packages
3. **Scoring**: Nucleo matcher provides relevance scores
4. **Sorting**: Results sorted by score, then truncated

Performance metrics:
- **Latency**: &lt;1ms for typical queries
- **Index Size**: ~15MB (full Arch repository)
- **Package Count**: ~15,000 packages
- **Search Algorithm**: Fuzzy matching with prefix optimization

### Step 3: Conditional AUR Search (50-200ms)

AUR search is only triggered when official results are insufficient:

```rust
let mut aur = Vec::new();
if official.len() < 5 {
    if let Ok(aur_pkgs) = state.aur.search(&query).await {
        for pkg in aur_pkgs {
            aur.push(PackageInfo {
                name: pkg.name,
                version: pkg.version,
                description: pkg.description,
                source: "aur".to_string(),
            });
        }
    }
}
```

AUR search characteristics:
- **Trigger**: &lt;5 official results
- **Latency**: 50-200ms (network bound)
- **API**: AUR RPC endpoint
- **Rate Limiting**: Respect AUR limits
- **Error Handling**: Graceful fallback on failure

### Step 4: Result Aggregation

Results from both sources are combined:

```rust
let mut packages: Vec<PackageInfo> = 
    Vec::with_capacity(official.len() + aur.len());
packages.extend(official);
packages.extend(aur);
```

Aggregation rules:
- **Priority**: Official packages first
- **Deduplication**: Not performed (different sources)
- **Ordering**: Maintains source ordering
- **Limiting**: Applied after aggregation

### Step 5: Cache Storage

Successful searches are cached for future requests:

```rust
state.cache.insert(query, packages.clone());
```

Caching behavior:
- **Storage**: Full result set cached
- **Key**: Exact query string
- **Eviction**: LRU when cache is full
- **TTL**: 5 minutes from insertion

### Step 6: Response Formatting

Final response is prepared and sent:

```rust
let total = packages.len();
let packages: Vec<_> = packages.into_iter().take(limit).collect();

Response::Success {
    id,
    result: ResponseResult::Search(SearchResult { packages, total }),
}
```

Response format:
- **packages**: Truncated result list
- **total**: Total available results
- **source**: "official" or "aur" per package

## Info Request Flow

### Cache-First Strategy

Info requests prioritize cache hits:

```rust
async fn handle_info(state: Arc<DaemonState>, id: RequestId, package: String) -> Response {
    // 1. Check cache first
    if let Some(cached) = state.cache.get_info(&package) {
        return Response::Success {
            id,
            result: ResponseResult::Info(cached),
        };
    }
    // ... fallback logic
}
```

### Official Index Lookup

Cache misses trigger direct index lookup:

```rust
if let Some(pkg) = state.index.get(&package) {
    state.cache.insert_info(pkg.clone());
    return Response::Success {
        id,
        result: ResponseResult::Info(pkg),
    };
}
```

Index lookup characteristics:
- **Operation**: O(1) hash map lookup
- **Data**: Complete package metadata
- **Caching**: Result cached for future requests
- **Fallback**: AUR if not found locally

### AUR Fallback

For packages not in official repos:

```rust
if let Ok(aur_pkg) = state.aur.info(&package).await {
    let pkg = DetailedPackageInfo {
        name: aur_pkg.name,
        version: aur_pkg.version,
        description: aur_pkg.description,
        // ... map AUR fields to common format
        source: "aur".to_string(),
    };
    return Response::Success {
        id,
        result: ResponseResult::Info(pkg),
    };
}
```

## Status Request Flow

### Dual Cache Strategy

Status requests use both persistent and in-memory caches:

```rust
async fn handle_status(state: Arc<DaemonState>, id: RequestId) -> Response {
    // 1. Check persistent cache (redb)
    if let Ok(Some(cached)) = state.persistent.get_status() {
        return Response::Success {
            id,
            result: ResponseResult::Status(cached),
        };
    }

    // 2. Check in-memory cache
    if let Some(cached) = state.cache.get_status() {
        return Response::Success {
            id,
            result: ResponseResult::Status(cached),
        };
    }
    // ... generate fresh status
}
```

### Status Generation

On cache miss, status is generated live:

```rust
let status = get_system_status().await?;
state.cache.update_status(status.clone());
state.persistent.set_status(&status)?;
```

Status includes:
- **System Information**: OS, kernel, architecture
- **Package Counts**: Total, explicit, dependencies
- **Vulnerability Summary**: Total and high-severity CVEs
- **Runtime Versions**: Active versions for all runtimes
- **Disk Usage**: Cache sizes, free space

## Explicit Package List Flow

### Cached Package Listing

Explicit packages (user-installed) are cached:

```rust
async fn handle_list_explicit(_state: Arc<DaemonState>, id: RequestId) -> Response {
    if let Some(cached) = _state.cache.get_explicit() {
        return Response::Success {
            id,
            result: ResponseResult::Explicit(ExplicitResult { packages: cached }),
        };
    }
    // ... generate list
}
```

### Fast Package Enumeration

Uses optimized libalpm operations:

```rust
match list_explicit_fast() {
    Ok(packages) => {
        _state.cache.update_explicit(packages.clone());
        Response::Success {
            id,
            result: ResponseResult::Explicit(ExplicitResult { packages }),
        }
    }
    Err(e) => Response::Error { /* ... */ }
}
```

Optimization techniques:
- **Direct libalpm**: No subprocess overhead
- **Filtered Query**: Only explicit packages
- **Caching**: Results cached for 5 minutes
- **Incremental Updates**: Could be implemented

## Security Audit Flow

### Parallel Vulnerability Scanning

Security audits use parallel processing:

```rust
async fn handle_security_audit(_state: Arc<DaemonState>, id: RequestId) -> Response {
    let scanner = Arc::new(VulnerabilityScanner::new());
    let installed = list_installed_fast()?;
    let mut set = tokio::task::JoinSet::new();
    
    for chunk in installed.chunks(10) {
        let scanner = Arc::clone(&scanner);
        set.spawn(async move {
            // Scan chunk for vulnerabilities
        });
    }
    
    // Collect and aggregate results
}
```

Parallel processing benefits:
- **Chunking**: 10 packages per task
- **Concurrency**: Up to 8 concurrent scans
- **Aggregation**: Results combined on completion
- **Timeout**: Per-package timeout to prevent hangs

### Severity Filtering

Only high-severity vulnerabilities are reported:

```rust
for vuln in vulnerabilities {
    if vuln.severity >= 7.0 {
        high_severity += 1;
        // Include in results
    }
    total_vulns += 1;
}
```

Severity thresholds:
- **High**: >=7.0 (CVSS)
- **Medium**: 4.0-6.9
- **Low**: &lt;4.0
- **Reporting**: Only high severity in summary

## Performance Optimization

### Search Optimization Strategies

1. **Prefix Fast Path**: Short queries use exact prefix matching
2. **Parallel Processing**: Rayon parallelizes fuzzy matching
3. **Result Limiting**: Early truncation for large result sets
4. **Caching**: Aggressive caching of common queries

### Cache Hit Optimization

- **Query Normalization**: Lowercase, trim whitespace
- **Fuzzy Matching**: Cache exact queries only
- **TTL Balancing**: 5 minutes balances freshness and performance
- **Size Management**: 1000 entry limit prevents memory bloat

### Network Optimization

- **Conditional AUR**: Only when needed
- **Connection Pooling**: Reuse HTTP connections
- **Timeout Handling**: 5-second AUR timeout
- **Error Recovery**: Graceful fallback on failures

## Error Handling

### Search Error Cases

1. **Empty Query**: Returns empty results
2. **Invalid Characters**: Handled gracefully
3. **AUR Failure**: Falls back to official only
4. **Index Error**: Direct libalpm fallback

### Response Error Codes

- **PACKAGE_NOT_FOUND**: Info request for non-existent package
- **INTERNAL_ERROR**: System failures during processing
- **INVALID_PARAMS**: Malformed request parameters
- **TIMEOUT**: AUR or external service timeout

## Monitoring and Metrics

### Search Performance Metrics

- **Cache Hit Rate**: Percentage of cache hits
- **Response Latency**: Per-operation timing
- **Result Counts**: Average results per query
- **AUR Usage**: Frequency of AUR fallbacks

### Alerting Thresholds

- **Latency >100ms**: Potential performance issue
- **Cache Hit Rate &lt;50%**: Cache ineffective
- **Error Rate >5%**: System problems
- **AUR Failures >10%**: Network issues

## Future Enhancements

### Search Improvements

1. **Semantic Search**: Package description analysis
2. **Popularity Ranking**: Download statistics
3. **Dependency Graph**: Related package suggestions
4. **Personalization**: User-specific result ranking

### Caching Enhancements

1. **Persistent Search Cache**: redb for frequent queries
2. **Intelligent Preloading**: Predictive caching
3. **Distributed Cache**: Multi-daemon sharing
4. **Compression**: Reduce memory usage

### Performance Optimizations

1. **Incremental Updates**: Delta updates for package index
2. **Background Refresh**: Non-blocking cache updates
3. **Result Streaming**: Large result sets streamed
4. **Edge Caching**: Local cache nodes
