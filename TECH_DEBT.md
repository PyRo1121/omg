# OMG Technical Debt Register — Audit v2

- **Audit date:** 2026-08-19 · **Repo state:** `08c899b` @ `main` + uncommitted `src/hooks/mod.rs` WIP (read-only audit; this file is the only write)
- **Method:** Phase 0 automated sweeps + 15 audit lanes vs `10-rust.md` / `00-global-workflow.md` standards. Sweep logs: `/tmp/omg-audit-v2/`.
- **Tags vs Audit v1:** `CONFIRMED` (still present), `CHANGED` (materially different), `NEW`.
- **Severity:** C = critical, H = high, M = medium, L = low/observation.

## Phase 0 results

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | FAIL — 5 diffs: `src/daemon/index.rs:286,663`, `src/cli/modern_ui.rs:180,347`, `tests/error_recovery_tests.rs:67` |
| `cargo clippy --workspace --all-targets` (default features) | 39 warnings, 0 errors (8 unneeded `return`, 6 `map().unwrap_or(false)`, 2 unfulfilled `#[expect]`, rest minor) |
| `cargo audit` | 1 vulnerability: **RUSTSEC-2026-0258** (h2 0.4.13 unbounded empty DATA frames; fix ≥0.4.16, via reqwest 0.13.2). 8 allowlisted warnings: unsound git2×3 / cxx / lru / scc; yanked spin 0.9.8 + 0.10.0. `.cargo/audit.toml` ignores (RUSTSEC-2023-0018, -0071) are documented. |
| `cargo tree -d` | Duplicates: getrandom×3, hashbrown×3, itertools 0.13/0.14, digest 0.10/0.11, const-oid, cpufeatures, crypto-common, block-buffer |
| `cargo deny` | Not run — binary not installed |
| Toolchain | `rust-toolchain.toml` pins 1.93.0; machine uses distro cargo 1.97.1 → pin inert locally, MSRV unverifiable here |
| Build blocker | `target` is a **broken symlink** → `.tmp/target-arch-check`; every cargo build fails (`Not a directory (os error 20)`) → see H2 |

---

## Critical

### C1 · CONFIRMED — Self-update has zero integrity verification
`src/cli/self_update.rs:52-147` · standards: 10-rust §2 (parse at boundaries), §8 (supply chain)

Downloads a tarball from `releases.pyro1121.com`, extracts it, and replaces the running binary with **no checksum, no signature, no version validation**. Sub-findings (NEW):

- **C1a** `:21-35,54` — server-controlled `target_version` string is interpolated into the download URL unvalidated (path injection into URL).
- **C1b** `:64-76` — `content_length` response header drives `Vec::with_capacity` with no upper bound → server-triggered allocation bomb.
- **C1c** `:53` — `let platform = "x86_64-unknown-linux-gnu"; // Auto-detect in real impl` — hardcoded platform, admitted stub.

**Fix:** require an Ed25519/minisign signature or pinned SHA-256 manifest; parse the version with a semver domain type before URL construction; cap preallocation (e.g. 512 MiB) and grow streaming; derive platform from `env::consts::ARCH`.

### C2 · CONFIRMED — Daemon deserialized-size guard is dead code
`src/daemon/server.rs:404-419`

`std::mem::size_of_val(&request)` measures the enum's *stack* size (hundreds of bytes); heap `String`/`Vec` payloads are invisible, so the 10 MB `MAX_DESERIALIZED_SIZE` check can never fire. Effective mitigations that do work: 1 MB codec frame cap (`:368-370`) and batch-depth cap (`:327-344`).

**Fix:** delete the dead check or compute payload size recursively; document the real wire limit.

### C3 · NEW — Vulnerable h2 on the network path
`Cargo.lock` (reqwest 0.13.2 → h2 0.4.13) · RUSTSEC-2026-0258 (DoS via unbounded empty DATA frames)

**Fix:** `cargo update -p h2` (≥0.4.16); keep the `audit.yml` CI gate green.

---

## High

### H1 · CHANGED — Telemetry honors opt-out, but scope & persistence are the problem
`src/core/telemetry.rs`, `telemetry_client.rs`, `analytics.rs`, `usage.rs`, `src/cli/telemetry.rs` · 10-rust §11

Verified fixed since v1: install ping gated at `src/bin/omg.rs:623`; every analytics entry point checks `is_enabled()` (`analytics.rs:282-284` and all `track_*`/`flush*`); usage sync is license-gated and hashes the hostname (`usage.rs:398-418`). Remaining issues:

