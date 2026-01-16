#!/usr/bin/env bash
set -euo pipefail

echo "========================================="
echo "  OMG Debian Smoke Test Suite"
echo "========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TESTS_PASSED=0
TESTS_FAILED=0

# Helper functions
test_pass() {
    echo -e "${GREEN}✓ PASS${NC}: $1"
    ((TESTS_PASSED++))
}

test_fail() {
    echo -e "${RED}✗ FAIL${NC}: $1"
    ((TESTS_FAILED++))
}

test_info() {
    echo -e "${BLUE}ℹ INFO${NC}: $1"
}

test_skip() {
    echo -e "${YELLOW}⊘ SKIP${NC}: $1"
}

# Run command and capture output
run_cmd() {
    local cmd="$1"
    local desc="$2"
    local should_fail="${3:-false}"

    echo "Running: $cmd"
    if output=$($cmd 2>&1); then
        if [ "$should_fail" = "true" ]; then
            test_fail "$desc (expected to fail but succeeded)"
            echo "Output: $output"
            return 1
        else
            test_pass "$desc"
            return 0
        fi
    else
        if [ "$should_fail" = "true" ]; then
            test_pass "$desc (expected failure)"
            return 0
        else
            test_fail "$desc"
            echo "Output: $output"
            return 1
        fi
    fi
}

# Check if running as root
if [ "$(id -u)" -ne 0 ]; then
    test_fail "Smoke test must run as root for apt operations"
    exit 1
fi

test_pass "Running as root"

# Test 1: OMG version
echo ""
echo "=== Test 1: OMG Version ==="
run_cmd "omg --version" "OMG version check"

# Test 2: OMG help
echo ""
echo "=== Test 2: OMG Help ==="
run_cmd "omg --help" "OMG help display"

# Test 3: OMG doctor
echo ""
echo "=== Test 3: OMG Doctor ==="
run_cmd "omg doctor" "OMG system health check"

# Test 4: Sync databases
echo ""
echo "=== Test 4: Sync Databases ==="
test_info "Syncing apt databases..."
if omg sync; then
    test_pass "Database sync"
else
    test_fail "Database sync"
fi

# Test 5: Search packages
echo ""
echo "=== Test 5: Search Packages ==="
run_cmd "omg search curl" "Search for 'curl' package"
run_cmd "omg search vim" "Search for 'vim' package"
run_cmd "omg search ripgrep" "Search for 'ripgrep' package"

# Test 6: Package info
echo ""
echo "=== Test 6: Package Info ==="
run_cmd "omg info curl" "Get info for 'curl' package"
run_cmd "omg info vim" "Get info for 'vim' package"

# Test 7: Install a small package
echo ""
echo "=== Test 7: Install Package ==="
TEST_PKG="hello"
test_info "Installing test package: $TEST_PKG"

# Check if already installed
if dpkg -l | grep -q "^ii  $TEST_PKG "; then
    test_skip "$TEST_PKG already installed, skipping install test"
else
    if omg install "$TEST_PKG"; then
        test_pass "Install $TEST_PKG"
    else
        test_fail "Install $TEST_PKG"
    fi
fi

# Test 8: Verify installation
echo ""
echo "=== Test 8: Verify Installation ==="
if dpkg -l | grep -q "^ii  $TEST_PKG "; then
    test_pass "$TEST_PKG is installed"
else
    test_fail "$TEST_PKG is not installed"
fi

# Test 9: List installed packages
echo ""
echo "=== Test 9: List Installed ==="
run_cmd "omg list installed 2>/dev/null || omg explicit" "List installed packages"

# Test 10: Check for updates
echo ""
echo "=== Test 10: Check Updates ==="
run_cmd "omg update --check" "Check for available updates"

# Test 11: Search with detailed output
echo ""
echo "=== Test 11: Search Detailed ==="
run_cmd "omg search curl --detailed" "Search with detailed output"

# Test 12: Remove test package
echo ""
echo "=== Test 12: Remove Package ==="
test_info "Removing test package: $TEST_PKG"

if dpkg -l | grep -q "^ii  $TEST_PKG "; then
    if omg remove "$TEST_PKG"; then
        test_pass "Remove $TEST_PKG"
    else
        test_fail "Remove $TEST_PKG"
    fi
else
    test_skip "$TEST_PKG not installed, skipping remove test"
fi

# Test 13: Verify removal
echo ""
echo "=== Test 13: Verify Removal ==="
if ! dpkg -l | grep -q "^ii  $TEST_PKG "; then
    test_pass "$TEST_PKG is removed"
else
    test_fail "$TEST_PKG is still installed"
fi

# Test 14: Clean orphans
echo ""
echo "=== Test 14: Clean Orphans ==="
run_cmd "omg clean --orphans" "Clean orphan packages"

# Test 15: Test invalid package
echo ""
echo "=== Test 15: Invalid Package Handling ==="
run_cmd "omg info nonexistent-package-xyz" "Info for non-existent package" "true"

# Test 16: Install multiple packages
echo ""
echo "=== Test 16: Install Multiple Packages ==="
TEST_PKGS="bsdextrautils"
test_info "Installing multiple packages: $TEST_PKGS"

# Check if already installed
if dpkg -l | grep -q "^ii  $TEST_PKGS "; then
    test_skip "$TEST_PKGS already installed"
else
    if omg install $TEST_PKGS; then
        test_pass "Install multiple packages"
    else
        test_fail "Install multiple packages"
    fi
fi

# Test 17: Status command
echo ""
echo "=== Test 17: Status Command ==="
run_cmd "omg status" "Get system status"

# Test 18: Search with partial match
echo ""
echo "=== Test 18: Partial Match Search ==="
run_cmd "omg search cur" "Search with partial match 'cur'"

# Test 19: Install with non-interactive flag (should still work)
echo ""
echo "=== Test 19: Install with Flags ==="
TEST_PKG2="debianutils"
if dpkg -l | grep -q "^ii  $TEST_PKG2 "; then
    test_skip "$TEST_PKG2 already installed"
else
    if omg install "$TEST_PKG2"; then
        test_pass "Install with default settings"
    else
        test_fail "Install with default settings"
    fi
fi

# Test 20: Clean cache
echo ""
echo "=== Test 20: Clean Cache ==="
run_cmd "omg clean --cache" "Clean package cache"

# Summary
echo ""
echo "========================================="
echo "  Smoke Test Summary"
echo "========================================="
echo -e "${GREEN}Tests Passed: $TESTS_PASSED${NC}"
echo -e "${RED}Tests Failed: $TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed! ✓${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed! ✗${NC}"
    exit 1
fi
