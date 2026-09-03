# Security and quality TODO

Source. Five slice audits from 2026-09-03. Each item cites the file and the line that proves it. Work the phases in order. Deletions come before additions.

Counts at this commit. Run `find src -name '*.rs' | wc -l` to regenerate. Run `find . -name '*.ts' -not -path '*/node_modules/*' -not -path './target/*'` to recheck the TypeScript surface. That command returns zero files. EffectTS has no surface here.

## Phase 0. Restore the baseline

- [x] Test target compile error in `src/cli/progress.rs`. Resolved in tree. Round D ran `cargo check --lib`, zero errors. Only 4 dead code warnings remain, covered by the next item.
- [ ] Delete the dead progress cluster in `src/cli/progress.rs`. `Accent::Aur` is never constructed. `PENDING_BYTES_FIGURES` is never used. `set_label` is never used. `lock_label` goes with it. Refresh or drop the three stale `#[expect]` attributes on the literal formatting lints. Verify with `cargo clippy --workspace --all-targets`.

## Phase 1. Security hardening

- [ ] Force HTTPS for AUR source fetch in `src/package_managers/aur_sources.rs:77`. Store a hash of fetched bytes at `:222`. Makepkg rechecks at build time. This restores defense in depth.
- [ ] Remove the cached file shortcut in `src/package_managers/aur_sources.rs:175`. A same uid planted file wins the fast path today. Fetch and verify instead.
- [ ] Route `omg-fast` through the shared connect path in `src/bin/omg-fast.rs:153`. It skips `validate_socket_parent` today. Other clients enforce it.
- [ ] Decide the keyserver trust question in `src/core/security/keyserver.rs:337`. Keys import on first sight with no fingerprint prompt. Either prompt once per fingerprint or record the silent TOFU choice in `SECURITY.md`.
- [ ] Recheck the `.gnupg` creation mode in `src/core/security/keyserver.rs:343`. The dir is created with the default mode and chmodded after. Close the window or revalidate an existing dir before import.
- [ ] Decide the policy default in `src/core/security/policy.rs:96`. A missing file yields the permissive default. Either keep it and document the typo path risk or fail closed.
- [ ] Validate the scaffold name in `src/cli/new.rs:27`. Leading `-` now rejected there. Path separators and interior `..` still pass via `is_safe_package_char` in `src/core/security/validation.rs:329`. Close that half. Match the discipline in `src/core/security/validation.rs:79`.
- [ ] Decide the sequoia pin in `Cargo.toml:93` and `Cargo.toml:250`. The comment claims a PQC prerelease. The lock resolves stable `2.4.1`. Either pin exact with `=` or rewrite the comment. Verify with `cargo tree --locked -p sequoia-openpgp`.

## Phase 2. Delete dead code

- [ ] Delete or wire up `PgpVerifier` in `src/core/security/pgp.rs:1`. It has zero production callers. AUR verification delegates to makepkg through `src/package_managers/aur/client.rs:2992`.
- [ ] Delete or wire up the dead elevation path in `src/core/privilege.rs:165`. `PrivilegeChecker`, `SystemPrivilegeChecker`, `elevate_if_needed`, `elevate_for_operation`, and `ELEVATION_MUTEX` have no production callers. `src/bin/omg.rs:739` documents the direct path.
- [ ] Delete `PackageServiceBuilder` in `src/core/packages/service.rs:31`. It has zero callers outside its file.
- [ ] Delete `get_explicit_count_fast` in `src/package_managers/alpm_direct.rs:367`. Grep shows no callers in `src/`.
- [ ] Delete `list_explicit_sync` in `src/package_managers/mock.rs:179`. It has no callers.
- [ ] Collapse the duplicated version resolvers into `runtimes/common.rs`. Five copies share one shape. See `runtimes/node.rs:196`, `runtimes/bun.rs:122`, `runtimes/go.rs:132`, `runtimes/python.rs:221`, `runtimes/ruby.rs:160`.
- [ ] Collapse the staged install copies into `runtimes/common.rs`. Each runtime reimplements download, extract, and publish. Start at `runtimes/node.rs:148`.
- [ ] Unify the two compliance exporters. `src/cli/enterprise.rs:84` and `src/cli/security.rs:1209` share one arg shape and diverge on file sets and permissions. Use `write_private_export` for inventory bearing files.
- [ ] Split the `Cmd` enum in `src/cli/tea/cmd.rs:18`. Control flow variants and presentation variants live in one enum. Dispatch repeats in `packages/mod.rs:83` and the tea runtime.

