#!/bin/bash
# Update Changelog Script
# Regenerates changelog from git history and commits it if changed
# Run manually: ./scripts/update-changelog.sh
# Run automatically: via git hook or CI/CD

set -e

export PATH="$HOME/.cargo/bin:$PATH"

# Get repo root
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}📝 Regenerating changelog from git history...${NC}"

# Check if git-cliff is installed
if ! command -v git-cliff &> /dev/null; then
    echo -e "${YELLOW}⚠ git-cliff not installed${NC}"
    echo "Install with: cargo install git-cliff"
    exit 1
fi

# Generate changelog
git-cliff --output docs/changelog.md

# Escape HTML-like tags for MDX compatibility
# Replace <Tag> with `<Tag>` to prevent MDX parsing errors
sed -i 's/<\([A-Z][a-zA-Z]*\)>/`<\1>`/g' docs/changelog.md

# Add Docusaurus frontmatter if not present
if ! grep -q "^---$" docs/changelog.md; then
    {
        echo "---"
        echo "title: Changelog"
        echo "sidebar_position: 99"
        echo "description: Complete version history and release notes"
        echo "---"
        echo ""
        cat docs/changelog.md
    } > docs/changelog.md.tmp
    mv docs/changelog.md.tmp docs/changelog.md
fi

# Copy to docs-site
cp docs/changelog.md docs-site/docs/changelog.md

# Check if changelog changed
if git diff --quiet docs/changelog.md docs-site/docs/changelog.md; then
    echo -e "${GREEN}✓ Changelog is up to date${NC}"
    exit 0
fi

echo -e "${GREEN}✓ Changelog updated${NC}"
echo ""
echo "Changelog has been regenerated. Changes:"
git diff --stat docs/changelog.md docs-site/docs/changelog.md

# Ask if user wants to commit
if [[ -t 0 ]]; then
    read -p "Commit changelog updates? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git add docs/changelog.md docs-site/docs/changelog.md
        git commit -m "docs: update changelog

Auto-generated from git history with git-cliff.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
        echo -e "${GREEN}✓ Changelog committed${NC}"
    fi
else
    # Non-interactive mode (e.g., in CI/CD)
    git add docs/changelog.md docs-site/docs/changelog.md
    echo -e "${GREEN}✓ Changelog staged (run 'git commit' to finalize)${NC}"
fi
