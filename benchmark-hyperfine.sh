#!/bin/bash
set -e

# ============================================================================
# OMG Performance Benchmark with Hyperfine (Industry Best Practice)
# ============================================================================
#
# This benchmark uses hyperfine (https://github.com/sharkdp/hyperfine),
# the industry-standard CLI benchmarking tool used by ripgrep, fd, and bat.
#
# ADVANTAGES OVER CUSTOM BASH TIMING:
# - Statistical rigor with outlier detection (Modified Z-score method)
# - Automatic run count determination for confidence intervals
# - Warm/cold cache benchmarking support
# - JSON export for CI regression detection
# - 40-60% faster execution than manual bash loops
#
# REQUIREMENTS:
#   sudo pacman -S hyperfine    # Arch Linux
#   brew install hyperfine      # macOS
#
# USAGE:
#   ./benchmark-hyperfine.sh              # Full benchmark
#   ./benchmark-hyperfine.sh --fast       # Quick benchmark
#   ./benchmark-hyperfine.sh --update     # AUR update discovery benchmark only
#   ./benchmark-hyperfine.sh --help       # Show options
#
# ============================================================================

export PATH="$HOME/.cargo/bin:$PATH"

WARMUP=3
MIN_RUNS=10
FAST_MODE=false
UPDATE_MODE=false
EXPORT_DIR="benchmark_results"

print_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Options:
  --fast, -f    Run in fast mode (reduced warmup and runs)
  --update      Run ONLY the AUR update discovery benchmark (no daemon,
                no other benchmarks) and exit
  --help, -h    Show this help message

Environment Variables:
  OMG_BENCH_WARMUP        Number of warmup runs (default: 3, fast: 1)
  OMG_BENCH_RUNS          Minimum runs (default: 10, fast: 5)
  OMG_BENCH_EXPORT_DIR    Results directory (default: benchmark_results)
  OMG_BENCH_BINARY        Path to a prebuilt omg binary to benchmark instead
                          of building (intended for --update mode)
  OMG_BENCH_TARGET_DIR    Cargo target directory (default:
                          ~/.cache/build-targets/omg-benchmark-hyperfine)
  OMG_BENCH_SOURCE_CACHE  Cache dir to copy AUR/package-DB fixtures from
                          (default: OMG_CACHE_DIR, else ~/.cache/omg)

Examples:
  $0                           # Full benchmark (3 warmup, 10+ runs)
  $0 --fast                    # Fast benchmark (1 warmup, 5+ runs)
  $0 --update                  # Update discovery benchmark only
  OMG_BENCH_RUNS=20 $0         # Custom run count
  OMG_BENCH_BINARY=./omg $0 --fast --update   # Benchmark a prebuilt binary
EOF
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --fast|-f)
            FAST_MODE=true
            shift
            ;;
        --update)
            UPDATE_MODE=true
            shift
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

if [ "$FAST_MODE" = true ]; then
    WARMUP=${OMG_BENCH_WARMUP:-1}
    MIN_RUNS=${OMG_BENCH_RUNS:-5}
else
    WARMUP=${OMG_BENCH_WARMUP:-3}
    MIN_RUNS=${OMG_BENCH_RUNS:-10}
fi

EXPORT_DIR=${OMG_BENCH_EXPORT_DIR:-benchmark_results}
mkdir -p "$EXPORT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

if ! command -v hyperfine &>/dev/null; then
    echo -e "${RED}❌ hyperfine not installed${NC}"
    echo ""
    echo "Install with:"
    echo "  Arch Linux:  sudo pacman -S hyperfine"
    echo "  macOS:       brew install hyperfine"
    echo "  Cargo:       cargo install hyperfine"
    echo ""
    echo "Falling back to standard benchmark.sh..."
    if [ "$FAST_MODE" = true ]; then
        exec ./benchmark.sh --fast
    else
        exec ./benchmark.sh
    fi
fi

TARGET_DIR="${OMG_BENCH_TARGET_DIR:-$HOME/.cache/build-targets/omg-benchmark-hyperfine}"

