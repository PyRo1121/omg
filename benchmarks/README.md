# Benchmark records

Canonical performance history for OMG.

| Path | What it is |
|---|---|
| `benchmarks/records/<id>/` | One hyperfine run: full JSON, markdown tables, `meta.json` |
| `benchmarks/records/INDEX.md` | Newest-first index of those runs |
| `benchmarks/latest.md` | Human summary of the most recent recorded run |
| `benchmarks/summary.json` | CI regression gate (search mean). Updated only by CI or `--update-gate` |
| `benchmarks/badge.json` | Shields.io endpoint for the README badge |
| `benchmarks/baselines/` | Pinned comparison snapshots |
| `benchmark_results/` | Scratch output from a local run (**gitignored**) |

## Reproduce

```bash
./benchmark-hyperfine.sh          # 3 warmup, 20–50 runs, writes a record
./benchmark-hyperfine.sh --fast   # CI / smoke
./benchmark-hyperfine.sh --update # AUR update-discovery only
```

The script refuses to record if preflight fails (search/info must print `firefox`,
explicit count must be a positive integer) or if the JSON looks fake (sub-1 ms
daemon search, sub-30 ms pacman search, non-zero exits).

`./benchmark.sh` is the bash-timing fallback when hyperfine is not installed. It
does not write `benchmarks/records/`.
