# OMG Benchmark Guide

Complete guide to running performance benchmarks for OMG.

---

## 🚀 Quick Start

### Standard Benchmark (10 iterations, 2 warmup)
```bash
./benchmark.sh
```
**Time:** ~2-3 minutes  
**Use when:** Publishing official benchmark results

### Fast Benchmark (5 iterations, 1 warmup)
```bash
./benchmark.sh --fast
```
**Time:** ~1 minute  
**Use when:** Quick validation during development

### Hyperfine Benchmark (Industry Standard)
```bash
./benchmark-hyperfine.sh
```
**Time:** ~1-2 minutes  
**Use when:** Statistical analysis, CI regression detection

---

## 📊 Benchmark Scripts

### `benchmark.sh` - Production Benchmark

**Features:**
- Custom bash timing with statistical analysis
- Fairness principles documented
- Compares OMG vs pacman vs yay
- Generates markdown report with environment details

**Options:**
```bash
./benchmark.sh              # Full (10 iters, 2 warmup)
./benchmark.sh --fast       # Fast (5 iters, 1 warmup)
./benchmark.sh --help       # Show options
```

**Environment Variables:**
```bash
OMG_BENCH_ITERATIONS=3 ./benchmark.sh    # Custom iterations
OMG_BENCH_WARMUP=1 ./benchmark.sh        # Custom warmup
```

**Use Cases:**
- Official benchmark results for README.md
- Detailed performance reports with min/max/avg
- When hyperfine is not available

---

### `benchmark-hyperfine.sh` - Modern Benchmark (Recommended)

**Features:**
- Industry-standard tool (used by ripgrep, fd, bat)
- Statistical rigor with outlier detection
- Automatic run count determination
- JSON export for CI regression detection
- 40-60% faster execution than custom bash loops

**Requirements:**
```bash
# Arch Linux
sudo pacman -S hyperfine

# macOS
brew install hyperfine

# Cargo
cargo install hyperfine
```

**Options:**
```bash
./benchmark-hyperfine.sh              # Full (3 warmup, 10+ runs)
./benchmark-hyperfine.sh --fast       # Fast (1 warmup, 5+ runs)
./benchmark-hyperfine.sh --help       # Show options
```

**Environment Variables:**
```bash
OMG_BENCH_WARMUP=5 ./benchmark-hyperfine.sh     # Custom warmup
OMG_BENCH_RUNS=20 ./benchmark-hyperfine.sh      # Custom min runs
OMG_BENCH_EXPORT_DIR=results ./benchmark-hyperfine.sh  # Custom output dir
```

**Use Cases:**
- CI/CD regression detection (JSON export)
- Statistical analysis with confidence intervals
- When you need industry-standard benchmarking

**Output:**
```
benchmark_results/
├── search.md         # Markdown table for search benchmark
├── search.json       # JSON data for CI regression tracking
├── info.md           # Package info benchmark
├── info.json
├── status.md         # Status command benchmark
├── status.json
├── explicit.md       # Explicit count benchmark
└── explicit.json
```

---

## ⚡ Performance Comparison

| Method | Time | Iterations | Warmup | Tool | CI-Ready |
|--------|------|------------|--------|------|----------|
| `benchmark.sh` | 100% | 10 | 2 | bash | ✅ |
| `benchmark.sh --fast` | 50% | 5 | 1 | bash | ✅ |
| `benchmark-hyperfine.sh` | 40% | auto | 3 | hyperfine | ✅ |
| `benchmark-hyperfine.sh --fast` | 30% | auto | 1 | hyperfine | ✅ |

---

## 🎯 Use Case Guide

### For Development

**Quick validation after code changes:**
```bash
./benchmark.sh --fast
```
**Time:** ~1 minute  
**Accuracy:** Good enough for development

### For CI/CD

**Regression detection:**
```bash
./benchmark-hyperfine.sh
```
**Why:**
- JSON export for automated comparison
- Statistical outlier detection
- Fails if performance regression detected

