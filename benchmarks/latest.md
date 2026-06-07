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
- **Kernel:** 6.17.0-1015-azure
- **CPU:**                              Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz
- **CPU Cores:**                                  4
- **RAM:** 15Gi

## Results

| Command | OMG (Daemon) | pacman | yay | Speedup vs pacman |
|---------|--------------|--------|-----|-------------------|
| search | 9.00ms | 206.00ms | ms | 22.8x |
| info | 8.80ms | 191.80ms | ms | 21.7x |
| status | 7.20ms | N/Ams | N/Ams | N/A |
| explicit | 2.20ms | 11.60ms | ms | 5.2x |

## Analysis

OMG's performance advantage comes from its **daemon architecture**:

1. **Pre-indexed database**: Package metadata loaded into memory at daemon start
2. **Unix socket IPC**: Sub-millisecond communication vs process spawn overhead
3. **In-memory fuzzy search**: No disk I/O during queries

This is a **fair architectural comparison** - OMG chose a different design that
trades memory usage (~50MB) for query speed. pacman and yay are designed for
lower memory footprint with on-demand disk access.
