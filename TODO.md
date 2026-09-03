# Security and quality TODO

Source. Five slice audits from 2026-09-03. Each item cites the file and the line that proves it. Work the phases in order. Deletions come before additions.

Counts at this commit. Run `find src -name '*.rs' | wc -l` to regenerate. Run `find . -name '*.ts' -not -path '*/node_modules/*' -not -path './target/*'` to recheck the TypeScript surface. That command returns zero files. EffectTS has no surface here.

## Phase 0. Restore the baseline

- [ ] Test target compile error in untracked `src/cli/progress.rs:383`. `bar.message().as_deref()` on a `String`. Lib target is clean, verified by `cargo check --lib` with zero errors. The test target fails only on this line. File is untracked and owned by another lane. Do not fix here, flag the owner.
- [ ] Delete the dead progress cluster in `src/cli/progress.rs`. `Accent::Aur` is never constructed. `PENDING_BYTES_FIGURES` is never used. `set_label` is never used. `lock_label` goes with it. Refresh or drop the three stale `#[expect]` attributes on the literal formatting lints. Verify with `cargo clippy --workspace --all-targets`.

## Phase 1. Security hardening

- [ ] Force HTTPS for AUR source fetch in `src/package_managers/aur_sources.rs:77`. Store a hash of fetched bytes at `:222`. Makepkg rechecks at build time. This restores defense in depth.
- [ ] Remove the cached file shortcut in `src/package_managers/aur_sources.rs:175`. A same uid planted file wins the fast path today. Fetch and verify instead.
- [ ] Route `omg-fast` through the shared connect path in `src/bin/omg-fast.rs:153`. It skips `validate_socket_parent` today. Other clients enforce it.
- [ ] Decide the keyserver trust question in `src/core/security/keyserver.rs:337`. Keys import on first sight with no fingerprint prompt. Either prompt once per fingerprint or record the silent TOFU choice in `SECURITY.md`.
- [ ] Recheck the `.gnupg` creation mode in `src/core/security/keyserver.rs:343`. The dir is created with the default mode and chmodded after. Close the window or revalidate an existing dir before import.
- [x] Policy default stays permissive but warns on stderr when the file is absent. Typo path risk now visible. Done.
- [ ] Validate the scaffold name in `src/cli/new.rs:27`. Leading `-` now rejected there. Path separators and interior `..` still pass via `is_safe_package_char` in `src/core/security/validation.rs:329`. Close that half. Match the discipline in `src/core/security/validation.rs:79`.
- [ ] Decide the sequoia pin in `Cargo.toml:93` and `Cargo.toml:250`. The comment claims a PQC prerelease. The lock resolves stable `2.4.1`. Either pin exact with `=` or rewrite the comment. Verify with `cargo tree --locked -p sequoia-openpgp`.

## Phase 2. Delete dead code

- [x] PGP verifier. Dismissed. Feature-gated public API with unit plus integration tests. No action. It has zero production callers. AUR verification delegates to makepkg through `src/package_managers/aur/client.rs:2992`.
- [x] Dead elevation path. Dismissed. Tested seam (unit plus privilege-escalation suites); removal needs its own redesign wave. No action. `PrivilegeChecker`, `SystemPrivilegeChecker`, `elevate_if_needed`, `elevate_for_operation`, and `ELEVATION_MUTEX` have no production callers. `src/bin/omg.rs:739` documents the direct path.
- [x] `PackageService::builder`. Dismissed. Round D follow up proved tests use it heavily (`tests/update_integration_tests.rs`, service unit tests). Test use of a builder is legitimate API use. No action.
- [x] `get_explicit_count_fast` in `src/package_managers/alpm_direct.rs:367`. Dismissed. `benches/count_bench.rs` benchmarks it. A benched function is not dead. No action.
- [x] `list_explicit_sync` in `src/package_managers/mock.rs:179`. Deleted. The doc claimed CLI test-mode use, grep disproved it.
- [x] Collapsed the duplicated version resolvers. Shared `resolve_version_request` in `runtimes/mod.rs`, all five managers route through it. 114 runtime tests green. Five copies share one shape. See `runtimes/node.rs:196`, `runtimes/bun.rs:122`, `runtimes/go.rs:132`, `runtimes/python.rs:221`, `runtimes/ruby.rs:160`.
- [ ] Collapse the staged install copies into `runtimes/common.rs`. Blocked. Shared home lives in dirty `runtimes/common.rs`. Revisit when clean. Each runtime reimplements download, extract, and publish. Start at `runtimes/node.rs:148`.
- [x] Compliance exporters unified on the owner-only writer for inventory files. Done.
- [ ] Split the `Cmd` enum in `src/cli/tea/cmd.rs:18`. Deferred. Redesign with dispatch callers, not a deletion. Needs its own wave. Control flow variants and presentation variants live in one enum. Dispatch repeats in `packages/mod.rs:83` and the tea runtime.

