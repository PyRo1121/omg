# OMG benchmark — latest recorded run

- **Record:** [`20260903_163647-aa6303b`](records/20260903_163647-aa6303b/)
- **When:** 2026-09-03T16:36:47Z
- **Commit:** `aa6303b6890011a9ca52be7d2a8316d3d4d570ff` (`aa6303b`)
- **Dirty tree:** yes
- **Host:** AMD EPYC 9V74 80-Core Processor, Linux 6.17.0-1022-azure, 15.6 GiB RAM
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
| `search-fast` | 1261 bytes, 21 lines |
| `search` | 162 bytes, 6 lines |
| `status` | 301 bytes, 12 lines |

## Results

### search

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 6.4 ± 0.6 | 5.5 | 7.9 | 1.15 ± 0.14 |
| `OMG (omg-fast)` | 5.5 ± 0.4 | 5.0 | 6.5 | 1.00 |
| `pacman` | 187.9 ± 1.8 | 184.3 | 190.0 | 33.88 ± 2.51 |

Daemon mean **6.4 ms** vs pacman **187.9 ms** (29.4×). Median 6.5 ms (15 runs).

### info

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 8.1 ± 0.6 | 7.3 | 9.5 | 1.39 ± 0.19 |
| `OMG (omg-fast)` | 5.8 ± 0.7 | 5.0 | 7.3 | 1.00 |
| `pacman` | 175.6 ± 2.4 | 171.5 | 181.7 | 30.29 ± 3.63 |

Daemon mean **8.1 ms** vs pacman **175.6 ms** (21.8×). Median 7.9 ms (15 runs).

### status

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 6.3 ± 0.5 | 5.6 | 7.1 | 1.20 ± 0.15 |
| `OMG (omg-fast)` | 5.2 ± 0.5 | 4.7 | 6.5 | 1.00 |

### explicit

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 5.7 ± 0.3 | 5.3 | 6.3 | 1.09 ± 0.14 |
| `OMG (omg-fast)` | 5.3 ± 0.6 | 4.8 | 7.0 | 1.00 |
| `pacman` | 9.1 ± 0.1 | 8.9 | 9.5 | 1.73 ± 0.20 |

Daemon mean **5.7 ms** vs pacman **9.1 ms** (1.6×). Median 5.7 ms (15 runs).

## Reproduce

```bash
./benchmark-hyperfine.sh
```

