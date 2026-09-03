## Description

Brief description of what this PR does.

Fixes #(issue number)

## Type of Change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring (no functional changes)
- [ ] Dependency update

## Changes Made

- List of changes
- Another change
- Yet another change

## Testing

**How has this been tested?**

- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing performed
- [ ] Benchmarks run (for performance changes)

**Test output:**
```bash
cargo test --features arch
# Paste relevant test output
```

## Performance Impact

**For performance-related changes:**

```bash
# Before
hyperfine "omg search firefox"
# Time: X ms

# After  
hyperfine "omg search firefox"
# Time: Y ms
# Improvement: Z%
```

## Breaking Changes

**Does this PR introduce breaking changes?**

- [ ] Yes (describe migration path below)
- [ ] No

**Migration guide** (if applicable):
```bash
# Old way
omg old-command

# New way
omg new-command
```

## Checklist

**Code Quality:**
- [ ] Code follows project style guidelines
- [ ] I have run `cargo fmt`
- [ ] I have run `cargo clippy --features arch -- -D warnings`
- [ ] No new compiler warnings introduced
- [ ] Comments added for complex logic
- [ ] Public APIs have rustdoc comments

**Testing:**
- [ ] Existing tests pass (`cargo test --features arch`)
- [ ] New tests added for new functionality
- [ ] Edge cases covered
- [ ] Error handling tested

**Documentation:**
- [ ] README updated (if needed)
- [ ] docs/changelog.md updated
- [ ] Inline documentation updated
- [ ] Example code added/updated (if applicable)

**Performance:**
- [ ] No performance regressions introduced
- [ ] Benchmarks run for performance-sensitive changes
- [ ] Memory usage checked (if applicable)

**Security:**
- [ ] No new security vulnerabilities introduced
- [ ] Input validation added for user-facing features
- [ ] No unsafe code added (or justified if needed)
- [ ] Dependencies reviewed with `cargo audit`

## Additional Notes

Any additional context, screenshots, or information for reviewers.

## Related Issues/PRs

- Related to #
- Depends on #
- Blocks #
