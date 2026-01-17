#!/bin/bash
set -e

# Use rustup toolchain to avoid cargo/rustc mismatch
export PATH="$HOME/.cargo/bin:$PATH"

# Configuration
ITERATIONS=10
WARMUP=2
OMG="./target/release/omg"
OMGD="./target/release/omgd"
BENCH_DIR="$(mktemp -d -t omg-bench-XXXX)"
export OMG_DAEMON_DATA_DIR="$BENCH_DIR/data"
export OMG_SOCKET_PATH="$BENCH_DIR/omg.sock"
DAEMON_LOG="$BENCH_DIR/omgd.log"
mkdir -p "$OMG_DAEMON_DATA_DIR"

if ! command -v bc >/dev/null 2>&1; then
    echo "❌ 'bc' is required for benchmarking (install: pacman -S bc)" >&2
    exit 1
fi

# Build release first
echo "🔨 Building release binaries..."
cargo build --release --features arch --quiet

echo "========================================================"
echo "🚀 OMG World-Class Performance Benchmark"
echo "========================================================"

# Start Daemon for max speed
echo "Starting OMG Daemon..."
$OMGD --foreground > "$DAEMON_LOG" 2>&1 &
DAEMON_PID=$!
sleep 2

if ! $OMG status > /dev/null 2>&1; then
    echo "❌ OMG daemon failed to start" >&2
    tail -n 50 "$DAEMON_LOG" >&2 || true
    kill $DAEMON_PID > /dev/null 2>&1 || true
    exit 1
fi

cleanup() {
    kill $DAEMON_PID > /dev/null 2>&1 || true
    rm -rf "$BENCH_DIR"
}
trap cleanup EXIT

# Arrays to store results for the final table
declare -A RESULTS
COMMANDS=("search" "info" "status" "explicit")
TARGETS=("OMG (Daemon)" "OMG (Cold)" "pacman" "yay")

run_bench() {
    local label=$1
    local cmd=$2
    local iters=$3
    local warm=$4
    
    echo -n "  Running $label... " >&2
    
    # Warmup
    for ((i=1; i<=warm; i++)); do
        eval "$cmd" > /dev/null 2>&1
    done
    
    local total=0
    local min=999999
    local max=0
    
    for ((i=1; i<=iters; i++)); do
        local start=$(date +%s%N)
        eval "$cmd" > /dev/null 2>&1
        local end=$(date +%s%N)
        local diff=$(( ($end - $start) / 1000000 ))
        
        total=$((total + diff))
        if (( diff < min )); then min=$diff; fi
        if (( diff > max )); then max=$diff; fi
    done
    
    local avg=$(echo "scale=2; $total / $iters" | bc)
    echo "${avg}ms (min: ${min}ms, max: ${max}ms)" >&2
    echo "$avg"
}

# 1. Search (firefox)
echo -e "\n📦 Benchmark: SEARCH (firefox)"
echo "-------------------------------"
RESULTS["search,OMG (Daemon)"]=$(run_bench "OMG (Daemon)" "$OMG search firefox" $ITERATIONS $WARMUP)

if command -v pacman &> /dev/null; then
    RESULTS["search,pacman"]=$(run_bench "pacman" "pacman -Ss firefox" $ITERATIONS $WARMUP)
fi
if command -v yay &> /dev/null; then
    RESULTS["search,yay"]=$(run_bench "yay" "yay -Ss firefox" $ITERATIONS $WARMUP)
fi

# 2. Info (firefox)
echo -e "\nℹ️  Benchmark: INFO (firefox)"
echo "-------------------------------"
RESULTS["info,OMG (Daemon)"]=$(run_bench "OMG (Daemon)" "$OMG info firefox" $ITERATIONS $WARMUP)
if command -v pacman &> /dev/null; then
    RESULTS["info,pacman"]=$(run_bench "pacman" "pacman -Si firefox" $ITERATIONS $WARMUP)
fi
if command -v yay &> /dev/null; then
    RESULTS["info,yay"]=$(run_bench "yay" "yay -Si firefox" $ITERATIONS $WARMUP)
fi

# 3. Status
echo -e "\n⚡ Benchmark: STATUS"
echo "-------------------------------"
RESULTS["status,OMG (Daemon)"]=$(run_bench "OMG (Daemon)" "$OMG status" $ITERATIONS $WARMUP)
RESULTS["status,pacman"]="N/A"
RESULTS["status,yay"]="N/A"

# 4. List Explicit
echo -e "\n📋 Benchmark: EXPLICIT"
echo "-------------------------------"
# Warm explicit cache once to hit daemon cache for measured runs
$OMG explicit --count > /dev/null 2>&1 || true
RESULTS["explicit,OMG (Daemon)"]=$(run_bench "OMG (Daemon)" "$OMG explicit --count" $ITERATIONS $WARMUP)
if command -v pacman &> /dev/null; then
    RESULTS["explicit,pacman"]=$(run_bench "pacman" "pacman -Qe" $ITERATIONS $WARMUP)
fi
if command -v yay &> /dev/null; then
    RESULTS["explicit,yay"]=$(run_bench "yay" "yay -Qe" $ITERATIONS $WARMUP)
fi

echo -e "\n========================================================"
echo "📊 Results Summary (Average Time in ms)"
echo "========================================================"

printf "| %-10s | %-12s | %-10s | %-10s | %-10s |\n" "Command" "OMG (Daemon)" "pacman" "yay" "Speedup"
printf "|------------|--------------|------------|------------|-----------|\n"

REPORT_FILE="benchmark_report.md"
{
    echo "# OMG Benchmark Report"
    echo
    echo "**Iterations:** ${ITERATIONS}  "
    echo "**Warmup:** ${WARMUP}"
    echo
    echo "## Test Environment"
    echo
    echo "- **OS:** $(uname -s)"
    echo "- **Kernel:** $(uname -r)"
    if command -v lscpu >/dev/null 2>&1; then
        echo "- **CPU:** $(lscpu | awk -F: '/Model name/ {gsub(/^ /, "", $2); print $2; exit}')"
        echo "- **CPU Cores:** $(lscpu | awk -F: '/^CPU\(s\)/ {gsub(/^ /, "", $2); print $2; exit}')"
    fi
    if command -v free >/dev/null 2>&1; then
        echo "- **RAM:** $(free -h | awk '/Mem:/ {print $2; exit}')"
    fi
    echo
    echo "| Command | OMG (Daemon) | pacman | yay | Speedup |"
    echo "|---------|--------------|--------|-----|---------|"
} > "$REPORT_FILE"

for cmd in "${COMMANDS[@]}"; do
    omg_d=${RESULTS["$cmd,OMG (Daemon)"]}
    pac=${RESULTS["$cmd,pacman"]}
    yay=${RESULTS["$cmd,yay"]}
    
    # Calculate speedup vs pacman if possible
    speedup="N/A"
    if [[ "$pac" != "N/A" && "$omg_d" != "0" ]]; then
        speedup=$(echo "scale=1; $pac / $omg_d" | bc 2>/dev/null || echo "N/A")
        speedup="${speedup}x"
    fi
    
    printf "| %-10s | %-12s | %-10s | %-10s | %-10s |\n" "$cmd" "${omg_d}ms" "${pac}ms" "${yay}ms" "$speedup"
    printf "| %s | %sms | %sms | %sms | %s |\n" "$cmd" "$omg_d" "$pac" "$yay" "$speedup" >> "$REPORT_FILE"
done

echo -e "\n✅ Benchmarks Complete"
echo "📄 Report saved to $REPORT_FILE"