## Phase 3. Fix dead and weak tests

- [ ] Rewrite the vacuous assertion in `tests/tdd_edge_cases.rs:48`. It builds a value and asserts the getter echoes it. Assert a real protocol boundary or delete the test.
- [ ] Replace the OR of negations in `tests/coverage_2.rs:68`. It passes on almost any output. Assert the expected behavior with a positive marker.
- [ ] Tighten the SLSA disjunction in `tests/coverage_2.rs:36`. One exact marker string replaces three loose ones.
- [ ] Tighten the flag contract test in `tests/coverage_2.rs:43`. Check the exit status or the effect. Refusal text alone lets silent accept pass.
- [ ] Tighten the health bounds in `tests/daemon_security_tests.rs:141`. Three accepted statuses and wide numeric bounds let a broken daemon pass.
- [ ] Replace the arbitrary count bound in `tests/daemon_security_tests.rs:242`. Assert the exact fresh state value.
- [ ] Make setup failure loud in `tests/daemon_security_tests.rs:35`. The skip counter mitigates the silent return. Security tests deserve a hard fail option.
- [ ] Deduplicate the repeated test names. `test_info_nonexistent_package` exists in five files. `test_update_check`, `test_install_remove_cycle`, `test_invalid_command`, and `test_concurrent_operations` repeat. Move them behind one helper in `tests/common/`.
- [ ] Cover `keyserver.rs` from integration tests or record why inline unit tests are the boundary. Zero files in `tests/` touch it today.
- [ ] Cover `validate_image_ref` in `src/core/security/validation.rs:147` or record the boundary. Zero files in `tests/` touch it.
- [ ] Decide the ignored lane in `tests/integration/security_real_world.rs:20`. All five real world security tests carry ignore. Either run them in CI or state the lane is manual.

## Phase 4. Prose and small hygiene

- [ ] Fix the malformed error string in `src/core/privilege.rs:116`. It emits a literal backslash in the user message.
- [ ] Reattach the misplaced doc block in `src/core/privilege.rs:62`. It documents a different function than the one below it.
- [ ] Remove narrating Step comments in `src/cli/doctor.rs:610`, `src/cli/init.rs:230`, `src/cli/new.rs:56`, `src/cli/packages/info.rs:70`, and `src/bin/omg-fast.rs:66`. Keep the why comments that cite a real reason.
- [ ] Sweep the banned dash character from docs and comments. Start with `A4-UPSTREAM-ALPM-RESEARCH-REPORT.md`, `FEDORA-ENGINE.md`, `SECURITY.md`, `CONTRIBUTING.md`, `WAVE12-BLOCKERS.md`, `scripts/README.md`, `src/bin/omg.rs`, `src/cli/doctor.rs`, `src/cli/security.rs`.
- [ ] Document the missing scripts in `scripts/README.md:9`. It lists three of eight. Add `collect-release-artifacts.sh`, `debian-smoke-test.sh`, `gen-release-notes.sh`, and `r2-rollback.sh`.
- [ ] Delete the stale tech debt claim in `docs/TECH-DEBT-REVIEW-2026-08-31.md:18` after confirming with `cargo check --features debian`. The cited apt code looks fixed already.
- [ ] Fix the low hygiene items. The identity wrapper in `src/core/caps.rs:20`. The unvalidated dockerfile and context paths in `src/core/container.rs:221`. The orphaned doc lines in `src/daemon/handlers.rs:591` and `src/daemon/server.rs:645`.

## Loop rule

One phase at a time. One small unit per commit. Verify each unit with its cited command before the next. Do not batch phases and check once at the end.

## Round A additions (2026-09-03)

Second loop, five slices over terrain the first pass covered lightly. Each item re verified by grep in the main thread before listing. Numbers continue the phases above.

