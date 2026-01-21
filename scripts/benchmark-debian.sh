#!/bin/bash
set -e

# ============================================================================
# OMG Debian/Ubuntu Performance Benchmark (True Comparison)
# ============================================================================

OMG="./target/release/omg"
OMGD="./target/release/omgd"
ITERATIONS=5
WARMUP=2

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "========================================================"
echo -e "${GREEN}🚀 OMG Debian/Ubuntu Performance Benchmark${NC}"
echo "========================================================"

# 1. Initial Sync
echo -e "\n[OMG] Syncing database..."
$OMG sync

# 2. Start Daemon
echo -e "[OMG] Starting Daemon..."
$OMGD --foreground > /tmp/omgd-bench.log 2>&1 &
DAEMON_PID=$!
sleep 5 # Give it time to index everything

cleanup() {
    kill $DAEMON_PID > /dev/null 2>&1 || true
}
trap cleanup EXIT

run_bench() {
    local label=$1
    local cmd=$2
    
    echo -n "  Running $label... " >&2
    
    # Warmup
    for ((i=1; i<=WARMUP; i++)); do
        eval "$cmd" > /dev/null 2>&1
    done
    
    local start=$(date +%s%N)
    for ((i=1; i<=ITERATIONS; i++)); do
        eval "$cmd" > /dev/null 2>&1
    done
    local end=$(date +%s%N)
    
    local avg_ns=$(( ($end - $start) / ITERATIONS ))
    local avg_ms=$(echo "scale=2; $avg_ns / 1000000" | bc)
    echo "${avg_ms}ms" >&2
    echo "$avg_ms"
}

# Search Test
echo -e "\n📦 Benchmark: SEARCH (vim)"
echo "-------------------------------"
APT_SEARCH=$(run_bench "apt-cache search" "apt-cache search vim")
OMG_SEARCH=$(run_bench "omg search" "$OMG search vim")
SEARCH_SPEEDUP=$(echo "scale=1; $APT_SEARCH / $OMG_SEARCH" | bc)
echo -e ">>> Speedup: ${GREEN}${SEARCH_SPEEDUP}x${NC}"

# Info Test
echo -e "\nℹ️  Benchmark: INFO (curl)"
echo "-------------------------------"
APT_INFO=$(run_bench "apt-cache show" "apt-cache show curl")
OMG_INFO=$(run_bench "omg info" "$OMG info curl")
INFO_SPEEDUP=$(echo "scale=1; $APT_INFO / $OMG_INFO" | bc)
echo -e ">>> Speedup: ${GREEN}${INFO_SPEEDUP}x${NC}"

# Status Test
echo -e "\n📋 Benchmark: STATUS"
echo "-------------------------------"
APT_STATUS=$(run_bench "dpkg -l (count)" "dpkg -l | wc -l")
OMG_STATUS=$(run_bench "omg status --fast" "$OMG status --fast")
STATUS_SPEEDUP=$(echo "scale=1; $APT_STATUS / $OMG_STATUS" | bc)
echo -e ">>> Speedup: ${GREEN}${STATUS_SPEEDUP}x${NC}"

# Suggestion Test
echo -e "\n🔍 Benchmark: SUGGEST (pyt)"
echo "-------------------------------"
OMG_SUGGEST=$(run_bench "omg suggest pyt" "$OMG suggest pyt")
echo -e ">>> Result: ${GREEN}${OMG_SUGGEST}ms${NC}"

# Full Metadata Test
echo -e "\n💎 Benchmark: INFO (full metadata)"
echo "-------------------------------"
OMG_INFO_FULL=$(run_bench "omg info --full" "$OMG info libc6")
echo -e ">>> Result: ${GREEN}${OMG_INFO_FULL}ms${NC}"

echo -e "\n========================================================"
echo -e "${GREEN}✅ Benchmarks Complete${NC}"
echo "========================================================"