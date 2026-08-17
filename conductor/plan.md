# Implementation Plan - Honest CLI, dead code, real tests

Tracked debt paydown from the 2026-08-17 codebase review. Follow global AGENTS.md and RUST-CODING-STANDARDS.md.

## Phase 1: Commands must not report success when they failed

- [x] License activate returns `Err` when validation/activation fails
- [x] `omg clean --cache` returns `Err` when cache cleanup fails
- [x] `omg migrate import` returns `Err` if any runtime or package install failed
- [x] `omg daemon start` returns `Err` if spawn fails or the daemon never becomes ready
- [x] TEA `Program::run` returns `Err` on `Cmd::Error` (info/status no longer exit 0 after printing an error)
- [x] `omg search --interactive` fails closed (flag was a no-op)
- [x] `omg outdated --security` fails closed (never classified security updates)
- [x] Fleet push 404 is an error; fleet remediate is not implemented
- [x] Team invite/roles/notify and enterprise policy set/inherit/server init fail closed instead of printing fake success

## Phase 2: Remove unused CLI/core modules

- [x] Delete unused `src/cli/json_output.rs`
- [x] Delete unused `src/cli/tables.rs`
- [x] Delete unused `src/core/constants.rs`
- [x] Delete unused TEA wrappers `run_{install,remove,update,search}_elm`
- [x] Delete unused `debian-resolvo` adapter, feature, and CI flag
- [x] Remove CI canary targeting missing `tests/aur_error_recovery.rs`

## Phase 3: Debian install SHA256 verification

- [x] Require SHA256 after download (and after content-store cache hit); fail closed if missing or mismatched

## Phase 4: Security policy honesty

- [x] Stop minting SLSA Locked/Level2/Level3 from package names
- [x] Enforce `require_pgp` in `check_package`
- [x] SPDX token matching so `MIT` does not match `LIMITED`

## Phase 5: Tests and lints

- [x] Replace panic-only / constructor-only tests on the touched paths with behavior assertions
- [x] Single Clippy source of truth in `Cargo.toml`
