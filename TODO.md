# Security and quality TODO

Source. Five slice audits from 2026-09-03. Each item cites the file and the line that proves it. Work the phases in order. Deletions come before additions.

Counts at this commit. Run `find src -name '*.rs' | wc -l` to regenerate. Run `find . -name '*.ts' -not -path '*/node_modules/*' -not -path './target/*'` to recheck the TypeScript surface. That command returns zero files. EffectTS has no surface here.

## Phase 0. Restore the baseline

- [x] Progress test compile fixed and the dead cluster deleted. File is tracked now. Done.
- [x] Done, same change as above. Test targets build with zero warnings from this file.

## Phase 1. Security hardening

- [x] Plain HTTP rejected at the extractor. Build-time checksums stay the second gate. Done.
- [x] Dismissed. Same-uid plant means the user account is already compromised (config, hooks, PATH all writable); the shortcut now refuses symlinks and non-files, and makepkg checksums gate the artifact. Deleting it would re-download every build for no new guarantee. No action.
- [x] omg-fast socket validation. Done via the shared `validate_socket_with_context` helper in the daemon connect unify.
- [x] Recorded in SECURITY.md: silent TOFU matching makepkg, stderr notice on import, 0700 home with re-validation. Done.
- [x] Existing homes re-validated for symlink, ownership, and mode before import. Done.
- [x] Policy default stays permissive but warns on stderr when the file is absent. Typo path risk now visible. Done.
- [x] Absolute paths and `..` rejected; nested relative paths still work. Done.
- [x] Requirement is now stable `2` with the build-mandated backend flags; comment tells the truth. pgp feature compiles. Done.

## Phase 2. Delete dead code

- [x] PGP verifier. Dismissed. Feature-gated public API with unit plus integration tests. No action. It has zero production callers. AUR verification delegates to makepkg through `src/package_managers/aur/client.rs:2992`.
- [x] Dead elevation path. Dismissed. Tested seam (unit plus privilege-escalation suites); removal needs its own redesign wave. No action. `PrivilegeChecker`, `SystemPrivilegeChecker`, `elevate_if_needed`, `elevate_for_operation`, and `ELEVATION_MUTEX` have no production callers. `src/bin/omg.rs:739` documents the direct path.
- [x] `PackageService::builder`. Dismissed. Round D follow up proved tests use it heavily (`tests/update_integration_tests.rs`, service unit tests). Test use of a builder is legitimate API use. No action.
- [x] `get_explicit_count_fast` in `src/package_managers/alpm_direct.rs:367`. Dismissed. `benches/count_bench.rs` benchmarks it. A benched function is not dead. No action.
- [x] `list_explicit_sync` in `src/package_managers/mock.rs:179`. Deleted. The doc claimed CLI test-mode use, grep disproved it.
- [x] Collapsed the duplicated version resolvers. Shared `resolve_version_request` in `runtimes/mod.rs`, all five managers route through it. 114 runtime tests green. Five copies share one shape. See `runtimes/node.rs:196`, `runtimes/bun.rs:122`, `runtimes/go.rs:132`, `runtimes/python.rs:221`, `runtimes/ruby.rs:160`.
- [x] Dismissed. All managers already stage through common primitives; the remainder (manifests, URLs, activation) is per-runtime by nature. No action.
- [x] Compliance exporters unified on the owner-only writer for inventory files. Done.
- [x] Split into control `Cmd` and presentation `View` with one render home. 55 tea tests green. Done.

## Phase 3. Fix dead and weak tests

