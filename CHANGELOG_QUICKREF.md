# Changelog Quick Reference

**Essential commands and patterns for OMG's changelog system**

---

## Installation

```bash
# Install git-cliff
cargo install git-cliff
# or
pacman -S git-cliff
```

---

## Daily Commands

```bash
# Preview unreleased changes (during development)
./scripts/generate-changelog.sh --preview

# Find commits that need better descriptions
./scripts/enhance-commit-messages.py

# Generate full changelog (after release)
./scripts/generate-changelog.sh

# Update with latest release only
./scripts/generate-changelog.sh --latest

# Generate unreleased changes for docs
./scripts/generate-changelog.sh --unreleased
```

---

## Commit Format

### Template

```
<type>(<scope>): <clear description>

[optional body with details]

[optional footer: Fixes #123]
```

### Types

| Type | Use For | Changelog Section |
|------|---------|-------------------|
| `feat` | New features | ✨ New Features |
| `fix` | Bug fixes | 🐛 Bug Fixes |
| `perf` | Performance | ⚡ Performance |
| `docs` | Documentation | 📚 Documentation |
| `refactor` | Code refactoring | ♻️ Refactoring |
| `test` | Tests | 🧪 Testing |
| `ci` | CI/CD | 👷 CI/CD |
| `chore` | Maintenance | 🔧 Maintenance |

### Scopes

| Scope | Description |
|-------|-------------|
| `cli` | Command-line interface |
| `daemon` | Background daemon |
| `debian` | Debian/Ubuntu support |
| `search` | Search functionality |
| `security` | Security features |
| `docs` | Documentation |

---

## Examples

### Good Commits ✓

**Performance:**
```
perf(debian): 3-5x faster package operations

- Incremental index updates vs full rebuilds
- LZ4 compression for 60% smaller cache
- Parallel parsing for large files

Benchmarks: 450ms → 130ms
```

**New Feature:**
```
feat(cli): add interactive dashboard

Adds `omg dash` for real-time monitoring:
- System status
- CVE alerts
- Update notifications

Built with ratatui.
```

**Bug Fix:**
```
fix(cli): sudo prompts work in interactive mode

Use std::process::Command for TTY inheritance.

Fixes #42
```

### Bad Commits ✗

```
❌ "fix"
❌ "update"
❌ "WIP"
❌ "notes"
❌ "chore: format"
```

---

## Commit Guidelines

### DO ✓

- Use conventional commit format
- Explain **what** changed and **why**
- Include benchmarks for performance changes
- Reference issues: `Fixes #123`
- Add details in commit body
- Think about users reading this in 6 months

### DON'T ✗

- One-word commits
- "WIP" or "temp" commits
- Skip the scope when it adds clarity
- Forget the body for complex changes
- Use jargon without explanation

---

## Release Workflow

### Before Release

```bash
# 1. Preview changes
./scripts/generate-changelog.sh --preview

# 2. Check for poor commits
./scripts/enhance-commit-messages.py

# 3. Update changelog
./scripts/generate-changelog.sh --unreleased

# 4. Review
cat docs/changelog.md

# 5. Commit
git add docs/changelog.md
git commit -m "docs: update changelog for v0.1.140"
```

### After Release

```bash
# 1. Create tag
git tag -a v0.1.140 -m "Release v0.1.140"

# 2. Regenerate full changelog
./scripts/generate-changelog.sh

# 3. Commit
git add docs/changelog.md
git commit -m "docs: regenerate full changelog"

# 4. Push
git push origin v0.1.140
git push origin main
```

---

## Amending Commits

### Last Commit

```bash
git commit --amend
# Edit message in editor
```

### Recent Commits

```bash
# Interactive rebase (last 5 commits)
git rebase -i HEAD~5

# Change 'pick' to 'reword' for commits to edit
# Git will pause at each for you to rewrite
```

### Specific Commit

```bash
# Rebase from commit before target
git rebase -i abc123~1

# Mark as 'reword'
```

---

