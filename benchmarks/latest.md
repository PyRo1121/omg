# OMG Benchmark Report

**Iterations:** 5  
**Warmup:** 1

## Methodology & Fairness

This benchmark follows fair comparison principles:

- **yay**: Uses `--repo` flag to skip AUR network calls
- **All tools**: Equal warmup iterations before measurement
- **OMG Daemon**: In-memory indexed search (architectural advantage)
- **pacman/yay**: Direct disk access each call (no caching)

### What We're Comparing

| Tool | Architecture | Cache |
|------|--------------|-------|
| OMG (Daemon) | Unix socket IPC + in-memory index | Hot (pre-loaded) |
| pacman | Direct ALPM library calls | Cold (disk) |
| yay | pacman wrapper | Cold (disk) |

## Test Environment

- **OS:** Linux
- **Kernel:** 6.14.0-1017-azure
- **CPU:**                              AMD EPYC 7763 64-Core Processor
- **CPU Cores:**                                  4
- **RAM:** 15Gi

## Results

| Command | OMG (Daemon) | pacman | yay | Speedup vs pacman |
|---------|--------------|--------|-----|-------------------|
| search | 13.00ms | 227.00ms | ms | 17.4x |
| info | 11.40ms | 209.80ms | ms | 18.4x |
| status | 10.00ms | N/Ams | N/Ams | N/A |
| explicit | 3.00ms | 16.00ms | ms | 5.3x |

## Analysis

OMG's performance advantage comes from its **daemon architecture**:

1. **Pre-indexed database**: Package metadata loaded into memory at daemon start
2. **Unix socket IPC**: Sub-millisecond communication vs process spawn overhead
3. **In-memory fuzzy search**: No disk I/O during queries

This is a **fair architectural comparison** - OMG chose a different design that
trades memory usage (~50MB) for query speed. pacman and yay are designed for
lower memory footprint with on-demand disk access.