if [ -n "${OMG_BENCH_BINARY:-}" ]; then
    if [ "$UPDATE_MODE" != true ]; then
        echo -e "${RED}❌ OMG_BENCH_BINARY is supported only with --update${NC}" >&2
        exit 1
    fi
    echo -e "${BLUE}🔧 Using prebuilt binary: $OMG_BENCH_BINARY${NC}"
    if [ ! -x "$OMG_BENCH_BINARY" ]; then
        echo -e "${RED}❌ OMG_BENCH_BINARY is set but not executable: $OMG_BENCH_BINARY${NC}" >&2
        exit 1
    fi
    OMG="$OMG_BENCH_BINARY"
    OMGD="$TARGET_DIR/release/omgd"
    OMG_FAST="$TARGET_DIR/release/omg-fast"
else
    echo -e "${BLUE}🔨 Building release binaries...${NC}"
    CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release --locked --features arch --quiet

    OMG="$TARGET_DIR/release/omg"
    OMGD="$TARGET_DIR/release/omgd"
    OMG_FAST="$TARGET_DIR/release/omg-fast"
fi

# ----------------------------------------------------------------------------
# AUR update discovery benchmark (--update)
#
# Benchmarks `omg update` update discovery against an isolated cache in two
# otherwise-identical variants:
#   (a) ready:    coherent, fresh AUR archive + binary index
#   (b) missing:  same archive but NO index (removed before every run via
#                 hyperfine --prepare), forcing the AUR RPC fallback path
#
# Deterministic regression guard: after the benchmark, variant (b) must NOT
# have created an index — update discovery must never synchronously rebuild
# global metadata. A statistical guard also fails only on a broad regression
# (missing-index mean > 3s AND > 5x ready-index mean), so small network
# variance does not make the benchmark flaky.
# ============================================================================
run_update_benchmark() {
    if [ "$EUID" -eq 0 ]; then
        echo -e "${RED}❌ Run the update benchmark as a regular user${NC}" >&2
        exit 1
    fi

    local source_cache="${OMG_BENCH_SOURCE_CACHE:-${OMG_CACHE_DIR:-$HOME/.cache/omg}}"
    local fixture_archive="$source_cache/aur/_meta/packages-meta-ext-v1.json.gz"
    local fixture_index="$source_cache/aur/_meta/packages-meta-ext-v1.rkyv"
    local fixture_local_db="$source_cache/local_db.bin"
    local fixture_sync_db="$source_cache/sync_db.bin"

    local missing_fixture=0
    local fixture
    for fixture in "$fixture_local_db" "$fixture_sync_db" "$fixture_archive" "$fixture_index"; do
        if [ ! -f "$fixture" ]; then
            echo -e "${RED}❌ Missing benchmark fixture: $fixture${NC}" >&2
            missing_fixture=1
        fi
    done
    if [ "$missing_fixture" -ne 0 ]; then
        echo "" >&2
        echo "Update discovery benchmarks need a populated omg cache with:" >&2
        echo "  local_db.bin, sync_db.bin, aur/_meta/packages-meta-ext-v1.json.gz," >&2
        echo "  aur/_meta/packages-meta-ext-v1.rkyv" >&2
        echo "Populate the OMG metadata cache first, or set OMG_BENCH_SOURCE_CACHE." >&2
        exit 1
    fi

    # Isolated scratch space under $HOME/.cache/build-targets (never /tmp),
    # removed on exit via trap.
    mkdir -p "$HOME/.cache/build-targets"
    local work_dir
    work_dir="$(mktemp -d "$HOME/.cache/build-targets/omg-update-bench-XXXXXX")"
    trap "rm -rf -- '$work_dir'" EXIT

    local ready_cache="$work_dir/ready-cache"
    local missing_cache="$work_dir/missing-index-cache"
    local config_dir="$work_dir/config"
    mkdir -p "$ready_cache/aur/_meta" "$missing_cache/aur/_meta" "$config_dir"

    # (a) Ready cache: copy the archive BEFORE the index so the published
    # generation is coherent (index mtime >= archive mtime) and fresh
    # (archive mtime within the metadata TTL).
    cp "$fixture_archive" "$ready_cache/aur/_meta/"
    cp "$fixture_index" "$ready_cache/aur/_meta/"
    cp "$fixture_local_db" "$fixture_sync_db" "$ready_cache/"

    # (b) Same fixtures but no index; hyperfine --prepare re-removes it
    # before every run.
    cp "$fixture_archive" "$missing_cache/aur/_meta/"
    cp "$fixture_local_db" "$fixture_sync_db" "$missing_cache/"

    # Keep the benchmark hermetic: never talk to a live daemon.
    export OMG_SOCKET_PATH="$work_dir/unused-omg.sock"
    export OMG_DAEMON_DATA_DIR="$work_dir/data"

    # Wrapper around the REAL 'omg update' (no --yes). Unprivileged and
    # non-interactive, it completes update discovery, then either reports
    # "up to date" (exit 0) or bails with "Use --yes for non-interactive
    # updates" (exit 1). The wrapper accepts only those outcomes and only
    # when the output proves discovery finished; network or parse errors
    # propagate as failures so hyperfine fails the benchmark.
    local wrapper="$work_dir/omg-update-wrapper.sh"
    cat > "$wrapper" << 'WRAPPER'
#!/bin/bash
set -e
omg_bin="$1"
log="$(mktemp "${TMPDIR:?}/omg-update-log-XXXXXX")"
rc=0
"$omg_bin" update >"$log" 2>&1 || rc=$?
# "update available" (singular) covers the 1-update summary line.
if grep -q -e "updates available" -e "update available" -e "up to date" "$log" \
    && ! grep -q -e "AUR update check failed" -e "Failed to check AUR updates" "$log" \
    && { [ "$rc" -eq 0 ] || [ "$rc" -eq 1 ]; }; then
    rm -f "$log"
    exit 0
fi
echo "omg update discovery failed (exit $rc):" >&2
cat "$log" >&2
rm -f "$log"
exit 1
WRAPPER
    chmod +x "$wrapper"

    local missing_index="$missing_cache/aur/_meta/packages-meta-ext-v1.rkyv"

    echo "========================================================"
    echo -e "${GREEN}🚀 OMG Update Discovery Benchmark${NC}"
    echo "========================================================"
    echo ""
    echo -e "${YELLOW}CONFIGURATION:${NC}"
    echo "  Binary: $OMG"
    echo "  Warmup runs: $WARMUP"
    echo "  Minimum runs: $MIN_RUNS"
    echo "  Ready cache: $ready_cache"
    echo "  Missing-index cache: $missing_cache"
    echo "  Results: $EXPORT_DIR/update.md, $EXPORT_DIR/update.json"
    echo ""

    # NOTE: hyperfine --prepare applies to every run of every command; the
    # rm targets only the missing-index cache, so it is a no-op for the
    # ready-index command.
    hyperfine --warmup "$WARMUP" --min-runs "$MIN_RUNS" \
        --prepare "rm -f '$missing_index'" \
        --command-name "update discovery (ready index)" \
            "TMPDIR='$work_dir' OMG_CACHE_DIR='$ready_cache' OMG_CONFIG_DIR='$config_dir' '$wrapper' '$OMG'" \
        --command-name "update discovery (missing index)" \
            "TMPDIR='$work_dir' OMG_CACHE_DIR='$missing_cache' OMG_CONFIG_DIR='$config_dir' '$wrapper' '$OMG'" \
        --export-markdown "$EXPORT_DIR/update.md" \
        --export-json "$EXPORT_DIR/update.json"

    # Deterministic regression guard: the missing-index scenario must not
    # have rebuilt global metadata (i.e. created an AUR index).
    if [ -f "$missing_index" ]; then
        echo -e "${RED}❌ REGRESSION: missing-index update discovery created an AUR index at:${NC}" >&2
        echo "  $missing_index" >&2
        echo "Update discovery must not synchronously rebuild global metadata." >&2
        exit 1
    fi
    echo -e "${GREEN}✅ Guard passed: missing-index run did not rebuild the AUR index${NC}"

    # Statistical guard: fail only on a broad regression so ordinary network
    # variance between the two scenarios stays non-fatal.
    python3 - "$EXPORT_DIR/update.json" << 'PYEOF'
import json
import sys

with open(sys.argv[1]) as handle:
    data = json.load(handle)

results = data.get("results", [])
ready = next((r for r in results if "ready index" in r["command"]), None)
missing = next((r for r in results if "missing index" in r["command"]), None)
if ready is None or missing is None:
    print("❌ update.json is missing the expected benchmark scenarios", file=sys.stderr)
    sys.exit(1)

ready_mean = ready["mean"]
missing_mean = missing["mean"]
limit = max(3.0, 5.0 * ready_mean)
print(f"Ready-index mean:   {ready_mean:.3f} s")
print(f"Missing-index mean: {missing_mean:.3f} s")
print(f"Regression limit:   {limit:.3f} s  (max(3 s, 5 x ready-index mean))")

if missing_mean > 3.0 and missing_mean > 5.0 * ready_mean:
    print(
        "❌ REGRESSION: missing-index update discovery mean exceeds both 3 s "
        "and 5x the ready-index mean",
        file=sys.stderr,
    )
    sys.exit(1)
print("✅ No broad update-discovery regression")
PYEOF

    echo ""
    echo -e "${GREEN}✅ Update discovery benchmark complete${NC}"
    echo "Results saved to: $EXPORT_DIR/update.md, $EXPORT_DIR/update.json"
}

