# OMG benchmark — latest recorded run

- **Record:** [`20260903_112232-60d20dc`](records/20260903_112232-60d20dc/)
- **When:** 2026-09-03T11:22:32Z
- **Commit:** `60d20dcc29e4bd8d38b27ee48fa0466f06341454` (`60d20dc`)
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
| `search-fast` | 1277 bytes, 21 lines |
| `search` | 162 bytes, 6 lines |
| `status` | 301 bytes, 12 lines |

## Results

### search

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 9.5 ± 0.6 | 8.2 | 10.3 | 1.06 ± 0.10 |
| `OMG (omg-fast)` | 8.9 ± 0.6 | 8.1 | 10.0 | 1.00 |
| `pacman` | 217.3 ± 1.1 | 215.6 | 219.0 | 24.37 ± 1.71 |

Daemon mean **9.5 ms** vs pacman **217.3 ms** (22.9×). Median 9.6 ms (15 runs).

### info

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 12.1 ± 0.7 | 11.1 | 13.5 | 1.37 ± 0.12 |
| `OMG (omg-fast)` | 8.8 ± 0.5 | 7.8 | 9.6 | 1.00 |
| `pacman` | 200.6 ± 6.0 | 197.1 | 222.3 | 22.67 ± 1.56 |

Daemon mean **12.1 ms** vs pacman **200.6 ms** (16.5×). Median 12.0 ms (15 runs).

### status

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 9.8 ± 0.9 | 8.7 | 11.9 | 1.34 ± 0.13 |
| `OMG (omg-fast)` | 7.3 ± 0.2 | 7.1 | 7.7 | 1.00 |

### explicit

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG (Daemon)` | 8.1 ± 0.6 | 7.5 | 10.0 | 1.11 ± 0.10 |
| `OMG (omg-fast)` | 7.3 ± 0.3 | 6.8 | 7.9 | 1.00 |
| `pacman` | 14.4 ± 0.3 | 14.0 | 15.2 | 1.98 ± 0.08 |

Daemon mean **8.1 ms** vs pacman **14.4 ms** (1.8×). Median 7.8 ms (15 runs).

## Reproduce

```bash
./benchmark-hyperfine.sh
```

