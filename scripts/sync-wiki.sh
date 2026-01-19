#!/bin/bash
set -e

# Sync markdown files from docs/ to the GitHub Wiki
# This script is intended to be run by GitHub Actions

REPO_NAME=$(basename "$(pwd)")
WIKI_DIR="wiki_repo"

# Clone the wiki
git clone "https://x-access-token:${GITHUB_TOKEN}@github.com/${GITHUB_REPOSITORY}.wiki.git" "$WIKI_DIR"

# Clear old content (except .git)
cd "$WIKI_DIR"
find . -mindepth 1 -maxdepth 1 ! -name '.git' -exec rm -rf {} +
cd ..

# Copy docs to wiki
for file in docs/*.md; do
    filename=$(basename "$file" .md)
    if [[ "$filename" == "index" ]]; then
        cp "$file" "$WIKI_DIR/Home.md"
    else
        cp "$file" "$WIKI_DIR/${filename}.md"
    fi
done

# Create _Sidebar.md
cat <<EOF > "$WIKI_DIR/_Sidebar.md"
# Documentation

**Getting Started**
- [[Home]]
- [[quickstart|Quick-Start]]
- [[cli|CLI-Reference]]
- [[configuration|Configuration]]

**Features**
- [[packages|Package-Management]]
- [[runtimes|Runtime-Management]]
- [[security|Security]]
- [[daemon|Daemon]]
- [[shell-integration|Shell-Integration]]

**Advanced**
- [[architecture|Architecture]]
- [[troubleshooting|Troubleshooting]]
- [[faq|FAQ]]
EOF

# Commit and push
cd "$WIKI_DIR"
git config user.name "github-actions[bot]"
git config user.email "github-actions[bot]@users.noreply.github.com"
git add .
if git diff --staged --quiet; then
    echo "No changes to sync."
else
    git commit -m "Sync documentation from $GITHUB_SHA"
    git push
fi
cd ..

rm -rf "$WIKI_DIR"