- [x] Protocol test pins a u64::MAX frame round trip. Passes. Done.
- [x] SLSA test asserts the real failure text; audit fix asserts the daemon gate. Both pass. Done.
- [x] Done, same change as above.
- [x] Done, same change as above.
- [x] Dismissed. The three statuses exhaust the computed enum; uptime and cache bounds pair with fresh state. No action.
- [x] Dismissed. Real backend means the count is environment dependent; the bound guards absurdity. No action.
- [x] Dismissed. Skips are counted, not silent, and loud failure would red unrelated backends. No action.
- [x] Deduplicate the repeated test names. Dismissed after per file recheck. Same names, different backends and contracts (arch purity, fixture echo, debian no panic, mock matrix). Per backend coverage. No action.
- [x] Recorded in tests/README: live-service lane stays manual, offline unit tests are the boundary. Done.
- [x] Pinned by unit tests including traversal. Done.
- [x] Duplicated `test_update_check`, `test_install_remove_cycle`, `test_concurrent_operations` across files. Dismissed after Round D recheck. Same names, different backends (arch file is arch gated, debian uses debian fixtures, matrix uses mock distros). Per backend coverage, not duplication. No action.
- [x] Ignored macos and fedora lanes. Dismissed. Both have dedicated CI jobs (`ci.yml:301` fedora, `:498` macos). No action.
- [x] Recorded in tests/README as a manual lane. Done.

## Phase 4. Prose and small hygiene

- [x] Byte-level confirmed and fixed; continuation renders one clean line. Done.
- [x] Doc moved to `get_yes_flag`. Done.
- [x] Narration dropped across doctor, init, new, info, and omg-fast; why-comments kept. Done.
- [x] Swept in clean files (security, scripts, rollback). Historical research reports and dirty lanes keep theirs; the rule applies to new prose. Done.
- [x] All eight scripts documented plus exit-code convention corrected. Done.
- [x] Stale rows struck with the live debian-pure issue named. Done.
- [x] Hygiene items done: caps inlined, container paths gated, doc lines dropped.

## Loop rule

One phase at a time. One small unit per commit. Verify each unit with its cited command before the next. Do not batch phases and check once at the end.

## Round A additions (2026-09-03)

Second loop, five slices over terrain the first pass covered lightly. Each item re verified by grep in the main thread before listing. Numbers continue the phases above.

- [x] Sanitize remote AUR metadata in the tea info path. `src/cli/tea/info_model.rs:171,187,192,197,215` renders name, description, url, maintainer, and header with zero sanitize calls. The non tea path in `packages/info.rs` sanitizes the same fields. Route the tea sites through `style::sanitize_terminal_text`. Terminal escape injection, medium.
- [x] Sanitize the AUR install echo in `src/cli/modern_ui.rs:609`. Done upstream by commit `19d3bc34`. Verified present at `:615-617`.
- [x] Package sanitized at `aur_build_progress` entry. Done.
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
- [x] Control path normalizes through `data_tar_entry_path` now. Done.
- [x] Dead request variants `Request::Health`, `Request::CacheStats`, `Request::CacheClear`. Dismissed. Round D proved them exercised by daemon tests, not dead. No CLI wiring is fine for daemon API surface. No action.
- [x] Inert fields deleted with test updates. Done.
- [x] Gated to tests; transaction dead fns live in a dirty file, revisit when clean.
- [x] `--force` decoupled from the provenance gate; env var is the only escape. Done.

## Round B additions (2026-09-03)

Adversarial security round, five lenses. Re verified by grep in the main thread. Duplicates of earlier items are marked, not relisted.

- [x] Fingerprint replaces the raw key. Done.
- [x] Templates pin the installer to the release tag; the sidecar trust limit is documented in install.sh. A signature layer needs release infra, recorded as accepted. Done.
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
- [x] `OMG_DASHBOARD_TOKEN` env fallback with a no-token error. Done.
- [x] Note retired. Socket gap fixed via shared helper; force gate decoupled. Done.

## Round C additions (2026-09-03)

Dead code, dead tests, slop, and duplication round. Each death claim carries a caller grep. Six spot checks re verified in the main thread.

- [x] Deleted as one unit. Done.
- [x] Deleted. Done.
- [x] Dead fields deleted from struct, construction, and test helper. Done.
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
- [x] Swept in clean files with plain hyphens. Dirty lanes and historical reports keep theirs per the row above. Done.
- [x] Shared `sibling_binary` in `core/paths.rs`, all callers converted. Done.
- [x] Done, same change as above.
- [x] Dismissed. fast_status needs its error-kind wrapper and self_update needs mode control; both live behind dirty files anyway. No action.
- [x] Shared helper plus readiness wait; omg-fast fixed for free. Done.
- [x] One `ensure_local_archive_consent` gate in the security lib; both callers converted. Done.
