#!/bin/bash
set -e

# OMG Docker Test Orchestrator
# Automates building and testing OMG on Debian/Ubuntu environments

usage() {
    echo "Usage: $0 [debian|ubuntu|all]"
    exit 1
}

DISTRO=${1:-all}

run_test() {
    local target=$1
    local dockerfile="Dockerfile.$target"
    local tag="omg-test-$target"

    echo "================================================================================"
    echo "🚀 TESTING ON $target"
    echo "================================================================================"

    if [ ! -f "$dockerfile" ]; then
        echo "❌ Error: $dockerfile not found"
        return 1
    fi

    echo "📦 Building Docker image: $tag..."
    docker build -t "$tag" -f "$dockerfile" .

    echo "🧪 Running Cargo tests (debian feature)..."
    docker run --rm -e OMG_RUN_SYSTEM_TESTS=1 -e OMG_RUN_DESTRUCTIVE_TESTS=1 "$tag" cargo test --test debian_tests --no-default-features --features debian -- --nocapture

    echo "🧪 Running Smoke Tests / Benchmarks..."
    docker run --rm "$tag" ./scripts/debian-smoke-test.sh

    echo "✅ $target tests completed successfully!"
}

if [ "$DISTRO" == "debian" ]; then
    run_test "debian"
elif [ "$DISTRO" == "ubuntu" ]; then
    run_test "ubuntu"
elif [ "$DISTRO" == "all" ]; then
    run_test "debian"
    run_test "ubuntu"
else
    usage
fi

echo ""
echo "✨ All selected distro tests passed! ✨"
