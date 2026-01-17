#!/bin/bash
# Benchmark OMG vs APT on Debian/Ubuntu
set -e

OMG="/omg/target/release/omg"
OMGD="/omg/target/release/omgd"
ITERATIONS=10
WARMUP=2

echo "════════════════════════════════════════════════════════════"
echo "  🚀 OMG vs APT Benchmark (Ubuntu 24.04)"
echo "════════════════════════════════════════════════════════════"
echo ""

# Ensure apt cache is updated
echo "Updating apt cache..."
apt-get update -qq 2>/dev/null || true

# Start daemon for cached performance
echo "Starting OMG daemon..."
$OMGD --foreground > /tmp/omgd.log 2>&1 &
DAEMON_PID=$!
sleep 3

# Verify daemon is running
if ! $OMG status >/dev/null 2>&1; then
    echo "⚠️  Daemon failed to start, running without cache"
    DAEMON_PID=""
fi

cleanup() {
    [ -n "$DAEMON_PID" ] && kill $DAEMON_PID 2>/dev/null || true
}
trap cleanup EXIT

declare -A RESULTS

run_bench() {
    local label=$1
    local cmd=$2
    local iters=$3
    local warm=$4
    
    # Warmup
    for ((i=1; i<=warm; i++)); do
        eval "$cmd" > /dev/null 2>&1 || true
    done
    
    local total=0
    local min=999999
    local max=0
    
    for ((i=1; i<=iters; i++)); do
        local start=$(date +%s%N)
        eval "$cmd" > /dev/null 2>&1 || true
        local end=$(date +%s%N)
        local diff=$(( ($end - $start) / 1000000 ))
        
        total=$((total + diff))
        if (( diff < min )); then min=$diff; fi
        if (( diff > max )); then max=$diff; fi
    done
    
    local avg=$((total / iters))
    echo "$avg"
}

echo "📦 Benchmark: SEARCH (firefox)"
echo "────────────────────────────────"
echo -n "  OMG (Daemon)... "
OMG_SEARCH=$(run_bench "OMG" "$OMG search firefox" $ITERATIONS $WARMUP)
echo "${OMG_SEARCH}ms"

echo -n "  apt-cache... "
APT_SEARCH=$(run_bench "APT" "apt-cache search firefox" $ITERATIONS $WARMUP)
echo "${APT_SEARCH}ms"

echo ""
echo "ℹ️  Benchmark: INFO (curl)"
echo "────────────────────────────────"
echo -n "  OMG (Daemon)... "
OMG_INFO=$(run_bench "OMG" "$OMG info curl" $ITERATIONS $WARMUP)
echo "${OMG_INFO}ms"

echo -n "  apt-cache... "
APT_INFO=$(run_bench "APT" "apt-cache show curl" $ITERATIONS $WARMUP)
echo "${APT_INFO}ms"

echo ""
echo "📋 Benchmark: LIST INSTALLED"
echo "────────────────────────────────"
echo -n "  OMG... "
OMG_LIST=$(run_bench "OMG" "$OMG list" $ITERATIONS $WARMUP)
echo "${OMG_LIST}ms"

echo -n "  dpkg... "
DPKG_LIST=$(run_bench "dpkg" "dpkg -l" $ITERATIONS $WARMUP)
echo "${DPKG_LIST}ms"

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  📊 RESULTS SUMMARY"
echo "════════════════════════════════════════════════════════════"
echo ""
printf "| %-10s | %-12s | %-12s | %-10s |\n" "Command" "OMG" "apt-cache" "Speedup"
printf "|------------|--------------|--------------|------------|\n"

# Search speedup
if [ "$OMG_SEARCH" -gt 0 ]; then
    SEARCH_SPEEDUP=$((APT_SEARCH / OMG_SEARCH))
else
    SEARCH_SPEEDUP="∞"
fi
printf "| %-10s | %-12s | %-12s | %-10s |\n" "search" "${OMG_SEARCH}ms" "${APT_SEARCH}ms" "${SEARCH_SPEEDUP}x"

# Info speedup
if [ "$OMG_INFO" -gt 0 ]; then
    INFO_SPEEDUP=$((APT_INFO / OMG_INFO))
else
    INFO_SPEEDUP="∞"
fi
printf "| %-10s | %-12s | %-12s | %-10s |\n" "info" "${OMG_INFO}ms" "${APT_INFO}ms" "${INFO_SPEEDUP}x"

# List speedup
if [ "$OMG_LIST" -gt 0 ]; then
    LIST_SPEEDUP=$((DPKG_LIST / OMG_LIST))
else
    LIST_SPEEDUP="∞"
fi
printf "| %-10s | %-12s | %-12s | %-10s |\n" "list" "${OMG_LIST}ms" "${DPKG_LIST}ms (dpkg)" "${LIST_SPEEDUP}x"

echo ""
echo "✅ Benchmark Complete"
