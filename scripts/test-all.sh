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
VALID_SEGMENTS=(lint build core runtimes cli packages security property comprehensive integration)
OUTPUT_FILE=$(mktemp "${TMPDIR:-/tmp}/omg-test-all.XXXXXX")
trap 'rm -f "$OUTPUT_FILE"' EXIT

while [[ $# -gt 0 ]]; do
  case $1 in
  --quick | -q)
    QUICK=true
    shift
    ;;
  --segment | -s)
    if [[ $# -lt 2 ]]; then
      echo "Missing segment name after $1" >&2
      exit 1
    fi
    SEGMENT="$2"
    shift 2
    ;;
  --verbose | -v)
    VERBOSE=true
    shift
    ;;
  --help | -h)
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
    echo "  build      - Compilation check"
    echo "  core       - Core module unit tests"
    echo "  runtimes   - Runtime manager tests"
    echo "  cli        - CLI argument and command tests"
    echo "  packages   - Package manager tests"
    echo "  security   - Security and input validation tests"
    echo "  property   - Property-based tests"
    echo "  comprehensive - CLI command and feature tests"
    echo "  integration - Full integration tests"
    exit 0
    ;;
  *)
    echo "Unknown option: $1"
    exit 1
    ;;
  esac
done

if [[ -n "$SEGMENT" ]]; then
  SEGMENT_IS_VALID=false
  for candidate in "${VALID_SEGMENTS[@]}"; do
    if [[ "$SEGMENT" == "$candidate" ]]; then
      SEGMENT_IS_VALID=true
      break
    fi
  done
  if ! $SEGMENT_IS_VALID; then
    echo "Unknown segment: $SEGMENT" >&2
    echo "Valid segments: ${VALID_SEGMENTS[*]}" >&2
    exit 1
  fi
fi

if $QUICK && [[ "$SEGMENT" == "property" || "$SEGMENT" == "integration" ]]; then
  echo "Segment '$SEGMENT' is excluded by --quick" >&2
  exit 1
fi

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

  local start
  start=$(date +%s%3N)

  local exit_code=0

  if $VERBOSE; then
    eval "$cmd" || exit_code=$?
  else
    eval "$cmd" >"$OUTPUT_FILE" 2>&1 || exit_code=$?
  fi

  local end
  end=$(date +%s%3N)
  local duration=$((end - start))

  if [ $exit_code -eq 0 ]; then
    echo -e "${GREEN}✓ ${name} passed${NC} (${duration}ms)"
    ((PASSED++))
  else
    echo -e "${RED}✗ ${name} failed${NC} (${duration}ms)"
    if ! $VERBOSE; then
      echo ""
      echo -e "${YELLOW}Output:${NC}"
      tail -30 "$OUTPUT_FILE"
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

  run_segment "Format Check" "cargo fmt -- --check"
  run_segment "Clippy (warnings as errors)" "cargo clippy --features arch --locked -- -D warnings"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 2: BUILD
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "build" || [ -z "$SEGMENT" ]; then
  echo ""
  echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
  echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 2: BUILD${NC} - Compilation check                                         ${CYAN}│${NC}"
  echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"

  run_segment "Debug Build" "cargo build --features arch --locked"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 3: CORE UNIT TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "core"; then
  echo ""
  echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
  echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 3: CORE${NC} - Core module unit tests                                     ${CYAN}│${NC}"
  echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"

  run_segment "Database Tests" "cargo test --features arch --locked --lib core::database"
  run_segment "Completion Tests" "cargo test --features arch --locked --lib core::completion"
  run_segment "Container Tests" "cargo test --features arch --locked --lib core::container"
  run_segment "Security Tests" "cargo test --features arch --locked --lib core::security"
  run_segment "System Info Tests" "cargo test --features arch --locked --lib core::sysinfo"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 4: RUNTIME MANAGER TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "runtimes"; then
  echo ""
  echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
  echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 4: RUNTIMES${NC} - Runtime manager unit tests                             ${CYAN}│${NC}"
  echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"

  run_segment "Common Utilities" "cargo test --features arch --locked --lib runtimes::common"
  run_segment "Node.js Manager" "cargo test --features arch --locked --lib runtimes::node"
  run_segment "Python Manager" "cargo test --features arch --locked --lib runtimes::python"
  run_segment "Go Manager" "cargo test --features arch --locked --lib runtimes::go"
  run_segment "Bun Manager" "cargo test --features arch --locked --lib runtimes::bun"
  run_segment "Ruby Manager" "cargo test --features arch --locked --lib runtimes::ruby"
  run_segment "Java Manager" "cargo test --features arch --locked --lib runtimes::java"
  run_segment "Rust Manager" "cargo test --features arch --locked --lib runtimes::rust"
  run_segment "Mise Manager" "cargo test --features arch --locked --lib runtimes::mise"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 5: CLI TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "cli"; then
  echo ""
  echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
  echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 5: CLI${NC} - Command-line interface tests                                ${CYAN}│${NC}"
  echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"

  run_segment "CLI Args Parsing" "cargo test --features arch --locked --lib cli::args"
  run_segment "Hooks Tests" "cargo test --features arch --locked --lib hooks"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 6: PACKAGE MANAGER TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "packages"; then
  echo ""
  echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
  echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 6: PACKAGES${NC} - Package manager tests                                  ${CYAN}│${NC}"
  echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"

  run_segment "Pacman DB Tests" "cargo test --features arch --locked --lib package_managers::pacman_db"
  run_segment "Parallel Sync Tests" "cargo test --features arch --locked --lib package_managers::parallel_sync"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 7: SECURITY TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "security"; then
  echo ""
  echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
  echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 7: SECURITY${NC} - Security and input validation tests                    ${CYAN}│${NC}"
  echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"

  run_segment "Input Validation" "cargo test --features arch --locked --test security_tests input_validation"
  run_segment "Privilege Tests" "cargo test --features arch --locked --test security_tests privilege_tests"
  run_segment "Filesystem Security" "cargo test --features arch --locked --test security_tests filesystem_security"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 8: PROPERTY-BASED TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "property" && [ "$QUICK" = "false" ]; then
  echo ""
  echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
  echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 8: PROPERTY${NC} - Property-based fuzzing tests                           ${CYAN}│${NC}"
  echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"

  run_segment "Property Tests" "cargo test --features arch --locked --test property_tests --test property_tests_v2"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SEGMENT 10: INTEGRATION TESTS
# ═══════════════════════════════════════════════════════════════════════════════

if should_run "integration" && [ "$QUICK" = "false" ]; then
  echo ""
  echo -e "${CYAN}┌─────────────────────────────────────────────────────────────────────────────────┐${NC}"
  echo -e "${CYAN}│${NC}  ${BOLD}SEGMENT 10: INTEGRATION${NC} - Full integration tests                             ${CYAN}│${NC}"
  echo -e "${CYAN}└─────────────────────────────────────────────────────────────────────────────────┘${NC}"

  run_segment "Arch Integration" "cargo test --features arch --locked --test arch_tests"
  run_segment "Integration Suite" "cargo test --features arch --locked --test integration_suite"
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

if [ "$PASSED" -eq 0 ] && [ "$FAILED" -eq 0 ]; then
  echo -e "${RED}No test segments executed.${NC}" >&2
  exit 1
fi

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
