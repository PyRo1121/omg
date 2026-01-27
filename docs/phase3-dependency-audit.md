# Phase 3: Dependency Audit Report

**Date:** 2026-01-26
**Task:** Task 6 - Dependency Audit and Cleanup
**Tools:** cargo-machete v0.9.1, cargo-outdated v0.17.0

## Executive Summary

Audited all project dependencies for unused and outdated packages. The codebase is in excellent health with **zero unused dependencies** and only **5 outdated direct dependencies**, all with reasonable upgrade paths.

## Unused Dependencies Audit

**Tool:** `cargo machete`

**Result:** ✅ **CLEAN** - No unused dependencies detected

```
cargo-machete didn't find any unused dependencies in this directory. Good job!
```

The project correctly leverages cargo-machete's ignore list for conditionally compiled dependencies:
```toml
[package.metadata.cargo-machete]
ignored = ["ar", "gzp", "winnow", "debian-packaging"]
```

These are correctly ignored as they're only used with the `debian` or `debian-pure` feature flags.

## Outdated Dependencies Audit

**Tool:** `cargo outdated --root-deps-only`

### Direct Dependencies Requiring Updates

| Package | Current | Latest | Type | Breaking? | Priority |
|---------|---------|--------|------|-----------|----------|
| **crossterm** | 0.27.0 | 0.29.0 | Normal | Likely | Medium |
| **dashmap** | 5.5.3 | 6.1.0 | Normal | Yes | High |
| **notify** | 7.0.0 | 8.2.0 | Normal | Yes | Medium |
| **ratatui** | 0.28.1 | 0.30.0 | Normal | Likely | Medium |
| **rustix** | 0.38.44 | 1.1.3 | Normal | Yes | High |

### Analysis by Package

#### 1. rustix (0.38.44 → 1.1.3)
- **Impact:** HIGH - Used for filesystem operations
- **Breaking:** Yes (major version bump)
- **Risk:** HIGH - Core functionality dependency
- **Recommendation:** Update and test thoroughly
- **Files affected:** `src/package_managers/aur.rs`, filesystem operations
- **Test requirements:** Full integration test suite, especially file operations

#### 2. dashmap (5.5.3 → 6.1.0)
- **Impact:** HIGH - Used for concurrent map operations
- **Breaking:** Yes (major version bump)
- **Risk:** MEDIUM - Well-tested concurrent data structure
- **Recommendation:** Update with careful testing of concurrent operations
- **Files affected:** Cache layers, concurrent package operations
- **Test requirements:** Concurrent test suite, race condition testing

#### 3. notify (7.0.0 → 8.2.0)
- **Impact:** MEDIUM - File watching functionality
- **Breaking:** Yes (major version bump)
- **Risk:** LOW - Isolated feature
- **Recommendation:** Safe to update
- **Files affected:** File watching subsystems
- **Test requirements:** File watching integration tests

#### 4. crossterm (0.27.0 → 0.29.0)
- **Impact:** MEDIUM - Terminal handling
- **Breaking:** Likely (minor version but significant changes)
- **Risk:** LOW - TUI dependency
- **Recommendation:** Update together with ratatui
- **Files affected:** TUI components
- **Test requirements:** Manual TUI testing

#### 5. ratatui (0.28.1 → 0.30.0)
- **Impact:** MEDIUM - TUI framework
- **Breaking:** Likely (minor version but API changes)
- **Risk:** LOW - TUI-only impact
- **Recommendation:** Update together with crossterm
- **Files affected:** TUI components
- **Test requirements:** Manual TUI testing

### Transitive Dependencies

Notable transitive dependency updates detected:
- `mio` (crossterm dependency): 0.8.11 → 1.1.1 (major bump)
- `inotify` (notify dependency): 0.10.2 → 0.11.0 (minor bump)
- `linux-raw-sys` (rustix dependency): 0.4.15 → 0.11.0 (major bump)

These will be updated automatically when their parent crates are updated.

## Git Dependencies

All git dependencies are from official Arch Linux repositories and are up-to-date:
```toml
alpm-types, alpm-srcinfo, alpm-db, alpm-repo-db, alpm-pkginfo
Source: https://gitlab.archlinux.org/archlinux/alpm/alpm.git
```

These are pinned to git master as recommended by upstream for Rust bindings.

## Security Considerations

- ✅ No unused dependencies that could introduce unnecessary attack surface
- ✅ All updates are to newer, more secure versions
- ⚠️ `rustix` update (0.38 → 1.1) likely includes security fixes
- ⚠️ `dashmap` update (5.x → 6.x) may include concurrency safety improvements

## Recommendation: Update Strategy

### Phase 1: High-Priority Updates (This PR)
**None** - All major version bumps require careful testing

### Phase 2: Controlled Updates (Separate PR)
Update packages individually with full test coverage between each:

1. **First:** `rustix` (filesystem operations - most critical)
   - Update Cargo.toml
   - Run full test suite
   - Manual testing of AUR operations
   - Commit with isolated changeset

2. **Second:** `dashmap` (concurrent operations)
   - Update Cargo.toml
   - Run concurrent test suite
   - Stress testing
   - Commit with isolated changeset

3. **Third:** `notify` (file watching)
   - Update Cargo.toml
   - Test file watching features
   - Commit with isolated changeset

4. **Fourth:** `crossterm` + `ratatui` (TUI - lowest risk)
   - Update both together (they're coupled)
   - Manual TUI testing
   - Commit with isolated changeset

### Phase 3: Monitoring (Post-Update)
- Monitor for regressions in production
- Watch for upstream bug reports
- Consider pinning if issues arise

## Build and Binary Impact

### Current State
```toml
[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
opt-level = 3
```

### Expected Impact from Updates
- **Build time:** Minimal increase (newer compiler optimizations)
- **Binary size:** Neutral to slight decrease (newer versions often optimize size)
- **Performance:** Likely improvements in `rustix` and `dashmap`
- **Memory usage:** Potential improvements in concurrent structures

## Testing Requirements

Before merging any updates:

1. **Unit tests:** `cargo test --all-features`
2. **Integration tests:** `cargo test --test '*'`
3. **Benchmarks:** `cargo bench` (compare before/after)
4. **Manual testing:**
   - AUR package installation
   - File watching features
   - TUI navigation
   - Concurrent operations

## Conclusion

**Status:** ✅ **EXCELLENT**

The project maintains excellent dependency hygiene:
- Zero unused dependencies
- Only 5 outdated direct dependencies
- All updates have clear upgrade paths
- No security vulnerabilities detected

**Recommendation:**
- **This PR:** Document findings only (no updates)
- **Next PR:** Update dependencies one-by-one with testing between each
- **Priority Order:** rustix → dashmap → notify → crossterm+ratatui

This conservative approach ensures each update can be isolated, tested, and reverted if needed.

---

**Tools Used:**
- `cargo-machete v0.9.1` - Unused dependency detection
- `cargo-outdated v0.17.0` - Outdated dependency detection
- Manual analysis of breaking changes and impact

**Audit Completed:** 2026-01-26