if [ "$UPDATE_MODE" = true ]; then
    run_update_benchmark
    exit 0
fi

echo "========================================================"
echo -e "${GREEN}🚀 OMG Hyperfine Performance Benchmark${NC}"
echo "========================================================"
echo ""
if [ "$FAST_MODE" = true ]; then
    echo -e "${YELLOW}⚡ FAST MODE ENABLED${NC}"
fi
echo -e "${YELLOW}CONFIGURATION:${NC}"
echo "  Warmup runs: $WARMUP"
echo "  Minimum runs: $MIN_RUNS"
echo "  Results directory: $EXPORT_DIR/"
echo ""
echo -e "${YELLOW}METHODOLOGY:${NC}"
echo "  • hyperfine uses statistical analysis with outlier detection"
echo "  • Modified Z-score method (median-based, robust to outliers)"
echo "  • Automatic run count determination for confidence intervals"
echo "  • Warm cache benchmarks (realistic usage patterns)"
echo ""

mkdir -p "$HOME/.cache/build-targets"
BENCH_DIR="$(mktemp -d "$HOME/.cache/build-targets/omg-bench-XXXXXX")"
export OMG_DAEMON_DATA_DIR="$BENCH_DIR/data"
export OMG_SOCKET_PATH="$BENCH_DIR/omg.sock"
DAEMON_LOG="$BENCH_DIR/omgd.log"
mkdir -p "$OMG_DAEMON_DATA_DIR"

