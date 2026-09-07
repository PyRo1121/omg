#!/bin/bash
set -e

# ============================================================================
# OMG Performance Benchmark with Hyperfine (Industry Best Practice)
# ============================================================================
#
# This benchmark uses hyperfine (https://github.com/sharkdp/hyperfine),
# the industry-standard CLI benchmarking tool used by ripgrep, fd, and bat.
#
# ADVANTAGES OVER CUSTOM BASH TIMING:
# - Statistical rigor with outlier detection (Modified Z-score method)
# - Automatic run count determination for confidence intervals
# - Warm/cold cache benchmarking support
# - JSON export for CI regression detection
# - 40-60% faster execution than manual bash loops
#
# REQUIREMENTS:
#   omg install hyperfine       # Arch Linux
#   brew install hyperfine      # macOS
#
# USAGE:
#   ./benchmark-hyperfine.sh              # Full benchmark
#   ./benchmark-hyperfine.sh --fast       # Quick benchmark
#   ./benchmark-hyperfine.sh --update     # AUR update discovery benchmark only
#   ./benchmark-hyperfine.sh --help       # Show options
#
# ============================================================================

export PATH="$HOME/.cargo/bin:$PATH"

WARMUP=3
MIN_RUNS=20
MAX_RUNS=50
FAST_MODE=false
UPDATE_MODE=false
EXPORT_DIR="benchmark_results"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UPDATE_WORK_DIRS=()
DAEMON_PID=""
BENCH_DIR=""

print_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Options:
  --fast, -f    Run in fast mode (reduced warmup and runs)
  --update      Run ONLY the AUR update discovery benchmark (no daemon,
                no other benchmarks) and exit
  --help, -h    Show this help message

Environment Variables:
  OMG_BENCH_WARMUP        Number of warmup runs (default: 3, fast: 1)
  OMG_BENCH_RUNS          Minimum runs (default: 20, fast: 5)
  OMG_BENCH_MAX_RUNS      Maximum runs (default: 50, fast: 15)
  OMG_BENCH_EXPORT_DIR    Scratch JSON/MD directory (default: benchmark_results)
  OMG_BENCH_SKIP_RECORD   Set to 1 to skip writing benchmarks/records/
  OMG_BENCH_SKIP_UPDATE   Set to 1 to skip update discovery in a full run
  OMG_BENCH_BINARY        Path to a prebuilt omg binary (skips cargo build).
                          omgd is taken from the same directory.
  OMG_BENCH_TARGET_DIR    Cargo target directory (default:
                          ~/.cache/build-targets/omg-benchmark-hyperfine)
  OMG_BENCH_SOURCE_CACHE  Cache dir to copy AUR/package-DB fixtures from
                          (default: OMG_CACHE_DIR, else ~/.cache/omg)

Examples:
  $0                           # Full benchmark (3 warmup, 10+ runs)
  $0 --fast                    # Fast benchmark (1 warmup, 5+ runs)
  $0 --update                  # Update discovery benchmark only
  OMG_BENCH_RUNS=20 $0         # Custom run count
  OMG_BENCH_BINARY=./omg $0 --fast --update   # Benchmark a prebuilt binary
EOF
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --fast|-f)
            FAST_MODE=true
            shift
            ;;
        --update)
            UPDATE_MODE=true
            shift
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

if [ "$FAST_MODE" = true ]; then
    WARMUP=${OMG_BENCH_WARMUP:-1}
    MIN_RUNS=${OMG_BENCH_RUNS:-5}
    MAX_RUNS=${OMG_BENCH_MAX_RUNS:-15}
else
    WARMUP=${OMG_BENCH_WARMUP:-3}
    MIN_RUNS=${OMG_BENCH_RUNS:-20}
    MAX_RUNS=${OMG_BENCH_MAX_RUNS:-50}
fi

EXPORT_DIR=${OMG_BENCH_EXPORT_DIR:-benchmark_results}
mkdir -p "$EXPORT_DIR"
BENCH_SOURCE_CACHE="${OMG_BENCH_SOURCE_CACHE:-${OMG_CACHE_DIR:-$HOME/.cache/omg}}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

if ! command -v hyperfine &>/dev/null; then
    echo -e "${RED}❌ hyperfine not installed${NC}"
    echo ""
    echo "Install with:"
    echo "  Arch Linux:  omg install hyperfine"
    echo "  macOS:       brew install hyperfine"
    echo "  Cargo:       cargo install hyperfine"
    echo ""
    echo "Falling back to standard benchmark.sh..."
    if [ "$FAST_MODE" = true ]; then
        exec ./benchmark.sh --fast
    else
        exec ./benchmark.sh
    fi
