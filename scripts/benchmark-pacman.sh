#!/usr/bin/env bash
# Run pacman comparison benchmarks and generate report

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

echo "🔨 Building OMG in release mode..."
cargo build --release --features arch

export OMG_BINARY="$PROJECT_ROOT/target/release/omg"

echo "📊 Running benchmarks..."
cargo bench --features arch --bench pacman_comparison

echo ""
echo "📈 Benchmark results saved to target/criterion/"
echo "To view detailed results:"
echo "  firefox target/criterion/report/index.html"
echo ""

# Generate summary report
if command -v jq &> /dev/null; then
    echo "=== Performance Summary ==="
    echo ""

    for dir in target/criterion/*/; do
        if [ -f "$dir/base/estimates.json" ]; then
            name=$(basename "$dir")
            mean=$(jq -r '.mean.point_estimate' "$dir/base/estimates.json")
            unit=$(jq -r '.mean.unit' "$dir/base/estimates.json")

            # Convert to milliseconds if needed
            if [ "$unit" = "ns" ]; then
                mean=$(echo "scale=2; $mean / 1000000" | bc)
                unit="ms"
            fi

            printf "%-30s: %.2f %s\n" "$name" "$mean" "$unit"
        fi
    done
fi

echo ""
echo "✅ Benchmarking complete!"
