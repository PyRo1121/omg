# OMG benchmark — latest recorded run

- **Record:** [`20260903_234941-4b3920e`](records/20260903_234941-4b3920e/)
- **When:** 2026-09-03T23:49:41Z
- **Commit:** `4b3920e577daa9de9d15b13e493d38bb33bd4cdd` (`4b3920e`)
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
| `search-fast` | 1277 bytes, 21 lines |
| `search` | 162 bytes, 6 lines |
| `status` | 301 bytes, 12 lines |

## Results

### search

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 10.5 ± 2.5 | 9.3 | 19.3 | 1.19 ± 0.29 |
| `OMG (omg-fast)` | 8.9 ± 0.5 | 8.0 | 10.0 | 1.00 |
| `pacman` | 230.5 ± 1.1 | 229.3 | 232.5 | 26.01 ± 1.61 |

Daemon mean **10.5 ms** vs pacman **230.5 ms** (21.9×). Median 9.8 ms (15 runs).

### info

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 11.6 ± 0.6 | 10.2 | 12.7 | 1.25 ± 0.10 |
| `OMG (omg-fast)` | 9.2 ± 0.6 | 8.5 | 11.0 | 1.00 |
| `pacman` | 213.9 ± 2.5 | 211.3 | 221.8 | 23.20 ± 1.44 |

Daemon mean **11.6 ms** vs pacman **213.9 ms** (18.5×). Median 11.5 ms (15 runs).

### status

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 9.8 ± 0.7 | 8.9 | 10.8 | 1.25 ± 0.09 |
| `OMG (omg-fast)` | 7.8 ± 0.2 | 7.6 | 8.2 | 1.00 |

### explicit

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 8.3 ± 0.2 | 8.1 | 8.8 | 1.04 ± 0.04 |
| `OMG (omg-fast)` | 8.0 ± 0.2 | 7.7 | 8.6 | 1.00 |
| `pacman` | 15.4 ± 0.2 | 15.1 | 15.7 | 1.93 ± 0.06 |

Daemon mean **8.3 ms** vs pacman **15.4 ms** (1.9×). Median 8.2 ms (15 runs).

## Reproduce

```bash
./benchmark-hyperfine.sh
```

