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
- **Kernel:** 6.17.0-1020-azure
- **CPU:**                              AMD EPYC 7763 64-Core Processor
- **CPU Cores:**                                  4
- **RAM:** 15Gi

## Results

| Command | OMG (Daemon) | pacman | yay | Speedup vs pacman |
|---------|--------------|--------|-----|-------------------|
| search | 14.40ms | 218.80ms | ms | 15.1x |
| info | 13.20ms | 200.80ms | ms | 15.2x |
| status | 11.00ms | N/Ams | N/Ams | N/A |
| explicit | 3.40ms | 17.20ms | ms | 5.0x |

## Analysis

OMG's performance advantage comes from its **daemon architecture**:

1. **Pre-indexed database**: Package metadata loaded into memory at daemon start
2. **Unix socket IPC**: Sub-millisecond communication vs process spawn overhead
3. **In-memory fuzzy search**: No disk I/O during queries

This is a **fair architectural comparison** - OMG chose a different design that
trades memory usage (~50MB) for query speed. pacman and yay are designed for
lower memory footprint with on-demand disk access.