fi

cleanup() {
    if [ -n "${DAEMON_PID:-}" ]; then
        kill "$DAEMON_PID" >/dev/null 2>&1 || true
        wait "$DAEMON_PID" 2>/dev/null || true
        DAEMON_PID=""
    fi
    if [ -n "${BENCH_DIR:-}" ] && [ -d "$BENCH_DIR" ]; then
        rm -rf -- "$BENCH_DIR"
        BENCH_DIR=""
    fi
    local dir
    for dir in "${UPDATE_WORK_DIRS[@]}"; do
        rm -rf -- "$dir"
    done
}
trap cleanup EXIT

run_hyperfine() {
    local json="$1"
    local md="$2"
    shift 2
    hyperfine --shell=none --output=pipe --input=null \
        --warmup "$WARMUP" --min-runs "$MIN_RUNS" --max-runs "$MAX_RUNS" \
        --export-json "$json" --export-markdown "$md" \
        "$@"
}

preflight_match() {
    local label="$1"
    local needle="$2"
    local outfile="$3"
    shift 3
    if ! "$@" >"$outfile" 2>&1; then
        echo -e "${RED}❌ Preflight failed: ${label} (non-zero exit)${NC}" >&2
        cat "$outfile" >&2
        exit 1
    fi
    if ! grep -qiE "$needle" "$outfile"; then
        echo -e "${RED}❌ Preflight failed: ${label} (no match for /${needle}/)${NC}" >&2
        echo "----- output -----" >&2
        head -c 4000 "$outfile" >&2
        echo "" >&2
        exit 1
    fi
    local bytes lines
    bytes=$(wc -c < "$outfile" | tr -d ' ')
    lines=$(wc -l < "$outfile" | tr -d ' ')
    echo -e "  ${GREEN}ok${NC} ${label}  ${bytes} bytes, ${lines} lines"
}

archive_results() {
    if [ "${OMG_BENCH_SKIP_RECORD:-}" = 1 ]; then
        echo "Skipping benchmarks/records/ (OMG_BENCH_SKIP_RECORD=1)"
        return 0
    fi
    python3 "$REPO_ROOT/scripts/record-benchmark-run.py" \
        --source "$EXPORT_DIR" \
        --warmup "$WARMUP" \
        --min-runs "$MIN_RUNS" \
        --max-runs "$MAX_RUNS"
}

TARGET_DIR="${OMG_BENCH_TARGET_DIR:-$HOME/.cache/build-targets/omg-benchmark-hyperfine}"

if [ -n "${OMG_BENCH_BINARY:-}" ]; then
    echo -e "${BLUE}🔧 Using prebuilt binary: $OMG_BENCH_BINARY${NC}"
    if [ ! -x "$OMG_BENCH_BINARY" ]; then
        echo -e "${RED}❌ OMG_BENCH_BINARY is set but not executable: $OMG_BENCH_BINARY${NC}" >&2
        exit 1
    fi
    OMG="$OMG_BENCH_BINARY"
    bin_dir="$(cd "$(dirname "$OMG")" && pwd)"
    OMGD="$bin_dir/omgd"
else
    echo -e "${BLUE}🔨 Building release binaries...${NC}"
    CARGO_INCREMENTAL=0 CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release --locked --features arch --quiet

    OMG="$TARGET_DIR/release/omg"
    OMGD="$TARGET_DIR/release/omgd"
fi

if [ ! -x "$OMG" ]; then
    echo -e "${RED}❌ omg binary not executable: $OMG${NC}" >&2
    exit 1
fi
if [ ! -x "$OMGD" ]; then
    echo -e "${RED}❌ omgd binary not found next to omg: $OMGD${NC}" >&2
    exit 1
fi