## Configuration

### cliff.toml Key Sections

**Categorization:**
```toml
{ message = "^feat", group = "✨ New Features" }
{ message = "^fix", group = "🐛 Bug Fixes" }
{ message = "^perf", group = "⚡ Performance" }
```

**Skip patterns:**
```toml
{ message = "^WIP", skip = true }
{ message = "^Release v", skip = true }
{ message = "^notes$", skip = true }
```

**Customization:**
```toml
# Add new category
{ message = "^breaking", group = "⚠️ Breaking Changes" }

# Skip pattern
{ message = "^experiment", skip = true }
```

---

## Breaking Changes

### Format

```
feat(cli)!: redesign environment commands

BREAKING CHANGE: Commands moved under `omg env`:
- Old: `omg capture`, `omg check`, `omg share`
- New: `omg env capture`, `omg env check`, `omg env share`

Migration:
  sed -i 's/omg capture/omg env capture/g' scripts/*.sh

Deprecated period: 3 months (removal in v0.2.0)
```

### Key Points

- Use `!` after type/scope: `feat(cli)!:`
- Include `BREAKING CHANGE:` in footer
- Explain what changed
- Provide migration path
- State deprecation timeline

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "No commits found" | Create tag: `git tag -a v0.1.0 -m "Initial"` |
| "git-cliff not found" | Install: `cargo install git-cliff` |
| Commits not categorized | Check format: `git log --oneline -10` |
| Missing commits | Check skip patterns in `cliff.toml` |
| Wrong category | Fix commit message type |

---

## GitHub Release

```bash
# Generate release notes
git-cliff --config cliff.toml --tag v0.1.140 --strip all -o notes.md

# Create release
gh release create v0.1.140 \
  --notes-file notes.md \
  --title "OMG v0.1.140" \
  target/release/omg \
  target/release/omgd
```

---

## Pre-commit Hook

Validate commits automatically:

```bash
# .git/hooks/commit-msg
#!/usr/bin/env bash
commit_msg=$(cat "$1")

if ! echo "$commit_msg" | grep -qE '^(feat|fix|perf|docs|refactor|test|ci|chore)(\(.+\))?: .+'; then
    echo "❌ Use conventional commit format"
    echo "Example: feat(cli): add dashboard"
    exit 1
fi
```

Make executable:
```bash
chmod +x .git/hooks/commit-msg
```

---

## Tips

### Writing Great Commits

1. **Subject line**: Clear, imperative mood ("add feature" not "added feature")
2. **Body**: Explain why, not how (code shows how)
3. **Details**: Include benchmarks, migration guides, reasoning
4. **References**: Link issues/PRs
5. **Scope**: Add context (which part changed?)

### Example

```
perf(search): switch to Nucleo for 10x faster fuzzy matching

Replace custom fuzzy matcher with Nucleo library:
- Handles 80k+ AUR packages smoothly
- SIMD-accelerated string matching
- Incremental scoring for real-time completion

Before: 45ms average search
After: 4ms average search (10x improvement)

Tested with 80,462 packages.
```

### Changelog First, Code Second

Before writing code, draft your changelog entry:

```
feat(daemon): add automatic crash recovery

If daemon crashes, automatically restart with:
- Preserved in-memory cache
- Reconnected clients
- Audit log entry of the crash

Uses systemd watchdog for monitoring.
```

This clarifies what you're building and why.

---

## Documentation

- **Full Guide**: `docs/CHANGELOG_GUIDE.md`
- **Examples**: `docs/CHANGELOG_EXAMPLE.md`
- **System Overview**: `CHANGELOG_SYSTEM.md`
- **Configuration**: `cliff.toml`

---

## Resources

- [git-cliff docs](https://git-cliff.org/docs/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [Semantic Versioning](https://semver.org/)

---

**Quick command to preview before commit:**
```bash
./scripts/generate-changelog.sh --preview && ./scripts/enhance-commit-messages.py
```

Save this reference. Your future self will thank you.