**CI Configuration Example:**
```yaml
# .github/workflows/benchmark.yml
- name: Run Benchmarks
  run: ./benchmark-hyperfine.sh

- name: Upload Results
  uses: actions/upload-artifact@v3
  with:
    name: benchmark-results
    path: benchmark_results/*.json

- name: Compare with Baseline
  run: |
    # Compare current vs baseline (fail on >10% regression)
    python scripts/compare-benchmarks.py \
      baseline.json \
      benchmark_results/search.json \
      --threshold 0.10
```

### For README/Marketing

**Official benchmark numbers:**
```bash
./benchmark.sh
```
**Why:**
- Detailed fairness documentation
- Min/max/avg statistics
- Consistent with historical results
- Environment details included

### For Research

**Deep statistical analysis:**
```bash
OMG_BENCH_RUNS=50 ./benchmark-hyperfine.sh
```
**Why:**
- High iteration count for confidence
- Outlier detection with Modified Z-score
- Median-based statistics (robust to outliers)

---

## 📈 Understanding Results

### benchmark.sh Output

```
| Command    | OMG (Daemon) | pacman | yay   | Speedup |
|------------|--------------|--------|-------|---------|
| search     | 6.00ms       | 133ms  | 150ms | 22.0x   |
| info       | 6.50ms       | 138ms  | 300ms | 21.0x   |
| status     | 7.00ms       | N/A    | N/A   | N/A     |
| explicit   | 1.20ms       | 14ms   | 27ms  | 12.0x   |
```

**Metrics:**
- **OMG (Daemon):** In-memory indexed search via Unix socket
- **pacman:** Direct ALPM access (baseline)
- **yay:** pacman wrapper with `--repo` flag (no AUR)
- **Speedup:** `pacman_time / omg_time`

### hyperfine Output

```
Benchmark 1: OMG (Daemon)
  Time (mean ± σ):       6.2 ms ±   0.4 ms    [User: 2.1 ms, System: 3.8 ms]
  Range (min … max):     5.8 ms …   7.1 ms    15 runs
 
Benchmark 2: pacman
  Time (mean ± σ):     133.1 ms ±   2.8 ms    [User: 89.2 ms, System: 43.7 ms]
  Range (min … max):   128.9 ms … 138.5 ms    22 runs
 
Summary
  'OMG (Daemon)' ran
   21.47 ± 1.32 times faster than 'pacman'
```

**Metrics:**
- **mean ± σ:** Average time ± standard deviation
- **Range:** Min and max observed times
- **runs:** Number of iterations hyperfine ran
- **Summary:** Speedup with confidence interval

---

## 🔧 Troubleshooting

### "hyperfine not found"

**Solution:**
```bash
# Arch Linux
sudo pacman -S hyperfine

# macOS
brew install hyperfine

# Cargo (any platform)
cargo install hyperfine

# Or fall back to standard benchmark
./benchmark.sh --fast
```

### "bc not found"

**Solution:**
```bash
# Arch Linux
sudo pacman -S bc

# Debian/Ubuntu
sudo apt install bc

# macOS
brew install bc
```

### Daemon Fails to Start

**Symptoms:**
```
❌ OMG daemon failed to start
```

**Solutions:**
1. Check if another daemon is running:
   ```bash
   killall omgd
   ./benchmark.sh
   ```

2. Check logs:
   ```bash
   # Daemon logs are in $BENCH_DIR/omgd.log
   # Path shown in error output
   ```

3. Clean state:
   ```bash
   rm -rf ~/.local/share/omg
   ./benchmark.sh
   ```

### Results Vary Widely

**Causes:**
- System load (other processes running)
- Filesystem cache state
- CPU throttling (thermal)

**Solutions:**
1. Close other applications
2. Run multiple times, use median result
3. Use hyperfine (automatic outlier detection)
4. Check CPU governor:
   ```bash
   cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
   # Should be "performance" for benchmarking
   ```

