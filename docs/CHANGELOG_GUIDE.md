# Changelog Generation Guide

**World-Class Changelogs for OMG**

This guide explains OMG's changelog generation system, designed to transform git commits into user-focused release notes that developers actually want to read.

---

## Philosophy

### User Impact Over Implementation Details

Bad changelog entries focus on code:
```
❌ "refactor: extract reusable analytics components"
❌ "chore: update dependencies"
❌ "fix: add explicit type hint for aur_client"
```

Good changelog entries focus on user benefit:
```
✓ "Performance: 3-5x faster Debian package operations through incremental indexing"
✓ "New Feature: Interactive TUI dashboard for system monitoring"
✓ "Bug Fix: Sudo prompts now work correctly in interactive mode"
```

### The Three Questions

Every changelog entry should answer:

1. **What changed?** (from the user's perspective)
2. **Why does it matter?** (the benefit or impact)
3. **What should I do?** (if action required - breaking changes, deprecations)

---

## Quick Start

### Prerequisites

Install `git-cliff`:

```bash
# Arch Linux
pacman -S git-cliff

# Cargo
cargo install git-cliff

# Homebrew
brew install git-cliff
```

### Generate a Changelog

```bash
# Preview unreleased changes (before creating a release)
./scripts/generate-changelog.sh --preview

# Generate full changelog (recommended after each release)
./scripts/generate-changelog.sh

# Update with just the latest release
./scripts/generate-changelog.sh --latest

# Generate unreleased changes for docs
./scripts/generate-changelog.sh --unreleased
```

### Output

Generated changelog: `/home/pyro1121/Documents/code/filemanager/omg/docs/changelog.md`

Automatic backup created before overwriting.

---

## Commit Message Guidelines

### Conventional Commit Format

Use conventional commits for automatic categorization:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types** (determines changelog section):

- `feat`: New features → "✨ New Features"
- `fix`: Bug fixes → "🐛 Bug Fixes"
- `perf`: Performance improvements → "⚡ Performance"
- `docs`: Documentation → "📚 Documentation"
- `refactor`: Code refactoring → "♻️ Refactoring"
- `test`: Tests → "🧪 Testing"
- `chore`: Maintenance → "🔧 Maintenance"
- `ci`: CI/CD changes → "👷 CI/CD"

**Scopes** (provides context):

- `debian`: Debian/Ubuntu support
- `cli`: Command-line interface
- `daemon`: Background daemon
- `search`: Package search functionality
- `security`: Security features
- `docs`: Documentation

### Examples of Great Commits

#### Performance Improvements

```
perf(debian): incremental index updates for 3-5x faster operations

- Add string interning for common fields to reduce memory usage
- Implement incremental updates tracking per-file mtimes vs full rebuilds
- Switch to LZ4 compression for 60-70% smaller cache with faster I/O
- Optimize package file parsing with 64KB buffers and parallel parsing

Benchmarks show 3-5x speedup on Debian/Ubuntu package operations.
```

#### New Features

```
feat(cli): interactive TUI dashboard for system monitoring

Adds `omg dash` command that launches a full-screen terminal UI
showing:
- Real-time system status
- CVE vulnerability alerts
- Package update notifications
- Fleet management overview

Built with ratatui for smooth 60fps rendering.
```

#### Bug Fixes

```
fix(cli): ensure sudo prompts work correctly in interactive mode

Use `std::process::Command` instead of tokio::process to ensure
TTY inheritance for interactive sudo prompts. This fixes the issue
where password prompts would not appear when running privileged
operations like `omg install`.

Fixes #42
```

### Examples of Commits to Avoid

These will be automatically skipped:

```
❌ "WIP"
❌ "notes"
❌ "temp fix"
❌ "Release v0.1.139"
❌ "chore: update lock file"
❌ "style: format code"
```

---

## Workflow

### During Development

**Preview upcoming changelog:**

```bash
./scripts/generate-changelog.sh --preview
```

This shows what will appear in the next release changelog. Use it to:

- Check if your commits are categorized correctly
- Identify commits that need better descriptions
- Plan the release notes

### Before a Release

**1. Review unreleased commits:**

```bash
./scripts/generate-changelog.sh --preview
```

**2. Enhance terse commits (optional):**

```bash
./scripts/enhance-commit-messages.py
```

This identifies commits with poor descriptions and provides templates for rewriting them.

**3. Update changelog for docs:**

```bash
./scripts/generate-changelog.sh --unreleased
```

**4. Review and commit:**

```bash
git add docs/changelog.md
git commit -m "docs: update changelog for v0.1.140"
```

### After a Release

**Generate complete changelog:**

```bash
./scripts/generate-changelog.sh
git add docs/changelog.md
git commit -m "docs: regenerate full changelog"
```

### For Hotfix Releases

**Generate changelog for specific tag:**

```bash
./scripts/generate-changelog.sh --tag v0.1.140
```

---

## Enhancing Commit Messages

The `enhance-commit-messages.py` script helps identify and improve terse commits.

### Usage

```bash
# Find commits that need better descriptions
./scripts/enhance-commit-messages.py

# Analyze specific range
./scripts/enhance-commit-messages.py --from v0.1.135 --to HEAD

# Show more commits
./scripts/enhance-commit-messages.py --limit 20
```

### How It Works

The script:

1. Analyzes commits in the specified range
2. Identifies terse/unclear commit messages
3. Provides context (files changed, diff stats)
4. Generates templates for AI-enhanced rewriting

### Rewriting Commits

**Option 1: Interactive Rebase** (recommended for recent commits)

```bash
# Start interactive rebase from N commits ago
git rebase -i HEAD~5

# Mark commits as 'reword' (change 'pick' to 'r')
# Git will pause at each commit for you to edit the message
```

**Option 2: Filter-Branch** (for older commits - rewrites history)

```bash
# WARNING: This rewrites git history
# Only use if you haven't pushed yet or coordinate with team

git filter-branch --msg-filter '
if [ "$GIT_COMMIT" = "abc123" ]; then
    echo "perf(debian): incremental index updates for 3-5x faster operations"
else
    cat
fi
' HEAD~10..HEAD
```

**Option 3: Amend Last Commit** (simplest for most recent commit)

```bash
git commit --amend
# Edit the message in your editor
```

---

## Configuration

### cliff.toml

The `cliff.toml` file controls changelog generation.

**Key settings:**

```toml
[git]
conventional_commits = true      # Parse conventional commit format
filter_unconventional = false    # Include all commits
sort_commits = "newest"          # Most recent first

[git.commit_parsers]
# Categorize commits into sections
{ message = "^feat", group = "✨ New Features" }
{ message = "^fix", group = "🐛 Bug Fixes" }
{ message = "^perf", group = "⚡ Performance" }

# Skip noise
{ message = "^WIP", skip = true }
{ message = "^Release v", skip = true }
```

### Customization

**Add new commit types:**

```toml
{ message = "^breaking", group = "⚠️ Breaking Changes" }
{ message = "^security", group = "🔒 Security" }
```

**Skip additional patterns:**

```toml
{ message = "^experiment", skip = true }
{ message = "^draft", skip = true }
```

**Change grouping order:**

Sections appear in the order they're defined in `commit_parsers`. Put high-priority sections (Features, Breaking Changes) first.

---

## Best Practices

### 1. Write Commits for Humans

Think about a developer reading the changelog in 6 months. What do they need to know?

**Bad:**
```
fix: update regex
```

**Good:**
```
fix(search): handle special characters in package names correctly

Package names with characters like '+', '@', or '~' now work properly
in search queries. Previously these would cause regex parse errors.
```

### 2. Include Context in Body

The subject line is a summary. The body should provide details.

```
feat(security): add vulnerability scanning for installed packages

Integrates with ALSA (Arch Linux Security Advisories) and OSV.dev
to scan installed packages for known CVEs. Results are shown in
`omg audit` and the TUI dashboard.

Each package gets a security grade: VERIFIED, COMMUNITY, or RISK.
```

### 3. Link to Issues/PRs

Reference issue numbers for automatic linking:

```
fix(daemon): prevent multiple daemon instances

Use file locking on the socket path to ensure only one daemon
runs at a time. This prevents port conflicts and resource waste.

Fixes #123
```

### 4. Group Related Changes

When making multiple related changes, use a detailed body:

```
perf(debian): optimize package index parsing

- Switch to 64KB read buffers (10x faster I/O)
- Use memchr for paragraph splitting (SIMD acceleration)
- Add parallel parsing for files >100 packages
- Implement string interning for common fields

Combined speedup: 3-5x faster on typical repositories.
```

### 5. Explain Breaking Changes

Breaking changes need clear migration guidance:

```
feat(cli): redesign environment command structure

BREAKING CHANGE: Environment commands moved under `omg env`:
- Old: `omg capture`, `omg check`, `omg share`
- New: `omg env capture`, `omg env check`, `omg env share`

Update shell scripts and CI/CD accordingly. Old commands will be
removed in v0.2.0 (deprecated period: 3 months).

Migration:
  sed -i 's/omg capture/omg env capture/g' scripts/*.sh
```

---

## Release Checklist

Use this checklist when preparing a release:

- [ ] All tests passing
- [ ] Benchmarks run and documented (if performance changes)
- [ ] Version bumped in `Cargo.toml`
- [ ] Preview changelog: `./scripts/generate-changelog.sh --preview`
- [ ] Review commit messages - enhance if needed
- [ ] Update changelog: `./scripts/generate-changelog.sh --unreleased`
- [ ] Review generated changelog for accuracy
- [ ] Create git tag: `git tag -a v0.1.140 -m "Release v0.1.140"`
- [ ] Regenerate full changelog: `./scripts/generate-changelog.sh`
- [ ] Commit changelog: `git commit -am "docs: update changelog for v0.1.140"`
- [ ] Push tag: `git push origin v0.1.140`
- [ ] Create GitHub release (use changelog content)

---

## Troubleshooting

### "No commits found"

Ensure you have git tags:

```bash
git tag -l
# If empty, create initial tag:
git tag -a v0.1.0 -m "Initial release"
```

### "git-cliff not found"

Install git-cliff:

```bash
cargo install git-cliff
# or
pacman -S git-cliff
```

### Commits not categorized correctly

Check your commit message format:

```bash
# View recent commits
git log --oneline -10

# Check specific commit
git show COMMIT_HASH
```

Ensure they follow conventional commit format: `type(scope): description`

### Missing commits in changelog

Check if they're being skipped:

```bash
# Test with filter disabled
git-cliff --config cliff.toml --unreleased --include-path "**" --no-filter
```

Look for patterns in `cliff.toml` that might be skipping your commits.

### Changelog duplicates sections

This happens if commit types are ambiguous. Use consistent prefixes:

```bash
# Bad (mixed types)
git log --oneline
feat: add feature
feature: add another

# Good (consistent)
feat: add feature
feat: add another feature
```

---

## Advanced Usage

### Generate Changelog for Specific Version Range

```bash
git-cliff --config cliff.toml v0.1.135..v0.1.139 -o release-notes.md
```

### Export to JSON

```bash
git-cliff --config cliff.toml --unreleased -o changelog.json --output-format json
```

### Custom Template

Create a custom template file:

```bash
# custom-template.md
{% for group, commits in commits | group_by(attribute="group") %}
## {{ group }}
{% for commit in commits %}
- {{ commit.message }} ({{ commit.hash | truncate(length=8) }})
{% endfor %}
{% endfor %}
```

Use it:

```bash
git-cliff --config cliff.toml --template custom-template.md
```

### Generate GitHub Release Notes

```bash
# Generate markdown for GitHub release
git-cliff --config cliff.toml --tag v0.1.140 --strip all -o release-notes.md

# Copy to clipboard (Linux)
git-cliff --config cliff.toml --tag v0.1.140 --strip all | xclip -selection clipboard

# Create release via gh CLI
gh release create v0.1.140 --notes-file release-notes.md
```

---

## Integration with CI/CD

### GitHub Actions

Add to `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  changelog:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Need full history for git-cliff

      - name: Install git-cliff
        run: cargo install git-cliff

      - name: Generate changelog
        run: |
          ./scripts/generate-changelog.sh --tag ${{ github.ref_name }}

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          body_path: docs/changelog.md
          files: |
            target/release/omg
            target/release/omgd
```

### Pre-commit Hook

Validate commit messages:

```bash
# .git/hooks/commit-msg
#!/usr/bin/env bash

commit_msg=$(cat "$1")

# Check conventional commit format
if ! echo "$commit_msg" | grep -qE '^(feat|fix|perf|docs|refactor|test|chore|ci)(\(.+\))?: .+'; then
    echo "ERROR: Commit message does not follow conventional commit format"
    echo ""
    echo "Format: <type>(<scope>): <description>"
    echo ""
    echo "Types: feat, fix, perf, docs, refactor, test, chore, ci"
    echo ""
    echo "Example: feat(cli): add interactive dashboard"
    exit 1
fi
```

Make it executable:

```bash
chmod +x .git/hooks/commit-msg
```

---

## Examples from OMG

### Example 1: Performance Improvement

**Commit:**
```
perf(debian): incremental index updates, string interning, and optimized parsing for 3-5x faster package operations

- Add string interning for common fields (arch/section/priority) to reduce memory
- Implement incremental index updates tracking per-file mtimes vs full rebuilds
- Switch to LZ4 compression for 60-70% smaller cache with faster I/O (v5 format)
- Optimize package file parsing: 64KB buffers, memchr paragraph splitting, parallel parsing for >100 packages
- Fast-path field parsing with minimal allocations

Benchmarks on Debian 12 with ~75k packages:
- Before: 450ms full index rebuild
- After: 130ms (3.5x faster)
- Incremental updates: 15-30ms (15-30x faster)

Cache size: 8.2MB → 2.8MB (66% reduction)
```

**Changelog Entry:**
```
### ⚡ Performance

- **Debian**: Incremental index updates, string interning, and optimized parsing for 3-5x faster package operations
  - Add string interning for common fields (arch/section/priority) to reduce memory
  - Implement incremental index updates tracking per-file mtimes vs full rebuilds
  - Switch to LZ4 compression for 60-70% smaller cache with faster I/O (v5 format)
  - Optimize package file parsing: 64KB buffers, memchr paragraph splitting, parallel parsing for >100 packages
```

### Example 2: New Feature

**Commit:**
```
feat(docs): match main site theme + analytics + progressive disclosure

- Replace VELOCITY theme (yellow/orange) with main site colors (indigo/cyan/purple)
- Add comprehensive analytics system with batching and session tracking
- Implement progressive disclosure: 2-level max navigation, collapsed advanced sections
- Add Quick Start section with copy-to-clipboard code blocks
- Fix memory leaks in SpeedMetric and TerminalDemo components
- Add accessibility improvements (aria-labels, reduced motion support)
- Configure Cloudflare Pages deployment with wrangler
```

**Changelog Entry:**
```
### ✨ New Features

- **Docs**: Match main site theme + analytics + progressive disclosure
  - Replace VELOCITY theme (yellow/orange) with main site colors (indigo/cyan/purple)
  - Add comprehensive analytics system with batching and session tracking
  - Implement progressive disclosure: 2-level max navigation, collapsed advanced sections
  - Add Quick Start section with copy-to-clipboard code blocks
  - Fix memory leaks in SpeedMetric and TerminalDemo components
  - Add accessibility improvements (aria-labels, reduced motion support)
  - Configure Cloudflare Pages deployment with wrangler
```

---

## FAQ

### Should I regenerate the full changelog after every commit?

No. Regenerate the full changelog:
- After each release
- Before creating a tag
- When you want to update docs

For daily work, use `--preview` to see unreleased changes.

### Can I manually edit the generated changelog?

Yes, but your changes will be overwritten next time you regenerate. Instead:

1. Fix the commit messages (via rebase/amend)
2. Customize templates in `cliff.toml`
3. Add manual sections BEFORE the generated content

### What if I need to exclude a commit from the changelog?

Add it to the skip patterns in `cliff.toml`:

```toml
{ message = "^experiment:", skip = true }
```

Or mark the commit with a skip tag:

```
chore: internal refactoring [skip changelog]
```

Then add a parser:

```toml
{ message = ".*\\[skip changelog\\].*", skip = true }
```

### How do I handle multi-line commit messages?

Git-cliff automatically includes the commit body. Format it with markdown:

```
feat(security): add vulnerability scanning

This feature integrates with:
- ALSA (Arch Linux Security Advisories)
- OSV.dev (Open Source Vulnerabilities)

Results appear in:
1. `omg audit` command
2. TUI dashboard
3. JSON API endpoint

Each package gets graded: VERIFIED, COMMUNITY, or RISK.
```

---

## Resources

- [git-cliff Documentation](https://git-cliff.org/docs/)
- [Conventional Commits Spec](https://www.conventionalcommits.org/)
- [Semantic Versioning](https://semver.org/)
- [Keep a Changelog](https://keepachangelog.com/)

---

## Contributing

When contributing to OMG, please follow these commit guidelines:

1. Use conventional commit format
2. Write clear, user-focused descriptions
3. Include context in the body for complex changes
4. Reference issue numbers
5. Run `./scripts/generate-changelog.sh --preview` before creating PRs

Your commit messages directly become release notes that users read. Make them count!
