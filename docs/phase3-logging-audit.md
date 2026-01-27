# Phase 3: Logging Audit - Convert to 100% Tracing

**Date:** 2026-01-26
**Task:** Task 5 - Convert to 100% Tracing
**Status:** In Progress

## Initial Audit Results

### Summary Statistics

- **println! occurrences:** 855
- **eprintln! occurrences:** 40
- **Total legacy logging:** 895 calls

### Files Affected

#### Files with println! (54 files)
- Runtime modules (10 files): rust.rs, ruby.rs, python.rs, node.rs, mise.rs, java.rs, go.rs, bun.rs, common.rs
- CLI modules (28 files): Various command implementations
- Package managers (7 files): aur.rs, arch.rs, pacman_db.rs, parallel_sync.rs, alpm_ops.rs, mock.rs
- Core (3 files): task_runner.rs, safe_ops.rs, env/fingerprint.rs
- Hooks (2 files): mod.rs, completions.rs
- Binaries (4 files): omg.rs, omgd.rs, omg-fast.rs, perf_audit.rs

#### Files with eprintln! (12 files)
- Binaries: omg.rs, omgd.rs, omg-fast.rs
- Package managers: aur.rs, parallel_sync.rs, mock.rs
- CLI: packages/status.rs, packages/info.rs, packages/mod.rs, tui/mod.rs, tea/mod.rs
- Core: safe_ops.rs

### Conversion Patterns

#### User-Facing Output (println!)
Most println! calls are user-facing progress indicators and success messages:
- Installation progress: "Installing X..." → `tracing::info!`
- Download progress: "Downloading X..." → `tracing::info!`
- Extraction progress: "Extracting..." → `tracing::info!`
- Success messages: "✓ Completed" → `tracing::info!`
- Informational hints: "Try: omg..." → `tracing::info!`

#### Error Output (eprintln!)
eprintln! calls are primarily errors and warnings:
- Build failures: "Build failed: X" → `tracing::error!`
- Fatal errors: "❌ Fatal error: X" → `tracing::error!`
- Warnings: "⚠ Warning: X" → `tracing::warn!`
- Fallback notifications: "Warning: X failed, falling back" → `tracing::warn!`
- Debug info in mock: "Mock saving state" → `tracing::debug!`

### Conversion Strategy

Given the large volume (895 calls), we'll convert in logical batches:

#### Batch 1: Binaries (omg.rs, omgd.rs, omg-fast.rs) - High Priority
- **Files:** 3 files
- **Estimated calls:** ~50
- **Rationale:** Entry points, most visible to users

#### Batch 2: Package Managers - Critical Path ✅ (Completed 2026-01-26)
- **Files:** aur.rs, parallel_sync.rs, mock.rs, arch.rs, alpm_ops.rs, pacman_db.rs
- **Converted calls:** 21 println!/eprintln! calls
- **Result:** All tests passing, 0 println!/eprintln! remain in package_managers/
- **Rationale:** Core functionality, error handling

#### Batch 3: Runtime Modules - High Volume ✅ (Completed 2026-01-26)
- **Files:** rust.rs, ruby.rs, python.rs, node.rs, java.rs, go.rs, bun.rs, mise.rs, common.rs
- **Converted calls:** 54 println! calls
- **Result:** All tests passing, 0 println! remain in runtimes/
- **Rationale:** Consistent patterns, lower risk

#### Batch 4: CLI Commands - Diverse
- **Files:** All CLI command modules
- **Estimated calls:** ~300
- **Rationale:** User-facing commands, varied patterns

#### Batch 5: Core & Hooks - Final
- **Files:** task_runner.rs, safe_ops.rs, hooks/mod.rs, etc.
- **Estimated calls:** ~45
- **Rationale:** Lower volume, infrastructure code

### Testing Strategy

After each batch:
1. Run `cargo check`
2. Run `cargo test`
3. Run `cargo clippy`
4. Manual smoke test of affected functionality
5. Commit if all checks pass

### Exceptions

The following println!/eprintln! calls may remain:
- Test code (in #[cfg(test)] modules)
- Debug/development code (if clearly marked)
- Binary output that MUST go to stdout/stderr (after review)

### Benefits

- **Structured logging:** Consistent format with spans and fields
- **Configurable levels:** Control verbosity via RUST_LOG
- **Better observability:** Integration with telemetry systems
- **Performance:** Tracing is zero-cost when disabled
- **Context preservation:** Automatic span tracking

## Batch Progress

### Batch 1: Binaries ✅ (Completed)
- [ ] src/bin/omg.rs
- [ ] src/bin/omgd.rs
- [ ] src/bin/omg-fast.rs

### Batch 2: Package Managers
- [ ] src/package_managers/aur.rs
- [ ] src/package_managers/parallel_sync.rs
- [ ] src/package_managers/mock.rs

### Batch 3: Runtime Modules ✅
- [x] src/runtimes/rust.rs (5 calls)
- [x] src/runtimes/ruby.rs (7 calls)
- [x] src/runtimes/python.rs (6 calls)
- [x] src/runtimes/node.rs (5 calls)
- [x] src/runtimes/java.rs (8 calls)
- [x] src/runtimes/go.rs (8 calls)
- [x] src/runtimes/bun.rs (5 calls)
- [x] src/runtimes/mise.rs (5 calls)
- [x] src/runtimes/common.rs (4 calls)

### Batch 4: CLI Commands
- [ ] (Multiple CLI files - to be detailed)

### Batch 5: Core & Hooks
- [ ] src/core/task_runner.rs
- [ ] src/core/safe_ops.rs
- [ ] src/hooks/mod.rs
- [ ] src/hooks/completions.rs

## Completion Criteria

- [ ] No println! calls in src/ (except tests)
- [ ] No eprintln! calls in src/ (except tests)
- [ ] All tests passing
- [ ] Clippy clean
- [ ] Manual testing confirms proper logging behavior
- [ ] Documentation updated

## Notes

- Started: 2026-01-26
- Estimated completion: TBD (depends on testing results)
- Risk: High volume requires careful testing
