#!/bin/bash
set -e

# OMG Docker Test Orchestrator
# Automates building and testing OMG on Debian/Ubuntu environments

usage() {
    echo "Usage: $0 [debian|ubuntu|trixie|all]"
    exit 1
}

DISTRO=${1:-all}

run_test() {
    local target=$1
    local base_image
    case "$target" in
        debian)
            base_image="debian:bookworm"
            ;;
        ubuntu)
            base_image="ubuntu:24.04"
            ;;
        trixie)
            base_image="debian:trixie"
            ;;
        *)
            echo "❌ Unsupported target: $target"
            return 1
            ;;
    esac
    local tag="omg-test-$target"

    echo "================================================================================"
    echo "🚀 TESTING ON $target"
    echo "================================================================================"

    echo "📦 Building Docker image: $tag..."
    docker build --target final --build-arg "BASE_IMAGE=$base_image" -t "$tag" -f Dockerfile.apt .

    echo "🧪 Running Cargo tests (debian feature)..."
    docker run --rm -e OMG_RUN_SYSTEM_TESTS=1 -e OMG_RUN_DESTRUCTIVE_TESTS=1 "$tag" cargo test --test debian_tests --no-default-features --features debian -- --nocapture

    echo "🧪 Running smoke command..."
    docker run --rm "$tag" /bin/bash -lc "omg --version && omg search bash"

    echo "✅ $target tests completed successfully!"
}

if [ "$DISTRO" == "debian" ]; then
    run_test "debian"
elif [ "$DISTRO" == "ubuntu" ]; then
    run_test "ubuntu"
elif [ "$DISTRO" == "trixie" ]; then
    run_test "trixie"
elif [ "$DISTRO" == "all" ]; then
    run_test "debian"
    run_test "ubuntu"
else
    usage
fi

echo ""
echo "✨ All selected distro tests passed! ✨"