# ----------------------------------------------------------------------------
# AUR update discovery benchmark (--update)
#
# Benchmarks `omg update` update discovery against an isolated cache in two
# otherwise-identical variants:
#   (a) ready:    coherent, fresh AUR archive + binary index
#   (b) missing:  same archive but NO index (removed before every run via
#                 hyperfine --prepare), forcing the AUR RPC fallback path
#
# Deterministic regression guard: after the benchmark, variant (b) must NOT
# have created an index — update discovery must never synchronously rebuild
# global metadata. A statistical guard also fails only on a broad regression
# (missing-index mean > 3s AND > 5x ready-index mean), so small network
# variance does not make the benchmark flaky.
# ============================================================================
run_update_benchmark() {
    if [ "$EUID" -eq 0 ]; then
        echo -e "${RED}❌ Run the update benchmark as a regular user${NC}" >&2
        return 1
    fi

    local source_cache="$BENCH_SOURCE_CACHE"
    local fixture_archive="$source_cache/aur/_meta/packages-meta-ext-v1.json.gz"
    local fixture_index="$source_cache/aur/_meta/packages-meta-ext-v1.rkyv"
    local fixture_local_db="$source_cache/local_db_rdeps.bin"
    if [ ! -f "$fixture_local_db" ] && [ -f "$source_cache/local_db.bin" ]; then
        fixture_local_db="$source_cache/local_db.bin"
    fi
    local fixture_sync_db="$source_cache/sync_db.bin"

    local missing_fixture=0
    local fixture
    for fixture in "$fixture_local_db" "$fixture_sync_db" "$fixture_archive" "$fixture_index"; do
        if [ ! -f "$fixture" ]; then
            echo -e "${RED}❌ Missing benchmark fixture: $fixture${NC}" >&2
            missing_fixture=1
        fi
    done
    if [ "$missing_fixture" -ne 0 ]; then
        echo "" >&2
        echo "Update discovery benchmarks need a populated omg cache with:" >&2
        echo "  local_db_rdeps.bin (or legacy local_db.bin), sync_db.bin," >&2
        echo "  aur/_meta/packages-meta-ext-v1.rkyv" >&2
        echo "Populate the OMG metadata cache first, or set OMG_BENCH_SOURCE_CACHE." >&2
        return 1
    fi

    # Isolated scratch space under $HOME/.cache/build-targets (never /tmp),
    # removed on exit via trap.
    mkdir -p "$HOME/.cache/build-targets"
    local work_dir
    work_dir="$(mktemp -d "$HOME/.cache/build-targets/omg-update-bench-XXXXXX")"
    UPDATE_WORK_DIRS+=("$work_dir")

    local ready_cache="$work_dir/ready-cache"
    local missing_cache="$work_dir/missing-index-cache"
    local config_dir="$work_dir/config"
    mkdir -p "$ready_cache/aur/_meta" "$missing_cache/aur/_meta" "$config_dir"

    # (a) Ready cache: copy the archive BEFORE the index so the published
    # generation is coherent (index mtime >= archive mtime) and fresh
    # (archive mtime within the metadata TTL).
    cp "$fixture_archive" "$ready_cache/aur/_meta/"
    cp "$fixture_index" "$ready_cache/aur/_meta/"
    cp "$fixture_local_db" "$fixture_sync_db" "$ready_cache/"

    # (b) Same fixtures but no index; hyperfine --prepare re-removes it
    # before every run.
    cp "$fixture_archive" "$missing_cache/aur/_meta/"
    cp "$fixture_local_db" "$fixture_sync_db" "$missing_cache/"

    # Keep the benchmark hermetic: never talk to a live daemon.
    export OMG_SOCKET_PATH="$work_dir/unused-omg.sock"
    export OMG_DAEMON_DATA_DIR="$work_dir/data"

    # Wrapper around the REAL 'omg update' (no --yes). Unprivileged and
    # non-interactive, it completes update discovery, then either reports
    # "up to date" (exit 0) or bails with "Use --yes for non-interactive
    # updates" (exit 1). The wrapper accepts only those outcomes and only
    # when the output proves discovery finished; network or parse errors
    # propagate as failures so hyperfine fails the benchmark.
    local wrapper="$work_dir/omg-update-wrapper.sh"
    cat > "$wrapper" << 'WRAPPER'
#!/bin/bash
set -e
omg_bin="$1"
log="$(mktemp "${TMPDIR:?}/omg-update-log-XXXXXX")"
rc=0
"$omg_bin" update >"$log" 2>&1 || rc=$?
# "update available" (singular) covers the 1-update summary line.
if grep -q -e "updates available" -e "update available" -e "up to date" "$log" \
    && ! grep -q -e "AUR update check failed" -e "Failed to check AUR updates" "$log" \
    && { [ "$rc" -eq 0 ] || [ "$rc" -eq 1 ]; }; then
    rm -f "$log"
    exit 0
fi
echo "omg update discovery failed (exit $rc):" >&2
cat "$log" >&2
rm -f "$log"
exit 1
WRAPPER
    chmod +x "$wrapper"

    local missing_index="$missing_cache/aur/_meta/packages-meta-ext-v1.rkyv"

    echo "========================================================"
    echo -e "${GREEN}🚀 OMG Update Discovery Benchmark${NC}"
    echo "========================================================"
    echo ""
    echo -e "${YELLOW}CONFIGURATION:${NC}"
    echo "  Binary: $OMG"
    echo "  Warmup runs: $WARMUP"
    echo "  Minimum runs: $MIN_RUNS"
    echo "  Ready cache: $ready_cache"
    echo "  Missing-index cache: $missing_cache"
    echo "  Results: $EXPORT_DIR/update.md, $EXPORT_DIR/update.json"
    echo ""

    # NOTE: hyperfine --prepare applies to every run of every command; the
    # rm targets only the missing-index cache, so it is a no-op for the
    # ready-index command.
    hyperfine --warmup "$WARMUP" --min-runs "$MIN_RUNS" \
        --prepare "rm -f '$missing_index'" \
        --command-name "update discovery (ready index)" \
            "TMPDIR='$work_dir' OMG_CACHE_DIR='$ready_cache' OMG_CONFIG_DIR='$config_dir' '$wrapper' '$OMG'" \
        --command-name "update discovery (missing index)" \
            "TMPDIR='$work_dir' OMG_CACHE_DIR='$missing_cache' OMG_CONFIG_DIR='$config_dir' '$wrapper' '$OMG'" \
        --export-markdown "$EXPORT_DIR/update.md" \
        --export-json "$EXPORT_DIR/update.json"

    # Deterministic regression guard: the missing-index scenario must not
    # have rebuilt global metadata (i.e. created an AUR index).
    if [ -f "$missing_index" ]; then
        echo -e "${RED}❌ REGRESSION: missing-index update discovery created an AUR index at:${NC}" >&2
        echo "  $missing_index" >&2
        echo "Update discovery must not synchronously rebuild global metadata." >&2
        return 1
    fi
    echo -e "${GREEN}✅ Guard passed: missing-index run did not rebuild the AUR index${NC}"

    # Statistical guard: fail only on a broad regression so ordinary network
    # variance between the two scenarios stays non-fatal.
    python3 - "$EXPORT_DIR/update.json" << 'PYEOF'
import json
import sys

with open(sys.argv[1]) as handle:
    data = json.load(handle)

results = data.get("results", [])
ready = next((r for r in results if "ready index" in r["command"]), None)
missing = next((r for r in results if "missing index" in r["command"]), None)
if ready is None or missing is None:
    print("❌ update.json is missing the expected benchmark scenarios", file=sys.stderr)
    sys.exit(1)

ready_mean = ready["mean"]
missing_mean = missing["mean"]
limit = max(3.0, 5.0 * ready_mean)
print(f"Ready-index mean:   {ready_mean:.3f} s")
print(f"Missing-index mean: {missing_mean:.3f} s")
print(f"Regression limit:   {limit:.3f} s  (max(3 s, 5 x ready-index mean))")

if missing_mean > 3.0 and missing_mean > 5.0 * ready_mean:
    print(
        "❌ REGRESSION: missing-index update discovery mean exceeds both 3 s "
        "and 5x the ready-index mean",
        file=sys.stderr,
    )
    sys.exit(1)
print("✅ No broad update-discovery regression")
PYEOF

    echo ""
    echo -e "${GREEN}✅ Update discovery benchmark complete${NC}"
    echo "Results saved to: $EXPORT_DIR/update.md, $EXPORT_DIR/update.json"
}

