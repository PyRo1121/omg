#!/bin/bash
# System Crash Diagnostics Script
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}=== SYSTEM CRASH DIAGNOSTICS ===${NC}\n"

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   echo -e "${RED}Run with sudo:${NC} sudo bash diag.sh"
   exit 1
fi

echo -e "${YELLOW}[1/6] BTRFS Device Statistics${NC}"
echo "─────────────────────────────────"
btrfs device stats / 2>/dev/null || echo "Could not read BTRFS stats"
echo ""

echo -e "${YELLOW}[2/6] BTRFS Filesystem Status${NC}"
echo "─────────────────────────────────"
btrfs filesystem show 2>/dev/null
btrfs filesystem df / 2>/dev/null
echo ""

echo -e "${YELLOW}[3/6] Recent BTRFS Kernel Messages${NC}"
echo "─────────────────────────────────"
dmesg | grep -iE "(btrfs|transaction|abort|csum|corrupt)" | tail -20 || echo "No BTRFS errors in dmesg"
echo ""

echo -e "${YELLOW}[4/6] Disk SMART Health${NC}"
echo "─────────────────────────────────"
if command -v smartctl &> /dev/null; then
    for disk in /dev/sda /dev/nvme0n1; do
        if [[ -e $disk ]]; then
            echo -e "\n${GREEN}$disk:${NC}"
            smartctl -H $disk 2>/dev/null | grep -E "(Health|PASSED|FAILED)" || echo "Could not read SMART"
            smartctl -A $disk 2>/dev/null | grep -iE "(reallocat|pending|uncorrect|error)" || true
        fi
    done
else
    echo -e "${RED}smartmontools not installed. Installing...${NC}"
    pacman -S smartmontools --noconfirm 2>/dev/null && smartctl -H /dev/sda
fi
echo ""

echo -e "${YELLOW}[5/6] Memory Error Check${NC}"
echo "─────────────────────────────────"
if [[ -f /sys/devices/system/edac/mc/mc0/ce_count ]]; then
    ce=$(cat /sys/devices/system/edac/mc/mc0/ce_count 2>/dev/null || echo "0")
    ue=$(cat /sys/devices/system/edac/mc/mc0/ue_count 2>/dev/null || echo "0")
    echo "Correctable errors: $ce"
    echo "Uncorrectable errors: $ue"
else
    echo "EDAC not available (ECC RAM reporting)"
fi
journalctl -b -p err --no-pager 2>/dev/null | grep -iE "(mce|memory|hardware error)" | tail -5 || echo "No memory errors in journal"
echo ""

echo -e "${YELLOW}[6/6] Starting BTRFS Scrub${NC}"
echo "─────────────────────────────────"
echo "This checks all data checksums (may take a while)..."
btrfs scrub start / 2>/dev/null
sleep 3
btrfs scrub status /
echo ""
echo -e "${GREEN}Scrub running in background. Check status with:${NC}"
echo "  sudo btrfs scrub status /"
echo ""

echo -e "${YELLOW}=== SUMMARY ===${NC}"
echo "If you see errors above, the likely causes are:"
echo "  - BTRFS stats showing errors → filesystem corruption"
echo "  - SMART showing reallocated/pending sectors → failing disk"
echo "  - Memory errors → bad RAM (run memtest86+ from GRUB)"
echo ""
echo "To check scrub results later: sudo btrfs scrub status /"
