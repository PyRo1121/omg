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

## Later. Onboarding

Not now. After omg is a working package tool. Revisit this section then. Do not implement from this list while update, install, and completions are still the product.

### Goal

Install to first successful `omg search` / `omg install` in under two minutes, with Tab completion that actually fires. Personalize for a distro only if the user asks. Never become a distro manager. Never attach distro work to `omg update`.

### What exists today

`install.sh` copies the binary to `~/.local/bin`, may append PATH and `omg hook` to the login rc, and runs `omg completions $shell` (stdout discarded; the command still writes files). Flags: `OMG_SKIP_SHELL=1`, `OMG_NO_TELEMETRY=1`.

`omg init` (`src/cli/init.rs`) is a 5-step TTY wizard. Non-TTY falls through to `--defaults`.

1. Shell (zsh/bash/fish, detected from `$SHELL`, parent `/proc`, or passwd).
2. Daemon start (shell init, on demand, systemd user unit, or manual).
3. Telemetry consent (default off in the wizard).
4. AUR build recommendation from CPU/RAM and ccache/sccache.
5. Capture `omg.lock`.

`omg doctor` already points a missing hook at `omg init`. First-run telemetry uses a marker file (`src/core/telemetry.rs` `is_first_run`), separate from the wizard.

`docs/cli.md` says init installs completions. The wizard does not call `install_completions`. Completions live in `src/hooks/completions.rs` and must land on zsh `fpath` (oh-my-zsh `~/.oh-my-zsh/completions/` plus `~/.zfunc`). That gap bit this machine.

Distro detection (`src/core/env/distro.rs`) is a package-backend enum: Arch, Debian, Ubuntu, Fedora, MacOS, Unknown. `ID=omarchy` with `ID_LIKE=arch` is Arch. There is no flavor/profile type. Do not overload `Distro` for onboarding. A profile is a user choice on top of the backend.

Omarchy's updater (`/usr/bin/omarchy-update`) is a distro ritual. After `pacman -Syu` it runs `omarchy-migrate`, a post-update hook, `yay -Sua`, `mise up`, orphan removal, then restart. Migrations such as `migrations/1788596255.sh` (`omarchy-pkg-add vi`) install brand-new packages. `omg update` is sysupgrade only. That split stays.

### Research (2026-09-06)

**rustup-init.** One installer. PATH modification is an explicit question. Completions are documented separately (`rustup completions`, zsh needs `fpath+=~/.zfunc` before `compinit`). `-y` for CI. rustup#2915: never dump an interactive wizard into a non-TTY or into another tool's pipeline (makepkg/paru). If stdin is not a terminal, take defaults or refuse. Do not auto-launch `omg init` from `omg update` or `makepkg`.

**uv (Astral).** The tool works with no wizard. The installer may edit PATH; `UV_NO_MODIFY_PATH` / `UV_UNMANAGED_INSTALL` for CI. Completions are a documented follow-up (`uv generate-shell-completion`), not a blocking prompt. Self-update can re-touch profiles unless opted out. omg should stay usable if the user skips init.

**mise.** Three steps: install CLI, activate in rc (or shims), then add tools. Completions are extra and need `fpath` setup, same class of bug we hit. `mise bootstrap` can install packages, repos, dotfiles, systemd units, and a login shell, but only from an explicit config the user wrote. Distro-shaped work is opt-in config, not implicit takeover. Copy that rule.

**paru / yay.** First run may write a config file. No distro flavor wizard. They stay AUR helpers. omg should stay a package tool the same way.

**Omarchy.** `omarchy setup …` is security and boot, not package onboarding. Desktop extras belong to `omarchy-migrate` / `omarchy update`. omg may hint. It must not reimplement those scripts.

**This repo's print CLI.** One rail, no boxes. The current init banner is a magenta box and a rocket. When we touch init, drop that. Match `src/cli/modern_ui.rs`. Keep ↑/↓/Enter/q. Keep `--defaults` for CI.

### Gaps to close when we build this

- Completions are not a wizard step. Docs claim they are. install.sh tries; `omg init` does not.
- zsh completion only works if the write path is on `fpath`. Creating `~/.oh-my-zsh/completions/` matters on Omarchy/omz.
- `--defaults` does not record telemetry (leaves settings as-is) and starts the daemon on demand, not via systemd.
- First command can ping telemetry from the install marker before the user has seen the consent step.
- No PATH check inside `omg init` (install.sh does it; `make install` may not).
- No distro profile. Skip is the only honest default.
- Wizard is not idempotent for completions or PATH. Hook install is.

### Target flow

Four layers. Each one works without the next.

