# OMG maintainability audit

## Scope and evidence

Ten independent reviewer scopes completed against HEAD `922db007` plus the dirty working tree. Two reviewers required retries. No product files were edited, builds or tests executed, dependencies installed, or package operations performed by this audit. This report is an artifact, not an implementation change.

This was broad static coverage, not an exhaustive line-by-line proof. Reviewers traced callers, tests, feature gates and platform branches. Findings below are source-backed observations, not runtime reproductions. Line references reflect the reviewed working tree and may move. Concurrent edits were observed, including QEMU workflow changes. Uncommitted runtime/environment work is labeled **WIP** and must not be represented as released behavior.

The parent independently re-read the daemon timer and response truncation, shell source emission and environment merge, Bun/Deno archive paths, AUR cleanup, and QEMU workflow. Other items remain reviewer findings requiring the listed checks. Existing `.audit/deep-audit.tsv` contains historical work; this is a separate snapshot and does not claim every observation is newly discovered.

Maintained website/Worker source is absent from this checkout. `site/` contains leftover documentation and generated output; worker directories contain no reviewed application source. Ignored generated output, dependencies, worktrees, secrets and host state were excluded. External library consumers, deployed services and live vendor contracts were not verified.

## First actions

1. Block automatic execution of untrusted project source scripts in the WIP shell integration.
2. Repair the daemon maintenance timer and reject incomplete successful security-audit responses.
3. Give runtime archives and AUR cleanup explicit concurrent-operation ownership.
4. Repair false-green tests and CI gates before relying on them during refactoring.
5. Consolidate runtime resolution, daemon/backend dispatch and cache freshness in small tested changes.
6. Remove repository-unused APIs only after checking public consumers and moving meaningful tests to live paths.

Do not begin with a large file-splitting exercise. Most expensive complexity here is duplicated policy and resource ownership, not file length.

## Priority findings

### A1. Automatic shell hooks execute repository-controlled scripts [WIP, high confidence]

Evidence: `src/hooks/mod.rs:291-295,310-314,946,1051,1138`; `src/config/mise_env.rs:845-858`.

Ancestor configuration supplies `_.source`; hook output emits native `source` commands, which generated prompt hooks evaluate. The reviewed path has no explicit trust approval. Quoting prevents shell interpolation but does not authorize executing the referenced file. Entering an untrusted project with integration enabled can therefore execute its code.

Smallest change: reject source directives in automatic hooks until an explicit content-bound trust decision exists. Verify an untrusted checkout cannot create a marker merely through prompt evaluation. Do not execute real untrusted source files to test this.

### A2. A shorter health tick continually cancels the maintenance sleep [high confidence]

Evidence: `src/daemon/server.rs:282,299`.

Each `select!` iteration recreates the five-minute sleep. The persistent one-minute health timer wins and restarts that deadline. Initial refresh runs, but normal periodic refresh and the nested mmap cleanup do not.

Smallest change: construct a persistent maintenance interval outside the loop. Use controlled time to prove refresh occurs despite intervening health ticks. Existing duration-constant tests do not establish scheduling behavior.

### A3. Oversized audit responses silently lose findings [high confidence]

Evidence: `src/daemon/server.rs:547-601`; `src/cli/security.rs:124,1302`.

Response shrinking halves vulnerability entries, recomputes smaller totals, and encodes ordinary success. CLI and compliance export receive no completeness flag. Server-side warnings do not fix the consumer contract. Triggering requires exceeding the response budget.

Smallest change: return the existing size-limit error for exhaustive audit results. Review explicit-package and update lists under the same completeness requirement. Verify oversized output is complete or explicitly fails, never successful with reduced totals.

### A4. Concurrent Bun/Deno installs share archive names [high confidence]

Evidence: `src/runtimes/bun.rs:105`; `src/runtimes/deno.rs:122`; `src/runtimes/common.rs:445`.

Different versions use a platform-only download filename in the shared versions directory. Another install can replace a verified archive before extraction or delete it during cleanup. Atomic download publication and atomic destination publication do not protect the interval between them.

Smallest change: use an operation-owned archive directory, as the Rust installer already does. Verify a deterministic two-install interleaving with different contents but identical archive filenames.

### A5. AUR cleanup removes active build locks [high confidence]

Evidence: `src/package_managers/aur/client.rs:1014,3970`; `src/cli/packages/clean.rs:310`.

`clean_all` removes the build tree, including its lock files, without coordinating with builders. An active process can retain a lock on the removed inode while a second process locks its replacement. Active checkouts and outputs are also removed.

Smallest change: put a shared/exclusive lifecycle lock outside the removable tree. Builds hold shared ownership; cleanup requires exclusive ownership. Destructive cleanup intent does not establish concurrency safety. Verify cleanup cannot remove live build state.

### A6. Project environment state leaks between directories [WIP, high confidence]

Evidence: `src/hooks/mod.rs:245,283-308,945,1050,1134`.

