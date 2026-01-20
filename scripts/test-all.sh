#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# OMG Pre-Push Test Suite
# ═══════════════════════════════════════════════════════════════════════════════
#
# World-class testing system with segmented test categories.
# Run before pushing to GitHub to ensure code quality.
#
# Usage:
#   ./scripts/test-all.sh           # Run all tests
#   ./scripts/test-all.sh --quick   # Quick tests only (no integration)
#   ./scripts/test-all.sh --segment core    # Run only core tests
#   ./scripts/test-all.sh --segment runtimes
#   ./scripts/test-all.sh --segment cli
#   ./scripts/test-all.sh --segment security
#   ./scripts/test-all.sh --segment integration
#
# Exit codes:
#   0 - All tests passed
#   1 - Test failure
#   2 - Build failure
#   3 - Lint failure

# Don't exit on error - we handle errors ourselves
set +e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0
SKIPPED=0

# Parse arguments
QUICK=false
SEGMENT=""
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick|-q)
            QUICK=true
            shift
            ;;
        --segment|-s)
            SEGMENT="$2"
            shift 2
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --quick, -q          Run quick tests only (skip integration)"
            echo "  --segment, -s NAME   Run only specific segment"
            echo "  --verbose, -v        Show verbose output"
            echo "  --help, -h           Show this help"
            echo ""
            echo "Segments:"
            echo "  lint       - Formatting and clippy"
            echo "  core       - Core module unit tests"
            echo "  runtimes   - Runtime manager tests"
            echo "  cli        - CLI argument and command tests"
            echo "  packages   - Package manager tests"
            echo "  security   - Security and input validation tests"
            echo "  property   - Property-based tests"
            echo "  integration - Full integration tests"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Header
echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║${NC}                    ${BOLD}OMG Pre-Push Test Suite${NC}                                   ${CYAN}║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Get start time
START_TIME=$(date +%s)

# Function to run a test segment
run_segment() {
    local name="$1"
    local cmd="$2"
    local required="${3:-true}"
    
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}▶ ${name}${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    
    local start=$(date +%s%3N)
    
    local exit_code=0
    
    if $VERBOSE; then
        eval "$cmd" || exit_code=$?
    else
        eval "$cmd" > /tmp/omg_test_output.txt 2>&1 || exit_code=$?
    fi
    
    local end=$(date +%s%3N)
    local duration=$((end - start))
    
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}✓ ${name} passed${NC} (${duration}ms)"
        ((PASSED++))
    else
        echo -e "${RED}✗ ${name} failed${NC} (${duration}ms)"
        if ! $VERBOSE; then
            echo ""
            echo -e "${YELLOW}Output:${NC}"
            tail -30 /tmp/omg_test_output.txt
            echo ""
        fi
        ((FAILED++))
        if [ "$required" = "true" ]; then
            return 1
        fi
    fi
    return 0
}