1. **Installer.** Binary on PATH. Optional hook. Optional completions. `OMG_SKIP_SHELL=1` stays. Do not run the wizard from `curl | bash` when stdin is the pipe (non-TTY). Print `omg init` as the next command if stdout is a TTY after the pipe returns.
2. **`omg init`.** Machine setup. Shell, completions, daemon, telemetry, build flags, optional env capture. Idempotent. `omg doctor` repairs it.
3. **Distro profile.** One skippable step. Writes a setting. May install aliases or a doctor hint. Does not run distro scripts. Does not change `omg update`.
4. **First command.** Prove it. `omg search firefox` or `omg status`. Completion proof is `omg install <Tab>` after a new shell.

### Wizard steps (when we ship it)

Preflight. If not a TTY, `--defaults` and exit. Detect shell, os-release ID/ID_LIKE, existing hook, existing completions, daemon state. Re-running init must not duplicate rc lines.

0. One line of chrome. Not a box. "Nothing here is required. q quits."
1. Shell hook. Same as today. Detected shell highlighted.
2. Completions. Call the same installer as `omg completions`. For zsh write both omz completions (create the dir if oh-my-zsh exists) and `~/.zfunc`. Tell the user they need a new shell, not `source` of a completion file that is not on `fpath`.
3. Daemon. Same as today. Prefer systemd user unit on Linux when available; keep Manual.
4. Telemetry. Default off. Record the choice in settings before any ping. First-run ping must respect that file.
5. Build settings. Same as today. Skip leaves current config.
6. Distro profile. **Skip is highlighted even when Omarchy is detected.** Options: Skip (pacman/yay only), Omarchy, maybe later CachyOS / EndeavourOS / vanilla Arch. Detected flavor is a label, not an auto-pick. `--distro skip|omarchy|detect` for scripts. `detect` still requires a TTY confirm unless `--yes` was passed with an explicit `--distro`.
7. Env capture. Default no on a personal machine. Team docs can tell people to pass yes.

Apply, then print three commands that work now. If profile is Omarchy, one extra line: desktop extras stay on `omarchy update` / `omarchy-migrate`. Point `omg doctor` as the repair loop.

### Distro profile (data, not a backend)

New optional setting, not a new `Distro` variant.

```toml
[profile]
# none | omarchy | (later names)
distro = "none"
```

| Choice | What we write | What we never do |
| --- | --- | --- |
| Skip / none | `distro = "none"` | Anything Omarchy-specific |
| Omarchy | `distro = "omarchy"` | Call `omarchy-migrate`, `mise`, `omarchy-hook`, snapshot, firmware, or orphans on `omg update` |
| Future names | Same pattern: hint + doctor | Reimplement that distro's updater |

Omarchy profile may:

- Leave a doctor check: if `omarchy-migrate --pending` exits 0, warn to run `omarchy-migrate` or `omarchy update`.
- Mention `omg clean --orphans` as the omg-side equivalent of their orphan prompt (separate command).
- Document that AUR is omg, not yay, so `omarchy-update-aur-pkgs` is redundant if they use omg.

Omarchy profile must not:

- `omarchy-pkg-add` from this repo.
- Copy files out of `/usr/share/omarchy/migrations/`.
- Pass `--overwrite '/usr/share/omarchy/*'` unless we later prove a real file-conflict failure and treat it as an ALPM flag, not a profile.

Backend detection stays Arch for Omarchy. Profile is flavor. Tests should pin `classify("omarchy", "arch") == Distro::Arch` when that helper is touched.

### Hard no

- No wizard, migrate, mise, or orphan prompt inside `omg update` or `omg install`.
- No auto-select of a distro because os-release matched.
- No blocking first-command UX ("run init before you can search").
- No interactive init from a pipe, CI, or elevated child.
- No new package-backend enum value named Omarchy.

### Flags

Keep `--defaults`, `--skip-shell`, `--skip-daemon`. Add later: `--skip-completions`, `--distro <id>`, `--yes` (with explicit `--distro`, never implied detect).

### Delivery order

Product first. Then init that matches the docs. Then the skippable profile.

- [ ] Do not start this until search, install, update, and zsh Tab are reliable on a real Omarchy box.
- [ ] Make `omg init` install completions the same way `omg completions` does. Put zsh files on a directory that is actually on `fpath`.
- [ ] Record telemetry consent before any first-run ping. Default off.
- [ ] Drop the box banner when init is retouched. Match the print CLI.
- [ ] Fix `docs/cli.md` so the listed setup steps match the code.
- [ ] Add PATH repair to init/doctor for `make install` users who never ran `install.sh`.
- [ ] Add `[profile] distro` with `none` as default. Skip highlighted in the menu.
- [ ] Omarchy profile: doctor hint to `omarchy-migrate` / `omarchy update`. No migrate from omg.
- [ ] `--distro` and `--defaults` never apply Omarchy extras without an explicit id.
- [ ] Pin `classify("omarchy", "arch")` as Arch when distro tests are next edited.
- [ ] Keep `omg update` as official plus AUR sysupgrade. Orphans stay on `omg clean --orphans`.
