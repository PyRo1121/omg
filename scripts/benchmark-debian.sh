#!/bin/bash
# Benchmark OMG vs APT vs Nala on Debian/Ubuntu
set -e

OMG="/omg/target/release/omg"
ITERATIONS=5

echo "========================================"
echo "  OMG vs APT vs Nala Benchmark"
echo "========================================"
echo ""

# Ensure apt cache is updated
apt-get update -qq 2>/dev/null

# Install nala if available
if ! command -v nala &>/dev/null; then
    echo "[INFO] Nala not installed, skipping nala benchmarks"
    HAS_NALA=0
else
    HAS_NALA=1
fi

benchmark() {
    local name="$1"
    local cmd="$2"
    local total=0
    
    for i in $(seq 1 $ITERATIONS); do
        start=$(date +%s%N)
        eval "$cmd" >/dev/null 2>&1
        end=$(date +%s%N)
        elapsed=$(( (end - start) / 1000000 ))
        total=$((total + elapsed))
    done
    
    avg=$((total / ITERATIONS))
    echo "  $name: ${avg}ms (avg of $ITERATIONS runs)"
}

echo "==> Search 'vim'"
echo "---"
benchmark "OMG" "$OMG search vim"
benchmark "APT" "apt-cache search vim"
[ "$HAS_NALA" = "1" ] && benchmark "Nala" "nala search vim"
echo ""

echo "==> Package Info 'curl'"
echo "---"
benchmark "OMG" "$OMG info curl"
benchmark "APT" "apt-cache show curl"
[ "$HAS_NALA" = "1" ] && benchmark "Nala" "nala show curl"
echo ""

echo "==> Search 'firefox'"
echo "---"
benchmark "OMG" "$OMG search firefox"
benchmark "APT" "apt-cache search firefox"
[ "$HAS_NALA" = "1" ] && benchmark "Nala" "nala search firefox"
echo ""

echo "==> Search 'python'"
echo "---"
benchmark "OMG" "$OMG search python"
benchmark "APT" "apt-cache search python"
[ "$HAS_NALA" = "1" ] && benchmark "Nala" "nala search python"
echo ""

echo "========================================"
echo "  Benchmark Complete"
echo "========================================"
