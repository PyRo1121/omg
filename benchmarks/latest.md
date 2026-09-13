# OMG benchmark — latest recorded run

- **Record:** [`20260913_043554-bc7fe49`](records/20260913_043554-bc7fe49/)
- **When:** 2026-09-13T04:35:54Z
- **Commit:** `bc7fe493bee5a002fefeeaf10c3f35872cbe9b40` (`bc7fe49`)
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
| `info` | 184 bytes, 8 lines |
| `pacman-info` | 1272 bytes, 25 lines |
| `pacman-search` | 21158 bytes, 458 lines |
| `search` | 172 bytes, 8 lines |
| `status` | 160 bytes, 11 lines |

## Results

### search

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG` | 19.2 ± 7.4 | 16.0 | 45.6 | 1.00 |
| `pacman` | 214.3 ± 1.1 | 212.8 | 216.3 | 11.14 ± 4.27 |

Daemon mean **19.2 ms** vs pacman **214.3 ms** (11.1×). Median 17.4 ms (15 runs).

### info

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG` | 27.0 ± 1.1 | 25.3 | 28.5 | 1.00 |
| `pacman` | 198.5 ± 3.3 | 196.3 | 209.8 | 7.36 ± 0.31 |

Daemon mean **27.0 ms** vs pacman **198.5 ms** (7.4×). Median 27.3 ms (15 runs).

### status

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG` | 9.6 ± 0.8 | 8.5 | 11.1 | 1.00 |

### explicit

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `OMG` | 16.9 ± 0.9 | 15.5 | 18.3 | 1.19 ± 0.07 |
| `pacman` | 14.2 ± 0.3 | 13.9 | 14.9 | 1.00 |

Daemon mean **16.9 ms** vs pacman **14.2 ms** (0.8×). Median 16.8 ms (15 runs).

## Reproduce

```bash
./benchmark-hyperfine.sh
```