# Function to check if segment should run
should_run() {
    local seg="$1"
    if [ -z "$SEGMENT" ]; then
        return 0
    fi
    if [ "$SEGMENT" = "$seg" ]; then
        return 0
    fi
    return 1
}

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 1: LINT (Formatting & Clippy)
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "lint"; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 1: LINT${NC} - Code formatting and static analysis                        ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "Format Check" "rustup run nightly cargo fmt -- --check"
    run_segment "Clippy (warnings as errors)" "rustup run nightly cargo clippy --features arch -- -D warnings"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 2: BUILD
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "build" || [ -z "$SEGMENT" ]; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 2: BUILD${NC} - Compilation check                                         ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "Debug Build" "rustup run nightly cargo build --features arch"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 3: CORE UNIT TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "core"; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 3: CORE${NC} - Core module unit tests                                     ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "Database Tests" "rustup run nightly cargo test --features arch --lib core::database"
    run_segment "Completion Tests" "rustup run nightly cargo test --features arch --lib core::completion"
    run_segment "Container Tests" "rustup run nightly cargo test --features arch --lib core::container"
    run_segment "Security Tests" "rustup run nightly cargo test --features arch --lib core::security"
    run_segment "System Info Tests" "rustup run nightly cargo test --features arch --lib core::sysinfo"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 4: RUNTIME MANAGER TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "runtimes"; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 4: RUNTIMES${NC} - Runtime manager unit tests                             ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "Common Utilities" "rustup run nightly cargo test --features arch --lib runtimes::common"
    run_segment "Node.js Manager" "rustup run nightly cargo test --features arch --lib runtimes::node"
    run_segment "Python Manager" "rustup run nightly cargo test --features arch --lib runtimes::python"
    run_segment "Go Manager" "rustup run nightly cargo test --features arch --lib runtimes::go"
    run_segment "Bun Manager" "rustup run nightly cargo test --features arch --lib runtimes::bun"
    run_segment "Ruby Manager" "rustup run nightly cargo test --features arch --lib runtimes::ruby"
    run_segment "Java Manager" "rustup run nightly cargo test --features arch --lib runtimes::java"
    run_segment "Rust Manager" "rustup run nightly cargo test --features arch --lib runtimes::rust"
    run_segment "Mise Manager" "rustup run nightly cargo test --features arch --lib runtimes::mise"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 5: CLI TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "cli"; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 5: CLI${NC} - Command-line interface tests                                ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "CLI Args Parsing" "rustup run nightly cargo test --features arch --lib cli::args"
    run_segment "Hooks Tests" "rustup run nightly cargo test --features arch --lib hooks"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 6: PACKAGE MANAGER TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "packages"; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 6: PACKAGES${NC} - Package manager tests                                  ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "Pacman DB Tests" "rustup run nightly cargo test --features arch --lib package_managers::pacman_db"
    run_segment "Parallel Sync Tests" "rustup run nightly cargo test --features arch --lib package_managers::parallel_sync"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 7: SECURITY TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "security"; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 7: SECURITY${NC} - Security and input validation tests                    ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "Input Validation" "rustup run nightly cargo test --features arch --test security_tests input_validation"
    run_segment "Privilege Tests" "rustup run nightly cargo test --features arch --test security_tests privilege_tests"
    run_segment "Filesystem Security" "rustup run nightly cargo test --features arch --test security_tests filesystem_security"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 8: PROPERTY-BASED TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "property" && [ "$QUICK" = "false" ]; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 8: PROPERTY${NC} - Property-based fuzzing tests                           ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "Property Tests" "rustup run nightly cargo test --features arch --test property_tests" "false"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 9: COMPREHENSIVE TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "comprehensive"; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 9: COMPREHENSIVE${NC} - All CLI commands and features                     ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "CLI Help Tests" "rustup run nightly cargo test --features arch --test comprehensive_tests cli_help"
    run_segment "CLI Search Tests" "rustup run nightly cargo test --features arch --test comprehensive_tests cli_search"
    run_segment "CLI Info Tests" "rustup run nightly cargo test --features arch --test comprehensive_tests cli_info"
    run_segment "CLI Runtime Tests" "rustup run nightly cargo test --features arch --test comprehensive_tests cli_runtimes"
    run_segment "CLI Env Tests" "rustup run nightly cargo test --features arch --test comprehensive_tests cli_env"
    run_segment "CLI Tool Tests" "rustup run nightly cargo test --features arch --test comprehensive_tests cli_tool"
    run_segment "CLI Status Tests" "rustup run nightly cargo test --features arch --test comprehensive_tests cli_status"
    run_segment "Project Detection" "rustup run nightly cargo test --features arch --test comprehensive_tests project_detection"
    run_segment "Error Handling" "rustup run nightly cargo test --features arch --test comprehensive_tests error_handling"
    run_segment "File Handling" "rustup run nightly cargo test --features arch --test comprehensive_tests file_handling"
    run_segment "Edge Cases" "rustup run nightly cargo test --features arch --test comprehensive_tests edge_cases"
    run_segment "Performance" "rustup run nightly cargo test --features arch --test comprehensive_tests performance"
    run_segment "Concurrency" "rustup run nightly cargo test --features arch --test comprehensive_tests concurrency"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 10: INTEGRATION TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "integration" && [ "$QUICK" = "false" ]; then
    echo ""
    echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
    echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 10: INTEGRATION${NC} - Full integration tests                             ${CYAN}│${NC}"
    echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"
    
    run_segment "Arch Integration" "rustup run nightly cargo test --features arch --test arch_tests" "false"
    run_segment "Integration Suite" "rustup run nightly cargo test --features arch --test integration_suite" "false"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SUMMARY
# ═══════════════════════════════════════════════════════════════════════════════

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║${NC}                           ${BOLD}TEST SUMMARY${NC}                                        ${CYAN}║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  ${GREEN}✓ Passed:${NC}  ${PASSED}"
echo -e "  ${RED}✗ Failed:${NC}  ${FAILED}"
echo -e "  ${YELLOW}⊘ Skipped:${NC} ${SKIPPED}"
echo -e "  ${BLUE}⏱ Duration:${NC} ${DURATION}s"
echo ""

if [ "$FAILED" -gt 0 ]; then
    echo -e "${RED}╔═══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║${NC}  ${BOLD}TESTS FAILED${NC} - Do not push until all tests pass!                             ${RED}║${NC}"
    echo -e "${RED}╚═══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    exit 1
else
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║${NC}  ${BOLD}ALL TESTS PASSED${NC} - Ready to push!                                           ${GREEN}║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    exit 0
fi
