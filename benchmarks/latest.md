# OMG benchmark — latest recorded run

- **Record:** [`20260903_015949-5c43ddcc`](records/20260903_015949-5c43ddcc/)
- **When:** 2026-09-03T01:59:49Z
- **Commit:** `5c43ddcc60ce151de0248f122eb2520176ea3387` (`v0.1.214-1730-g5c43ddcc-dirty`)
- **Dirty tree:** yes
- **Host:** Intel(R) Core(TM) i9-14900K, Linux 7.2.2-arch1-1, 31.1 GiB RAM
- **Hyperfine:** hyperfine 1.20.0
- **Flags:** `--shell=none --output=pipe`, warmup=3, min-runs=20, max-runs=50

Scratch output is gitignored (`benchmark_results/`). Canonical copies of the
full hyperfine JSON (every run, every exit code) live under `benchmarks/records/`.

## Preflight

Each timed command was executed once before hyperfine. Search and info had to
print `firefox`. Explicit count had to be a positive integer. Status had to
succeed. Runs with a non-zero exit are rejected.

| Check | Evidence |
|---|---|
| `explicit_count` | 273 |
| `explicit` | 4 bytes, 1 line |
| `info-fast` | 85 bytes, 3 lines |
| `info` | 585 bytes, 12 lines |
| `pacman-info` | 1279 bytes, 25 lines |
| `pacman-search` | 21170 bytes, 458 lines |
| `search-fast` | 1325 bytes, 21 lines |
| `search` | 166 bytes, 6 lines |
| `status` | 382 bytes, 14 lines |
| `yay-search` | 26381 bytes, 458 lines |

## Results

### search

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 13.1 ± 9.6 | 7.7 | 77.1 | 1.17 ± 0.88 |
| `OMG (omg-fast)` | 11.2 ± 2.0 | 8.0 | 14.9 | 1.00 |
| `pacman` | 247.4 ± 12.7 | 229.9 | 281.1 | 22.12 ± 4.13 |
| `yay (--repo)` | 366.3 ± 25.1 | 310.3 | 403.1 | 32.76 ± 6.29 |

Daemon mean **13.1 ms** vs pacman **247.4 ms** (18.9×). Median 11.4 ms (50 runs).

### info

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 26.4 ± 7.6 | 15.3 | 48.9 | 2.47 ± 0.92 |
| `OMG (omg-fast)` | 10.7 ± 2.5 | 6.1 | 16.8 | 1.00 |
| `pacman` | 225.6 ± 10.0 | 210.9 | 244.1 | 21.07 ± 5.03 |
| `yay (--repo)` | 542.9 ± 25.7 | 502.1 | 595.0 | 50.70 ± 12.14 |

Daemon mean **26.4 ms** vs pacman **225.6 ms** (8.5×). Median 24.5 ms (50 runs).

### status

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 11.9 ± 2.1 | 8.7 | 17.6 | 1.11 ± 0.31 |
| `OMG (omg-fast)` | 10.7 ± 2.3 | 7.2 | 19.8 | 1.00 |

### explicit

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 10.4 ± 2.0 | 6.9 | 15.5 | 1.00 ± 0.28 |
| `OMG (omg-fast)` | 10.4 ± 2.1 | 5.8 | 16.2 | 1.00 |
| `pacman` | 31.9 ± 3.1 | 24.3 | 38.8 | 3.08 ± 0.69 |
| `yay` | 53.6 ± 5.2 | 41.4 | 64.6 | 5.17 ± 1.16 |

Daemon mean **10.4 ms** vs pacman **31.9 ms** (3.1×). Median 10.3 ms (50 runs).

### update

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `update discovery (ready index)` | 829.1 ± 35.6 | 767.1 | 887.6 | 1.00 |
| `update discovery (missing index)` | 973.9 ± 91.1 | 794.0 | 1182.9 | 1.17 ± 0.12 |

## Reproduce

```bash
./benchmark-hyperfine.sh
```