if [ "$UPDATE_MODE" = true ]; then
    run_update_benchmark
    archive_results
    exit 0
fi

echo "========================================================"
echo -e "${GREEN}🚀 OMG Hyperfine Performance Benchmark${NC}"
echo "========================================================"
echo ""
if [ "$FAST_MODE" = true ]; then
    echo -e "${YELLOW}⚡ FAST MODE ENABLED${NC}"
fi
echo -e "${YELLOW}CONFIGURATION:${NC}"
echo "  Warmup runs: $WARMUP"
echo "  Minimum runs: $MIN_RUNS"
echo "  Maximum runs: $MAX_RUNS"
echo "  Scratch directory: $EXPORT_DIR/"
echo "  Records: benchmarks/records/"
echo ""
echo -e "${YELLOW}METHODOLOGY:${NC}"
echo "  • hyperfine --shell=none (no shell startup in the timed path)"
echo "  • --output=pipe so tools cannot skip work when stdout is /dev/null"
echo "  • Preflight: each command must print real results before timing starts"
echo "  • Warm cache (typical interactive use, not first-boot)"
echo ""

mkdir -p "$HOME/.cache/build-targets"
BENCH_DIR="$(mktemp -d "$HOME/.cache/build-targets/omg-bench-XXXXXX")"
export OMG_DAEMON_DATA_DIR="$BENCH_DIR/data"
export OMG_SOCKET_PATH="$BENCH_DIR/omg.sock"
export OMG_CACHE_DIR="$BENCH_DIR/cache"
DAEMON_LOG="$BENCH_DIR/omgd.log"
mkdir -p "$OMG_DAEMON_DATA_DIR" "$OMG_CACHE_DIR"