1. Default-ON (opt-out only) analytics transmit `machine_id`, `license_key`, kernel, TZ/locale, and the **full `installed_packages` list** (`usage.rs:426`) — contradicts "Anonymous data collection (no personal information)" (`telemetry.rs:5`).
2. **`license_key` is persisted to disk in every queued event** (`analytics.rs:97-107`, queue save `:259-265`, `~/.local/share/omg/event_queue.json`) — credential at rest, retried up to 5×.
3. `track_error`/`CommandEvent.error` ship raw error strings (may embed user paths).
4. Five parallel telemetry modules — consolidation debt.

**Fix:** drop `license_key` from event payloads (server joins on `machine_id`); chmod 0600 telemetry state files; reword docs; merge modules.

### H2 · NEW — Broken `target` symlink blocks all local builds
`target -> .tmp/target-arch-check` (dangling; leftover from an Aug 15 arch check)

Every `cargo build/clippy/test` fails with `Not a directory (os error 20)`.

**Fix:** remove the symlink. (Audit stayed read-only; deletion needs owner approval.)

### H3 · NEW — CI lint gate skips the shipped feature set; tree currently red
`.github/workflows/ci.yml`; `Cargo.toml:343-356`

- CI clippy runs `-D warnings` only with `--no-default-features --features pgp,license`; the default `arch` build (39 warnings today) is ungated.
- `cargo fmt --check` fails on 5 files right now → CI red or skipped (`ci_failure.log` sits in the repo root).
- `unwrap_used`/`expect_used` lints are disabled (`Cargo.toml:355` "too noisy") yet ~50 `#[expect(clippy::expect_used/unwrap_used)]` annotations remain; 2 already warn "unfulfilled".

**Fix:** add a default-features clippy job; fix the 5 fmt files; `cargo clippy --fix`; either re-enable the lints with a `#[cfg(test)]`-scoped allow or strip the stale `#[expect]`s.

### H4 · CONFIRMED — 57 `#[ignore]` tests across 9 files
`src/package_managers/pacman_db/db.rs`, `homebrew.rs`, `tests/{integration/security_real_world,integration_suite,docker_e2e,install_update_comprehensive,fedora_tests,macos_tests,windows_tests}.rs`

Reasons are documented (network/system/platform), but no CI path runs them automatically, and `pacman_db/db.rs:1700` "may fail if db is corrupted" is a non-reason.

**Fix:** scheduled per-platform job running `--ignored`; tighten weak reasons.

---

## Medium

| ID | Tag | Finding | Location |
|---|---|---|---|
| M5 | NEW | `elevate_if_needed` creates a nested tokio runtime (`Runtime::new().block_on`, `:197-199`) — panics if ever called from async; `with_root`'s `unreachable!()` (`:419`) is reachable in `#[cfg(test)]` builds (elevate returns `Ok(())` there); doc promises `Ok(bool)` it doesn't return | `src/core/privilege.rs:166-205,407-422` |
| M6 | CONFIRMED | `sh -c` executes workspace-config command strings (user-trusted config, but injection-shaped surface, no env hygiene) | `src/cli/workspace.rs:422-427` |
| M7 | NEW | ~20 `.expect("lock poisoned")` panics on mutex poisoning; `dnf.rs` panics when `$HOME` unset — expected conditions as panics. Use `unwrap_or_else(PoisonError::into_inner)` (already used elsewhere in repo) | `homebrew.rs:451-466`, `dnf.rs:425-723`, `debian_db/transaction.rs` |
| M8 | NEW | All 3 daemon spawns drop the `Child` handle (no reaping/ownership); `init.rs:909` spawns `omg` via PATH (hijack vector) while others prefer sibling-exe pattern | `cli/commands.rs:753-759`, `cli/init.rs:909-912`, `core/client.rs:293-298` |
| M9 | NEW | Duplicate-dep bloat (getrandom/hashbrown ×3 …) + yanked `spin` — compile-time/binary cost | `Cargo.lock` |
| M10 | NEW | Telemetry/session state files with weak perms & long retention (queue 1000–5000 events incl. `license_key`) | `analytics.rs:24-26`, `telemetry.rs:36-38` |
| M11 | NEW | **101 placeholder-family comments**: admitted stubs at `packages/status.rs:179` ("just a placeholder"), `tea/update_model.rs:187`, `tea/mod.rs:262-298` (×5), `enterprise.rs:502` (proxy metrics), `doctor.rs:367`, `bin/omg-fast.rs:142`, `self_update.rs:53` | across `src/` |
| M12 | NEW | 39 clippy warnings + 5 fmt diffs + ~50 stale `#[expect]` annotations for disabled lints (details in H3) | repo-wide |
| M13 | NEW | `println!` in 16 library-layer modules (core/PM/daemon/runtimes) — UI/domain separation | e.g. `task_runner.rs`, `aur/client.rs`, `parallel_sync.rs` |
| M14 | CONFIRMED⁺ | Root/repo debris: `ci_failure.log`, `update_debug.log`, screenshot PNG, **3 lockfiles** (`Cargo.lock` + `package-lock.json` + `bun.lock`), 6 Dockerfiles, 3 benchmark dirs (`benches/`, `benchmarks/`, `benchmark_results/`), empty `exports/`, `site/node_modules`, `site/FIX_TELEMETRY_NOW.md`, `conductor/` (16 tracked files, purpose unclear), `.worktrees/`, `.opencode/`, `.sisyphus/`, `.ui-design/`, `tasks/`, `deploy-tui/` | repo root |