- [ ] Sanitize remote AUR metadata in the tea info path. `src/cli/tea/info_model.rs:171,187,192,197,215` renders name, description, url, maintainer, and header with zero sanitize calls. The non tea path in `packages/info.rs` sanitizes the same fields. Route the tea sites through `style::sanitize_terminal_text`. Terminal escape injection, medium.
- [ ] Sanitize the AUR install echo in `src/cli/modern_ui.rs:609`. Name, version, and description print raw before the confirm prompt. Same fix, one call site each.
- [ ] Sanitize the AUR progress prefix fed from `src/package_managers/aur/client.rs:2838` into `src/cli/modern_ui.rs:240`. The newer `ProgressTask` lanes sanitize. This older path does not.
- [ ] Replace the TUI sanitizer in `src/cli/tui/ui.rs:23`. It strips control chars only. Bidi overrides pass through. Delegate to `style::sanitize_terminal_text` and sanitize `pkg.name` and `pkg.version` at `ui.rs:827,837`. Medium.
- [ ] Sanitize manifest echo in `src/cli/migrate.rs:124,129,194`. Manifest strings arrive from another machine and print raw. Execution is validated, display is not. Medium.
- [ ] Stop cloning decoded payloads in `src/core/client.rs:379`. `as_search`, `as_status`, `as_audit`, and `as_updates` clone an already owned value. Pass ownership into the extractor. Medium, hottest return path.
- [ ] Add `validate_socket_parent` to `src/bin/omg-fast.rs:151`. Duplicate of the Phase 1 item above. Close one of the two when fixed. Corroborated twice. Low.
- [ ] Check config symlinks in `src/config/settings.rs:239`. Load follows symlinks with no ownership or mode check. Mirror the `read_regular_file` guard. Medium.
- [ ] Gate `omg workspace run` custom commands in `src/cli/workspace.rs:376`. Repo committed shell runs with no consent prompt and no `current_dir` containment. Add a confirm or `--yes` gate. Medium.
- [ ] Pin the workspace hook binary in `src/core/env/team.rs:444`. Emitted hooks call bare `omg` from PATH. A shadowing entry runs on every pull. Low medium.
- [ ] Confirm before privileged remediation in `src/cli/doctor.rs:615`. `doctor --turbo` runs `sudo setcap -r` unattended and misreports failure as healthy. Add a prompt and report the real error. Medium.
- [ ] Reject `..` in `validate_image_ref` in `src/core/security/validation.rs:147`. Package names have the check, image refs do not. Argv usage contains it today. Low.
- [ ] Reject `..` in `pkg.filename` in `src/package_managers/debian_pure.rs:627`. Index supplied name joins the fetch path unchecked. Hash still gates bytes. Low.
- [ ] Unify control.tar extraction with the data.tar hardening in `src/package_managers/debian_db/transaction.rs:1683`. Control path relies on crate defaults. Low.
- [ ] Delete dead request variants `Request::Health`, `Request::CacheStats`, `Request::CacheClear` in `src/daemon/protocol.rs:59`. Zero production constructors outside the dispatch arms. Delete with `DaemonClient::status` in `src/core/client.rs:272` and `:478`. Low.
- [ ] Delete dead config `TeamConfig.auto_sync` in `src/core/env/team.rs:41`, the full `NotificationSettings` block at `:45`, and enforce or delete `Workspace.runtimes` in `src/cli/workspace.rs:25`. Parsed but never read, grep proven. Low.
- [ ] Delete dead `get_configured_repos` in `src/core/pacman_conf.rs:294`, `dry_run` in `src/package_managers/debian_db/transaction.rs:2473`, and `total_download_size` at `:977`. Zero production callers each. Low.
- [ ] Fix the `--force` provenance bypass in `src/cli/self_update.rs:140`. Force doubles as the unverified provenance escape while docs name only the env var. Document or separate. Low.

## Round B additions (2026-09-03)

Adversarial security round, five lenses. Re verified by grep in the main thread. Duplicates of earlier items are marked, not relisted.