## Phase 3. Fix dead and weak tests

- [x] Protocol test pins a u64::MAX frame round trip. Passes. Done.
- [x] SLSA test asserts the real failure text; audit fix asserts the daemon gate. Both pass. Done.
- [x] Done, same change as above.
- [x] Done, same change as above.
- [x] Dismissed. The three statuses exhaust the computed enum; uptime and cache bounds pair with fresh state. No action.
- [x] Dismissed. Real backend means the count is environment dependent; the bound guards absurdity. No action.
- [x] Dismissed. Skips are counted, not silent, and loud failure would red unrelated backends. No action.
- [x] Deduplicate the repeated test names. Dismissed after per file recheck. Same names, different backends and contracts (arch purity, fixture echo, debian no panic, mock matrix). Per backend coverage. No action.
- [ ] Cover `keyserver.rs` from integration tests or record why inline unit tests are the boundary. Zero files in `tests/` touch it today.
- [ ] Cover `validate_image_ref` in `src/core/security/validation.rs:147` or record the boundary. Zero files in `tests/` touch it.
- [x] Duplicated `test_update_check`, `test_install_remove_cycle`, `test_concurrent_operations` across files. Dismissed after Round D recheck. Same names, different backends (arch file is arch gated, debian uses debian fixtures, matrix uses mock distros). Per backend coverage, not duplication. No action.
- [x] Ignored macos and fedora lanes. Dismissed. Both have dedicated CI jobs (`ci.yml:301` fedora, `:498` macos). No action.
- [ ] Decide the ignored lane in `tests/integration/security_real_world.rs:20`. All five real world security tests carry ignore. Either run them in CI or state the lane is manual.

## Phase 4. Prose and small hygiene

- [x] Byte-level confirmed and fixed; continuation renders one clean line. Done.
- [x] Doc moved to `get_yes_flag`. Done.
- [x] Narration dropped across doctor, init, new, info, and omg-fast; why-comments kept. Done.
- [ ] Sweep the banned dash character from docs and comments. Start with `A4-UPSTREAM-ALPM-RESEARCH-REPORT.md`, `FEDORA-ENGINE.md`, `SECURITY.md`, `CONTRIBUTING.md`, `WAVE12-BLOCKERS.md`, `scripts/README.md`, `src/bin/omg.rs`, `src/cli/doctor.rs`, `src/cli/security.rs`.
- [x] All eight scripts documented plus exit-code convention corrected. Done.
- [x] Stale rows struck with the live debian-pure issue named. Done.
- [x] Hygiene items done: caps inlined, container paths gated, doc lines dropped.

## Loop rule

One phase at a time. One small unit per commit. Verify each unit with its cited command before the next. Do not batch phases and check once at the end.

## Round A additions (2026-09-03)

Second loop, five slices over terrain the first pass covered lightly. Each item re verified by grep in the main thread before listing. Numbers continue the phases above.

- [x] Sanitize remote AUR metadata in the tea info path. `src/cli/tea/info_model.rs:171,187,192,197,215` renders name, description, url, maintainer, and header with zero sanitize calls. The non tea path in `packages/info.rs` sanitizes the same fields. Route the tea sites through `style::sanitize_terminal_text`. Terminal escape injection, medium.
- [x] Sanitize the AUR install echo in `src/cli/modern_ui.rs:609`. Done upstream by commit `19d3bc34`. Verified present at `:615-617`.
- [ ] Sanitize the AUR progress prefix fed from `src/package_managers/aur/client.rs:2838` into `src/cli/modern_ui.rs:240`. Blocked. File is dirty in another lane. Revisit when clean.
- [x] Replace the TUI sanitizer in `src/cli/tui/ui.rs:23`. It strips control chars only. Bidi overrides pass through. Delegate to `style::sanitize_terminal_text` and sanitize `pkg.name` and `pkg.version` at `ui.rs:827,837`. Medium.
- [x] Sanitize manifest echo in `src/cli/migrate.rs:124,129,194`. Manifest strings arrive from another machine and print raw. Execution is validated, display is not. Medium.
- [x] Extractors take ownership, zero clones. Done.
- [x] omg-fast validates the socket parent via the shared helper. Done with the daemon connect unify.
- [x] Config refuses symlinks and non-files. Done.
- [x] Workspace custom commands print and ask consent in attended terminals; nested runs use the sibling binary. Done.
- [x] Team hooks embed the running executable path. Done.
- [x] Consent prompt plus real error reporting. Done.
- [x] Check added and pinned by test. Done.
- [x] Traversal filenames rejected before fetch. Done.
- [ ] Unify control.tar extraction with the data.tar hardening in `src/package_managers/debian_db/transaction.rs:1683`. Control path relies on crate defaults. Low.
- [x] Dead request variants `Request::Health`, `Request::CacheStats`, `Request::CacheClear`. Dismissed. Round D proved them exercised by daemon tests, not dead. No CLI wiring is fine for daemon API surface. No action.
- [x] Inert fields deleted with test updates. Done.
- [x] Gated to tests; transaction dead fns live in a dirty file, revisit when clean.
- [ ] Fix the `--force` provenance bypass in `src/cli/self_update.rs:140`. Force doubles as the unverified provenance escape while docs name only the env var. Document or separate. Low.