source_cache="${OMG_BENCH_SOURCE_CACHE:-${HOME}/.cache/omg}"
if [ -f "$source_cache/sync_db.bin" ]; then
    cp "$source_cache/sync_db.bin" "$OMG_CACHE_DIR/"
    if [ -f "$source_cache/local_db_rdeps.bin" ]; then
        cp "$source_cache/local_db_rdeps.bin" "$OMG_CACHE_DIR/"
    elif [ -f "$source_cache/local_db.bin" ]; then
        cp "$source_cache/local_db.bin" "$OMG_CACHE_DIR/"
    fi
    echo -e "${BLUE}📦 Seeded daemon cache from $source_cache${NC}"
fi

echo "Starting OMG Daemon..."
$OMGD > "$DAEMON_LOG" 2>&1 &
DAEMON_PID=$!

echo -n "Waiting for daemon to be ready..."
daemon_ready=0
for _ in $(seq 1 80); do
    if $OMG status > /dev/null 2>&1; then
        daemon_ready=1
        echo " status ok"
        break
    fi
    sleep 0.1
done
if [ "$daemon_ready" -ne 1 ]; then
    echo -e "${RED}❌ OMG daemon failed to start${NC}" >&2
    tail -n 50 "$DAEMON_LOG" >&2 || true
    exit 1
fi

echo -n "Waiting for search index (firefox)..."
search_ready=0
for _ in $(seq 1 100); do
    if $OMG search firefox --no-aur 2>/dev/null | grep -qi firefox; then
        search_ready=1
        echo " ready"
        break
    fi
    sleep 0.1
done
if [ "$search_ready" -ne 1 ]; then
    echo -e "${RED}❌ Daemon never returned a firefox search hit${NC}" >&2
    tail -n 50 "$DAEMON_LOG" >&2 || true
    exit 1
fi

echo -e "\n${BLUE}🔎 Preflight (prove the timed commands do real work)${NC}"
PRE_DIR="$BENCH_DIR/preflight"
mkdir -p "$PRE_DIR"
preflight_match "omg search firefox --no-aur" "firefox" "$PRE_DIR/search.txt" \
    "$OMG" search firefox --no-aur
preflight_match "omg info firefox" "firefox" "$PRE_DIR/info.txt" \
    "$OMG" info firefox
preflight_match "omg status" "." "$PRE_DIR/status.txt" "$OMG" status
if ! "$OMG" ec >"$PRE_DIR/explicit.txt" 2>&1; then
    echo -e "${RED}❌ Preflight failed: omg ec${NC}" >&2
    cat "$PRE_DIR/explicit.txt" >&2
    exit 1
fi
EXPLICIT_COUNT="$(tr -d '[:space:]' < "$PRE_DIR/explicit.txt")"
if ! [[ "$EXPLICIT_COUNT" =~ ^[0-9]+$ ]] || [ "$EXPLICIT_COUNT" -le 0 ]; then
    echo -e "${RED}❌ Preflight failed: explicit count was '${EXPLICIT_COUNT}'${NC}" >&2
    exit 1
fi
echo -e "  ${GREEN}ok${NC} omg ec = ${EXPLICIT_COUNT}"
if command -v pacman >/dev/null 2>&1; then
    preflight_match "pacman -Ss firefox" "firefox" "$PRE_DIR/pacman-search.txt" \
        pacman -Ss firefox
    preflight_match "pacman -Si firefox" "firefox" "$PRE_DIR/pacman-info.txt" \
        pacman -Si firefox
