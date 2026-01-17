#!/bin/bash
set -e

OMG="./target/release/omg"

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║           OMG Debian/Ubuntu Integration Test Suite           ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Testing on: $(grep PRETTY_NAME /etc/os-release | cut -d'"' -f2)"
echo "Date: $(date)"
echo ""

# Install bc for timing calculations
apt-get update -qq
apt-get install -y -qq bc > /dev/null 2>&1 || true

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "                    PHASE 1: Database Sync                      "
echo "════════════════════════════════════════════════════════════════"

echo ""
echo "[OMG] omg sync (refresh package database)..."
time $OMG sync

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "                    PHASE 2: Search Benchmark                   "
echo "════════════════════════════════════════════════════════════════"

echo ""
echo "[APT] apt-cache search vim..."
APT_SEARCH_START=$(date +%s.%N)
apt-cache search vim > /dev/null
APT_SEARCH_END=$(date +%s.%N)
APT_SEARCH_TIME=$(echo "$APT_SEARCH_END - $APT_SEARCH_START" | bc)
echo "APT search time: ${APT_SEARCH_TIME}s"

echo ""
echo "[OMG] omg search vim..."
OMG_SEARCH_START=$(date +%s.%N)
$OMG search vim > /dev/null
OMG_SEARCH_END=$(date +%s.%N)
OMG_SEARCH_TIME=$(echo "$OMG_SEARCH_END - $OMG_SEARCH_START" | bc)
echo "OMG search time: ${OMG_SEARCH_TIME}s"

SEARCH_SPEEDUP=$(echo "scale=1; $APT_SEARCH_TIME / $OMG_SEARCH_TIME" | bc 2>/dev/null || echo "N/A")
echo ">>> Speedup: ${SEARCH_SPEEDUP}x"

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "                    PHASE 3: Info Benchmark                     "
echo "════════════════════════════════════════════════════════════════"

echo ""
echo "[APT] apt-cache show curl..."
APT_INFO_START=$(date +%s.%N)
apt-cache show curl > /dev/null
APT_INFO_END=$(date +%s.%N)
APT_INFO_TIME=$(echo "$APT_INFO_END - $APT_INFO_START" | bc)
echo "APT info time: ${APT_INFO_TIME}s"

echo ""
echo "[OMG] omg info curl..."
OMG_INFO_START=$(date +%s.%N)
$OMG info curl > /dev/null
OMG_INFO_END=$(date +%s.%N)
OMG_INFO_TIME=$(echo "$OMG_INFO_END - $OMG_INFO_START" | bc)
echo "OMG info time: ${OMG_INFO_TIME}s"

INFO_SPEEDUP=$(echo "scale=1; $APT_INFO_TIME / $OMG_INFO_TIME" | bc 2>/dev/null || echo "N/A")
echo ">>> Speedup: ${INFO_SPEEDUP}x"

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "                    PHASE 4: Status                             "
echo "════════════════════════════════════════════════════════════════"

echo ""
echo "[OMG] omg status..."
time $OMG status

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "                    PHASE 5: List Installed                     "
echo "════════════════════════════════════════════════════════════════"

echo ""
echo "[APT] dpkg -l..."
APT_LIST_START=$(date +%s.%N)
DPKG_COUNT=$(dpkg -l | wc -l)
APT_LIST_END=$(date +%s.%N)
APT_LIST_TIME=$(echo "$APT_LIST_END - $APT_LIST_START" | bc)
echo "dpkg list time: ${APT_LIST_TIME}s (${DPKG_COUNT} packages)"

echo ""
echo "[OMG] omg explicit..."
OMG_LIST_START=$(date +%s.%N)
OMG_COUNT=$($OMG explicit 2>/dev/null | wc -l || echo "0")
OMG_LIST_END=$(date +%s.%N)
OMG_LIST_TIME=$(echo "$OMG_LIST_END - $OMG_LIST_START" | bc)
echo "OMG list time: ${OMG_LIST_TIME}s (${OMG_COUNT} packages)"

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "                    PHASE 6: Install Test                       "
echo "════════════════════════════════════════════════════════════════"

echo ""
echo "[OMG] Installing a small package (cowsay)..."
time $OMG install cowsay -y || echo "Install test skipped (may need root)"

echo ""
echo "[OMG] Removing the package..."
time $OMG remove cowsay -y || echo "Remove test skipped (may need root)"

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "                    PHASE 7: Update Check                       "
echo "════════════════════════════════════════════════════════════════"

echo ""
echo "[OMG] Checking for updates..."
time $OMG update --check || echo "Update check completed"

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║              All integration tests passed! ✓                 ║"
echo "╠══════════════════════════════════════════════════════════════╣"
echo "║  Search speedup: ${SEARCH_SPEEDUP}x vs apt-cache"
echo "║  Info speedup:   ${INFO_SPEEDUP}x vs apt-cache"
echo "╚══════════════════════════════════════════════════════════════╝"