- [ ] Stop sending the raw license key in telemetry batches. `src/core/telemetry_client.rs:112,140` fills `license_key` from the live bearer credential into every batch POST. Send a fingerprint or a linked flag instead. `src/core/usage.rs:861` already treats the key as unfit for reports. Medium.
- [ ] Pin the installer bootstrap. `install.sh:8` pipes mutable `main` with no script checksum, and the binary verifies by same origin sha256 sidecar only at `install.sh:330`. Add a version pin plus a signature or attestation check. Medium.
- [ ] Remove pipe to shell from generated CI templates. `src/cli/ci.rs:196,233,329,391,439,468,510` embeds `curl install.sh | sh` seven times. Emit the pinned installer instead. Medium.
- [ ] Pin scaffold dependencies and disable lifecycle scripts. `src/cli/new.rs:140` writes `latest` pins then runs plain `npm install`. Pin versions and pass `--ignore-scripts` like `src/runtimes/pi.rs:53` does. Medium.
- [ ] Guard the gist upload read in `src/cli/env.rs:172`. It reads `omg.lock` raw before shipping to a remote gist, bypassing the guarded `read_lockfile`. Route it through the guarded reader. Medium.
- [ ] Require full fingerprints for PGP keys in `src/package_managers/aur/client.rs:137`. Long IDs are collision feasible. Arch guidance wants 40 char fingerprints. Medium low.
- [ ] Require digests for runtime container refs in `src/core/container.rs:87`. Tags drift silently while repo Dockerfiles pin digests. Medium low.
- [ ] Resolve sibling `omg` instead of PATH lookup in `src/cli/workspace.rs:494,610`. Reuse the sibling resolution from `src/cli/commands.rs:709`. Same hijack class as the daemon path and the `omgd` fallback below. Fix all three in one wave. Medium.
- [ ] Replace the container ref denylist with the allowlist in `src/cli/container.rs:55`. It rejects three metacharacters while the core gate allowlists. One allowlist everywhere. Low.
- [x] Container env key parsing. Resolved upstream by commit 6e9516e2. Parsing lives in `src/cli/container.rs:16`, not lossy, rejects empty keys, tested at `:517`. No action.
- [ ] Look up the chown user without `USER` env in `src/package_managers/aur/client.rs:2379`. Env attacker settable reaching a root spawned process. Prefer pwd lookup. Low.
- [ ] Prefer sibling `omgd` over PATH fallback in `src/cli/commands.rs:675`. Same hijack class, dev installs exposed. Low.
- [ ] Guard history reads and lock in `src/core/history.rs:147,276`. Reads follow symlinks, lock opens with no mode. Mirror `read_lockfile`. Low.
- [ ] Guard the license clock lock in `src/core/license.rs:468`. Third instance of the lock family gap. Same fix. Low.
- [ ] Guard snapshot reads in `src/cli/snapshot.rs:37`. Reads follow symlinks. Close the create race at `:80` where exists check then persist lets concurrent creates replace each other. Low.
- [ ] Converge sudo env scrubbing into one function. `src/core/privilege.rs:196` scrubs three vars while `payload_command` strips library and interpreter paths. One shared list for all sudo sites. Low.
- [ ] Accept a dashboard token outside argv in `src/cli/args.rs:1012`. Argv leaks into history and process list. Add env or prompt input. Low.
- [ ] Note. omg-fast socket gap and self update force gate confirmed again this round, still unfixed. The force item escalates to medium. No new rows, fix the existing ones.

## Round C additions (2026-09-03)

Dead code, dead tests, slop, and duplication round. Each death claim carries a caller grep. Six spot checks re verified in the main thread.

