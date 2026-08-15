#!/usr/bin/env bash
# Helper script to run Docker E2E tests locally

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

echo "🔨 Building OMG binary..."
cargo build --release --features arch --locked

echo "🐳 Running Docker E2E tests..."
OMG_RUN_DOCKER_TESTS=1 cargo test --features arch --test docker_e2e --locked -- --ignored --test-threads=1

echo "✅ Docker E2E tests completed successfully!"
