# AUR Module Refactoring Plan

**Status:** Deferred (requires careful migration due to 2382 LOC monolithic file)  
**Priority:** Medium (maintainability improvement, not blocking)  
**Estimated Effort:** 4-6 hours (careful extraction + testing)

## Current State

`src/package_managers/aur.rs` is a monolithic 2382-line file containing:
- Type definitions (`AurError`, `AurClient`, `MakepkgEnv`)
- Utility functions (user/permission handling, progress indicators)
- RPC client implementation (search, info, updates)
- Build orchestration (makepkg, dependencies, caching)
- Security sandbox (bubblewrap, chroot)
- Git operations (clone, pull)
- Package archive parsing
- Tests (10 unit tests added in recent session)

## Recommended Structure

Following Cargo's multi-file module pattern:

```
src/package_managers/aur/
├── mod.rs           # Public API re-exports
├── types.rs         # AurError, AurClient, MakepkgEnv, AurResponse
├── utils.rs         # Helper functions (user handling, spinners)
├── client.rs        # AurClient::search/info/get_update_list
├── build.rs         # makepkg execution, caching, install
├── sandbox.rs       # Bubblewrap/chroot security
├── git.rs           # Clone/pull operations
└── archive.rs       # Package archive parsing
```

## Migration Strategy

### Phase 1: Extract Pure Utilities (Low Risk)
1. Create `aur/utils.rs`:
   - `has_word_boundary_match()`
   - `get_original_user()`
   - `get_original_user_home()`
   - `is_root_owned()`
   - `create_dir_as_user()`
   - `create_dir_as_user_sync()`
   - `remove_dir_as_user()`
   - `create_spinner()`

2. Update `aur.rs` to import from `utils`
3. Run full test suite (345 tests)

### Phase 2: Extract Types (Low Risk)
1. Create `aur/types.rs`:
   - `AurError` enum
   - `AurClient` struct (fields only)
   - `MakepkgEnv` struct
   - `AurResponse` struct

2. Update `aur.rs` to import from `types`
3. Run full test suite

### Phase 3: Extract Git Operations (Medium Risk)
1. Create `aur/git.rs`:
   - `git_clone()`
   - `git_pull()`
   - Related error handling

2. Update `AurClient` impl to use `git::*`
3. Run full test suite

### Phase 4: Extract Build Logic (High Risk)
1. Create `aur/build.rs`:
   - `makepkg_env()`
   - `run_build()`
   - `run_sandboxed_makepkg()`
   - `run_chroot_build()`
   - `run_native_makepkg()`
   - `cached_package()`
   - `write_cache_key()`

2. Update `AurClient` to delegate to `build::*`
3. Run full test suite + manual testing

### Phase 5: Extract Sandbox (Medium Risk)
1. Create `aur/sandbox.rs`:
   - Bubblewrap isolation logic
   - Chroot setup
   - Security validation

2. Update `build.rs` to use `sandbox::*`
3. Run full test suite

### Phase 6: Extract Archive Parsing (Low Risk)
1. Create `aur/archive.rs`:
   - `find_built_package()`
   - `pkg_name_from_archive()`
   - `parse_pkginfo_name()`
   - `expected_pkg_names()`
   - `find_package_in_dir()`

2. Update `build.rs` to use `archive::*`
3. Run full test suite

### Phase 7: Finalize
1. Create `aur/mod.rs` with public re-exports
2. Move tests to appropriate submodules
3. Delete monolithic `aur.rs`
4. Update all imports in codebase
5. Run full test suite (all 345 tests must pass)
6. Manual smoke testing of AUR operations

## Testing Requirements

Each phase must pass:
- ✅ All 345 unit tests
- ✅ Clippy with pedantic lints
- ✅ No new warnings
- ✅ Manual AUR install test

## Why This Was Deferred

During the quality improvement session (Feb 1, 2026), this refactoring was started but deferred because:

1. **Complexity:** 2382 lines with tightly coupled logic
2. **Risk:** High chance of breaking existing functionality
3. **Time:** 4-6 hours estimated for careful migration
4. **Dependencies:** 10+ other files import from `aur.rs`
5. **Testing Burden:** Must verify all 345 tests after each phase

**Decision:** Focus on test coverage improvements first (completed), defer structural refactoring to dedicated session.

## Progress Log

- **2026-02-01 19:45:** Created experimental `aur/` directory with `types.rs` and `utils.rs`
- **2026-02-01 20:10:** Attempted full migration to `client.rs` - too complex, rolled back
- **2026-02-01 20:15:** Decided to defer, documented plan in this file
- **Status:** Deferred - all tests passing with current monolithic structure

## Next Steps

When ready to proceed:
1. Create feature branch: `refactor/aur-module-split`
2. Follow phased approach above
3. Commit after each successful phase
4. Open PR with full test coverage proof

---

**Note:** This is a **quality-of-life** improvement, not a blocker. The current monolithic structure works correctly and is well-tested (345 tests passing, 7.5/10 Oracle score).