- [ ] Delete dead `toggle`, `status`, and `set_enabled` in `src/cli/telemetry.rs:12,56,84`. Zero callers. Dispatch uses `privacy_status` and friends. Delete as one unit.
- [ ] Delete dead `highlight` in `src/cli/style.rs:213`. Only a comment mentions it.
- [ ] Delete or read `CliContext` fields `verbose`, `quiet`, `no_color` in `src/cli/mod.rs:114`. Written once, never read. Only `ctx.json` is consumed.
- [ ] Delete or gate the tier feature API (`current_tier`, `has_feature`, `require_feature`, `features_for_tier`) in `src/core/license.rs:902,908,913,928`. Zero CLI callers. Public lib surface, so gate or deprecate rather than silent delete.
- [ ] Delete `pacman_sync_dir` in `src/core/paths.rs:322` or move it behind test cfg. Only a test calls it. Production uses the validated variant.
- [ ] Delete `pacman_cache_dirs` in `src/core/paths.rs:352`. Zero callers. The gated `_result` twin owns the contract.
- [ ] Fix the unreachable `SlsaError::RekorBodyHashHex` in `src/core/security/slsa.rs:78`. The decode path returns `Ok(false)` instead of raising it. Wire the error or delete the variant.
- [ ] Delete `require_detached_signature_files` in `src/core/security/pgp.rs:302` or move it behind test cfg. Test only callers today. Distinct from the dead verifier above.
- [ ] Delete `read_metadata_archive` in `src/package_managers/aur_metadata.rs:250` or move it behind test cfg. Its own doc prefers `AurIndex`. One test caller.
- [ ] Add asserts or delete the discard everything tests in `tests/debian_e2e_tests.rs:619` and `tests/alpm_transaction_e2e.rs:343`. Results dropped with `let _` and `drop`. Cannot fail.
- [ ] Replace the OR of negations paywall guards in `tests/arch_tests.rs:548,571` and `tests/security_tests.rs:584,595,606`. Near miss strings pass. Assert one exact marker.
- [ ] Replace the any output guards in `tests/integration_suite.rs:733,1169`, `tests/privilege_tests.rs:344,431`, `tests/debian_tests.rs:256`, `tests/update_integration.rs:467`. Panic text satisfies them. `error_tests.rs` already owns the replacement helper.
- [ ] Deduplicate the copied test bodies in `tests/install_update_comprehensive.rs:400,948,660,959` and `tests/integration_suite.rs:164,1566`. One is byte identical to an unrelated name. One helper each.
- [ ] Move daemon fixtures into `tests/common/`. `tests/daemon_e2e_caching.rs:23`, `tests/daemon_e2e_concurrency.rs:29`, and the IPC fixture build the same state three times.
- [ ] Fix the false makedepend claim in `README.md:111`. No removal path exists in src. Delete the clause.
- [ ] Fix the AUR fallback claim in `docs/package-search.md:27`. Search runs concurrently and unconditionally. No sparse threshold exists.
- [ ] Fix the daemon queue claim in `docs/daemon.md:64`. No task queue exists in `src/daemon`.
- [ ] Sweep marketing adjectives from `src/cli/tui/ui.rs:1`, `src/cli/ui.rs:166,172`, `src/cli/tea/mod.rs:4`, `src/cli/args.rs:1140`, and the docs list in the Round C report. One mechanical pass.
- [ ] Sweep en dashes from `src/package_managers/aur/client.rs:111` and the docs ranges. Encode as a CI grep with a narrow token list so it cannot recur.
- [ ] Unify sibling binary resolution with a new `sibling_binary` helper in `core/paths.rs`. Copies at `src/cli/commands.rs:708` and `src/cli/init.rs:888` disagree on existence semantics. Same wave as the Round B PATH hijack items.
- [ ] Converge sudo scrub lists into one const at the top of `core/privilege.rs`. Two sites carry overlapping lists at `:136` and `:381`. Same root as the Round B scrub item. Fix once.
- [ ] Route atomic writes through `core/safe_ops.rs:168`. `src/core/fast_status.rs:75` and `src/cli/self_update.rs:173` reimplement it. One mode controlled variant covers both.
- [ ] Unify daemon connect and readiness in `core/client.rs:34`. Retry loop at `src/cli/commands.rs:688` belongs in the client layer. The shared helper fixes omg-fast for free. Same root as the socket items above.
- [ ] Unify install consent and backend dispatch with new lib fns in `core/security/mod.rs`. Twins at `src/bin/omg.rs:134,779` and `src/cli/packages/install.rs:119,155` already diverge on debian local files. Start with the consent gate before the dispatch refactor.
