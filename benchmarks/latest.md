# OMG benchmark — latest recorded run

- **Record:** [`20260906_044101-7e05fca`](records/20260906_044101-7e05fca/)
- **When:** 2026-09-06T04:41:01Z
- **Commit:** `7e05fca621f989e76843421d35a94a281bc8a06d` (`7e05fca`)
- **Dirty tree:** yes
- **Host:** AMD EPYC 7763 64-Core Processor, Linux 6.17.0-1022-azure, 15.6 GiB RAM
- **Hyperfine:** unknown
- **Flags:** `--shell=none --output=pipe`, warmup=None, min-runs=None, max-runs=None

Scratch output is gitignored (`benchmark_results/`). Canonical copies of the
full hyperfine JSON (every run, every exit code) live under `benchmarks/records/`.

## Preflight

Each timed command was executed once before hyperfine. Search and info had to
print `firefox`. Explicit count had to be a positive integer. Status had to
succeed. Runs with a non-zero exit are rejected.

| Check | Evidence |
|---|---|
| `explicit_count` | 10 |
| `explicit` | 3 bytes, 1 lines |
| `info-fast` | 85 bytes, 3 lines |
| `info` | 585 bytes, 12 lines |
| `pacman-info` | 1272 bytes, 25 lines |
| `pacman-search` | 21158 bytes, 458 lines |
| `search-fast` | 1309 bytes, 21 lines |
| `search` | 166 bytes, 6 lines |
| `status` | 160 bytes, 11 lines |

## Results

### search

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 9.9 ± 0.8 | 8.8 | 11.7 | 1.06 ± 0.15 |
| `OMG (omg-fast)` | 9.4 ± 1.0 | 8.1 | 12.0 | 1.00 |
| `pacman` | 229.9 ± 1.2 | 228.4 | 232.7 | 24.56 ± 2.75 |

Daemon mean **9.9 ms** vs pacman **229.9 ms** (23.2×). Median 9.7 ms (15 runs).

### info

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 11.9 ± 0.6 | 11.1 | 13.7 | 1.34 ± 0.10 |
| `OMG (omg-fast)` | 8.9 ± 0.4 | 8.1 | 9.7 | 1.00 |
| `pacman` | 212.5 ± 1.0 | 211.4 | 215.3 | 23.91 ± 1.13 |

Daemon mean **11.9 ms** vs pacman **212.5 ms** (17.8×). Median 11.8 ms (15 runs).

### status

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 10.0 ± 0.6 | 8.9 | 10.9 | 1.28 ± 0.11 |
| `OMG (omg-fast)` | 7.9 ± 0.5 | 7.6 | 9.5 | 1.00 |

### explicit

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 8.3 ± 0.1 | 8.1 | 8.6 | 1.05 ± 0.03 |
| `OMG (omg-fast)` | 7.8 ± 0.2 | 7.6 | 8.4 | 1.00 |
| `pacman` | 15.3 ± 0.2 | 14.9 | 15.7 | 1.95 ± 0.06 |

Daemon mean **8.3 ms** vs pacman **15.3 ms** (1.9×). Median 8.2 ms (15 runs).

## Reproduce

```bash
./benchmark-hyperfine.sh
```

