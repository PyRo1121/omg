#!/usr/bin/env bash
# Install git hooks for OMG development

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
HOOKS_DIR="$REPO_ROOT/.git/hooks"

echo "Installing git hooks..."

# Create pre-push hook
cat > "$HOOKS_DIR/pre-push" << 'EOF'
#!/usr/bin/env bash
# OMG Pre-Push Hook - Runs quick tests before pushing

echo "Running pre-push tests..."

# Run quick tests (skip integration tests for speed)
if ! ./scripts/test-all.sh --quick; then
    echo ""
    echo "Pre-push tests failed! Push aborted."
    echo "Run './scripts/test-all.sh --verbose' to see details."
    exit 1
fi

echo "Pre-push tests passed!"
exit 0
EOF

chmod +x "$HOOKS_DIR/pre-push"

# Create pre-commit hook (optional - just format check)
cat > "$HOOKS_DIR/pre-commit" << 'EOF'
#!/usr/bin/env bash
# OMG Pre-Commit Hook - Quick format check

# Only check staged Rust files
STAGED_RS=$(git diff --cached --name-only --diff-filter=ACM | grep '\.rs$' || true)

if [ -n "$STAGED_RS" ]; then
    echo "Checking format of staged Rust files..."
    if ! rustup run nightly cargo fmt -- --check 2>/dev/null; then
        echo ""
        echo "Format check failed! Run 'cargo fmt' to fix."
        exit 1
    fi
fi

exit 0
EOF

chmod +x "$HOOKS_DIR/pre-commit"

echo "✓ Git hooks installed successfully!"
echo ""
echo "Installed hooks:"
echo "  - pre-commit: Format check on staged .rs files"
echo "  - pre-push: Quick test suite before push"
echo ""
echo "To skip hooks temporarily, use: git push --no-verify"