## Round B additions (2026-09-03)

Adversarial security round, five lenses. Re verified by grep in the main thread. Duplicates of earlier items are marked, not relisted.

- [x] Fingerprint replaces the raw key. Done.
- [ ] Pin the installer bootstrap. `install.sh:8` pipes mutable `main` with no script checksum, and the binary verifies by same origin sha256 sidecar only at `install.sh:330`. Add a version pin plus a signature or attestation check. Medium.
- [x] Templates pin the installer to the release tag. Done.
- [x] Lifecycle scripts disabled. Version pins need chosen versions, left as the decision half of this row.
- [x] Gist upload reads through the guarded reader. Done.
- [x] Dismissed. 16-char long IDs are standard in Arch `validpgpkeys`; rejecting them breaks legitimate builds. The HKPS transport plus ID match stays the boundary. No action.
- [x] Tag-only pulls now warn on stderr pointing at digest pinning. Mandating digests would break tag users; warning is the reversible step. Done.
- [x] Sibling `omg` resolved via shared helper. Done with the unify wave below.
- [x] Shared allowlists route both kinds. Done.
- [x] Container env key parsing. Resolved upstream by commit 6e9516e2. Parsing lives in `src/cli/container.rs:16`, not lossy, rejects empty keys, tested at `:517`. No action.
- [x] Chown user prefers pwd lookup over `USER` env. Done.
- [x] Done via shared `sibling_binary` helper.
- [x] Symlink refusal plus private lock mode. Done.
- [x] Same fix. Done.
- [x] Reads refuse symlinks; create claims via link. Done.
- [x] One `PRIVILEGED_ENV_SCRUB` list plus helper, all three sudo sites. Done.
- [ ] Accept a dashboard token outside argv in `src/cli/args.rs:1012`. Argv leaks into history and process list. Add env or prompt input. Low.
- [ ] Note. omg-fast socket gap and self update force gate confirmed again this round, still unfixed. The force item escalates to medium. No new rows, fix the existing ones.

## Round C additions (2026-09-03)

Dead code, dead tests, slop, and duplication round. Each death claim carries a caller grep. Six spot checks re verified in the main thread.

- [x] Deleted as one unit. Done.
- [x] Deleted. Done.
- [ ] Delete or read `CliContext` fields `verbose`, `quiet`, `no_color` in `src/cli/mod.rs:114`. Written once, never read. Only `ctx.json` is consumed.
- [x] Tier feature API (`current_tier`, `has_feature`, `require_feature`, `features_for_tier`) in `src/core/license.rs`. Dismissed. Public lib surface pinned by integration tests in `tests/coverage_11.rs` and `tests/e2e_tests.rs`. Test only use does not prove dead public API. No action.
- [x] Gated to tests. Done.
- [x] Deleted with both orphaned helper copies. Done.
- [x] Variant deleted; mismatch stays Ok(false) by documented design. Done.
- [x] Dismissed. Feature-gated public API pinned by unit and integration tests, like the tier API. No action.
- [x] Gated to tests. Done.
- [x] Empty prepare asserts success; version comparisons pin determinism. Done.
- [x] Unified on the tier plus pricing marker pattern. Done.
- [x] Dismissed. Every site already pairs the output check with a 101 panic-code assert. No action.
- [x] True dup deleted, unicode test made real, invalid-command dup deleted. Done.
- [x] Shared `DaemonTestFixture`, both files converted, suites green. Done.
- [x] Clause fixed. Done.
- [x] Describes concurrent search now. Done.
- [x] Queue sentence removed. Done.
- [x] Mechanical pass over code and docs. Done.
- [ ] Sweep en dashes from `src/package_managers/aur/client.rs:111` and the docs ranges. Encode as a CI grep with a narrow token list so it cannot recur.
- [x] Shared `sibling_binary` in `core/paths.rs`, all callers converted. Done.
- [x] Done, same change as above.
- [x] Dismissed. fast_status needs its error-kind wrapper and self_update needs mode control; both live behind dirty files anyway. No action.
- [x] Shared helper plus readiness wait; omg-fast fixed for free. Done.
- [ ] Unify install consent and backend dispatch with new lib fns in `core/security/mod.rs`. Twins at `src/bin/omg.rs:134,779` and `src/cli/packages/install.rs:119,155` already diverge on debian local files. Start with the consent gate before the dispatch refactor.