## Low / observations

- `server.rs:374-375` const `NonZeroU32::new(...).unwrap()` (works, brittle idiom); 69 narrowing numeric casts; release-profile comment claims `catch_unwind` needs `panic="unwind"` (usage not verified); stray `--` arg in daemon spawns; 55 global `static`s (acceptable for CLI caching); anyhow-dominant error style (122 files) with thiserror in only 12 — acceptable for a CLI, note for domain modules.
- `aur/client.rs:2676` `sh -c` is test-only (sandbox escape test) — not a prod shell site.

## Checked and clean (coverage credit)

- `core/http.rs`: documented `# Panics` expects, no TLS weakening, sane timeouts.
- `secrets.rs`: all 19 expects are justified `LazyLock<Regex>` statics with comments.
- No lock-held-across-`.await` violations found (repo-wide scan).
- Unsafe surface minimal & annotated: `memmap2` mmaps + test-only `set_var`, 11 scoped `#[expect(unsafe_code)]` punches, 20 SAFETY comments.
- `allow(dead_code)` only 5 sites; `#[allow]`/`#[expect]` inventory far cleaner than v1's raw count.
- **Prod panic surface is far smaller than v1's raw counts**: 808 unwrap/expect hits are ~95% test-module code (`task_runner` 49→0 prod, `runtimes/common` 46→0, `content_store` 37→0, `pgp` 28→0, `hooks/mod.rs` 48→0, `completion` 18→0, `tea/renderer` 15→0, …). Remaining prod sites: justified statics (http/secrets), lock-poisoned expects (M7), dnf `$HOME` (M7), resolver invariants (defensible), `privilege.rs` `unreachable!()` (M5).
- Telemetry opt-out plumbing is thorough (checked every entry point).
- `.cargo/audit.toml` and `deny.toml` ignores are documented with rationale.
- CI: 11 workflows incl. codeql, mutation, coverage, docker-e2e.

## `src/hooks/mod.rs` WIP verdict

**Keep and finish.** The uncommitted diff converts silent `.ok()?` failure-swallowing into contextual `Result` errors (NotFound→None, other IO errors propagate with path context) and adds two tests including a fail-closed unreadable-directory test. This is exactly the direction this audit recommends; do not revert.

## Recommended fix order

1. **H2** — remove broken `target` symlink (unblocks all builds)
2. **C3** — `cargo update -p h2` (one command)
3. **C1** — signature/manifest verification for self-update
4. **H3/M12** — CI default-features clippy job + fmt fixes + strip stale `#[expect]`s
5. **H1/M10** — drop `license_key` from payloads, 0600 state files
6. **M5, M7, M8** — privilege runtime fix, poison handling, spawn ownership
7. Remainder opportunistically; M11/M13/M14 during normal touch-the-file work

## Coverage & limitations

Deep-read: `self_update`, `daemon/server`, `privilege`, `secrets`, `telemetry`×2, `analytics`, `http`, `usage(sync)`, `workspace`, `bin/omg`, sections of `commands`/`init`/`client`/`debian_db`/`hooks`. Exhaustive greps across all 180 files (panics, unsafe, spawns, shells, egress, locks, slop lexicon, attributes, ignores, deps, CI, hygiene). **Not line-by-line read:** ~60k LOC of PM backends/TUI/tea/runtimes — covered by greps + clippy only. **No tests were run** (per agreement; note they cannot run until H2 is fixed anyway).