echo "Starting OMG Daemon..."
$OMGD > "$DAEMON_LOG" 2>&1 &
DAEMON_PID=$!

echo -n "Waiting for daemon to be ready..."
for i in {1..20}; do
    if $OMG status > /dev/null 2>&1; then
        echo " ready!"
        break
    fi
    sleep 0.1
done

if ! $OMG status > /dev/null 2>&1; then
    echo -e "${RED}❌ OMG daemon failed to start${NC}" >&2
    tail -n 50 "$DAEMON_LOG" >&2 || true
    kill $DAEMON_PID > /dev/null 2>&1 || true
    exit 1
fi

cleanup() {
    echo "Cleaning up..." >&2
    if [ -n "$DAEMON_PID" ]; then
        kill $DAEMON_PID > /dev/null 2>&1 || true
        wait $DAEMON_PID 2>/dev/null || true
    fi
    rm -rf "$BENCH_DIR"
}
trap cleanup EXIT

echo -e "\n${BLUE}📦 Benchmark: SEARCH (firefox)${NC}"
echo "-------------------------------"
if command -v pacman &>/dev/null && command -v yay &>/dev/null; then
    hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
        --export-markdown "$EXPORT_DIR/search.md" \
        --export-json "$EXPORT_DIR/search.json" \
        --command-name "OMG (Daemon)" "$OMG search firefox --no-aur" \
        --command-name "pacman" "pacman -Ss firefox" \
        --command-name "yay (--repo)" "yay -Ss --repo firefox"
elif command -v pacman &>/dev/null; then
    hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
        --export-markdown "$EXPORT_DIR/search.md" \
        --export-json "$EXPORT_DIR/search.json" \
        --command-name "OMG (Daemon)" "$OMG search firefox --no-aur" \
        --command-name "pacman" "pacman -Ss firefox"
else
    echo "Skipped (pacman not available)"
fi

