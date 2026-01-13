#!/bin/bash
set -e

# Build release first
cargo build --release --quiet

OMG="./target/release/omg"

echo "Running COMPREHENSIVE benchmarks..."
echo "------------------------------------------------"

# Start Daemon for max speed
echo "Starting Daemon..."
./target/release/omgd --foreground > /dev/null 2>&1 &
DAEMON_PID=$!
sleep 2

# Warm up cache
$OMG search firefox > /dev/null

run_bench() {
    name=$1
    cmd=$2
    start=$(date +%s%N)
    eval "$cmd" > /dev/null
    end=$(date +%s%N)
    time=$(( ($end - $start) / 1000000 ))
    echo "$name: ${time} ms"
}

run_bench "Search 'firefox' (Daemon)" "$OMG search firefox"
run_bench "Info 'firefox' (Daemon)" "$OMG info firefox"
run_bench "Status (Daemon)" "$OMG status"
run_bench "List Explicit (Daemon)" "$OMG explicit"
run_bench "List Node Versions" "$OMG list node"
run_bench "Help" "$OMG --help"
run_bench "Version" "$OMG --version"

# Kill Daemon
kill $DAEMON_PID > /dev/null 2>&1 || true

echo "------------------------------------------------"
echo "Cold Start Benchmarks (No Daemon)"
run_bench "Status (Cold)" "$OMG status"
run_bench "Info 'firefox' (Cold)" "$OMG info firefox"

echo "------------------------------------------------"

if command -v pacman &> /dev/null; then
    start=$(date +%s%N)
    pacman -Ss firefox > /dev/null
    end=$(date +%s%N)
    pacman_time=$(( ($end - $start) / 1000000 ))
    echo "Pacman: ${pacman_time} ms"
    
    if [ $omg_time -gt 0 ]; then
        # speedup=$(echo "scale=2; $pacman_time / $omg_time" | bc)
        echo "Compare manually: Pacman ($pacman_time) vs OMG ($omg_time)"
    fi
else
    echo "Pacman not found, skipping comparison"
fi

echo "------------------------------------------------"

# 2. Version Listing (Node)
echo "Benchmarking: List Node versions (Local)"
start=$(date +%s%N)
$OMG list node > /dev/null
end=$(date +%s%N)
echo "OMG: $(( ($end - $start) / 1000000 )) ms"

echo "------------------------------------------------"

# 3. Startup Time
echo "Benchmarking: Startup (Version check)"
start=$(date +%s%N)
$OMG --version > /dev/null
end=$(date +%s%N)
echo "OMG: $(( ($end - $start) / 1000000 )) ms"

echo "------------------------------------------------"
