#!/usr/bin/env bash
# Extract release notes for a specific version from changelog
# Usage: ./scripts/extract-release-notes.sh <version>
# Example: ./scripts/extract-release-notes.sh 0.1.204

set -euo pipefail

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    echo "Error: Version required"
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.204"
    exit 1
fi

VERSION_CLEAN="${VERSION#v}"

CHANGELOG_FILE="${CHANGELOG_FILE:-docs/changelog.md}"

if [ ! -f "$CHANGELOG_FILE" ]; then
    echo "Error: Changelog file not found: $CHANGELOG_FILE"
    exit 1
fi

awk -v version="$VERSION_CLEAN" '
    BEGIN { 
        in_version = 0
        found = 0
    }
    
    /^## \[/ {
        if (in_version) {
            exit
        }
        if ($0 ~ "\\[" version "\\]") {
            in_version = 1
            found = 1
            next
        }
    }
    
    in_version {
        print
    }
    
    END {
        if (!found) {
            print "No changelog entries found for version " version > "/dev/stderr"
            exit 1
        }
    }
' "$CHANGELOG_FILE"
