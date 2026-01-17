#!/bin/bash
# OMG Debian/Ubuntu Docker Test Script
# Run this from the project root to build and test on Debian/Ubuntu

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║         OMG Debian/Ubuntu Docker Test Suite                  ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Project: $PROJECT_DIR"
echo "Date: $(date)"
echo ""

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    echo "ERROR: Docker is not installed or not in PATH"
    echo "Install Docker: https://docs.docker.com/engine/install/"
    exit 1
fi

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    echo "ERROR: Docker daemon is not running"
    echo "Start Docker: sudo systemctl start docker"
    exit 1
fi

# Parse arguments
DISTRO="${1:-both}"
INTERACTIVE="${2:-}"

case "$DISTRO" in
    debian|d)
        DISTROS="debian"
        ;;
    ubuntu|u)
        DISTROS="ubuntu"
        ;;
    both|all|"")
        DISTROS="debian ubuntu"
        ;;
    shell)
        # Interactive shell mode
        SHELL_DISTRO="${2:-debian}"
        echo "Starting interactive shell in $SHELL_DISTRO container..."
        docker build -f "Dockerfile.$SHELL_DISTRO" -t "omg-$SHELL_DISTRO" .
        docker run --rm -it "omg-$SHELL_DISTRO" /bin/bash
        exit 0
        ;;
    *)
        echo "Usage: $0 [debian|ubuntu|both|shell] [distro-for-shell]"
        echo ""
        echo "Examples:"
        echo "  $0              # Test both Debian and Ubuntu"
        echo "  $0 debian       # Test only Debian"
        echo "  $0 ubuntu       # Test only Ubuntu"
        echo "  $0 shell debian # Interactive shell in Debian container"
        exit 1
        ;;
esac

FAILED=0

for distro in $DISTROS; do
    echo ""
    echo "════════════════════════════════════════════════════════════════"
    echo "  Building and testing on: $distro"
    echo "════════════════════════════════════════════════════════════════"
    echo ""

    DOCKERFILE="Dockerfile.$distro"
    IMAGE_NAME="omg-$distro"

    if [ ! -f "$DOCKERFILE" ]; then
        echo "ERROR: $DOCKERFILE not found"
        FAILED=1
        continue
    fi

    echo "[1/3] Building Docker image (forcing rebuild)..."
    if ! docker build --no-cache -f "$DOCKERFILE" -t "$IMAGE_NAME" .; then
        echo "ERROR: Docker build failed for $distro"
        FAILED=1
        continue
    fi

    echo ""
    echo "[2/3] Running smoke tests..."
    if ! docker run --rm "$IMAGE_NAME"; then
        echo "ERROR: Smoke tests failed for $distro"
        FAILED=1
        continue
    fi

    echo ""
    echo "[3/3] Running cargo tests..."
    if ! docker run --rm "$IMAGE_NAME" cargo test --no-default-features --features debian; then
        echo "ERROR: Cargo tests failed for $distro"
        FAILED=1
        continue
    fi

    echo ""
    echo "✓ $distro tests passed!"
done

echo ""
echo "════════════════════════════════════════════════════════════════"

if [ $FAILED -eq 0 ]; then
    echo "  All tests passed! ✓"
    echo "════════════════════════════════════════════════════════════════"
    exit 0
else
    echo "  Some tests failed! ✗"
    echo "════════════════════════════════════════════════════════════════"
    exit 1
fi