Hooks restore PATH but not arbitrary exported/unset project variables. Leaving a project does not restore original values or original absence. Subsequent projects inherit the stale environment, including values used to evaluate defaults.

Smallest change: track and reverse the shell environment delta before applying the next project. Arbitrary sourced-script effects need separate treatment. Verify A → B → outside transitions, including absent variables and explicit unsets.

### A7. Cache freshness has several independent correctness defects [high confidence]

- **Debian deleted lists:** `src/package_managers/debian_db/db.rs:629,655,720,894`. File-map change is detected, but the rebuild decision checks only remaining files. Removed repositories can survive in inventory. Compare complete source fingerprints; test deletion with unchanged surviving files in warm and cold readers.
- **DNF external mutation:** `src/package_managers/dnf.rs:190,233,1312`. Nonempty installed inventory is accepted indefinitely, including cached positives. OMG mutations invalidate it; external rpm/dnf changes do not. Validate an observable database identity on every inventory query.
- **Arch decreasing identity — corrected in [PR #357](https://github.com/PyRo1121/omg/pull/357), `2107704d`:** direct libalpm, worker, and daemon catalog healing now compare observed identities for inequality rather than increases. The temporary-directory regression failed before the fix; afterward, sync/local deletion and backdated timestamps invalidate reuse while unchanged identities remain reusable. Six scoped identity tests and fifteen daemon-handler tests pass, plus Arch library Clippy. The obsolete public `disk_is_newer_than` helper was removed; invalidation consumers migrate to `current != loaded`. The underlying maximum-mtime observer still permits masked changes/collisions, and initialization/publication races remain separate work. Evidence: `alpm-epoch-{red,green,handlers-green,clippy}.log` in the remediation evidence directory.
- **Persisted daemon status — corrected in [PR #359](https://github.com/PyRo1121/omg/pull/359), `d832c1cf`:** v2 snapshots carry publication time and enforce the configured status TTL. Startup/request disk promotion was removed so reads cannot renew their lifetime or seed fresh explicit-count state. The explicit v1 reader validates but does not serve untimed legacy snapshots; only a fresh publication upgrades known legacy data. Unknown/malformed files are rejected and preserved through ordinary reads, writes and invalidation. The legacy regression failed before the fix. Afterward, 26 Arch and 22 Debian-pure scoped tests and both library Clippy checks pass. Evidence: `persisted-status-*` logs in the remediation evidence directory. Publication age is not per-field observation age; concurrent external replacement and refresh-generation races remain outside this fix.
- **Debian FST publication — corrected in [PR #361](https://github.com/PyRo1121/omg/pull/361), `6f5df995` + `58a212f4`:** derive SHA-256 identity from actual canonical name-to-row entries, verify FST checksums, and compare against the target index mapping. Stop trusting/writing separate `.gen` files. Retain checked mmap/FST guards in a consistent lock order. The same-second mismatch regression failed before the fix; native prefix lookup with the rejected pair returns `zsh` for `ba`, while a matching replacement returns `bash`. Counterevidence: mmap exact lookup re-resolves names and does not misroute merely because row positions change. Existing FST format is retained; fingerprints concern lookup mappings, not all package metadata. Fifteen scoped tests and Debian-pure library Clippy pass. Combined #353/#355/#356/#361 verification at `6941db5ba2775de8a9fc8356ce4ea840c477782c` passes 21 scoped tests and Clippy. Logs: `fst-mapping-*`, `fst-prefix-*`, `fst-combined-*`; the rejected exact-misrouting hypothesis is retained separately in `fst-exact-misrouting-hypothesis-rejected.log`. Whole-bundle publication/source races and cold-open performance remain unverified.

### A7 remediation update, 2026-09-08

- [PR #364](https://github.com/PyRo1121/omg/pull/364), `ba38f256`, replaces Arch's maximum-mtime sync observation with a fingerprint of sorted names, directory/entry metadata and symlink targets. Parsed sync-cache reuse and libalpm/daemon observation share this identity. A future-dated-neighbor regression failed before the fix; 54 scoped Arch tests and library Clippy pass afterward (`sync-identity-*` evidence). Sync cache representation uses `sync_db_source_v1`; legacy files are untouched. `SyncDbEpoch` loses meaningless ordering, and empty directories have distinct observed identities. This is metadata observation, not content authentication or writer locking; local/config observation and initialization/publication races remain.

- [PR #362](https://github.com/PyRo1121/omg/pull/362), `0b7e497c`, implements the user-selected single-mmap design. Native metadata is deserialized from the exact validated mapping; the duplicate LZ4 reader/writer and timestamp pairing are removed. Writers retain their own mapped temporary inode through publication rather than reopening a replaceable pathname. The pre-fix fixture exposed version `1` versus `2` under the same timestamp; afterward both views agree, and retained/new readers remain correct across replacement with version `3`. Existing mmap schema is unchanged; legacy LZ4 files are ignored and untouched. Seventeen scoped Debian-pure tests and library Clippy pass (`single-snapshot-*` evidence). Source/in-memory publication races and real-world disk-cold performance remain separate work.
- [Benchmark evidence on #362](https://github.com/PyRo1121/omg/pull/362#issuecomment-5582111943), `7552b03a`: an ignored synthetic benchmark runs fresh readers with warm OS cache, one warmup and seven alternating pairs per size. Across two optimized test-profile runs, 10k-package medians changed from 21.200/22.798 ms to 17.064/19.648 ms; 50k medians changed from 113.457/113.437 ms to 96.712/94.994 ms. Sample median reductions are 13.8–19.5%, not a production or disk-cold performance claim. Raw samples and the exact command are posted on the PR; local logs are `snapshot-load-benchmark{,-repeat}.log`. Snapshot regression and library Clippy pass.

- [PR #353](https://github.com/PyRo1121/omg/pull/353), commit `af4c7c7a`, fixes sequential Debian list-removal invalidation and consolidates the memory/disk/rebuild decision. Ten scoped tests and Debian-pure library Clippy pass. Concurrent APT updates and multi-file publication remain outside that verification.
- [PR #355](https://github.com/PyRo1121/omg/pull/355), commit `376ab2b5`, removes the stale [`DebianIndexCache.installed_set`](https://github.com/PyRo1121/omg/blob/af4c7c7a/src/package_managers/debian_db/db.rs) and routes search/info fallbacks through `installed_names()`. Thirteen scoped tests cover current-status rendering and existing cache contracts. This is a source-backed ownership correction, not a pre-patch host-bound public-entrypoint reproduction. These paths use exact/prefix/substring matching, not fuzzy matching.
- [PR #356](https://github.com/PyRo1121/omg/pull/356), commit `7463e9f9`, adds qualified mmap lookups matching the normal index. A real mapped fixture reproduced the pre-fix miss for `demo:amd64`; seven scoped tests pass after the fix. Info checks installed state using the resolved package name.
- Combined verification of #353/#355/#356 at local `verify/audit-debian-combined` commit `f3e1ae918f408f505cb9791de1aef28c4f3e3f6f`: nineteen scoped tests and Debian-pure library Clippy pass on main base `bab687a7`. Evidence was posted on all three PRs. Full platform and concurrent APT/publication checks remain open.

## Refactoring and dead-code inventory

### B. CLI ownership

**B1. Root/re-exec upgrade history differs.** `src/cli/packages/update/arch.rs:134,147`; `src/bin/omg.rs:50-53`. The direct-root path initializes history after package mutation; re-exec initializes before it. Share the recorded upgrade operation while retaining sync/async execution boundaries. Test history-initialization and operation failures. High confidence.

**B2. Update flags lose their meaning in wrappers.** `src/cli/packages/update.rs:18`; `src/bin/omg.rs:1156`; `src/cli/args.rs:117`. Non-Arch turbo follows the same sync-permitting flow as fast; fast/turbo dispatch drops `no_sync`. Resolve mode once, preserve backend-specific implementation. Assert turbo never synchronizes and define combined-flag behavior. High confidence.

**B3. Status maintains three lookup chains.** `src/cli/commands.rs:381`; `src/cli/packages/status.rs:46,101`. Bare, fallback and JSON status independently choose cache/native/daemon paths; synchronous backend coverage differs. Share a typed snapshot and identical async lookup first. Keep intentional no-Tokio startup optimization. Verify counts/backend/error parity. High confidence.

**B4. Info fallback repeats lookup.** `src/bin/omg.rs:483`; `src/cli/packages/info.rs:218`. The fast-path boolean discards exhausted/error state, then fallback calls `info_sync` again. Give one dispatcher ownership or carry a typed outcome. Verify one probe per source, while retaining legitimate daemon-to-native fallback. High confidence.

**B5. Runtime dispatch already has alias/inventory drift [WIP].** `src/cli/runtimes.rs:107,374,395,459`. Installed and remote branches normalize differently; accepted aliases miss remote dispatch. All-runtime branches maintain different inventories. Canonicalize once and share collection, keeping provider-specific formatting. Verify aliases and native/registry parity. High confidence.

**B6. Sequential/parallel workspace failure policies differ.** `src/cli/workspace.rs:318,379,397`. Sequential execution runs dependents after prerequisite failure; parallel execution skips them. Share failure propagation while continuing independent projects. High confidence in difference, intended policy needs confirmation. Test both modes on the same dependency graph.

**B7. Init has a weaker duplicate daemon startup.** `src/cli/init.rs:838`; `src/cli/commands.rs:616`. Init equates spawn success with readiness; the normal command waits. Extract a non-printing lifecycle operation, preserve sibling-only lookup and warn-and-continue policy where intentional. Test immediate exit and delayed readiness. High confidence.

**B8. Error rendering makes trailing error returns unreachable.** `src/cli/env.rs:59,166`; `container.rs:69,138,253,262,277,464`; `team.rs:23,157`; `enterprise.rs:99`; executor `src/cli/packages/mod.rs:117`. `execute_cmd(...error...)?` already returns early. Remove dead tails while preserving original causes and suggestion output. High confidence.

### C. Core responsibilities

**C1. Runtime pin interpretation differs between tasks and hooks.** `src/core/task_runner.rs:1255,1286`; `src/hooks/mod.rs:811`. Bare semver requirements accept versions the hook's exact/partial policy rejects. NVM lookup is also duplicated with different containment checks. Put normalization and validated alias lookup in the existing runtime layer. Test exact, partial, alias and actual child PATH. High confidence.

**C2. Task environments duplicate the new interpreter [WIP].** `src/core/task_runner.rs:410,1074`; `src/config/mise_env.rs:438,666-842`. Task handling has different template accumulation, file expansion and ignored-option behavior. Share parsed domain types and explicitly reject task-unsupported semantics. Shell sourcing/application can remain distinct. High structural confidence; intended compatibility needs review.

**C3. Privilege injection chain has only repository test consumers.** `src/core/privilege.rs:321,429,487`; production entry `src/bin/omg.rs:845-848`. Candidate chain includes `PrivilegeChecker`, system/mock implementations, sync elevation, whitelist wrapper and its mutex. Check downstream public API use before removal; move rejection tests onto live privilege operations. Do not remove dirty sudo fallback or production `run_self_sudo`/trusted execution. High repository-local confidence, deletion safety conditional.

**C4. Historical feature-entitlement catalogue is test-supported only.** `src/core/license.rs:132,247,911,931`. Feature enum/classifications/arrays describe a retired gating model. Remove after consumer review; preserve tiers, verified identity, stored-license readers and account display. Replace catalogue assertions with command-level ungated behavior checks. High repository-local confidence, external use unknown.

**C5. Two synchronous daemon APIs own the same transport.** `src/core/client.rs:93,464`. `DaemonClient` mixes optional sync/async modes while `SyncDaemonClient` independently implements sync. Invalid method/mode pairs become runtime errors. Make one async client and one mandatory-stream sync client. Preserve startup performance, timeouts and framing checks. High confidence.

**C6. Team configuration is duplicated in persisted status.** `src/core/env/team.rs:75,284,304`; `src/cli/team.rs:209`. Join updates `team.toml` but status retains old embedded configuration. Refresh status from its authority; remove redundant persistence only through an explicit migration preserving members. Verify join/change/reload. High confidence.

**C7. Pacman path helpers disagree.** `src/core/paths.rs:301,370`; `src/cli/doctor.rs:343`. Infallible helpers ignore pacman.conf while fallible helpers honor RootDir/DBPath. Diagnosis can inspect a different database from operations. Consolidate Arch resolution without breaking portable fixtures. Verify custom paths, invalid config and overrides. High confidence.

**C8. Usage state exposes a second unlocked persistence path.** `src/core/usage.rs:251,520,532`. Production callers lock correctly, but `record_command` itself writes and competing wrappers own load/lock/save. Make record operations pure and route updates through `update_locked`. Preserve dirty ownership fixes and timestamp-only merges. Structural finding, not demonstrated current production data loss.

### D. Runtime installation

**D1. Publication is confused with readiness [partly WIP].** `src/runtimes/common.rs:1208`; `erlang.rs:380`; `php.rs:498`. Directory validity does not read the completion marker. Erlang/PHP publish before required initialization/smoke checks. Validate before publishing when possible; model Erlang final-path initialization explicitly. Do not globally require a marker: legacy installs and ordinary binary directories use the predicate. Verify interruption/concurrent activation.

**D2. Generic archive stripping can destroy layout [WIP].** `src/runtimes/github_tool.rs:557`. Strip-one then retry-only-if-no-binary can flatten `bin/tool` and `lib/support` while still declaring success. Extract intact, locate the binary and link it. Verify a flat multi-directory launcher fixture. Transformation is certain; affected live vendor archives were not checked.

**D3. Erlang checksum fixture contradicts parser [WIP].** `src/runtimes/erlang.rs:185,526,537`. A stable fixture has a 64-character checksum while the parser requires 40 and the test expects success. Confirm publisher format before choosing checksum types; do not loosen verification arbitrarily. Certain static contradiction, live production impact unknown.

**D4. PHP rolling-channel refresh is unreachable [WIP].** `src/runtimes/php.rs:439`; `src/cli/runtimes.rs:94`. CLI and manager both skip an installed channel despite documented reinstall refresh. Manager should own immutable-version versus mutable-channel behavior. Verify two builds of one channel and failure preservation. Uninstall/reinstall is a workaround, not in-place refresh.

**D5. Exact generic installs are limited by display pagination [WIP].** `src/runtimes/github_tool.rs:395`. Only 90 release rows are searched, so older valid exact pins become not-found. Retain selected metadata and use explicit tag conventions for exact lookup. Bounded discovery remains useful; it cannot prove absence. Verify an exact tag beyond the window and request counts.

**D6. Verified download implementations are copied [WIP].** `src/runtimes/common.rs:355,458,561`; `swift.rs:200`. SHA-256/512/1 repeat HTTP, bounds, progress, temp ownership and publication. Share one private digest-parameterized verified transport using existing digest support; keep vendor parsing separate and verification mandatory. Test mismatch, truncation, limits and cancellation for each digest.

**D7. Generic latest relies on publication order [WIP].** `src/runtimes/github_tool.rs:59,72`; compare `bun.rs:203`, `deno.rs:330`. No sorting precedes first-stable selection. A recently published old maintenance release can beat a newer stable version. Reuse version comparison with deterministic ordering. Test shuffled/backport releases.

**D8. Swift is unregistered WIP, not proven abandoned.** `src/runtimes/swift.rs:89`; `src/runtimes/mod.rs`. No reviewed module/CLI/hook registration means its tests are not compiled through the current module tree. Ask its owner whether to finish or remove it. Do not count uncommitted unfinished integration as established dead product code.

### E. Package backend interfaces

**E1. Removal history uses candidate rather than installed metadata.** `src/core/packages/service.rs:256`; `src/package_managers/apt.rs:327`; `homebrew.rs:1068`; compare `alpm_direct.rs:166`. Backend `info` semantics differ. Capture installed versions from one inventory snapshot before mutation. Verify installed v1/candidate v2 records v1. DNF native history replaces this provisional evidence; APT/Homebrew do not. High confidence.

**E2. Homebrew uninstall depends on online catalog membership.** `src/package_managers/homebrew.rs:191,1013,1022`. Installation catalog classification excludes some locally installed/tapped packages and cold-cache offline removal. Preserve kind in local inventory and use it for removal; retain explicit formula/cask disambiguation. Verify offline removal of a locally installed entry absent from APIs. High confidence.

**E3. Retired Debian mutation engine remains publicly compiled.** `src/package_managers/debian_db/mod.rs:25,43`; `transaction.rs:463,1018`. Non-test execution rejects mutations but large unpack/configure/rollback machinery remains. Separate retained read-only planning from obsolete mutation implementation. Keep live dry-run resolver at `src/cli/packages/install/debian.rs:77` and meaningful tests/bench formatting. External API review required before narrowing exports.

**E4. PackageService bypasses its injected backend.** `src/core/packages/service.rs:24-30,398-413`. Ambient daemon update discovery can return inventory for a different backend than the subsequent mutation. Keep daemon acceleration in the production adapter, not the backend-independent service. Test differing fixture and daemon inventories. Ordinary host CLI often uses the same backend, limiting immediate exposure.

**E5. Public removal API ignores its recursive argument.** `src/core/packages/service.rs:250`; `src/cli/packages/common.rs:356-359`; `remove.rs:79`. CLI configures the backend correctly, but library callers cannot rely on the parameter. Remove the obsolete argument and document backend-owned semantics with an explicit API change. Not a claim that CLI recursive removal is broken.

### F. AUR/security lifecycle

**F1. Audit writer has no drain owner.** `src/core/security/audit.rs:832`; `src/daemon/server.rs:438`. Static sender and discarded thread handle allow accepted events to disappear at orderly exit. Add bounded flush/shutdown acknowledgement after intake stops. Preserve intentional overload behavior and synchronous recording. Verify enqueue-then-shutdown persistence or explicit incompleteness. High confidence.

**F2. Audit initialization/recovery bypass append locking.** `src/core/security/audit.rs:273,634,857`. Initial tail read lacks shared lock; quarantine lacks exclusive lock/recheck. Normal append serialization is sound but does not cover those phases. Add consistent locking and interleaving tests. High confidence in gap, medium in observed failure without reproduction.

**F3. AUR wave memory is not bounded by build concurrency.** `src/package_managers/aur/parallel_build.rs:307,350`; `src/core/security/artifact.rs:54`. Completed sealed memfds accumulate for the entire wave; per-file 8 GiB cap is not an aggregate cap. Add operation byte/count budget while preserving sealed handoff and transaction semantics. Verify bounded failure and release with small fixture limits.

**F4. Cache eligibility and final authorization disagree.** `src/package_managers/aur/client.rs:2466,2503,2539,5373`. Cache eligibility omits architecture; final authorization requires it. Invalid cached archive suppresses rebuild then fails closed. Share applicable pure identity checks, retain final sealed verification. Verify wrong/missing architecture causes a cache miss, not a failed install from an accepted hit.

**F5. Top-level/dependency build lifecycles are duplicated.** `src/package_managers/aur/client.rs:249,1973,2148`. Review, PGP, dependency work, lock handling, cache/build/authorization repeat. `reviewed_digest` is optional though constructors provide it. First make required evidence mandatory; then extract shared cache/build/authorize work. Preserve recursive traversal/cycle handling and UI differences. Test policy parity.

**F6. Rollback scratch cleanup is success-only.** `src/package_managers/aur/client.rs:1624,1777`. Error/cancellation after unique checkout creation leaves unreusable trees. Use operation-owned temp-directory cleanup under the existing rollback area; keep logs separately. Test missing version, rejected review, failure and cancellation. High confidence.

**F7. SLSA verification retains whole artifact bytes unnecessarily.** `src/core/security/slsa.rs:892,957`. Synchronous full read remains alive across network operations and repeated hashing. Stream digest in blocking work and use supported prehash verification through a private invariant-preserving boundary. Retain chain/identity/SET/tamper checks. Verify RSA/P-256 fixtures and bounded memory; no benchmark was run.

### G. Daemon architecture

**G1. Prewarming seeds a smaller search authority.** `src/daemon/server.rs:242`; `src/daemon/handlers.rs:702,724`. Prewarm caches 50 results where normal backing cache uses 1,000. Later larger requests underreport matches/totals until eviction. Use the same helper and backing limit. Test more than 50 matches against cold and warm search.

**G2. Duplicate backend dispatch omits Fedora/Homebrew.** `src/daemon/handlers.rs:229,240,903,934`; backend methods `dnf.rs:1257,1283`, `homebrew.rs:1113,1141`. Search indexing supports them, but production status/explicit dispatch recognizes only apt/apt-pure/pacman. Call the existing trait, keeping optimization in adapters. Test production-equivalent cold requests, not only injected-state branches.

**G3. Server cancellation does not own completion.** `src/daemon/server.rs:323,417,438`. Detached monitors/connections and unawaited refresh work can survive server return; fatal accept exits bypass common cancellation. Retain handles and use one bounded shutdown path. Existing socket cleanup is useful but does not prove drainage. Test stop during refresh/request and no post-return writes.

**G4. Daemon and ALPM worker both own refresh.** `src/daemon/handlers.rs:157,184,506`; `src/package_managers/alpm_worker.rs:50,92,103`. Daemon replaces workers while workers already heal catalog handles; startup waits synchronously. Keep worker identity stable and make it own refresh. First preserve configuration-only reload, since catalog epochs omit pacman.conf. Structural confidence high, safe removal conditional on those tests.

**G5. DebianSearch duplicates a live generic protocol [removal candidate].** `src/daemon/handlers.rs:547,588`; `cache.rs:97`; `protocol.rs:105`. Repository production clients use Search; tests/benchmarks use the parallel cache/request that relabels results Apt even without a Debian gate. Migrate useful tests, review external clients and bump protocol if removed. Repository-local deadness is not proof of no external consumers.

### H. Config and presentation

**H1. Child assignment cannot reverse parent unset [WIP].** `src/config/mise_env.rs:869-876`; `src/hooks/mod.rs:283-287`. Merge adds an assignment but retains its earlier unset marker; rendering exports then unsets it. Enforce one final action per variable. Test parent unset → child value in actual shell output. High confidence.

**H2. InfoModel is a parallel unused production implementation.** `src/cli/tea/wrappers.rs:12-14`; `info_model.rs:448-458`; live dispatch `src/bin/omg.rs:1355`. Tests/public exports retain it, but normal CLI uses generic package info while Tea supports fewer backends. Retire after external-consumer check; preserve live status/report models. TUI itself has callers and is not dead.

**H3. Two output executors disagree about remedies.** `src/cli/components/mod.rs:145-150`; `src/cli/tea/mod.rs:284-285`; `src/cli/packages/mod.rs:92-104`. Tea stops at error before suggestion; package executor continues the batch. Use one explicit report/error contract and executor. Existing tests intentionally pin fail-fast behavior, so change the contract deliberately and verify original error plus remedy.

**H4. Hook removal replaces symlink-managed rc files.** `src/hooks/mod.rs:191,217`; `src/core/safe_ops.rs:219`. Read follows the symlink; atomic replacement replaces the link rather than its target. Detect and reject with guidance, or adopt an explicit target-preserving policy. Content backup does not preserve dotfile linkage. Verify a symlink fixture.

### I. Tests and benchmarks

**I1. AUR benchmark removes host packages.** `benches/aur_install_bench.sh:27,48,109`. Cleanup uses privileged recursive removal, ignores failure and can remove its own yay comparator. Header warns about clean systems but does not enforce isolation. Require disposable guests and inventory restoration checks; comparator must not be a measured removal target. Do not run this on the host.

**I2. Shared test timeout does not bound descendant lifetime.** `tests/common/mod.rs:332,344-345`. Killing direct child followed by blocking reader joins can hang if descendants retain pipes. Own process groups and bound drain/reap. Test a child spawning a pipe-inheriting sleeper. Mechanism source-backed, not reproduced.

**I3. Debian completion oracle accepts signals/timeouts.** `tests/debian_e2e_tests.rs:660,895,913`; stronger helper `tests/common/assertions.rs:16`. Checking only panic text/101 lets signal exit -1 pass; weak workflow assertions need no meaningful success. Reuse the existing completion oracle plus command-specific assertions. Verify synthetic abnormal results fail.

**I4. Installed-package benchmark measures runtime listing.** `benches/real_world_benchmark.rs:118`; `src/cli/runtimes.rs:336`. `omg list` and `dpkg -l` are different operations; PATH also selects an arbitrary installed binary. Require explicit artifact identity and matching inventories before timing.

**I5. Resolver benchmarks can time errors or empty selection.** `benches/debian_bench.rs:126,143,152-153`. Selection error is discarded and results black-boxed without success. Use deterministic fixtures and assert dependency closure before measurement. Missing-package fixtures must abort setup.

**I6. Default property tests attempt network installs for a banner assertion.** `tests/property_tests.rs:112,147,155`. Fresh homes plus 100 generated `use node` cases can download metadata/archives; ordinary installation failures still pass after banner output. Separate pure validation, seeded activation and explicitly gated network tests. Verify offline lane makes no requests.

No unregistered Criterion target was confirmed: current 12 targets declare `harness = false`. Nested security tests and Debian timing tests have real registration/invocation. Do not delete test doubles merely because they resemble each other.

### J. Build/release/tooling

**J1. QEMU report partially stale after concurrent edit.** `.github/workflows/qemu-matrix.yml:58,112`. Reviewer saw invalid matrix context in workflow concurrency; parent recheck found it already removed. That subfinding is resolved in the latest inspected file, not fixed by this audit. Checkout pins still differed from the canonical CI pin in the reviewer report and remain suspect; remote ref resolution was not checked. Validate action resolution before calling the workflow launch-blocking.

**J2. Required exact-SHA benchmark is not always scheduled.** `.github/workflows/ci.yml:784`; `.github/workflows/benchmark.yml:6-15,22`; `scripts/require-workflow-success.sh:33-35`. Path filters omit qualifying CI changes and newer runs can cancel required older evidence. Align scheduling/cancellation with exact-SHA consumers. Verify lockfile-only, tests-only and consecutive main pushes. Manual dispatch is recovery, not guaranteed evidence.

**J3. Redirected builds install from the wrong directory.** `install.sh:720,732-734`; `Makefile:75-76`. Cargo honors CARGO_TARGET_DIR but copy paths use literal target/release, potentially installing stale checkout binaries. Resolve actual artifact paths. Verify fake redirected build plus deliberately stale local artifact.

**J4. Multi-file bash syntax command only parses the first file.** `Makefile:271`; `.github/workflows/ci.yml:124`. Subsequent filenames are arguments to install.sh, not additional inputs to `bash -n`. Loop per file, reuse the gate in CI. Verify malformed second-file fixture fails.

**J5. Debian build features differ across supported entry points.** `install.sh:702`; `Dockerfile.apt:33`; `Dockerfile.debian:41`; `Dockerfile.ubuntu:37`; release workflow `:163`. Installation/smoke builds omit features shipped in releases. Align policy or explicitly label reduced-capability test builds. WIP Swift rejects no-PGP builds, but it is not yet registered; do not claim this currently breaks shipped Swift. No claim that APT signature checks are bypassed.

**J6. Benchmark CI builds twice.** `.github/workflows/benchmark.yml:99,112`; `benchmark-hyperfine.sh:196-209`. Workflow builds target/release; script independently builds in its own directory. Use its existing `OMG_BENCH_BINARY` interface and adjacent daemon. Verify one build and recorded artifact identity. Cost savings were not measured.

**J7. Container smoke pipelines mask search failure.** `docker-compose.yml:19-21,28-30`; `Dockerfile.fedora:27`. Search piped to successful head returns success without pipefail. Capture and check search before truncating output; blindly adding pipefail can create SIGPIPE false failures. Verify fake search failure propagates.

**J8. Backend-only dependencies are unconditional.** `Cargo.toml:162,169-170,244`; `src/package_managers/mod.rs:20-30`. Reviewer found dashmap/urlencoding direct uses only under Arch AUR and fst/rustc-hash only under Debian. Make optional under owning features, not globally deleted. Some remain transitive, savings unmeasured. No globally dead direct dependency was established after reference inspection.

### K. Documentation/contracts

**K1. Unsupported security settings are advertised and silently ignored.** `docs/configuration.md:196-205`; `src/core/security/policy.rs:67-79,144-153`. Listed max_cve_severity/require_sbom/verify_slsa/trusted_maintainers are absent from policy fields; unknown TOML keys are not rejected. Remove unsupported examples and reject misspellings/unsupported security keys with tests. Examples are commented by default; supported policy enforcement exists.

**K2. Documented automatic verification pipeline exceeds implementation.** `docs/architecture.md:275-310`; `docs/security.md:51`; `src/core/security/policy.rs:200-206`; `src/cli/security.rs:639-641`. SLSA is an explicit audit capability, not demonstrated automatic provenance for every install. Official packages are explicitly not graded Locked. Separate backend guarantees from optional audits. Do not delete working SLSA verification or imply no installation verification exists.

**K3. Recovery instructions target retired redb files.** `docs/troubleshooting.md:388`; `docs/performance-tips.md:401`; `docs/configuration.md:33`; `CONTRIBUTING.md:65`; actual `src/daemon/db.rs:31`. Update operational paths and contributor tree; preserve historical changelog references. Current JSON-cache docs already provide a correct reference.

**K4. Contributor feature matrix contradicts release builds.** `CONTRIBUTING.md:583-590`; `.github/workflows/release.yml:162,206`; `Cargo.toml:363-366`. Docs claim Debian deliberately omits pgp/license; configured release enables them. Update from actual commands with no-default-features. Published binaries were not inspected.

**K5. Search architecture assigns AUR work to the wrong owner.** `docs/architecture.md:143-161`; `docs/cache.md:42`; actual `src/cli/packages/search.rs:226-243`, `src/daemon/handlers.rs:722-742`. CLI joins official and AUR work; daemon indexes official results. Correct ownership/cache claims using already-correct package-search docs. No latency measurements were made.

The seven endpoint metadata entries in `contracts/service-api-v1.json` matched inspected Rust callers. `src/core/service_api.rs:25-59` does not establish payload-schema or deployed-server compatibility. Those remain separate verification work.

## Archived discovery output (2026-09-08)

The separate generated discovery run `922db007d1df29270b4e5575654b1b2d26bb371f_20260908T013219Z_qiscu5_o` is preserved losslessly in `deep-discovery-922db007.tar.zst` beside this report. Its manifest marks the run **canceled**, sealed at `2026-09-08T04:07:03.357690Z`; its 81 candidate findings are not a completed or independently verified audit.

- Original: 881 regular files, about 514 MiB, including repeated checkpoints and intermediate outputs.
- Archive: about 2.49 MiB; all files retained, not merely the final report.
- SHA-256: `9e3aa605699cc9d1d10e026d02792e7aadf789afcef42d4e21b4470164ec9d08`.
- Verification: `tar -I zstd -df` compared the archive with the original tree; a complete member-list comparison checked for omitted files. Gitleaks scanned 536,912,826 bytes with recursive decoding disabled and reported no leaks. The earlier scan with default decoding timed out; this is not a claim of exhaustive secret detection.
- The uncompressed directory was removed after verification to avoid retaining half a gigabyte of duplicate generated output. Do not unpack the whole archive into the repository or `/tmp` for routine review.

Read individual files without materializing the full tree, for example:

```sh
tar -I zstd -xOf .audit/deep-discovery-922db007.tar.zst \
  922db007d1df29270b4e5575654b1b2d26bb371f_20260908T013219Z_qiscu5_o/findings.json \
  | jq '.findings[] | {findingId, title, locations, validation: .validation.status}'
```

Revalidate candidates against current source and regression tests before acting on them. This archive is historical evidence, not executable instructions or proof of current vulnerabilities.

## Suggested implementation sequence

### Slice 1: protect execution and observation

Fix A1, A2, A3, I2 and I3 with hermetic trust, clock, frame-size and process fixtures. Keep product tests from needing host mutation or live vendors.

### Slice 2: own mutable resources

Fix A4/A5, D1, F1/F2/F3/F6 and G3. Each operation needs a clear owner for archives, locks, threads, subprocesses and cleanup. Preserve crypto verification and immutable-byte handoff.

### Slice 3: one policy per concern

Consolidate C1/C2, B3/B5, C5, E1/E4, G2/G4 and A7. Separate installed/candidate metadata and freshness identity. Use narrow interfaces already in the project, not a new generic workflow framework.

### Slice 4: remove retired paths

Review C3/C4, E3, G5 and H2 for external consumers. Migrate useful tests to production paths, make intentional API/protocol changes, and remove obsolete code rather than add compatibility wrappers. Durable data is the exception: preserve versioned readers or transactional migrations and reject unknown forward versions.

### Slice 5: repair evidence and documentation

Fix J2-J7 and I4-I6. Then update K1-K5 to describe tested behavior. Feature-gate J8 only after supported feature checks.

## Limits and verification status

No runtime reproductions, builds, full suites, platform-matrix execution, destructive benchmarks or deployments were run. Suggested tests are future work, not passing evidence. Public exports with no repository production consumers remain conditional removal candidates. Cryptographic code was examined for maintainability, not exhaustively audited for cryptographic soundness.

All ten scopes returned substantive reports, but portions of large parsers, telemetry transport, historical benchmark artifacts, completion/rendering code and individual test bodies received selective inspection. The separate website/service repository needs its own audit. Recheck dirty-file findings against the final integration diff before implementing changes.
