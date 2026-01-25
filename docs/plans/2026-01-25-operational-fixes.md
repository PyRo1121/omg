# Operational Fixes and Documentation Revert Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Revert documentation to Docusaurus, fix CI build failures, and resolve CLI operational issues (daemon socket, sudo elevation).

**Architecture:**
- Documentation: Restoring the dedicated Docusaurus site in `docs-site/`.
- CLI: Fixing type inference in package service for non-Arch targets.
- Daemon: Improving socket path reliability and error reporting.

**Tech Stack:** Rust (CLI), Docusaurus (Docs), GitHub Actions (CI).

---

## Phase 1: Documentation Revert

### Task 1: Restore `docs-site`
**Goal:** Bring back the Docusaurus documentation project.

**Files:**
- Restore: `docs-site/` (directory)

**Step 1: Revert the migration commit**
```bash
# Locate the commit that deleted docs-site
git revert f6d488cd3838cb7ad7356dd6a405b0b1209520d0 --no-edit
```

**Step 2: Remove docs from `site` content**
```bash
rm -rf site/src/content/docs
```

**Step 3: Commit**
```bash
git add docs-site
git rm -r site/src/content/docs
git commit -m "chore: revert documentation to Docusaurus and remove integrated docs from site"
```

---

## Phase 2: CI & Build Fixes

### Task 2: Fix Non-Arch Build
**Goal:** Add missing type annotations that cause Ubuntu/Debian builds to fail in CI.

**Files:**
- Modify: `src/core/packages/service.rs:354`

**Step 1: Apply type hint**
Change `let aur_client = None;` to `let aur_client: Option<api::AurClient> = None;` (or equivalent type).

**Step 2: Commit**
```bash
git add src/core/packages/service.rs
git commit -m "fix: add type hint for aur_client in non-arch builds"
```

---

## Phase 3: Operational CLI Fixes

### Task 3: Fix Daemon Socket Detection
**Goal:** Improve reliability of socket connection when running under `sudo`.

**Files:**
- Modify: `src/core/paths.rs` (if exists) or check path resolution in `src/cli/doctor.rs`
- Modify: `src/daemon/server.rs`

**Step 1: Adjust Socket Path Preference**
Ensure that if `XDG_RUNTIME_DIR` is missing (common under `sudo`), we check `/run/user/<id>/omg.sock` directly before falling back to `/tmp`.

**Step 2: Update Doctor Diagnostics**
Improve the "Socket exists but connection failed" message to suggest permission issues or state if the socket belongs to a different user.

**Step 3: Commit**
```bash
git add src/daemon/server.rs src/cli/doctor.rs
git commit -m "fix: improve daemon socket path detection and diagnostics"
```

### Task 4: Improve Sudo Elevation UX
**Goal:** Provide clearer guidance when `sudo -n` fails due to missing password/configuration.

**Files:**
- Modify: `src/core/privilege.rs`

**Step 1: Enhance Error Message**
When `sudo -n` fails and `is_automation` (or `--yes`) is true, explicitly point the user to `visudo` instructions or explain that `--yes` requires `NOPASSWD`.

**Step 2: Commit**
```bash
git add src/core/privilege.rs
git commit -m "fix: clarify sudo elevation errors for automation mode"
```

---

## Phase 4: Performance Restoration

### Task 5: Fix Search Regression
**Goal:** Resolve the 2-second search delay identified in benchmarks.

**Files:**
- Modify: `src/cli/packages/search.rs`
- Modify: `src/core/packages/service.rs`

**Step 1: Profile/Inspect Search Logic**
Identify why the daemon search is exponentially slower than direct pacman calls. Likely a blocking O(N^2) merge or redundant network call.

**Step 2: Implement Fix**
Optimize the result merging or caching logic.

**Step 3: Commit**
```bash
git add src/cli/packages/search.rs src/core/packages/service.rs
git commit -m "perf: resolve search performance regression"
```
