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
#   ./benchmark-hyperfine.sh --help       # Show options
#
# ============================================================================

export PATH="$HOME/.cargo/bin:$PATH"

WARMUP=3
MIN_RUNS=10
FAST_MODE=false
EXPORT_DIR="benchmark_results"

print_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Options:
  --fast, -f    Run in fast mode (reduced warmup and runs)
  --help, -h    Show this help message

Environment Variables:
  OMG_BENCH_WARMUP       Number of warmup runs (default: 3, fast: 1)
  OMG_BENCH_RUNS         Minimum runs (default: 10, fast: 5)
  OMG_BENCH_EXPORT_DIR   Results directory (default: benchmark_results)

Examples:
  $0                           # Full benchmark (3 warmup, 10+ runs)
  $0 --fast                    # Fast benchmark (1 warmup, 5+ runs)
  OMG_BENCH_RUNS=20 $0         # Custom run count
EOF
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --fast|-f)
            FAST_MODE=true
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

echo -e "${BLUE}🔨 Building release binaries...${NC}"
cargo build --release --features arch --quiet

OMG="./target/release/omg"
OMGD="./target/release/omgd"
OMG_FAST="./target/release/omg-fast"

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

BENCH_DIR="$(mktemp -d -t omg-bench-XXXX)"
export OMG_DAEMON_DATA_DIR="$BENCH_DIR/data"
export OMG_SOCKET_PATH="$BENCH_DIR/omg.sock"
DAEMON_LOG="$BENCH_DIR/omgd.log"
mkdir -p "$OMG_DAEMON_DATA_DIR"

echo "Starting OMG Daemon..."
$OMGD --foreground > "$DAEMON_LOG" 2>&1 &
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
            --command-name "pacman" "pacman -Qe | wc -l" \
            --command-name "yay" "yay -Qe | wc -l"
    elif command -v pacman &>/dev/null; then
        hyperfine --warmup $WARMUP --min-runs $MIN_RUNS \
            --export-markdown "$EXPORT_DIR/explicit.md" \
            --export-json "$EXPORT_DIR/explicit.json" \
            --command-name "OMG (omg-fast)" "$OMG_FAST ec" \
            --command-name "OMG (daemon)" "$OMG explicit --count" \
            --command-name "pacman" "pacman -Qe | wc -l"
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
            --command-name "pacman" "pacman -Qe | wc -l"
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