---

## 🎓 Best Practices

### 1. Benchmark on Representative Hardware

**Don't benchmark on:**
- Virtual machines (inconsistent performance)
- Systems under heavy load
- Laptops on battery (CPU throttling)

**Do benchmark on:**
- Dedicated machines
- Systems with performance CPU governor
- Same hardware as production

### 2. Warm vs Cold Cache

**Warm cache (current benchmarks):**
- Represents typical usage
- Filesystem cache populated
- CPU caches hot

**Cold cache (not currently benchmarked):**
- Represents first-run performance
- Requires cache clearing between runs
- More realistic for install/update scenarios

To add cold cache benchmarking:
```bash
# Clear caches before each run (requires sudo)
export RESET_CACHES="sync; echo 3 | sudo tee /proc/sys/vm/drop_caches"
hyperfine --prepare "$RESET_CACHES" --min-runs 3 "omg search firefox"
```

### 3. Iteration Count Guidelines

| Command Speed | Recommended Iterations |
|---------------|------------------------|
| < 10ms | 20-50 |
| 10-100ms | 10-20 |
| 100ms-1s | 5-10 |
| > 1s | 3-5 |

OMG commands are <10ms, so 10+ iterations is appropriate.

### 4. Statistical Validity

**Minimum for confidence:**
- At least 5 runs for slow commands (>1s)
- At least 10 runs for fast commands (<100ms)
- More runs if high variance observed

**hyperfine automatically adjusts** run count until:
- Confidence interval is narrow enough
- Or maximum iterations reached

---

## 📚 References

### Hyperfine Documentation
- GitHub: https://github.com/sharkdp/hyperfine
- Used by: ripgrep, fd, bat, exa, tokei, hexyl

### Benchmark Methodologies
- fd benchmarks: https://github.com/sharkdp/fd-benchmarks
- Modified Z-score outlier detection
- Median-based statistics (robust to outliers)

### OMG Performance Targets
- Search: **6ms** (22x faster than pacman)
- Info: **6.5ms** (21x faster than pacman)
- Status: **7ms**
- Explicit: **1.2ms** (12x faster than pacman)

---

## 🤝 Contributing

When adding new benchmarks:

1. **Add to both scripts** (benchmark.sh and benchmark-hyperfine.sh)
2. **Use same fairness principles** (--no-aur for OMG, --repo for yay)
3. **Document methodology** in comments
4. **Update this guide** with new benchmark info

Example:
```bash
# In benchmark.sh
RESULTS["new_cmd,OMG"]=$(run_bench "OMG" "$OMG new-command" $ITERATIONS $WARMUP)

# In benchmark-hyperfine.sh
hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
    --export-markdown "$EXPORT_DIR/new_cmd.md" \
    --command-name "OMG" "$OMG new-command"
```

---

## ❓ FAQ

**Q: Which benchmark should I use?**  
A: For development: `./benchmark.sh --fast`. For CI: `./benchmark-hyperfine.sh`. For README: `./benchmark.sh`.

**Q: Why are there two benchmark scripts?**  
A: `benchmark.sh` is the original with custom timing. `benchmark-hyperfine.sh` uses industry-standard tool with better statistics. Both are maintained for compatibility.

**Q: How fast is --fast mode?**  
A: ~50% faster (5 iterations vs 10). Accuracy is still good for development.

**Q: Can I run benchmarks in CI?**  
A: Yes! Use `benchmark-hyperfine.sh` for JSON export. Compare with baseline to detect regressions.

**Q: Why does hyperfine run different iteration counts?**  
A: It automatically determines how many runs are needed for statistical confidence. Faster commands need more runs to overcome timing jitter.

**Q: Should I clear caches before benchmarking?**  
A: Current benchmarks use warm cache (realistic usage). For cold cache (install/update scenarios), use `--prepare` flag with cache clearing.

---

**For questions or issues, see:** [docs/troubleshooting.md](docs/troubleshooting.md)
