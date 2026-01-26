#!/bin/bash
# Build script for OMG Deploy TUI
set -e

cd "$(dirname "$0")"

# Clean up broken file in project root if it exists
if [ -f "../deploy-tui.go" ]; then
    echo "==> Removing broken deploy-tui.go from project root..."
    rm -f "../deploy-tui.go"
fi

echo "==> Downloading Go dependencies..."
go mod tidy

echo "==> Building deploy-tui..."
go build -o deploy-tui .

echo "==> Build complete!"
echo ""
echo "Run with: ./deploy-tui"