fi
if command -v yay >/dev/null 2>&1; then
    preflight_match "yay -Ss --repo firefox" "firefox" "$PRE_DIR/yay-search.txt" \
        yay -Ss --repo firefox
fi

python3 - "$EXPORT_DIR/preflight.json" "$PRE_DIR" "$EXPLICIT_COUNT" << 'PY'
import json, sys
from pathlib import Path
out, pre_dir, count = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
evidence = {"explicit_count": int(count)}
for path in sorted(pre_dir.glob("*.txt")):
    text = path.read_text(errors="replace")
    evidence[path.stem] = {"bytes": len(text.encode()), "lines": text.count("\n")}
out.write_text(json.dumps(evidence, indent=2) + "\n")
PY

cat > "$BENCH_DIR/pacman-explicit-count" << 'EOF'
#!/bin/bash
set -euo pipefail
pacman -Qe | wc -l
EOF
cat > "$BENCH_DIR/yay-explicit-count" << 'EOF'
#!/bin/bash
set -euo pipefail
yay -Qe | wc -l
EOF
chmod +x "$BENCH_DIR/pacman-explicit-count" "$BENCH_DIR/yay-explicit-count"

echo -e "\n${BLUE}📦 Benchmark: SEARCH (firefox)${NC}"
echo "-------------------------------"
search_cmds=(--command-name "OMG" "$OMG search firefox --no-aur")
if command -v pacman >/dev/null 2>&1; then
    search_cmds+=(--command-name "pacman" "pacman -Ss firefox")
fi
if command -v yay >/dev/null 2>&1; then
    search_cmds+=(--command-name "yay (--repo)" "yay -Ss --repo firefox")
fi
run_hyperfine "$EXPORT_DIR/search.json" "$EXPORT_DIR/search.md" "${search_cmds[@]}"

echo -e "\n${BLUE}ℹ️  Benchmark: INFO (firefox)${NC}"
echo "-------------------------------"
info_cmds=(--command-name "OMG" "$OMG info firefox")
if command -v pacman >/dev/null 2>&1; then
    info_cmds+=(--command-name "pacman" "pacman -Si firefox")
fi
if command -v yay >/dev/null 2>&1; then
    info_cmds+=(--command-name "yay (--repo)" "yay -Si --repo firefox")
fi
run_hyperfine "$EXPORT_DIR/info.json" "$EXPORT_DIR/info.md" "${info_cmds[@]}"

echo -e "\n${BLUE}⚡ Benchmark: STATUS${NC}"
echo "-------------------------------"
status_cmds=(--command-name "OMG" "$OMG status")
run_hyperfine "$EXPORT_DIR/status.json" "$EXPORT_DIR/status.md" "${status_cmds[@]}"

echo -e "\n${BLUE}📋 Benchmark: EXPLICIT COUNT${NC}"
echo "-------------------------------"
explicit_cmds=(--command-name "OMG" "$OMG ec")
if command -v pacman >/dev/null 2>&1; then
    explicit_cmds+=(--command-name "pacman" "$BENCH_DIR/pacman-explicit-count")
fi
if command -v yay >/dev/null 2>&1; then
    explicit_cmds+=(--command-name "yay" "$BENCH_DIR/yay-explicit-count")
fi
run_hyperfine "$EXPORT_DIR/explicit.json" "$EXPORT_DIR/explicit.md" "${explicit_cmds[@]}"

if [ "$FAST_MODE" != true ] && [ "${OMG_BENCH_SKIP_UPDATE:-}" != 1 ]; then
    echo -e "\n${BLUE}🔄 Benchmark: UPDATE DISCOVERY${NC}"
    echo "-------------------------------"
    if ! run_update_benchmark; then
        echo -e "${YELLOW}Update discovery benchmark skipped or failed; query results still stand.${NC}"
    fi
fi

echo ""
echo "========================================================"
echo -e "${GREEN}✅ Benchmarks Complete!${NC}"
echo "========================================================"
echo ""
echo "Scratch JSON/MD: $EXPORT_DIR/"
echo ""
echo -e "${YELLOW}Summary:${NC}"
echo ""

for md_file in "$EXPORT_DIR"/*.md; do
    if [ -f "$md_file" ]; then
        echo "$(basename "$md_file" .md):"
        cat "$md_file"
        echo ""
    fi
done

archive_results