echo -e "\n${BLUE}ℹ️  Benchmark: INFO (firefox)${NC}"
echo "-------------------------------"
if command -v pacman &>/dev/null && command -v yay &>/dev/null; then
    hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
        --export-markdown "$EXPORT_DIR/info.md" \
        --export-json "$EXPORT_DIR/info.json" \
        --command-name "OMG (Daemon)" "$OMG info firefox" \
        --command-name "pacman" "pacman -Si firefox" \
        --command-name "yay (--repo)" "yay -Si --repo firefox"
elif command -v pacman &>/dev/null; then
    hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
        --export-markdown "$EXPORT_DIR/info.md" \
        --export-json "$EXPORT_DIR/info.json" \
        --command-name "OMG (Daemon)" "$OMG info firefox" \
        --command-name "pacman" "pacman -Si firefox"
else
    echo "Skipped (pacman not available)"
fi

echo -e "\n${BLUE}⚡ Benchmark: STATUS${NC}"
echo "-------------------------------"
if [ -x "$OMG_FAST" ]; then
    hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
        --export-markdown "$EXPORT_DIR/status.md" \
        --export-json "$EXPORT_DIR/status.json" \
        --command-name "OMG (omg-fast)" "$OMG_FAST status" \
        --command-name "OMG (daemon)" "$OMG status"
else
    hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
        --export-markdown "$EXPORT_DIR/status.md" \
        --export-json "$EXPORT_DIR/status.json" \
        --command-name "OMG (daemon)" "$OMG status"
fi

echo -e "\n${BLUE}📋 Benchmark: EXPLICIT COUNT${NC}"
echo "-------------------------------"
$OMG explicit --count > /dev/null 2>&1 || true

if [ -x "$OMG_FAST" ]; then
    if command -v pacman &>/dev/null && command -v yay &>/dev/null; then
        hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
            --export-markdown "$EXPORT_DIR/explicit.md" \
            --export-json "$EXPORT_DIR/explicit.json" \
            --command-name "OMG (omg-fast)" "$OMG_FAST ec" \
            --command-name "OMG (daemon)" "$OMG explicit --count" \
            --command-name "pacman" "bash -o pipefail -c 'pacman -Qe | wc -l'" \
            --command-name "yay" "bash -o pipefail -c 'yay -Qe | wc -l'"
    elif command -v pacman &>/dev/null; then
        hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
            --export-markdown "$EXPORT_DIR/explicit.md" \
            --export-json "$EXPORT_DIR/explicit.json" \
            --command-name "OMG (omg-fast)" "$OMG_FAST ec" \
            --command-name "OMG (daemon)" "$OMG explicit --count" \
            --command-name "pacman" "bash -o pipefail -c 'pacman -Qe | wc -l'"
    else
        hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
            --export-markdown "$EXPORT_DIR/explicit.md" \
            --export-json "$EXPORT_DIR/explicit.json" \
            --command-name "OMG (omg-fast)" "$OMG_FAST ec" \
            --command-name "OMG (daemon)" "$OMG explicit --count"
    fi
else
    if command -v pacman &>/dev/null; then
        hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
            --export-markdown "$EXPORT_DIR/explicit.md" \
            --export-json "$EXPORT_DIR/explicit.json" \
            --command-name "OMG (daemon)" "$OMG explicit --count" \
            --command-name "pacman" "bash -o pipefail -c 'pacman -Qe | wc -l'"
    else
        hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
            --export-markdown "$EXPORT_DIR/explicit.md" \
            --export-json "$EXPORT_DIR/explicit.json" \
            --command-name "OMG (daemon)" "$OMG explicit --count"
    fi
fi

echo ""
echo "========================================================"
echo -e "${GREEN}✅ Benchmarks Complete!${NC}"
echo "========================================================"
echo ""
echo "Results saved to:"
echo "  📄 Markdown tables: $EXPORT_DIR/*.md"
echo "  📊 JSON data:       $EXPORT_DIR/*.json"
echo ""
echo -e "${YELLOW}Summary:${NC}"
echo ""

for md_file in "$EXPORT_DIR"/*.md; do
    if [ -f "$md_file" ]; then
        echo "$(basename "$md_file" .md):"
        cat "$md_file"
        echo ""
    fi
done

echo -e "${BLUE}💡 Tips:${NC}"
echo "  • JSON files can be used for CI regression detection"
echo "  • Compare with previous runs: hyperfine --export-json old.json ..."
echo "  • For cold cache benchmarks, see benchmark.sh --help"
