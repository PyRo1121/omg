# OMG benchmark — latest recorded run

- **Record:** [`20260903_021851-e4d3f42`](records/20260903_021851-e4d3f42/)
- **When:** 2026-09-03T02:18:51Z
- **Commit:** `e4d3f42db88a63261f89c9a4e55e38940e977039` (`e4d3f42`)
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
| `info-fast` | 83 bytes, 3 lines |
| `info` | 583 bytes, 12 lines |
| `pacman-info` | 1270 bytes, 25 lines |
| `pacman-search` | 20950 bytes, 458 lines |
| `search-fast` | 1269 bytes, 21 lines |
| `search` | 162 bytes, 6 lines |
| `status` | 301 bytes, 12 lines |

## Results

### search

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 7.7 ± 0.6 | 7.1 | 9.3 | 1.11 ± 0.12 |
| `OMG (omg-fast)` | 7.0 ± 0.5 | 6.3 | 8.1 | 1.00 |
| `pacman` | 182.0 ± 1.4 | 180.4 | 185.0 | 26.19 ± 1.91 |

Daemon mean **7.7 ms** vs pacman **182.0 ms** (23.5×). Median 7.5 ms (15 runs).

### info

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 10.0 ± 0.8 | 9.0 | 11.6 | 1.33 ± 0.13 |
| `OMG (omg-fast)` | 7.5 ± 0.4 | 6.7 | 8.2 | 1.00 |
| `pacman` | 167.9 ± 0.8 | 166.7 | 169.9 | 22.34 ± 1.26 |

Daemon mean **10.0 ms** vs pacman **167.9 ms** (16.8×). Median 9.8 ms (15 runs).

### status

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 7.8 ± 0.6 | 7.1 | 9.2 | 1.23 ± 0.11 |
| `OMG (omg-fast)` | 6.4 ± 0.4 | 5.9 | 7.3 | 1.00 |

### explicit

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 6.5 ± 0.2 | 6.2 | 7.0 | 1.08 ± 0.04 |
| `OMG (omg-fast)` | 6.0 ± 0.1 | 5.9 | 6.2 | 1.00 |
| `pacman` | 12.4 ± 0.3 | 12.2 | 13.2 | 2.07 ± 0.06 |

Daemon mean **6.5 ms** vs pacman **12.4 ms** (1.9×). Median 6.4 ms (15 runs).

## Reproduce

```bash
./benchmark-hyperfine.sh
```

