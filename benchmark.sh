#!/bin/bash
set -e

# Configuration
ITERATIONS=10
WARMUP=2
OMG="./target/release/omg"
OMGD="./target/release/omgd"

# Build release first
echo "🔨 Building release binaries..."
cargo build --release --quiet

echo "========================================================"
echo "🚀 OMG World-Class Performance Benchmark"
echo "========================================================"

# Start Daemon for max speed
echo "Starting OMG Daemon..."
$OMGD --foreground > /dev/null 2>&1 &
DAEMON_PID=$!
sleep 2

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

# Cold start means killing daemon first or just running it normally if daemon is off
# However, for a fair comparison of "Cold", we just run it. 
# But let's assume "Cold" is without the daemon bridge. 
# Since we implemented sync paths that try daemon first, "Cold" is just without a running daemon.
# To simulate cold start while daemon is running, we can't easily bypass unless we add a flag.
# For now, let's just benchmark what we have.

# Stop daemon for cold benchmark? No, let's just do it at the end.
# Actually, let's just compare OMG (Daemon) vs Others first.

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
# Pacman doesn't have a direct equivalent to 'status', maybe 'check'? 
# Yay has 'vget', but let's skip for now or use a placeholder.
RESULTS["status,pacman"]="N/A"
RESULTS["status,yay"]="N/A"

# 4. List Explicit
echo -e "\n📋 Benchmark: EXPLICIT"
echo "-------------------------------"
RESULTS["explicit,OMG (Daemon)"]=$(run_bench "OMG (Daemon)" "$OMG explicit" $ITERATIONS $WARMUP)
if command -v pacman &> /dev/null; then
    RESULTS["explicit,pacman"]=$(run_bench "pacman" "pacman -Qe" $ITERATIONS $WARMUP)
fi
if command -v yay &> /dev/null; then
    RESULTS["explicit,yay"]=$(run_bench "yay" "yay -Qe" $ITERATIONS $WARMUP)
fi

# Kill Daemon
kill $DAEMON_PID > /dev/null 2>&1 || true

# 5. Cold Starts (without daemon)
echo -e "\n❄️  Benchmark: COLD STARTS (No Daemon)"
echo "-------------------------------"
# Note: we don't restart the daemon here.
RESULTS["search,OMG (Cold)"]=$(run_bench "OMG (Cold) Search" "$OMG search firefox" $ITERATIONS 0)
RESULTS["info,OMG (Cold)"]=$(run_bench "OMG (Cold) Info" "$OMG info firefox" $ITERATIONS 0)
RESULTS["status,OMG (Cold)"]=$(run_bench "OMG (Cold) Status" "$OMG status" $ITERATIONS 0)
RESULTS["explicit,OMG (Cold)"]=$(run_bench "OMG (Cold) Explicit" "$OMG explicit" $ITERATIONS 0)


echo -e "\n========================================================"
echo "📊 Results Summary (Average Time in ms)"
echo "========================================================"

printf "| %-10s | %-12s | %-10s | %-10s | %-10s | %-10s |\n" "Command" "OMG (Daemon)" "OMG (Cold)" "pacman" "yay" "Speedup"
printf "|------------|--------------|------------|------------|------------|-----------|\n"

for cmd in "${COMMANDS[@]}"; do
    omg_d=${RESULTS["$cmd,OMG (Daemon)"]}
    omg_c=${RESULTS["$cmd,OMG (Cold)"]}
    pac=${RESULTS["$cmd,pacman"]}
    yay=${RESULTS["$cmd,yay"]}
    
    # Calculate speedup vs pacman if possible
    speedup="N/A"
    if [[ "$pac" != "N/A" && "$omg_d" != "0" ]]; then
        speedup=$(echo "scale=1; $pac / $omg_d" | bc 2>/dev/null || echo "N/A")
        speedup="${speedup}x"
    fi
    
    printf "| %-10s | %-12s | %-10s | %-10s | %-10s | %-10s |\n" "$cmd" "${omg_d}ms" "${omg_c}ms" "${pac}ms" "${yay}ms" "$speedup"
done

echo -e "\n✅ Benchmarks Complete"

