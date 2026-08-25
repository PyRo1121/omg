# OMG + OMG Dash Web — Full Codebase Audit (Master Report)

**Date:** 2026-08-24 · **Method:** 35 parallel read-only audit agents (Herdr fleet), every source line in scope.
**Repos:** `/home/pyro1121/Documents/omg` (Rust CLI/daemon) · `/home/pyro1121/Documents/omg-web` (SolidStart site + Cloudflare Workers).

## Fleet & coverage

| Area | Slices |
|---|---|
| OMG Rust: binaries, config, fuzz, benches | 01 |
| OMG Rust: src/cli (core, packages, tea/tui) | 02–06 |
| OMG Rust: src/core (incl. security/, env/) | 07–09 |
| OMG Rust: daemon, hooks, runtimes | 10–11 |
| OMG Rust: package managers | 12–14 |
| OMG Rust: tests/ | 15–18 |
| OMG Rust: install.sh, Dockerfiles, CI shell | 19 |
| Web: site/src components | 20–21 |
| Web: site/src lib, routes, pages, design-system | 22–25 |
| Web: shared, e2e, configs | 26 |
| Web: workers handlers (auth/admin/billing/license/analytics) | 27–29 |
| Web: workers core, store, contracts, router/releases | 30–31 |
| CI pipeline security (omg-web) | 32 |
| Cross-cutting sweeps (auth/OTP/admin; Rust injection/privilege; XSS/runes) | 33–35 |

All 35 slice reports follow this summary verbatim. ~530 detailed findings total across slices
(approximately 20 CRITICAL / 37 HIGH / 42 MEDIUM / 42 LOW / 42 INFO by headline tags; several
slices report additional sub-findings inside each entry).

## Headline CRITICAL findings

- **slice-13 (Debian/APT path):** package indices never verified against signed InRelease (trust-chain gap); pure-Rust install path executes maintainer scripts / writes dpkg DB without root check or privilege escalation; pure `sync` downloads data nothing reads.
- **slice-19 (CI/install shell):** `ci-success` gate depends on a nonexistent `integration` job — main CI gate is invalid; PGO build script profiles a binary it never builds.
- **slice-27 (billing webhooks):** `customer.created` stores raw-case email → duplicate customer identities / lost entitlements; `resolveStripeCustomerId` can bind `undefined` → D1 throws → webhook retries forever.
- Plus one CRITICAL reported in most other slices (see each report's CRITICAL section; some slices recorded their worst issues as HIGH — treat both tiers as must-fix).

## Notable HIGH themes

- Rust CLI: interactive rollback requires `--yes` anyway; `poetry` scaffolding reports success on failure; transaction-ID prefix matching picks oldest match.
- Package managers: inconsistent shell-escaping and error handling across backends; AUR metadata trust assumptions.
- Daemon: cache/health thresholds vs configured totals mismatch.
- Web workers: license lookup returns arbitrary license for multi-license customers; rate-limit bypass when `API_RATE_LIMITER` binding is undefined; CORS helper ignores origin parameter and hardcodes credentials:true.
- Analytics correctness: visitor/session counters skewed (every pageview counts as new session); fabricated "Power Users"/churn/LTV metrics hardcoded on dashboards.

## Recommended triage order

1. Security trust-chain: InRelease verification, dpkg/maintainer-script root check, admin authz sweep (33).
2. Billing/webhook integrity (27) before it corrupts customer entitlements further.
3. CI validity (19) so future fixes are actually gated.
4. Rate limiting/CORS hardening (28, 30).
5. Correctness bugs by user impact (rollback flow, scaffold failures, analytics fiction).

---


---

# SLICE 01

# Audit slice-01: src/bin/, src/config/, src/lib.rs, fuzz/, benches/

Read-only audit of `~/Documents/omg` scope files. All line numbers refer to the working tree at audit time.

---

## CRITICAL

None.

## HIGH

### H-1. Index panic in elevated fast-path splitter (`sudo omg -- <pkg>` crashes)
- **File:** `src/bin/omg.rs:128`
- **Code:**
  ```rust
  let separator_pos = args.iter().position(|a| a == "--")?;
  if args[2..separator_pos]
      .iter()
      .chain(&args[separator_pos + 1..])
      .any(|arg| arg.starts_with('-'))
  ```
- **Why it is a bug:** If `--` is argv[1] (e.g. root/elevated invocation `omg -- pkg`, which happens with `sudo omg -- install foo` style quoting mistakes or shells that pass through `--`), then `command = args.get(1)` is `"--"`, `separator_pos == 1`, and the range `args[2..1]` is invalid — Rust panics with "slice index starts at 2 but ends at 1". The function is reached from `try_fast_elevated` before any clap parsing, so there is no graceful error; the privileged binary panics. Also, when `separator_pos == 1`, `command` is literally `"--"` which can never match a real command, so the whole invocation should simply fall through.
- **Fix:** Guard the split: `if separator_pos < 2 { return None; }` (or check `args.get(1) != "--"`), so such invocations defer to clap.

---

## MEDIUM

### M-1. History-recording failure masks a successful transaction in fast install/remove path
- **File:** `src/bin/omg.rs:273–289` (`record_fast_transaction`)
- **Code:**
  ```rust
  HistoryManager::new()?.finish_operation(kind, changes, result)
  ```
- **Why it is a bug:** If `HistoryManager::new()` fails (disk full, permissions), the `?` discards the already-computed transaction `result`. A package that was actually installed/removed successfully is reported as an error to the user (and exits non-zero). Contrast with `execute_fast_system_update` (lines ~96–104) which explicitly comments "never mask the transaction result with a recording error" — the package fast path does not follow its own rule.
- **Fix:** Log a warning on history failure and return the original `result`.

### M-2. Non-socket stale node is removed regardless of ownership
- **File:** `src/bin/omgd.rs:141–158`
- **Code:**
  ```rust
  Ok(meta) if { ... meta.file_type().is_socket() } => { /* uid check */ }
  Ok(_) => {
      tracing::debug!("Stale path {:?} is not a socket; removing", socket_path);
  }
  ```
- **Why it is a bug:** For sockets an owner check (uid must be self or root) refuses foreign objects, but for any other file type (regular file, fifo, symlink, directory?) placed by another user at the socket path, the daemon deletes it unconditionally. A local attacker who can create `/run/user/<uid>/omg.sock` as a symlink/fifo can have the daemon unlink/replace their object — inconsistent policy that defeats the stated intent ("refuse rather than delete").
- **Fix:** Apply the same uid-ownership requirement to every removable node, or refuse removal of any non-socket entry.

### M-3. TOCTOU between stale-node metadata check and `remove_file`
- **File:** `src/bin/omgd.rs:149–164`
- **Why it is a bug:** Between `std::fs::metadata(&socket_path)` and `std::fs::remove_file(&socket_path)`, an attacker with write access to the parent directory can swap the node (e.g. replace the checked socket with a symlink to another user's file). `remove_file` on a symlink removes the link itself (benign), but the general check-then-unlink race means the ownership attestation does not hold for the object actually removed. Low practical impact given `prepare_socket_parent` hardens the directory, but the comment claims safety that the code does not fully deliver.
- **Fix:** Use `fstatat`/`O_PATH|O_NOFOLLOW` open + `unlinkat`, or re-verify after unlink.

### M-4. Elevated `update`/`upgrade` fast path silently ignores trailing package tokens
- **File:** `src/bin/omg.rs:217`
- **Code:**
  ```rust
  "update" | "upgrade" => Some(execute_fast_system_update("")),
  ```
- **Why it is a bug:** `split_elevated_invocation` returns `(command, packages)`, and the contract documented for the fast path is "every token after `--` must be a package name". But the update arm discards `package_tokens` entirely and runs a full system upgrade. An invocation like `omg update -- somepkg` (or a mid-flow delegation that appends tokens) silently upgrades *everything* instead of erroring or honoring the argument list — surprising, potentially destructive behavior in a privileged path.
- **Fix:** Reject non-empty `packages` for update/upgrade (fall through to clap or bail with a clear message).

### M-5. Insecure-by-default AUR builds (`allow_unsafe_builds: true`)
- **File:** `src/config/settings.rs:150`
- **Code:**
  ```rust
  allow_unsafe_builds: true,
  ```
- **Why it is a bug:** The field name says "unsafe"; the default permits native AUR builds without sandboxing even though `build_method` defaults to `Native` and `secure_makepkg: true`. PKGBUILDs execute attacker-supplied build scripts (AUR is community content); defaulting to unsandboxed execution contradicts the "secure by default / pit of success" principle used elsewhere in this codebase (tight umask, socket uid checks).
- **Fix:** Default `allow_unsafe_builds: false` and require explicit opt-in, or rename/document so the default cannot be misread.

### M-6. Config `data_dir` and `socket_path` are never path-validated
- **File:** `src/config/settings.rs:236–247` (`validate_paths`)
- **Why it is a bug:** `validate_paths` only validates the four optional `aur.*` directories. The top-level `data_dir` and `socket_path` settings are accepted from the config file without traversal/null-byte checks, yet they drive where the daemon binds its IPC socket and where runtimes/state live. A malicious or typo'd `socket_path = "/var/run/omg.sock"` or `data_dir` containing `..` bypasses the validation regime applied everywhere else.
- **Fix:** Run `validate_config_path(&self.data_dir, "data_dir")` and `(&self.socket_path, "socket_path")` inside `validate_paths`.

### M-7. Absolute-path allowlist permits other users' home directories
- **File:** `src/config/settings.rs:24–40` (`validate_config_path`)
- **Code:**
  ```rust
  let is_safe = path_str.starts_with("/home/")
  ```
- **Why it is a bug:** Any absolute path under `/home/` is accepted, including other users' homes (`/home/alice/.ssh`, `/home/root`-style layouts). The tool writes into configured dirs (PKGDEST, ccache dir); a shared/maliciously-edited config could point writes outside the invoking user's home. The prefix string match also accepts `/homeX/...`? No (`"/home/"` requires slash), but it does accept `/home//evil`. Additionally the error message omits `/var/tmp/`, which the check actually allows — misleading diagnostics.
- **Fix:** Compare against the current user's home via `dirs::home_dir()`, and sync the error text with the actual allowlist.

### M-8. Exit status of privileged child not propagated after `run_self_sudo`
- **File:** `src/bin/omg.rs` (`async_main`, root-required branch, ~line 620)
- **Code:**
  ```rust
  omg_lib::core::privilege::run_self_sudo(&args_refs).await?;
  std::process::exit(0);
  ```
- **Why it is a bug:** After re-execing under sudo, the parent unconditionally `exit(0)`. Unless `run_self_sudo` both waits for the child and folds its status into its own `Result` (implementation outside this slice), scripts calling `omg sync` / `omg clean` see success even when the elevated operation failed — a UX/scripting-breaking defect (CI pipelines continue on failure). Even if `run_self_sudo` returns `Err` only on spawn failure, a sudo password prompt cancellation vs. command failure are indistinguishable.
- **Fix:** Ensure the wrapper propagates the child's exit code (e.g. `exit(code)` matching the child's status).

---

## LOW

### L-1. Lock file created 0600 but pre-existing wider permissions never tightened
- **File:** `src/bin/omgd.rs:229–240` (`claim_daemon_lock`)
- **Why:** `.mode(0o600)` only applies at creation. If `<sock>.lock` exists with looser perms (e.g., from an older version or manual touch), it is reused as-is, leaking the daemon PID file readability. Also the `.lock` file is never deleted (leftover in runtime dir; benign but clutter).
- **Fix:** `fchmod`/`set_permissions(0o600)` after open; optionally unlink on clean shutdown.

### L-2. `omg-fast` treats bare `s` as full status display
- **File:** `src/bin/omg-fast.rs:47–77` and usage header lines 1–12
- **Why:** Documented usage is `omg-fast s <query>`, but with no query, `cmd == "s"` falls into the `"status" | "s"` arm and prints the system status block. A user typing `omg-fast s` expecting search gets status output; also `search` alias is missing from this arm while `total/explicit/orphan/updates` aliases exist for counts but `s`/`i` behave inconsistently between `omg` (where `s` = search) and `omg-fast`.
- **Fix:** Print usage for bare `s` instead of status, or document the behavior.

### L-3. Protocol mismatch silently treated as success in `omg-fast`
- **File:** `src/bin/omg-fast.rs:135–137` and `168–170`
- **Code:** `Response::Success { .. } => {}`
- **Why:** If the daemon replies `Success` with an unexpected variant (version skew), the binary prints nothing and exits 0 — callers scripting `omg-fast i foo` cannot distinguish "info printed" from "protocol mismatch".
- **Fix:** Bail with an explanatory error on unexpected variants.

### L-4. Search result count vs displayed rows mismatch
- **File:** `src/bin/omg-fast.rs:126–131`
- **Why:** Prints `Found {res.total} packages:` then hard-takes 20 rows (`limit: Some(20)` and `.take(20)`); with >20 hits the message overstates what is shown with no hint.
- **Fix:** Print `Found {} (showing 20)` or honor total.

### L-5. `omg-fast info` allows `/` in package names
- **File:** `src/bin/omg-fast.rs:56–62`
- **Why:** The local validator permits `/` (and `@`) unlike the library's own `validate_package_name` fuzz invariant (which rejects leading `/` and `..`). Defense-in-depth relies entirely on the daemon re-validating; a hostile/compromised daemon peer is out of scope, but the two validators disagree, so the fuzz-tested invariant can drift from what the fast binary sends.
- **Fix:** Reuse `omg_lib::core::security::validate_package_name` here (cost is trivial) instead of a divergent inline rule.

### L-6. `..` substring rejection causes false-positive config rejections
- **File:** `src/config/settings.rs:29–31`
- **Why:** `path_str.contains("..")` rejects legitimate paths like `/home/u/pkg..cache` or relative `a..b/dest`. Traversal-safe normalization (components-based) would be precise; substring matching is both too strict (valid configs rejected) and, for symlink-heavy setups, not genuinely sufficient.
- **Fix:** Check path components for `ParentDir` instead of raw substring.

### L-7. `Settings::save` is not atomic
- **File:** `src/config/settings.rs:283–295`
- **Why:** `fs::write` truncates in place; a crash/power loss mid-write leaves a corrupt `config.toml` (which `load()` will then reject outright due to strict key/TTL validation). Standard fix: write temp file + `rename`.
- **Fix:** Write `config.toml.tmp` then atomically rename.

### L-8. `aur.build_concurrency` accepts 0 and unbounded values
- **File:** `src/config/settings.rs:139` / load validation (~line 258)
- **Why:** Only TTL is bounds-checked. `build_concurrency = 0` from config likely breaks semaphore/spawn logic downstream (default clamps to ≥1, config values do not); absurdly large values allow resource exhaustion.
- **Fix:** Validate `1 <= build_concurrency <= some_max` in `load()`.

### L-9. Fast explicit-count path ignores extra positional args
- **File:** `src/bin/omg.rs:335–349` (`try_fast_explicit_count`)
- **Why:** `omg explicit garbage --count` matches (`args[1]=="explicit"` plus `-c` anywhere) and prints the count from the status file while silently ignoring `garbage`; also `-c` is matched anywhere in argv including after `--`. Minor UX inconsistency vs clap's error reporting for the same input when the status file is absent.
- **Fix:** Require exact arg shape (no stray positionals) before taking the fast path.

### L-10. Fast-path success box overflow for long messages
- **File:** `src/bin/omg.rs:44–72` (`print_fast_success`)
- **Why:** The banner is fixed-width (41 dashes); action strings like `"installed"` fit, but the format `"{n} packages {action}!"` with multi-word actions or wide CJK package names overflows the border — cosmetic corruption of privileged-path output.
- **Fix:** Size the box from `msg.chars().count()`.

### L-11. System-update history records empty change set when snapshot fails
- **File:** `src/bin/omg.rs:83–90` (`execute_fast_system_update`)
- **Why:** `get_update_list().unwrap_or_default()` — if the pre-update snapshot errors (lock contention, db issue), the history entry is written with zero changes for a real upgrade, making `omg history`/rollback misleading. The `unwrap_or_default` also hides the snapshot error.
- **Fix:** At least log the snapshot error; consider marking the entry as "versions unknown".

### L-12. `has_json_flag` misses short/aliased forms
- **File:** `src/bin/omg.rs:326–329`
- **Why:** Only literal `--json` detected. If clap defines `-j` or accepts `--json=true`, a pre-parse fast path could emit human output where JSON was requested. Currently likely no `-j` exists, so informational/drift risk.
- **Fix:** Centralize flag detection next to the clap definition or add a test asserting the flag set.

### L-13. Fuzz target bypasses all config validation layers
- **File:** `fuzz/fuzz_targets/parse_config.rs:7–19`
- **Why:** It deserializes `Settings` via raw `toml::from_str`, never exercising `Settings::load`'s `validate_known_keys`, `validate_paths`, or TTL bounds — exactly the security-relevant parsing logic in scope. Panics/invariant violations in those validators are invisible to CI fuzzing.
- **Fix:** Fuzz a wrapper that calls the same validation pipeline (expose a `load_from_str` for tests).

### L-14. Benchmarks include `TempDir` creation/removal inside timed iterations
- **File:** `benches/io_bench.rs` (`bench_write_strategies`, `bench_async_io`, `bench_copy_strategies`, `bench_buffer_sizes`)
- **Why:** `TempDir::new().unwrap()` inside `b.iter(...)` measures tmpfs mkdir/rmdir alongside the intended write strategy, skewing small-size comparisons; async read benches also recreate identical fixtures per size unnecessarily.
- **Fix:** Create the temp dir per benchmark group, files per iteration.

### L-15. `pacman_comparison` benchmarks ignore output correctness
- **File:** `benches/pacman_comparison.rs:21–33`
- **Why:** `run_omg`/`run_pacman` discard stdout entirely; a future regression where `omg search` succeeds but prints nothing still "passes" the benchmark, producing meaningless comparisons. (Contrast `real_world_benchmark.rs`, which deliberately fails loudly.)
- **Fix:** Assert non-empty output like the real-world bench does.

### L-16. `real_world_benchmark` measures network-dependent operations
- **File:** `benches/real_world_benchmark.rs:46–66`
- **Why:** `apt-cache search python` and `omg search` may hit network-backed metadata/AUR; timings vary wildly across runs/environments, undermining the comparison's purpose. Informational measurement-validity concern.
- **Fix:** Pin to local-db-only queries or document variance.

### L-17. `aur_install_bench.sh` timing granularity and silent yay failure handling
- **File:** `benches/aur_install_bench.sh:57–88`
- **Why:** Uses second-resolution `date +%s.%N` (fine) but if `yay -S` fails mid-run under `set -e`, the script aborts leaving the previously installed test package removed and no summary; also `benchmark_install`'s output ordering mixes stdout (the duration) with stderr logs — fragile for `$(...)` capture if any tool prints to stdout before the duration line (e.g. pacman progress leaking through).
- **Fix:** Redirect installer stdout to stderr/dev-null inside `benchmark_install`; only echo the final duration to stdout.

---

## INFO

### I-1. `lib.rs` docs contain duplicated blank line / marketing numbers
- **File:** `src/lib.rs:8–14`
- **Why:** Hard-coded performance claims ("22x faster than pacman") rot quickly and duplicate benchmark intent; double blank line before `## Architecture`. Cosmetic/doc-hygiene.

### I-2. `config/mod.rs` module doc says "OMG" config but exports only two items
- **File:** `src/config/mod.rs:1–5`
- **Why:** Fine; noting the `mod settings;` is private with re-export — consistent. No action.

### I-3. Daemon ping kept for legacy compatibility
- **File:** `src/bin/omgd.rs:110–119`
- **Why:** Comment itself flags the ping as transitional ("once every daemon holds the claim, the lock alone decides"). Dead-ish code path scheduled for removal; track it.

### I-4. Error message in `finish` uses Debug formatting
- **File:** `src/bin/omg.rs` (`finish`, ~line 800)
- **Why:** `{error:?}` on anyhow prints the chain including backtrace-style sections; intentional per doc comment, but Debug output can expose internal paths in user-facing stderr more verbosely than Display. Acceptable; noted for consistency review.

### I-5. `ipc_messages` fuzz asserts bitcode round-trip determinism
- **File:** `fuzz/fuzz_targets/ipc_messages.rs`
- **Why:** Sound invariants; note that `bitcode` encoding determinism across crate versions isn't guaranteed, so cross-version daemons/clients could theoretically fail frame equality assumptions elsewhere. No defect here.

### I-6. Debian bench scenario tables duplicated conceptually across targets
- **File:** `benches/debian_common/mod.rs` (good) vs inline scenario vecs in `debian_core_bench.rs:39–63`
- **Why:** Some query lists remain inline rather than shared; maintenance duplication only.

### I-7. `daemon_benchmark.rs` cache-insert counter closure
- **File:** `benches/daemon_benchmark.rs:70–78`
- **Why:** `counter` increments inside `b.iter` (FnMut — compiles) so each iteration uses a fresh key; correct, just unusual pattern worth a comment.

### I-8. Windows stubs exit(1) with plain message
- **Files:** `src/bin/omgd.rs:277–281`, `src/bin/omg-fast.rs:213–217`
- **Why:** Correct and minimal; no finding.

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 1 |
| MEDIUM | 8 |
| LOW | 17 |
| INFO | 8 |
| **Total** | **34** |

Highest-priority fixes: H-1 (argv slice panic in privileged path), M-1 (success masked by history failure), M-4 (ignored package tokens in elevated update).


---

# SLICE 02

# Audit slice-02 — `src/cli/` core files (args.rs, commands.rs, help.rs, mod.rs, config.rs, init.rs, new.rs, migrate.rs)

Agent: audit02 · Read-only source audit. Line numbers approximate to file revision at audit time.

---

## HIGH

### H-1. Interactive rollback is impossible: `--yes` required even after interactive confirmation
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (~lines 838–860)
```rust
    if console::user_attended()
        && !Confirm::with_theme(&ui::prompt_theme())
            .with_prompt("Proceed with rollback?")
            .default(false)
            .interact()?
    {
        return Ok(());
    }

    // Non-interactive mode: require --yes flag for destructive operation
    if !yes {
        anyhow::bail!(
            "This destructive command requires --yes flag in non-interactive mode.\n\n\
```
**Why it is a bug:** The confirmation prompt runs first and returns `Ok(())` only when the user *declines*. If the user **accepts**, execution falls through to the unconditional `if !yes { bail! }` check, which aborts with a message about "non-interactive mode" even though the session *is* interactive and was just confirmed. An interactive user can never complete a rollback without also passing `--yes`, which makes the prompt dead weight and contradicts the error text ("Or run in interactive mode to select a transaction").
**Fix:** Gate the bail on non-interactivity: `if !yes && !console::user_attended() { anyhow::bail!(...) }`.

### H-2. `scaffold_python` reports success when poetry exits non-zero
**File:** `/home/pyro1121/Documents/omg/src/cli/new.rs` (~lines 216–250)
```rust
    let status = Command::new("poetry").args(["new", name]).status();
    pb.finish_and_clear();
    if status.is_err() { /* fallback */ }
    println!("  {} Created Python (Poetry) project", style::success("✓"));
```
**Why it is a bug:** `status` is a `Result<ExitStatus>`. `is_err()` only catches spawn failure (poetry missing). If poetry exists but fails (invalid name, no network, interrupted), the function prints "Created Python (Poetry) project" and returns `Ok(())` while nothing (or a partial directory) was created; subsequent `.tool-versions`/git-init steps then run against a broken target.
**Fix:** Match on both spawn error and `ExitStatus::success()`; fall back to venv scaffolding on either.

---

## MEDIUM

### M-1. Transaction-ID prefix match resolves ambiguously and prefers the oldest entry
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (~line 790)
```rust
        entries
            .iter()
            .find(|e| e.id.starts_with(&target_id))
            .context("Transaction ID not found")?
```
**Why it is a bug:** History entries are scanned oldest-first. A short prefix (the CLI itself displays only 8 chars via `short_id`) that matches multiple transactions silently selects the **oldest** match rather than the most recent, or errors ambiguously-differently than users expect. There is no ambiguity detection.
**Fix:** Collect all prefix matches; if >1, either pick the newest or refuse with "ambiguous ID".

### M-2. One unreadable pacman cache dir aborts the entire rollback restore
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (`find_cached_arch_package`, ~lines 700–720)
```rust
        std::fs::read_dir(cache_dir)
            .with_context(|| format!("Failed to read pacman cache: {}", cache_dir.display()))?;
        if let Ok(path) = find_cached_arch_package_in(cache_dir, package, version) {
            return Ok(path);
        }
```
**Why it is a bug:** The bare `read_dir(...)?` propagates an error immediately if any single configured cache dir is unreadable, instead of skipping to the next cache dir. Additionally the package is searched twice per dir (once here, once inside `find_cached_arch_package_in`) — wasteful duplicate directory scan. During a mid-transaction rollback this turns a recoverable situation into a hard failure.
**Fix:** Treat read failures as "skip this dir" (log + continue); drop the redundant pre-scan.

### M-3. Debian-path rollback silently drops AUR rebuild warnings
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (Restore arm, Debian branch ~lines 900–920)
```rust
                #[cfg(feature = "debian")]
                {
                    ...
                    println!("{}", style::success("✓ Rollback completed successfully"));
                    return Ok(());
                }
```
**Why it is a bug:** On the Debian path the function returns early without ever reporting `rebuild_from_aur` packages, unlike the Arch path which explicitly lists them. Users lose the record that those packages were not restored — silent partial rollback presented as full success.
**Fix:** Print the same "could not be downgraded automatically" listing before returning on the Debian branch.

### M-4. `migrate import` installs attacker-controlled package names with confirmation skipped
**File:** `/home/pyro1121/Documents/omg/src/cli/migrate.rs` (~line 200)
```rust
        if let Err(e) = crate::cli::packages::install(&to_install, true, false).await {
```
**Why it is a bug:** The manifest is an untrusted input file (parsed with serde from arbitrary JSON). Package names are passed straight into install with `yes=true` (skip confirm). A malicious/shared manifest can cause arbitrary package installation (supply-chain vector), including names that collide with typosquats. No allowlist validation of `original_name` / mapped names against safe-name rules occurs before install.
**Fix:** Validate each mapped name with `crate::core::security::validate_package_name`, and require explicit confirmation (or a flag) before installing from a manifest whose distro differs, or always show the final list and prompt unless `--yes` was explicitly given by the user.

### M-5. `migrate export` writes to an unvalidated arbitrary output path
**File:** `/home/pyro1121/Documents/omg/src/cli/migrate.rs` (~line 40)
```rust
    crate::core::safe_ops::atomic_write_file_sync(output, content)?;
```
**Why it is a bug:** `import` calls `safe_ops::validate_path` on its input, but `export` writes to whatever string is supplied with no path validation — inconsistent boundary handling allows overwriting arbitrary user-writable files (e.g. `~/.zshrc`) via `--output`.
**Fix:** Apply the same `validate_path` check to the export output path.

### M-6. Skipping recommended build settings downgrades `build_concurrency` to 1
**File:** `/home/pyro1121/Documents/omg/src/cli/init.rs` (`select_build_config`, ~lines 470–485)
```rust
    Ok(if applies {
        recommendation
    } else {
        BuildRecommendation {
            makeflags: String::new(),
            enable_ccache: false,
            enable_sccache: false,
            disable_secure_makepkg: false,
            build_concurrency: 1,
            explanation: Vec::new(),
        }
    })
```
and `apply_build_config` unconditionally does:
```rust
    settings.aur.build_concurrency = config.build_concurrency;
```
**Why it is a bug:** Choosing "Skip (use defaults)" overwrites whatever concurrency the user previously had with `1`, i.e. serial AUR builds — worse than any sensible default and not what "use defaults" implies.
**Fix:** When skipped, leave `settings.aur.build_concurrency` untouched (only apply fields when the recommendation is accepted).

### M-7. Hardcoded daemon path in generated systemd unit
**File:** `/home/pyro1121/Documents/omg/src/cli/init.rs` (`create_systemd_service`, ~lines 640–660)
```rust
ExecStart=%h/.local/bin/omgd --foreground
```
**Why it is a bug:** The unit assumes omgd lives in `~/.local/bin`, but the installer elsewhere resolves omgd relative to the running executable's directory (see `configure_daemon_startup`). If OMG was installed anywhere else (e.g. `~/.cargo/bin`, `/usr/bin`), the enabled service will crash-loop (`Restart=on-failure`) forever.
**Fix:** Write the resolved current_exe-based omgd path into the unit instead of `%h/.local/bin/omgd`.

### M-8. `HOME` fallback produces a literal `~` path that silently creates junk files
**File:** `/home/pyro1121/Documents/omg/src/cli/init.rs` (`install_shell_hook`, ~line 575)
```rust
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
    let config_path = shell.config_file().replace('~', &home);
```
**Why it is a bug:** If `HOME` is unset, the substituted path becomes `~/.zshrc` (a literal tilde directory name relative to cwd). The subsequent `OpenOptions::create(true).append(true)` then creates a bogus file named `~` in the current directory instead of failing or using a real home lookup (`dirs::home_dir()`).
**Fix:** Fail with a clear error (or use `std::env::home_dir()`/`dirs`) when `HOME` is missing rather than substituting `"~"`.

### M-9. Stale `FastStatus` snapshot trusted without freshness check
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (`read_status_snapshot`, ~lines 262–275)
```rust
    if let Some(fast) = crate::core::fast_status::FastStatus::read_from_file(
        &crate::core::paths::fast_status_path(),
    ) {
        return Ok((...));
    }
```
**Why it is a bug:** `omg status` returns whatever the cached binary blob says with no age validation and no vulnerability/runtime data (always `None`). After a package operation performed by another tool (pacman directly), status reports stale counts as current; security row flips to "Not scanned" even when the daemon has scanned data. At minimum the file mtime should be checked and stale snapshots rejected.
**Fix:** Validate snapshot age (e.g. < N minutes) before trusting; otherwise fall through to daemon/direct query.

### M-10. Completion context detection matches "tool"/"env" anywhere in the command line
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (`complete`, ~lines 40–43)
```rust
    let in_tool = full.split_whitespace().any(|token| token == "tool");
    let in_env = full.split_whitespace().any(|token| token == "env");
```
**Why it is a bug:** Any command line merely containing the word "tool" or "env" as an argument (e.g. `omg install env`, `omg remove tool-environment` won't match but `omg info tool` will) flips completion into the tool/env subcommand candidate set, producing wrong suggestions for package-name completion.
**Fix:** Parse positionally: only treat "tool"/"env" as context when it appears as the first token after `omg`.

---

## LOW

### L-1. `Run --parallel` documented as "(comma-separated)" but is a bool taking no value
**File:** `/home/pyro1121/Documents/omg/src/cli/args.rs` (~line 553)
```rust
        /// Run multiple tasks in parallel (comma-separated)
        #[arg(short, long)]
        parallel: bool,
```
Doc comment misleads users into typing `--parallel build,test`. Fix the doc or accept a value list.

### L-2. `Update --fast` and `--turbo` are mutually exclusive semantically but not declared so
**File:** `/home/pyro1121/Documents/omg/src/cli/args.rs` (~lines 118–130). Unlike `Doctor --turbo` (which uses `conflicts_with_all`), Update accepts `--fast --turbo` simultaneously; behavior depends on downstream precedence. Add `conflicts_with = "turbo"`.

### L-3. Dead alias branches in `new.rs` dispatch and `lock_runtimes`
**File:** `/home/pyro1121/Documents/omg/src/cli/new.rs` (~lines 24–31, 300+). Since `Commands::New` takes `value_enum ProjectStack`, `stack` is always one of rust/react/node/python/go; the aliases `"rs" | "react-ts" | "ts" | "py" | "golang"` in the match arms are unreachable dead code. Harmless but misleading; remove or handle uniformly.

### L-4. `npm create vite@latest` argument order deviates from documented usage
**File:** `/home/pyro1121/Documents/omg/src/cli/new.rs` (`scaffold_react`)
```rust
        .args(["create", "vite@latest", "--", name, "--template", "react-ts"])
```
Canonical form is `npm create vite@latest <name> -- --template react-ts`. Placing `<name>` after `--` makes it part of the initializer args; it currently works because Vite parses positionals, but it is fragile across npm versions. Also the exit status of the auto `npm install` failure is only warned for React but entirely unchecked for Node scaffold (`scaffold_node` runs `npm install .status()?` ignoring success).

### L-5. `go mod init` exit status unchecked
**File:** `/home/pyro1121/Documents/omg/src/cli/new.rs` (`scaffold_go`): `Command::new("go").args(["mod","init",name])...status()?` ignores whether it succeeded; a Go project without go.mod is still reported as created. Similarly `git init` output/status ignored in `run()`.

### L-6. TOCTOU between exists-check and scaffold creation
**File:** `/home/pyro1121/Documents/omg/src/cli/new.rs` (~lines 12–16): `target_dir.exists()` check then external scaffolders create the dir; a concurrent creator races. Minor for a local CLI; could use `create_dir` exclusive semantics where possible.

### L-7. `config validate` double-counts a broken config file as two issues and duplicates messaging
**File:** `/home/pyro1121/Documents/omg/src/cli/config.rs` (`validate`): a TOML syntax error increments `issues`, then the guaranteed-failing `Settings::load()` increments again — "Found 2 issue(s)" for one defect. Collapse into one diagnostic.

### L-8. `config reset` backup inherits original file permissions and keeps secrets-style content indefinitely
**File:** `/home/pyro1121/Documents/omg/src/cli/config.rs` (`reset`): backup copy preserves mode of the original (which validate warns may be loose, e.g. 0644) and is never cleaned up. Consider chmod 600 on backup.

### L-9. `history --json` serializes borrowed entries including internal fields; filtered header mismatch
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (`history`): header says "Transaction History (last {limit})" whenever no filters given even though `take(limit)` applies after reverse — correct, but with filters set the count shown is unaffected; cosmetic only. More notably, `filtered` is `Vec<&Transaction>` — JSON output shape differs from what a consumer deserializing `Vec<Transaction>` expects? (Serde handles references transparently, so INFO-level only.)

### L-10. Rollback of `Install` transactions removes packages without checking they are still installed / depended upon
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (`RollbackAction::Remove` path): blind `package_manager.remove(&packages)`; if a since-installed package now depends on one of them, removal semantics depend on the backend and may strip needed deps or fail mid-way. Consider checking reverse deps first (helper `local_reverse_deps` already exists in mod.rs).

### L-11. Wizard cannot be cancelled with Ctrl+C in raw-mode menus
**File:** `/home/pyro1121/Documents/omg/src/cli/init.rs` (`select_shell`, `select_daemon_startup`, `select_binary_menu`): raw mode disables SIGINT; only `q` quits. Ctrl+C arrives as `KeyCode::Char('c')` with CONTROL modifier and hits the `_ => {}` arm. Users accustomed to Ctrl+C get stuck. Handle `KeyCode::Char('c') if key.modifiers.contains(CONTROL)` → bail.

### L-12. Hard-coded step numbering in init wizard conflicts with skip flags
**File:** `/home/pyro1121/Documents/omg/src/cli/init.rs`: prompts print fixed "Step 1/5 … Step 5/5", but with `--skip-shell`/`--skip-daemon` the displayed numbering skips numbers (jumps from Step 3 to Step 4 with earlier steps hidden). Cosmetic UX inconsistency.

### L-13. `finish_apply` references a nonexistent "unmapped list above"
**File:** `/home/pyro1121/Documents/omg/src/cli/migrate.rs` (~line 210):
```rust
    println!("  Some packages may need manual installation - check the unmapped list above.");
```
No unmapped list is ever printed (identity mappings are silently kept). Either track and print genuinely unmapped packages or drop the sentence.

### L-14. Daemon readiness loop leaks spawned child handling / no zombie reaping concerns & fixed 3s budget
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (`daemon`): the spawned omgd child handle is dropped without wait (fine post-exit), but on slow systems (>3s index build) the CLI reports failure ("socket not created") while omgd later starts successfully — user retries and gets confusing "already running"/failure mix. Consider longer backoff or report "starting in background".

### L-15. `metrics` treats any non-Metrics response as generic error
**File:** `/home/pyro1121/Documents/omg/src/cli/commands.rs` (`metrics`): `_ => anyhow::bail!("Unexpected response from daemon")` discards the actual response variant which would aid debugging (e.g. Error payload). Include the variant in the message.

### L-16. Shell hook dedup check too narrow
**File:** `/home/pyro1121/Documents/omg/src/cli/init.rs` (`install_shell_hook`): dedup tests `content.contains("omg hook")`; a commented-out or unrelated line containing "omg hook" suppresses installation, and conversely the daemon-starter block (`pgrep -x omgd ...`) added under `start_daemon=true` is never revisited if the plain hook already existed — toggling that preference later has no effect.

---

## INFO

### I-1. `help.rs` hardcodes marketing claims and duplicated URL strings
`22x faster`, `6ms searches` etc. embedded in help text will drift from reality; also two different docs URLs appear (`https://pyro1121.com/docs`). The `_cli` parameter of `print_essential_help` is unused.

### I-2. `mod.rs` `local_reverse_deps` scans all local pkgs linearly
Fine for correctness; note for future scale (no index).

### I-3. `parse_timestamp_opt`/`format_short_timestamp` well-guarded
Good fail-closed design; no issue.

### I-4. `normalize_transaction_id` accepts hex prefixes length 1–32 plus UUIDs; empty and traversal inputs correctly rejected (tested).
Positive note; no change needed.

### I-5. `migrate.rs` version pinning (`MANIFEST_FORMAT_VERSION == "1.0"`) correctly rejects forward versions — good practice.

### I-6. `config.rs` MAKEFLAGS character allowlist correctly excludes shell metacharacters; bounds-checked `build_concurrency` (0 and >128 rejected) — sound.

### I-7. `init.rs` `RawModeGuard` RAII correctly restores tty state on all exit paths including `?` propagation — good.

### I-8. `daemon()` deliberately never unlinks a stale socket (documented security rationale) — consistent with omgd-side ownership checks; no action.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH     | 2 |
| MEDIUM   | 10 |
| LOW      | 16 |
| INFO     | 8 |

Most impactful items: H-1 (interactive rollback always fails without `--yes`), H-2 (false success on poetry failure), M-4/M-5 (untrusted manifest → unconfirmed installs; unvalidated export path), M-6 (skip-recommendations degrades concurrency to 1), M-7 (broken systemd unit path).


---

# SLICE 03

# Audit slice-03 — CLI: doctor, env, run, self_update, outdated, why, size, blame, diff

Auditor: audit03 · Read-only source audit of `/home/pyro1121/Documents/omg/src/cli/{doctor.rs,env.rs,run.rs,self_update.rs,outdated.rs,why.rs,size.rs,blame.rs,diff.rs}` (2,702 lines).

---

## HIGH

### H-1. Self-update breaks when temp dir and binary live on different filesystems (EXDEV)
- **File:** `src/cli/self_update.rs:86–102`
```rust
let temp_dir = tempfile::tempdir().context("Failed to create temp directory for update")?;
...
let backup_path = current_exe.with_extension("old");
fs::rename(&current_exe, &backup_path).context("Failed to backup current binary")?;

match fs::rename(&new_binary, &current_exe) {
```
- **Why it is a bug:** `tempfile::tempdir()` honors `TMPDIR` and defaults to `/tmp`, which is commonly a tmpfs while `~/.local/bin`/`/usr/bin` is a disk filesystem. `fs::rename` cannot move across filesystems (`EXDEV`). Worse, the code first renames the *running* binary to `.old`; when the cross-device rename of the new binary fails, every update run aborts after having already moved the executable away. The restore path exists but the update can never succeed on such systems — a deterministic functional failure, not a rare race.
- **Fix:** Copy the new binary into the target directory first (e.g. write `current_exe` + `.tmp` sibling in the same directory via `fs::copy`), fsync, then do same-directory renames. Only fall back across devices deliberately.

### H-2. `omg diff` hint suggests an invalid command (`env sync` takes a Gist ID, not a lockfile path)
- **File:** `src/cli/diff.rs:140`
```rust
println!("       {}", style::command(&format!("omg env sync {to}")));
```
- **Why it is a bug:** In `src/cli/env.rs:186–212`, `EnvCommands::Sync { url }` treats its argument as a GitHub Gist URL/ID and builds `https://api.github.com/gists/{gist_id}`. Passing a local lockfile path like `omg.lock.backup` produces a request to `https://api.github.com/gists/omg.lock.backup` which will 404. The user-facing guidance printed at the end of `omg diff` is therefore always wrong for the file-based diff use case.
- **Fix:** Point users at the correct workflow (e.g. "share the target lockfile with `omg env share`, then `omg env sync <gist-url>`"), or add a local-file sync mode.

### H-3. EOL "within 6 months" warning never fires — jiff rejects month spans on `Timestamp`
- **File:** `src/cli/doctor.rs:210–216`
```rust
let six_months = jiff::Span::new().months(6);
if let Ok(warning_ts) = now.checked_add(six_months)
    && warning_ts > eol_timestamp
{
    eol_warning = Some(format!("EOL on {eol_date}"));
}
```
- **Why it is a bug:** `jiff::Timestamp::checked_add` with a span containing calendar units greater than hours returns `Err` (`smallest_non_time_non_zero_unit_error` in jiff's `Timestamp::checked_add_span`; verified against jiff 0.2.x sources). `if let Ok(...)` silently swallows the error, so the proactive "EOL on …" branch is dead code and doctor only ever reports already-EOL runtimes.
- **Fix:** Convert to a `Zoned` first (`now.to_zoned(TimeZone::UTC)?.checked_add(six_months)`), or compare using a fixed-day approximation (`Duration::hours(24 * 183)`).

---

## MEDIUM

### M-1. `check_path` uses substring matching instead of PATH-entry matching
- **File:** `src/cli/doctor.rs:349–360`
```rust
return path.contains(parent.to_str().unwrap_or(""));
```
- **Why it is a bug:** `PATH.contains(dir)` is a raw substring test, so `/home/u/bin` matches PATH `/home/u/bin-extra:...` (false positive) and any failure to stringify the exe parent yields `contains("") == true` (always-pass). It also reports failure if omg is reachable through a different PATH entry than `current_exe()`'s parent (false negative).
- **Fix:** Split `PATH` on `:` and check component-wise equality with the exe's parent directory; treat empty needle as failure.

### M-2. `omg env sync` fetches attacker-chosen `raw_url` without host validation (SSRF / content substitution)
- **File:** `src/cli/env.rs:234–240`
```rust
} else {
    // Fetch raw if content is truncated/missing in metadata
    client.get(&file.raw_url).send().await?.text().await?
};

std::fs::write("omg.lock", content).context("Failed to write omg.lock from Gist")?;
```
- **Why it is a bug:** The gist JSON returned by the API is untrusted input for a *sync* operation; `file.raw_url` is used verbatim as the fetch target. A malicious/compromised gist (or MITM of the API response) can redirect the client to any URL, and whatever bytes come back overwrite the user's local `omg.lock`. There is no integrity verification (the lockfile's own stored hash is inside the fetched content itself, so it provides no authenticity).
- **Fix:** Validate `raw_url` starts with `https://api.github.com/` or `https://gist.githubusercontent.com/`, and confirm with the user before overwriting an existing `omg.lock`.

### M-3. `omg env sync` destroys the existing local lockfile before validating downloaded content
- **File:** `src/cli/env.rs:239–245`
```rust
std::fs::write("omg.lock", content).context("Failed to write omg.lock from Gist")?;
...
// Auto-check
check().await?;
```
- **Why it is a bug:** The remote content is written over `omg.lock` unvalidated. If the payload is corrupt/malformed TOML, `EnvironmentState::load` inside `check()` then fails, and the user's original lockfile is gone (no backup). Data loss caused by an unvalidated external write.
- **Fix:** Parse into `EnvironmentState` from the in-memory string *before* writing; keep the previous file until validation succeeds.

### M-4. Non-success HTTP statuses in network diagnostics are not counted as issues
- **File:** `src/cli/doctor.rs:143–155`
```rust
if status.is_success() || status.is_redirection() {
    println!("  {} {} ({} ms)", ...);
} else {
    println!("  {} {} (HTTP {})", style::warning("⚠"), name, status.as_u16());
}
```
- **Why it is a bug:** Connection errors and timeouts increment `issues`, but a mirror answering HTTP 500/403 does not — inconsistent logic means `omg doctor --network` can print warnings yet conclude "System is healthy!" (or miscount total issues).
- **Fix:** Increment `issues` (or track a separate `warnings` counter reported in the summary) for non-2xx/3xx responses too.

### M-5. `UpdateType::Unknown` packages are counted in totals but displayed nowhere
- **File:** `src/cli/outdated.rs:59–110` (classification at 161–164)
- **Excerpt:** filters only match `Major`/`Minor`/`Patch`; `Unknown` packages appear in none of the three cards, yet `"{} packages total"` uses `outdated.len()`.
- **Why it is a bug:** For non-semver versions (common with e.g. pacman epochs like `1.2.3-4` depending on `from_versions` semantics, or VCS packages), affected updates are silently invisible while the summary claims N total packages. Summary counts (major+minor+patch) won't sum to total, confusing users who then run `omg update` blind.
- **Fix:** Add an "Unknown version format" card listing non-classified entries, or exclude them from the total with a note.

### M-6. Blocking DNS resolution inside async context
- **File:** `src/cli/doctor.rs:169–180`
```rust
match std::net::ToSocketAddrs::to_socket_addrs(&format!("{host}:443")) {
```
- **Why it is a bug:** `to_socket_addrs` is synchronous blocking I/O executed directly inside `async fn check_network()` on a tokio worker thread; a hanging resolver stalls the runtime thread (and the whole doctor run, since there is no timeout around it, unlike the HTTP checks' 5 s timeout).
- **Fix:** Wrap in `tokio::task::spawn_blocking` with a timeout, or use a resolver API that supports deadlines.

### M-7. `run --watch --parallel`: flags silently conflict, watch wins
- **File:** `src/cli/run.rs:14–26`
```rust
if self.watch {
    task_runner::run_task_watch(&self.task, &self.args, backend)?;
} else if self.parallel {
```
- **Why it is a bug:** If both `--watch` and `--parallel` are passed, `parallel` is silently ignored — no error, no warning. Additionally `run_task_watch` is a blocking call awaited directly in async `execute` (same blocking-runtime concern as M-6 if it loops internally).
- **Fix:** Reject the combination at clap level or emit a warning; spawn blocking work via `spawn_blocking` where appropriate.

### M-8. Turbo-mode capabilities are silently destroyed by self-update
- **File:** `src/cli/doctor.rs:395–440` (setcap on `current_exe`) interacting with `src/cli/self_update.rs:99–105`
- **Why it is a bug:** File capabilities (`cap_dac_override,cap_fowner,cap_chown+ep`) are extended attributes on the inode. Self-update replaces the binary via `fs::rename` of a freshly extracted file; xattrs are not carried over, so after every successful `omg self-update`, turbo mode is gone with zero notification — package operations start prompting for sudo again (or failing) until the user re-runs setcap.
- **Fix:** After replacing the binary, detect prior capabilities and either copy them (requires privileges — likely unavailable post-sudo) or print a prominent "re-run `omg doctor turbo`" notice.

### M-9. Gist "private" sharing mislabels secret gists and leaks full environment inventory
- **File:** `src/cli/env.rs:150–196`
- **Why it is a bug:** GitHub secret gists (`public: false`) are readable by anyone with the URL. `omg env share --public false` uploads the complete installed-package/runtime inventory (potentially revealing internal tooling, hostname-derived data if present in the lockfile schema) under a URL that is effectively public-by-obscurity. The UI prints "Visibility: Private", overstating confidentiality. No confirmation prompt before upload.
- **Fix:** Warn that secret gists are URL-accessible; consider requiring explicit confirmation before uploading environment inventories.

### M-10. `description.len()` limits bytes, not characters
- **File:** `src/cli/env.rs:119`
- **Why it is a bug:** `.len()` is UTF-8 byte length; a 400-character CJK description (~1200 bytes) is rejected despite being well under a sane character limit, and vice-versa the message says "characters". Minor correctness/UX mismatch.
- **Fix:** Use `description.chars().count()` if the limit is meant to be characters.

---

## LOW

### L-1. `check_shell_hook` is a stub that always reports success
- **File:** `src/cli/doctor.rs:362–370`
```rust
const fn check_shell_hook() -> bool { ... true }
```
- **Why it is a bug:** Doctor prints "Shell hook active" unconditionally — the check verifies nothing (its own comments admit this). Users with broken hooks get a green ✓. Dead/broken check presented as real diagnostics.
- **Fix:** Either implement a real probe (e.g. check `$OMG_SHELL_HOOK` style env markers the hook sets, or scan shell rc files) or remove the check from output.

### L-2. `parse_version` strips repeated leading `v`s
- **File:** `src/cli/self_update.rs:229–233` — `trim_start_matches('v')` removes all leading 'v' chars, accepting `"vvv1.2.3"`. Harmless in practice but sloppy input handling for the release-feed boundary. Fix: strip at most one prefix via `strip_prefix('v').unwrap_or(trimmed)`.

### L-3. Checksum sidecar comes from the same server as the archive — TOFU only
- **File:** `src/cli/self_update.rs:60–63, 279–303`
- **Why it is a weakness:** The SHA-256 gate protects against truncated/corrupt downloads, but both digest and artifact are fetched from `releases.pyro1121.com` over TLS-only trust; compromise of that host yields validly-signed-by-position malicious updates. Doc comment ("fails closed") slightly oversells the guarantee. Consider minisign/sigstore signatures with an offline key. (Design observation, not an exploitable defect in code shown.)

### L-4. `size` no-backend bail message contradicts compiled-feature reality and tested behavior
- **File:** `src/cli/size.rs:31–37`
```rust
#[cfg(not(feature = "arch"))]
{ let _ = (tree, limit); anyhow::bail!("size command requires the arch feature"); }
```
- **Why it is a bug:** When compiled with debian features but not arch, running on Arch (or anywhere the debian dispatch doesn't trigger) yields "requires the arch feature" even though a Debian backend exists; meanwhile the cfg(test) helper asserts the message "not available without an Arch or Debian package backend", which is never produced by `run()` — the test validates dead code, not the real path. Fix: unify the fallback message and test the actual function.

### L-5. `get_cache_size` counts only top-level directory entries
- **File:** `src/cli/size.rs:357–377`
- **Why it is a bug:** `fs::read_dir` without recursion misses subdirectories' contents (pacman caches are flat, but other cache dirs returned by `pacman_cache_dirs()` may nest); reported cache size can materially undercount. Also `metadata.len()` on directories is meaningless. Fix: recursive walk or note the approximation.

### L-6. `limit = 0` renders "Top 0 Packages" with an empty card
- **File:** `src/cli/size.rs:57–62, 168–181`
- **Why it is a bug:** No clamping/validation of `limit`; `--limit 0` produces a nonsense header plus empty card instead of an error or default. Fix: validate `limit >= 1` at the CLI layer.

### L-7. Dependency-path BFS labels the root node "explicit" without checking install reason
- **File:** `src/cli/why.rs:253–289` (`build_dependency_path`)
```rust
result.push((format!("└─ {p}"), "explicit".to_string()));
```
- **Why it is a bug:** The BFS root is `required_by.first()` — merely the first dependent found by `local_reverse_deps`, which may itself be installed as a dependency. Labeling it "explicit" can present false information in the "Dependency Path Example". Also BFS is unbounded (fine for local DB size, but no depth cap).
- **Fix:** Look up the root package's actual reason before labeling; optionally bound depth.

### L-8. `blame` assumes history is ascending by time when showing "most recent 10"
- **File:** `src/cli/blame.rs:74–77` — `relevant.iter().rev().take(10)`
- **Why it is a bug:** Correctness depends on `HistoryManager::load()` returning transactions oldest-first; if ordering ever differs (merged histories, manual edits), the shown window is arbitrary while counts claim recency. Fragile implicit contract; sort by timestamp explicitly.

### L-9. ANSI styling baked into stored strings in blame
- **File:** `src/cli/blame.rs:139–147` — reason strings pass through `style::version(...)`/`style::path(...)` before being placed in `kv_list` values, unlike `why.rs` which stores plain strings. Produces double-styling/inconsistent rendering and embeds escapes in non-tty output. Fix: store plain text, style at render.

### L-10. `sync` accepts trailing-empty gist id from URLs ending in `/`
- **File:** `src/cli/env.rs:205–209` — `".../gists/123/"` yields empty id → request to `/gists/` → confusing 404 rather than a validation message. Also query fragments (`#...`) are carried into the API path. Fix: trim and percent-validate the extracted segment.

---

## INFO

### I-1. EOL table lacks entries for currently-common versions
- **File:** `src/cli/doctor.rs:14–32` — no Node 20/22, Python 3.10–3.12, Go 1.21+, Ruby 3.1+, so those runtimes always show ✓ regardless of actual EOL state; `bun` is probed (`runtimes` list line 187) but has zero table entries, making the probe pointless. Stale hardcoded table will silently rot.

### I-2. Prefix matching in EOL check can over-match
- **File:** `src/cli/doctor.rs:198–201` — `version.starts_with(ver_prefix)`: e.g. hypothetical `rust "1.7"` would match prefix `"1.70"`. Currently harmless with the given table; use parsed major/minor comparison instead of string prefixes.

### I-3. `check_daemon` mixes diagnostic printing into a boolean predicate
- **File:** `src/cli/doctor.rs:383–447` — detailed hints printed inside the checker make the function non-composable and duplicate formatting responsibilities; cosmetic architecture issue.

### I-4. Progress bar shows total 0 when Content-Length absent
- **File:** `src/cli/self_update.rs:316` — `ProgressBar::new(response.content_length().unwrap_or(0))`; cosmetic misleading progress for chunked responses.

### I-5. Backup `.old` file can persist if process dies between the two renames
- **File:** `src/cli/self_update.rs:99–130` — acknowledged in code comment as harmless; noting for completeness (a crash leaves `<exe>.old` behind).

### I-6. `enable_turbo_mode` grants broad capabilities (dac_override/fowner/chown) to the entire omg binary
- **File:** `src/cli/doctor.rs:415–417` — by design, but any code-execution bug in omg thereafter runs with near-root file power; worth documenting in security docs. Not counted as a defect since it is the feature's stated purpose.

### I-7. `why`/`size` Debian dispatch happens before package-name-independent distro detection each call — duplicated `is_debian_like()` checks across functions
- **Files:** `src/cli/blame.rs:126–131, 199–204`; `src/cli/why.rs:24–29` — repeated per-call detection; consistency hazard if one call site forgets the dispatch (currently consistent). Informational.

---

## Summary
| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 3 |
| MEDIUM | 10 |
| LOW | 10 |
| INFO | 7 |
| **Total** | **30** |


---

# SLICE 04

# Audit slice-04 — CLI: security, enterprise, fleet, team, telemetry, license, ci, container, daemon_status, git_hooks, snapshot, man, style, ui, modern_ui, tool, runtimes, workspace

Scope: `~/Documents/omg/src/cli/{security,enterprise,fleet,team,telemetry,license,ci,container,daemon_status,git_hooks,snapshot,man,style,ui,modern_ui,tool,runtimes,workspace}.rs` (~8.9k lines). READ-ONLY audit; no builds executed.

---

## HIGH

### H-1. Fleet push sends lockfile and machine data with no authentication
- File: `src/cli/fleet.rs:120-140`
```rust
let push_result = crate::core::http::shared_client()
    .post("https://api.pyro1121.com/api/fleet/push")
    .json(&serde_json::json!({
        "team": target,
        "message": msg,
        "lock_content": lock_content,
        "machine_count": count
    }))
```
- Why it's a bug: Every other server interaction in the codebase authenticates via `licensed_get(...)` / `.bearer_auth(&license.key)` (see `src/core/license.rs:640-660`). The fleet push sends the full `omg.lock` content (a fingerprint of the user's environment) plus team identifier with no bearer token and no license key in the body. The server cannot attribute or authorize the push; any client can POST to this endpoint claiming any team, and sensitive environment data is transmitted without an ownership binding. Also a broken feature: pushes can never be associated with the caller's fleet.
- Fix: require license and use `.bearer_auth(&license.key)` (or include the license key in the signed body), mirroring `license::propose_change`.

### H-2. Enterprise audit export fabricates an access-control matrix
- File: `src/cli/enterprise.rs:293-300`
```rust
fn generate_access_control_csv() -> String {
    let mut csv = "user,role,scope,permissions\n".to_string();
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let _ = std::fmt::write(&mut csv, format_args!("{user},owner,global,all\n"));
```
- Why it's a bug: This file is produced by `omg enterprise audit-export` whose output is explicitly labeled "Ready for auditor review" (`enterprise.rs:118`). It invents a record asserting the current user is `owner` with `all` permissions on scope `global`, regardless of reality. Submitting fabricated access-control evidence in a SOC2/ISO27001 audit is a compliance-integrity failure (and the codebase elsewhere deliberately removed similar fabrication, e.g. `security.rs export_compliance` bails for unimplemented frameworks).
- Fix: either fetch real role data from the identity/licensing API or bail with "access-control evidence not implemented", like `export_compliance` does for non-SOC2 frameworks.

### H-3. `omg enterprise reports` embeds unvalidated `report_type` in output filename (path injection)
- File: `src/cli/enterprise.rs:44-50`
```rust
let report = generate_report(report_type).await?;
let filename = format!("omg-report-{}-{}.json", report_type, ...);
fs::write(&filename, &content)?;
```
- Why it's a bug: `report_type` comes straight from the CLI (`EnterpriseCommands::Reports { report_type }`) and is never validated (unlike sibling commands that validate framework/scope/period). A value such as `../../evil` writes outside the working directory, and `/abs/path` writes to an absolute path. Unlike `audit_export` (which calls `validate_relative_path(output)`), there is zero validation here. Additionally, `generate_report` accepts *any* string as report type — the six section names shown to the user ("Executive Summary", ...) are static decoration unrelated to the requested type, so the artifact misrepresents its content.
- Fix: validate `report_type` against an allowlist (executive/compliance/adoption/cost), and pass the filename through `crate::core::security::validate_relative_path` semantics before writing.

## MEDIUM

### M-1. Generated GitHub Actions advanced workflow cannot run: Rust never installed on non-container jobs and `container:` is undefined for most matrix legs
- File: `src/cli/ci.rs:157-238` (advanced template)
```yaml
matrix:
  os: [ubuntu-latest]
  features: ["arch", "debian", "license,pgp", "arch,debian,license,pgp"]
  include:
    - os: ubuntu-latest
      container: archlinux:latest
      features: "arch"
container: ${{ matrix.container }}
...
- name: Lint
  run: |
    cargo fmt --check
```
- Why it's a bug: (a) Only the `include` leg defines `container`; the other three matrix legs expand `container: ${{ matrix.container }}` to an empty expression, which GitHub rejects/errors at job setup. (b) The `Lint`, `Build`, `Test` steps run `cargo` but the toolchain-install step is gated on `if: matrix.container == 'archlinux:latest'` and only installs inside the Arch container; on the three non-container legs no rustup/dtolnay action ever runs, so every job fails with `cargo: command not found`. The generated "CI configuration" is dead-on-arrival.
- Fix: give each matrix leg an explicit `container:` value (or drop job-level container), and install Rust unconditionally (e.g. `dtolnay/rust-toolchain@stable`) except in the Arch container.

### M-2. Snapshot restore switches runtimes *before* the user confirms package changes
- File: `src/cli/snapshot.rs:196-260`
```rust
// Switch runtimes
for (runtime, _, target_ver) in &runtime_changes {
    println!("    Switching {runtime} to {target_ver}...");
    crate::cli::runtimes::use_version(runtime, Some(target_ver)).await?;
}
... later:
if !confirm { println!("  {} Package changes skipped", ...); return Ok(()); }
```
- Why it's a bug: The confirmation prompt text says "Do you want to apply these package changes?" but by the time it is asked, runtime versions have already been mutated irreversibly (the old versions aren't recorded/restorable from this flow). Declining does not restore pre-command state, so the user's answer does not control what actually happened. In the worst case a user previewing a risky runtime downgrade and then answering "No" still gets downgraded.
- Fix: compute the full change set, prompt once covering both runtimes and packages (defaulting to No), and only mutate after confirmation; treat runtime switch failures as abort-before-package-changes.

### M-3. Byte-slice of externally-supplied `machine_id` can panic on non-ASCII input
- File: `src/cli/team.rs:265-270`
```rust
member_list.push(format!(
    "{} {} ({})",
    sync_icon, hostname,
    &member.machine_id[..8.min(member.machine_id.len())]
));
```
- Why it's a bug: Slicing at byte offset 8 panics if index 8 falls inside a multi-byte UTF-8 char. `machine_id` comes from the remote API response (`license::fetch_team_members`), i.e. untrusted data; a malformed/hostile server response crashes the whole `omg team members` command. The same pattern exists in `snapshot.rs:151` (`&snap.hash[..8.min(...)]`, lower risk since locally generated hex) and `container.rs` id slicing (`&c.id[..12.min(c.id.len())]`).
- Fix: use `machine_id.chars().take(8).collect::<String>()` (as done correctly in `team.rs extract_team_id`).

### M-4. `omg audit fix` silently treats invalid/typo'd `--min-severity` as "medium"
- File: `src/cli/security.rs:969-974`
```rust
let min_sev = match min_severity.to_lowercase().as_str() {
    "critical" => 9.0,
    "high" => 7.0,
    "low" => 0.0,
    _ => 4.0, // Default to medium or unknown
};
```
- Why it's a bug: A typo like `--min-severity critcal` silently widens the threshold to medium instead of failing. For a command that upgrades system packages, silent reinterpretation of a safety threshold is dangerous (e.g. `--min-severity high `with trailing space → 4.0, pulling in more upgrades than intended). Elsewhere the codebase validates enum-ish inputs explicitly (`read_audit_entries` bails on invalid severity).
- Fix: `_ => anyhow::bail!("Invalid min-severity '{min_severity}'. Valid: low, medium, high, critical")`.

### M-5. `omg tool remove` deletes ALL dangling symlinks in the shared bin dir, including other tools'
- File: `src/cli/tool.rs:551-573`
```rust
for entry in entries {
    let path = entry?.path();
    if let Ok(target) = fs::read_link(&path)
        && !target.exists()
    {
        fs::remove_file(&path)?;
        ...
    }
}
```
- Why it's a bug: After removing one managed tool, the cleanup sweep removes every broken symlink in `~/.local/share/omg/bin`, including links belonging to tools that are still installed but whose target is temporarily unavailable (e.g. network-mounted home, partially updated cargo root, symlink chains through another removed version dir). It also removes broken links the *user* created manually in that directory. Removal should be limited to links whose target path was under the just-deleted `install_dir`.
- Fix: compare the read link target against the removed `tools/<manager>/<name>` prefix and only delete matching links.

### M-6. Pacman-registry tools leave stale empty isolation dirs and are invisible to `tool update all` bookkeeping
- File: `src/cli/tool.rs:388-397` and `installed_tool_names()` (`tool.rs:117-155`)
```rust
"pacman" => {
    pb.finish_and_clear();
    return crate::cli::packages::install(&[pkg.to_string()], false, false).await;
}
```
- Why it's a bug: `install_managed` creates `tools/pacman/<pkg>` (created at line ~370 via `fs::create_dir_all(&install_dir)`) before dispatch; the pacman branch returns early leaving a permanent empty directory that pretends the tool is omg-managed. `omg tool list` won't show it (list reads bin-dir symlinks; pacman installs to /usr/bin so nothing is linked into bin_dir either), yet `remove("ripgrep")` finds the empty dir, prints "Removing ripgrep from pacman...", deletes nothing meaningful, and claims "Removal complete" while the global pacman package remains installed.
- Fix: don't create the isolation dir for the pacman branch, and make `remove` detect pacman-sourced tools and offer/delegate to `pacman -R`.

### M-7. `omg man generate` fallback creates a literal `~` directory in the CWD
- File: `src/cli/man.rs:22-25`
```rust
dirs::data_dir()
    .unwrap_or_else(|| PathBuf::from("~/.local/share"))
    .join("man").join("man1")
```
- Why it's a bug: When `dirs::data_dir()` returns `None` (e.g. `XDG_DATA_HOME` unset and no home resolution), the literal relative path `./~/.local/share/man/man1` is created — a directory literally named `~` in the current working directory. The tilde is never expanded here (unlike the user-supplied branch which calls `shellexpand`). Silent wrong-location write + filesystem clutter.
- Fix: fall back to `std::env::var("HOME")` + expansion, or error out when the data dir cannot be determined.

### M-8. `check_eol` reports unsupported runtimes (rust/bun) as green "Active" with EOL "Unknown"
- File: `src/cli/security.rs:1139-1215`
```rust
let runtimes = ["node", "python", "rust", "go", "ruby", "java", "bun"];
...
// no eol_data rows exist for rust or bun
println!("  {} {} v{} - {} (EOL: {})", ..., style::success(status), ...);
```
- Why it's a bug: For `rust` and `bun` (and any runtime missing from the hardcoded table, e.g. future node 24/python 3.14), the loop finds no matching row, leaves `status="Active"` and `eol_date_str="Unknown"`, and prints a green ✓ "Active (EOL: Unknown)". Presenting an unverifiable EOL claim as a success checkmark defeats the purpose of an EOL audit and trains users to ignore warnings. Also the whole table is hardcoded (comment admits last reviewed 2026-02) — node 20 EOLs 2026-04-30 and python 3.10 late 2026 will silently go stale.
- Fix: when no row matches, print a neutral "Unknown — no EOL data" line without success styling (or skip), and surface table staleness (e.g. warn when now exceeds newest embedded date).

### M-9. `omg audit log --severity X` ignores `--limit`
- File: `src/cli/security.rs:20-40` (`read_audit_entries`)
```rust
if let Some(sev) = severity_filter {
    ...
    logger.filter_by_severity(min_severity)   // limit never applied
} else {
    logger.get_recent(limit)
};
```
- Why it's a bug: With a severity filter the entire filtered log is loaded and returned; `limit` is only applied cosmetically via `entries.iter().take(limit)` at display time. For large logs this loads everything into memory despite an explicit user bound, and combined with export (`--export`) the exported CSV/JSON contains unbounded entries regardless of `--limit` — surprising and inconsistent between the two code paths.
- Fix: apply `limit` inside the severity-filtered path (truncate after filter) or document that `--limit` doesn't apply with `--severity`.

### M-10. License key sent as URL query parameter (leaks into server/proxy logs)
- File: `src/cli/telemetry.rs:76-82`
```rust
let url = if let Some(ref key) = license_key {
    format!("{PRIVACY_API_URL}status?license_key={key}")
} else { ... };
... .get(&url) ...
```
- Why it's a bug: The full license key (the account credential used as a bearer token elsewhere) is placed in the URL query string, where it lands in access logs, proxies, and browser history patterns. All sibling privacy endpoints correctly use POST bodies. Given keys are validated as long-lived credentials, query-string transmission is a secret-handling defect.
- Fix: convert to a POST (or GET with `Authorization: Bearer`) like the export/delete/opt-out calls.

## LOW

### L-1. `scan_licenses`: `--format json|csv` without `--export` silently produces no listing
- File: `src/cli/security.rs:700-712` and 800-860. Format is validated up front, but the JSON/CSV rendering only happens inside `if let Some(export_path)`; with `format=json` and no `--export`, the user gets only the summary and the 20-row table is skipped (`else if format == "table"`). UX-breaking: the flag appears accepted but changes nothing. Fix: error when `format != table && export.is_none()`, or honor format for stdout.

### L-2. `view_audit_log --export <path>` performs no path validation and truncates display vs export asymmetry
- File: `src/cli/security.rs:216-244`. Export writes to arbitrary paths (absolute/relative, overwriting existing files without warning) whereas sibling commands call `validate_relative_path`. Consistency + accidental-overwrite hazard. Also exports ignore the display `take(limit)`? No — entries already limited/unlimited per L/M-9; see M-9.

### L-3. `fleet push` counts inactive machines in `machine_count`
- File: `src/cli/fleet.rs:112-114`: `count = members.len()` includes inactive machines, while `status` carefully distinguishes active/inactive. Push summary overstates fleet size. Use `members.iter().filter(|m| m.is_active).count()`.

### L-4. `fleet status` machine-list "... and N more" uses total machines, not remaining *active* ones
- File: `src/cli/fleet.rs:88-91`: list shows up to 10 active machines, then `total_machines - 10` — if some machines are inactive the "N more" count is wrong (can even exceed actual remaining actives). Should be `active_machines.saturating_sub(10)`.

### L-5. `team join` gist auto-init can derive empty team id
- File: `src/cli/team.rs:290-305`: for `https://gist.github.com/user/` (trailing slash) `segments.last()` is `""` producing team id `gist-`; also `github.com` URLs without a repo path yield `"team"`, silently initializing a workspace named "team". Validate the derived id is non-empty/alphanumeric before `workspace.init`.

### L-6. `workspace diff` line counting includes diff headers
- File: `src/cli/workspace.rs:430-437`: counts every line starting with `+`/`-`, including the `+++`/`---` file header lines, so every changed file reports at least +1/-1 even for pure additions/deletions. Filter out lines starting with `+++`/`---`.

### L-7. `workspace add` stores raw relative project paths
- File: `src/cli/workspace.rs:180-210`: `path` is persisted verbatim; `run_project_command` later does `.current_dir(path)` relative to whatever CWD the user happens to be in. Running `omg workspace run` from a subdirectory breaks all projects. Canonicalize to an absolute path (or resolve against the workspace file location) at `add` time.

### L-8. `workspace run` filter is substring-based
- File: `src/cli/workspace.rs:288-292`: `name.contains(f)` means `--filter api` matches `api`, `api-gateway`, and `myapi`. Exact-match (or glob) would prevent accidentally running destructive commands in unintended projects.

### L-9. `workspace run_parallel` loses per-project failure context ordering and hides spawn panic
- File: `src/cli/workspace.rs:355-380`: results printed in completion of handle order (fine), but `handle.await?` propagates a `JoinError` (panic in `spawn_blocking`, e.g. printing to closed stdout) as a global abort, discarding all other results. Consider mapping JoinError to a per-project failure.

### L-10. `link_binaries` clobbers any same-named file in the omg bin dir
- File: `src/cli/tool.rs:452-458`: `fs::remove_file(&dest)` runs whenever anything (including a real user binary/script, not just our symlink) exists at the destination. Should refuse unless dest is a symlink pointing into `tools/`.

### L-11. npm/go installs discard stderr entirely
- File: `src/cli/tool.rs:405-420, 449-462`: `.stderr(std::process::Stdio::null())` means a failed `npm install`/`go install` gives the user only "NPM install of 'x' failed" with no diagnostic. Capture stderr and include tail in the error message (cargo at least keeps stderr visible).

### L-12. `python -m venv --` uses an unsupported `--` separator convention
- File: `src/cli/tool.rs:430-436`: `Command::new("python").args(["-m", "venv", "--", install_path])`. CPython's venv historically does not document `--`; older interpreters treat `--` as the destination directory name. Works on recent versions but fragile across the Python versions OMG manages. Drop the `--`.

### L-13. `git_hooks::find_git_dir` misplaces hooks in worktrees/submodules
- File: `src/cli/git_hooks.rs:70-81`: `git rev-parse --git-dir` in a linked worktree returns the worktree's private gitdir (e.g. `.git/worktrees/foo`), which has no effective `hooks/` — hooks live in the common dir. `install` happily creates `.git/worktrees/foo/hooks` where git never looks. Use `git rev-parse --git-path hooks` which resolves correctly.

### L-14. `daemon_status`: Status-request failure silently skipped
- File: `src/cli/daemon_status.rs:150-190`: `if let Ok(ResponseResult::Status(status)) = ...` swallows both transport errors and unexpected response variants with no output — the "Package Cache"/runtime sections just vanish. Print a dim warning like the Metrics arm does.

### L-15. `snapshot delete` can orphan snapshot files from the index on partial failure
- File: `src/cli/snapshot.rs:300-320`: removes the file first, then rewrites the index; if `save_index` fails, the index retains metadata for a now-missing file, and future `restore <id>` says "not found" while `list` still shows it. Conversely order swap risks dangling files. At minimum, on index-save failure attempt to restore/rename the file or warn explicitly.

### L-16. `snapshot restore` runtime switching ignores `--yes` semantics ordering
- Related to M-2: even with `--yes`, runtimes are switched before package ops with no aggregate plan display beyond the change list — acceptable, but note `packages::remove(&to_remove, false, true, false)` third arg presumably "no confirm": removing packages non-interactively after a single generic prompt is a wide blast radius; ensure the prompt text enumerates removals (it shows counts only in the banner above).

### L-17. `telemetry::privacy_status` non-success HTTP statuses hit the local fallback silently
- File: `src/cli/telemetry.rs:107-145`: `Ok(_) | Err(_)` arm treats a 500/auth failure identically to being offline, showing static rights text with no hint the server was unreachable or errored. Distinguish at least "server error ({status})" from offline.

### L-18. `style::size` GB boundary label precision inconsistent (B integer, KB/MB 1 decimal, GB 2)
- File: `src/cli/style.rs:186-196`: cosmetic inconsistency only; INFO-level.

### L-19. `ui::Style::render` applies padding outside ANSI reset — fine — but `Color::Black` fg on dark terminals renders invisibly
- File: `src/cli/ui.rs:56-77`: `Some(Color::Black) => s.black()` used by `print_header` background pairing is fine, but any direct Black-fg use is unreadable on dark themes; the module claims WCAG AA compliance. INFO.

### L-20. `modern_ui` unicode spinner ticks hardcoded regardless of `use_unicode()`
- File: `src/cli/modern_ui.rs:27-33`: unlike `style::spinner` (which picks `-\\|/` when unicode disabled), `modern_spinner` always uses braille ticks; on non-UTF-8 locales garbage characters appear. Reuse `style::use_unicode()`.

### L-21. `runtimes::use_version` validates version *before* stripping the conventional `v` prefix
- File: `src/cli/runtimes.rs:150-160`: `validate_runtime_version(&version)?` runs on the raw value; whether `v20` passes depends on `validate_version`'s charset. Node tags are commonly written `v20.11.0` (`.nvmrc` rarely, but user args often). If the validator rejects `v`, the documented flow breaks; if it accepts `v`, the trim happens only for native managers, not mise (`MISE.use_version(&runtime, &version)` receives `v`-prefixed). Normalize once before validation/dispatch.

### L-22. `resolve_active_version` looks up hooks map with lowercased name after canonicalization mismatch potential
- File: `src/cli/runtimes.rs:14-24`: callers like `snapshot restore` pass canonical names, but `hooks::get_active_versions()` map keys must be lowercase; if a `.tool-versions` file contains `Python`, behavior depends on hook parsing (not in slice). Flagging for cross-slice verification. INFO.

### L-23. `container::run` forces `interactive || !detach` and `rm: !detach`
- File: `src/cli/container.rs:135-146`: running foreground always allocates TTY interactivity and auto-removes; a user wanting a foreground, non-interactive, kept container cannot express it. Design limitation; document or expose flags.

### L-24. `container::build` tag validation allows `|` and backticks but blocks only control chars and `;`
- File: `src/cli/container.rs:280-290`: `c == ';'` only. Since execution goes through the ContainerManager (presumably arg-vector exec, not shell) this is defense-in-depth only, but the blocklist is inconsistent with `validate_container_ref` which also blocks `|`/`&`. Unify on one validator.

### L-25. `extract_team_id` for GitHub URLs keeps nested path (`org/repo/tree/branch`) intact
- File: `src/cli/team.rs:288-303`: `https://github.com/o/r/tree/main` → team id `o/r/tree/main`, which then violates nothing locally (init validates only the explicit `init` path — `join`'s auto-init bypasses the `team_id` charset validation applied in `init`). Auto-derived ids should reuse the same validation as `init`.

### L-26. `enterprise license_scan` violation list duplicates packages once per GPL license token
- File: `src/cli/enterprise.rs:470-486`: inner loop over `pkg.licenses` pushes a violation per matching license, so dual-licensed `GPL-2.0 OR MIT`-style sets produce duplicate rows. Dedup by (package, reason).

### L-27. `enterprise reports` cost-savings figure is fabricated arithmetic
- File: `src/cli/enterprise.rs:330-333`: `cost_savings_estimate: format!("${}", machine_count * 120)` labeled "Estimate $120 saved per machine" only in a comment — the JSON report presents it as data with no caveat field. Same honesty concern family as H-2, lower impact.

### L-28. `security::version_components` stops at first non-numeric segment, mis-parsing dates-as-versions
- File: `src/cli/security.rs:1104-1117`: e.g. node built as `22.0.0-nightly` → `[22,0,0]` fine, but `20250101` style or `4.2a` → `[4]`; acceptable heuristic, documented. INFO.

### L-29. `check_eol` counts "Ending Soon" and "EOL" together into a single `issues` number
- File: `src/cli/security.rs:1180-1196, 1230-1236`: summary "N runtime(s) need attention" conflates informational warnings with hard EOL failures; exit-code implications (none currently) should distinguish. INFO/UX.

### L-30. `LicenseCategory::from_license` classifies LGPL/MPL merely by substring and ranks MPL equal to GPL
- File: `src/cli/security.rs:520-545`: `token.contains("mpl")` → Copyleft; MPL-2.0 is weak copyleft and commonly allowed where GPL is not; the summary offers only three buckets, potentially alarming users. Also `token.contains("commercial")` marks Proprietary but a token like "commercial-only" vs "noncommercial" flips meaning ("noncommercial" contains "commercial"). Edge-case misclassification. Improve token matching.

## INFO

### I-1. `AuditCommands::Scan` prints via daemon result consuming `res.vulnerabilities` — fine; but `high_severity` count may disagree with per-pkg rendering (server-side). Cross-slice verify.
### I-2. `dialoguer;` bare `use dialoguer;` import at `src/cli/security.rs:52` is redundant (used later via full path in fix_vulnerabilities) — dead import noise.
### I-3. `require_slsa_verified` intentionally fails all SLSA checks ("verification is not implemented") — honest, tested; noted as known limitation, keep.
### I-4. `security.rs export_compliance` non-SOC2 frameworks bail honestly — good; but CLI upstream (`audit Export`) apparently allows selecting them; consider hiding them from help to reduce dead options.
### I-5. `ci::write_config_file` previews instead of overwriting — good behavior; but parent dirs are created even when only previewing, leaving empty `.github/workflows/` dirs behind. Minor.
### I-6. `git_hooks PRE_COMMIT_HOOK` greps unstaged changes only; staged-but-uncommitted lock drift isn't warned — matches doc comment; fine.
### I-7. `tool.rs installed_tool_names` double-validates dirs with `is_valid_version_dir` (which is really "is dir") — naming confusion only.
### I-8. `fleet.rs health_bar` rounding `(pct/10).round()` can show 11 filled? No — `.min(10)` guards. OK.
### I-9. `telemetry opt_in_api` posts to `/opt-out` with `opt_out:false` — semantically odd endpoint reuse; works if server supports it; verify server contract.
### I-10. `man.rs` generates only two nesting levels of subcommand pages; deeper nests (e.g. `omg team golden-path create`) get pages only as `omg-team-golden-path.1`, their children unwritten. Acceptable coverage gap; document.
### I-11. `workspace::sync` help text/name suggests mutation but delegates to read-only `env check` (documented in code comment); rename or update user docs to avoid surprise.
### I-12. `ui::print_kv` fixed 12-char right alignment truncates nothing but long keys misalign — cosmetic.
### I-13. `style.rs` caches color/unicode detection process-wide via OnceLock — env changes mid-process (tests) handled via cfg(test) bypass; correct.
### I-14. `snapshot.rs generate_snapshot_id` takes first 10 chars of timestamp string — depends on jiff Display format stability (`2026-02-17T...`); if format changes, ID shape test breaks loudly (has test). OK.
### I-15. `team.rs activity` filters events whose timestamps fail parsing out silently — a malformed server timestamp makes events invisible rather than shown with "unknown time"; consider including them flagged.

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 3 |
| MEDIUM | 10 |
| LOW | 30 |
| INFO | 15 |

Total findings: 58.

Top priorities: authenticate fleet push (H-1); stop fabricating access-control audit evidence (H-2); validate `report_type` filename injection (H-3); fix generated CI workflow (M-1); reorder snapshot-restore confirmation (M-2).


---

# SLICE 05

# Audit slice-05 — `src/cli/packages/` (OMG, Rust)

Agent: audit05. Scope: every line of `/home/pyro1121/Documents/omg/src/cli/packages/` (19 files, ~4,149 LOC). Read-only audit; no builds or tests executed.

---

## Findings

### 1. MEDIUM — `--yes` silently installs the `-bin` AUR variant instead of the requested package
**File:** `src/cli/packages/install/arch.rs:436–461` (`try_aur_package`)
```rust
let bin_match = results.iter().find(|p| p.name == bin_name);
if let Some(bin_pkg) = bin_match {
    ...
    return Ok(bin_pkg.clone());
}
```
When both `foo` and `foo-bin` exist in the AUR and `omg install foo --yes` is run (or stdin is not a TTY), the code installs `foo-bin` — a *different package name* maintained by a different person — without ever asking. The informational banner about the pre-built binary only prints when an exact match also exists, but even then no confirmation of the substitution occurs before `handle_aur_package` auto-accepts under `--yes`. This is a supply-chain/trust concern: `-bin` packages repackage upstream binaries and are distinct AUR targets.
**Fix:** when an exact match exists alongside a `-bin` match, ask which one to install (or honor the exact match by default); never substitute identities non-interactively.

### 2. MEDIUM — Dead success path: "Installation cancelled" message unreachable
**File:** `src/cli/packages/install/arch.rs:511–518` (`handle_aur_package`)
```rust
record_aur_history(&aur_pkg.name, None, Err(anyhow::anyhow!("cancelled by user")))?;
modern_ui::print_error("Installation cancelled");
anyhow::bail!("Installation cancelled by user");
```
`HistoryManager::finish_operation` returns the operation error when the outcome is `Err` (verified in `src/core/history.rs:165–167`). The `?` therefore returns `"cancelled by user"` immediately, so `print_error("Installation cancelled")` never executes and the error surfaced to the user/CLI is the bare internal string `"cancelled by user"`, not the intended user-facing message.
**Fix:** record history first, ignore its returned error binding (`let _ = record_aur_history(...)`) or match on it, then print and bail with the intended message.

### 3. MEDIUM — Fallback executor silently drops Exec/Progress/Spinner/Table commands
**File:** `src/cli/packages/mod.rs:88–95` (`execute_cmd`)
```rust
Cmd::None | Cmd::Msg(()) | Cmd::Exec(_) | Cmd::Progress(_) | Cmd::Spinner(_) | Cmd::Table(_) => {
    // Not supported or applicable in fallback mode
}
```
The fallback path used in CI / non-TTY environments discards entire tables and exec side effects with no trace. Any command whose primary output is a `Cmd::Table` produces zero output in fallback mode — a UX-breaking silent failure for scripts that parse stdout.
**Fix:** render tables as plain text and at minimum log/emit a placeholder for dropped variants.

### 4. MEDIUM — Install dry run does not handle local package files
**File:** `src/cli/packages/install/arch.rs:215–290` (`install_dry_run`) vs `install/arch.rs:50–55` (`install`)
The real install path special-cases local package files via `is_local_package_file(pkg)` and skips repository resolution, but `install_dry_run` has no such branch: it feeds the file path to daemon search / `get_sync_pkg_info`, then to `AurClient::info(path)`, ultimately failing with `Package '/path/to/foo.pkg.tar.zst' was not found`. So `omg install ./foo.pkg.tar.zst --dry-run` errors while the same command without `--dry-run` succeeds.
**Fix:** add the same `is_local_package_file` branch to the dry-run loop (read metadata via `local::extract_local_metadata`).

### 5. MEDIUM — Dry-run download total only sums the first 50 updates but is presented as the total
**File:** `src/cli/packages/update/arch.rs:406–445` (`update_dry_run`)
```rust
for update in updates.iter().take(50) {
    ... total_download += info.download_size.unwrap_or(0); ...
}
...
println!("  {} Estimated download: {:.2} MB", ..., total_download ...);
```
When more than 50 updates exist, the "+N more" lines are shown, but the estimated download size covers only the displayed 50 — silently understating disk/network requirements. Same truncation pattern exists in `common.rs:224` (which at least reports "unknown" size, so it is consistent there).
**Fix:** sum over all updates (or label the estimate as covering the first 50).

### 6. LOW — Deferred-sync elevation arm uses a no-op operation closure
**File:** `src/cli/packages/update/arch.rs:316–325`
```rust
crate::package_managers::arch::run_privileged_operation(
    "fullupdate", &[], || async { Ok(()) },
).await?;
```
`run_privileged_operation` runs the closure directly whenever `can_write_pacman_db()` is true (`package_managers/arch.rs:42–47`). Today this arm is only reached when `needs_deferred_sync` (i.e., cannot write pacman db), so the no-op is not exercised — but it is exactly the "already-privileged invocation claims success without doing anything" bug class the codebase fixed elsewhere (see `run_sysupgrade` doc comment). Nothing structurally prevents future refactors from routing here while privileged.
**Fix:** pass `run_sysupgrade` as the closure so the fast path performs the upgrade too, and let the child-delegation comment govern the elevated path.

### 7. LOW — Search-query metacharacter blocklist is incomplete and inconsistent
**File:** `src/cli/packages/common.rs:44–47` (`validate_search_query`)
```rust
if query.chars().any(|c| ";|&><$".contains(c)) { ... }
```
Backticks, single/double quotes, parentheses, `!`, and `#` are accepted. If any backend ever interpolates queries into a shell (apt wrappers, helper scripts), these are injectable. Even if today's backends use argv-only execution, the validator advertises "shell metacharacters detected" while permitting most of them.
**Fix:** either allow-list `[A-Za-z0-9._+-]` (all real package/query alphabets) or extend the deny list and rename the error honestly.

### 8. LOW — `remove` ignores `_yes` entirely; destructive removal is always non-interactive
**File:** `src/cli/packages/remove.rs:29–49`
Removal accepts `_yes` "for CLI symmetry" but no backend ever prompts, including interactive TTY sessions. A typo'd `omg remove linu` (prefix/regex behavior depending on backend) removes packages with no confirmation gate, unlike install/update which confirm.
**Fix:** prompt before removal unless `--yes` is passed (or document loudly that removal is unconfirmed).

### 9. LOW — `recursive` flag misreported in Debian/generic dry runs vs CLI surface
**File:** `src/cli/packages/remove.rs:19–24, 51–62`; `remove/arch.rs:31–35`
Arch's real transaction always recurses (`RECURSE | UNNEEDED`) regardless of the flag, yet the Arch dry run prints "Orphaned dependencies would also be removed" **only when `recursive` is true** — i.e., the preview can claim no recursion while the actual removal will recurse. The doc comments acknowledge the flag is advisory, but the preview output is still factually wrong for `recursive=false`.
**Fix:** print the recursion truth unconditionally in the Arch dry run (or actually gate libalpm flags on the argument).

### 10. LOW — `clean --orphans` on the APT feature build calls blocking FFI/sudo work inline in async context
**File:** `src/cli/packages/clean.rs:198`
```rust
apt_remove_orphans()?;
```
In the arch-capable block's debian-only arm, `apt_remove_orphans()` runs synchronously inside the `async fn clean`, unlike the debian-like branch above which correctly wraps it in `tokio::task::spawn_blocking`. Stalls the executor during apt operations.
**Fix:** wrap in `spawn_blocking` like line ~180 does.

### 11. LOW — Debian-feature-only build on a non-Debian host routes orphan cleanup to apt
**File:** `src/cli/packages/clean.rs:150–260`
The final arch-capable block is compiled under `any(feature = "arch", not(feature = "debian-pure"))`. With only the `debian` feature enabled on a non-Debian host, `do_orphans` reaches `apt_remove_orphans()` (finding 10's call site) on a machine with no dpkg. Similarly `do_cache` bails with an APT-specific message. Edge-case config, but wrong-backend execution rather than a clean "unsupported host" error.
**Fix:** gate the apt arms additionally on `is_debian_like()` and otherwise bail with a host-mismatch error.

### 12. LOW — JSON `info` mode skips the AUR fallback that text mode performs
**File:** `src/cli/packages/info.rs:236–300` (`info_json`) vs `info_fallback`
Text mode falls back to AUR lookup (`search_detailed`) when the package isn't official; `info_json` bails with `Package '<x>' not found`. Machine consumers get inconsistent data depending on where a package lives.
**Fix:** add a serialized AUR branch (or document JSON as official-only).

### 13. LOW — `search_sync_cli_with_limit` silently ignores the `detailed` flag on Debian
**File:** `src/cli/packages/search.rs:245–266`
On Debian-like hosts the function immediately delegates to `search_sync_official_only(query, limit)`, dropping `detailed`; AUR detail fields are Arch-only anyway, but callers receive `Ok(true)` implying the requested mode was honored.
**Fix:** document or return `Ok(false)` when `detailed` cannot be honored so callers fall back.

### 14. INFO — `explicit` test-mode isolation only covers Debian backends
**File:** `src/cli/packages/explicit.rs:104–117`
Test-mode routing to `MockPackageManager` exists only under `#[cfg(any(feature = "debian", feature = "debian-pure"))]`. On Arch-enabled test builds, `FastStatus::read_explicit_count()` / `pacman_db::get_explicit_count()` read the host database even in test mode (unless `fast_status` itself is test-aware — not visible in this module).
**Fix:** verify/add a test-mode branch for the Arch counting paths.

### 15. INFO — Pure-Rust `.PKGINFO` scanner hard-fails on any hostile entry anywhere in the archive
**File:** `src/cli/packages/local.rs:113–118`
```rust
if path_str.contains("..") || path_str.starts_with('/') {
    anyhow::bail!("Security: Rejecting malicious path in package archive: {path_str}");
}
```
The traversal/symlink protections are good, but the scan aborts the whole extraction on any offending *non-`.PKGINFO`* entry (e.g., a payload file named `usr/share/x/../y`) instead of skipping it and continuing to look for `.PKGINFO`. Metadata extraction of an installable-but-imperfectly-built archive fails outright. Also `contains("..")` rejects benign names like `foo..bar`.
**Fix:** restrict rejection to entries considered for reading; skip others with a warning.

### 16. INFO — `parse_pkginfo_manual` takes the last duplicate key
**File:** `src/cli/packages/local.rs:139–170`
Repeated `pkgname =` lines overwrite earlier ones (last wins) rather than rejecting malformed metadata; values are not validated against package-name rules before being recorded into history/display.
**Fix:** reject duplicates or validate the parsed name with `validate_package_name`.

### 17. INFO — AUR parallel-build failure accounting counts requests, not built outputs
**File:** `src/cli/packages/update/arch.rs:370–382`
`failed_count += aur_packages.len()` counts update entries, but `build_jobs_for_updates` groups split packages into shared jobs, and one failed job may correspond to several entries (or vice versa). Reported "N failed" can be inaccurate.
**Fix:** derive counts from job outcomes.

### 18. INFO — Daemon search error downgrades the whole dry-run client mid-loop
**File:** `src/cli/packages/install/arch.rs:259–261`
```rust
Err(_) => { daemon_client = None; }
```
A single transient IPC error permanently disables the daemon fast path for all remaining packages in the dry run (falls back to slower direct ALPM per package). Not incorrect, just degraded resilience; the error is swallowed without logging.
**Fix:** log the error and consider retrying/reacquiring instead of dropping for the rest of the loop.

### 19. INFO — `extract_missing_package` parsing is locale/string-format coupled
**File:** `src/cli/packages/install/arch.rs:298–308`
The AUR-fallback trigger depends on exact substrings `"not found in any configured repository"` and `"Package '"` produced by the ALPM transaction layer. Any wording change (upstream pacman output, translation, refactor) silently disables the fallback — fail-safe direction (no false AUR installs), but the coupling is fragile and untested against the real producer of the string.
**Fix:** have the transaction layer return structured missing-target info instead of parsing prose.

### 20. INFO — `update_turbo` double-reads the update list across a privilege boundary (TOCTOU)
**File:** `src/cli/packages/update/arch.rs:96–119`
`get_update_list()` runs unprivileged to decide whether to say "up to date", then the elevated child independently re-runs the upgrade via `run_sysupgrade`. Repos can change between the two reads; also the parent prints "Found N update(s)" based on stale data while the child upgrades whatever it finds. Benign for correctness of the upgrade itself, minor display inconsistency.

### 21. INFO — `status_fallback` treats any non-`Other` Elm UI failure as recoverable
**File:** `src/cli/packages/status.rs:39–50`
Only `ErrorKind::Other` propagates; every other I/O error (including e.g. permission errors opening the TTY) triggers a silent fallback that re-queries everything, doubling work after a partial UI render. Consider narrowing which kinds are truly transient.

---

## Verified non-issues (checked, sound)

- `record_install_history` + `finish_operation` correctly propagate the original install error (`history.rs:165–173`), so failed installs do not report success (initially suspected).
- `extract_missing_package` regression tests cover conflicting-files and different-package-name misrouting (`install/arch.rs:tests`).
- Replacement-hop budget bounds suggestion recursion; budget exhaustion errors cleanly.
- `update()` deferred-sync history single-ownership logic matches child/parent recording contract and is unit-tested.
- Local archive xz decompression is memory-budgeted (`BudgetedSink`); zst/gz use streaming decoders.
- Daemon response enums in `explicit.rs` are exhaustively enumerated, forcing revisit on new variants.
- `sync_db.rs` correctly distinguishes daemon-absent (debug) from refresh-refused (error).


---

# SLICE 06

# Audit slice-06 — `src/cli/tea/`, `src/cli/tui/`, `src/cli/components/` (read-only)

Auditor: audit06. Scope: every line of `src/cli/tea/{mod,cmd,renderer,async_bridge,wrappers,info_model,search_model,status_model,update_model}.rs`, `src/cli/tui/{mod,app,ui}.rs`, `src/cli/components/mod.rs`. No builds/tests executed.

## HIGH

### H1. `Cmd::Error` aborts batch → suggestion in `error_with_suggestion` / `permission_error` is never printed
- File: `src/cli/components/mod.rs:186-199` (`error_with_suggestion`) and `src/cli/components/mod.rs:120-136` (`permission_error`); behavior defined in `src/cli/tea/mod.rs:236-241` + `src/cli/tea/mod.rs:141-146`.
- Excerpt:
  ```rust
  Cmd::batch([
      Cmd::spacer(),
      Cmd::error(error.into()),
      Cmd::info(format!("💡 {}", suggestion.into())),
      Cmd::spacer(),
  ])
  ```
  and in `execute_output_cmd`: `Cmd::Error(msg) => { renderer.error(&msg)?; return Err(io::Error::other(msg)); }`.
- Why it is a bug: `Program::process_cmd` / `run_report` return immediately on `Cmd::Error`. Both components place `Cmd::error` **before** the suggestion inside a batch, so the `💡 suggestion` (and the "Try running: sudo …" hint) are dead output that can never render. The user gets an error with no remediation hint — exactly what these helpers exist to provide. The component unit tests only inspect `Cmd` structure, not execution order, so this slipped through.
- Fix: emit the error last (`Cmd::info(suggestion)` first, `Cmd::error(...)` final), or introduce a non-fatal `Cmd::ErrorNonFatal`, or make `run_report` drain remaining output before returning the error.

### H2. `UpdateModel --yes` flow always ends in guaranteed failure; interactive confirm can never be accepted
- File: `src/cli/tea/update_model.rs:196-203` (`UpdateMsg::Execute`) and `update_model.rs:180-194` (confirm branch).
- Excerpt:
  ```rust
  } else if self.yes {
      Cmd::batch([summary_cmd, Cmd::exec(|| UpdateMsg::Execute)])
  } else {
      self.state = UpdateState::Confirming;
      Cmd::batch([summary_cmd, Components::confirm("System Upgrade", "Enter")])
  }
  ...
  UpdateMsg::Execute => { ... Cmd::error("System upgrade execution is not implemented; nothing was installed") }
  ```
- Why it is a bug: (a) `--yes` shows the summary then deterministically fails with "not implemented" — the auto-confirm path is wired to nothing, so any caller exposing `omg update --yes` through this model ships a broken command. (b) In the interactive path the model prints "Proceed? (Enter or --yes to skip)" but the synchronous `Program` runtime has no keyboard input loop; pressing Enter does nothing and the program just renders the empty view and exits, leaving state stuck in `Confirming`. The prompt is UX theater.
- Fix: either wire a real executor (drive the package manager from `Execute`) or remove the `yes`/confirm arms from this model until an executor exists; do not print a confirmation prompt that cannot be answered.

### H3. `cleanup_terminal` clears the user's primary screen after leaving the alternate screen
- File: `src/cli/tui/mod.rs:73-83`.
- Excerpt:
  ```rust
  disable_raw_mode()?;
  execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
  terminal.show_cursor()?;
  terminal.clear()?;   // <-- runs on the PRIMARY screen
  ```
- Why it is a bug: after `LeaveAlternateScreen`, the backend writes to the user's normal terminal. `terminal.clear()` emits "clear entire screen", wiping the shell content the user had before launching the TUI (scrollback-visible area). Every TUI exit destroys primary-screen content — a destructive side effect on the user's session.
- Fix: remove `terminal.clear()` here (ratatui apps normally clear *before* entering the alt screen if needed), or clear while still on the alternate screen.

## MEDIUM

### M1. Search/info failure and no-results messages are printed twice (Cmd + view duplication)
- Files: `src/cli/tea/search_model.rs:170-198` (`SearchMsg::NoResults`/`ResultsFound` cmds) vs `search_model.rs:288-299` (`SearchState::NoResults` view), `search_model.rs:200-207` vs `view()` Failed arm; same pattern in `src/cli/tea/info_model.rs:139-149` vs `view()` NotFound/Failed arms.
- Excerpt: update returns `Cmd::warning(format!("No packages found for '{}'", self.query))`; later `Program::run` calls `self.render()`, and `view()` prints `\n✓ No packages found for '...'\n` again.
- Why it is a bug: the synchronous Elm runtime processes the update's output commands AND then renders the final view, so "No packages found for '<q>'" appears twice, and errors appear both as styled `✗ ...` from `Cmd::error` and again from the `Failed` view. Confusing duplicated UX output.
- Fix: make the transient states' `view()` return `String::new()` (like `UpdateModel::view`) so only one channel emits the message.

### M2. `search_time_ms` is never set — header always displays "0.0ms"
- File: `src/cli/tea/search_model.rs:57` (field), never assigned anywhere except `Default` (grep: no write). Displayed at `search_model.rs:243`: `format!("{} results ({:.1}ms)", ...)`.
- Why it is a bug: the results header permanently claims `(0.0ms)` regardless of actual search duration. Dead/misleading metric shown to users.
- Fix: record elapsed time in `fetch_search_results` (carry it back on `ResultsFound`) or drop it from the header.

### M3. `UpdateType::from_versions` drops pacman epoch and misclassifies equal/downgraded versions as Patch
- File: `src/cli/tea/update_model.rs:66-84`.
- Excerpt:
  ```rust
  let old_str = old_ver.trim_start_matches(|c: char| !c.is_numeric());
  ...
  if new.major > old.major { Self::Major }
  else if new.minor > old.minor { Self::Minor }
  else { Self::Patch }
  ```
- Why it is a bug: (a) pacman epochs like `2:1.0.0-1` lose the epoch (`trim_start_matches` strips `2:`), so an epoch bump `1:1.0 → 2:1.0` is reported as `Patch` though it can be a major behavioral jump; (b) identical versions or downgrades fall through to `Patch` — there is no equality/downgrade handling, so the classification is wrong whenever `new <= old`.
- Fix: parse epoch explicitly (`split(':')` before trimming), and add an `Equal`/downgrade guard instead of defaulting to Patch.

### M4. Blocking network search runs inline on the UI task — TUI freezes during search
- File: `src/cli/tui/mod.rs:126-131` (`committed` branch calls `run_search(app, &last_search).await` inline) and `mod.rs:140-146` (debounce branch); `src/cli/tui/app.rs:223+` (`search_packages` awaits daemon/apt/pacman I/O).
- Why it is a bug: unlike install/update/clean (spawned via `spawn_action`), search I/O is awaited directly inside `run_app`, so key handling and drawing stall until the search completes (potentially seconds on apt/pacman fallbacks). The UI also stops responding to `q` during the freeze.
- Fix: route searches through `spawn_action` like other actions, or at minimum draw an explicit "searching…" state before awaiting.

### M5. Selection index not re-clamped when new search results arrive — navigation can get stuck
- File: `src/cli/tui/app.rs` (`selected_index` reset only in `/` handler at `app.rs:474-482`; Down-guard at `app.rs:455-463` uses `if self.selected_index < max`).
- Why it is a bug: if a previous result set left `selected_index = 10` and a new query returns 3 rows, `Down/j` is a no-op (`10 < 2` false) while `Up/k` walks down from an invisible offset; the highlighted row is none (index out of range) so the user sees no selection yet Enter-popup logic operates on `.get(selected_index)` → "No package selected". Confusing dead navigation state.
- Fix: clamp `selected_index = selected_index.min(len.saturating_sub(1))` whenever `search_results` is replaced in `search_packages`.

### M6. Popup selection can differ from what the popup displayed (j/k still navigate while popup open)
- File: `src/cli/tui/mod.rs:196-212` (Enter-with-popup arm) and `src/cli/tui/app.rs:516-518` (`KeyCode::Esc` closes popup but other keys fall through to `handle_key`).
- Why it is a bug: while the confirm popup is open, `j`/`k`/arrows still move `selected_index` (they fall through to normal handling since they don't match the special Enter arm). Pressing Enter then installs whatever row is now selected, which may not be the package named in the popup title the user just confirmed. Confirmation mismatch — potentially installs an unintended package.
- Fix: while `show_popup` is true, swallow all keys except Enter/Esc in `handle_key`.

### M7. Daemon search results hardcode `installed:false` / `repo:"official"` even for AUR rows
- File: `src/cli/tui/app.rs:245-260` and `src/cli/tea/search_model.rs:330-352` (`fetch_search_results` daemon path).
- Excerpt: `repo: "official".to_string(), installed: false` unconditionally.
- Why it is a bug: daemon responses with `source == "AUR"` (which the tea model itself checks: `if pkg.source == "AUR" { PackageSource::Aur }`) are labeled `official` in the TUI repo column, and installed status is silently dropped — misleading metadata in the UI.
- Fix: propagate `pkg.source` into `repo` and any installed flag from the daemon payload.

### M8. Nested-batch fold in `Components::kv_list` is O(n²) and creates n-deep nesting
- File: `src/cli/components/mod.rs:96-104`.
- Excerpt: `content.into_iter().fold(Cmd::<M>::none(), |acc, c| Cmd::batch(vec![acc, Cmd::println(c)]))`.
- Why it is a bug: builds a left-nested batch of depth n (quadratic allocation); `Cmd`'s derived/manual Debug recursion over deeply nested batches could overflow the stack for very large lists, and `process_cmd` pays the nesting cost.
- Fix: collect into one flat `Vec<Cmd<M>>` and return `Cmd::Batch(list)`.

### M9. `Components::step` renders as two lines instead of one "[n/N] ⟳ msg"
- File: `src/cli/components/mod.rs:31-49` with `src/cli/tea/mod.rs:267-270` (StyledText fallback = `println(config.text)`).
- Why it is a bug: the intended single-line step indicator `[1/3] ⟳ Processing` actually renders as `[1/3] ⟳\n Processing` because `StyledTextConfig` has no newline-suppression in the synchronous renderer, and the follow-up `Cmd::println(" {}", message)` adds a leading space on its own line. Broken layout wherever steps are used.
- Fix: have `step` build one plain string and use a single `Cmd::println`, or give the renderer real inline-styled text support.

### M10. Team status fetched from remote API every 5 seconds
- File: `src/cli/tui/app.rs:118-119` (`tick` refresh interval) → `refresh()` → `fetch_team_status()` (`app.rs:143-181`) which calls `crate::core::license::fetch_team_members().await` for licensed users.
- Why it is a bug: full refresh (daemon reconnect + history load + team API HTTP call) fires every 5 s for the entire TUI session — needless network chatter/API rate exposure and latency spikes in the UI loop.
- Fix: cache team status with a much longer TTL (e.g. 60 s+) independent of the 5 s dashboard refresh.

## LOW

### L1. Final view flush skipped when view is empty
- File: `src/cli/tea/mod.rs:110-118` (`render()` returns early on empty view).
- Why: all `execute_output_cmd` writes go through `BufWriter`; if the model's final `view()` is empty (e.g. `UpdateModel`), no explicit flush happens after processing commands — output survives only via `BufWriter`'s drop-flush, whose IO errors are silently discarded. A `flush()` after `process_cmd` in `run()` would guarantee delivery.
- Fix: call `self.renderer.flush()?` unconditionally at the end of `run()`.

### L2. `Renderer::header` no_color fallback changes layout vs colored version
- File: `src/cli/tea/renderer.rs:113-129`.
- Why: colored mode renders padded/bold badge; no-color prints `\n[title] body` — different shape but acceptable; more importantly the colored path uses `ui::Style::background(Black)` which is illegible on black-on-black terminals. Cosmetic accessibility issue.

### L3. Daemon info source heuristic: anything ≠ `"official"` is treated as AUR
- File: `src/cli/tea/info_model.rs:275-279` (`source: if info.source == "official" { Official } else { Aur }`).
- Why: unknown/new daemon source strings are silently mislabeled "AUR (Arch User Repository)". Prefer explicit match with a logged fallback.

### L4. Daemon errors silently swallowed in search/info paths
- Files: `src/cli/tea/search_model.rs:321-325` (`if let Ok(...) && let Ok(res) = ...`), `src/cli/tea/info_model.rs:264-267` (`if let Ok(Ok(info)) = timeout(...)`), `src/cli/tui/app.rs:235-240`.
- Why: daemon connect/search failures fall through to fallback without even a `tracing::debug!`, making field diagnosis ("why did my daemon search get ignored?") impossible. Not a correctness bug by design (fallback exists), but observability gap.

### L5. `SearchState::Searching` / `InfoState::Loading` views are effectively unreachable
- Files: `search_model.rs:238-240`, `info_model.rs:184-186`.
- Why: the synchronous `Program` renders only before init-cmd processing and once after completion, so loading states never display; dead code paths that suggest interactivity the runtime doesn't have.

### L6. No-results view uses green ✓ checkmark for an empty result set
- File: `src/cli/tea/search_model.rs:290-295` (`"\n✓ {}\n"` around "No packages found").
- Why: success icon for a failed lookup is semantically wrong; should be neutral/warning styling.

### L7. Result numbering restarts per group, and `render_result` repo filter is redundant/confusing
- File: `src/cli/tea/search_model.rs:130-155` + enumeration at `search_model.rs:255-263` / `273-281`.
- Why: official list numbered 1..n and AUR restarts at 1..m; combined with per-repo suffix `[extra]` etc., numbering implies a single selectable list that isn't (no selection exists in CLI search). Minor UX ambiguity.

### L8. `MemAvailable` missing → memory gauge shows 100 %
- File: `src/cli/tui/app.rs:337-356` (`available` stays 0 if `MemAvailable:` line absent → `(total-0)/total*100`).
- Why: on exotic kernels/configs missing MemAvailable, usage reads 100 % instead of falling back to MemFree. Edge-case cosmetic.

### L9. `get_memory_usage`/`sample_cpu_totals` parse failures partially tolerated inconsistently
- File: `app.rs:300-317`: `field.parse::<u64>().ok()?` aborts whole sample if any cpu field is non-numeric, whereas meminfo uses `unwrap_or(0)`. Not harmful, but inconsistent robustness; also `total.checked_add(value)?` silently yields `None` (0 % CPU) rather than clamping on absurd counters.

### L10. Security-audit action reports outcome only to tracing log
- File: `src/cli/tui/mod.rs:171-182`.
- Why: pressing `a` runs the scan; the vulnerability count goes to `tracing::warn/info!` which the TUI doesn't surface anywhere visible; on success `report_action_result` clears `action_error` and shows nothing. User cannot tell whether the audit ran or found anything. Also the Security tab advertises `f Fix Vulnerabilities` and `p Edit Policy` actions (`ui.rs:1000-1050` action panel) with no handlers — dead advertised shortcuts (the status-bar hints were already pruned for this reason, but the Actions panel wasn't).

### L11. `async_bridge` spawns a thread + possibly a fresh Runtime per call
- File: `src/cli/tea/async_bridge.rs:7-18`.
- Why: outside a runtime, each call constructs and tears down a full tokio Runtime; inside a runtime it blocks a spawned OS thread. Correct but heavyweight per command; fine for one-shot CLI, worth noting for repeated calls (status tick etc.).

### L12. Thread join swallows panic payload detail
- File: `async_bridge.rs:12-14`: `.map_err(|_| anyhow!("Background thread panicked ..."))` discards the panic message/backtrace. Include the payload if available.

### L13. Hardcoded RPC ids `id: 0` everywhere
- Files: `status_model.rs:135`, `app.rs:99`, `app.rs:238`.
- Why: if the protocol ever correlates responses by id, all concurrent callers share id 0. Currently serialized/single-call, so latent only.

### L14. `run_report` silently ignores `Cmd::Exec`
- File: `src/cli/tea/mod.rs:349-360` + `execute_output_cmd` no-op arm.
- Why: documented, but a report built with an `Exec` carrying lazily-produced output loses that output silently. Consider asserting/debug-logging dropped control-flow commands in report mode.

### L15. comfy-table card ignores `NO_COLOR` enforcement
- File: `src/cli/tea/renderer.rs:132-153`.
- Why: the renderer gates its own colors on `no_color`, but the comfy-table preset is rendered independently; currently UTF8_FULL adds no color so impact is nil, but any future colored preset would leak ANSI under NO_COLOR. Add `table.force_no_tty()` when `self.no_color`.

### L16. `Tab::Runtimes | Activity | Team` status bar advertises `r Refresh`, but refresh forces a dashboard-only data refresh with no visual feedback on those tabs
- File: `src/cli/tui/ui.rs:1120-1123` hints vs `app.rs:441-450` ('r' sets `last_tick` past; refresh updates daemon status/history/metrics, none displayed differently). Minor expectation mismatch.

## INFO

### I1. `MAX_CMD_STEPS` counts every queue pop including pure-output commands
- `src/cli/tea/mod.rs:60,127-133`: a legitimately huge batch (>100k outputs) would trip the cycle budget erroneously. Practically unreachable; note only.

### I2. Batch ordering relies on reversed push idiom in two places
- `mod.rs:138-142` and `mod.rs:352-356`. Correct today (LIFO stack); fragile if someone converts to VecDeque without re-checking order. Comment exists — acceptable.

### I3. `Cmd::Progress`/`Spinner` variants are accepted and dropped by the sync runtime
- `mod.rs:250-256`. Documented; models must not use them. Consider removing variants from this runtime's public API to prevent silent loss.

### I4. `truncate_width` reserves 1 column for ellipsis; zero-width strings safe
- `tui/ui.rs:20-40`. Logic verified correct (including `max_width == 0` and combining marks).

### I5. `/proc/net/dev` field indexing verified correct
- `app.rs:377-391`: `fields.next()` consumes rx-bytes (idx0), `fields.nth(7)` lands on tx-bytes (idx8). Correct despite looking suspicious; deserves a comment to prevent a future "fix".

### I6. Mouse capture enabled but mouse events unused
- `tui/mod.rs:36,80`: EnableMouseCapture with no mouse handling; events consumed and discarded. Harmless overhead; either handle or disable.

### I7. `usage_stats` loaded once, never persisted by the TUI
- `app.rs:88`. Display-only; stale within long sessions. Fine for now.

### I8. `pub fn styled_label` / `pub fn package_name` missing `#[must_use]` where sibling APIs have it
- `info_model.rs:52`, `search_model.rs:47`. Style consistency only.

### I9. Duplicate `use owo_colors::OwoColorize;` placement mid-file
- `search_model.rs:43` appears after type definitions rather than with other imports; cosmetic.

### I10. `UpdateMsg::Check` arm is dead in practice
- `update_model.rs:160-169`: nothing sends `Check` (init performs the check directly). Kept for API completeness; flagged as dead code risk.

---

**Totals:** 3 HIGH · 10 MEDIUM · 16 LOW · 10 INFO = **39 findings**

Cross-cutting themes: (1) fatal `Cmd::Error` semantics silently truncate companion output (H1 pattern likely affects other slices using `error_with_suggestion`); (2) the synchronous Elm runtime cannot honor interactive promises (prompts, spinners, progress) — several models print affordances the runtime cannot fulfill (H2, L5, I3); (3) TUI blocking work on the UI task and 5 s remote polling degrade responsiveness (M4, M10).


---

# SLICE 07

# Audit slice-07 — `src/core` (client, http, error, types, format, paths, caps, safe_ops, privilege, sudoloop)

Auditor: audit07 · Scope: 10 files, ~2654 lines (every line read) · Read-only audit.

---

## HIGH

### H-1. `privilege.rs` — dead production elevation API with broken arg-offset contract (`skip(1)` drops the first real argument)
**File:** `src/core/privilege.rs:196-206` (`elevate_if_needed`) and `src/core/privilege.rs:65-71, 246-248`
```rust
// elevate_if_needed:
let args_refs: Vec<&str> = args
    .iter()
    .skip(1)
    .map(std::string::String::as_str)
    .collect();
```
```rust
// PrivilegeChecker::elevate -> elevate_for_operation -> elevate_if_needed
fn elevate(&self, operation: &str, args: &[String]) -> std::io::Result<()> {
    elevate_for_operation(operation, args)
}
```
`elevate_if_needed` unconditionally skips `args[0]`, assuming callers pass full `std::env::args()` (argv[0] = program name). But its only production-reachable entry point besides `with_root(std::env::args())` is `elevate_for_operation`, whose doc/test contract passes *operation arguments without a program name* (`checker.elevate("install", &["omg", "install", "firefox"])` in tests does include argv[0], but `SystemPrivilegeChecker.elevate(operation, args)` is designed to receive plain payload args). Any caller passing payload-only args silently loses the first argument under sudo — e.g. `install firefox` becomes `install` with no package. Currently masked because `elevate_if_needed`, `elevate_for_operation`, `with_root`, and the whole `PrivilegeChecker` DI trait have **zero production call sites** (only tests; see I-1), so this is latent, but it is a live trap for the next caller.
**Fix:** remove the blind `.skip(1)` and make the argv[0] convention an explicit parameter, or delete the unused API surface entirely (preferred per project rules).

---

## MEDIUM

### M-1. `safe_ops.rs` — `atomic_write_file_sync` destroys existing file permissions/ownership
**File:** `src/core/safe_ops.rs:88-116`
```rust
let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
...
temporary.persist(&path)
```
`tempfile::NamedTempFile` is created mode `0600`. Persisting it over an existing target replaces that file atomically but discards the original's mode, owner/group, ACLs/xattrs. When run as root/elevated (this codebase elevates freely), a previously user-owned `0644` config or state file silently becomes a root-owned `0600` file — breaking subsequent reads by the normal user and any daemon running as that user.
**Fix:** stat the destination before writing and re-apply mode/owner after persist (or chown/chmod the temp file before rename when the destination exists).

### M-2. `safe_ops.rs` — `create_dir_all` on parent uses default umask permissions
**File:** `src/core/safe_ops.rs:96-98`
```rust
std::fs::create_dir_all(parent)
```
For paths whose parent chain doesn't exist yet, directories are created world-readable per umask (typically `0755`). Combined with M-1 this can publish `0600` content inside a newly created *world-readable* directory. If this helper is ever used for secrets/tokens, that's an information disclosure path.
**Fix:** create parents explicitly with a restrictive mode (e.g. `DirBuilder::mode(0o700)` for the leaf, or document the requirement).

### M-3. `paths.rs` — `validate_socket_parent` / connect TOCTOU and missing socket-file checks
**File:** `src/core/paths.rs:344-393` and `src/core/client.rs:29-46`
The client validates the socket parent directory (symlink-free, uid-owned, `0o077 == 0`) and *then* connects. Between validation and `connect()` an attacker with local access who can win the rename race can swap the directory or plant their own socket at the same path, receiving client requests (which include queries; not secrets today, but the protocol has no peer authentication at all). Also note the check verifies only the *parent*: the `omg.sock` file itself may be a symlink to another socket. The window is small and exploitation requires same-uid or privileged local access, hence MEDIUM not HIGH.
**Fix:** open the socket with `O_PATH|O_NOFOLLOW` style verification of the final component too, or use `connectat` semantics; ideally also perform a handshake/peer-credential check (`SO_PEERCRED`) in the protocol.

### M-4. `sudoloop.rs` — background refresh loop runs `sudo -v` forever with no password path and spams failures
**File:** `src/core/sudoloop.rs:47-90`
```rust
let result = Command::new("sudo")
    .arg("-v")
    .kill_on_drop(true)
    .output()
    .await;
```
`Command::output()` defaults stdin to null, so if the sudo timestamp expires and a password is required, every 30 s the loop spawns sudo which fails immediately, logging a warning each cycle for the lifetime of a long AUR build. It also never re-prompts. Additionally `SudoLoop::start()` does not check `can_use_sudoloop()`; starting the loop as root pointlessly spawns `sudo -v` every 30 s (harmless but wasteful).
**Fix:** stop the loop (or back off exponentially) after N consecutive failures; gate `start()` on `can_use_sudoloop()` internally.

### M-5. `http.rs` — hard 5-minute total timeout caps large downloads
**File:** `src/core/http.rs:14`
```rust
const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(5);
```
`.timeout()` covers the *entire* request including body transfer. On connections slower than roughly `(package size)/300 s` (e.g. >~150 MB on a 4 Mbit link), every large package download aborts mid-transfer regardless of healthy streaming. For a package manager this is a real-world UX-breaking defect on slow links.
**Fix:** drop the total timeout for the download client and rely solely on `read_timeout` (stall detection), which is already configured.

### M-6. `format.rs` — `truncate` violates its documented "never exceeds `max` bytes" contract for `max < 3`
**File:** `src/core/format.rs:11-22`
```rust
/// The result never exceeds `max` bytes.
pub fn truncate(s: &str, max: usize) -> String {
```
`truncate("hello", 2)` returns `"..."` (3 bytes) and `truncate("hello", 1)` returns `"..."` (3 bytes) — exceeding `max`. The test even asserts this behavior (`truncate_with_tiny_max_yields_ellipsis_only` asserts `"..."` for max=2), codifying the contract violation. Any layout code trusting the doc (fixed-width TUI cells) will overflow columns for multibyte/tiny budgets.
**Fix:** either cap output at `max` (return empty or a truncated ellipsis slice) or fix the doc.

---

## LOW

### L-1. `privilege.rs` — unreachable `exit(0)` masks nothing but documents wrong control flow
**File:** `src/core/privilege.rs:225-230`
```rust
// If run_self_sudo returns, it means the command succeeded.
std::process::exit(0);
```
`run_self_sudo` never returns normally — it always ends in `std::process::exit(status.code().unwrap_or(1))` or an error propagated by `?`. The trailing `exit(0)` is dead code; if it ever *were* reachable (future refactor making `run_self_sudo` return), it would report success (exit 0) for a failed elevated command.
**Fix:** replace with `unreachable!("run_self_sudo always exits")` or return an error.

### L-2. `privilege.rs` — dev-mode detection keys on env-var *presence*, not value
**File:** `src/core/privilege.rs:266-269`
```rust
let is_test_mode =
    std::env::var("OMG_TEST_MODE").is_ok() || std::env::var("CARGO_PRIMARY_PACKAGE").is_ok();
```
An installed binary launched from an environment where `CARGO_PRIMARY_PACKAGE=1` leaked (e.g. invoked from a cargo process without full env sanitization) silently refuses all privilege elevation with a confusing "development mode" error. Conversely `OMG_TEST_MODE=""` counts as test mode. Compare with `client.rs::daemon_disabled` / `paths::test_mode`, which correctly match on values `"1" | "true" | "TRUE"`.
**Fix:** use the same value-matching helper used elsewhere for `OMG_TEST_MODE`; drop or scope the `CARGO_PRIMARY_PACKAGE` heuristic.

### L-3. `privilege.rs` — `payload_command` still sets `OMG_ELEVATED=1` env var that its own comment says cannot survive sudo
**File:** `src/core/privilege.rs:289-297`
```rust
.env("OMG_ELEVATED", "1")
...
.arg(crate::core::privilege::ELEVATED_MARKER)
```
The block comment explains sudo `env_reset` strips `OMG_ELEVATED`, hence the argv marker; setting the variable anyway is contradictory dead configuration. If a sudoers entry ever preserves it via `env_keep`, the env flag alone still isn't trusted (good — `is_elevated` requires root), but keeping both channels invites divergence.
**Fix:** remove the `.env("OMG_ELEVATED", "1")` line and rely solely on the marker.

### L-4. `paths.rs` — DOAS branch assumes `/home/<user>` home directory
**File:** `src/core/paths.rs:120-127`
```rust
if let Ok(doas_user) = std::env::var("DOAS_USER") ... {
    let home = PathBuf::from(format!("/home/{doas_user}"));
    return home.join(".cache/omg");
}
```
Unlike the SUDO branch (which honors `SUDO_HOME`), the doas branch hardcodes `/home/<user>`. Users with homes outside `/home` (common on NixOS, some LDAP setups, `/var/home`) get cache reads/writes against a non-existent or wrong directory while running elevated.
**Fix:** resolve the home dir via `getent passwd <user>` / `usrinfo`-equivalent rather than string concatenation.

### L-5. `paths.rs` — `is_valid_username` misused to validate `SUDO_HOME` (a *path*) and permits dangerous names like `-`, `.`
**File:** `src/core/paths.rs:87-93` used at `:104-110`
`is_valid_username` rejects `/`, `\0`, `..` and length >256 — reasonable for a username, but it is applied verbatim to `SUDO_HOME`, which is a path and legitimately may contain `/` (so any real absolute home path set via `SUDO_HOME` is rejected and warned about). Meanwhile relative junk like `foo` or `-` passes and becomes a cwd-relative cache root written as root.
**Fix:** split validators: charset rules for usernames; absolute-path + no-`..`-component rules for `SUDO_HOME`.

### L-6. `caps.rs` — turbo-hint file written without creating `data_dir`, failure silently ignored → hint reprints forever
**File:** `src/core/caps.rs:52-66`
```rust
let hint_file = crate::core::paths::data_dir().join(".turbo_hint_shown");
if hint_file.exists() { return false; }
... eprintln!(...) ...
if let Ok(mut file) = std::fs::File::create(&hint_file) { let _ = file.write_all(b"1"); }
```
If `data_dir()` doesn't exist yet (fresh install, first run), `File::create` fails, the error is swallowed, and the "one-time" tip prints on every invocation until something else creates the data dir. Also `exists()`-then-create is racy (benign).
**Fix:** `fs::create_dir_all(data_dir())` before writing, or track shown-state in an existing config/state file.

### L-7. `client.rs` — async/sync response error paths ignore the response `id`, allowing cross-request error attribution
**File:** `src/core/client.rs:186-190` and `:455-459`
```rust
Response::Error { id: _, code, message } => {
    anyhow::bail!("Daemon error ({code}): {message}");
}
```
Success responses are ID-checked, errors are not. On a shared/pooled connection a stale or pipelined error frame would be attributed to the current request. With strictly serial round-trips per connection this is currently benign, but the asymmetry weakens the protocol invariant the ID checking otherwise enforces.
**Fix:** compare `id` against the sent id and bail on mismatch even for errors.

### L-8. `client.rs` — fixed 30 s read/write timeouts on sync clients; long daemon operations would fail
**File:** `src/core/client.rs:37-42`
Both read and write timeouts are hardcoded 30 s with no way to extend per request. Today only fast queries go through sync clients, but nothing documents or enforces that constraint; adding e.g. `RefreshIndex` to `PooledSyncClient` would time out on large DBs.
**Fix:** make the timeout configurable per client or per call.

### L-9. `http.rs` — download client duplicates builder config instead of calling `build_client`
**File:** `src/core/http.rs:30-43` vs `:60-73`
`DOWNLOAD_CLIENT` hand-copies the exact same six builder settings that `build_client()` centralizes, defeating the stated purpose ("centralizes reqwest client configuration"). Drift risk when tuning pooling/timeouts.
**Fix:** `build_client(DOWNLOAD_TIMEOUT, DOWNLOAD_CONNECT_TIMEOUT, DOWNLOAD_READ_TIMEOUT)`.

### L-10. `error.rs` — suggestion matching on lowercased substrings over-fires
**File:** `src/core/error.rs:9-47`
E.g. any error containing "daemon" suggests starting the daemon even when the daemon was merely mentioned in a context chain ("failed to lock pacman db owned by daemon"); "not found" + "command" ordering means "command not found"-style tool errors only match when both words appear anywhere. Fragile text heuristics can produce misleading advice; acceptable for CLI sugar but worth noting since the typed-error contract was deliberately deleted.
**Fix:** none required short-term; consider attaching structured tags to errors at creation sites instead of substring matching.

### L-11. `types.rs` — `FromStr for RuntimeBackend` accepts inconsistent separator mixes
**File:** `src/core/types.rs:23-35`
```rust
"native-then-mise" | "native_then_mise" | "native_then-mise" => Ok(Self::NativeThenMise),
```
Accepts `"native_then-mise"` (mixed separators) while rejecting e.g. `"nativethenmise"` — harmless but sloppy; serde kebab-case round-trip accepts only `"native-then-mise"`, so configs parsed via FromStr and serde disagree on what's canonical.
**Fix:** accept exactly the kebab-case form plus maybe underscore variant; drop the mixed form.

### L-12. `format.rs` — binary sizes labeled with decimal SI prefixes
**File:** `src/core/format.rs:31-44`
1024-divisions labeled `KB/MB/GB/TB` (should be KiB/MiB…). Test suite locks in the mislabeling. Purely cosmetic but users comparing against pacman's own GiB-style numbers may be confused.
**Fix:** switch labels to IEC units or divide by 1000.

---

## INFO

### I-1. `privilege.rs` — large unused-in-production API surface
**Files:** `src/core/privilege.rs` — `PrivilegeChecker`, `SystemPrivilegeChecker`, `MockPrivilegeChecker`, `set_privilege_checker`, `get_privilege_checker`, `elevate_for_operation`, `with_root`, `elevate_if_needed` (~180 lines).
Repo-wide grep shows these have no production callers (production uses `run_self_sudo` / `run_privileged_child` directly, `src/bin/omg.rs:685`, apt/dnf/arch managers). All of it is compiled into release builds purely to support unit tests of itself. Dead abstraction weight plus the latent H-1 bug.
**Fix:** delete, or `#[cfg(test)]`-gate the DI machinery.

### I-2. `safe_ops.rs` — `validate_path` name overpromises; no traversal containment
**File:** `src/core/safe_ops.rs:56-74`
Only checks emptiness, UTF-8, and NUL (NUL is impossible in Rust `OsStr`-derived paths on Unix anyway — dead check). No rejection of `..`, no canonicalization (acknowledged in comment). Callers reading the name may assume traversal safety they don't get.
**Fix:** rename to `check_basic_path` or add explicit traversal rules per call-site contract.

### I-3. `client.rs` — empty `Drop for PooledSyncClient`
**File:** `src/core/client.rs:529-533`
Manual `Drop` impl that does nothing beyond what auto-drop provides; adds noise.
**Fix:** delete the impl.

### I-4. `client.rs` — `is_running()` treats disabled-daemon env as "not running"
**File:** `src/core/client.rs:139-141`
With `OMG_DISABLE_DAEMON=1`, `is_running()` returns false even if a daemon is actually up. Semantically defensible but undocumented.

### I-5. `sudoloop.rs` — `refresh_now` races the periodic loop
**File:** `src/core/sudoloop.rs:130-160`
An immediate refresh can run concurrently with the loop's own `sudo -v`; two sudo processes may prompt simultaneously in edge cases. Benign in practice (second validates cached timestamp) but worth a mutex if prompts appear.

### I-6. `paths.rs` — `prepare_socket_parent` creates only one level and races on `exists()` check
**File:** `src/core/paths.rs:395-413`
`DirBuilder.create` fails if a concurrent process creates the parent between the `exists()` probe and `create` (EEXIST). Daemon startup could spuriously fail once; retry-on-EEXIST would fix. Single-level creation is fine given `/tmp/omg-<uid>` and `$XDG_RUNTIME_DIR` layouts, but an `OMG_SOCKET_PATH=a/b/c/omg.sock` fails where `create_dir_all` would succeed.

### I-7. `caps.rs` — capability check only inspects `CAP_DAC_OVERRIDE`
**File:** `src/core/caps.rs:14-20`
Doc header advertises setup with `cap_dac_override,cap_fowner,cap_chown+ep`, but `has_package_caps` verifies only `DAC_OVERRIDE`. A binary with partial caps passes the "turbo" check then fails later mid-operation with opaque permission errors.
**Fix:** verify the full advertised set.

### I-8. `privilege.rs` — pre-flight `sudo -n -v` uses `Stdio::null()` stdin
**File:** `src/core/privilege.rs:315-320`
Correct for the intended non-interactive check, but means a `requiretty` sudoers setup makes the pre-flight fail and routes every run through the interactive path — fine, just noting environment sensitivity.

### I-9. `error.rs` — module retains misleading name
`src/core/error.rs` now contains only suggestion strings after the typed-error deletion (wave-9); the name suggests an error type module.

### I-10. `client.rs` — `extract_response`'s `request_id` parameter exists only for the error string
**File:** `src/core/client.rs:368-377`
Fine, but combined with L-7 the ID discipline is enforced in three separate places (async `call`, `sync_roundtrip`, extract); consolidating would reduce drift risk.

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 1 |
| MEDIUM | 6 |
| LOW | 12 |
| INFO | 10 |

No injection or secret-handling vulnerabilities found in this slice; the sudo env-scrubbing (`LD_PRELOAD` etc.) and socket-directory hardening are well done. Highest-value actions: fix the `elevate_if_needed` arg-offset trap or delete the dead elevation API (H-1/I-1), preserve file metadata in atomic writes (M-1/M-2), and reconsider the 5-minute hard download timeout (M-5).


---

# SLICE 08

# Audit slice-08 — `src/core` (audit08)

Scope: `archive.rs`, `container.rs`, `history.rs`, `metrics.rs`, `pacman_conf.rs`, `runtime_resolver.rs`, `sysinfo.rs`, `task_runner.rs`, `usage.rs`, `fast_status.rs`, `completion.rs`, `license.rs`, `testing/`, `mod.rs`. Read-only audit; no builds executed.

## HIGH

### H-1. `.omg.toml` ecosystem mapping that filters to zero matches silently falls through to fallback/passthrough execution
- File: `src/core/task_runner.rs` (in `TaskDetector::resolve`, ~lines 470–500)
- Excerpt:
  ```rust
  // 2. Filter by .omg.toml mapping
  if let Some(preferred_ecosystem) = self.config.scripts.get(task_name)
      && matches.iter().any(|task| task.ecosystem.matches(preferred_ecosystem))
  {
      matches.retain(|task| task.ecosystem.matches(preferred_ecosystem));
  }
  ```
  Unlike the `using` branch (which bails with an error when the filter empties the list), this branch has no post-filter emptiness check. If a stale/typo'd `.omg.toml` maps `test = "deno"` in a Node+Rust project, `resolve` returns `Ok(vec![])`, and `run_task_advanced` then treats the task as "not found": it runs the ordered fallback table and, ultimately, executes `task_name` as a raw PATH command. A config typo therefore silently runs a *different* program than the detected one.
- Why bug: inconsistent error handling between two equivalent filter paths; user-facing wrong-execution behavior.
- Fix: after the `.omg.toml` retain, if `matches.is_empty()`, bail with `"Task '{task_name}' mapped to unknown ecosystem '{preferred_ecosystem}'"` (mirroring the `using` branch).

### H-2. Background usage sync spawned via `tokio::spawn` can be silently dropped on short-lived current-thread runtimes
- File: `src/core/usage.rs` (`maybe_sync_background`, ~line 700)
- Excerpt:
  ```rust
  if stats.needs_sync() || stats.needs_immediate_sync() {
      tokio::spawn(async move {
          if let Err(e) = stats.sync(&license.key).await { ... }
      });
  }
  ```
  All call sites (`track_install_result` etc.) are in async fns, but several run under the scoped *current-thread* runtime created by `run_async` in `task_runner.rs`; on such a runtime, a task spawned right before `block_on` returns is never polled, so telemetry is silently lost (and any lock-held state is fine only because sync re-reads). Also, `tokio::spawn` panics if ever reached outside a runtime context (future sync callers).
- Fix: use `tokio::spawn` only when `Handle::try_current()` succeeds AND the runtime is multi-thread; otherwise run the sync inline or persist a "sync pending" flag consumed by the awaited `sync_usage_now`.

## MEDIUM

### M-1. `ensure_node_runtime`: system `node` shortcut ignores the requested version entirely
- File: `src/core/task_runner.rs` (~lines 940–960)
- Excerpt:
  ```rust
  // Check if Node is available via system first (nvm, fnm, volta, or system node)
  if which::which("node").is_ok() {
      return Ok(normalized.to_string());
  }
  ```
  If *any* system/nvm/fnm node exists, the function reports success for e.g. `20.11.1` without verifying that version is what will run. `execute_process` then injects nothing into PATH (native path already present) and the task runs whatever unrelated node is first on PATH. Same pattern in `ensure_bun_runtime`. Silent wrong-version execution contradicts the module's stated purpose ("ensures 'npm' uses the correct node version").
- Fix: verify the resolved system binary's version matches (`node --version`) before short-circuiting, or resolve the specific installation dir and prepend it.

### M-2. `extra_args` metacharacter ban rejects legitimate arguments
- File: `src/core/task_runner.rs` (`execute_process`, ~lines 745–765)
- Excerpt:
  ```rust
  if arg.contains(';') || arg.contains('|') || arg.contains('&')
      || arg.contains('`') || arg.contains('$') || arg.contains('\n') {
      anyhow::bail!("Invalid argument '{arg}' - contains shell metacharacters");
  }
  ```
  The command is spawned argv-directly with no shell (the code itself documents this), so these characters are inert. Yet users cannot pass e.g. `--define '$foo'`, sed expressions with `;`, grep patterns with `|`, etc. Security theater that breaks real workflows; the comment "SECURITY: prevent command injection" is misleading since no injection surface exists.
- Fix: drop the check (argv spawning is safe), or reduce it to rejecting NUL/control bytes only, matching `validate_executable_command`.

### M-3. `Command::new(cmd)` constructed *before* `validate_executable_command`
- File: `src/core/task_runner.rs` (~lines 720–735)
- Excerpt:
  ```rust
  let mut command = Command::new(cmd);
  // SECURITY: cmd is spawned argv-directly ...
  validate_executable_command(cmd)?;
  ```
  `std::process::Command::new` **panics** if the string contains an interior NUL byte. Validation that would have rejected NUL as a clean error instead happens after the panic point. Currently hard to reach (passthrough names are charset-validated), but the ordering defeats the validation's purpose and will abort the process rather than error if a manifest-sourced command ever reaches here.
- Fix: move `validate_executable_command(cmd)?` above `let mut command = Command::new(cmd);`.

### M-4. Public `HistoryManager::save` bypasses the cross-process lock
- File: `src/core/history.rs` (~lines 100–105)
- `add_transaction` carefully locks `history.json.lock` around load-modify-save, but the public `save(&self, history)` writes the whole file atomically with no lock. Any caller composing `load()` + mutate + `save()` races concurrent `add_transaction` writers and can drop their transactions (exactly the bug the lock was added to fix). The lock should be mandatory inside `save`, or `save` should be private/`pub(crate)` with documented single-writer constraints.

### M-5. `completion.rs` fetch of AUR package list has no timeout / uses unshared client
- File: `src/core/completion.rs` (`fetch_aur_names`, ~lines 210–225)
  ```rust
  let response = reqwest::get(url).await?;
  ```
  Uses ad-hoc `reqwest::get` instead of `crate::core::http::shared_client()` used everywhere else; no timeout means a hung AUR connection blocks completion indefinitely (interactive UX hang). Every other network path in the codebase applies explicit timeouts.
- Fix: use `shared_client().get(url).timeout(Duration::from_secs(...))`.

### M-6. Usage tracking performs redundant full-file saves per operation
- File: `src/core/usage.rs` (`track_runtime_switch`, ~lines 640–660)
  ```rust
  stats.record_runtime(runtime);            // -> save_best_effort()
  stats.record_specialized_command(...);    // -> save_best_effort()
  ```
  `record_runtime` calls `check_achievements` + `save_best_effort`, then `record_specialized_command` loads nothing but saves again. Two serialized JSON writes (whole file incl. growing `installed_packages` map) per runtime switch while holding the cross-process lock. Wasteful and widens the lock window.
- Fix: batch mutations then save once (e.g. make internal `_on` variants non-saving and save explicitly at the end of each `track_*`).

### M-7. `license.rs` team endpoints send the raw license key in JSON body while others use bearer auth
- File: `src/core/license.rs` (`propose_change`, `review_proposal`, ~lines 590–650)
  ```rust
  .json(&serde_json::json!({ "key": license.key, ... }))
  ```
  `licensed_get` correctly uses `.bearer_auth(&license.key)`; the POST endpoints embed the key in the body — inconsistent credential transport, more likely to end up in server-side request logging, and un-redactable from any intermediary that logs bodies. Pick one canonical mechanism (bearer header).

## LOW

### L-1. `history.rs`: doc comment for rollback logic attached to the wrong function
- File: `src/core/history.rs` (~lines 75–80). The paragraph "Collect deduplicated `(package, version)` pairs whose `old_version` appears in successful Remove/Update transactions…" sits directly above `pub fn load()` but describes `rollback_referenced_versions`. Misleading docs; move it.

### L-2. `pacman_conf.rs`: `Include` handling is shallow
- File: `src/core/pacman_conf.rs` (`resolve_servers`, ~lines 200–230)
  - Mirrorlist lines other than `Server =` are ignored; pacman mirrorlists also support comments-with-content fine but *nested `Include`* and `$arch` defaults are not handled.
  - Relative `Include` paths are read relative to the process CWD, not the conf file's directory — `PacmanConfig::parse` on a non-/etc/pacman.conf file with relative includes resolves differently from pacman itself.
- Fix: resolve includes relative to the conf/mirrorlist parent directory and document recursion limits.

### L-3. `sysinfo.rs`: field/message mismatch for the RAM recommendation
- File: `src/core/sysinfo.rs` (~lines 70–85): `disable_secure_makepkg` is set at ≥16 GB RAM while the explanation prints "disabling cleanbuild" — two different feature names; whichever is real, one of them misleads the init wizard user.

### L-4. `sysinfo.rs`: `is_tool_available` checks existence, not executability
- File: `src/core/sysinfo.rs` (~lines 130–145): `Path::new(dir).join(name).exists()` counts directories and non-executable files as available tools; PATH splitting on `':'` is Unix-only (fine for this codebase but undocumented).

### L-5. `runtime_resolver.rs`: `find_in_path` ignores the executable bit
- File: `src/core/runtime_resolver.rs` (~lines 65–75): `candidate.is_file()` accepts non-executable files, so mise/corepack availability probes (`mise_available`, `ensure_js_package_manager`) can false-positive on leftover data files named like binaries.

### L-6. `completion.rs`: `format_version` persisted but never validated on load
- File: `src/core/completion.rs` (`PersistedCompletionCache`, ~lines 15–40). A future v2 cache file is parsed as v1 (entries silently misread) because `load()` never checks `format_version`. Either validate-and-discard on mismatch or delete the dead field.

### L-7. `completion.rs`: misleading variable name `hours_since`
- File: `src/core/completion.rs` (~line 190): `let hours_since = Timestamp::now().as_second() - last_refresh.as_second(); if hours_since < 24 * 3600` — the value is seconds. Cosmetic but invites future unit bugs.

### L-8. `container.rs`: `-it` unconditionally for interactive configs breaks non-TTY contexts
- File: `src/core/container.rs` (`ContainerManager::run`, ~line 110): default `interactive: true` always passes `-it`; running omg container commands under CI/scripts/no TTY makes docker fail with "the input device is not a TTY". Should probe tty availability or accept `--tty=false`.

### L-9. `container.rs`: `build_with_options` validates nothing
- File: `src/core/container.rs` (~lines 150–180): unlike `run`/`pull`, `tag`, `build_args`, and dockerfile/context paths receive no validation. Argv-spawning prevents shell injection, but an option-like tag is passed as the value of `-t` so it is safe by position; still, the inconsistency with the file's own security posture (validate everything user-influenced) deserves at least `validate_image_ref(tag)`.

### L-10. `license.rs`: misleading 403 message for proposals fetch
- File: `src/core/license.rs` (`fetch_proposals`, ~line 660): passes `"Failed to fetch proposals"` as the `forbidden_message`, so a tier-gated 403 shows a generic failure instead of explaining the Team-tier requirement (all sibling functions pass explanatory messages).

### L-11. `license.rs`: corrupt license file silently downgraded after one warn
- File: `src/core/license.rs` (`load_license`, ~lines 330–360): malformed JSON → `None` with a `tracing::warn!`. This is integrity-bound paid-state data; per the project's own durability rule it should be preserved/quarantined for recovery rather than treated as absent (user must re-activate with no hint beyond a log line most CLI users never see).

### L-12. `metrics.rs`: `dec_active_connections` can drive the gauge negative
- File: `src/core/metrics.rs` (~line 95): plain `fetch_sub(1)` on `AtomicI64`; mismatched inc/dec pairs (e.g. early-return paths) produce negative gauges with no detection. Consider saturating or debug-asserting ≥0.

### L-13. `fast_status.rs`: status file written 0600, unreadable across users
- File: `src/core/fast_status.rs` (`write_to_file`): `NamedTempFile` persists with mode 0600; if daemon and interactive CLI run under different UIDs (or sudo), the zero-IPC fast path silently degrades to `None`. Consider 0644 (data is non-sensitive counters).

### L-14. `task_runner.rs` watch mode drops events for legitimately-named dirs
- File: `src/core/task_runner.rs` (`run_task_watch`, ~lines 1180–1200): any path component named `target`, `node_modules`, or `.git` anywhere in the tree suppresses the event — a project with source under e.g. `crates/parser/target-mappings/…`? (only exact component match, so mostly safe), but a monorepo with a real package directory literally named `target` loses all watch coverage there with no warning.

### L-15. `testing/helpers.rs`: `retry` panics when `max_attempts == 0`
- File: `src/core/testing/helpers.rs` (~lines 55–75): `Err(last_error.unwrap())` — with zero attempts `last_error` is `None` → panic instead of returning an error. Test-only, but the helper advertises `Result`.

### L-16. `testing/mocks.rs`: trait `install` diverges from `install_package` semantics
- File: `src/core/testing/mocks.rs` (~lines 90–110): `PackageManager::install` inserts into `installed` set without requiring the package exist in `packages` nor flipping `Package.installed`; `list_installed` joins against `packages`, so installed-but-unregistered names are invisible while `is_installed` says true. Tests relying on consistency between those two methods can pass/fail spuriously.

## INFO

### I-1. Stub JWT verification key is intentional but means all paid tiers are Free
- File: `src/core/license.rs` (~lines 30–36): `STUB_JWT_VERIFICATION_KEY` filler key; every license fails closed to Free with one warn. Documented and fail-closed — correct behavior, flagging as known product gap (shipping paid features gated behind a stub means Pro features are unusable for everyone).

### I-2. `usage.rs` telemetry payload breadth
- Sends machine_id, hashed hostname prefix, OS/arch, full `installed_packages` and `runtime_usage_counts` maps to `https://api.pyro1121.com/api/report-usage` whenever a valid license exists. HTTPS + hashed hostname; noted for privacy review. License key travels in the payload body (same concern as M-7 pattern).

### I-3. `Ecosystem::matches` Debug-format comparison for Custom variants
- File: `src/core/task_runner.rs` (~lines 50–90): `format!("{self:?}").to_lowercase()` yields `custom("x")` for `Custom(_)`; harmless today (Custom priority 0, never matched from config strings sensibly) but fragile if Custom ecosystems become configurable.

### I-4. `detect_js_package_manager` defaults to bun
- File: `src/core/task_runner.rs` (~lines 850–885): a bare `package.json` with no lockfile and no `packageManager` field selects **bun**, not npm — surprising default for the majority npm audience; tasks then run via `bun run`, which changes script-env semantics (e.g. `npm_*` env vars absent).

### I-5. Makefile parser limitations
- File: `src/core/task_runner.rs` (`detect_makefile_tasks`, ~lines 240–280): line-based parser ignores `\`-continued rule lines, `define … endef` blocks (targets inside may be misparsed), skips static-pattern rules whose left side is actually runnable (`objects: %.o := …` handled; `objects: $(objs): %.o: %.c` dropped due to `%` check), and misses targets containing dots (intentional but excludes legit phony names like `docker.build`).

### I-6. `pacman_conf.rs` duplicate-section/repo-name collisions not deduped
- Repeated `[core]` sections push two `RepoConfig`s with the same name; consumers iterating `repos` see duplicates. Matches pacman append semantics loosely; worth documenting.

### I-7. `mod.rs` gating of testing module
- `#[cfg(any(test, debug_assertions))] pub mod testing;` — test infrastructure ships in debug/profile-dev builds of release binaries built without `--release`; acceptable per comment, but `#[macro_export]` macros (`assert_err!`, `assert_ok!`, `async_test_timeout!`) land at crate root for those builds.

### I-8. `archive.rs` looks correct
- Rejects RootDir/Prefix/ParentDir components, skips CurDir, strips N leading normals, returns None on full consumption — no traversal escape found.

### I-9. `fast_status.rs` freshness check accepts future timestamps
- `now.saturating_sub(timestamp) > TTL` — a clock-skewed writer with timestamp far in the future stays "fresh" indefinitely until wall clock catches up. Minor.

### I-10. `usage.rs` streak logic counts parse-failure as streak start
- Unparseable `last_query_date` resets streak to 1 even mid-streak (corrupt date string silently restarts streaks). Cosmetic given data is self-produced.

---
**Totals:** 2 HIGH, 7 MEDIUM, 16 LOW, 10 INFO — 35 findings.


---

# SLICE 09

# slice-09 — security/, env/, packages/, telemetry (audit09)

Scope: `src/core/security/` (all), `src/core/env/` (all), `src/core/packages/`, `src/core/telemetry.rs`, `src/core/telemetry_client.rs`. Read-only audit.

## Findings

### 1. HIGH — Local `.pkg.tar.*` files bypass all policy and vulnerability checks
- **File:** `src/core/packages/service.rs:66-77` (`install`)
- **Excerpt:**
  ```rust
  if pkg.ends_with(".pkg.tar.zst") || pkg.ends_with(".pkg.tar.xz") {
      official.push(pkg.clone());
      changes.push(PackageChange { name: pkg.clone(), ..., new_version: Some("local".to_string()), source: "local" });
      continue;
  }
  ```
- **Why it is a bug:** The whole point of `PackageService::install` is to run `assign_grade` + `policy.check_package` before installing. Local package files short-circuit with `continue` and are handed straight to the backend install. A local file is exactly the highest-risk artifact (AUR-built, unsigned, attacker-supplied), yet it skips banned-package, minimum-grade, require_pgp, license, and vulnerability checks entirely. Also note only `.zst`/`.xz` suffixes are checked here while `validation::is_local_package_file` accepts `.gz`/`.bz2`/`.tar` — inconsistent bypass detection.
- **Fix:** Route local files through an explicit policy decision (at minimum banned-package and minimum-grade checks, treating them as `Community`), or refuse them when `require_pgp` is set; reuse `validate_package_name_or_file`/`is_local_package_file` instead of ad-hoc `ends_with`.

### 2. HIGH — PGP verification ignores the hash-algorithm policy (SHA-1 / deprecated algorithms accepted)
- **File:** `src/core/security/pgp.rs:196-205` (`signature_hasher`) used by `verify_detached`/`verify_memory`
- **Excerpt:**
  ```rust
  Ok(sig.hash_algo().context()
      .map_err(|source| PgpError::HashContext { source: SequoiaSource(source) })?
      .for_signature(sig.version()))
  ```
- **Why it is a bug:** Key validity uses `StandardPolicy`, but signature hash algorithms are accepted whenever Sequoia can *construct* a hash context — which includes SHA-1 and other deprecated algorithms that `StandardPolicy` rejects. An attacker who can obtain/forge any SHA-1-signed artifact path gets a verification primitive the policy layer intends to forbid. Hand-rolled packet parsing also skips signature expiry checks and key-liveness-at-signing-time.
- **Fix:** Reject signatures whose `hash_algo()` is not acceptable under the same `StandardPolicy` (`policy.accept_signature_hash`), and check `sig.signature_creation_time()` against key lifetime before verifying.

### 3. MEDIUM — ALSA version check compares exact equality; misses nearly all vulnerable installs
- **File:** `src/core/security/vulnerability.rs:238-251` (`scan_system`)
- **Excerpt:**
  ```rust
  // Simple version check: if it matches the 'affected' exact version
  if local_pkg.version.to_string() == issue.affected {
      vuln_count += 1;
  ```
- **Why it is a bug:** ALSA advisory data identifies affected ranges; an installed system is vulnerable for every version below the fix, not only the single exact string. Any installed version older *or newer* than the literal `affected` string reports clean. A "0 vulnerabilities" result is shown for systems riddled with known-vulnerable packages — a fail-open in a security surface.
- **Fix:** Use `alpm_pkg_vercmp`-style comparison (`installed < fixed`) or OSV per-package queries as the source of truth; exact-match should at most be a documented last resort, never the sole check.

### 4. MEDIUM — SBOM vulnerability matching ignores installed versions (false positives)
- **File:** `src/core/security/sbom.rs:316-345` (`generate_system_sbom`)
- **Excerpt:**
  ```rust
  if let Some(pkg) = installed.iter().find(|p| p.name == *pkg_name) {
      ... vulnerabilities.push(SbomVulnerability { id: issue.name.clone(), ...
          affects: vec![SbomVulnAffects { affects_ref: bom_ref }] });
  ```
- **Why it is a bug:** Unlike `scan_system` (which at least attempts a version comparison), SBOM generation marks every installed package sharing a name with an ALSA issue as affected, regardless of version — including already-fixed versions. Compliance exports overstate exposure; the inconsistency between the two consumers of the same data is itself a defect.
- **Fix:** Share one version-aware matcher (see finding 3) between `scan_system` and SBOM generation.

### 5. MEDIUM — Vulnerability grading on non-Arch/non-Debian systems silently queries the wrong OSV ecosystem
- **File:** `src/core/security/vulnerability.rs:276-286` (`scan_package`)
- **Excerpt:**
  ```rust
  if cfg!(any(feature = "debian", feature = "debian-pure")) && crate::core::env::distro::is_debian_like() {
      return Err(VulnerabilityError::Unavailable { reason: "OSV ecosystem ... refusing to scan Debian packages against the Arch Linux ecosystem".into() });
  }
  ...
  ecosystem: "Arch Linux".to_string(),
  ```
- **Why it is a bug:** Debian-like systems fail closed (good), but Fedora, openSUSE, Alpine, macOS etc. fall through and get scanned against the **Arch Linux** ecosystem. Results are wrong-but-plausible (usually empty → "no vulns") and feed directly into `SecurityPolicy::assign_grade`, minting `Verified` grades from irrelevant evidence.
- **Fix:** Gate on `detect_distro() == Arch` (or map distro→OSV ecosystem); return `Unavailable` for unsupported ecosystems like the Debian branch does.

### 6. MEDIUM — Audit log permanently fails closed after a single corrupt line (event-loss DoS)
- **File:** `src/core/security/audit.rs:432-452` (`read_all_entries` via `get_last_hash` ← `log_locked`), and `audit.rs:589-601` (`record_global`)
- **Why it is a bug:** Every append re-reads the full log to find the tail hash. One corrupt/truncated line (crash mid-write of another writer is impossible here due to sync_all, but disk corruption, partial line from an external editor, or an attacker-writable data dir suffices) makes `get_last_hash` return `CorruptLine` forever. `record_global` then logs a warning and **drops every subsequent audit event** — silent, permanent loss of the tamper-evident trail with no recovery path exposed.
- **Fix:** Distinguish recoverable tail damage (e.g., truncate an incomplete final line under the lock) and/or surface a loud typed error/quarantine mode instead of dropping events indefinitely.

### 7. MEDIUM — Team member identity is the local username; cross-machine collisions overwrite teammates' status
- **File:** `src/core/env/team.rs:352-368` (`update_status`)
- **Excerpt:**
  ```rust
  if let Some(existing) = status.members.iter_mut().find(|m| m.id == member.id) {
      *existing = member;
  } else { status.members.push(member); }
  ```
- **Why it is a bug:** `member.id` defaults to `whoami::username()`. Two machines with the same account name (`deploy`, common first-name accounts) silently replace each other's entry in shared team status, corrupting drift reporting for real teammates. There is no uniqueness guarantee for a *team* scope.
- **Fix:** Derive member identity from something machine-scoped (machine-id hash + username) or require an explicit unique `member_id` at `init`.

### 8. LOW/MEDIUM — Gist remote URL matched by substring
- **File:** `src/core/env/team.rs:479-489` (`pull`)
- **Excerpt:**
  ```rust
  if remote_url.contains("gist.github.com") {
      super::super::super::cli::env::sync(remote_url.clone()).await?;
  ```
- **Why it is a bug:** `contains` accepts `https://evil.example/?gist.github.com`, `https://gist.github.com.evil.example/…`, etc. The guard gives a false sense of allowlisting while handing an arbitrary URL to `sync`.
- **Fix:** Parse with `Url` and compare scheme + host equality against `gist.github.com`.

### 9. LOW — Telemetry queue drop-oldest can discard never-sent events counted as sent
- **File:** `src/core/telemetry.rs:262-277` (`EventQueue::push`) vs `flush_events` (`telemetry.rs:652-690`)
- **Why it is a bug:** `flush_events` snapshots N events, then awaits the network **without holding the queue lock** (by design). If pushes during the await trigger the `MAX_QUEUE_SIZE` eviction (`drain(0..drop_count)`), `confirm_sent(N)` drains N items from a deque whose oldest entries were already evicted — deleting up to `drop_count` *newer* events that were never transmitted. Silent event loss under burst load.
- **Fix:** Snapshot-and-remove atomically under the queue mutex before the network call (re-insert on failure), or tag events with IDs instead of positional draining.

### 10. LOW — `require_pgp` is satisfied by repo-metadata `Verified` grade without any actual signature check
- **File:** `src/core/security/policy.rs:186-200` (`assign_grade`) + `check_package` (`policy.rs:224-228`)
- **Why it is a bug:** `assign_grade(is_official=true)` returns `Verified` purely because a package came from an official repo index; `check_package` then treats `grade >= Verified` as satisfying `require_pgp`. No `PgpVerifier` runs on this path. The comment acknowledges it, but from the user's perspective `require_pgp = true` promises signature enforcement it does not deliver for the service-level install flow.
- **Fix:** Rename the flag/grade semantics or thread actual signature evidence into grading.

### 11. LOW — License AND-expressions pass the allowlist on a single token
- **File:**** `src/core/security/policy.rs:247-264` (`license_matches_allowlist`)
- **Excerpt:** any token match passes, e.g. `"MIT AND GPL-3.0-or-later"` passes an allowlist of `["MIT"]`.
- **Why it is a bug:** For `OR`, token-match is correct SPDX semantics (choose MIT). For `AND`, all branches' obligations apply; passing on one identifier misstates compliance. The tokenizer drops `AND`/`OR` so they cannot be distinguished downstream.
- **Fix:** Keep the operator tokens and require all branches of `AND` groups to be allowed.

### 12. LOW — Secret scanner aborts the whole directory scan on one unparseable scannable file
- **File:** `src/core/security/secrets.rs:247-300` (`scan_directory_recursive` → `scan_file`)
- **Why it is a bug:** `read_to_string` fails on invalid UTF-8; any binary-ish file with a scannable extension (`.txt`, `.json`, `.env` logs, minified assets) fails the entire scan via `?`. Deliberately "fail closed", but operationally this lets one stray file prevent scanning everything else — attackers/users lose coverage rather than one file. Also `.git` is pruned but `.svn`/`.hg`/`.terraform`/`dist` are not; symlinked files inside a scanned tree are silently skipped (documented).
- **Fix:** Collect per-file read errors into the result (skip + warn) while keeping hard failures for the root; extend prune list.

### 13. LOW — `is_local_package_file` doc claims "paths to actual .pkg.tar.* files on disk" but existence is never checked
- **File:** `src/core/security/validation.rs:117-141`
- **Why it is a bug:** Only prefix `/`, extension suffix, and no `..` are verified. Combined with finding 1's policy bypass, a nonexistent or arbitrary absolute path ending in `.pkg.tar.zst` is treated as a trusted local artifact class. Doc/comment mismatch; also trailing-slash or whitespace-only names within package names (`"foo/"`) pass `validate_package_name`.
- **Fix:** Either check `Path::new(name).is_file()` where used, or correct the documentation and keep the pure-syntax contract explicit.

### 14. LOW — Duplicate certs accumulate unboundedly in the local keyring
- **File:** `src/core/security/keyserver.rs:383-414` (`append_to_keyring`)
- **Why it is a bug:** Appends with no dedup and no size bound; repeated fetches of the same key ID grow the keyring file forever and linearly slow `is_key_in_keyring`. No inter-process locking either (two concurrent appends rely on O_APPEND atomicity).
- **Fix:** Check `is_key_in_keyring` before append (or dedup by fingerprint) and hold a lock file during append.

### 15. LOW — `fetch_keys` returns results in nondeterministic order
- **File:** `src/core/security/keyserver.rs:347-360`
- **Why it is a bug:** `buffer_unordered` means output order ≠ input order; the `(String, Result)` pairs make it recoverable, but any caller zipping results positionally breaks. Not documented.
- **Fix:** Document explicitly, or sort results back into input order.

### 16. LOW — Dead config: `TeamConfig.auto_sync` is never read
- **File:** `src/core/env/team.rs:27`, set at lines 41/231, referenced only in tests.
- **Why it is a bug:** Users can set `auto_sync = false` in `.omg/team.toml` and nothing changes — the post-merge/post-checkout git hooks always fire `omg env check` regardless. Misleading persisted setting.
- **Fix:** Honor the flag in `install_git_hooks`/hook behavior or remove the field.

### 17. LOW — `git_commit_lock` treats "nothing to commit" as an error
- **File:** `src/core/env/team.rs:561-594`
- **Why it is a bug:** With `auto_push = true`, a second `push()` with an unchanged lock makes `git commit` exit non-zero ("nothing to commit") and `push()` returns `Err` even though the lock was saved successfully. UX-breaking spurious failure.
- **Fix:** Treat the specific "nothing to commit" outcome as success (or check `git status --porcelain` first).

### 18. LOW — Weak/deprecated-hash and issuer-less signatures: `issuers.is_empty()` matches all certs
- **File:** `src/core/security/pgp.rs:216-222` (`matches_any_trusted_cert`)
- **Why it is a bug:** A signature packet stripped of its Issuer subpacket is tried against every trusted signing key. Cryptographically still safe (verification must succeed), but it defeats the fast-path rejection and multiplies work O(certs×keys) per packet — DoS-amplification when scanning many sig packets against a large distro keyring.
- **Fix:** Skip sigs with empty issuers unless the cert set is small, or treat missing issuer as an error.

### 19. LOW — Distro keyring paths are likely wrong on real systems
- **File:** `src/core/security/pgp.rs:88-97` (`system_keyring_path`)
- **Excerpt:** `"/etc/pki/rpm-gpg/RPM-GPG-KEY-fedora"` (real files are release-versioned, e.g. `RPM-GPG-KEY-fedora-41-*`), Arch path `/usr/share/pacman/keyrings/archlinux.gpg` vs pacman's actual `/usr/share/pacman/keyrings/` layout.
- **Why it is a bug:** On stock Fedora these paths don't exist → `PgpVerifier::new()` fails `KeyringMissing`, degrading every dependent flow to errors rather than working verification.
- **Fix:** Glob/version-detect the real keyring files per distro, or ship/bundle the needed keys.

### 20. INFO — Audit entry hashes depend on Rust `Debug` formatting of enums
- **File:** `src/core/security/audit.rs:150-167` (`compute_hash`: `format!("{:?}", self.event_type)`)
- **Why:** Renaming a variant or changing derive output invalidates verification of all historical log entries. Consider serializing stable snake_case names (already available via serde renames).

### 21. INFO — Doc/count mismatch: "20 secret types"
- **File:** `src/core/security/secrets.rs:2` — header says 20 types; enum/pattern list has 19 (`SecretSeverity::Low` exists but no pattern assigns it, so `low_count` in `SecretScanResult` is always 0).

### 22. INFO — Raw matched secrets stored in serializable findings
- **File:** `src/core/security/secrets.rs:64-84` — `matched_text` (full secret) is `Serialize` on `SecretFinding`/`SecretScanResult`; persisting scan results (JSON export, daemon API) leaks live credentials despite the "do not print" comment. Consider `#[serde(skip)]` or redact-on-serialize.

### 23. INFO — `sanitize_package_name` collision hazard exported
- **File:** `src/core/security/validation.rs:170-177` — deletion-based sanitizer can map hostile input onto an unrelated valid package name; the doc warns but the function remains callable. Prefer removal or internal-only use.

### 24. INFO — `validate_image_ref` reuses package-name error variants
- **File:** `src/core/security/validation.rs:96-118` — image-ref failures surface as `PackageNameEmpty`/`PackageNameTooLong`/`PackageNameStartsWithDash` ("Package name..."), producing misleading messages for image inputs. Also max length hardcoded 256 vs named constants elsewhere.

### 25. INFO — Minor audit-log race in readers
- **File:** `src/core/security/audit.rs:377-407` — `verify_integrity`/`get_recent` read without taking the `.lock`; concurrent appends can yield a partially observed tail (last line is flushed+synced per write, so torn lines are unlikely but a reader can miss the newest entry or observe chain head mid-update). Cosmetic given tamper-evidence goals; take the shared lock for consistency.

### 26. INFO — Enhanced telemetry loads/parses the license from storage on every gated call
- **File:** `src/core/telemetry.rs:470-475` (`is_enhanced_telemetry_enabled` → `load_license()`), called per tracked command/performance/feature event and again in `should_send_telemetry` (`telemetry_client.rs:198-209`) per batch.
- **Why:** Repeated synchronous disk I/O on hot paths; also TOCTOU between gating and send. Cache the license decision per process.

### 27. INFO — `query_rekor` does not validate `artifact_hash` format
- **File:** `src/core/security/slsa.rs:289-310` — any string is embedded into `sha256:{}` JSON body; server-side garbage-in/garbage-out, no injection risk (JSON-encoded) but malformed input produces confusing upstream failures.
- **Fix:** Require 64 lowercase hex chars before querying.

## Summary
28 findings: 0 CRITICAL, 2 HIGH, 5 MEDIUM, 12 LOW, 9 INFO (counting 1–27 plus items noted inline). Highest-priority fixes: local-package policy bypass (#1), PGP hash-policy gap (#2), and the two ALSA version-matching defects (#3, #4).


---

# SLICE 10

# Audit slice-10: `src/daemon/` (server, protocol, handlers, db, cache, index, status_policy)

Auditor: audit10 · Scope: every line of `/home/pyro1121/Documents/omg/src/daemon/*.rs` (~3,585 LOC). READ-ONLY audit; no builds/tests run.

## Findings

### 1. MEDIUM — Debian search caches limit-truncated results under the bare query key (cross-request cache poisoning)
**File:** `src/daemon/handlers.rs`, lines ~288–345 (`handle_debian_search`)
```rust
let searched = tokio::task::spawn_blocking(move || task_index.search(&query_for_task, limit)).await;
...
state.with_current_index(&index, || {
    state.cache.insert_debian_arc(query, Arc::clone(&results));
});
```
The full (already limited) result set is cached under the query string alone. The first requester's `limit` becomes baked into the cache entry; a later `DebianSearch { query: "git", limit: 50 }` after an earlier `limit: 5` request is served from cache and truncated via `.take(limit)` to only 5 results. Contrast with `handle_search`, which deliberately searches with `MAX_SEARCH_LIMIT` and stores the full set ("Cache the full result set; serve truncated views per request limit"). This is exactly the bug the search path fixed.
**Fix:** mirror `handle_search`: run `index.search(&query, MAX_SEARCH_LIMIT)`, insert that into `debian_cache`, and `.take(limit)` on both hit and miss paths.

### 2. MEDIUM — Oversized-deserialized request is dropped without any response; client stalls until timeout
**File:** `src/daemon/server.rs`, lines ~470–490 (`handle_client`)
```rust
if estimated_size > MAX_DESERIALIZED_SIZE {
    let msg = format!(...);
    tracing::warn!("{}", msg);
    audit_log(...);
    GLOBAL_METRICS.inc_requests_failed();
    continue;                       // <-- no Response::Error sent
}
```
Every other malformed/rejected input path in this function answers once (parse error, version mismatch, rate limited) precisely so the client doesn't hang. Here the loop just `continue`s, so a well-formed client waits the full client-side timeout for a reply that never comes. Inconsistent contract, breaks UX for the affected request and any pipelined requests behind it are still processed (client can't correlate).
**Fix:** send `Response::Error { id: request.id(), code: PARSE_ERROR or INVALID_PARAMS, message: msg }` before continuing (or break).

### 3. MEDIUM — Status memory-cache TTL (120s) shorter than worker refresh interval (300s): uncached heavy system queries between refreshes
**Files:** `src/daemon/cache.rs` line ~180 (`Self::new_with_ttls(1000, 300, 120)`), `src/daemon/handlers.rs` (`handle_status` step 3), `src/daemon/server.rs` (`STATUS_REFRESH_INTERVAL = 5 min`)
The background worker refreshes status every 5 minutes and writes both memory and persistent cache. But the memory status entry expires after 120 s. From t=2 min to t=5 min of each cycle, every `Status` request misses memory, reads the persistent snapshot (only rewritten every 5 min — often also stale), or, when the persistent file was invalidated by RefreshIndex, falls through to `system_status_for_backend()` — a full native package-database query executed per request via `spawn_blocking`. That snapshot is intentionally not cached (`debug_assert!(!cacheable)`), so repeated status calls each pay the full cost. On Arch this means repeated ALPM sync-db scans; a burst of `omg status` clients multiplies it.
**Fix:** align the memory status TTL with `STATUS_REFRESH_INTERVAL` (the test in server.rs already asserts the FastStatus file TTL equals it); or make the on-demand path publish its (scan-less) result to the memory cache with a short TTL instead of never caching.

### 4. MEDIUM — No peer authentication on the Unix socket: any local user can control the daemon
**File:** `src/daemon/server.rs` (whole accept path), socket creation outside slice but relevant here
No `SO_PEERCRED` check is performed on accepted connections. Any local account able to reach the socket path can invoke `CacheClear`, `RefreshIndex` (forces a full index rebuild + AlpmWorker respawn — a cheap local DoS lever, repeatable at will within rate limits), read full system package inventory, and hammer `SecurityAudit` (network vulnerability lookups for every installed package). Rate limiting bounds but does not prevent this.
**Fix:** after `accept()`, query `SO_PEERCRED` and reject peers whose uid differs from the daemon's euid (or the socket-owner uid).

### 5. LOW — Rate limiting applied *after* deserialization: parse cost is unthrottled
**File:** `src/daemon/server.rs`, `handle_client`: order is frame-decode → `bitcode::deserialize` → heap-size check → **then** `rate_limiter.check()`.
A client can stream up to `CLIENT_BURST_SIZE`+ valid-framed payloads and force deserialization attempts on all of them before any limiter fires; malformed-payload paths (`PARSE_ERROR`) also bypass the limiter entirely (they `break`, but one connection can reconnect repeatedly — the global limiter in `handle_request` never sees undecodable frames either).
**Fix:** move the per-connection `rate_limiter.check()` ahead of deserialization (check once per received frame), keeping the id extraction for the error envelope best-effort.

### 6. LOW — Double-counted cap in trigram-branch description scan
**File:** `src/daemon/index.rs`, `search()` (~line 380)
```rust
if name_match_count + scored_matches.len() >= limit * 4 { break; }
```
`scored_matches` already contains the name matches that `name_match_count` counts, so the sum double-counts them; the effective description-match budget is `limit*4 - 2*name_matches` rather than the intended `limit*4` total, and the condition is evaluated *before* processing each item, so results depend on where matches happen to sit in the item list. Description-only results are silently truncated at an arbitrary, position-dependent point (also true of the intent itself).
**Fix:** track a separate `desc_match_count` and compare `scored_matches.len()` against the intended budget; document that description scan is heuristic.

### 7. LOW — Short-query (<3 chars) branch pushes unbounded name matches and inconsistently skips description matches
**File:** `src/daemon/index.rs`, `search()` else-branch (~line 395)
```rust
for (idx, item) in self.items.iter().enumerate() {
    if let Some(score) = Self::score_name_match(...) { scored_matches.push(...); name_match_count += 1; }
    else if name_match_count < limit { /* desc match push */ }
}
```
(a) Name matches are pushed for *every* item regardless of `limit`; a 1–2 char query like `"li"` over a 100k-package index collects tens of thousands of scored matches before the post-loop `select_nth_unstable` truncation — wasted CPU/memory inside the daemon's blocking pool. (b) Once `name_match_count >= limit`, description matches stop being collected mid-scan, so which descriptions appear depends on item ordering — non-deterministic-looking output.
**Fix:** bound total pushes (e.g. stop after collecting `> limit*4` candidates) and use a single consistent candidate cap for description matches.

### 8. LOW — TOCTOU window between index swap and system-backend swap in RefreshIndex
**File:** `src/daemon/handlers.rs`, `handle_refresh_index`
```rust
let packages = state.replace_index(index);
state.refresh_system_backends();
```
Between the two calls, requests observe the freshly rebuilt index (post-sync data) but the pre-sync `AlpmWorker`, whose frozen libalpm syncdbs produce a stale update list — the exact staleness the comment says this code exists to prevent. Similarly `handle_list_updates` clones the worker handle and later re-reads `system_backends` for the IgnorePkg filter; a concurrent refresh can pair a new config parse with old worker output (benign) or vice versa.
**Fix:** perform both swaps while holding the `system_backends` write lock (or a single combined state lock) so publication is atomic.

### 9. LOW — Connection-limit refusals and frame-error closes give the client no signal
**File:** `src/daemon/server.rs`
When `connection_permits.try_acquire_owned()` fails the stream is silently dropped (client blocks until its own timeout); likewise the frame-decode `Err` path breaks without answering (documented as intentional for desync, but oversize-frame errors from `LengthDelimitedCodec` are recoverable-length errors, not desync). A brief `Response::Error{code: RATE_LIMITED / INTERNAL_ERROR}` before drop would let clients fail fast instead of hanging.
**Fix:** best-effort error response before dropping on refusal; distinguish codec oversize errors from I/O errors.

### 10. LOW — Failed vulnerability scan keeps stale count indefinitely
**File:** `src/daemon/status_policy.rs`, `vulnerability_count_from_scan`
On persistent scanner failure the previous count is republished every cycle forever (`previous_vulns` is read back out of the cache each round), with only a `warn!`. `StatusResult.vulnerabilities_scanned` stays `true`, so clients cannot distinguish a fresh scan from a weeks-old count. Documented trade-off, but there is no age/staleness signal on the wire.
**Fix:** add a scan timestamp (or `scan_age_secs`) to `StatusResult`, or flip `vulnerabilities_scanned` to false after N consecutive failures.

### 11. LOW — `DaemonState::new()` accepts an unsupported package manager at startup
**File:** `src/daemon/handlers.rs`, `DaemonState::new`
`get_package_manager()?` may yield a backend whose name isn't handled by `native_backend_query!` (e.g. an unrecognized distro name). Startup succeeds; every `Status`/`Explicit`/`ExplicitCount` request then fails at runtime with "Unsupported package manager". Fail-fast at construction would be strictly better.
**Fix:** validate `package_manager.name()` against the backends compiled into this build during `new()` and return a startup error otherwise.

### 12. INFO — `MAX_DESERIALIZED_SIZE` guard is dead code given the 1 MiB frame cap
**Files:** `src/daemon/server.rs` (`MAX_REQUEST_SIZE = 1 MiB`, `MAX_DESERIALIZED_SIZE = 10 MiB`)
A decoded frame can never exceed 1 MiB, so `request.heap_size()` (≈ frame payload size + stack size) can never exceed 10 MiB. The "compression bomb" guard can never fire; it adds a check plus tests implying protection that structurally cannot be needed.
**Fix:** remove the guard or reduce `MAX_DESERIALIZED_SIZE` to just above `MAX_REQUEST_SIZE` and document it as defense-in-depth.

### 13. INFO — Bloom filter false-positive rate ≈ 3% with k=3, m=8n bits
**File:** `src/daemon/index.rs`, `PackageBloomFilter::new`
`num_bits = expected_items * 8` with 3 probes gives FPP ≈ (1−e^(−3/8))³ ≈ 3.1%. Harmless (false positives fall through to a hash-map lookup), but if the filter were ever relied on for correctness-sensitive skipping, 8n/k=3 is undersized. Also note `PackageBloomFilter::hash_positions` derives h2/h3 by multiplying h1 — correlated hashes slightly worsen the real FPP.
**Fix:** fine as-is; consider k=4/m=16n or double hashing if the filter gains semantic weight.

### 14. INFO — Only SIGINT/SIGTERM handled; SIGHUP ignored
**File:** `src/daemon/server.rs`, `wait_for_termination_signal`
A daemonized omg receiving SIGHUP (terminal hangup of its parent session) will not shut down and will not clean up its socket file until the socket-existence health check notices an external removal (which it won't — the socket still exists). If the process is killed with SIGKILL, stale socket handling depends on external code outside this slice.
**Fix:** register `SignalKind::hangup()` alongside terminate, or document why HUP is ignored.

### 15. INFO — `Request` id 0 doubles as reserved error-envelope id
**Files:** `src/daemon/server.rs` (error responses with `id: 0`), `src/daemon/protocol.rs`
Nothing prevents a legitimate client from sending `id: 0`, so a PARSE_ERROR reply for a version-mismatched/garbled frame is indistinguishable on the wire from a reply to a real request 0. Cosmetic ambiguity only (those replies are sent immediately before close).
**Fix:** document id 0 as reserved in the protocol docs and have clients start ids at 1.

### 16. INFO — `StringPool::get` panics on out-of-range handle
**File:** `src/daemon/index.rs`
`&self.strings[handle as usize]` indexes unchecked; handles come only from `intern`, so unreachable today, but a corrupt/u32-overflowing handle (>4 Gi entries) would panic the blocking pool thread rather than erroring.
**Fix:** acceptable; add a debug_assert or use `.get()` with fallback if paranoia is desired.

### 17. INFO — `stats().size` is eventually-consistent and `max_size` is per-sub-cache
**File:** `src/daemon/cache.rs`, `stats()`
Documented in-code, but consumers (Health thresholds `HEALTH_DEGRADED_CACHE_THRESHOLD` = 50k / unhealthy = 100k) compare an aggregate across 7 sub-caches against a per-sub-cache capacity — with default capacity 1000/sub-cache the aggregate max is ~7×1000, so the degraded/unhealthy cache-size branches can essentially never trigger via normal operation; they'd only fire if `PackageCache::new` were constructed with huge capacities. Health "degraded/unhealthy" classification is therefore effectively driven only by `requests_failed`.
**Fix:** define the health threshold against actual configured totals, or report per-cache sizes.

### 18. INFO — `update_status` couples explicit-count freshness to status TTL mismatch
**File:** `src/daemon/cache.rs`, `update_status` writes `explicit_count` (TTL 300 s) whenever a status lands (TTL 120 s). After the status entry expires but the count hasn't, `ExplicitCount` serves a value whose epoch ended 2 minutes prior — benign because both originate from the same worker cycle, but the coupling is implicit and undocumented on the count accessors.
**Fix:** comment or split keys per epoch.

### 19. INFO — Synchronous Debian index pre-warm inside `from_index` delays daemon startup
**File:** `src/daemon/handlers.rs`, `from_index` (`ensure_index_loaded()` called inline). Large apt caches block daemon readiness; startup-time work could be moved to the existing background worker's first tick.
**Fix:** optional; low impact since clients retry/wait.

### Positive notes (no action)
- `with_current_index` epoch-guard preventing stale-search cache resurrection after index refresh is correct and well-tested.
- Version-prefixed framing with answer-once-then-close on mismatch is a good contract.
- `status_policy` correctly refuses to invent zero vulnerabilities from failed scans.
- `search(limit == 0)` guard fixes a documented earlier panic (`limit - 1` underflow).
- Accept-loop errno classification and bounded connection semaphore are sound.

## Summary
19 findings: 0 CRITICAL, 0 HIGH, 4 MEDIUM (#1 debian-search cache poisoning, #2 missing response on oversized request, #3 status TTL/refresh mismatch causing per-request heavy queries, #4 no socket peer auth), 8 LOW, 7 INFO.


---

# SLICE 11

# Audit slice-11 — `src/hooks/` (incl. completions/) and `src/runtimes/`

Read-only audit. All paths relative to `/home/pyro1121/Documents/omg`.

---

## MEDIUM

### M1. Zsh hook cache never refreshes when `$EPOCHSECONDS` is unset
- File: `src/hooks/mod.rs:712-724` (ZSH_HOOK, `_omg_refresh_cache`)
- Code:
  ```zsh
  local now=$EPOCHSECONDS
  (( now - _OMG_CACHE_TIME < 60 )) && return
  _OMG_CACHE_TIME=$now
  ```
- Why it is a bug: `$EPOCHSECONDS` only exists after `zsh/datetime` is loaded (`zmodload zsh/datetime`). The hook never loads it. Without it, `now` is empty (arithmetic 0), so `0 - 0 < 60` is always true and the function returns on the very first call — `_OMG_TOTAL/_OMG_EXPLICIT/...` stay at their initialized `0` forever and `omg-ec`, `omg-tc`, etc. permanently report 0 packages. Also, the first call sets `_OMG_CACHE_TIME=0` and returns *before* ever reading the status file, so even a fresh shell shows zeros until something else refreshes.
- Fix: add `zmodload -F zsh/datetime b:zsh/datetime` (or `(( $+EPOCHSECONDS )) || zmodload zsh/datetime`) at hook top; read the file before setting `_OMG_CACHE_TIME`; use `${EPOCHSECONDS:-$(date +%s)}` as fallback.

### M2. Status-file fallback to world-writable `/tmp` is spoofable
- Files: `src/hooks/mod.rs:716, 731-753` (ZSH_HOOK), `770-786` (BASH_HOOK)
- Code: `local f="${XDG_RUNTIME_DIR:-/tmp}/omg.status"`
- Why it is a bug: when `XDG_RUNTIME_DIR` is unset (common in cron, su sessions, some terminals), every user on a multi-user box can pre-create `/tmp/omg.status`. Any local user can feed arbitrary counts into other users' prompts (`omg-ec`, prompt integrations). Not memory-unsafe, but an unauthenticated local spoofing vector with no ownership/permission check before reading.
- Fix: refuse the file unless `[[ -O "$f" ]]` (owner check) or drop the `/tmp` fallback entirely; also verify it's a regular file not a symlink.

### M3. `nvm_node_bin` uses NVM alias contents as a path component without validation
- File: `src/hooks/mod.rs:573-591`
- Code:
  ```rust
  let resolved = match resolve_nvm_alias(&nvm_dir, alias)? { Some(alias) => alias, None => version.to_string() };
  let normalized = resolved.trim_start_matches('v');
  let bin_path = nvm_dir.join("versions/node").join(format!("v{normalized}")).join("bin");
  Ok(...is_valid_version_dir(&bin_path)...then_some(bin_path))
  ```
- Why it is a bug: unlike every native path in this module, the resolved value is **not** passed through `validate_runtime_version`. An `.nvmrc` pin of `lts` whose `~/.nvm/alias/lts` file contains `../../foo` yields `~/.nvm/versions/node/v../../foo/bin`, which escapes the versions tree; if that directory exists, its path is emitted onto the user's PATH by `hook_env`. Exploitation requires write access to `~/.nvm`, but the module's own stated invariant ("version string ... must pass validate_runtime_version before it may become a path component", hooks/mod.rs:470-476) is violated here.
- Fix: apply `validate_runtime_version(&resolved)` (or reuse `node_version_bin_path`) before building the path.

### M4. `parse_rust_toolchain_file` accepts comment lines and any line containing "channel"
- File: `src/hooks/mod.rs:248-265`
- Code:
  ```rust
  if line.contains("channel") && let Some(version) = line.split('=').nth(1) {
      let v = version.trim().trim_matches('"').trim_matches('\'');
      versions.insert(runtime.to_string(), v.to_string());
  }
  ```
- Why it is a bug: (a) A commented-out directive `# channel = "nightly"` overrides the real channel because the loop keeps iterating and later matches win. (b) Any line merely containing the word "channel" (e.g. `targets = ["x86_64-channel"] = ...`, doc comments) is misparsed. (c) `split('=')` on `channel = "sta=ble"` yields `'"sta'`. Result: wrong Rust toolchain activated from a valid TOML file.
- Fix: parse as TOML (`toml::from_str` with a small struct) like the mise path does, or at minimum skip lines whose trimmed form starts with `#` and require the key to be exactly `channel`.

### M5. Bash completion splits suggestions on whitespace
- File: `src/hooks/completions/bash.sh:13,30`
- Code: `COMPREPLY=($(compgen -W "$suggestions" -- "$cur"))`
- Why it is a bug: unquoted command substitution both word-splits and glob-expands. Suggestions containing spaces break, and suggestion text containing glob metacharacters (`*`, `?`) is expanded against the filesystem — output of `omg complete` is injected unquoted into the shell. Low practical impact for package names but a classic injection-shaped pattern.
- Fix: `while IFS= read -r s; do COMPREPLY+=("$s"); done < <(compgen ...)`, or populate `COMPREPLY` via a mapfile over newline-delimited suggestions.

### M6. `download_with_progress` has no size cap on the download itself
- File: `src/runtimes/common.rs:139-208`
- Why it is a bug: decompressed archives are bounded (`BudgetedSink` / `BudgetedReader`, 2 GiB cap) but the compressed stream is written to disk unbounded. A compromised/hostile mirror (or a vendor serving a multi-hundred-GiB object that still hashes correctly only in a DoS sense) can fill the disk. Checksum verification happens only after the whole body was persisted.
- Fix: enforce `Content-Length <= MAX_DECOMPRESSED_BYTES` and abort the stream once `downloaded` exceeds a bound.

### M7. `MiseManager::extract_tarball` runs blocking decompression + full-archive scan inside async context, with no decompression budget
- File: `src/runtimes/mise.rs:213-235` (called from async `install`, line ~100)
- Why it is a bug: unlike `extract_tar_gz`/`extract_tar_xz` in common.rs (which wrap work in `tokio::task::spawn_blocking` and budget xz output), this scans the entire gzip stream synchronously on the async executor, stalling the runtime, and has no `BudgetedSink` equivalent (gzip could be bounded with `GzDecoder` + `take`). Inconsistent with the module family's own hardening standard.
- Fix: wrap in `spawn_blocking`; bound the copy with `entry.take(MAX)`.

### M8. Fish hook runs `omg hook-env` twice per prompt
- File: `src/hooks/mod.rs:794-798` (FISH_HOOK)
- Code: `function _omg_hook --on-variable PWD --on-event fish_prompt`
- Why it is a bug: registering both handlers means every prompt triggers two external process spawns (`PWD` change fires, then `fish_prompt` fires), doubling the per-prompt cost the hook was designed to minimize.
- Fix: keep only `--on-event fish_prompt` (which already covers directory changes).

---

## LOW

### L1. `detect_versions` walks every ancestor up to `/`
- File: `src/hooks/mod.rs:337-358`
- Why: no stop boundary (e.g. `$HOME`, git root, filesystem root marker like mise/rustup do). On deep paths this performs ~15 `exists()` syscalls × depth on every prompt, and picks up version pins from system directories (`/usr`, `/etc` if any pin files exist there). Also errors from unreadable pins anywhere along the chain fail the whole hook.
- Fix: stop at `$HOME` or a sentinel (`.git`, device root), matching upstream tool behavior.

### L2. HashMap iteration order makes emitted PATH order nondeterministic
- File: `src/hooks/mod.rs:282-322` (`build_path_additions` iterates `versions: HashMap`)
- Why: with multiple runtimes pinned, the order of PATH additions differs between invocations, producing flaky shadowing semantics (which `node` wins depends on hash seed).
- Fix: iterate a sorted/BTreeMap collection.

### L3. `resolve_nvm_alias` does not follow chained aliases
- File: `src/hooks/mod.rs:594-603`
- Why: nvm aliases commonly chain (`default -> lts/hydrogen -> 18.19.0`). Only one hop is resolved; the raw target (e.g. `lts/hydrogen`) is then used as a version, the bin path won't exist, and the node entry silently drops out of PATH. Silent resolution failure, no diagnostic.
- Fix: loop-resolve until the target parses as a version or a hop limit is reached.

### L4. `normalize_version_req` mangles spaces-only requirements inconsistently
- File: `src/hooks/mod.rs:544-568`
- Why: `"18 || 20"` becomes `"18,||,20"` → parse error → `None` → falls back to NVM lookup instead of the native store, silently ignoring locally installed matching versions. Also `"*"` and `"x"` are handled but `"latest"` is not special-cased here (handled elsewhere), so engines values like `"latest"` degrade silently. Minor correctness gap, no crash.
- Fix: split on `||` explicitly and reject unknown tokens with a debug log rather than silent None.

### L5. `mise_tool_version` array handling picks the first entry
- File: `src/hooks/mod.rs:196-210`
- Why: mise semantics for `node = ["22", "20"]` treat the last entry as default; OMG picks the first, activating a different toolchain than mise itself would.
- Fix: take the last element.

### L6. `complete_staged_install` TOCTOU between existence check and rename
- File: `src/runtimes/common.rs:536-553`
- Code: checks `fs::symlink_metadata(version_dir).is_ok()` then renames.
- Why: a concurrent install can publish between check and rename; `rename` onto an existing empty dir succeeds silently (POSIX rename replaces directories), onto non-empty fails — so behavior is racy rather than deterministic "refuse". Narrow window, low impact given single-user data dir, but the documented "Fails if the final path appeared" guarantee isn't actually enforced atomically.
- Fix: use `renameat2(RENAME_NOREPLACE)` (via `nix`/libc) or accept-and-document the race.

### L7. `version_cmp` treats pre-release equal to release and ignores empty numeric segments
- File: `src/runtimes/common.rs:884-903`
- Why: `"1.0.0-beta"` sorts equal to `"1.0.0"` (documented), but in descending sorts used by `list_installed_versions`/bun/python listing the relative order of such pairs is unstable (`sort_unstable_by`), so listings can flip-flop. `"1..2"` also compares as `[1,2]`.
- Fix: tie-break equal numeric prefixes by comparing the raw suffixes lexicographically.

### L8. Java `use_version` doesn't normalize a leading `v` while all sibling managers do
- File: `src/runtimes/java.rs:150-163` vs. `node.rs:203`, `go.rs:117`, etc.
- Why: `omg use java@v17` installs/validates fine at install time (validator permits it? install validates raw too) but `use_version` joins `v17` directly, whereas node/go/bun/python strip the prefix — inconsistent UX; `use java v17` after installing `17` reports JAVA_HOME at a nonexistent path (activate_version fails closed, at least).
- Fix: `normalize_version(version)` for consistency.

### L9. Ruby platform hardcoded to `ubuntu-22.04`
- File: `src/runtimes/ruby.rs:190-198`
- Why: ruby-builder publishes per-distro assets (ubuntu-20.04/22.04/24.04); on glibc-mismatched systems (e.g. very new distros are fine, older ones are not) the pinned asset may fail with loader errors. Also macOS naming: ruby-builder darwin assets are typically `...-darwin-arm64`/`x86_64` — this matches, but only by coincidence of naming; no musl support at all. Silent runtime failure mode.
- Fix: select newest available ubuntu-* asset for the version instead of a constant, and document musl unsupported.

### L10. Python `list_available` fetches only 10 most recent releases
- File: `src/runtimes/rust/../python.rs:71` (`?per_page=10`)
- Why: python-build-standalone tags releases frequently (multiple per version); 10 releases can cover fewer than a handful of CPython minor versions, so `omg list python --available` misses many installable versions and `install 3.9.x` spuriously reports "not found" though assets exist.
- Fix: paginate or raise per_page and filter client-side (like bun/ruby use 20).

### L11. Bun `fetch_checksum` sends User-Agent header but `list_available`/`get_latest_version` rely on client default; GitHub API rate limits differ
- File: `src/runtimes/bun.rs:64` vs `121-127`, `mise.rs:171-186`
- Why: inconsistent headers; requests without a descriptive UA get degraded GitHub API treatment. Cosmetic/reliability.
- Fix: set UA uniformly (the shared `download_client()` presumably does; then the explicit headers are redundant noise — pick one).

### L12. `rust.rs list_available` returns both the concrete stable version and the literal `"stable"` alias
- File: `src/runtimes/rust.rs:92-118`
- Why: duplicates in user-facing listing; picking the first row vs the alias row changes whether a dated toolchain or moving alias gets installed. Confusing UX, not a crash.
- Fix: return the concrete version plus channels distinctly labelled.

### L13. `extract_component_entries` filters on substring `"/bin/"` etc. before stripping
- File: `src/runtimes/rust.rs:277-310`
- Why: entries whose path is exactly `component/bin` root-level dirs, or archives laid out without the double prefix, are silently skipped → toolchain publishes "successfully" (marker written) with missing binaries. Failure is silent because nothing verifies `bin/cargo` exists before `complete_staged_install` (only `activate_version` checks `bin/rustc` later, and only for rustc).
- Fix: validate required profile components exist in staging before publishing.

### L14. Dead code: `print_completion_instructions`
- File: `src/hooks/completions.rs:173-197`
- Why: no callers anywhere in the tree (grep confirms only the definition). It also advertises `--stdout` eval usage that differs from the installed-file flow.
- Fix: delete it (per repo baseline: remove obsolete paths).

### L15. Zsh completion appends to `suggestions` without resetting it, and reads possibly-unset array
- File: `src/hooks/completions/zsh.zsh:14-37`
- Why: `_omg_dynamic_complete` does `suggestions+=(...)`. If the case branch finds no match and falls through to the fallback branch, results accumulate (duplicates). With `setopt nounset` users, `${#suggestions[@]}` on a never-set array errors under some zsh versions. Minor.
- Fix: `suggestions=()` at the top of `_omg` and guard with `${#suggestions[@]:-0}`.

### L16. Hard link targets in archives are not re-validated against escaping after joining
- File: `src/runtimes/common.rs:355-373`
- Why: hard-link `target` goes through `stripped_archive_path` (rejects ParentDir/RootDir) then `dest_dir.join(target)` — safe today, but unlike symlinks there is no explicit containment assertion tying it to `dest_dir`, and the deferred creation happens after all files exist, meaning a hard link can point at any regular file extracted earlier *outside* its own subtree within dest — acceptable, but note `create_archive_links` creates hard links without checking the source exists (fails with confusing error if the target entry was stripped away/skipped).
- Fix: pre-check `target.exists()` and emit a clear error naming both paths.

### L17. `hook_env` prints nothing when versions are detected but nothing is installed/resolves
- File: `src/hooks/mod.rs:180-232`
- Why: a project pins `node@20` with nothing installed and mise absent → PATH untouched, user silently gets whatever global toolchain. No hint to run `omg install`. UX defect (silent fallback), deliberate but undocumented in output.
- Fix: emit a comment line (shell-safe) or cache a flag for the prompt to display "runtime X pinned but missing".

### L18. `Settings::load()?` failure breaks every prompt
- File: `src/hooks/mod.rs:187`
- Why: a corrupt settings file turns each shell prompt into a failed `eval` (empty output) plus stderr noise; the hook should degrade gracefully (skip PATH management) rather than erroring per-prompt.
- Fix: log-once / ignore settings errors in the hot path.

---

## INFO

### I1. `normalize_runtime_name` maps `"python3"`→python, but not `"nodejs"` variants used by some `.tool-versions` files (`nodejs 20` appears in the wild); unmapped names pass through and are silently ignored downstream (`build_path_additions` `_ => continue`). Consider logging unrecognized runtimes once. (`src/hooks/mod.rs:63-77`, `282+`)

### I2. `read_pin_file` returns raw content for ANY existing file including directories-as-pins; `fs::read_to_string` on a directory errors with a context error that fails the whole hook — a stray `mkdir .python-version` in any parent directory breaks PATH switching for every subdirectory beneath it. Fail-closed is defensible; consider treating IsADirectory as None. (`src/hooks/mod.rs:87-96`)

### I3. `package.json` parse failure aborts detection for the whole tree (`try_parse_version_file` propagates). A minified-but-valid JSON is fine, but JSONC-style package.json (comments, used by some tools) permanently disables the hook in that repo. (`src/hooks/mod.rs:97-135, 305-308`)

### I4. `add_mise_path_fallbacks` adds `mise_runtime_bin_path` results to PATH without checking the directory exists (no `is_valid_version_dir` gate like the native branch) — nonexistent dirs land on PATH when backend is Mise/NativeThenMise. Harmless-ish (PATH pollution) but inconsistent with the validated native path. (`src/hooks/mod.rs:616-640`)

### I5. `fish_add_path -g` prepends each addition individually, reversing multi-runtime order relative to the zsh/bash branch which prepends them as one group. Ordering inconsistency across shells. (`src/hooks/mod.rs:216-222`)

### I6. `parse_sha256_digest` accepts mixed case and lowercases — good; but callers compare with `eq_ignore_ascii_case` in download_with_progress after lowercase-normalizing expected — consistent. Note checksum sources are fetched over the same TLS channel as the artifact (TOFU-style, no pinning): supply-chain strength equals CA trust. Applies to node SHASUMS256.txt, go .sha256, Adoptium, GitHub digests. (`src/runtimes/common.rs:905-920`, node.rs:177-192)

### I7. `mise current` output parsing assumes `"<runtime> <version>"` or `<runtime>@<version>`; mise's actual default format varies by version/aliases — third positional fallback `Ok(Some(line.to_owned()))` returns the whole line as "version" if formats drift, feeding garbage into later path construction (which validators will reject, failing closed). Fragile parsing, safe failure. (`src/runtimes/mise.rs:252-296`)

### I8. `is_available` caches the system-mise probe in a process-global `OnceLock`; a system mise installed *after* the first probe is invisible for the process lifetime. Fine for short CLI runs, surprising for long-lived daemon use. (`src/runtimes/mise.rs:66-80`)

### I9. Completion command lists are maintained in three places (bash.sh inline string, zsh.zsh array, fish.fish individual lines) and have already drifted (bash includes `init`/`up`/`d`; zsh includes all aliases; fish lists aliases separately). Drift guarantees future omissions. Generate from clap like the powershell/elvish branches do. (`src/hooks/completions/*`)

### I10. `RustToolchainSpec::parse` date detection requires exactly 3 trailing segments of 2/2/4 digits; `nightly-2024-1-15` (non-padded) silently becomes host triple `2024-1-15` → toolchain name contains it → later manifest fetch 404 with confusing message. Acceptable but the error won't mention date parsing. (`src/runtimes/rust.rs:528-565`)

### I11. BudgetedSink::write returns `Err` instead of performing a partial write when `len > remaining`; compliant with Write's contract but means callers see the offending chunk rejected whole — fine for the bomb-abort purpose; noted for future reuse. (`src/runtimes/common.rs:104-122`)

### I12. `java install` does not normalize version nor handle `"v17"`; validator likely rejects it, giving an early clear error — consistent enough, noted alongside L8. (`src/runtimes/java.rs:93-95`)

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 0 |
| MEDIUM | 8 |
| LOW | 18 |
| INFO | 12 |

Strongest themes: shell-hook robustness (EPOCHSECONDS/cache, /tmp spoofing, double-firing fish hook), one validation gap in the NVM alias path, unquoted-completion injection shape in bash completions, and consistency gaps among the seven runtime managers. Extraction/staging/install-marker machinery in `runtimes/common.rs` is notably well-hardened (budgeted decompression, deferred links, staged atomic publishes, symlink rejection).


---

# SLICE 12

# Audit slice-12: `src/package_managers` (ALPM/pacman/AUR core)

Agent: audit12 · Scope: `alpm_direct.rs`, `alpm_ops.rs`, `alpm_worker.rs`, `arch.rs`, `aur_deps.rs`, `aur_index.rs`, `aur_metadata.rs`, `aur_sources.rs`, `pacman_db/{db,mod}.rs`, `pkgbuild.rs`, `parallel_sync.rs`. Read-only review.

## HIGH

### H-1. Path traversal via AUR `.SRCINFO` source filenames (arbitrary file write)
**File:** `~/Documents/omg/src/package_managers/aur_sources.rs`, lines ~100–125 (`extract_http_source`) and ~160 (`download_sources`: `let dest_path = srcdest.join(&source.filename);`)
```rust
let filename = custom_filename.unwrap_or_else(|| {
    url.rsplit('/').next().unwrap_or("unknown")...
});
...
let dest_path = srcdest.join(&source.filename);
```
**Why a bug:** `.SRCINFO` for an AUR package is untrusted attacker-controlled data (anyone can publish an AUR package). Neither branch sanitizes the resulting filename:
- Rename syntax: `"../../home/user/.bashrc::https://evil/x"` → `filename = "../../home/user/.bashrc"`, and `srcdest.join(...)` escapes SRCDEST → arbitrary file write at the invoking user's privileges.
- URL-derived names: `https://evil/x/..` yields last segment literally `..`; also a bare `..` or absolute-ish segments pass through.
The pre-download then persists the file at that path before makepkg's checksum step ever runs.
**Fix:** reject filenames containing `/`, `\`, `..`, empty, or not matching `[A-Za-z0-9._+@-]+` (and reject pure `.`/`..`); fall back to skipping the source with a warning.

### H-2. Sync databases downloaded and installed with zero signature/integrity verification
**File:** `~/Documents/omg/src/package_managers/parallel_sync.rs`, `download_db` / `download_response_to_dest` (lines ~180–280) writing directly into `/var/lib/pacman/sync/`.
**Why a bug:** pacman verifies `.db.sig` against the keyring when present; omg fetches `<mirror>/<repo>.db` over whatever protocol the mirrorlist specifies (including plain HTTP), never requests/checks `.db.sig`, and atomically persists the response as the authoritative repo database. A compromised/malicious mirror or MITM can feed a forged DB (fake versions, malicious replaces/provides, dependency redirection). Package-file signatures mitigate outright binary forgery but not metadata-level attacks (downgrades, conflict manipulation, steering installs to attacker mirrors).
**Fix:** download `<repo>.db.sig` alongside and verify with the pacman keyring before persisting (or shell out to `pacman-key`/use sequoia), and prefer https mirrors.

### H-3. Truncated repo DB accepted — no size/content-length validation on DB downloads
**File:** `parallel_sync.rs`, `download_response_to_dest` (lines ~168–178).
```rust
while let Some(chunk) = response.chunk().await? { file.write_all(&chunk).await...; }
persist_same_dir_temp(file, temporary_path, dest).await
```
**Why a bug:** unlike `aur_sources::download_to_file` which validates `downloaded != expected_length`, the DB downloader never compares bytes written to `Content-Length`. A connection cut mid-body yields a truncated tar that is persisted as `core.db` with current mtime. The pure-Rust parser (`pacman_db::parse_sync_db`) will happily parse all complete entries, so the cache becomes silently incomplete (missing packages/updates) and is considered fresh because mtime matches.
**Fix:** capture `content_length()` and fail persistence if byte count mismatches (mirroring `aur_sources`).

## MEDIUM

### M-1. ALPM worker handle never invalidated after sync — permanently stale updates
**File:** `~/Documents/omg/src/package_managers/alpm_worker.rs`, lines ~33–80.
**Why a bug:** the worker thread builds its `Alpm` handle once at startup and loops forever. Unlike `alpm_direct` (epoch-based `clear_alpm_cache()` called by `sync_databases_parallel` after every sync), the worker never re-reads the sync DBs, so a long-running daemon answers `list_updates` from pre-sync data indefinitely.
**Fix:** check `CACHE_EPOCH` per request (reuse `alpm_direct::with_handle*`) or expose invalidation to the worker.

### M-2. Auto-answer callbacks remove conflicting packages and replace packages without consent
**File:** `~/Documents/omg/src/package_managers/alpm_ops.rs`, `setup_alpm_callbacks`, lines ~436–444.
```rust
alpm::Question::Replace(q) => q.set_replace(true),
alpm::Question::Conflict(mut q) => q.set_remove(true),
alpm::Question::SelectProvider(mut q) => q.set_index(0),
```
**Why a bug:** pacman interactively asks the user before replacing/removing conflicting packages and before choosing among providers. omg silently answers "yes" — a transaction like `omg install foo` can uninstall unrelated user packages or pick an arbitrary provider with no confirmation. This is a destructive default on a privileged operation.
**Fix:** surface these questions (prompt via TUI or default-deny with an explicit error listing the conflict).

### M-3. Removing a non-installed package silently reports success
**File:** `alpm_ops.rs`, `prepare_alpm_transaction`, lines ~556–562.
```rust
if let Ok(pkg) = tx_guard.0.localdb().pkg(pkg_name.as_str()) {
    tx_guard.0.trans_remove_pkg(pkg)...;
}
```
**Why a bug:** if the package isn't installed, it's skipped without any record; the commit path then hits the "Nothing to do - system is up to date" success message. Users removing a typo'd or already-removed package get a green ✓ instead of an error (and history records a removal that never happened).
**Fix:** bail with "package X is not installed" when `localdb().pkg` returns `PkgNotFound`.

### M-4. One stray directory in `/var/lib/pacman/local` breaks the whole local DB cache
**File:** `pacman_db/db.rs`, `parse_local_db`, lines ~430–437.
```rust
let desc_path = pkg_path.join("desc");
if !desc_path.exists() {
    anyhow::bail!("Local package directory {} is missing desc", ...);
}
```
**Why a bug:** any non-package directory (interrupted alpm transaction leftovers, admin scratch dirs, editor artifacts) aborts parsing of the entire local DB. Every dependent feature (`check_updates_cached`, counts, explicit list, orphan detection, AUR detection) then hard-fails until the stray dir is removed.
**Fix:** log a warning and skip directories lacking `desc` instead of failing the whole parse (a missing desc in an otherwise valid pkg dir could remain fatal only for that entry).

### M-5. AUR dependency satisfaction ignores version constraints
**File:** `~/Documents/omg/src/package_managers/aur_deps.rs`, lines ~110–122.
```rust
let pkg_name = extract_package_name(&dep);
if localdb.pkg(pkg_name).is_ok() { satisfied.push(dep); } else { missing.push(dep); }
```
**Why a bug:** `foo>=2` is reported *satisfied* when `foo 1.0` is installed. The build-time makepkg check catches it late, after omg has already told the user all deps are present (misleading UX, wasted partial build). Same for `<`, `=`.
**Fix:** use libalpm's dependency satisfier (`alpm::find_satisfier` / `Depend::satisfied_by`) instead of name-only lookup.

### M-6. PKGBUILD array/comment parsing corrupts values containing `#` or early `)`
**File:** `~/Documents/omg/src/package_managers/pkgbuild.rs`, `parse_content` (~lines 90–120) and `parse_array` (~lines 155–190).
**Why a bug:**
- `line.split('#').next()` strips everything after `#` even inside quotes: `pkgdesc="C# tools"` → `C`, and source URLs containing fragments are truncated.
- Multi-line detection `val.starts_with('(') && !val.ends_with(')')` misfires when the opening line ends with a comment (`depends=( # deps`), and the continuation terminator `next_line.contains(')')` stops at the first `)` anywhere — including inside quoted elements — truncating arrays or swallowing subsequent assignments into them.
- `trim_matches('"')`/`trim_matches('\'')` strip *all* leading/trailing quote characters, mangling values like `""foo""`.
Metadata shown to users (deps, checksums pairing with sources) can be wrong; sha256sums and sources lists can desynchronize by index.
**Fix:** implement quote-aware tokenization (respect single/double quotes and inline-comment rules of bash) rather than naive line splitting.

### M-7. `get_pkg_info_from_db` reports installed size as download size
**File:** `alpm_ops.rs`, lines ~150–153.
```rust
size: pkg.isize() as u64,
install_size: Some(pkg.isize()),
download_size: Some(pkg.size() as u64),
```
**Why a bug:** in libalpm, `pkg.size()` is the *installed* size and `pkg.download_size()` is the compressed download size. This function sets `download_size` to `pkg.size()` (installed size), while the sibling implementation in `alpm_direct::get_package_info` correctly uses `pkg.download_size()`. `display_pkg_info` prints this field under "Download:", showing wrong numbers for sync packages fetched through `get_sync_pkg_info`'s libalpm path.
**Fix:** use `pkg.download_size()` here too.

### M-8. Local-db TTL eviction is defeated by the disk cache
**File:** `pacman_db/db.rs`, `ensure_local_cache_loaded` / `ensure_sync_cache_loaded` (lines ~640–760) + `is_cache_expired`.
**Why a bug:** the 30-minute TTL clears the in-memory cache, but the code immediately loads the disk-persisted cache, whose `last_accessed` is `#[serde(skip)]` → always `None` → `is_cache_expired(None) == false` → reusable whenever mtimes match. So the TTL "safety net" never forces a true reparse while directory mtimes are unchanged; combined with the fact that edits *inside* an existing local pkg dir don't change the parent dir mtime, stale data can persist for the life of the disk cache file.
**Fix:** persist `last_accessed` (or store a write timestamp) in the disk format and honor TTL there too.

## LOW

### L-1. Orphan removal TOCTOU between listing and privileged removal
**File:** `~/Documents/omg/src/package_managers/arch.rs`, `remove_orphans` (lines ~200–230): orphans are listed unprivileged, printed, then the list is passed verbatim to the root child. Concurrent installs/removals (another terminal, another omg instance) can change the orphan set; a since-promoted dependency gets removed. Fix: re-validate orphan status inside the elevated transaction (e.g. rely on `TransFlag::UNNEEDED | RECURSE` semantics rather than an enumerated snapshot).

### L-2. `clean_cache` counts `.sig` files as removed packages and misattributes failures
**File:** `alpm_ops.rs`, `clean_cache` (lines ~250–310). `removed += 1` for signature files inflates "N packages removed"; freed-bytes credit uses `unwrap_or(0)` on metadata failure (undercount); failure warning prints `old.display()` (the .pkg path) even when the failure was the `.sig`. Cosmetic/reporting accuracy.

### L-3. Dead fallback branch in `download_db` mirror reordering
**File:** `parallel_sync.rs` (~lines 195–210): `race_mirrors` returns `Some(usize)` unconditionally (falls back to `Some(0)`), so `if let Some(...) = ... { } else { urls }` else-arm is dead code. Also `race_mirrors` treats a 2 s timeout across all mirrors as "use mirror 0" — fine, but the dead branch suggests a `None` case that cannot occur. Simplify.

### L-4. Hardcoded standard-repo lists duplicated and divergent
**Files:** `parallel_sync.rs` `standard_repos` (6 entries incl. testing repos) vs `MIRRORLIST_REPOS` (same file, 6 entries, "keep in sync" comment) vs `pacman_db/db.rs::collect_sync_db_paths` hardcoded `["core","extra","multilib"]` (only 3). Today behavior is consistent by luck (testing repos flow through the read_dir arm), but adding a new official repo requires editing three places. Extract one shared constant/module.

### L-5. Unknown-compression sync DB falls back to gzip decoder producing confusing errors
**File:** `pacman_db/db.rs`, `parse_sync_db` (~lines 300–330): unknown magic bytes → `GzDecoder` → "invalid gzip header" instead of "unsupported compression format". Also zstd decoding buffers the entire decompressed DB in RAM (`Vec`) rather than streaming. Improve error message; streaming decode optional.

### L-6. `is_keyring_related_error` keyword "corrupt" over-matches
**File:** `alpm_ops.rs` (~line 700): any trans_prepare error containing "corrupt" (e.g. corrupted *database*) produces keyring-repair advice (`pacman-key --init` etc.), sending users down the wrong remediation path. Narrow the match to PGP/signature phrasing.

### L-7. Download progress bars beyond `MAX_CONCURRENT_DOWNLOAD_BARS` are invisible and never cleaned up
**File:** `alpm_ops.rs`, `set_dl_cb` Init arm (~lines 500–520): once 4 bars exist, further downloads create no bar, so their `Progress` events target nothing; error paths in `aur_sources::download_file` also leave bars unfinished (spinner lingers). Cosmetic.

### L-8. Mirrorlist fallback applies all servers to every repo including custom ones
**File:** `alpm_ops.rs`, `configure_mirrors` fallback path (~lines 760–790): when `PacmanConfig::parse` fails, the raw mirrorlist servers are added to *all* syncdbs, including custom repos whose real servers live only in pacman.conf (which just failed to parse anyway — arguably fine, but the `$repo` substitution will inject e.g. `chaotic-aur` into Arch mirror URLs, generating junk servers). Minor noise; consider bailing instead.

### L-9. `list_explicit_fast` mixes data sources
**File:** `alpm_direct.rs` (~lines 245–262): prefers the pure-Rust `pacman_db::list_local_cached()` (mtime/TTL-cached) while every other `_fast` fn uses the live ALPM handle. After an install, if the pacman_db disk cache mtime check passes stale data, explicit list can disagree with `is_installed_fast`/counts within the same command run. Pick one source of truth for consistency-sensitive callers.

### L-10. `sync_aur_metadata` NOT_MODIFIED with missing cache file silently succeeds
**File:** `aur_metadata.rs` (~lines 130–165): server says 304 but local archive is absent (deleted between runs) → touch/rebuild both guarded by `cache_path.exists()` → returns `Ok(())` with no data and no index; subsequent searches silently see no AUR index depending on caller handling. Should treat 304-without-file as a cache miss and do a full GET.

### L-11. Failed AUR meta sidecar persist causes full redownload next run
**File:** `aur_metadata.rs` (~lines 170–195): if the small `.meta` sidecar fails to persist (warned, non-fatal) the next sync sends no validators → full multi-MB redownload. Consider deriving validators from the archive file itself or making sidecar failure louder.

### L-12. `prepare_alpm_transaction` silently drops `packages` when `sysupgrade` is set
**File:** `alpm_ops.rs` (~lines 545–550): `if sysupgrade { ... } else { for pkg_name in packages {...} }`. Current callers pass an empty vec, but the API allows `execute_transaction(vec!["foo"], false, true, None)` which would perform a full upgrade and ignore "foo". Make the states mutually exclusive (enum) or assert emptiness.

### L-13. HoldPkg is enforced for removal but not for upgrades
**File:** `alpm_ops.rs` (~lines 556–562): explicit HoldPkg bail exists only in the `remove` arm; `sync_sysupgrade` relies solely on libalpm `IgnorePkg`. pacman warns on HoldPkg upgrades; omg doesn't. Low impact (IgnorePkg usually covers it).

## INFO

### I-1. `catch_unwind(AssertUnwindSafe)` depends on non-abort panic strategy
**File:** `pacman_db/db.rs`, `parse_desc_content`: if a release profile ever enables `panic = "abort"`, corrupted desc files abort the process instead of falling back. Worth a comment/build assertion.

### I-2. Query-path syncdbs registered with `SigLevel::USE_DEFAULT` without setting a default siglevel
**File:** `alpm_direct.rs`, `create_alpm_handle`: harmless today because query paths don't verify signatures, but differs from the transaction path which explicitly configures siglevels. Note-only.

### I-3. `AurIndex::open` mmap soundness relies on writer discipline
**File:** `aur_index.rs` (~lines 60–78): the SAFETY comment is thorough and the temp-file+persist discipline appears honored everywhere in-tree; flagging so future in-place writers know they break the invariant. Also `NamedTempFile::new_in(parent)` in `build_index` doesn't set restrictive permissions (index is public metadata; low concern).

### I-4. `extract_package_name` misses `:` description separator used in optdepends-style strings
**File:** `aur_deps.rs`: currently only fed depends/makedeps/checkdeps so unaffected, but fragile if optdepends are ever included (`foo: bar support` would be looked up verbatim).

### I-5. `search_sync` has no result cap
**File:** `alpm_direct.rs`: a one-character query scans all syncdbs and returns tens of thousands of rows with per-row `localdb().pkg()` probes. Correct but potentially slow/memory heavy for UI completion; consider a limit parameter upstream.

### I-6. Duplicate architecture helper
**Files:** `aur_deps.rs::current_arch` and `aur_sources.rs::current_arch` are copy-pasted identical functions. Consolidate into a shared module.

### I-7. `substitute` in pkgbuild.rs expands variables inside previously substituted values
**File:** `pkgbuild.rs` (~lines 120–135): sequential replacement means a value containing `$<othervar>` text (literal `$` in checksums/comments) can be secondarily expanded. Naive-but-bounded; note-only given M-6 rewrite suggestion.

## Verified-good (no finding)
- `parse_sync_db` magic-probe rewind regression covered by tests (gzip+zstd).
- Atomic persists (tempfile + `persist`) used consistently for caches, index, DBs, AUR metadata.
- `O_NOFOLLOW` safe read for PKGBUILD; PGP key import refused inside ALPM callback (good security posture).
- `collect_updates` honors IgnorePkg/should_ignore with first-repo-wins priority consistent with registration order.
- Double-checked locking around both caches correctly revalidates after slow parse.
- `updates_for` missing-package fallback (regression v0.1.215) well-tested.

**Totals:** 3 HIGH, 8 MEDIUM, 13 LOW, 7 INFO = **31 findings**.


---

# SLICE 13

# Audit slice-13: package_managers/{apt.rs, debian_pure.rs, debian_db/, dnf.rs, homebrew.rs}

Read-only source audit. Line numbers refer to files as of audit time.

## CRITICAL

### C1. Packages indices are never verified against the signed InRelease (trust-chain gap) — `debian_db/parallel_sync.rs`
- Lines ~300–395 (`sync_repository`), component download loop ~330–365.
- `verify_inrelease_signature` authenticates only the InRelease document. The downloaded `Packages.gz`/`Packages.xz` indices are never checked against the checksums (`SHA256`/`MD5Sum`) declared in that signed Release file, nor is by-hash used. The comment claims "Component indexes and their package hashes are only trusted when anchored to this verified document", but no code performs that binding.
- Impact: a MITM or hostile mirror can substitute a modified Packages index. Downstream, `populate_action_url` (debian_pure.rs) trusts the index's `Filename` + `SHA256`, and `transaction.rs::download_package_streaming` verifies the .deb against *that attacker-controlled hash*, then executes maintainer scripts from it as root. The entire signature gate is bypassable.
- Fix: parse the InRelease `SHA256:`/`MD5Sum:` section and verify each downloaded Packages artifact's digest before writing it to cache; prefer `by-hash` locations.

### C2. Pure-Rust install path executes maintainer scripts / writes dpkg DB without root check or privilege escalation — `debian_pure.rs` vs `apt.rs`
- `debian_pure.rs::install/remove/update/sync` (lines ~70–260) perform extraction to `/`, writes to `/var/lib/dpkg/status`, and run maintainer scripts with no `is_root()` check and no `run_privileged_child` escalation — unlike `apt.rs`/`dnf.rs` which escalate.
- Impact when run unprivileged: transaction partially mutates the system (whatever the user can write), then fails mid-way and triggers rollback; also confusing failures. When combined with C1, scripts run as root from potentially attacker-influenced packages.
- Fix: add the same `is_root()`/`run_privileged_child` gate as the other backends before any transaction.

### C3. Pure `sync` downloads repository data nothing ever reads (functional disconnect) — `debian_db/parallel_sync.rs` + `debian_db/db.rs`
- `sync_all_repositories` writes decompressed indices to `<cache_dir>/apt/<repo>/{component}_{arch}_Packages` (parallel_sync.rs ~345–360). Grep confirms **no code anywhere reads those files**: `db.rs::ensure_index_loaded` / `search_fast` / `get_info_fast` exclusively parse `/var/lib/apt/lists/*_Packages`.
- Impact: `omg sync` on the pure backend reports success ("faster than apt update") but never refreshes the database actually used for search/info/install; users get stale results after adding repos or on fresh systems without apt lists.
- Fix: either point the index parser at omg's synced cache, or have sync write into `/var/lib/apt/lists`-compatible layout consumed by `ensure_index_loaded`.

## HIGH

### H1. dnf transactions append `-y` after the `--` separator — `dnf.rs::run_dnf` (~line 252)
```rust
let status = cmd.args(args).arg("-y").status()?;
```
Callers build `["install", "--", pkg…]`; the final argv is `dnf install -- pkg… -y`. Everything after `--` is a positional (package spec), so dnf tries to operate on a package literally named `-y` → transaction fails (or mis-parses).
Fix: insert `-y` before the separator: `["-y"] ++ args`.

### H2. Wrong InstallReason decoding for the SQLite RPM path — `dnf.rs::parse_package_from_blob` (~line 320)
```rust
let reason = if reason_val == 0 { InstallReason::User } else { InstallReason::Dependency };
```
librpm/dnf reason enum is `0=UNKNOWN, 1=DEPENDENCY, 2=USER, 3=WEAKDEP`. This maps USER(2)→Dependency, so `list_explicit` and `get_status.explicit` are wrong for every user-installed package on Fedora 33+ (the primary SQLite path). The subprocess fallback (`parse_rpm_qa_line`) uses different semantics again ("0"/"user").
Fix: map `2 => User`, everything else Dependency; align the `-qa` parser.

### H3. Fresh-cache early-return leaves search buffer & installed set empty — `debian_db/db.rs::ensure_index_loaded` (~lines 640–660)
```rust
if has_changed_files && cache_is_fresh && !index.packages.is_empty() {
    let mut cache = DEBIAN_INDEX_CACHE.write()...;
    cache.index = Some(index);
    cache.file_mtimes = current_files;
    cache.last_accessed = unix_now_secs();
    return Ok(());
}
```
This path stores the index but never populates `cache.search_buffer`, `package_offsets`, or `cache.installed_set`. Any later SIMD-fallback search sees an empty buffer → silently zero results; installed flags computed against an empty set → all "not installed".
Fix: populate the search buffer and installed set on this early-return too (or fall through to shared population code).

### H4. FST/mmap fast path drops mixed-case names — `debian_db/db.rs::fst_mmap_search` (~lines 1495–1540)
FST keys are lowercased names (`lower_name_to_idx`), but lookup does `mmap.get(query_lower)` / `mmap.get(name_str)` where mmap keys preserve original case (`name_to_idx`). For any package whose stored name contains uppercase (third-party repos, e.g. `MySQL`, `NVIDIA`), exact match misses and prefix matches yield nothing → the ultra-fast path returns empty where the in-memory `fst_search` would find the package.
Fix: store a lowercase→canonical-name map in the archive, or index mmap by lowercase keys.

### H5. Installed-package cache keyed by name only; multi-arch RPMs collide and never invalidate — `dnf.rs`
- `installed_cache: DashMap<String, InstalledPackage>` inserts by `pkg.name` (load_installed_packages ~line 105): `glibc.x86_64` and `glibc.i686` overwrite each other → undercounted inventory and ambiguous `info`.
- Cache has no mtime/stat validation; packages installed by external `dnf`/`rpm` remain invisible until an operation on the same manager instance clears it. `is_installed` answers from the stale cache first (line ~575).
Fix: key by name+arch (or store Vec per name); validate against rpmdb mtime like the Debian backend does.

### H6. Version selection in Cellar is lexicographic — `homebrew.rs::read_package_info` (~line 400)
```rust
versions.sort_by(|a, b| a.0.cmp(&b.0));
let Some((version, version_path)) = versions.last() ...
```
`"1.9" > "1.10"` lexicographically, so with two kegs installed the older one is reported as current, used for updates comparison, receipts, etc.
Fix: sort with `parse_version_or_zero`/semver-style comparison.

### H7. Homebrew on Linux prefix unsupported — `homebrew.rs::detect_prefix` (~line 150)
Only `/opt/homebrew` (ARM mac) and `/usr/local` (Intel mac) are probed; Linuxbrew's default `/home/linuxbrew/.linuxbrew` is missing, so the whole backend silently reports zero installed packages and points `run_brew` at a nonexistent brew binary on Linux hosts (where this Rust binary actually runs).
Fix: probe `/home/linuxbrew/.linuxbrew` and honor `$HOMEBREW_PREFIX`.

## MEDIUM

### M1. Topological-sort edges drop dependency alternatives — `debian_db/resolver.rs::record_dependency_edges` (~line 300)
Edges use `parse_dep_name(dep)` which truncates at `|`, so `"x | base"` records only `x`. If resolution satisfied the dep via alternative `base` (resolve_dependency tries alternatives), the edge `base → dependent` is missing and Kahn's algorithm can order the dependent before its real dependency → broken first-boot behavior of installed packages.
Fix: record the edge for whichever candidate `resolve_dependency` actually selected.

### M2. Circular dependencies abort installs — `resolver.rs::topological_sort` (~line 380)
Real Debian packages routinely have circular deps (dpkg breaks them at configure time). The pure backend hard-fails with "Circular dependency detected" instead of ordering them unpack-all/configure-all like apt.
Fix: emit remaining cycle members in arbitrary order (unpack phase tolerates it) instead of erroring, matching the unpack/configure split already present in transaction.rs.

### M3. `data.tar` extraction overwrites existing config files unconditionally — `transaction.rs::extract_tar_to_root_at` (~line 900)
Unlike dpkg, no conffile protection exists during unpack: user-modified `/etc` files are clobbered before `configure_packages` even reads `conffiles`. Rollback explicitly does not restore overwritten contents (documented), so a failed upgrade permanently destroys local configuration.
Fix: detect existing conffiles and skip/preserve them (dpkg semantics) before writing regular files.

### M4. `configure_packages` runs postinst before registering status — `transaction.rs::configure_packages` (~line 545)
postinst runs, then `record_dpkg_status_entries` writes status afterwards. Maintainer scripts commonly invoke `dpkg-query` on their own package; it will report not-installed/unpacked. Also if status write fails after postinst succeeded, rollback removes files but the script's system effects persist.
Fix: write status entries (state "unpack ok half-configured") before running postinst, mirroring dpkg.

### M5. Repo-cache directory name collisions — `parallel_sync.rs::repo_cache_dir` (~line 585)
`format!("{}_{}", uri, suite).replace(['/',':','.'],"_").replace("__","_")` maps distinct URIs to the same dir (e.g. `http://h/a_b` vs `http://h/a/b`, or suites differing only by dots). One repo's cached InRelease/.synced can then be served for another (stale/wrong trust anchor, wrong components).
Fix: use a hash (or percent-encoding) of uri+suite instead of lossy character replacement.

### M6. Duplicate entries in update listings — `db.rs::get_updates_from_mmap` (~line 1980) and `compute_updates` fallback
Both iterate **all** rows of `packages` (the index keeps every arch/repo variant via `add_package` push), filtering only by name membership in `installed_map`. A package present in main+contrib or arch+all yields multiple identical `UpdateInfo` entries; UI shows duplicated upgrades. (The FST/name maps dedup, but the vec iteration doesn't.)
Fix: dedup by name (e.g. collect into a set) or iterate `name_to_idx` values.

### M7. `parse_rpm_qa_line` field splitting breaks on tabs in summary — `dnf.rs` (~line 175)
`%{SUMMARY}` may contain `\t`; `fields.len() < 5` then errors out and `read_rpm_via_query` fails wholesale (one odd package kills the whole inventory). Also `fields[4]` mis-indexed when summary contains tabs.
Fix: `splitn(5, '\t')` and take the last field as reason.

### M8. Missing input validation parity in pure backend — `debian_pure.rs::install/remove`
Neither calls `crate::core::security::validate_package_names/_name`, unlike `apt.rs` and `dnf.rs`. Package strings flow into resolver lookups and later into filesystem paths (`temp_dir.join(&action.name)` in `download_package_streaming`, `extract_dir.join(&action.name)` in `configure_packages`). A crafted name like `../../etc` becomes a path component. Defense currently relies on resolver rejecting unknown names, but locally-present hostile names (e.g. from IPC/daemon callers) reach path joins.
Fix: validate at the backend entry points like the other managers do.

### M9. `update_dpkg_status_for_removal` mis-parses paragraphs without trailing structure — `transaction.rs` (~line 1560)
Paragraph state resets only on `line.is_empty()`; a status file using `\r\n` (or any whitespace-only line) never terminates the paragraph, so `found_package` logic and Status rewriting can target the wrong paragraph or bail "Package not found". Also non-target blank lines are normalized, subtly reformatting the DB.
Fix: split on `"\n\n"` like the other parsers (`status_paragraphs`).

### M10. Removal loop claims dependency ordering but does none — `transaction.rs::remove_packages_sequentially` (~line 1180)
Comment says "Process packages in dependency order (leaves first)" but packages are processed exactly as given; removing a parent before its dependency-running child can leave prerm scripts executing in a broken environment.
Fix: topologically order removals (reverse of install graph) or fix the comment.

### M11. Search fast-path TTL eviction can clear the mmap while another thread holds stale assumptions — `db.rs::ensure_mmap_loaded` / `ensure_fst_loaded`
The read→drop→write-lock dance between checking expiry and clearing means two threads can interleave (A reloads, B clears A's fresh copy). Benign today (next caller reloads), but `is_expired` returning true after 30 min idle forces expensive reloads in long-running daemon sessions despite unchanged files — the TTL evicts based on access time, not file freshness, unlike the mtime-validated DPKG cache.
Fix: validate underlying file mtime instead of blind TTL eviction.

### M12. `is_apt_cache_fresh` treats any recently-touched lists dir as globally fresh — `parallel_sync.rs` (~line 55)
Directory mtime or a single fresh `_Packages` file short-circuits the entire multi-repo sync; one repo updated by apt while another repo was added to sources.list stays unsynced for 6 h.
Fix: compare per-repo Release freshness instead of a global heuristic.

### M13. Content-store `store()` skips integrity check on existing blob — `content_store.rs::store` (~line 60)
If a previous interrupted write left a truncated file at `dest` (pre-dating the atomic temp+rename fix, or renamed from a partial copy on a FS without rename atomicity guarantees), `dest.exists()` short-circuits and the corrupt blob is hard-linked into installations. Hash verification downstream catches it, but then the poisoned blob is never repaired (hard_link path fails → redownload each time).
Fix: verify size (cheap) or re-hash when the existing file size differs from the source.

### M14. `check_disk_space` probes the wrong directories — `validation.rs::check_disk_space` (~line 15)
Downloads go to `TempDir::new()` (`/tmp`) and the content store lives in `paths::cache_dir()`, but callers pass `std::env::temp_dir()` only for the download check and `/` for install; a tmpfs-backed small /tmp or separate /var/cache partition isn't accounted for. Also `installed_size` headroom of 20 % is below apt's own accounting for overlapping upgrades.
Fix: stat the actual download dir and content-store mount points.

### M15. deb822 `Enabled:` parsing rejects numeric forms — `sources.rs::expand_deb822_stanza` (~line 430)
`enabled = s != "no" && s != "false"`: values `no ` variants are fine, but apt also honors `Enabled: 0`/case differences (`NO`)? (Case-insensitive per RFC822-ish conventions). `enabled: No` works only because key lowercasing happens but value comparison is exact-case. A repo disabled with `Enabled: NO` is treated enabled → packages synced/installed from a repo the admin disabled.
Fix: case-insensitive comparison and accept `0`.

## LOW

### L1. `split_epoch` mis-splits versions whose upstream contains `:` — `resolver.rs` (~line 520)
`memchr(':')` takes the FIRST colon; a malformed/foreign version `1.0:abc` gets epoch parsed from `1.0` (fails→0) and remainder `abc`, comparing inconsistently against the same string elsewhere. dpkg splits at the *last* colon for tolerance. Low likelihood on valid Debian versions.

### L2. `parse_single_dep` finds `)` anywhere in the string — `resolver.rs` (~line 555)
`memchr(b')')` searches from position 0 rather than after `paren_start`; harmless for well-formed deps but a dep like `foo)bar (>= 1)` silently drops the constraint instead of failing.

### L3. `parse_status_paragraph` lets continuation lines clobber scalar fields — `db.rs` (~line 1665)
Continuation lines (indented Description text) aren't skipped; an indented line containing `Version: x` inside a description overwrites the version. Also multiline descriptions lose their continuation text here (only first line kept).

### L4. `find_info_from_apt_lists_fast` returns first match across files regardless of suite priority — `db.rs` (~line 1160)
Scans lists files in readdir order and returns the first `Package: name` hit, so info may come from bookworm-backports instead of the highest-priority suite; also no arch preference.

### L5. TTL expiry forces full rebuild even when nothing changed — `db.rs::ensure_index_loaded` (~line 540)
When `last_accessed` expires, `*cache = DebianIndexCache::default()` wipes `file_mtimes`, making `has_changed_files` true on next call → full reparsing of all Packages files every 30 idle minutes unless the LZ4 cache happens to be fresher. Wasted CPU/IO in daemon mode.

### L6. `verify_inrelease_signature` blocks the async executor — `parallel_sync.rs` (~line 240)
`std::process::Command::new("gpgv")...status()` runs synchronously inside an async fn (called from `sync_repository`), stalling a tokio worker for the duration of gpgv.

### L7. `unix_now_secs` collapses pre-epoch clocks to 0 — `db.rs` (~line 30)
`unwrap_or_default()` yields 0, which `is_access_expired` interprets as "never accessed" → caches never evict if the system clock is before 1970 (VMs without RTC). Cosmetic robustness.

### L8. `remove_deb_files` TOCTOU on metadata — `db.rs` (~line 2470)
`fs::metadata` size is summed before `remove_file`; a file replaced between the calls misreports freed bytes. Trivial accounting issue.

### L9. Dead code: dnf `repomd` module unused — `dnf.rs` bottom (~lines 620+)
`parse_repomd`/`load_verified_repomd` are `pub` but referenced nowhere else in the crate (grep confirms). Either wire them into the planned repo-metadata integration or remove; as-is they are untested-in-production surface (well unit-tested, but dead).

### L10. `list_updates` (apt) emits empty old_version — `apt.rs::list_updates` (~line 340)
`pkg.installed().map(...).unwrap_or_default()` — upgradable packages should always be installed, but if not, an empty old_version flows into UpdateInfo/UI instead of an explicit marker.

### L11. `map_local_package` clamping claim mismatch — `apt.rs` (~line 590)
Comment says sizes are "clamped to i64::MAX via unwrap_or" but the code uses `as i64` cast (wrap-negative for absurd sizes). Harmless practically; comment misleading.

### L12. Homebrew casks always reported not installed in search/info — `homebrew.rs::fuzzy_search` / `info` (~lines 465, 700)
`installed: false` hardcoded for casks; `formula.installed` comes from formulae.brew.sh JSON which does not carry local install state, so formula `installed` flag is effectively always false there too. Cellar cross-check (as done in `read_installed_packages`) is not applied to search results.

### L13. Silent skip of unreadable Cellar entries — `homebrew.rs::read_installed_packages` (~line 370)
`if let Ok(pkg) = self.read_package_info(...)` drops packages on any IO/parse error with no log — inventory quietly undercounts on permission problems.

### L14. `save_cache_to_disk` non-atomic write — `homebrew.rs` (~line 290)
`fs::write` directly to `formula.rkyv`; crash mid-write leaves a corrupt cache. Load path tolerates it (falls through), so impact is limited to losing the cache; still trivially fixable with temp+rename (and the docstring falsely claims checksum validation via the unused `_meta_path`).

### L15. Global `INSTALLED_CACHE` shared across differently-configured manager instances — `homebrew.rs` (~line 50)
Two `HomebrewPackageManager`s with different prefixes (ARM vs Intel detection differences across instances) thrash one global cache keyed only by names.

### L16. `run_brew` passes package names without `--` separator — `homebrew.rs::install/remove`
`brew install <names...>`: safe only because `validate_package_names` rejects leading-dash tokens; a validator regression turns names into brew options. Cheap hardening: insert `--`.

### L17. `get_status(fast=true)` reports zeros for orphans/updates on Debian backends — `mod.rs::resolve_status_counts` + `apt.rs::get_status`
Documented, but any UI surfacing these zeros presents fabricated "0 orphans / 0 updates" as real data on the fast path.

### L18. `sync_with_progress` prints failure summary but returns only the first error — `parallel_sync.rs` (~line 200)
`for result in results { result?; }` discards the remaining failure details the summary just counted.

### L19. `ContentStore::total_size` counts temp files — `content_store.rs` (~line 130)
`.tmp` residue is included in reported store size (only `package_count` filters dotfiles); inconsistent stats.

## INFO

### I1. `populate_package_urls` clones the entire package index per transaction — `debian_pure.rs` (~line 400)
`get_detailed_packages()?` clones ~94k packages (with descriptions/depends vectors) for every install/update. The code itself notes the optimization TODO (`get_packages_by_names`).

### I2. `similarity_prefix` suggestion scan is O(index) per miss — `resolver.rs::add_package`
Iterates all available names for suggestions on every unknown package; fine interactively, hot if batched.

### I3. `DebianMmapIndex::Drop` only logs — `db.rs` (~line 250)
Explicit Drop impl documents cleanup but adds no behavior beyond `Mmap::drop`; fine, noted for audit completeness.

### I4. Duplicated doc-comment lines — `content_store.rs::store` (~lines 47–49)
"Returns the hash for later retrieval…" repeated twice; cosmetic.

### I5. `parse_repomd` accepts only self-closing `<location/>` — `dnf.rs`
Non-self-closing form would fail closed (missing location) rather than parse; acceptable but worth noting when the module is wired up.

### I6. `prepare_status_entry` fabricates `Status: install ok installed` — `transaction.rs` (~line 1330)
Installed packages are registered fully configured even when postinst hasn't run yet (see M4); consistent with M4, listed for traceability.

---
**Totals:** 3 CRITICAL, 7 HIGH, 15 MEDIUM, 19 LOW, 6 INFO — 50 findings.


---

# SLICE 14

# Slice 14 — package_managers: mock.rs, traits.rs, types.rs, mod.rs + cross-cutting consistency

Audit of `/home/pyro1121/Documents/omg/src/package_managers/{mock.rs, traits.rs, types.rs, mod.rs}`
plus a cross-cutting consistency review of all backends (apt.rs, arch.rs, dnf.rs, homebrew.rs,
debian_pure.rs, aur/*). Read-only; no builds or tests executed.

---

## HIGH

### H-1. Fast-path helpers bypass `OMG_TEST_MODE` — mock backend silently skipped
- **File:** `src/package_managers/mod.rs`, lines ~44–150 (`list_installed_fast`, `is_installed_fast`, `get_package_info`, `list_orphans_fast`, `get_counts`, `get_system_status`) and lines ~46–60 (`search_sync` / `list_explicit_fast` debian branch ordering).
- **Excerpt:**
  ```rust
  pub fn list_installed_fast() -> anyhow::Result<Vec<LocalPackage>> {
      #[cfg(any(feature = "debian", feature = "debian-pure"))]
      if crate::core::env::distro::is_debian_like() {
          return local_packages_from_debian_db();
      }
      #[cfg(feature = "arch")]
      return alpm_direct::list_installed_fast();
  ```
- **Why it is a bug:** `mock.rs` documents "Enabled only when `OMG_TEST_MODE=1` is set", and
  `get_package_manager()` (mod.rs ~line 300) explicitly routes test mode to the mock with a comment
  that fast-path helpers must not recurse into the real backend. Yet none of the six `_fast`
  functions above consult `crate::core::paths::test_mode()` at all. Under `OMG_TEST_MODE=1` on an
  Arch build, `list_installed_fast`, `get_counts`, `get_system_status`, `list_orphans_fast`,
  `is_installed_fast`, and `get_package_info` all hit the real alpm database; on Debian-like hosts,
  even `search_sync` and `list_explicit_fast` check `is_debian_like()` *before* their test-mode
  branch, so the mock is unreachable there. Tests run in test mode will read (and status commands
  will report) the developer's real system packages instead of the fixture state.
- **Fix:** Check `crate::core::paths::test_mode()` first in every fast helper and route through
  `MockPackageManager` (e.g. via `get_package_manager()` or the existing `list_explicit_sync`),
  matching the contract asserted in `get_package_manager`.

### H-2. Non-Arch mock update detection uses raw lexicographic string compare — reintroduces the exact bug `types.rs` documents as fixed
- **File:** `src/package_managers/mock.rs`, lines ~250–253 (`is_newer`) and ~478–482 (`list_updates`).
- **Excerpt:**
  ```rust
  #[cfg(not(feature = "arch"))]
  fn is_newer(old: &str, new: &str) -> bool {
      matches!(old.cmp(new), std::cmp::Ordering::Less)
  }
  ...
  let is_update_needed = Self::is_newer(installed_ver, available_ver);
  ```
- **Why it is a bug:** `"1.9".cmp("1.10") == Greater`, so on every non-Arch build the mock reports
  `firefox 1.9 → 1.10` as "no update". `types.rs` (lines ~57–63) contains a regression guard whose
  comment says precisely this: *"this was a bare String, so 1.9 > 1.10 compared lexicographically
  and security updates were silently reported as up to date"*. The mock re-introduces it by not
  using `DebVersion`/`parse_version_or_zero` on the non-Arch path (the Arch path does use
  `parse_version_or_zero`).
- **Fix:** Delete `is_newer` and compare `parse_version_or_zero(available) > parse_version_or_zero(installed)`
  unconditionally (both cfg branches become identical).

## MEDIUM

### M-1. `deb_char_order` diverges from dpkg policy: punctuation sorts below letters instead of above
- **File:** `src/package_managers/types.rs`, lines ~110–117 (`deb_char_order`).
- **Excerpt:**
  ```rust
  fn deb_char_order(c: u8) -> i64 {
      match c {
          b'~' => -1,
          b'0'..=b'9' => 0,
          _ => i64::from(c),
      }
  }
  ```
- **Why it is a bug:** dpkg's `verrevcmp`/`order()` weights non-alphanumeric characters as `c + 256`
  so that all letters sort before all non-letters within a non-digit fragment. Here `.`, `+`, `:`
  etc. keep their plain ASCII value (< `'a'`), so e.g. upstream versions `"1.0"` vs `"1a"` compare
  as `1.0 < 1a` where dpkg orders `1.0 > 1a`. Any real-world version containing both letters and
  punctuation in one fragment (common: `1.0~beta`, but also `2.38-4+b1` style revision fragments,
  `1.0+jre` vs `1.0a`) can be mis-ordered, hiding or fabricating updates for non-Arch/homebrew-style
  versions compared through this type.
- **Fix:** `_ => i64::from(c) + 256` for the non-alnum arm (letters stay `i64::from(c)`), matching
  dpkg §5.6.12.

### M-2. apt install validates with `validate_package_names`, rejecting the local `.deb` files its own installer supports
- **File:** `src/package_managers/apt.rs`, lines ~82 and ~479–495 (`install`, `install_blocking`); contrast `arch.rs` line 102.
- **Excerpt:** apt install calls `validate_package_names(&packages)?` while `install_blocking`
  partitions inputs into `.deb`/`.ddeb` file paths vs names:
  ```rust
  path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("deb") ...)
  ```
- **Why it is a bug:** `validate_package_names` → `validate_package_name` rejects any input starting
  with `/` (`PackageNameAbsolute`) and any input starting with `.`. Absolute `.deb` paths therefore
  always fail validation before reaching `install_blocking`, making the entire local-file partition
  dead code and breaking UX for local .deb installs. The Arch backend correctly uses
  `validate_package_names_or_files`; apt does not (and there is no `.deb` analogue in
  `is_local_package_file`, which only accepts `.pkg.tar.*`).
- **Fix:** Add a `.deb`/`.ddeb` variant of `is_local_package_file` and use
  `validate_package_names_or_files`-style validation in apt's install path (keep strict name
  validation for remove).

### M-3. `get_package_manager` fallback silently returns the wrong backend for detected-but-unfeatured distros
- **File:** `src/package_managers/mod.rs`, lines ~330–345 (the `_ =>` fallback arm).
- **Excerpt:**
  ```rust
  _ => {
      #[cfg(feature = "arch")]
      return Ok(Arc::new(ArchPackageManager::new()));
  ```
- **Why it is a bug:** If distro detection yields `Distro::Fedora` (or MacOS/Debian) but the binary
  was built without that feature, the wildcard arm hands back an ArchPackageManager anyway (when the
  `arch` feature is compiled in). Subsequent install/remove would attempt pacman/libalpm operations
  on a machine without them, producing confusing failures at best and wrong-system mutations at
  worst. A silent wrong-backend fallback contradicts the explicit-failure philosophy used for
  debian-pure two arms above.
- **Fix:** Match on the *detected* distro in the fallback and return an explicit
  "this build has no backend for <distro>" error instead of defaulting to Arch.

### M-4. Mock state read-modify-write is not atomic across processes; concurrent writers lose updates
- **File:** `src/package_managers/mock.rs`, lines ~190–246 (`set_installed_version`, `set_available_version`, `create_update_scenario`, `install`, `remove` — all follow load → mutate → `save_state`).
- **Why it is a bug:** The whole point of persisted state is "stateful tests across CLI runs"
  (module doc). Two concurrent CLI invocations (or `create_update_scenario` interleaved with an
  install) each load the file, mutate in memory, and overwrite — last writer wins and the other's
  insert/remove is silently lost. There is no lockfile or compare-and-swap. In-memory
  `MockPackageManager` instances share nothing either (state lives only in the JSON file), so the
  `Arc<Mutex<…>>` on `MockPackageDb` gives false confidence about state safety.
- **Fix:** Use an advisory file lock (e.g. `fs2`/`flock`) around load-modify-save, or serialize all
  mutations through a single process-wide mutex keyed on the state path.

## LOW

### L-1. Epoch overflow silently coerces to epoch 0 in dpkg version comparison
- **File:** `src/package_managers/types.rs`, lines ~152–160 (`split_epoch`).
- **Excerpt:** `(epoch.parse::<u64>().unwrap_or(0), rest)`
- **Why it is a bug:** An epoch exceeding `u64` (e.g. adversarial or corrupt metadata `99999999999999999999:1.0`) parses to `Err` and is coerced to epoch `0`, ordering it *below* `1:0` when it should sort highest. Silent coercion rather than clamping/explicit failure can hide updates.
- **Fix:** Use `saturating_parse`-style clamping to `u64::MAX`, or treat overflow as a comparison error.

### L-2. Legacy mock-state migration silently discards installed versions
- **File:** `src/package_managers/mock.rs`, lines ~222–240 (`load_state` legacy branch).
- **Excerpt:** `value.as_str().map(|name| (name.to_string(), "0".to_string()))`
- **Why it is a bug:** When reading the old array-format state, every installed version becomes `"0"`. Update scenarios built against legacy files then report spurious upgrades for everything, and `list_installed` shows version `0`. Data loss is silent (no warning log).
- **Fix:** Emit a `tracing::warn!` and/or preserve the version if present in the legacy schema; document the migration.

### L-3. `std::sync::MutexGuard` held across file I/O inside async futures (mock db lock)
- **File:** `src/package_managers/mock.rs`, e.g. `search` (~line 268), `info`, `list_updates`: `pkgs` guard is alive while `load_state()`/`save_state()` perform blocking disk I/O.
- **Why it is a bug:** Blocking file I/O under a std Mutex inside a `Box<dyn Future>` blocks the executor thread and stalls other tasks needing the same db lock; if the future is dropped mid-poll the guard pattern also makes cancellation semantics murky. Test-only code, hence LOW.
- **Fix:** Clone needed data out of the lock, drop the guard, then do I/O; or use `spawn_blocking`.

### L-4. Homebrew command construction lacks `--` end-of-options separator
- **File:** `src/package_managers/homebrew.rs`, lines ~769–796 (`install`, `remove`): `let mut args = vec!["install"]; ...` passed to `run_brew`.
- **Why it is a bug:** dnf and apt privileged paths consistently append `--` before operands; brew does not. Validation (`PackageNameStartsWithDash`) currently blocks leading-dash names so exploitation is prevented today, but the inconsistency means any future relaxation of validation (e.g. allowing flags like `--cask`) becomes an option-injection hole, and formula names beginning with `-` are already rejected rather than safely passed through.
- **Fix:** Insert `"--"` after the subcommand for symmetry: `vec!["install", "--"]`.

### L-5. `is_safe_package_char` permits `/` in package names
- **File:** `src/core/security/validation.rs`, line ~211 (consumed by `validate_package_name` used by all managers).
- **Excerpt:** `matches!(c, '-' | '_' | '+' | '.' | '@' | '/')`
- **Why it is a bug:** No rpm/deb/apk/pacman package name legally contains `/`. Allowing slashes means values like `../../usr/bin/x` pass the character allowlist (the separate `contains("..")` check catches only literal `..`, not `a/b/../c`-style or single-segment paths). With the `--` separators in place this is defense-in-depth, but the allowlist is wider than the grammar it protects.
- **Fix:** Drop `/` from the allowlist for package names (it is already separately handled for image refs and local-file paths).

### L-6. Mock `get_status` always reports `updates = 0` and `orphans = 0`
- **File:** `src/package_managers/mock.rs`, lines ~420–430.
- **Why it is a bug:** `list_updates()` can return non-empty results, but `get_status` hardcodes the updates count to `0`, so CLI/daemon surfaces that use `get_status` contradict those using `list_updates` in the same mock environment. Orphans = 0 is documented/deliberate; the updates count is simply inconsistent.
- **Fix:** Compute updates via the same logic as `list_updates` (share a helper).

### L-7. Mock `remove` succeeds silently for never-installed packages
- **File:** `src/package_managers/mock.rs`, lines ~343–354.
- **Why it is a bug:** Unlike install (which errors on unknown packages), remove neither errors nor warns when the package was not in `installed`. Cross-backend behavioral inconsistency: pacman/apt/dnf removals of nonexistent packages fail loudly, so tests written against the mock can pass where production would fail.
- **Fix:** Bail with "package {pkg} is not installed" to mirror real semantics (or add a `--force`-style knob).

## INFO

### I-1. Mock search cannot see packages that exist only in persisted `available` state
- **File:** `src/package_managers/mock.rs`, `search` (~lines 260–290) vs `install` (~line 315).
- `install` accepts any key of `state.available` even when absent from the static db, but `search` filters only `db.packages`, so such packages are installable yet invisible/unsearchable. Harmless in practice but surprising; consider merging `state.available` keys into search results.

### I-2. `traits.rs` `get_status(fast: bool)` returns an opaque 4-tuple
- **File:** `src/package_managers/traits.rs`, lines ~50–53.
- `(usize, usize, usize, usize)` (total, explicit, orphans, updates) is easy to transpose at call sites and already caused divergence risk (see types.rs orphan-rule comment). A named struct would make misuse unrepresentable. Not a live bug.

### I-3. `contains_ignore_case` is byte-oriented ASCII-only by design
- **File:** `src/package_managers/types.rs`, lines ~22–37. Correct for its stated purpose; uppercase multi-byte needles (e.g. Turkish İ) silently won't match. Documented behavior; no action.

### I-4. Redundant `self.db.clone()` before locking in mock methods
- **File:** `src/package_managers/mock.rs` (`search`, `info`, `list_updates`). Cloning the `MockPackageDb` Arc and then locking through the clone is equivalent to locking `self.db` directly; noise only.

### I-5. `backend_name_for_distro` maps unknown distros to backend `"mock"`
- **File:** `src/package_managers/mock.rs`, lines ~120–130. Means `MockPackageManager::new("nixos").name() == "mock"` and its state file is `mock_state_mock.json`, shared by all unknown distros. Fine for tests; worth a comment.

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 2 |
| MEDIUM | 4 |
| LOW | 7 |
| INFO | 5 |

Total: **18 findings**.

Positives noted: no shell interpolation anywhere (all subprocesses use `Command::arg/args`, no `sh -c`); consistent `validate_package_names` gating on install/remove/info across arch/apt/dnf/homebrew/AUR; `types.rs` dpkg comparison and canonical orphan predicate are well-tested; mock state writes are atomic per-write (`safe_ops::atomic_write_file_sync`).


---

# SLICE 15

# Audit slice-15 — tests/ files a–c (absolute_coverage, alpm_harness, alpm_harness_integration, alpm_transaction_e2e, arch_cli_contracts, arch_tests, aur_dependency_resolution, cli_comprehensive, cli_integration, cross_platform_mock_tests)

Read-only audit. All findings verified against source at ~/Documents/omg/tests.

---

## HIGH

### H-1. Test helper runs `sudo pacman -Rns --noconfirm` against the real system package DB
- **File:** `tests/aur_dependency_resolution.rs`, lines ~536–548 (`cleanup_package`)
- **Excerpt:**
  ```rust
  let _ = Command::new("sudo")
      .args(["pacman", "-Rns", "--noconfirm", package])
      .output();
  ```
- **Why it's a bug:** This is a destructive system mutation executed from a test helper. When `OMG_RUN_SYSTEM_TESTS=1` + `require_arch!()` gates open (CI containers with sudo passwordless, or a developer's Arch host), three tests (`test_realworld_aur_helper_dependencies`, `test_realworld_aur_to_aur_dependencies`, `test_realworld_optional_dependencies`) will *remove* the real `yay-bin` (and whatever state `-Rns` cascades to) with no confirmation. Additionally, plain `sudo` without `-n` can block indefinitely on a password prompt in non-interactive environments (the `.output()` call has no timeout), hanging the test binary until the global test harness timeout.
- **Fix:** Use `sudo -n` (fail fast instead of prompting), add an explicit opt-in env gate for the uninstall step (reuse `require_destructive_tests!` semantics), and/or target a fake root (`pacman --root`) instead of the live database. Also propagate/check the command result instead of discarding it with `let _ =`.

---

## MEDIUM

### M-1. Dead JSON validation — parse result discarded
- **File:** `tests/arch_tests.rs`, `mod new_features`, `test_outdated_json_output` (~line 407)
- **Excerpt:**
  ```rust
  if result.success && !result.stdout.trim().is_empty() {
      let _: Result<serde_json::Value, _> = serde_json::from_str(&result.stdout);
  }
  ```
- **Why it's a bug:** The `Result` is bound and immediately dropped; invalid JSON never fails the test. The test name promises "valid JSON output" but asserts nothing — a fully dead test that gives false coverage of the `outdated --json` contract.
- **Fix:** `let parsed: serde_json::Value = serde_json::from_str(&result.stdout).expect("outdated --json emitted invalid JSON");` and optionally assert it is an array/object per the documented schema.

### M-2. Harness silently truncates an existing sync DB on re-add
- **File:** `tests/alpm_harness.rs`, `add_sync_pkgs` (~line 76): `let file = File::create(&db_path)?;`
- **Why it's a bug:** `File::create` truncates. Calling `add_sync_pkg("core", &a)` followed by `add_sync_pkg("core", &b)` silently destroys package `a`. Several call sites already carry workarounds/comments ("must add all at once"), e.g. `alpm_harness_integration.rs::test_harness_package_search` and `test_harness_package_listing`; any future caller who misses this gets a silently wrong fixture and possibly a green test validating the wrong thing.
- **Fix:** Open with append+read-modify-write, or return an error when the db file already exists, or document/enforce single-call-per-db via typestate. At minimum panic/error on existing db so misuse is loud.

### M-3. Callback tests register callbacks but assert nothing about them
- **File:** `tests/alpm_transaction_e2e.rs`, `test_alpm_log_callback` (~line 193) and `test_alpm_progress_callback` (~line 207)
- **Excerpt:**
  ```rust
  let log_count = Arc::new(AtomicU64::new(0));
  ...
  alpm.set_log_cb(Arc::clone(&log_count), |_level, _msg, counter| {
      counter.fetch_add(1, Ordering::Relaxed);
  });
  alpm.trans_init(...); alpm.trans_release(...);
  // end of test — log_count never checked
  ```
- **Why it's a bug:** Neither test ever reads the counter, so both pass even if the callback mechanism is completely broken (never invoked, mis-dispatched, UB in the trampoline). They are dead tests wearing the name of callback verification.
- **Fix:** After init/release, assert `log_count.load(Relaxed) > 0` (ALPM emits logs during trans ops) or trigger an operation guaranteed to fire progress/log events, then assert.

### M-4. Vacuous "should not panic" tests provide false coverage across arch_tests.rs
- **File:** `tests/arch_tests.rs` — `test_why_command`, `test_why_reverse_dependencies`, `test_size_tree`, `test_blame_command`, `test_diff_command`, `test_snapshot_create`, `test_ci_init_github`, `test_migrate_export`, `test_audit_sbom_generation`, `test_audit_secrets_scan`, `test_audit_policy_enforcement`, plus steps 3–4 of `scenario_full_workflow` and all assertions in `scenario_team_collaboration`
- **Excerpt (representative):**
  ```rust
  let result = project.run(&["snapshot", "create"]);
  assert!(!result.stderr_contains("panicked at"), "Should not panic");
  ```
- **Why it's a bug:** These only check absence of the literal string "panicked at" in stderr. A command that exits non-zero, prints an error, writes nothing, or corrupts state still passes. E.g. `test_ci_init_github` claims "Should generate GitHub Actions config" but never checks any file exists; `test_snapshot_create` never verifies a snapshot was created; `scenario_team_collaboration`'s whole body is one no-panic check inside an `if let`. Coverage dashboards count these as tested behavior when they verify nothing beyond process launch.
- **Fix:** Assert observable outcomes: success status, created artifacts (`project.file_exists(".github/workflows/omg.yml")` etc.), or explicit expected failure messages. If only smoke-level checking is intended, rename/mark them as smoke tests.

### M-5. Cross-platform mock suite: unsynchronized readers race serial writers on shared state files
- **File:** `tests/cross_platform_mock_tests.rs`, `TEST_DATA_DIR` (lines 17–18) vs. mod structure
- **Why it's a bug:** All platform mocks share one `LazyLock<TempDir>`; each backend persists to `mock_state_{platform}.json` under it. Mutating tests are `#[serial]`, but several mutating-or-reading tests are NOT marked `#[serial]`: e.g. `search_functionality::*`, `package_info::*` read state while serial writers (`update_scenarios::test_arch_updates`, `install_remove_operations::*`, `status_reporting::*`) run concurrently — Rust's test harness runs non-serial tests in parallel with serial ones. If `MockPackageManager` read-modify-writes the JSON per operation, concurrent access can observe torn/partial writes or lose updates, producing order-dependent flakes (e.g. `search` seeing a half-written state file).
- **Fix:** Mark every test touching the shared `TEST_DATA_DIR` `#[serial]`, or give each test its own data dir via `MockBackend::new_in(platform, unique_dir)`.

### M-6. Weak OR-chains can mask real failures in negative-path CLI tests
- **Files:**
  - `tests/cli_comprehensive.rs::test_install_nonexistent` (~line 21): passes if `result.success` is true — i.e., a run where `omg install <nonexistent>` **succeeds** passes the "nonexistent install should fail" test.
  - `tests/arch_tests.rs::test_info_nonexistent_package` (~line 130): `assert!(!result.success || result.contains("not found"))` — same shape inverted, acceptable, but `cli_integration.rs::test_info_nonexistent_package` accepts `combined.contains("not found") || contains("No package") || !result.success` where a success exit plus incidental text also passes.
- **Why it's a bug:** Leading `success ||` disjunct makes the security/correctness property unenforced: a regression that happily "installs" nothing and exits 0 would not be caught.
- **Fix:** For nonexistent-package cases require `!result.success` AND an explanatory message; drop the success escape hatch.

---

## LOW

### L-1. `topological_levels` complex-graph assertion too weak to catch off-by-one
- **File:** `tests/aur_dependency_resolution.rs::test_toposort_complex_graph` (~line 205)
- **Excerpt:** after exact asserts for levels 0–2, only `assert!(levels.len() >= 5);`
- **Why:** The graph has exactly 6 levels (a; b; c; {d,e}; {f,g}; h). `>= 5` tolerates a buggy level merge/split. Also comment "f and g in different levels or same depending on completion" is wrong — f needs d and e, g needs e, so both are deterministically at level 4.
- **Fix:** `assert_eq!(levels.len(), 6); assert_eq!(levels[3].len(), 2); assert_eq!(levels[4].len(), 2); assert_eq!(levels[5], vec!["h"]);`

### L-2. Destructive-gated real-world AUR tests mutate host state by installing packages
- **File:** `tests/aur_dependency_resolution.rs` lines ~370–430 (`run_omg(&["install", "yay-bin"])` ×3)
- **Why:** Even behind `require_system_tests!`/`require_arch!`, these install real software into the running system and are not additionally gated by `require_destructive_tests!`, inconsistent with `arch_tests.rs::test_sync_databases` which does gate destructive ops. Also `test_realworld_optional_dependencies` is vacuous: both branches of its `if` just print success.
- **Fix:** Add `require_destructive_tests!()`; replace the if/else with a real assertion or delete the test.

### L-3. Real-system ALPM tests assume non-empty databases / bash installed
- **File:** `tests/alpm_transaction_e2e.rs` lines ~140–185
- **Why:** `test_alpm_sync_database_query` fails on any Arch system/container whose sync DBs haven't been populated (`pacman -Sy` never run) even though the code under test is fine; `test_alpm_dependency_chain_query` silently passes when bash isn't installed (the `if let Ok(bash)` swallows the interesting case) and its `provides` satisfaction check compares names only, ignoring version constraints (`dep.name()` vs `prov.name()`), which can false-positive on versioned provides.
- **Fix:** Skip-with-message when localdb/syncdbs are empty instead of failing; handle the bash-absent case explicitly; match dep versions against provides versions.

### L-4. `trans_add_pkg` results discarded — transaction tests can pass vacuously
- **File:** `tests/alpm_transaction_e2e.rs`, `test_alpm_transaction_prepare` (~line 90) and `test_alpm_transaction_multiple_packages` (~line 118)
- **Excerpt:** `let _ = alpm.trans_add_pkg(pkg);`
- **Why:** If every lookup/add silently fails, `test_alpm_transaction_prepare` still passes (nothing asserted about prepare outcome either — `drop(alpm.trans_prepare())`). Only `test_alpm_transaction_multiple_packages` follows up with a count assert; prepare does not.
- **Fix:** Expect the add result; assert prepare outcome (Ok, or a specific expected error class for the minimal harness).

### L-5. Environment-dependent flakiness: `explicit` output must be non-empty
- **File:** `tests/cli_integration.rs::test_list_explicit` (~line 96)
- **Why:** `assert!(!result.stdout.is_empty(), "Should list explicit packages")` fails legitimately on a minimal chroot/container with zero explicitly installed packages. Same family: `arch_tests.rs::test_explicit_packages_count` requires parseable count (fine) but `test_size_command` requires "MB"/"GB" literals, which breaks if formatting changes to KiB/MiB (note `test_alpm_size_calculation` in the same file already accepts KiB — the two duplicate tests disagree on units).
- **Fix:** Accept empty list explicitly ("No explicit packages" path); unify size-unit expectations across `alpm_direct::test_alpm_size_calculation` and `new_features::test_size_command`.

### L-6. `combined_output` concatenates stdout+stderr with no separator
- **Files:** `tests/common/mod.rs` line 159 (used throughout scope), `tests/cli_integration.rs::test_info_pacman` line ~63 (`format!("{}{}", result.stdout, result.stderr)`)
- **Why:** Token-splicing across the boundary can create accidental matches ("...foo" + "bar..." → "foobar") or hide mismatches in substring assertions like `contains("not found")`.
- **Fix:** Join with `'\n'`.

### L-7. `AlpmHarness::alpm()` unwraps path UTF-8 conversion
- **File:** `tests/alpm_harness.rs` lines ~62–64: `self.root_path.to_str().unwrap()`
- **Why:** Panics rather than erroring if TMPDIR is non-UTF-8. Low impact (temp dirs are normally UTF-8), but the function returns `Result` and should use `?`/context for consistency with the file's error discipline.
- **Fix:** `let root = self.root_path.to_str().context("non-UTF8 temp path")?;`

### L-8. `CurrentDirGuard::drop` panics inside Drop
- **File:** `tests/absolute_coverage.rs` lines ~85–95
- **Why:** `std::env::set_current_dir(&self.previous).expect(...)` — if restore fails during unwind, this panics-in-drop and can abort the process, masking the original test failure. Also the guard changes process-global CWD; correctness relies on `#[serial]`, which is satisfied today but fragile if someone adds a non-serial CWD test to the same binary.
- **Fix:** Log-and-best-effort restore (or `let _ =`) in Drop, keeping a hard expect only in `change_to`.

### L-9. Duplicate inner `#![cfg]` attributes are easy to misread
- **File:** `tests/aur_dependency_resolution.rs` lines 8 and 15 (`#![cfg(feature = "arch")]` then `#![cfg(all(feature = "arch", target_os = "linux"))]`)
- **Why:** Multiple `cfg` attrs AND together, so this compiles to feature=arch ∧ linux — correct but confusing; a reader may think the second overrides the first. Also means the pure graph/toposort unit tests silently vanish on non-Linux even though they're platform-independent logic.
- **Fix:** Collapse into one attribute; consider moving pure-graph tests out from platform gating.

### L-10. Tautological / near-tautological assertions
- **Files / lines:**
  - `tests/arch_tests.rs::test_explicit_packages_list`: `!result.stdout.trim().is_empty() || result.stdout_contains("0")` — the second disjunct fires whenever "0" appears anywhere (timestamps, counts, version strings).
  - `tests/cli_comprehensive.rs::test_hook_fish`: `output.contains("source") || output.contains("omg")` — almost always true because "omg" appears in usage text.
  - `tests/cli_integration.rs::test_search_empty_query`: `result.success || !result.stdout.is_empty()` — passes for nearly any outcome including a crash-with-partial-output.
  - `tests/cli_comprehensive.rs::test_update_dry_run`: `result.success || combined.contains("up to date")` — success alone passes, so the "--check shows updates" property is unverified.
- **Fix:** Tighten each to the specific documented behavior.

### L-11. Injection-prevention tests don't distinguish safe rejection from silent acceptance
- **File:** `tests/arch_tests.rs`, `security::test_injection_prevention_search` / `test_injection_prevention_info` (~lines 520–560)
- **Why:** Assertions only check that "pwned"/passwd content is absent from stdout. A build where the injection payload is passed verbatim to pacman (which errors harmlessly today for unrelated reasons) passes identically to one where input is validated up front. No assertion on exit status or on an explicit validation error message.
- **Fix:** Assert the CLI rejects the input (non-zero exit or explicit "invalid package name" message) for the clearly-invalid payloads in `INJECTION_ATTEMPTS`.

### L-12. Misleading dead module: `integration_with_omg` doesn't touch OMG code
- **File:** `tests/alpm_harness_integration.rs` lines ~289–311 (`test_omg_alpm_ops_with_harness`)
- **Why:** Comment admits "This would normally call functions from src/package_managers/alpm_ops.rs ... For now, just verify the harness provides valid ALPM handles". The name claims production integration; it exercises none. Dead/misleading test.
- **Fix:** Either wire it to the real `alpm_direct` API against the harness, or rename to `harness_smoke` and move into harness unit tests.

### L-13. Unused harness setup in version-comparison test
- **File:** `tests/alpm_harness_integration.rs::test_harness_version_comparison` (~line 51)
- **Why:** Creates a full harness + two sync packages + an `_alpm` handle, then compares static `alpm::Version::new(...)` values that need none of it. Wasted I/O per run; misleading (suggests versions come from the DB).
- **Fix:** Drop the harness entirely — the comparisons are self-contained; keep DB-backed comparison in a separate test that actually queries pkg versions.

---

## INFO

### I-1. Pointless wrapper struct in cross-platform mock tests
- **File:** `tests/cross_platform_mock_tests.rs` lines 20–37: `struct MockPackageManager;` with associated fns adds no state over calling `MockBackend::new_in` directly.
- **Fix:** Replace with free functions or direct calls.

### I-2. Weak drift-error assertion
- **File:** `tests/absolute_coverage.rs::test_env_check_fails_on_drift` (~line 116): asserts only `is_err()` for a `"{}"` lockfile; a regression that errors for the wrong reason (parse failure vs drift detection) passes.
- **Fix:** Assert the drift-specific message like the sibling `test_env_check_fails_without_lock` does.

### I-3. License-check OR-chain over-broad
- **File:** `tests/absolute_coverage.rs::test_fleet_status_requires_license` (~line 168): accepts any error containing "license", "feature", or "tier"; an unrelated licensing crash mentioning "feature" passes.
- **Fix:** Pin to the exact expected message/tier error kind.

### I-4. Shared LazyLock TempDir lives for entire test process
- **File:** `tests/cross_platform_mock_tests.rs` lines 17–18. Acceptable pattern, but leftover state leaks between tests within a run unless every test resets (see M-5); note `test_state_persistence` intentionally depends on it.

### I-5. `test_concurrent_operations` spawns 10 concurrent full CLI processes
- **File:** `tests/arch_tests.rs` edge_cases (~line 600). Each `run_omg` creates temp dirs and shells out; on constrained CI this is slow/flaky-prone, though functionally isolated (per-child OMG_DATA_DIR). Consider reducing to 4 or gating behind system tests.

### I-6. `println!("✓ ...")` noise throughout aur_dependency_resolution.rs
- Pure cosmetics; assertions already communicate outcomes; output interleaves badly under parallel test threads.

### I-7. `test_alpm_double_release_protection` / `test_alpm_init_without_release` depend on alpm-rs returning Err (not panicking) for invalid state transitions
- File: `tests/alpm_transaction_e2e.rs` lines ~262–290. Fine as written; noting they encode library-behavior contracts rather than OMG behavior — if the alpm crate changes, these fail confusingly. Consider a comment labeling them as upstream-contract tests.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH | 1 |
| MEDIUM | 6 |
| LOW | 13 |
| INFO | 7 |
| **Total** | **27** |

Highest-priority actions: stop `sudo pacman -Rns` from test helpers (H-1); make `test_outdated_json_output` actually assert (M-1); fix the mock-state concurrency marking (M-5); tighten OR-chains that let negative-path regressions pass (M-6, L-10).


---

# SLICE 16

# Audit slice-16 — `tests/` files d–e (omg)

Scope: `daemon_e2e_caching.rs`, `daemon_e2e_concurrency.rs`, `daemon_e2e_ipc.rs`, `daemon_e2e_lifecycle.rs`, `daemon_integration_tests.rs`, `daemon_security_tests.rs`, `debian_cache_tests.rs`, `debian_daemon_tests.rs`, `debian_e2e_tests.rs`, `debian_ipc_tests.rs`, `debian_pure_integration.rs`, `debian_search_integration.rs`, `debian_tests.rs`, `docker_e2e.rs`, `e2e_package_operations.rs`, `e2e_runtime_management.rs`, `e2e_system_commands.rs`, `e2e_tests.rs`, `env_lockfile_integrity.rs`, `error_recovery_tests.rs`, `error_tests.rs`, `exhaustive_cli_matrix.rs` (~9,000 lines). Read-only audit; supporting evidence read from `tests/common/mod.rs` and `src/core/env/fingerprint.rs`.

## HIGH

### H-1. Broken code under `docker_tests` feature: `report_skip` used as a value-producing match arm
- File: `tests/debian_e2e_tests.rs:421-470` (tests `test_resolver_missing_package`, `test_resolver_with_dependencies`, `test_resolver_topological_sort`)
```rust
let mut resolver = match DependencyResolver::new() {
    Ok(r) => r,
    Err(e) => common::report_skip(&format!(
        "DependencyResolver unavailable in this environment: {e:#}"
    )),
};
```
- Why it is a bug: `common::report_skip` returns `()` (`tests/common/mod.rs:575-578`), but the `Ok` arm yields a `DependencyResolver`. This is a type error and the file will not compile whenever the `docker_tests` feature is enabled. The code is currently dead (feature never built), i.e., latent broken code that will detonate on first use.
- Fix: diverge, e.g. `Err(e) => { common::report_skip(...); return; }` with the resolver declared as `Option`, or make `report_skip`-style guards a macro that `return`s.

### H-2. Test-process-global env mutation without restore leaks state across tests
- Files:
  - `tests/daemon_e2e_caching.rs:28-33`
  - `tests/daemon_e2e_concurrency.rs:36-41`
  - `tests/daemon_security_tests.rs:42-43, 92-93, 157-158`
```rust
unsafe {
    std::env::set_var("OMG_DAEMON_DATA_DIR", &data_dir);
    std::env::set_var("OMG_DATA_DIR", &data_dir);
}
```
- Why it is a bug: these fixtures permanently overwrite process-global env vars pointing at a `TempDir` that is deleted when the fixture drops. Every later test in the same binary (and any lazily-initialized global that captured the path, e.g. the audit logger) observes a stale/deleted directory or the wrong data dir. The suite itself acknowledges the correct pattern — `tests/daemon_e2e_ipc.rs:34-46` uses scoped `temp_env::with_vars` precisely because "the audit logger and daemon state capture their data-dir paths during construction". The inconsistent fixtures are flaky-by-construction and rely entirely on `#[serial]` to avoid UB from concurrent `set_var`.
- Fix: use `temp_env::with_vars` scoping everywhere, matching `daemon_e2e_ipc.rs`.

### H-3. Pervasive vacuous assertions in Debian CLI E2E suite (tests can only fail on panics)
- File: `tests/debian_e2e_tests.rs:690-1039` (e.g. `test_cli_search_on_debian`, `test_cli_info_debian_package`, `test_cli_status_shows_debian_info`, `test_cli_debian_dependency_resolution`, `test_cli_debian_sources_list_parsing`, `test_cli_debian_virtual_packages`, `test_cli_debian_architecture_handling`, `test_cli_debian_multi_arch_support`, `test_cli_debian_install_with_recommends`)
```rust
let result = run_omg_cli(&["search", "curl"]);
// May or may not succeed depending on setup - just check it doesn't panic
let _ = result.success;
...
assert!(!combined.contains("panicked"), "...");
```
- Why it is a bug: ~15 tests explicitly discard `result.success` and assert only the absence of `"panicked"`. A regression that makes every command exit non-zero with garbage output still passes. This contradicts the project's own stated test philosophy (`tests/error_tests.rs` header: "vacuous-assertion finding"). Also `contains("panicked")` misses exit code 101 checks done elsewhere (`error_tests.rs::assert_no_panic`).
- Fix: pin deterministic outcomes per command against the mock Debian backend (as `exhaustive_cli_matrix.rs::debian_matrix` already does) instead of panic-grepping.

## MEDIUM

### M-1. `clear_license()` clears the wrong environment → license-isolation setup is a no-op
- Files: `tests/common/mod.rs:212-218` (helper), used at `tests/exhaustive_cli_matrix.rs:471, 487`
```rust
pub fn clear_license() {
    let data_dir = match env::var("OMG_DATA_DIR") { ... }; // test PROCESS env
    let _ = fs::remove_file(data_dir.join("license.json"));
}
```
- Why it is a bug: `run_omg*` gives each child invocation its own fresh `TempDir` for `OMG_DATA_DIR`; the parent process's `OMG_DATA_DIR` is typically unset in CI, so `clear_license()` returns early or deletes a file no child will ever read. The comment "Ensure no license for consistent test behavior" is not actually enforced; tests pass for the wrong reason today.
- Fix: pass the intended data dir explicitly, or drop the call since per-invocation isolation already guarantees no license.

### M-2. Discarded parse results = assertions that can never fail
- Files / lines:
  - `tests/e2e_system_commands.rs:114-124` (`test_config_set_and_get`) and `:614-624` (`test_workflow_config_set_and_list`): if `config set` fails, both inner blocks are skipped and the test passes — a regression that breaks `config set` entirely is invisible.
  - `tests/debian_tests.rs:155-162` (`test_explicit_packages_count`): `let _: Result<u32,_> = stdout.parse();` — parse result thrown away.
  - `tests/debian_tests.rs:530-537` (`test_outdated_json_output`): `let _: Result<serde_json::Value,_> = serde_json::from_str(...)` discarded — invalid JSON passes.
- Fix: `assert!(parsed.is_ok(), "...")` and unwrap-with-context; for config tests, require success or assert the specific failure reason.

### M-3. Socket-readiness by fixed sleep; unbounded allocation from length prefix (IPC harness)
- File: `tests/daemon_e2e_ipc.rs:71-75`
```rust
// Wait for socket to be ready
sleep(Duration::from_millis(100)).await;
```
and `tests/daemon_e2e_ipc.rs:110-116` / `:275-280`
```rust
let resp_len = u32::from_be_bytes(len_buf) as usize;
let mut resp_bytes = vec![0u8; resp_len];
```
- Why it is a bug: (a) 100 ms sleep is a race — on loaded machines `UnixStream::connect` fails before bind, making the whole IPC suite flaky; poll for socket existence instead (the lifecycle fixture does this correctly at `daemon_e2e_lifecycle.rs:66-76`). (b) `resp_len` is trusted up to 4 GiB before allocation; a buggy/malicious peer OOMs the test harness. Cap at the protocol max frame size and error out otherwise.

### M-4. Rate-limit test depends on shared global limiter state left by other tests
- File: `tests/daemon_security_tests.rs:55-73`
```rust
// Send 250 requests to ensure we hit the limit (burst is 200)
for _i in 0..250 {
    let response = handle_request(Arc::clone(&state), req.clone()).await;
    ... RATE_LIMITED ... break;
}
assert!(limit_hit, "Should have hit global rate limit");
```
- Why it is a bug: the limiter is global ("100/s burst 200"); requests counted by previously run serial tests (or wall-clock refill timing) shift how many of the 250 pings land before the limit trips. If prior tests consumed budget within the same second, fewer than needed remain... conversely if the window fully refills between iterations on a fast machine the loop can finish under budget. Either direction produces intermittent failures. Also each `DaemonState` re-init doesn't reset the global limiter, coupling this test to execution order despite `#[serial]`.
- Fix: construct the rate limiter with test-controlled limits via an injection seam, or drain/wait a full window before the loop and assert on the exact request number.

### M-5. Audit-log assertion races async writer with fixed 100 ms sleep + `unreachable!`
- File: `tests/daemon_security_tests.rs:120-145`
```rust
std::thread::sleep(std::time::Duration::from_millis(100));
if audit_file.exists() {
    ... asserts ...
} else {
    unreachable!("Audit log file not found at {audit_file:?}");
}
```
- Why it is a bug: if flushing is ever made async (the comment admits it is "currently" blocking), the file may legitimately not exist yet after 100 ms and the test panics with an unreachable rather than reporting skip/failure context. Poll-with-deadline is the robust pattern used elsewhere in this suite.
- Fix: poll up to a few seconds for the file, then assert contents.

### M-6. Graceful-shutdown test has no kill fallback; can hang the suite's assumptions and leave a live daemon until Drop
- File: `tests/daemon_e2e_lifecycle.rs:170-210` (`test_daemon_graceful_shutdown`)
- Why it is a bug: after SIGINT, the exit wait loop runs 5 s and then the test proceeds to assert socket cleanup regardless of whether the daemon actually exited. If SIGINT handling regresses, the assertion failure message misleads ("Socket should be removed…") while the real problem is "process never exited"; also the socket-cleanup deadline loop then burns another 5 s. `Drop` does SIGKILL as backstop, so no permanent leak, but the failure diagnosis and total runtime degrade badly.
- Fix: assert `daemon.child.try_wait()` shows exit before checking socket removal, with distinct messages.

### M-7. `expected_packages_url` re-implements private production logic — tautological pinning
- File: `tests/debian_e2e_tests.rs:31-42`
```rust
/// Mirror of the Packages-URL construction used by `parallel_sync` ...
fn expected_packages_url(repo: &Repository, arch: &str) -> String { ... }
```
- Why it is a bug: the "pin" compares production output against a hand-copied mirror of the same algorithm kept in the test. If `parallel_sync` changes URL layout (e.g. adds `by-hash`, component joins), the mirror stays stale and the tests keep passing while the documented format silently drifts — the exact failure mode the doc-comment claims to prevent. Also uses only `components[0]`, hiding multi-component behavior.
- Fix: hardcode expected URL strings per fixture repo (as `test_repository_urls` partially does) instead of calling a mirrored formatter.

### M-8. `strip_ansi` mishandles OSC / non-alphabetic-terminated sequences
- File: `tests/docker_e2e.rs:88-104`
```rust
if c == '\x1b' {
    for inner in chars.by_ref() {
        if inner.is_ascii_alphabetic() { break; }
    }
}
```
- Why it is a bug: OSC sequences (`ESC ]0;title BEL`) terminate on BEL `\x07` or ST, not an alphabetic char; the loop swallows all following output up to the next letter, potentially eating the very package names later `contains` assertions look for (flaky false negatives). CSI private modes ending in digits/`;`/`m` are fine, but `ESC [ ? 25 l` ends at 'l' ok; hyperlink OSC `ESC ]8;;url ESC \` would swallow through the URL. Docker output with progress bars commonly uses OSC title sets.
- Fix: handle `ESC [` … final-byte@0x40-0x7E, and `ESC ]` … BEL/ST explicitly.

### M-9. Misleadingly named "E2E" license/usage tests exercise no production flow
- File: `tests/e2e_tests.rs:186-205` (`test_offline_license_validation_with_cached_token`), `:574-594` (`test_license_activation_flow_mocked`), `:651-658` (`test_network_timeout_simulation`)
```rust
env.create_data_file("license.json", license_json)?;
assert!(env.data_file_exists("license.json"), ...);   // writes a file, asserts it exists
...
let timeout = Duration::from_millis(1);
assert!(timeout < Duration::from_secs(1), "Timeout should be configurable for testing");
```
- Why it is a bug: writing a file with `fs::write` and asserting it exists is a tautology that validates nothing about offline license validation; the "network timeout simulation" asserts `1ms < 1s`. These occupy the names of real coverage and give false confidence that activation/offline paths are tested.
- Fix: drive the real loader (`omg_lib::core::license`) against the cached file, or delete the pseudo-tests.

## LOW

### L-1. Empty test sections / dead scaffolding
- `tests/daemon_e2e_caching.rs:268-276`: headers "Test 6: Persistent Cache (Disk-backed)" and "Test 8: Cache Performance Metrics" exist with zero tests beneath them — dead comments implying coverage that doesn't exist.
- `tests/daemon_e2e_lifecycle.rs:293-295`: "Test 5: Startup Performance" header, no test.
- Fix: delete or implement.

### L-2. Brittle version-pin and stale date literals
- `tests/e2e_system_commands.rs:366`: `output.contains("omg") && output.contains("0.1")` — breaks on any version bump to 0.2/1.0; compare against `env!("CARGO_PKG_VERSION")`.
- `tests/e2e_tests.rs:162,196,799`: hardcoded `expires_at: "2025-12-31"` — now-past dates; harmless today but any future expiry-validation logic added to parsing roundtrips would behave differently than intended.
- `tests/debian_e2e_tests.rs:355-370` (`test_transaction_dry_run`): asserts raw byte counts `"5242880"` / `"20971520"` appear verbatim in human-facing output — breaks if sizes get human-formatted (which `dry_run` plausibly should).

### L-3. Fixture equality relies on accidentally pre-sorted input
- File: `tests/env_lockfile_integrity.rs:22-34` + `save_then_load_round_trips_and_recomputes_the_hash`
- `state.save()` normalizes (sorts packages, `fingerprint.rs:182`), yet the test asserts `loaded == state` where `state.packages = ["curl","git"]` happens to already be sorted. Reordering the fixture to e.g. `["git","curl"]` would fail the round-trip assert even though behavior is correct. Fix: compare against a sorted clone or sort the fixture explicitly with a comment.

### L-4. `to_str().unwrap()` on temp paths / non-UTF8 environment intolerance
- `tests/debian_daemon_tests.rs:11-12, 40-41` (`temp_dir.path().to_str().unwrap().to_string()`), similar in `exhaustive_cli_matrix.rs` install/remove cycles. Panics on systems with non-UTF8 temp dirs; use `OsStr` env values like `daemon_e2e_ipc.rs` does.

### L-5. Weak/duplicative platform-purity guard defined locally instead of in common
- `tests/debian_tests.rs:126-135`: `#[macro_export] macro_rules! require_debian_like` duplicates the `require_*` macro family already in `tests/common/mod.rs:586-660` and exports it at crate root of a test target (pollutes the crate namespace; `$crate::common::TestConfig` path dependency). Consolidate into `common`.

### L-6. Substring-based purity assertions can false-positive
- `tests/platform_semantics.rs:11-20` tokenizes correctly, but callers such as `debian_tests.rs::test_search_essential_packages` run `assert_no_arch_terms` over arbitrary package descriptions — a Debian package whose description mentions "AUR"/"pacman" (they exist) fails the test spuriously. Acceptable trade-off, worth a comment; severity low.

### L-7. `test_input_validation_audit` and friends clear alpm caches but the security suite mixes `temp_env`-less global mutation with tests that use scoped mutation (see H-2); ordering-sensitive even under `#[serial]` because `init_audit_logger()` likely caches a global path from whichever test ran first.
- Files: `tests/daemon_security_tests.rs` throughout vs `tests/daemon_integration_tests.rs:17-31` which scopes correctly.

### L-8. `test_docker_nonexistent_package` accepts `contains("error")` anywhere in output
- `tests/docker_e2e.rs:238-247`: a successful-looking listing containing the word "error" in some banner satisfies the assertion; also `_success` deliberately ignored means a crash-with-message passes. Pin the exact not-found wording instead.

### L-9. Concurrency smoke tests assert only "some succeeded"
- `tests/daemon_e2e_concurrency.rs:80-83` (`assert!(success_count > 0, ...)`) and `:245-249`: under a systemic regression (all 50 searches fail), `success_count > 0` fails correctly, but partial-failure regimes (say 49 errors) pass silently with just a `println!`. For read-only search on a healthy mock backend there is no legitimate reason for any error; assert `success_count == 50` or log-and-fail on any error.

### L-10. `daemon_e2e_lifecycle.rs` sends signals via external `kill` binary
- Lines 143-160, 176-181: spawning `/usr/bin/kill` instead of using libc `kill(2)` (or `nix::sys::signal`) introduces PATH-dependence and a PID-reuse window between `pid()` and signal delivery. Minor robustness issue in test infra.

## INFO

### I-1. `PackageCache` LRU eviction test depends on `get` refreshing recency across `sync()`
- `tests/daemon_e2e_caching.rs:225-256`: valid pin, but note `insert_arc` of 4th entry evicting `query-2` relies on `get("query-1")` updating LRU order *before* `sync()`; if `get` is peek-only this test breaks. Documented assumption only.

### I-2. `test_metrics_increment_on_requests` hardcodes delta arithmetic (+6)
- `tests/daemon_integration_tests.rs:140-172`: correct today (5 pings + final metrics request); fragile to handler-count changes but intentionally precise — fine.

### I-3. `debian_search_integration.rs` test verifies almost nothing and contains grammar/comment drift
- `tests/debian_search_integration.rs:8-23`: comment says it "primarily verifying" fallback routing but only asserts `.success()`; the daemon-routing claim is untested. Rename or add socket-level assertion.

### I-4. `debian_cache_tests.rs` lacks feature gate
- Unlike sibling debian files, `tests/debian_cache_tests.rs` compiles regardless of debian features (uses only cache+protocol types) — intentional? If PackageCache gains debian-specific gating this file breaks default builds. Minor consistency nit.

### I-5. `exhaustive_cli_matrix.rs:388-391` stray blank line between `#[serial]` and `fn test_empty_search` — style noise.

### I-6. `docker_e2e.rs::build_docker_image` writes `omg-binary` into the repo working tree and `expect`s on copy/build
- Lines 30-52: a failed `fs::copy` panics the test thread leaving `omg-binary` behind; prefer `?`-style with cleanup guarantee. Gated behind explicit env flag so impact minimal.

### I-7. `daemon_e2e_ipc.rs::handle_connection` closes connection on first deserialization error without sending an error frame
- Lines 96-113: clients get a bare EOF for malformed frames; the test at line 380 codifies "connection closed is acceptable". Consider an `INVALID_REQUEST` error response before close for client debuggability (production-parity question, flagged here because the test enshrines the current behavior).

### I-8. `e2e_tests.rs::test_sync_payload_format` rebuilds the sync payload inline instead of invoking production sync code
- Line ~430: payload shape can drift from the real `sync()` implementation unnoticed. Mirror-of-mirror risk (same class as M-7).

### I-9. `error_recovery_tests.rs:104-108` (`test_parallel_builder_self_dependency_filtered`)
- Asserts only `graph.contains_key("pkg")` — does not verify the self-edge was actually filtered (name promises more than body delivers). Add `assert!(!graph["pkg"].contains("pkg"))`.

## Summary
- CRITICAL: 0
- HIGH: 3 (H-1 compile-broken docker_tests code, H-2 global env leak, H-3 vacuous Debian CLI suite)
- MEDIUM: 9
- LOW: 10
- INFO: 9
Total: 31 findings.

The dominant theme: large portions of this test suite assert only "did not panic" or accept either outcome (`success || contains(...)`) — including suites whose own header comments claim vacuous assertions were eliminated. Secondary theme: process-global env mutation without restore in three fixtures versus the correct `temp_env` pattern used in two others.


---

# SLICE 17

# Audit slice-17 — tests/ files f–p (omg)

Scope: `tests/{failure_tests.rs, fedora_tests.rs, fuzzy_suggestion_tests.rs, install_integration.rs, install_update_comprehensive.rs, integration_suite.rs, logic_tests.rs, log*, macos_tests.rs, metrics_tests.rs, platform_semantics.rs, privilege_tests.rs, property_tests*.rs}` (plus `tests/common/mod.rs` read for helper semantics; findings limited to in-scope call sites).

## HIGH

### H-1. Vacuous "error message parsing" test asserts nothing about the product
- File: `tests/privilege_tests.rs:214–229` (`test_error_message_parsing_password_required`)
```rust
let test_cases = vec![
    "sudo: a password is required",
    ...
];
for pattern in test_cases {
    assert!(!pattern.is_empty(), "Pattern should not be empty");
}
```
- Why it is a bug: The test never invokes any omg code. It only asserts that its own hardcoded string literals are non-empty — a tautology that always passes and falsely advertises coverage of privilege error-message parsing.
- Fix: Either delete the test or feed each pattern through the real detection function (e.g., `privilege::is_password_required(msg)`) and assert the expected classification.

### H-2. `run_mock_sudo` tests only exercise bash, not omg's sudo path
- File: `tests/privilege_tests.rs:88–118` plus tests at 233–281, 424–437, 545–562, 640–668 (`test_sudo_n_flag_fallback_on_password_required`, `test_sudo_n_flag_no_tty_detection`, `test_sudo_permission_denied_detection`, `test_interactive_fallback_triggered`, `regression_exit_code_vs_string_detection`, `test_yes_flag_with_nopasswd_sudo`, `test_yes_flag_without_nopasswd_fails_clearly`)
- Excerpt:
```rust
fn run_mock_sudo(&self, args: &[&str], scenario: SudoScenario) -> TestResult {
    let script_body = match scenario { ... "echo 'sudo: a password is required' >&2; exit 1" ... };
    let output = Command::new("bash").arg("-c").arg(&script_body)...
```
- Why it is a bug: None of these tests invoke the `omg` binary or `omg_lib::core::privilege`. They run a shell script that echoes the very strings the assertions then grep for. E.g. `test_sudo_permission_denied_detection` calls `.assert_contains("permission")` against output the mock itself produced. All of them pass unconditionally regardless of regressions in the actual sudo/elevation code, giving false confidence on security-critical behavior.
- Fix: Drive the real elevation path (inject a fake `sudo` via `PATH` pointing at a scenario script) so the mock's exit status/stderr is *interpreted by* omg's privilege module, then assert on omg's behavior.

### H-3. Dead property "tests": version comparison/update-detection properties assert nothing
- File: `tests/property_tests.rs:356–407`
```rust
let _comparison = old.cmp(&new);
prop_assert!(!old.contains("panicked at") && !new.contains("panicked at"));
...
let old = parse_version(&old_version);
let new = parse_version(&new_version);
let _is_newer = new > old;
prop_assert!(!old_version.contains("panicked at") && !new_version.contains("panicked at"));
```
- Why it is a bug: These proptests never call the product. They compare locally generated Rust strings with each other and then assert those generated strings don't contain `"panicked at"` — always true. `prop_update_detection` even discards `_is_newer`. ~50 cases × 2 properties of pure wasted CI time masquerading as version-comparison coverage.
- Fix: Delete both properties or replace them with real assertions against omg's version comparison / update-detection functions.

## MEDIUM

### M-1. Fuzz input list includes NUL byte that panics the harness when fuzzing is enabled
- File: `tests/property_tests.rs:759–786` (`fuzz_random_cli_args`)
```rust
let test_args = vec![
    vec![""],
    ...
    vec!["\0"],   // <-- Command rejects NUL in args
```
with `common/mod.rs:299`: `cmd.spawn().expect("Failed to execute omg")`.
- Why it is a bug: The file itself documents ("Null byte tests removed - std::process::Command rejects null bytes") that `Command` rejects NUL arguments, yet the fuzz list contains `vec!["\0"]`. When `OMG_RUN_FUZZ_TESTS=1`, `spawn()` returns `Err(InvalidInput)` and the `.expect` panics — the fuzz test fails on its own harness, not on an omg defect.
- Fix: Remove `vec!["\0"]` from the list (or handle spawn errors gracefully).

### M-2. Regression loop iterates messages but tests the same thing four times
- File: `tests/privilege_tests.rs:571–592` (`regression_string_matching_error_detection`)
```rust
for msg in error_messages {
    let result = runner.run_mock_sudo(&["-n", "omg", "update"], SudoScenario::PasswordRequired);
    let _ = msg; // scenario output varies; the code path is what matters
    assert_eq!(result.exit_code, 1, ...);
```
- Why it is a bug: `msg` is explicitly discarded (`let _ = msg`) and every iteration runs the identical `PasswordRequired` scenario; the loop is pure duplication and the per-message intent ("exit-code based detection regardless of wording") is not actually exercised against any message variants or against omg itself. Also tautological (see H-2).
- Fix: Collapse to a single assertion or parametrize scenarios so each message maps to a distinct mock output.

### M-3. Environment-dependent permission test breaks/fails as root
- File: `tests/failure_tests.rs:63–105` (`test_permission_denied_on_cache_fails_gracefully`)
```rust
perms.set_readonly(true);
std::fs::set_permissions(&sync_dir, perms)?;
...
let result = alpm_ops::execute_transaction(vec!["pkg-a".to_string()], false, false, None);
assert!(result.is_err(), "Transaction should fail due to permissions");
```
- Why it is a bug: On Unix `set_readonly(true)` clears write bits; when the test runs as root (common in container CI), root bypasses DAC permissions, alpm opens the DB successfully, `execute_transaction` succeeds, and the assertion fails — a spurious red build. There is no `if is_root() { skip }` guard unlike other suites in this repo.
- Fix: Skip when `omg_lib::core::is_root()` (as `fedora_tests.rs` does), or use a user-namespace/unprivileged-user mechanism.

### M-4. Conflict-test error assertion accepts almost any error
- File: `tests/failure_tests.rs:52–60`
```rust
assert!(
    err.contains("conflicting packages")
        || err.contains("Transaction failed")
        || err.contains("pkg-a"),
```
- Why it is a bug: `err.contains("pkg-a")` matches any error that merely echoes the requested package name (e.g. "target pkg-a not found", a sync failure naming pkg-a). The test can pass even if conflict detection is entirely broken, defeating its purpose.
- Fix: Assert specifically on the conflict/preparation-error variant, drop the `pkg-a` alternative.

### M-5. macOS invalid-formula install test performs a real network operation without gating
- File: `tests/macos_tests.rs:154–168` (`test_install_invalid_formula`)
```rust
#[tokio::test]
async fn test_install_invalid_formula() {
    let pm = HomebrewPackageManager::new();
    let result = pm.install(&["nonexistent-formula-xyz-12345".to_string()]).await;
```
- Why it is a bug: Every other network-touching test in this file is `#[ignore]`d, but this one runs live `brew install` on every macOS test run: requires network, may be slow/hang, may mutate Homebrew cache state, and fails the suite on offline machines.
- Fix: Add `#[ignore = "requires network access to Homebrew API"]` like its siblings.

### M-6. Fedora tests inconsistently gated — ungated tests hit the real system/network
- File: `tests/fedora_tests.rs:47–58` (`test_search_nonexistent_package`), `70–77` (`test_list_updates`), `166–182` (`test_install_invalid_package`)
- Why it is a bug: `test_search_nonexistent_package` and `test_list_updates` run real `dnf` queries (network + system dnf metadata) with no `require_system_tests!()` while sibling tests are gated; `test_install_invalid_package` gates only on `is_root()`. On non-Fedora machines or CI without network these either fail spuriously or exercise nothing.
- Fix: Apply the same `require_system_tests!()` gate used by the neighboring tests.

### M-7. Security-policy test writes a policy file and verifies nothing
- File: `tests/integration_suite.rs:652–673` (`test_security_policy_file_loading`)
```rust
// Run a command that would load policy
// The actual policy enforcement is tested in unit tests
```
- Why it is a bug: The test creates `policy.toml` in a temp config dir, then ends without running any command. Nothing is loaded, nothing asserted. It passes unconditionally and falsely appears in reports as policy-loading coverage.
- Fix: Run an omg command against the temp config dir and assert the policy took effect (e.g. banned package rejected), or delete the test.

### M-8. Conditional/vacuous-pass assertions in privilege UX tests
- File: `tests/privilege_tests.rs:341–361` (`test_update_suggests_sudo_without_root`), `389–406` (`test_dev_build_detection_blocks_elevation`)
```rust
if !result.success {
    assert!(combined.contains("sudo") || ...);
}
...
if combined.contains("Privilege elevation") || combined.contains("development builds") {
    assert!(...);
}
```
- Why it is a bug: Both tests wrap their entire body in conditionals that depend on the outcome being verified. If `update --yes` happens to succeed, or the dev-build banner text changes, zero assertions run and the test silently passes. This is exactly the anti-pattern other tests in this repo were rewritten to eliminate ("FALSIFIABLE" comments elsewhere).
- Fix: Pin deterministic preconditions (force failure mode) and assert unconditionally.

### M-9. `known_system_package()` mismatch with hardcoded "pacman" assertions under debian features
- File: `tests/integration_suite.rs:64–80` vs `240–256` (`test_info_official_package`), also `1315–1325` (`test_info_shows_package_details`)
```rust
const fn known_system_package() -> &'static str { "apt" }   // debian cfg
...
assert!(result.stdout.contains("pacman"), "Should show package name");
```
- Why it is a bug: When compiled with `debian`/`debian-pure` (no arch feature), the helper correctly returns `"apt"` but the assertion still demands the literal string `"pacman"` in output — guaranteed failure the moment the `#[ignore]` is lifted on a Debian machine. Same hardcoded-"pacman" pattern repeats in `pacman_database::test_info_shows_package_details`.
- Fix: Assert `result.stdout.contains(known_system_package())`.

### M-10. `scenario_switching_projects` OR-assertion lets single-project detection pass
- File: `tests/integration_suite.rs:1105–1130`
```rust
assert!(
    result1.stdout.contains("18") || result2.stdout.contains("20"),
    "Should detect different versions per project"
);
```
- Why it is a bug: The disjunction passes if *either* project's version is detected — e.g. project-1 resolution completely broken (never finds "18") still yields green as long as project 2 works. The stated goal ("different versions per project") is not pinned.
- Fix: Assert each result independently (`result1 → 18`, `result2 → 20`).

### M-11. `CommandNotFound` scenario reports the wrong command name
- File: `tests/privilege_tests.rs:104–107`
```rust
SudoScenario::CommandNotFound => {
    let cmd = args.get(1).unwrap_or(&"command");
    format!("echo 'sudo: {cmd}: command not found' >&2; exit 1")
}
```
- Why it is a bug: For `run_mock_sudo(&["nonexistent-command"], CommandNotFound)` there is no index 1, so the fallback emits `command: command not found` instead of echoing the actual missing command. The test (`test_sudo_command_not_found`) still passes only because its grep is loose; the scenario doesn't model what it claims (the named command being absent).
- Fix: Use `args.first().unwrap_or(&"command")` so index 0 (the invoked command) is reported.

## LOW

### L-1. Silent early-return skips metrics test without accounting
- File: `tests/metrics_tests.rs:44–46` and `84–86`
```rust
let Some(state) = state else { return };
```
- Why it is a bug: If `DaemonState::new()` fails, the test returns Ok — recorded as a *pass* with no `[omg-skip]` line. The suite built `report_skip()` precisely so CI can detect silent coverage loss (`grep -c '[omg-skip]'`); these two tests bypass that contract (unlike `fuzzy_suggestion_tests.rs`, which uses it).
- Fix: Call `common::report_skip(...)` before returning (and import the common module).

### L-2. `unreachable!` on legitimate daemon error responses
- File: `tests/metrics_tests.rs:96–106`; same pattern in `tests/fuzzy_suggestion_tests.rs:82–99`
```rust
} else {
    unreachable!("Expected Metrics response");
};
```
- Why it is a bug: A valid `Response::Error` (daemon degraded, validation change, tier restriction) turns into a panic with no diagnostic context about which response actually arrived, producing confusing 101-exit failures instead of an assertion message showing the response.
- Fix: Match, capture the unexpected variant, and `panic!("expected Metrics response, got: {response:?}")`.

### L-3. macOS `is_installed("homebrew")` checks a nonexistent formula and discards the result
- File: `tests/macos_tests.rs:100–108` (`test_is_installed_check`)
```rust
let _is_brew_installed = pm.is_installed("homebrew").await.unwrap();
Ok(())
```
- Why it is a bug: "homebrew" is not a formula/cask; the query is meaningless and the binding is immediately discarded, so the test verifies only that the command doesn't error. Its name promises an installed-check assertion it never makes.
- Fix: Check a formula that exists (e.g. `brew`'s own formula name on the host) or rename/re-scope the test.

### L-4. `test_formula_info` passes vacuously when info returns `None`
- File: `tests/macos_tests.rs:76–93`
```rust
let info = pm.info("wget").await?;
if let Some(pkg) = info { ... }
```
- Why it is a bug: If `info` silently returns `None` for an existing formula (a lookup regression), the test still passes. Only the happy path is verified.
- Fix: `let Some(pkg) = info else { panic!("wget should resolve") };`.

### L-5. Global yes-flag mutation without `#[serial]`
- File: `tests/privilege_tests.rs:618–632` (`test_yes_flag_prevents_password_prompt`), `670–683` (`test_yes_flag_prevents_fallback_to_interactive`)
```rust
privilege::set_yes_flag(true);
assert!(privilege::get_yes_flag());
privilege::set_yes_flag(false);
```
- Why it is a bug: `privilege` presumably stores the flag in process-global state; these tests mutate it without `serial_test`, so parallel Rust tests in the same binary observe flipped values mid-run (other tests in the same file spawn subprocesses, mitigating but not eliminating the hazard). Also leaves global state mutated between set/assert pairs if an assert fails.
- Fix: Mark `#[serial]` and/or restore via a guard.

### L-6. Weak special-character coverage in injection test
- File: `tests/privilege_tests.rs:464–487` (`test_special_chars_in_package_names`)
```rust
let special_names = ["test-package", "test_package", "test.package", "test123", "TEST123"];
...
assert!(result.exit_code >= 0, ...);
```
- Why it is a bug: Despite the comment "Test that special characters are handled safely," none of the inputs contain shell metacharacters (`; | $ \` etc.) that would distinguish safe argv handling from injection. Additionally `exit_code >= 0` is true for every normally-exiting process (only signal kills yield −1), so the crash check is nearly vacuous; the `root:` leak check duplicates property_tests.
- Fix: Include metachar payloads and assert on explicit success/error codes.

### L-7. Destructive cleanup force-removes with `-Rdd`
- File: `tests/install_update_comprehensive.rs:494–500` (`cleanup_package`)
```rust
let _ = Command::new("sudo").args(["pacman", "-Rdd", "--noconfirm", pkg]).output();
```
- Why it is a bug: `-Rdd` skips dependency checking, which can break previously-installed dependents of `cowsay` on the developer's real system; errors are discarded (`let _`), so failed cleanup silently leaves the test package installed.
- Fix: Use plain `-R --noconfirm` (cowsay has no reverse deps) and surface cleanup failures.

### L-8. Injection/traversal security tests only assert "no panic"
- File: `tests/install_update_comprehensive.rs:551–563` (`test_command_injection_prevented`, `test_path_traversal_prevented`)
```rust
let result = run_omg(&["install", "pkg; rm -rf /"]);
result.assert_no_panic();
```
- Why it is a bug: A vulnerable implementation that *executed* the payload could still avoid panicking; the test names claim prevention but verify only crash-freedom. No assertion that no shell spawned, no file was touched, and a proper "not found/validation" error was returned.
- Fix: Assert structured rejection output (like `install_integration.rs` pins "not found") and absence of side effects.

### L-9. `env share` weak disjunction allows token-less success to pass
- File: `tests/integration_suite.rs:611–621` (`test_env_share_without_token`)
```rust
assert!(
    !result.success
        || result.stderr.contains("GITHUB_TOKEN")
        || result.stderr.contains("token"),
    "Should require GITHUB_TOKEN"
);
```
- Why it is a bug: The `||` chain means a successful share *without* any token (e.g. anonymous gist support added later, or token read from another source) still passes, since `!result.success` short-circuits the requirement. The test cannot fail on the exact regression it guards against unless the command happens to also fail.
- Fix: Assert `result.stderr` mentions the token requirement, and separately assert failure when truly no credential source exists.

### L-10. Module-wide `#![expect(unused_variables)]` hides dead code
- File: `tests/integration_suite.rs:26`
```rust
#![expect(unused_variables)]
```
- Why it is a bug: Suppresses unused-variable diagnostics across all 1,600 lines, letting genuinely dead bindings accumulate anywhere in the suite (e.g. the repeatedly rebound unused `result` in `scenario_security_audit_workflow`). Other test files scope lint exceptions narrowly.
- Fix: Remove the blanket expect; fix or underscore-name the offending variables.

### L-11. `prop_runtime_normalization` never checks normalization consistency
- File: `tests/property_tests.rs:222–236`
```rust
/// Runtime names should be normalized consistently
...
let result1 = run_omg(&["which", runtime]);
prop_assert!(!result1.stderr.contains("panicked at"));
```
- Why it is a bug: Doc comment promises consistency across aliases ("nodejs"/"node", etc.), but only crash-freedom is asserted; outputs of alias variants are never compared, so a regression making `which NODE` behave differently than `which node` passes.
- Fix: Group outputs by canonical runtime and assert identical exit codes/output.

### L-12. Env-expansion property silently verifies nothing when variable unset
- File: `tests/property_tests.rs:239–249`
```rust
if let Ok(val) = std::env::var(&var_name) {
    prop_assert!(!result.stdout.contains(&val));
}
```
- Why it is a bug: Generated var names (`[A-Z]{3,10}`) are usually unset in CI, so the entire check is skipped for most cases; additionally, if the value is the empty string, `stdout.contains("")` is trivially true and the property fails spuriously.
- Fix: Set the variable explicitly via the runner env (e.g. `var=canary-token`) before invoking, and skip empty values.

### L-13. Semver success-branch disjunction weakens the assertion
- File: `tests/property_tests.rs:332–341`
```rust
prop_assert!(
    result.stdout.contains(&version) || result.stderr.is_empty(),
    ...
```
- Why it is a bug: Any success where stderr happens to be empty passes without mentioning the installed/requested version, so the "success should mention version" contract isn't enforced whenever stderr is clean.
- Fix: Assert stdout contains the version (or a defined success marker) unconditionally on success.

## INFO

### I-1. Deliberately dead seeding helper awaiting upstream fix
- File: `tests/install_integration.rs:25–33`
```rust
#[allow(dead_code)]
fn seed_installed(...)
```
- Documented staging for withheld success-path pins (HANDOFF note, lines 35–46). Not a defect per se, but the file currently provides only a single negative test while its doc comment advertises install integration coverage; track until the upstream install-path defect settles and re-enable the four pinned tests.

### I-2. Whitelist test excludes elevated internal entrypoints from coverage
- File: `tests/privilege_tests.rs:151–164` (`test_whitelist_allowed_operations`)
- Comment acknowledges `upgrade`/`fullupdate`/`turboupdate` are internal elevated entrypoints deliberately excluded from clap-level testing, so the whitelist itself is never directly exercised end-to-end here. Coverage gap worth noting for the privilege audit.

### I-3. `parse_version` masks malformed components with 0
- File: `tests/property_tests.rs:409–424`
```rust
.map(|s| s.parse().unwrap_or(0))
```
- Test-local helper silently coerces unparsable parts (including negative numbers from the boundary-version list) to 0, making comparisons semantically wrong ("1.-1.0" == "1.1.0"-ish). Currently harmless because its only consumers are the vacuous properties in H-3, but becomes a bug if reused.

### I-4. `logic_tests.rs` env-lock discipline is sound but fragile
- File: `tests/logic_tests.rs:15–17, 22–27`
- Uses a static `Mutex` plus `init_test_env()` once-mutation documented in `common/mod.rs`. Correct today, but any future test touching env vars without the lock reintroduces a race; consider standardizing on `#[serial]` like the rest of the suite.

### I-5. Duplicate test bodies across files
- Files: `tests/integration_suite.rs:1585–1600` (`test_invalid_lock_file_error` / error_handling::`test_invalid_lock_file`) and multiple near-identical CLI smoke tests duplicated between `integration_suite.rs`, `install_update_comprehensive.rs`, and `privilege_tests.rs`.
- Maintenance smell: contract changes must be updated in several places; consolidate shared cases into `tests/common`.

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 3 |
| MEDIUM | 11 |
| LOW | 13 |
| INFO | 5 |

Total: 32 findings. Dominant themes: (1) tautological privilege/"sudo" tests that test their own mocks rather than omg (security-critical blind spot), (2) vacuous/dead property tests inflating coverage numbers, (3) conditional assertion bodies that pass when the behavior under test regresses, (4) inconsistent environment/root/network gating causing spurious failures or silent skips.


---

# SLICE 18

# Audit slice-18 — tests/ remainder (real_world, safe_ops, security_*, tdd_edge_cases, telemetry_*, team_dashboard, ubuntu_*, update_*), tests/common/, tests/integration/

Read-only audit. All line numbers refer to current working tree of `~/Documents/omg`.

---

## HIGH

### H-1. `OMG_TEST_MODE` permanently removed process-wide after one test
- **File:** `tests/security_privilege_escalation_tests.rs:33-77` (`test_elevation_whitelist_allowed_operations`)
- **Excerpt:**
  ```rust
  unsafe { std::env::set_var("OMG_TEST_MODE", "1"); }
  ...
  unsafe { std::env::remove_var("OMG_TEST_MODE"); }
  ```
- **Why it is a bug:** The suite's `common::init_test_env()` sets `OMG_TEST_MODE=1` exactly once via `Once::call_once` (tests/common/mod.rs:36-55) and documents that invariant. This test *removes* the variable at the end and there is no way to restore it — `INIT` has already fired, so any later `init_test_env()` call is a no-op. Every other in-process unit/integration test in the same binary (or same test process when the harness merges binaries) that relies on `OMG_TEST_MODE=1` (e.g. `elevate_for_operation`, telemetry suppression paths) runs with the variable unset after this test executes. Test ordering under the default parallel harness makes this a nondeterministic cross-test contamination bug.
- **Fix:** Use `temp_env::with_vars` / `common::with_test_env` inside a `#[serial]` scope instead of raw `set_var`/`remove_var`, so the value is restored.

### H-2. Global yes-flag mutated by non-serial tests
- **File:** `tests/security_privilege_escalation_tests.rs:88-106` (`test_yes_flag_global_state`, `test_yes_flag_non_interactive_mode`)
- **Excerpt:** `set_yes_flag(true); assert!(get_yes_flag()); ... set_yes_flag(false);`
- **Why it is a bug:** `get/set_yes_flag` operate on a process-global static; these two tests are *not* `#[serial]` (unlike their neighbors in `privilege_escalation`). Any other test that reads the flag concurrently observes flipped state, and the tests race each other. They also silently leave global state (`false`) as a side effect for subsequent tests.
- **Fix:** Mark both `#[serial]`, or better, make the flag injectable/scoped in production code instead of a global.

---

## MEDIUM

### M-1. SUID/SGID sweep uses a fake, non-recursive `walkdir`
- **File:** `tests/security_tests.rs:831-874` (module `walkdir`), used at :806-825 (`test_no_suid_creation`)
- **Excerpt:** `// Add walkdir for the SUID test` + a hand-rolled `WalkDir` whose `IntoIterator` only does `std::fs::read_dir(&self.path)` — no recursion into subdirectories.
- **Why it is a bug:** `test_no_suid_creation` claims to verify "no SUID files were created" under the project tree but only inspects top-level entries; anything omg writes into a subdirectory (`config/…`, `.omg/…`, cache dirs) is never checked. A local shim also shadows any real `walkdir` dependency, hiding the limitation from readers.
- **Fix:** Recurse (`std::fs::read_dir` stack loop) or add the real `walkdir` dev-dependency and delete the shim.

### M-2. Tar-traversal test passes on both vulnerable and safe outcomes
- **File:** `tests/security_audit_tests.rs:17-83` (`test_tar_path_traversal_rejection`)
- **Excerpt:**
  ```rust
  if let Err(e) = &result { ... } else {
      println!("Warning: Extraction succeeded, likely due to ... sanitization.");
  }
  ```
- **Why it is a bug:** On the success path the test only prints a warning and returns `Ok`; on the error path the accepted message list includes the extremely generic keyword `"archive"`, which almost any tar error will contain. The test therefore cannot fail for either a real traversal hole or an unrelated error — it is effectively a smoke test masquerading as a security regression test.
- **Fix:** Pin one deterministic outcome: assert extraction rejects the archive with a specific traversal error, and additionally assert `../evil.txt` was not written anywhere (check parent dir of the temp dir).

### M-3. `packages_url_for` re-implements production logic in the test
- **File:** `tests/ubuntu_compatibility_tests.rs:12-30`
- **Excerpt:** `/// Mirror of the Packages-URL construction used by \`parallel_sync\` so the documented URL format stays pinned even though the helper itself is private to production code.`
- **Why it is a bug:** The URL builder is duplicated rather than exercised; if production changes its layout (component iteration, arch handling), these "Ubuntu Packages URL" tests keep passing against the stale mirror. The mirror itself also diverges from real behavior: it only uses `components[0]` (production presumably emits one URL per component) and fabricates a non-existent `{base}/Packages` layout for component-less repos.
- **Fix:** Expose the production helper behind `#[doc(hidden)] pub` or a `pub(crate)` + test feature, delete the mirror, and add a multi-component case.

### M-4. Telemetry "queue persistence"/"permissions" tests only exercise the test fixture
- **File:** `tests/telemetry_tests.rs:150-246` (`test_queue_persistence_across_restart`, `test_queue_path_location`, `test_corrupted_queue_recovery`, `test_empty_queue`, `test_session_persistence`, `test_queue_file_permissions`, `test_session_file_permissions`)
- **Excerpt:** e.g. `fixture.write_queue(&events)?; let loaded = fixture.read_queue()?; assert_eq!(loaded.len(), 2);`
- **Why it is a bug:** These round-trip the fixture's own `serde_json` write/read helpers against files the fixture itself wrote; no production queue/session code is invoked. Names claim restart-recovery, corruption recovery, and permission properties of the real telemetry client that are never tested — green CI here provides false confidence about actual crash-safety of the telemetry queue.
- **Fix:** Drive the real `telemetry_client` enqueue/persist path against `data_dir`, or rename the tests to reflect that they only cover fixture serialization.

### M-5. Mock package manager allows duplicate installs, corrupting counts
- **File:** `tests/common/mocks.rs:56-70` (`MockPackageDb::install`) and `:118-133` (`get_status`)
- **Excerpt:** `self.installed.lock().unwrap().push(name.to_owned());` with no dedup check.
- **Why it is a bug:** Installing the same package twice pushes the name twice; `list_explicit`, `explicit` count in `get_status`, and `remove` (removes only first occurrence) then disagree with reality. Any test that installs twice (or asserts explicit==unique installed) gets silently wrong numbers, weakening the mock as a oracle.
- **Fix:** In `install`, return early `Ok(())` if `is_installed(name)`; consider `Vec` → deduped collection.

### M-6. `test_output_does_not_leak_paths` always fails when `HOME` is unset
- **File:** `tests/update_integration.rs:432-443`
- **Excerpt:**
  ```rust
  !combined.contains(env::var("HOME").unwrap_or_default().as_str()),
  ```
- **Why it is a bug:** If `HOME` is unset (containers, systemd-run environments, CI), `unwrap_or_default()` yields `""`, and `combined.contains("")` is always `true`, so the assertion `!true == false` fails unconditionally — an environment-dependent false failure. Separately, the blanket `/home/` ban can produce false positives if omg legitimately prints a path under `/home` (e.g. user-supplied project dir).
- **Fix:** Skip the HOME clause when the variable is absent (`env::var("HOME").ok().map(...)`), and restrict the `/home/` check to the invoking user's home.

### M-7. Env-mutating privilege test races with parallel suite despite `init_test_env` contract
- **File:** `tests/security_privilege_escalation_tests.rs:37-41` combined with `tests/common/mod.rs:29-55`
- **Excerpt:** `#[serial] #[expect(unsafe_code)] fn test_elevation_whitelist_allowed_operations() { unsafe { std::env::set_var(...) } }`
- **Why it is a bug:** Even with `#[serial]`, `serial_test` only serializes tests annotated `#[serial]`; plain `run_omg`-style tests spawn children concurrently and other in-process threads read env. Mutating the process environment via `unsafe set_var` is UB-adjacent in Rust 2024 edition semantics and contradicts the suite's own documented rule ("Tests that need to modify environment variables should use `with_test_env`"). Overlaps H-1 but stands alone as a standards violation.
- **Fix:** Same as H-1 — route through `with_test_env`.

---

## LOW

### L-1. Weak/vacuous injection assertions ("absence of `root:`")
- **Files:** `tests/security_tests.rs:40-120` (`test_command_injection_semicolon`, `_pipe`, `_backtick`, `_dollar`, `test_path_traversal_basic`, `test_path_traversal_encoded`)
- **Excerpt:** `assert!(!result.stdout.contains("root:"), "Command injection via: {payload}");`
- **Why it is a bug:** The tests pass whenever the substring `root:` is absent — including when omg ignores the payload entirely, errors out for unrelated reasons, or leaks the file content in another format (e.g. without `root:` line prefix). No positive control (a known-injectable channel producing output) exists, so these cannot detect a partial injection. Note stderr is not even checked in several variants (`test_command_injection_semicolon` checks stdout only).
- **Fix:** Assert command failure plus the specific validation error message (like `test_sql_injection_patterns` does at :122-136), and check `combined_output()`.

### L-2. `test_no_unnecessary_root` has inverted-looking weak logic
- **File:** `tests/security_tests.rs:751-769`
- **Excerpt:** `assert!(result.success || !result.stderr.contains("root"), ...)`
- **Why it is a bug:** A legitimately failing command whose stderr merely mentions "root" anywhere (e.g. help text, unrelated error containing the word) fails the test; conversely a command that actually demands root but words it differently passes. The assertion measures coincidence of wording, not privilege behavior.
- **Fix:** Assert exit status expectations per command explicitly, or drop the stderr keyword heuristic.

### L-3. Tautological property tests
- **File:** `tests/team_dashboard_tests.rs:904-927` (`test_team_id_validation`, `test_member_name_reasonable_length`)
- **Excerpt:** generates `team_id in "[a-zA-Z0-9/_-]{1,100}"` then asserts every char matches `[a-zA-Z0-9/_-]`.
- **Why it is a bug:** Both proptests re-state the generator's own regex over its own output. They never touch any production validation function, so they can never catch a regression — pure coverage theater.
- **Fix:** Point them at a real `TeamConfig`/team-id validator, or delete them.

### L-4. Getter "tests" assert nothing
- **File:** `tests/team_dashboard_tests.rs:764-787` (`app_getter_tests`: `test_get_total_packages`, `test_get_orphan_packages`, `test_get_updates_available`, `test_get_security_vulnerabilities`, `test_get_runtime_versions`)
- **Excerpt:** `let _total = app.get_total_packages();`
- **Why it is a bug:** Results are bound to `_` and discarded; the only failure mode is a panic. These provide near-zero regression protection while costing runtime.
- **Fix:** Assert concrete invariants against a seeded `App` (e.g. after inserting N packages, total == N).

### L-5. `test_app_with_team_workspace` tests nothing about team integration
- **File:** `tests/team_dashboard_tests.rs:934-957`
- **Excerpt:** comment: *"Note: The app will try to load from current_dir, not our temp_dir. This test demonstrates the integration pattern"* — then asserts only `app.current_tab == Tab::Dashboard`.
- **Why it is a bug:** The workspace is initialized in a temp dir that `App::new()` never reads; the test admits it exercises no integration. Misleading name; dead test weight.
- **Fix:** Either chdir/scope the app's workspace root to the temp dir and assert loaded members, or remove.

### L-6. Timing-bound timer assertions are flake-prone
- **File:** `tests/telemetry_e2e_test.rs:262-285` (`test_timer_basic_usage`: upper bound `< 200ms` after 50 ms sleep; `test_timer_accuracy`: `< 150ms` after 100 ms sleep)
- **Why it is a bug:** Upper bounds of 4×/1.5× the sleep duration fail on loaded/oversubscribed CI runners where tokio sleep wakeup overshoots. Classic flaky-test source.
- **Fix:** Drop the tight upper bounds or use a generous ceiling (e.g. `< 5s`) that still catches a broken monotonic clock.

### L-7. Dead assignment / abandoned assertion in secret-scanner test
- **File:** `tests/security_privilege_escalation_tests.rs:614-628` (`test_secret_scanner_detects_leaks`)
- **Excerpt:** `let _findings = scanner.scan_content(content, "test.env"); // Note: May be filtered as placeholder...`
- **Why it is a bug:** The AWS-key scan result is computed and discarded with an apologetic comment — the AWS detection path is never actually asserted here (only private keys are). Combined with `security_tests.rs::secrets_detection::test_detect_aws_keys`, coverage intent is split and the discarded half rots.
- **Fix:** Use a realistic AWS key literal that isn't placeholder-filtered and assert `SecretType::AwsAccessKey` here too.

### L-8. `is_valid_package_name` helper tests itself, not production
- **File:** `tests/security_audit_tests.rs:86-125`
- **Excerpt:** comment: *"Helper to validate package names (should be implemented in core)"* — the test validates this local closure.
- **Why it is a bug:** `test_package_name_sanitization` proves only that the test's own regex rejects payloads; production `validation::validate_package_name` is never invoked in this binary. Also note the helper forbids leading `.`/`-` whereas production accepts scoped names like `@angular/cli` (see `security_privilege_escalation_tests.rs:158`), i.e. the two validators disagree — evidence the local helper models nothing real.
- **Fix:** Delete the helper and call `omg_lib::core::security::validation::validate_package_name`.

### L-9. Shell interpolation in test-only helper
- **File:** `tests/common/mod.rs:393-401` (`run_shell` usage in `is_package_installed`)
- **Excerpt:** `run_shell(&format!("pacman -Q {name} 2>/dev/null"))`
- **Why it is a bug:** Package name is interpolated into an `sh -c` string. Callers today pass literals, but any future test feeding a fixtures::INJECTION_ATTEMPTS string would execute attacker-chosen shell on the host. Test-scope only, hence LOW.
- **Fix:** Use `Command::new("pacman").arg("-Q").arg(name)` with `Stdio::null()`.

### L-10. Timeout kill leaves grandchildren running
- **File:** `tests/common/mod.rs:238-260` (`run_omg_with_options` timeout branch)
- **Excerpt:** `let _ = child.kill(); break;` — only the direct child is killed; no process-group handling.
- **Why it is a bug:** If `omg` spawned elevating children (sudo) or daemons, they survive the kill and may hold locks/write to the already-dropped TempDirs' paths, causing flaky follow-up failures. Also the killed child yields exit code -1 via `code().unwrap_or(-1)` indistinguishable from a signal in reporting.
- **Fix:** Spawn the child in its own process group (`process_group(0)` via std `CommandExt`) and kill the group on timeout.

### L-11. `assert_package_info` version-token heuristic accepts prose
- **File:** `tests/common/assertions.rs:20-35`
- **Excerpt:** `digits >= 2 && token.contains('.')`
- **Why it is a bug:** Tokens like `"v2.foo"`, `"3.5million"`, timestamps (`"12:00.5"`), or section numbers satisfy the heuristic. It is stricter than the old `contains('.')` but still not a version check; a regression printing garbage-with-two-digits passes.
- **Fix:** Anchor to a regex like `^\d+(\.\d+)+$`.

### L-12. Environment-dependent default-config assertion
- **File:** `tests/team_dashboard_tests.rs:851-860` (`test_team_config_defaults`)
- **Excerpt:** `assert!(!config.member_id.is_empty()); // Should be populated from whoami`
- **Why it is a bug:** `TeamConfig::default()` deriving `member_id` from the host `whoami` makes the test fail on systems lacking `whoami` or returning empty (minimal containers); it also makes the "default" value machine-dependent, so serialization comparisons across hosts are unstable.
- **Fix:** Inject the username; test fallback behavior explicitly rather than assuming the host provides one.

### L-13. Real-world PGP test panics on malformed cache entries
- **File:** `tests/integration/security_real_world.rs:226-233` (`test_pgp_verification_real_packages`)
- **Excerpt:** `for entry in entries.take(50) { let entry = entry.unwrap(); ... }`
- **Why it is a bug:** An unreadable `/var/cache/pacman/pkg` entry (permission/race during pacman operation) turns a "skip gracefully" test into a panic. Also `PgpVerifier::new().expect("system keyring must load on Arch")` hard-fails instead of skipping on machines without a usable keyring, contradicting the doc comment "Skips gracefully".
- **Fix:** `flatten()` the iterator and downgrade keyring load failure to a reported skip (`report_skip`).

### L-14. Cache-effectiveness timing ratio assertion inherently flaky
- **File:** `tests/integration/security_real_world.rs:300-330` (`test_vulnerability_cache_effectiveness`, `#[ignore]`d)
- **Excerpt:** `assert!(second_duration < first_duration / 10, ...)`
- **Why it is a bug:** A 10× wall-clock speedup requirement fails whenever the first network call happens to be fast (warm connection) or scheduler jitter inflates measurement; also division-by-zero risk handled only via `.max(1)` micros in the message, not in the comparison (sub-microsecond durations compare as equal → fails).
- **Fix:** Compare against a fixed floor (cached hit < few ms) or instrument cache-hit counters instead of timing.

### L-15. `parse_version_or_zero("   ")` pins whitespace as a valid version
- **File:** `tests/tdd_edge_cases.rs:16-17`
- **Excerpt:** `assert_eq!(parse_version_or_zero(""), "");` and `assert_eq!(parse_version_or_zero("   "), "   ");`
- **Why it is a bug:** The test enshrines that unparsed garbage (empty, whitespace-only, 26-digit overflow strings, `v`-prefixed) round-trips unchanged rather than being rejected/normalized. Downstream comparisons (update checks in `UpdateType::from_versions`, etc.) comparing such strings can misclassify. At minimum the whitespace case deserves a trim-or-error contract decision; pinning `"   "` as a version is almost certainly unintended.
- **Fix:** Decide and enforce: reject non-parseable versions at the boundary or normalize (trim, strip `v`).

### L-16. `test_sensitive_file_protection` case-sensitive secret match
- **File:** `tests/security_tests.rs:186-199`
- **Excerpt:** `assert!(!result.stdout.contains("secret"), "Leaked secrets.toml content");`
- **Why it is a bug:** Only lowercase `secret` is banned; output like `Secret: …` or `SECRET_KEY` echoes pass. Similarly only exact `abc123` for the .env value. Weak leak detection.
- **Fix:** Case-insensitive contains over `combined_output()` for both marker values.

### L-17. `test_output_is_utf8` asserts nothing meaningful
- **File:** `tests/update_integration.rs:421-428`
- **Excerpt:** comment *"String is always valid UTF-8 in Rust"*; asserts stdout or stderr non-empty.
- **Why it is a bug:** Self-admitted no-op relative to its name; the non-empty half duplicates `assert_runs_without_panic`. Misleading test name.
- **Fix:** Rename to `test_produces_output` or actually validate lossless byte-level UTF-8 via `String::from_utf8` on raw bytes (would require exposing bytes through `CommandResult`).

---

## INFO

### I-1. Vacuous `if !result.success` guards throughout update_integration
- **File:** `tests/update_integration.rs:100-115, 128-142, 152-166, 216-232, 245-258, 495-512` etc.
- **Why:** Many assertions run only when the command fails; when it succeeds (rooted CI, permissive sandbox) the entire body is skipped and the test passes having verified nothing about error UX. Acceptable as graceful-degradation design, but worth noting coverage evaporates in privileged environments.
- **Fix:** Where possible force the failure mode (env knob, non-root user namespace) instead of conditionally asserting.

### I-2. `test_privilege_bypass_attempts` never calls elevation
- **File:** `tests/security_privilege_escalation_tests.rs:583-597`
- **Why:** Despite the name, the test only re-runs `validate_package_name` on args; `elevate_for_operation` with malicious args (the actual bypass surface) is never exercised — unlike `test_elevation_whitelist_blocks_dangerous` which does. Redundant/mislabeled.
- **Fix:** Call `elevate_for_operation("install", &args)` and expect whitelist/validation rejection.

### I-3. `test_command_uses_args_not_shell` constructs commands and discards them
- **File:** `tests/security_audit_tests.rs:110-124`
- **Why:** Builds a safe `Command`, comments about the unsafe alternative, asserts nothing. Documentation-as-test.
- **Fix:** Convert to a grep-style source assertion (like `network_security::test_runtime_and_http_clients_are_https` in security_tests.rs) or delete.

### I-4. `real_world_integration.rs` is an empty shell
- **File:** `tests/real_world_integration.rs:1-7`
- **Why:** Contains only `mod integration;` and all contained tests are `#[ignore]`. Not a bug per se, but the binary compiles to nothing runnable by default; ensure CI has an explicit `-- --ignored` job or the "real-world" suite is dead weight.

### I-5. Duplicate test names across binaries
- **Files:** `tests/telemetry_e2e_test.rs:38` vs `tests/telemetry_tests.rs:203` (both `test_command_event_serialization`), `test_batch_payload_structure`, `test_timestamp_format`, `test_platform_string_format`, `test_session_creation`.
- **Why:** Legal across separate integration binaries but confusing in aggregate reports/blame; the weaker copy (telemetry_tests) can be mistaken for the stronger one (telemetry_e2e).
- **Fix:** Consolidate or rename with suffixes.

### I-6. `safe_ops_integration.rs` marked async but mostly sync work
- **File:** `tests/safe_ops_integration.rs:11-19, 27-45`
- **Why:** `#[tokio::test]` wrappers around purely synchronous assertions (`RateLimiterConfig`, `TransactionGuard`, `AtomicCounter`) add runtime startup cost for no benefit. Cosmetic.
- **Fix:** Make sync tests `#[test]`.

### I-7. `TestConfig::skip_if_*` prints skip reason twice-ish
- **File:** `tests/common/mod.rs:97-121` + macros at :573-600
- **Why:** `skip_if_no_system` eprints "Skipping … (set OMG_RUN_SYSTEM_TESTS=1)" and then `require_system_tests!` additionally calls `report_skip` printing `[omg-skip] system tests disabled…`. Two different channels/messages for the same event; harmless but noisy and the grep-count contract depends only on the second.

### I-8. `test_daemon_protocol_boundaries` reduced to a single Ping probe
- **File:** `tests/tdd_edge_cases.rs:52-62`
- **Why:** Comment explains `Request::Batch` removal; remaining coverage is just `id() == u64::MAX`. Fine, but the test name promises protocol boundary testing it no longer delivers.

### I-9. Injection fixtures include `%00`-style encodings that CLI can't act on
- **File:** `tests/common/fixtures.rs:56-66` (`INJECTION_ATTEMPTS` includes `"/etc/passwd%00.txt"`)
- **Why:** Percent-decoding never occurs in a CLI arg context, so those cases can never exercise a real decode path; they only pad lists. Harmless.

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 2 |
| MEDIUM | 7 |
| LOW | 17 |
| INFO | 9 |
| **Total** | **35** |

Dominant themes: (1) process-global state mutation in tests breaking isolation (H-1/H-2/M-7); (2) security tests that cannot fail or test themselves instead of production code (M-2/L-8/L-1/I-3); (3) tests exercising the harness rather than the product (M-4); (4) vacuous/tautological assertions giving false confidence (L-3/L-4/L-5/I-1).


---

# SLICE 19

# Audit slice-19 — build/install/CI shell & Docker surface (omg)

Scope: `install.sh`, `Makefile`, `Dockerfile.*`, `docker-compose.yml`, `release_and_publish.sh`, `benchmark*.sh`, `build-pgo.sh`, `scripts/`, `.github/workflows/`. Read-only audit; ~2,900 lines reviewed.

## CRITICAL

### C1. `ci-success` gate depends on a nonexistent `integration` job — CI is invalid
- File: `.github/workflows/ci.yml:462`
- Excerpt:
  ```yaml
  ci-success:
    needs: [quick-gate, linux-matrix, macos, integration]
  ```
- Why: There is no `integration:` job anywhere in the file (jobs defined: `quick-gate`, `linux-matrix`, `feature-intersections`, `macos`, `ci-success`). GitHub Actions rejects a workflow that references an undefined job in `needs`, so every push/PR to main fails workflow parsing — or, depending on API behavior, the required status check can never be satisfied. Either way the entire CI gate is broken.
- Fix: Remove `integration` from `needs` (and from the summary table/env), or add the missing integration job.

### C2. PGO script benchmarks a binary it never builds — workload profile is garbage
- File: `build-pgo.sh:33,35-46`
- Excerpt:
  ```bash
  RUSTFLAGS="-Cprofile-generate=$PGO_DATA_DIR" \
      cargo build --profile pgo-instrument --bin "$BINARY_NAME"   # BINARY_NAME=omgd
  ...
  ./target/pgo-instrument/omg search vim >/dev/null 2>&1 || true
  ```
- Why: Only `omgd` is built with instrumentation. Every `./target/pgo-instrument/omg ...` invocation targets a file that does not exist; errors are swallowed by `|| true`. The resulting profile covers only daemon startup, so the "optimized" release-pgo build is optimized for the wrong (nearly empty) workload while claiming "8-15% runtime speedup on hot paths".
- Fix: Build both binaries instrumented (`--bins`) and run the client commands against the running daemon.

## HIGH

### H1. Installer grants near-root capabilities (`cap_dac_override`) to a user-writable binary
- File: `install.sh:800`
- Excerpt:
  ```bash
  sudo setcap 'cap_dac_override,cap_fowner,cap_chown+ep' "$INSTALL_DIR/omg"
  ```
- Why: `$INSTALL_DIR` defaults to `~/.local/bin`, owned by the unprivileged user. Any process running as that user (or any malware in the user session) can overwrite the binary or exploit it, and with `cap_dac_override+ep` gains effectively unrestricted read/write of every file on the system regardless of permissions. This converts any user-level compromise into full system compromise. Also note file capabilities are dropped if the binary is replaced by later updates via `mv` (install_binary renames), silently disabling turbo mode — but the standing risk is the privilege design itself.
- Fix: Do not grant DAC-override capabilities to a user-owned binary. Use a privileged helper/daemon owned by root with a narrow policy, or document sudo-per-operation instead. At minimum install to a root-owned path and use file caps there.

### H2. `audit.yml`: supply-chain gates defeated by missing `pipefail`
- File: `.github/workflows/audit.yml:126,131,136,141`
- Excerpt:
  ```yaml
  cargo deny check advisories 2>&1 | tee -a $GITHUB_STEP_SUMMARY
  ```
- Why: GitHub Actions runs steps with `bash -e` but not `pipefail`; the pipeline's exit status is `tee`'s (always 0). All four `cargo deny check` gates (advisories, licenses, bans, sources) therefore never fail the job — vulnerable/banned dependencies pass CI as long as the report prints.
- Fix: Add `set -euo pipefail` to the step (or `shell: bash` with `bash -euo pipefail`), e.g. `cargo deny check advisories 2>&1 | tee -a "$GITHUB_STEP_SUMMARY"; exit ${PIPESTATUS[0]}`.

### H3. Makefile/Dockerfiles reference multiple scripts and binaries that do not exist
- Files / lines:
  - `Makefile:270` → `./scripts/verify_benchmark_setup.sh` (target `bench-verify`) — **missing**
  - `Makefile:275` → `./run_ubuntu_benchmark.sh` (`bench-ubuntu`) — **missing**
  - `Makefile:280-281` → feature `debian-pure` + `./test_debian_speed.sh` (`bench-ubuntu-local`) — script **missing**
  - `Makefile:291` → `./scripts/generate_benchmark_report.sh` (`bench-report`) — **missing**
  - `Dockerfile.arch-e2e:23` and `Dockerfile.ubuntu-benchmark:28` → `COPY omg-binary ...` — no `omg-binary` at repo root
  - `Dockerfile.benchmark:17,21` → `COPY target/release/omg` (excluded by `.dockerignore` target/) and `COPY benchmark_debian.sh` — **missing**
  - `Dockerfile.ubuntu-benchmark:32` → `COPY benchmark_ubuntu.sh` — **missing**
- Why: These targets/images fail immediately when invoked (`make bench-verify|bench-ubuntu|bench-report`, `make bench-ubuntu-local`, `docker build -f Dockerfile.arch-e2e/.benchmark/.ubuntu-benchmark`). Dead/broken code paths shipped in-tree.
- Fix: Delete the broken targets/Dockerfiles or restore the referenced files.

### H4. `release.yml` sync-r2 uploads garbage version marker on `workflow_dispatch`
- File: `.github/workflows/release.yml:365,382-387`
- Excerpt:
  ```yaml
  if: github.event_name == 'push' || (github.event_name == 'workflow_dispatch' && github.event.inputs.dry_run == 'false')
  ...
  VERSION="${GITHUB_REF#refs/tags/v}"
  echo -n "$VERSION" > release/latest-version
  npx --yes wrangler@4.123.0 r2 object put "omg-releases/latest-version" ...
  ```
- Why: On manual dispatch from a branch, `GITHUB_REF` is `refs/heads/main`, so `latest-version` becomes the literal string `heads/main` and is published to `releases.pyro1121.com/latest-version`, which clients likely consume to resolve "latest". This poisons the update channel (and similarly the artifact names in release notes are fine only because the release job uses tag refs). Same expression in the release job's notes step would produce nonsense notes on dispatch.
- Fix: Derive the version from `github.ref_name` guarded to tag events, or abort sync-r2 unless `startsWith(github.ref, 'refs/tags/v')`.

## MEDIUM

### M1. `install.sh` runs under `set -u` only — no `set -e`/`pipefail`; several failure paths continue silently
- File: `install.sh:18`
- Examples:
  - Line 296: download failure is handled, but line 305 checksum-sidecar curl failure falls into the "no sidecar" branch correctly; however line 258 `grep ... | head | cut` pipelines have no pipefail so partial metadata corruption could yield a wrong-but-nonempty version.
  - Line 774: `"${INSTALL_DIR/omg" completions ...` failure masked by `|| true` (intended).
  - Lines 709-715 / 750-765: appends to rc files unchecked; a read-only or full disk yields a "success" message.
- Fix: `set -euo pipefail` and audit each intentional `|| true`.

### M2. `release_and_publish.sh write_version` rewrites every top-level `version =` line and can desync lockfile
- File: `release_and_publish.sh:488-493`
- Excerpt:
  ```bash
  sed -i "s/^version = \".*\"/version = \"${version}\"/" Cargo.toml
  cargo update -p omg --precise "$version" 2>/dev/null || true
  ```
- Why: The sed is global over all lines matching `^version = "` (today only `[package]`, but any future workspace member/dep pinned at column 0 gets clobbered). The `cargo update --precise` failure is silenced with `|| true`, so Cargo.lock can remain at the old version while Cargo.toml bumps — then `--locked` builds (used by CI/release) fail or, worse, publish a release whose lock doesn't match the manifest. `bump_patch` also does raw arithmetic without validating components (`1.2` → error).
- Fix: Restrict sed to the `[package]` section (or use `cargo set-version`), and fail hard when the lockfile update fails.

### M3. `release_and_publish.sh` tag push fallback uses `--force-with-lease` on release tags
- File: `release_and_publish.sh:934-937`
- Excerpt:
  ```bash
  if ! git push origin "v${version}" 2>&1; then
    log_info "Tag push failed, trying with --force-with-lease"
    git push origin "v${version}" --force-with-lease
  fi
  ```
- Why: A release tag that already exists remotely (e.g. created by CI or another maintainer pointing at different commits) gets force-overwritten, allowing a signed/tagged artifact lineage to be rewritten silently. Release tags should be immutable.
- Fix: Never force-push tags; die and require manual resolution.

### M4. benchmark.sh speedup computation breaks when pacman/yay are absent
- File: `benchmark.sh:298-308`
- Excerpt:
  ```bash
  pac=${RESULTS["$cmd,pacman"]}
  ...
  if [[ "$pac" != "N/A" && "$omg_d" != "0" ]]; then
      speedup=$(echo "scale=1; $pac / $omg_d" | bc ...)
  ```
- Why: When pacman/yay aren't installed, `RESULTS["$cmd,pacman"]` is never set, so `$pac` expands empty (script has no `set -u`); the guard passes ("empty" ≠ "N/A") and `bc` receives `/ 6.5` producing a syntax-error message plus stray newline in `$speedup`, printed as garbage like `\n x` in the results table and written into `benchmark_report.md`.
- Fix: Default unset entries to `N/A` and add `-z "$pac"` to the guard.

### M5. `Dockerfile.arch-e2e` + `docker-compose.yml` arch service is self-contradictory and unbuildable
- Files: `Dockerfile.arch-e2e:23`, `docker-compose.yml:306-314`
- Excerpt:
  ```dockerfile
  COPY omg-binary /usr/local/bin/omg        # image bakes in a prebuilt binary
  ```
  ```yaml
  arch:
    build: { dockerfile: Dockerfile.arch-e2e }
    command: bash -c "cargo build --release --locked -j 2 && ./target/release/omg ..."
  ```
- Why: (a) `omg-binary` does not exist in the repo/context, so `docker compose build arch` always fails. (b) Even if provided, the compose command rebuilds from source inside a container whose toolchain was never installed (the Dockerfile installs no rust), so the command fails too. Two incompatible designs merged into one broken path.
- Fix: Pick one: a source-build image with rustup (like docker-e2e.yml does inline), or a binary-injection image with a documented pre-step.

### M6. `check-perf-regression.py`: baseline-missing returns success, malformed baseline fails closed — asymmetric and hyperfine matcher too loose
- File: `scripts/check-perf-regression.py:54-63,20-23`
- Why:
  - Missing baseline → `return 0` ("skip") means the regression gate silently passes whenever `benchmarks/summary.json` is absent (e.g., first run after repo state change), while corrupt JSON → `return 1`. Inconsistent policy invites silent skips.
  - `'omg' in result['command'].lower()` matches any command containing "omg", including hypothetical future entries like "OMG (omg-fast)" or a renamed comparator, picking whichever comes first rather than the OMG Daemon row specifically.
- Fix: Match on exact command name `"OMG (Daemon)"`; decide one explicit policy for missing baselines (fail or warn loudly).

### M7. `extract-release-notes.sh`: version interpolated unescaped into awk regex
- File: `scripts/extract-release-notes.sh:147`
- Excerpt:
  ```awk
  if ($0 ~ "\\[" version "\\]") {
  ```
- Why: `.` and other regex metachars in the version match any character, so `0.1.2` extracts the section for `0.1x2y` headings; a version containing `.*` etc. can match unintended sections and emit wrong release notes into the GitHub release body (release.yml:295 feeds this straight into release notes).
- Fix: Escape metachars or use `index($0, "[" version "]")`.

### M8. `sync_releases_to_r2` runs wrangler against the wrong directory when omg-web layout differs
- File: `release_and_publish.sh:989-1000`
- Why: If neither `../omg-web/workers/releases` nor `$ROOT_DIR/workers/releases` exists, `releases_worker_dir=""` and `run_wrangler "."` executes `wrangler r2 object put` from the Rust repo root, picking up whatever `wrangler.toml` exists there (repo has `workers/router` only) — potentially deploying/uploading against an unintended Worker config/account, or failing after the GitHub release was already published (partial distribution state).
- Fix: Fail fast when the releases worker dir cannot be found instead of defaulting to `.`.

### M9. `install.sh` RETURN trap deletes temp dir on intermediate returns — and is redundant/fragile
- File: `install.sh:289`
- Excerpt:
  ```bash
  trap 'cleanup_tmp_dir' RETURN
  ```
- Why: A RETURN trap set inside `install_from_release` fires on return from *that function*, but bash semantics make function-local RETURN traps easy to leak/misfire across nested sourced contexts; more importantly, cleanup is already guaranteed by the global `trap cleanup EXIT` (line 352), so this adds a second deletion path that can race the EXIT cleanup if extended later. Minor robustness smell in the security-critical installer.
- Fix: Drop the RETURN trap; rely on the single EXIT trap.

### M10. `benchmark-hyperfine.sh` fallback `exec ./benchmark.sh` assumes CWD = repo root
- File: `benchmark-hyperfine.sh:432-436` (also `benchmark.sh:82-84` relative `./target/release/...`)
- Why: Invoked from any other directory (`bash ~/Documents/omg/benchmark-hyperfine.sh`), the exec and all relative binary paths break mid-run after a multi-minute cargo build. No `cd "$(dirname "$0")"` guard unlike other scripts in the repo.
- Fix: `cd "$(dirname "${BASH_SOURCE[0]}")"` at startup.

## LOW

### L1. `install.sh`: `OMG_VERSION` interpolated unvalidated into GitHub API URL
- File: `install.sh:232` — `curl -fsSL "${api_base}/tags/${OMG_VERSION}"`. A crafted value (spaces, `?`, `..`) alters the request path/query. Low impact (client-side, HTTPS, same host) but parse/validate against `^v?[0-9A-Za-z.\-]+$`.

### L2. `install.sh`: PATH-dedup grep uses unescaped `INSTALL_DIR` as ERE
- File: `install.sh:746` — dots in a custom `INSTALL_DIR` match any char; a dir like `/home/u.local/bin` may falsely match an existing unrelated line and skip adding PATH. Use `grep -qF -- "export PATH=\"$INSTALL_DIR"` style fixed-string checks per shell type.

### L3. `install.sh`: telemetry opt-out silently skipped if rc file missing (fish config dir never created)
- File: `install.sh:707-717` — for fish, `~/.config/fish/config.fish` often doesn't exist; the whole `if [[ -f ]]` block skips writing, and the user is told telemetry is disabled when no opt-out was persisted. Create parent dirs and the file.

### L4. `install.sh`: unsupported arch values still flow into asset names
- File: `install.sh:190-192,211` — `armv7l`, `i686`, or exotic `uname -m` output produces artifact names that will never exist, yielding a generic "No prebuilt binary found". Emit a clear unsupported-arch error instead.

### L5. `install.sh`: checksum sidecar fetched from the same origin/channel as the archive
- File: `install.sh:303-320` — sha256 verification protects against truncated/corrupt downloads but not against a compromised release bucket/account (no signature verification). Documented trust model should note this; consider minisign/sigstore.

### L6. `Makefile`: `.PHONY` list incomplete
- File: `Makefile:3` — missing `install tdd coverage test-lib test-fuzz-quick dev dev-stop dev-check docker-debian-shell docker-test bench-verify bench-ubuntu bench-ubuntu-local bench-ubuntu-save bench-report qa fmt-check clippy-strict audit`. A stray file named e.g. `install` or `coverage` would shadow the target.

### L7. `Makefile`: fuzz artifact check treats pre-existing crash artifacts from earlier runs as current failures
- File: `Makefile:108,116,135` — `if [ -d fuzz/artifacts ]` fails whenever the directory exists, even if this run found nothing and leftovers weren't cleaned; conversely a fresh clone never has it. Gate on artifacts created during this run.

### L8. `Makefile`: `make install` copies `omgd` without existence check
- File: `Makefile:77` — `cp target/release/omgd ~/.local/bin/` fails the whole install if the daemon bin wasn't produced (unlike install.sh which tolerates it).

### L9. `release_and_publish.sh`: interactive prompt under non-TTY dies with confusing EOF
- File: `release_and_publish.sh:465` — `read -p ... -n1` returns nonzero on EOF under `set -e`; automation runs get `read: 1: ...` instead of a clear message. Guard with `[[ -t 0 ]] || exit 1` and a clear error.

### L10. `release_and_publish.sh`: `run_cmd` discards stderr in quiet mode
- File: `release_and_publish.sh:383-389` — `"$@" 2>&1` merges stderr into captured stdout (unused), hiding diagnostics on failure. Redirect to a log file or leave uncaptured.

### L11. `benchmark.sh`: `min` seeded at 999999 ms
- File: `benchmark.sh:180` — iterations slower than ~16.7 minutes report a bogus minimum. Seed from first sample.

### L12. `benchmark.sh`: `eval` of constructed command strings
- File: `benchmark.sh:176,185` — commands are internal constants today, but `eval` + string-built commands is fragile; any future parameterization becomes an injection point. Prefer arrays/`"${cmd[@]}"`.

### L13. `benchmark.sh`: report table header/format mismatch risk and terminal-vs-file duplication
- File: `benchmark.sh:253-311` — the stdout table prints `${omg_d}ms` while the file rows print bare numbers with a separate header block; if `RESULTS` keys change, the two layouts drift silently (already: stdout shows "Speedup", file header says "Speedup vs pacman"). Single source of truth recommended.

### L14. `generate-benchmark-chart.py`: charts plot hardcoded numbers presented as measured data
- File: `scripts/generate-benchmark-chart.py:26-28,97-99,136-137` — values are frozen copies of old README tables (with a hardcoded "Kernel: Linux 6.18.3" environment blurb). Regenerating charts after new benchmarks silently republishes stale numbers. Read from `benchmarks/summary.json` / result files instead.

### L15. Dockerfiles pin nothing where CI pins digests (drift between local and CI)
- Files: `Dockerfile.arch-e2e:2` (`archlinux:latest`), `Dockerfile.fedora:2` (`fedora:latest`), vs digest-pinned images in ci.yml/release.yml. Local E2E tests run against moving distro targets that CI never sees.
- Also `Dockerfile.debian:35` and `Dockerfile.ubuntu:31` build **without** `--locked` and without `license,pgp` features, unlike every CI leg — local Debian smoke tests validate a dependency set and feature combo CI never tests.

### L16. `Dockerfile.apt` runtime image installs `libapt-pkg-dev`
- File: `Dockerfile.apt:34` — final stage only needs the shared library (`libapt-pkg` runtime package); installing `-dev` pulls the whole toolchain-adjacent package set into the shipped image.

### L17. `Dockerfile.debian` PATH includes nonexistent dirs
- File: `Dockerfile.debian:38` — `/home/omguser/omg/.cargo/bin` doesn't exist (cargo lives in `~/.cargo/bin`); harmless but misleading.

### L18. `docker-e2e.yml` uploads entire `target/debug/` on failure
- File: `.github/workflows/docker-e2e.yml:317-322` — multi-GB artifact of all test binaries on every failure, slow and wasteful (retention default 90d). Upload logs/paths instead.

### L19. `changelog.yml`/`benchmark.yml` write token via credential.helper echo
- Files: `changelog.yml:236`, `benchmark.yml:178` — `git config credential.helper '!f() { echo password=$GITHUB_TOKEN; }; f'` places the token in `.git/config` for the step duration and exposes it to any process reading config; standard but prefer `persist-credentials: true` checkout or `env: GIT_ASKPASS`. Also `git config --unset credential.helper` is skipped when push fails (step aborts), leaving the token in config for subsequent steps in the same job.

### L20. `benchmark.yml`: badge regex validation happens in a separate step from use
- File: `.github/workflows/audit/../benchmark.yml:104-111,123-124` — `SEARCH_TIME`/`SPEEDUP` outputs are computed with `jq ... | awk`; if jq selects nothing (command name drift), outputs become empty strings, `bc` then errors (`scale=1;  / ...`), and the badge step's `[[ =~ ]]` guards abort — good — but the regression-gate step already ran with an empty summary.json mismatch path (see M6). Consolidate validation at extraction time.

### L21. `secrets.yml`: TruffleHog push scan uses `github.event.before` which is all-zeros for forced pushes/new branches
- File: `.github/workflows/secrets.yml:60-62` — with a zero base, behavior varies (full-history or no-op scan depending on trufflehog version), giving inconsistent coverage exactly on the pushes most worth scanning.

## INFO

### I1. `install.sh` sets `RUSTFLAGS="-C target-cpu=native"` for source installs
- File: `install.sh:612` — binaries built this way are non-portable to older CPUs of the same arch; acceptable for self-builds but undocumented.

### I2. `install.sh` banner `clear`s the terminal unconditionally
- File: `install.sh:413` — wipes user scrollback when run locally; minor UX.

### I3. `install.sh` spinner subshells use `local` outside a visible function scope boundary
- File: `install.sh:380-384` — works because the subshell inherits function context, but `local chars`/`local i`/`local c` inside `( )` is unusual; plain assignments suffice.

### I4. `docker-compose.yml` debian service invokes `bash scripts/debian-smoke-test.sh`, which itself builds the same Docker image again
- Files: `docker-compose.yml:320-325`, `scripts/debian-smoke-test.sh:110-111` — the smoke test runs `docker build` from *inside* a container (needs dockerd socket, absent in compose), so the last command of the debian leg fails whenever reached. Nested container-engine assumption is broken.

### I5. `.env` contains a live-looking OAuth access token on disk (untracked)
- File: `.env` (gitignored, not committed) — `STITCH_ACCESS_TOKEN=ya29.…` Google OAuth token stored in plaintext in the repo working tree. Not in git, but gitleaks/trufflehog history scans won't catch future accidental inclusion patterns beyond standard rules; rotate and move to a secret manager. Flagged for awareness only.

### I6. `Makefile` help text advertises removed/broken targets
- File: `Makefile:45-47` — lists `bench-ubuntu`, `bench-ubuntu-local`, `bench-verify` which are broken per H3.

### I7. `mutation.yml` accepts exit codes 0|2|3 from cargo-mutants
- File: `.github/workflows/mutation.yml:65-71` — deliberate tolerance, but undocumented which semantics map to which code; add a comment to prevent future masking of real failures.

### I8. `coverage.yml` regenerates reports `if: always()` even when the test run failed
- File: `.github/workflows/coverage.yml:84-116` — partial/failed-run coverage is uploaded and could mislead codecov trends. Consider `if: success()` for uploads or mark artifacts accordingly.

---

**Totals:** 2 CRITICAL, 4 HIGH, 10 MEDIUM, 21 LOW, 8 INFO — 45 findings.


---

# SLICE 20

# Slice 20 — `site/src/components/` (top-level files a–m + landing/, dashboard spot-check)

Audit agent: audit20 · READ-ONLY · repo `~/Documents/omg-web`

## Scope & coverage

Fully read line-by-line:
- `site/src/components/Benchmarks.tsx`
- `site/src/components/Footer.tsx`
- `site/src/components/Header.tsx`
- `site/src/components/Hero.tsx`
- `site/src/components/Installation.tsx`
- `site/src/components/MarketingOfferDialog.tsx`
- `site/src/components/Pricing.tsx`
- `site/src/components/UpgradeModal.tsx`
- `site/src/components/landing/FeatureGrid.tsx`
- `site/src/components/landing/LicenseSuccessModal.tsx`

Spot-checked (pattern scan + targeted reads; full 10.5k-line `dashboard/admin/**` tree is assumed to be covered by the dedicated dashboard slices):
- `dashboard/admin/tag-color.ts`, `dashboard/admin/shared/ErrorCard.tsx`, `shared/TabErrorBoundary.tsx`, `admin/CustomerDetailDrawer.tsx` (focus-trap/billing-portal sections), `admin/insights/InsightsTab.tsx` (localStorage bookmark section), `dashboard/premium/{index,types}.ts`

Cross-referenced: `site/src/lib/api.ts`, `shared/marketing-offer.ts`, `routes/index.tsx`.

---

## Findings

### 1. HIGH — Unconditional URL rewrite to `/` on mount in LicenseSuccessModal
**File:** `site/src/components/landing/LicenseSuccessModal.tsx:53-56`
```tsx
onMount(() => {
  const params = new URLSearchParams(window.location.search);
  const sessionId = params.get('session_id');
  window.history.replaceState({}, '', '/');
```
`window.history.replaceState({}, '', '/')` executes on **every mount of the landing page**, regardless of whether a Stripe return occurred. Any query string on `/` (UTM tags, ref codes, hash-less deep-link params used by marketing) is silently destroyed on page load, breaking attribution tracking and shareable links. It also wipes the document title/history entry state (`{}` state). The rewrite should only happen after confirming this is a checkout return (`success=true && session_id`), and ideally preserve the pathname instead of hardcoding `'/'`.
**Fix:** move `replaceState` inside the success branch and use `window.location.pathname`.

### 2. MEDIUM — Sign-out performed via GET navigation (logout CSRF / prefetch risk)
**File:** `site/src/components/Header.tsx:106-111`
```tsx
<DropdownMenu.Item
  as="a"
  href="/api/auth/sign-out"
  ...
>
```
Sign-out is a plain anchor navigation (GET). If the endpoint performs a destructive session change on GET (typical better-auth config allows it unless configured for POST-only), any cross-site `<img src="/api/auth/sign-out">` or browser prefetch logs the user out (logout CSRF). Best practice is a POST form with CSRF token or `fetch(..., {method:'POST'})`.
**Fix:** trigger sign-out via a POST request from an `onSelect` handler, not a GET anchor.

### 3. MEDIUM — Global `keydown` shortcut missing modifier/contentEditable guards
**File:** `site/src/components/Header.tsx:27-41`
```ts
if (
  event.key === 'd' &&
  !event.ctrlKey &&
  !event.metaKey &&
  !(event.target instanceof HTMLInputElement) &&
  !(event.target instanceof HTMLTextAreaElement)
) { event.preventDefault(); navigate('/dashboard'); }
```
Problems:
- `event.altKey` / `event.shiftKey` are not excluded → Shift+D ("D") and Alt+D hijack navigation.
- No guard for `contentEditable` hosts, `<select>`, or elements with `role="textbox"` — typing "d" in any custom editor navigates away.
- Fires even when a Kobalte menu/dialog has focus, and while composing IME input.
**Fix:** also check `!event.altKey`, skip when `(event.target as HTMLElement).isContentEditable`, and consider requiring no shift.

### 4. MEDIUM — Clipboard write promises unguarded → unhandled rejections, no user feedback
**Files:**
- `site/src/components/Installation.tsx:34-38` (`copyCommand`)
- `site/src/components/landing/LicenseSuccessModal.tsx:100-104` (`copyLicense`)
```ts
await navigator.clipboard.writeText(commandFor(activeTab()));
setCopied(true);
```
`navigator.clipboard.writeText` rejects in non-secure contexts, when the document is hidden, or when permission is denied. There is no try/catch; callers do `onClick={() => void copyCommand()}`, so the rejection becomes an **unhandled promise rejection** and the UI silently shows nothing happened (no error state). Contrast: `MarketingOfferDialog.copyCode` correctly wraps this in try/catch.
**Fix:** wrap in try/catch and surface an error/fallback (e.g., legacy `document.execCommand('copy')` fallback or "copy failed" message).

### 5. MEDIUM — Single-shot license verification with no retry/polling
**File:** `site/src/components/landing/LicenseSuccessModal.tsx:60-63, 30-47`
```ts
void verifyCheckoutSession(sessionId).then(result => {
  setState(result.ok ? result.state : { _tag: 'unverified' });
});
```
When the webhook has not yet landed, status may be `processing`; the modal shows "Provisioning license" but **never re-checks** — the user must close and manually reopen the page (whose `session_id` was already stripped by finding #1, making recovery impossible without digging through the Stripe receipt email). A transient network failure also lands permanently in "Verification unavailable".
**Fix:** poll `verifyCheckoutSession` with backoff while state is `verifying`/`processing` (e.g., every 3s up to ~60s), and keep the session id in memory (not only the URL).

### 6. LOW — `copied` indicator never resets in MarketingOfferDialog
**File:** `site/src/components/MarketingOfferDialog.tsx:39-46`
```ts
const copyCode = async (): Promise<void> => {
  ...
    setCopied(true);
```
Unlike `Installation.tsx` (1.8 s reset timer) and `LicenseSuccessModal` (1.8 s reset), the copy confirmation (`Check` icon replacing `Copy`) stays forever once set. Inconsistent UX and the affordance looks stuck.
**Fix:** mirror the other components: `setTimeout(() => setCopied(false), 1800)` with `onCleanup`.

### 7. LOW — Overly broad error mapping masks real causes in UpgradeModal
**File:** `site/src/components/UpgradeModal.tsx:110-118`
```ts
cause instanceof ApiError && cause.status === 400 && props.promotionCode !== undefined
  ? 'This offer must be used with the same account email that requested it.'
```
Any 400 from `createCheckout` while a promotion code happens to exist in props is reported as an email-mismatch message, even if the 400 was caused by something else entirely (malformed offer id, expired code rejected differently, validation error). Misleading diagnostics for users and support.
**Fix:** match on a server-provided error code/message rather than blanket status+coincidence.

### 8. LOW — MarketingOfferDialog swallows errors without observability
**File:** `site/src/components/MarketingOfferDialog.tsx:29-31`
```ts
} catch {
  setError('We could not create the offer right now. Try again in a moment.');
}
```
The cause is dropped entirely — unlike `LicenseSuccessModal` which calls `reportClientError`. Rate-limit (429), server outage, and schema-mismatch failures are indistinguishable client-side.
**Fix:** forward the cause to `reportClientError` before showing the generic message.

### 9. LOW — Stale offer retained across dialog sessions
**File:** `site/src/components/MarketingOfferDialog.tsx:20-33` + `Pricing.tsx:96-101`
Once one offer is claimed, `offer()` stays set forever; reopening "Get a private code" always shows the old code and there is no way to claim for a different email without a full page reload. Also `Pricing`'s `promotionCode` signal persists after `UpgradeModal` unmounts. If the code expires (dialog shows expiry date but nothing disables use), the stale code flows into checkout and fails server-side with a confusing message (see #7).
**Fix:** reset dialog/email state per open, or allow "use a different email".

### 10. LOW — `LicenseSuccessModal.copyLicense` timer not cleaned up
**File:** `site/src/components/landing/LicenseSuccessModal.tsx:105`
`window.setTimeout(() => setCopied(false), 1800)` result is discarded and never cleared in `onCleanup`. If the component unmounts within 1.8 s of copying, the callback writes to a disposed signal (harmless in Solid but a latent leak pattern; `Installation.tsx` does this correctly).
**Fix:** store timer id and clear in `onCleanup`.

### 11. LOW — Shared `copied` flag across all install tabs
**File:** `site/src/components/Installation.tsx:28-40`
A single `copied()` signal backs three tab panels; each panel's button renders "Copied ✓". If a user copies on tab 1 then switches tabs within 1.8 s, the other tabs' buttons falsely read "Copied". Cosmetic but incorrect feedback.
**Fix:** key copied state by tab id.

### 12. INFO — Mobile nav lacks focus trap / Escape / outside-click handling
**File:** `site/src/components/Header.tsx:139-176`
The mobile menu is a raw conditional `<nav>`: Escape doesn't close it, focus isn't trapped, and clicking outside doesn't dismiss (only link clicks or the toggle do). Accessibility gap versus the Kobalte dropdown used on desktop.

### 13. INFO — List items rendered without keys in `.map`
**Files:** `Header.tsx:66-84, 143-160`; `Hero.tsx:57-61`
Solid tolerates keyless lists, but static `NAV_ITEMS`/`REPLACED_MANAGERS` maps produce anonymous `<li>` nodes; fine today, fragile under future dynamic lists.

### 14. INFO — Numbered-list formatting breaks past 9 items
**File:** `site/src/components/landing/FeatureGrid.tsx:55`
```tsx
<span class="pt-1 font-mono text-[10px] text-[var(--signal)]">0{index() + 1}</span>
```
Hardcoded `"0"` prefix produces `010` for item 10+. Only 3 items exist today; add `String(index()+1).padStart(2,'0')` if list grows.

### 15. INFO — Raw `err.message` surfaced in admin error boundaries
**Files:** `dashboard/admin/shared/TabErrorBoundary.tsx:21`; `ErrorCard.tsx` default message
Boundary fallbacks print the thrown error message verbatim (`{err.message}`). Safe against XSS (Solid escapes text) but can leak internal messages (URLs, stack-ish detail) to admin UI users; acceptable for an admin-only surface, noted for completeness.

### 16. INFO — `InsightsTab.toggleBookmark` persists storage inside a signal updater
**File:** `dashboard/admin/insights/InsightsTab.tsx:80-89`
`persistBookmarkedInsights(next)` runs inside the `setBookmarkedInsights(prev => ...)` updater function. Solid invokes updaters synchronously exactly once so this works, but side effects inside updaters are fragile (double-invocation patterns, batching semantics) — move persistence into an effect or after the setter returns. Parsing of the stored value itself is done correctly via Effect Schema (good).

### 17. INFO — Positive notes (no action): strong security hygiene observed
- `UpgradeModal.startCheckout` validates the returned redirect is `https://checkout.stripe.com` before navigating (`UpgradeModal.tsx:88-97`) — excellent open-redirect defense.
- `CustomerDetailDrawer.handleOpenBillingPortal` similarly validates `https://billing.stripe.com` origin.
- `tag-color.ts` properly constrains untrusted tag colors to `^#[0-9a-fA-F]{6}$` before inline-style interpolation, eliminating CSS injection.
- `CHECKOUT_SESSION_ID_PATTERN` (`^cs_[A-Za-z0-9]{10,200}$`) validates the Stripe session id before it reaches the API query string; `encodeURIComponent` used anyway.
- UpgradeModal's attempt-counter race guard (`checkoutAttempt`) correctly cancels stale redirects after modal close/reopen.

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 1 |
| MEDIUM | 4 |
| LOW | 7 |
| INFO | 5 |

Total findings: 17. Highest-priority fixes: #1 (URL rewrite destroying landing-page query strings), #2 (GET sign-out), #5 (license verification dead-end combined with #1).


---

# SLICE 21

# Audit slice-21 — omg-web `site/src/components/` (files n–z)

Scope audited line-by-line:
- `site/src/components/Pricing.tsx`
- `site/src/components/UpgradeModal.tsx`
- `site/src/components/ui/BrandIcons.tsx`
- `site/src/components/ui/Icons.tsx`
- `site/src/components/ui/Skeleton.tsx`
- `site/src/components/ui/Tooltip.tsx`

(`MarketingOfferDialog.tsx` starts with "M" and was treated as the other agent's half; not duplicated here.)

---

## Findings

### 1. HIGH — Dead component: `ui/Tooltip.tsx` is never imported anywhere
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/ui/Tooltip.tsx` (entire file, 118 lines)
- **Excerpt:** `export const Tooltip: Component<TooltipProps> = props => { ... }`
- **Why it's a bug:** Repo-wide grep shows zero imports of `ui/Tooltip` outside the file itself. It is dead code that still carries real defects (see #2, #3) and will silently rot. Per project standards ("remove obsolete paths instead of keeping them"), it should either be deleted or fixed and adopted.
- **Fix:** Delete the file, or wire `aria-describedby={tooltipId}` etc. and start using it where tooltips are needed.

### 2. MEDIUM — Tooltip position is computed once at hover time with no viewport clamping or scroll handling
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/ui/Tooltip.tsx:31-58, 100-110`
- **Excerpt:**
  ```ts
  const rect = target.getBoundingClientRect();
  ...
  y = rect.top - 8;
  ```
  rendered into a `Portal` with `position: fixed; left/top`.
- **Why it's a bug:** (a) A tooltip near a viewport edge renders off-screen/clipped — no flip/clamp logic exists for any of the four positions. (b) The coordinates are captured once on `mouseenter`; scrolling or layout shift while visible leaves the floating tooltip stranded in the wrong place until re-hover. This is the classic reason to use Floating UI.
- **Fix:** Clamp/flip against `window.innerWidth/Height`, and close (or reposition via a `scroll` listener / `IntersectionObserver`) when the anchor moves.

### 3. LOW — Tooltip is not announced to assistive tech (`id` never referenced)
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/ui/Tooltip.tsx:96-104`
- **Excerpt:** `<div id={tooltipId} role="tooltip" ...>` — nothing sets `aria-describedby={tooltipId}` on the trigger wrapper.
- **Why it's a bug:** The `role="tooltip"` node is orphaned; screen-reader users get neither the hover nor the described-by association. Also there is no touch-device path (only `mouseenter`/`focusin`), so mobile users can never see the content.
- **Fix:** Add `aria-describedby` when visible (or use Kobalte's Tooltip), and add a tap/click fallback.

### 4. MEDIUM — Programmatic redirect after async checkout may be popup-blocked and loses user gesture
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/UpgradeModal.tsx:63-77`
- **Excerpt:**
  ```ts
  redirectTimer = setTimeout(() => {
    ...
    const link = document.createElement('a');
    link.href = checkoutUrl.toString();
    ...
    link.click();
  }, 500);
  ```
- **Why it's a bug:** The navigation happens ≥500 ms after the click gesture, inside a timer, after an `await`. Some browsers/extensions treat synthetic anchor clicks detached from user activation as popups/unwanted navigations and block them (especially Safari ITP and Brave). When blocked, the modal sits in `processing` forever with no recovery path — the button stays disabled (`disabled={step() === 'processing'}`) and the user must close and restart.
- **Fix:** Prefer `window.location.assign(checkoutUrl)` (same-tab navigation is not popup-blocked), and/or add a timeout/fallback "Open checkout" link in the processing step so users can recover.

### 5. LOW — Misleading error message conflates any 400 with the promotion-email rule
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/UpgradeModal.tsx:79-86`
- **Excerpt:**
  ```ts
  : cause instanceof ApiError && cause.status === 400 && props.promotionCode !== undefined
    ? 'This offer must be used with the same account email that requested it.'
    : 'Unable to start checkout. Please try again.'
  ```
- **Why it's a bug:** Any 400 from `POST /billing/checkout` while a promo code happens to be present (e.g. invalid tier, malformed body, expired offer rejected differently) is reported as the email-mismatch message. Users receive a wrong diagnosis and may e.g. re-sign-up with a different email pointlessly.
- **Fix:** Surface a server-provided error code/message from the ApiError body, or restrict this message to the specific error code the backend returns for email mismatch.

### 6. LOW — Stale-error/stale-step state if checkout resolves after the dialog was closed without `close()`
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/UpgradeModal.tsx:44-52, 66-69`
- **Excerpt:** the `createEffect` only calls `cancelCheckout()` when `props.isOpen` goes false; `setStep('select')`/`setError(null)` happen only in `close()`. In `startCheckout`, the post-await guard `if (!props.isOpen || attempt !== checkoutAttempt) return;` leaves `step()` stuck at `'processing'` if the modal is reopened by a different code path before unmount.
- **Why it's a bug:** Currently benign because `Pricing.tsx:141-147` unmounts the modal via `<Show when={showUpgradeModal()}>` on close (remounting resets signals). But the component's own contract (`isOpen` prop) suggests it supports staying mounted while hidden; in that usage the reopened modal would show a perpetual "Opening checkout" spinner with the button disabled.
- **Fix:** In the `isOpen === false` branch of the effect also `setStep('select'); setError(null);`, making the component safe regardless of mount strategy.

### 7. LOW — Promotion code persists for the lifetime of the page once obtained
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/Pricing.tsx:33-41, 126-131`
- **Excerpt:** `onOfferCreated={offer => setPromotionCode(offer.code)}` — never cleared on close, checkout completion, or failure.
- **Why it's a bug:** After one offer flow (even an abandoned one), every subsequent checkout attempt silently sends the old promo code, and the "Introductory 20% discount ready" banner remains indefinitely. If the server has single-use/expiring codes, later checkouts fail with the confusing 400 message from finding #5.
- **Fix:** Clear `promotionCode` when the offer dialog is dismissed without use, and after a checkout attempt consumes it (or verify validity before display).

### 8. INFO — Hard-coded legacy theme tokens in `Skeleton.tsx` clash with the design system used elsewhere
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/ui/Skeleton.tsx:16, 28`
- **Excerpt:** `bg-white/5`, `rounded-5xl bg-void-850 border border-white/5 shadow-2xl`
- **Why it's an issue:** Every sibling component uses the manifest CSS-variable tokens (`var(--paper-raised)`, `var(--rule)`, …). `void-850`/`nebula-200` classes appear to be leftovers from an older palette (also used by unused `Tooltip.tsx`). Visual inconsistency and possible missing utilities if the old Tailwind palette is removed.
- **Fix:** Port Skeleton (and delete-or-port Tooltip) onto the `--*` custom-property tokens.

### 9. INFO — Tier selection buttons have no selected-state semantics
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/UpgradeModal.tsx:92-135`
- **Excerpt:** `<button type="button" ... onClick={() => chooseTier(tierKey)}>` (plan cards)
- **Why it's an issue:** Two large clickable cards function as a radio group but are plain buttons with no `aria-pressed`/`role="radio"`/`aria-checked` and no visually-indicated current selection; keyboard/screen-reader users cannot tell which plan is active in the details step.
- **Fix:** Use a radiogroup (or add `aria-pressed` + a selected style).

### 10. INFO — Redundant open-state plumbing between `Pricing` and `UpgradeModal`
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/Pricing.tsx:140-148`
- **Excerpt:** `<Show when={showUpgradeModal()}><UpgradeModal isOpen={showUpgradeModal()} ...>` 
- **Why it's an issue:** `isOpen` is always `true` when mounted, so the entire `props.isOpen` machinery inside UpgradeModal (effect branches, guards at lines 47, 67, 72) is unreachable as wired. Not a runtime bug today, but two overlapping sources of truth invite the stale-state class of bugs noted in finding #6.
- **Fix:** Keep one mechanism — either pass `isOpen` and keep it always mounted, or keep the `Show` and drop the prop guards.

### 11. INFO — BrandIcons mask URL interpolation
- **File:** `/home/pyro1121/Documents/omg-web/site/src/components/ui/BrandIcons.tsx:15-19`
- **Excerpt:** ``mask: `url("${iconUrl}") center / contain no-repeat` ``
- **Why it's minor:** Safe as written because `iconUrl` comes from bundler-imported static assets, but quoting is manual; any future dynamic source would be a CSS-injection sink. No current exploit.
- **Fix:** Fine as-is for static imports; avoid ever passing dynamic strings here.

---

## Verified non-issues (checked explicitly)

- **Checkout redirect host allowlist** (`UpgradeModal.tsx:56-59`): protocol must be `https:` and hostname exactly `checkout.stripe.com` — solid open-redirect protection.
- **Race guard**: `checkoutAttempt` monotonic counter plus `redirectTimer` clearing correctly cancels superseded/closed checkouts; `onCleanup(cancelCheckout)` prevents timer leaks after unmount.
- **No XSS sinks**: no `innerHTML`, no `dangerouslySetInnerHTML` in any scoped file; Solid escapes interpolated text.
- **Pricing free-tier CTA** anchors to `#install` — valid in-page target.
- `Icons.tsx` is a pure re-export module — no logic to get wrong.

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH | 1 |
| MEDIUM | 3 |
| LOW | 4 |
| INFO | 3 |


---

# SLICE 22

# Audit slice-22 — `omg-web/site/src/lib/`

Agent: audit22 · Scope: admin.ts, analytics-client.ts, api-error.ts, api-hooks.ts, api.ts, auth-client.ts, auth.ts, better-auth-sign-out.ts, error-message.ts, licensing-bff.ts, lookup.ts, mailto.ts, observability.ts, performance-entry.ts, prelude.ts, query.ts (read-only audit; supporting files `site/shared/licensing-routes.ts` read for context only).

## MEDIUM

### M1. Duplicate page-exit events: `runPageExitCallbacks` is invoked by three overlapping listeners
- **File:** `site/src/lib/analytics-client.ts:515-541, 647-649`
- **Excerpt:**
  ```ts
  window.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') runPageExitCallbacks();
  });
  window.addEventListener('pagehide', runPageExitCallbacks);
  window.addEventListener('beforeunload', runPageExitCallbacks);
  ...
  onPageExit(() => { trackTimeOnPage(); flushEvents(); });
  ```
- **Why it is a bug:** On a real tab close/navigation all three listeners fire (visibilitychange→hidden fires first, then pagehide; beforeunload also fires for navigations). The shared callback set therefore runs two or three times per exit. `reportWebVitals` is idempotent via its `vitalsReported` flag, but the `initAnalytics` exit callback is not: `trackTimeOnPage()` queues a second (slightly different) `time_on_page` event and `flushEvents()` runs again. Result: inflated time-on-page counts and duplicate batches.
- **Fix:** Add a `hasRun` boolean in `runPageExitCallbacks` (or remove the redundant listeners — `pagehide` alone covers both navigation and tab close on modern browsers).

### M2. Failed analytics batches are re-queued forever with no cap or drop policy
- **File:** `site/src/lib/analytics-client.ts:224-239`
- **Excerpt:**
  ```ts
  if (!response.ok) {
    eventQueue = [...events, ...eventQueue];
  }
  } catch {
    eventQueue = [...events, ...eventQueue];
  }
  ```
- **Why it is a bug:** If the endpoint returns a persistent non-2xx (e.g. 400 for a schema change, 413, 429) or the network is down, every failed batch is re-appended and re-sent every 3s indefinitely. Two consequences: (a) unbounded memory growth of `eventQueue` over long-lived tabs (SPA sessions can last days), and (b) permanently-invalid payloads can never succeed yet are retried forever. Also note `keepalive: true` bodies are capped at 64 KB by browsers; a batch near `MAX_BATCH_SIZE` with large UTM values could exceed that and fail deterministically, then loop.
- **Fix:** Cap the queue (e.g. drop oldest beyond N events), give up after K attempts (attach an attempt counter), and don't retry 4xx responses.

### M3. `getAdminCustomerTags` builds its query string by raw string interpolation instead of `withQuery`
- **File:** `site/src/lib/api.ts:195-198`
- **Excerpt:**
  ```ts
  export const getAdminCustomerTags = (customerId: string) =>
    apiRequest(
      Http.TagsResponseSchema,
      `${LicensingRoutes.adminCustomerTagsGet.path}?customerId=${customerId}`
    );
  ```
- **Why it is a bug:** Every sibling call site uses `withQuery`, which percent-encodes via `URLSearchParams`. Here an unencoded `customerId` is interpolated; a customer id containing `&`, `#`, `%`, space, or `+` produces a malformed/corrupted query (e.g. `+` decoded as space server-side, `&x=` injected as an extra parameter). This is both a correctness defect and an inconsistency with the documented pattern (`withQuery`'s doc comment says entries mirror previous URLSearchParams construction).
- **Fix:** Use `withQuery(LicensingRoutes.adminCustomerTagsGet.path, ['customerId', customerId])`.

### M4. Query default `retry: 2` retries hopeless 4xx responses
- **File:** `site/src/lib/query.ts:48-58`
- **Excerpt:**
  ```ts
  queries: {
    staleTime: 60000,
    gcTime: 5 * 60 * 1000,
    retry: 2,
    ...
    throwOnError: isServerQueryError,
  ```
- **Why it is a bug:** Mutations get the status-aware `shouldRetryMutation`, but queries retry any error twice, including 401/403/404 from admin endpoints. An unauthorized admin session triggers three identical failing requests before surfacing; combined with `useAdminFirehose`'s 5 s refetch interval this multiplies useless load. (Retry delay also starts at 1 s, so the UI shows loading longer than necessary.)
- **Fix:** Give queries the same status-aware retry predicate as mutations (`retry: (count, err) => shouldRetryMutation(count, err)`).

## LOW

### L1. `ctaTypeForLink` classifies CTA by naive substring matching on href
- **File:** `site/src/lib/analytics-client.ts:573-586`
- **Excerpt:**
  ```ts
  const href = link.getAttribute('href') || '';
  if (href.includes('install')) return 'install';
  if (href.includes('signup') || href.includes('login')) return 'signup';
  ...
  ```
- **Why it is a bug:** Substring checks misfire on unrelated URLs, e.g. `/blog/how-to-install-x` → `'install'`, `/changelog/login-page-redesign` → `'signup'`, `/pricing-faq/archive` → `'pricing'`. Analytics data is silently polluted. Also `link.getAttribute('href')` returns the raw attribute, so relative links like `install` match but anchor-only `#download` does not, inconsistently.
- **Fix:** Match on path segments (`new URL(href, location.origin).pathname`) with exact segment equality, or require explicit `data-track-cta` attributes only.

### L2. LCP observer listeners leak when interaction precedes first LCP entry
- **File:** `site/src/lib/analytics-client.ts:398-420`
- **Excerpt:**
  ```ts
  let reported = false;
  const stopObserving = () => {
    if (!reported && metrics.lcp !== undefined) { reported = true; observer.disconnect(); }
  };
  for (const event of ['keydown', 'click', 'visibilitychange']) {
    window.addEventListener(event, stopObserving, { once: true, capture: true });
  }
  ```
- **Why it is a bug:** Each listener is `{ once: true }`: if the user clicks before LCP is recorded, that click consumes one listener without disconnecting the observer; if the user clicks twice before LCP, two listeners are consumed. The third click leaves one listener plus a live PerformanceObserver for the rest of the page lifetime. Minor leak / continued observation work.
- **Fix:** Don't use `once`; keep the three persistent listeners and have `stopObserving` remove all three when it fires.

### L3. Hardcoded production fallback for Better Auth base URL on SSR
- **File:** `site/src/lib/auth-client.ts:4-8`
- **Excerpt:**
  ```ts
  baseURL: import.meta.env.SSR
    ? import.meta.env['VITE_BETTER_AUTH_URL'] || 'https://omg.latham.cloud'
    : window.location.origin,
  ```
- **Why it is a bug:** A hardcoded third-party-looking origin is baked into source as the silent fallback. In staging/preview deployments where `VITE_BETTER_AUTH_URL` is unset, server-side auth calls silently hit production instead of failing loudly — wrong-environment auth (session cookies minted against prod). Also inconsistent with client branch which uses `window.location.origin`.
- **Fix:** Throw on missing env at module init instead of falling back to a fixed origin.

### L4. `requireSameOrigin` rejects older browsers that omit `Sec-Fetch-Site`
- **File:** `site/src/lib/licensing-bff.ts:139-160`
- **Excerpt:**
  ```ts
  return inbound.headers.get('Sec-Fetch-Site') === 'same-origin'
    ? Effect.void
    : Effect.fail(new LicensingSameOriginRequired());
  ```
- **Why it is a bug:** Browsers without Fetch Metadata support (Safari < 16.4 era, older WebViews) send neither `Origin` nor `Sec-Fetch-Site` on same-origin GETs, so every dashboard read fails with "Same-origin request required" for those users. Security posture is correct for modern browsers, but this is a UX-breaking compatibility cliff with no fallback message surfaced distinctly.
- **Fix:** Accept requests with no Origin and no Sec-Fetch-Site only if some additional weak signal holds, or return a specific "browser unsupported" response so clients can show guidance.

### L5. Identity `name > 128 chars` hard-blocks licensing access entirely
- **File:** `site/src/lib/licensing-bff.ts:23-30`
- **Excerpt:**
  ```ts
  name: Schema.String.pipe(Schema.maxLength(128)),
  role: Schema.Literal('admin', 'user'),
  ```
- **Why it is a bug:** `LicensingIdentitySchema` decodes the Better Auth identity; a user whose social-provider profile name exceeds 128 characters fails parsing inside `mintWorkerSession` *after* same-origin and route checks, producing `LicensingBffParseError` → presumably a 500. One weird profile name bricks all licensed functionality for that account rather than being truncated/rejected at signup.
- **Fix:** Truncate/sanitize the name before forwarding to the Worker instead of failing the whole proxy.

### L6. `getErrorMessage` classification relies on exact English copy matching (fragile by design, acknowledged)
- **File:** `site/src/lib/error-message.ts:17-27`
- **Excerpt:**
  ```ts
  case 'failed to fetch':
  case 'networkerror when attempting to fetch resource.':
  ```
- **Why it is a bug:** Documented in the header, but still a latent defect: better-auth copy changes (or locale differences) silently degrade security-relevant states like "email not verified" into the generic fallback, confusing users mid-flow. The Firefox-specific casing variant `'networkerror …'` only matches because of `.toLowerCase()`; a Safari variant would not.
- **Fix:** Prefer structured error codes from better-auth's client (`error.code`/`error.status`) over message text.

### L7. `formatRelativeTime` boundary mismatch between minutes/hours/days
- **File:** `site/src/lib/api.ts:332-350`
- **Excerpt:**
  ```ts
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);
  ...
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  ```
- **Why it is a bug:** Not strictly incorrect, but `diffDays` derived from ms while display switches at 7 d means a timestamp 6d23h ago renders "6d ago" and 7d0h ago falls through to absolute date — fine — however `diffMs` is clamped with `Math.max(0, …)` so *future* timestamps render "Just now", hiding clock-skewed data (a future date looks fresh). Minor correctness/UX issue for admin tables fed by external systems.
- **Fix:** Render future timestamps distinctly (e.g. "in Xh") or clamp-aware flag.

## INFO

### I1. `mailto:` link percent-encodes `@`
- **File:** `site/src/lib/mailto.ts:14-16`
- **Excerpt:** ``link.href = `mailto:${encodeURIComponent(decoded.right)}`;``
- RFC 6068 requires percent-encoding of `@`? Actually `@` is allowed literally in the mailbox; `encodeURIComponent` encodes it to `%40`. User agents decode the userinfo before handing off, so this works in practice, but it is unusual encoding and some strict mail handlers have had issues. Using `encodeURI`-style preservation of `@` (or manual encoding of only reserved chars) would be more conventional. Validation itself (maxLength 254 + pattern) is solid and prevents `javascript:`-style injection since the value must match an email pattern.

### I2. `auth.ts` derives `baseURL` from `origin`, discarding any configured path prefix
- **File:** `site/src/lib/auth.ts:38-42` — `const baseUrl = parsedBaseUrl.origin;`
- Deploying Better Auth under a sub-path (e.g. `https://host/site`) would break callbacks because the path is stripped. Fine for current apex deployment; worth a comment/assertion.

### I3. `analytics-client.ts` `getPageContext` strips query but keeps the fragment
- **File:** `site/src/lib/analytics-client.ts:120` — `.split('?').at(0)`
- `page_url` retains `#hash`, which can embed UI state/route fragments; privacy goal ("strip query params") only partially met.

### I4. `performance-entry.ts` `navigationTtfbMs` reads only `entries[0]` though doc says "first navigation timing entry"
- Correct given `performance.getEntriesByType('navigation')` returns at most one entry, but callers pass the array unvalidated; if a future caller passes arbitrary arrays the semantics silently change. Cosmetic.

### I5. `api.ts` `ApiError` used before its class declaration in file order
- `runWorkerRequest` (line ~24) references `ApiError` declared at line ~78. Legal (TDZ resolved at call time) but fragile ordering; moving the class above its use improves readability.

### I6. `licensing-bff.ts` strips only one representation of `Set-Cookie`
- **File:** `site/src/lib/licensing-bff.ts:305` — `responseHeaders.delete('Set-Cookie')`
- On Cloudflare Workers, multiple `Set-Cookie` headers are accessible via `getSetCookie()`; `Headers.delete('Set-Cookie')` does remove them in current workerd, but relying on that is implementation-sensitive. Safer: build the outbound Headers explicitly from an allowlist (Content-Type, etc.) instead of copying everything and deleting.

### I7. `admin.ts` rebuilds `CloudflareEnv` field-by-field, silently dropping `ADMIN_API_SECRET`/`LICENSING_API`
- **File:** `site/src/lib/admin.ts:31-40`
- Currently harmless (createAuth doesn't need them), but any future consumer of `requireAdmin().env` expecting the binding will get `undefined` with no type error at the reconstruction site. Prefer passing `cloudflareEnv` directly.

### I8. `lookup.ts` linear scan
- `valueForKey` is O(n) per lookup; fine for current small tables, just noting complexity for future large tables.

### I9. Dead/unused export surface
- `admin.ts` returns `{ env, userId, db }` — verify all three are consumed by callers; `db` returned alongside `env` duplicates construction responsibility. No dead code found otherwise; `better-auth-sign-out.test.ts`, `query.test.ts`, `lookup.test.ts`, `licensing-bff.test.ts`, `performance-entry.test.ts` exist and cover their units.

## Verified-correct highlights (no action)
- `requireAdmin` re-reads the persisted role from D1 on every request (does not trust session claims) and never leaks parse details (`storedDataErrorResponse`).
- BFF enforces email verification → same-origin → allowlisted method/path → bounded body (streaming cap enforced mid-stream with reader cancel) → short-lived minted session; no cookie/secret forwarded downstream; Authorization replaced with minted token.
- `readBoundedBody` correctly handles absent Content-Length and NaN parse.
- `withQuery`, `queryErrorStatus`, `shouldRetryMutation` logic reviewed line-by-line; correct.
- No SQL/command injection vectors found in scope; all DB access via drizzle parameterized queries; Stripe customer URL gated by `/^cus_[A-Za-z0-9]+$/`.

**Total findings: 18** (0 CRITICAL, 0 HIGH, 4 MEDIUM, 7 LOW, 7 INFO)


---

# SLICE 23

# Audit slice-23 — omg-web `site/src/lib` (dashboard-contract*, dashboard-page.ts, segment-condition*, worker-api*, contracts/, state/, stores/)

Scope audited line-by-line: `dashboard-contract.ts`, `dashboard-page.ts`, `segment-condition.ts`, `worker-api.ts`, `contracts/` (worker-http, dashboard, telemetry-dashboard, licensing-dashboard, dashboard-store, d1-rows, tier), `state/dashboard-view.ts`, `stores/dashboardStore.ts`, and their tests. Cross-checked against `~/types/ui/filters.ts`, `site/shared/*`, and two consumers (`routes/api/dashboard.ts`, `components/dashboard/AdminDashboard.tsx`) for context only.

---

## MEDIUM

### M1. Duplicate divergent date-range union — `'14d'` offered in UI but absent from canonical `DateRange`; would silently wipe all persisted state if ever persisted
- **File:** `site/src/lib/dashboard-page.ts:40` (and `:85-94`), vs `site/src/types/ui/filters.ts:1` and `site/src/lib/contracts/dashboard-store.ts:19`
- **Code:**
  ```ts
  export type DashboardDateRange = '7d' | '14d' | '30d' | '90d';   // dashboard-page.ts
  export type DateRange = '7d' | '30d' | '90d' | 'custom';          // types/ui/filters.ts
  const DateRangeSchema = Schema.Literal('7d', '30d', '90d', 'custom'); // dashboard-store contract
  ```
- **Why it is a bug:** Two overlapping-but-different unions model the same concept. `DASHBOARD_DATE_RANGES` renders a `'14d'` option (`dashboard-page.ts:91`) consumed by `pages/DashboardPage.tsx:517-521`. Today the value stays in a local signal, but any future assignment into `store.filters.dateRange` / a `SavedView.dateRange` compiles fine only if cast; at runtime `decodePersistedDashboardState` rejects the whole payload (`Schema.Literal(1)` version passes but the inner literal fails) → `getInitialState()` returns defaults and the user silently loses **all** saved views, filters and CRM view mode on next load. This is a latent data-loss trap plus a UX inconsistency (a range that the rest of the system cannot represent).
- **Fix:** Delete `'14d'` from `DashboardDateRange`/`DASHBOARD_DATE_RANGES` or add `'14d'` to `DateRange` + `DateRangeSchema` + the AdminDashboard select (`AdminDashboard.tsx:515-522`). Keep one single source-of-truth type.

### M2. `requestText`: non-JSON error body is misclassified as a network failure, discarding the HTTP status
- **File:** `site/src/lib/worker-api.ts:60-77`
- **Code:**
  ```ts
  if (!response.ok) {
    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: cause => new WorkerApiNetworkError(cause),
    });
    return yield* Effect.fail(new WorkerApiHttpError(response.status, parseApiError(payload, ...)));
  }
  ```
- **Why it is a bug:** For an endpoint whose error body is not JSON (HTML error page, plain text from a proxy, empty body), `response.json()` rejects and the caller receives `WorkerApiNetworkError` ("Request failed") instead of `WorkerApiHttpError` with the real status code. Callers that branch on `status` (e.g. retry-on-429, show "upgrade required" on 402) silently break; monitoring/diagnostics misreport server errors as network outages.
- **Fix:** Check `response.ok` first; attempt `response.json()` inside its own try/catch falling back to `response.text()`, and always fail with `WorkerApiHttpError(response.status, …)` for non-2xx regardless of body shape.

### M3. `requestDecodedJson`: same status-masking for non-JSON bodies, and 204/no-body success responses are misreported as network errors
- **File:** `site/src/lib/worker-api.ts:79-107`
- **Code:**
  ```ts
  const response = yield* fetcher.fetch(url, init);
  const payload = yield* Effect.tryPromise({
    try: async () => { const body: unknown = await response.json(); return body; },
    catch: cause => new WorkerApiNetworkError(cause),
  });
  ```
- **Why it is a bug:** (a) A non-2xx response with a non-JSON body becomes `WorkerApiNetworkError`, hiding the status code (same defect class as M2). (b) A 2xx response with an empty body (`204 No Content`) also surfaces as `WorkerApiNetworkError` even though the request "succeeded" — the user sees a network-failure message for a successful operation.
- **Fix:** Branch on `response.ok` before decoding; treat empty bodies explicitly (`undefined` payload for 204), and wrap decode failures of *successful* responses as parse failures, not network failures.

---

## LOW

### L1. Account-dashboard load path never surfaces the API's error message (inconsistent with telemetry path)
- **File:** `site/src/lib/state/dashboard-view.ts:46-50`
- **Code:**
  ```ts
  if (!response.ok) {
    yield* Effect.fail(new DashboardLoadError('Failed to load dashboard data'));
  }
  ```
- **Why it is a bug:** The telemetry pipeline in the same file parses the error payload via `parseApiError` to show e.g. "Session expired", while this path shows a fixed generic string and never consumes the response body. Users cannot distinguish auth-expiry from outage.
- **Fix:** Mirror the telemetry pipeline: read the JSON body, run `parseApiError(payload, 'Failed to load dashboard data')`.

### L2. `saveView()` uses `crypto.randomUUID()`, which throws outside secure contexts
- **File:** `site/src/lib/stores/dashboardStore.ts:186`
- **Code:** `id: crypto.randomUUID(),`
- **Why it is a bug:** `crypto.randomUUID` is only available in secure contexts (HTTPS/localhost). If the app is ever served over HTTP (staging, LAN preview), clicking "Save view" throws an uncaught TypeError instead of saving.
- **Fix:** Fall back to a manual UUID v4 implementation (`crypto.getRandomValues`) when `randomUUID` is unavailable.

### L3. Re-clicking the active tab pushes duplicate entries into tab history, making "back" a no-op
- **File:** `site/src/lib/stores/dashboardStore.ts:139-146`
- **Code:**
  ```ts
  setTab(tab: AdminTab) {
    setState('navigation', prev => ({
      activeTab: tab,
      tabHistory: [...prev.tabHistory.slice(-(TAB_HISTORY_LIMIT - 1)), prev.activeTab],
    }));
  },
  ```
- **Why it is a bug:** Clicking the already-active tab repeatedly fills the 10-slot history with duplicates; `goToPreviousTab()` then pops to the same tab, so the back affordance appears broken. Also nothing dedupes ping-pong (A→B→A→B exhausts history with no useful entries).
- **Fix:** Guard with `if (tab === state.navigation.activeTab) return;` (and optionally skip when equal to top-of-history).

### L4. Licensing→telemetry projection hardcodes daily package counts to 0, zeroing charts/exports
- **File:** `site/src/lib/contracts/licensing-dashboard.ts:56-58`
- **Code:**
  ```ts
  packages_installed: 0,
  packages_searched: 0,
  ```
- **Why it is a bug:** The canonical licensing payload's daily rows genuinely lack per-day package fields (verified in `shared/licensing-dashboard.ts:50-55`), so every projected row reports zero. Downstream, `getPackageBarHeight`/`getTotalPackages`/CSV export render flat-zero package series for customers served by the canonical endpoint — a silent data-quality defect rather than an honest "unavailable" state.
- **Fix:** Either extend the Worker's canonical daily payload with real package counts, or remove package bars from views driven solely by the canonical payload instead of presenting fabricated zeros.

### L5. Achievement metadata destroyed by projection (points always 0, category hardcoded, progress binary)
- **File:** `site/src/lib/contracts/licensing-dashboard.ts:70-82`
- **Code:**
  ```ts
  icon: achievement.emoji,
  category: 'usage',
  points: 0,
  progress: achievement.unlocked ? 100 : 0,
  ```
- **Why it is a bug:** Real points/category/progress data cannot survive the projection, so any UI showing points totals or partial progress displays wrong values (0 points, all-or-nothing progress). The raw `emoji` string is also passed through as `icon` where the type expects an icon identifier — `getAchievementIcon` ignores it (`_emoji` unused).
- **Fix:** Map genuine fields when the worker provides them; otherwise drop these display elements rather than showing fabricated zeros.

### L6. `formatDashboardTimeSaved` rounds across unit boundaries: 3,599,999 ms renders as "60m", not "1h"
- **File:** `site/src/lib/dashboard-page.ts:157-165`
- **Code:**
  ```ts
  const hours = milliseconds / 3_600_000;
  if (hours < 1) return `${Math.round((milliseconds / 60_000) * 10) / 10}m`;
  ```
- **Why it is a bug:** Any duration within ~3 s below a boundary displays at the wrong unit ("60m", "24.0h"). Cosmetic but visible in stat cards.
- **Fix:** Round first, then choose the unit based on the rounded value (e.g. `const mins = ms/60000; if (Math.round(mins) >= 60) …hours…`).

---

## INFO

### I1. CSV export writes unquoted, unescaped rows
- **File:** `site/src/lib/dashboard-page.ts:286-306` (`createTelemetryExport`)
- **Excerpt:** `content: rows.map(row => row.join(',')).join('\n')`
- **Note:** Current fields are ISO dates and numbers, so output is well-formed today, but any future string column (hostname, note) will silently corrupt the CSV and spreadsheet-formula injection becomes possible. Suggest a shared `toCsv` helper with quoting now.

### I2. `Flag` schema transform's encode side returns `1 | 0` typed as `boolean`
- **File:** `site/src/lib/contracts/worker-http.ts:20-25`
- **Note:** Runtime-harmless (encode direction unused for responses) but a type-level lie; use `Schema.transformOrFail` or annotate the encoder return as `boolean` honestly (`toI ? true : false`).

### I3. `parseTier` silently maps any unknown tier to `'free'`
- **File:** `site/src/lib/contracts/tier.ts:14-16`
- **Note:** A paying customer with a mistyped/new tier string ('gold', 'pro_plus') is displayed and possibly entitlement-checked as free. Consider logging the invalid value or failing loudly for admin-facing paths.

### I4. Dead/ignored parameter in `getAchievementIcon(_emoji, name)`
- **File:** `site/src/lib/dashboard-page.ts:126`
- **Note:** `_emoji` accepted but unused; either drop the parameter or use the emoji as fallback icon source.

### I5. Persisted saved-view names are unbounded strings rendered back into the DOM
- **File:** `site/src/lib/contracts/dashboard-store.ts:31` (`name: Schema.String`), `stores/dashboardStore.ts:176`
- **Note:** Solid escapes text so no XSS, but multi-megabyte names can bloat localStorage (persist is best-effort try/catch, so failure is silent). Suggest `maxLength` on the name schema and on the save modal input.

### I6. `WORKER_API_ORIGIN` allowlist is exact-path and origin-pinned — good
- **File:** `site/src/lib/worker-api.ts:44-53`
- **Note:** Positive: `new URL(input, origin)` normalization defeats `..` traversal, cross-origin fetches are rejected, and the public Worker route is pinned to exactly `/api/site/analytics/track`. No finding; recorded as verified.

### I7. Session tokens are decoded but never leave the BFF
- **File:** `site/src/lib/contracts/d1-rows.ts` (`SessionRowSchema.token`), consumer `routes/api/dashboard.ts:118` uses token only for the `isCurrent` comparison.
- **Note:** Verified no session-token leakage into the outbound dashboard payload.

### I8. Generation guards in `state/dashboard-view.ts` correctly prevent stale-response clobbering
- **File:** `site/src/lib/state/dashboard-view.ts:113-155`
- **Note:** Verified race handling is sound (`loadId !== xLoadId` checks, `Effect.runPromiseExit` never rejects). No finding.

---

## Test-coverage observations

- `worker-api.test.ts` asserts a "typed HTTP failure" only for a JSON error body; add cases for non-JSON error bodies (M2/M3) and 204 success (M3) — both currently regress silently.
- No test covers `requestText`, `browserWorkerFetcher` allowlist rejections, or `dashboardStore.setTab` duplicate-history behavior.
- `dashboard-store.test.ts` covers forward-version rejection and bad tabs well; add a case proving a `'14d'` dateRange is rejected (documents M1 intent).

**Totals:** 0 CRITICAL · 3 HIGH-equivalent recorded as MEDIUM (M1–M3) · 3 MEDIUM · 6 LOW · 8 INFO — 17 findings.


---

# SLICE 24

# Slice 24 — omg-web routes/, entry-client.tsx, entry-server.tsx, app.tsx

Read-only audit. Files covered: all of `site/src/routes/` (incl. `routes/api/`), `entry-client.tsx`, `entry-server.tsx`, `app.tsx`.

---

## HIGH

### H-1. Full Better Auth session object (incl. session token) serialized to the client
- File: `site/src/routes/dashboard.tsx`, lines 12–41 (requireAuth) and 70 (`createAsync(() => requireAuthQuery())`)
```ts
const session = await auth.api.getSession({ headers: event.request.headers });
if (!session?.user) { throw redirect('/login'); }
return session;   // returns { session: { token, ipAddress, userAgent, expiresAt, ... }, user: {...} }
```
- Why it is a bug: `requireAuthQuery` is a `'use server'` router query; its return value is serialized across the server/client boundary and embedded in the SSR/hydration payload. The full Better Auth result includes `session.token` (the session credential), `session.ipAddress`, and `session.userAgent`. `pages/DashboardPage.tsx` explicitly documents that "Session secrets (token, IP, agent) are deliberately not part of the consumed contract", yet the route hands the raw session to it, contradicting that contract and leaking the token into HTML/JS where any XSS or browser extension can read it (cookie is httpOnly; this defeats that protection).
- Fix: return only the projected user shape:
```ts
return { user: { id: session.user.id, name: session.user.name, email: session.user.email,
                 emailVerified: session.user.emailVerified, image: session.user.image } };
```

## MEDIUM

### M-1. `/api/offer` reads entire request body into memory before enforcing size limit
- File: `site/src/routes/api/offer.ts`, lines ~37–44
```ts
const body = yield* Effect.tryPromise({ try: () => event.request.text(), ... });
if (new TextEncoder().encode(body).byteLength > MAX_OFFER_BODY_BYTES) {
  return yield* Effect.fail(new OfferProxyRejected(413));
}
```
- Why it is a bug: the 4096-byte limit is enforced *after* `request.text()` has already buffered the whole body in Worker memory. An unauthenticated client can stream arbitrarily large bodies (e.g. multi-hundred-MB POST) to a public endpoint, causing memory pressure/CPU-limit kills on the Worker (DoS).
- Fix: check `request.headers.get('Content-Length')` first and reject early, then read with a bounded reader (or at minimum reject on Content-Length > MAX before buffering).

### M-2. `/api/offer` forwards unvalidated body labeled as JSON to an admin-authenticated internal service
- File: `site/src/routes/api/offer.ts`, lines ~46–60
```ts
headers: { 'Content-Type': 'application/json', 'X-Admin-Secret': secret, 'X-Internal-Call': 'service-binding', ... },
body,   // raw client-controlled text, never parsed/validated
```
- Why it is a bug: arbitrary unauthenticated bytes are forwarded to the licensing service under full `X-Admin-Secret` authorization with a fixed path but attacker-chosen payload. If the upstream handler for `marketingOffer.path` ever dispatches on payload content (fields selecting other operations) or mis-parses, this becomes a privilege escalation into admin scope. Defense-in-depth requires validating the marketing-offer schema (email field etc.) at this boundary before proxying.
- Fix: parse/validate the body against the offer-request Effect Schema here; reject invalid payloads with 400 before attaching the admin secret.

### M-3. `/api/dashboard`: missing D1 binding not distinguished from missing context
- File: `site/src/routes/api/dashboard.ts`, lines ~88–101 (`readCloudflareEnv`)
```ts
function readCloudflareEnv(event: APIEvent): CloudflareEnv | null {
  const env = event.nativeEvent.context.cloudflare?.env;
  if (!env) return null;
  return { DB: env.DB, ... };   // DB may be undefined
}
```
- Why it is a bug: unlike `routes/api/auth/[...auth].ts` (`getEnv` explicitly throws when `cf?.DB` is absent) and `admin.tsx` (explicit handling), this route only checks that `context.cloudflare?.env` exists. If bindings are partially configured, `drizzle(env.DB, ...)` fails deep inside `Effect.tryPromise`, surfacing as a generic `DashboardUnavailable` 500 after attempting a call with `undefined` binding — masking a deployment error as an infrastructure blip and producing confusing Sentry noise.
- Fix: mirror `getEnv` from the auth route: fail fast with a distinct error when `env.DB` is undefined.

### M-4. Module-scope singleton `QueryClient` shared across concurrent SSR requests
- File: `site/src/app.tsx` line 15 (`import { queryClient } from './lib/query'`) and `lib/query.ts` line 55 (`export const queryClient = new QueryClient(...)`)
- Why it is a bug: TanStack Query's docs require one QueryClient per server request. A module-level singleton means dehydrated cache state / in-flight queries can bleed between concurrent SSR requests (cross-user data leakage if any authenticated query runs during SSR, e.g. AdminDashboard prefetch paths), plus unbounded cross-request cache growth. Currently mitigated because `AdminDashboard` is `clientOnly(...)` in `admin.tsx`, but nothing structurally prevents a future server-side query from leaking user A's cached data to user B.
- Fix: create the client inside a per-request factory (e.g. via `createHandler` context or a lazily created client keyed to the request event).

## LOW

### L-1. `requireAuth` silently redirects to /login on server misconfiguration
- File: `site/src/routes/dashboard.tsx`, lines 14–17
```ts
if (!event || !cf?.DB) { throw redirect('/login'); }
```
- Why it is a bug: a missing D1 binding (deployment error) produces an infinite-looking login redirect loop UX rather than a 500/error page, and hides the outage from Sentry (contrast with `admin.tsx`, which distinguishes "no cookies → login" from "misconfigured → thrown error"). Users with valid sessions see "please sign in" repeatedly.
- Fix: differentiate: no cookie → redirect('/login'); cookie present but DB/binding unavailable → throw Error (500) so observability captures it.

### L-2. Deleted-user fallback grants default role `user`
- File: `site/src/routes/api/licensing/[...path].ts`, lines ~100–110
```ts
if (isInvalidD1Row(roleLookup)) {
  return yield* Effect.fail(new LicensingIdentityStoreUnavailable('invalid user role row'));
}
return { ..., role: roleLookup._tag === 'present' && roleLookup.value.role === 'admin' ? 'admin' : 'user', ... };
```
- Why it is a bug: if the user row is absent (`_tag === 'absent'`, e.g. account deleted while a live session cookie exists), the code proceeds with `role: 'user'` instead of failing unauthorized. The stale-session identity is then proxied to the licensing service. Failing closed (LicensingUnauthorized) would be safer.
- Fix: treat `_tag === 'absent'` for the user row as unauthorized.

### L-3. HEAD requests to API routes unhandled / catch-all swallows even programming errors uniformly
- File: `site/src/routes/api/auth/[...auth].ts`, lines 62–67
- Why it is a bug/INFO: only GET/POST/PUT/PATCH/DELETE/OPTIONS are exported; HEAD requests fall through to framework default (likely 405 without Allow header). Additionally every exception — including programmer errors such as a typo'd import — is reported identically as generic 500 with CORS headers, which is fine for clients but the OPTIONS short-circuit bypasses `getEnv`, so a totally misconfigured deployment still advertises working CORS preflight (minor inconsistency, may mask outages from monitoring that probes OPTIONS).
- Fix: export HEAD = GET handler semantics if desired; consider letting preflight also verify configuration.

## INFO

### I-1. Duplicate/divergent JSON-LD structured data
- Files: `site/src/entry-server.tsx` lines ~19–35 vs `site/src/routes/index.tsx` lines ~10–21.
- Two different SoftwareApplication blobs are emitted (home page emits its own; entry-server emits another globally, including on every page). They disagree: `operatingSystem` "Linux" vs "Linux, macOS, WSL"; description claims "22x faster than pacman" vs neutral copy. Search engines receive conflicting entity data depending on crawl path. Consolidate into one source of truth rendered once.

### I-2. robots.txt comment contradicts rules
- File: `site/src/routes/robots.txt.ts` lines 8–11: comment says "allows crawlers to reach HTML pages (including pages with `noindex`)" — harmless doc drift; also `Allow: /_build/` appears after `Disallow:` entries (valid, order-independent per RFC 9309, but stylistically misleading). AI-bot blocks (GPTBot/ClaudeBot etc.) apply site-wide including public marketing pages — confirm intentional given SEO investment.

### I-3. sitemap.xml served with `X-Robots-Tag: noindex`
- File: `site/src/routes/sitemap.xml.ts` line ~75. Uncommon (sitemaps aren't indexed anyway); harmless but redundant. Also `escapeXml` is applied only to `<loc>` — currently safe since paths are static constants, but fragile if dynamic entries are added later.

### I-4. `login.tsx` OAuth loading state never cleared on success navigation
- File: `site/src/routes/login.tsx` lines ~30–43 (`handleOAuthLogin`): on success (`result.error` falsy) `setLoading(false)` still runs in `finally` before the social redirect completes, briefly re-enabling buttons — cosmetic only since `signIn.social` navigates away.

### I-5. `docs.tsx` static `.map` without keyed `<For>`
- File: `site/src/routes/docs.tsx` (COMMAND_GROUPS and REFERENCE_LINKS maps, lines ~110/~175). Static arrays so no reactivity bug, but Solid idiom prefers `<For>`/`<Index>`; fine as-is for constant data.

### I-6. `entry-client.tsx` mount return value exported as default
- File: `site/src/entry-client.tsx` line 9: `export default clientMount;` — the disposal function is exported but never consumed by convention; dead surface, harmless.

## Verified non-issues (checked, OK)
- `api/auth/[...auth].ts` CORS: fixed origin + credentials, applied to success and error responses consistently.
- `api/dashboard.ts`: session rows projected without tokens; outbound payload re-parsed via Schema before responding.
- `licensing/[...path].ts`: email-verification gate before identity issuance; opaque messages for infra failures; same-origin enforcement delegated to `proxyLicensingRequest`.
- `sitemap.xml.ts` escaping of loc values correct.
- `admin.tsx` authorization flow correctly maps 401→/login, 403→/dashboard, and preserves errors for observability.


---

# SLICE 25

# Audit slice-25 — omg-web: DashboardPage, design-system, db, hooks, types, app.css

Read-only audit of every line in scope:
`site/src/pages/DashboardPage.tsx`, `site/src/design-system/**`, `site/src/db/auth-schema.ts`,
`site/src/hooks/telemetry-message.{ts,test.ts}`, `site/src/types/**`, `site/src/app.css`
(+ referenced helpers in `site/src/lib/dashboard-page.ts`, `tokens.css` read for cross-checks).

---

## HIGH

### H1. `auth_account` unique index omits `provider_id` — cross-provider account collision
- **File:** `site/src/db/auth-schema.ts:63–66`
```ts
uniqueIndex('auth_account_issuer_accountId_idx').on(table.issuer, table.accountId),
```
- **Why it is a bug:** The uniqueness constraint is `(issuer, account_id)`. The Better Auth convention is `(provider_id, account_id)` (with issuer only relevant for OIDC). With the shared default `issuer = ''`, two *different* providers (e.g. GitHub and Google) that surface colliding numeric external IDs (`"12345"`) map to the same unique key. The second provider link fails with a UNIQUE violation (login broken), or — depending on Better Auth lookup order — an account row belonging to one provider is resolved for another, which is an account-linking/authz hazard.
- **Fix:** `uniqueIndex(...).on(table.providerId, table.accountId)` (keep issuer in a separate partial index for OIDC if needed).

### H2. `Drawer` runs mount/cleanup lifecycle once at page load, not per open — broken focus management and always-on Escape handler
- **File:** `site/src/design-system/components/layouts/DashboardLayout.tsx:318–338`
```tsx
export const Drawer: ParentComponent<DrawerProps> = props => {
  let panelRef: HTMLDivElement | undefined;
  onMount(() => {
    const previouslyFocused = ...;
    panelRef?.focus();
    const handleEscape = ...
    window.addEventListener('keydown', handleEscape);
    onCleanup(...)
  });
  return (
    <Show when={props.open}> ... </Show>
  );
};
```
- **Why it is a bug:** In Solid, `onMount`/`onCleanup` run once when the component mounts, but the DOM inside `<Show when={props.open}>` is created/destroyed on each toggle. Consequences:
  1. At initial render (`open=false`) the Escape listener is registered anyway; pressing Escape anywhere on the page calls `props.onClose()` even with no drawer visible.
  2. When the drawer later opens, focus is **never** moved into the panel (`panelRef` was undefined at mount) — contradicting the documented "focus moved into the panel on open".
  3. Focus restore to `previouslyFocused` fires at component teardown (page navigation), stealing focus unexpectedly; and repeated open/close never recaptures/restores focus.
  - Also: no focus trap, so Tab escapes the "modal" dialog into background content (see M4).
- **Fix:** Move the listener/focus logic into a child component rendered inside `<Show>` (or use `createEffect` on `props.open` with cleanup), and add a focus trap.

---

## MEDIUM

### M1. Analytics date-range selector changes nothing — dead/misleading control
- **File:** `site/src/pages/DashboardPage.tsx:536–560` (buttons) and `:625` (`Activity Trends ({dateRange()})`)
```tsx
onClick={() => setDateRange(option.value)}
...
<span class="gradient-text">Activity Trends ({dateRange()})</span>
```
- **Why it is a bug:** `dateRange()` only re-labels the chart heading; telemetry is fetched once via `loadTelemetry()` and never refetched or filtered by range. Users selecting "7d"/"90d" see the same data under a different title — a functional lie in the UI.
- **Fix:** Either wire the selector into `loadTelemetry(range)` or remove the control until supported.

### M2. Horizontal `BarChart` animation class animates `stroke-dashoffset` on a `div` — animation silently does nothing
- **File:** `site/src/design-system/components/Charts.tsx:476–482`; `site/src/design-system/tokens.css:267–271`
```tsx
props.animated && 'animate-[score-fill_1s_ease-out_forwards]'
```
```css
@keyframes score-fill {
  from { stroke-dashoffset: 283; }
  to   { stroke-dashoffset: var(--score-offset); }
}
```
- **Why it is a bug:** `score-fill` animates an SVG-only property on an HTML `div`, and `--score-offset` is set nowhere in the codebase. The animated horizontal bars simply appear with no fill animation; dead/broken code path.
- **Fix:** Animate `width` from 0 (e.g. a dedicated keyframe using `scaleX`/width) instead of reusing the gauge keyframe.

### M3. Heatmap x-axis labels misalign because skipped labels don't reserve space
- **File:** `site/src/design-system/components/Charts.tsx:120–131`
```tsx
<For each={props.xLabels}>
  {(label, i) => (
    <Show when={i() % 3 === 0}>
      <div class={cn('text-2xs ... ', size())} style={{ width: size() }}>{label}</div>
    </Show>
  )}
</For>
```
- **Why it is a bug:** Hidden labels are removed from the flex flow entirely rather than rendered invisibly, so gaps collapse between only the visible labels. Label columns progressively drift left relative to the grid cells below (e.g. 24 columns → 8 labels spread across ~1/3 the width plus gaps). Same defect pattern applies to y-label alignment (`w-8` fixed vs cell rows including gap).
- **Fix:** Render every label slot and use `invisible`/opacity for skipped ones so spacing matches the grid.

### M4. `Drawer` modal has no focus trap
- **File:** `site/src/design-system/components/layouts/DashboardLayout.tsx:340–420`
- **Why it is a bug:** `aria-modal="true"` announces the rest of the page as inert, but keyboard Tab still reaches background controls behind the overlay (only Escape/backdrop close are handled). Screen-reader/keyboard users can interact with content that is declared non-existent. Compounds H2.
- **Fix:** Implement a focus trap (cycle Tab within panel) or use native `<dialog>`.

### M5. Sparkline gradient `id` derived from raw CSS color string — broken `url(#…)` reference with default color
- **File:** `site/src/design-system/components/Charts.tsx:196–199, 236–243`
```ts
const color = () => props.color || 'var(--color-indigo-500, #6366f1)';
const gradientId = () => `sparkline-grad-${color().replace('#','')}-${gradientIdSuffix}`;
... fill={`url(#${gradientId()})`}
```
- **Why it is a bug:** With the default (or any `var(...)`/`rgb(...)` color), the id contains parentheses, commas, spaces — invalid for both HTML ids and CSS `url(#...)` fragment references. The area fill silently renders nothing whenever `showArea` is used without an explicit hex color. Duplicate colors across instances are disambiguated by `createUniqueId`, but the sanitization itself is wrong.
- **Fix:** Use only the unique suffix for the id (it already guarantees uniqueness) and pass the raw color only to `stop-color`.

### M6. `machine` table lacks a unique constraint on `(license_id, machine_id)`
- **File:** `site/src/db/auth-schema.ts:158–190`
- **Why it is a bug:** `machineId` is indexed but not unique per license. Any race or retry in machine registration inserts duplicate rows; the dashboard then shows duplicate machine cards and inflates `countActiveMachines`, potentially letting users exceed `max_machines` semantics entirely client-side-visible. Whether the API upserts cannot be relied upon as a DB-level guarantee.
- **Fix:** `uniqueIndex('machine_license_machine_idx').on(table.licenseId, table.machineId)` + upsert on conflict.

### M7. `usage_daily` has no unique constraint on `(license_id, date)`
- **File:** `site/src/db/auth-schema.ts:192–224` (index is non-unique: `index('usage_licenseId_date_idx')`)
- **Why it is a bug:** Two concurrent telemetry writes for the same day create duplicate rows; all dashboards summing `daily` double-count commands/packages/time-saved. Aggregation correctness depends entirely on app-level read merging that this schema doesn't enforce.
- **Fix:** Unique index on `(licenseId, date)` with atomic increment/upsert.

### M8. License tier enums disagree: DB has no `'pro'`, UI/WS contract requires it
- **Files:** `site/src/db/auth-schema.ts:110` (`enum: ['free','team','enterprise']`) vs `site/src/hooks/telemetry-message.ts:10–16` (`'free'|'pro'|'team'|'enterprise'`) and `src/lib/contracts/tier.ts` (`'free','pro','team','enterprise'`), TierBadge configs a `pro` entry.
- **Why it is a bug:** A 'pro' customer cannot exist per the DB schema, yet the entire presentation layer (TierBadge pro config with Crown icon, LicenseTierSchema) supports and advertises it. Whichever side is authoritative, the other will either reject valid data (WS messages with `license_tier:'pro'` decode to `null` and are dropped) or display tiers that can never be provisioned.
- **Fix:** Align all three definitions on one tier list.

### M9. Unbounded achievement progress rendered raw — layout corruption for out-of-range values
- **File:** `site/src/pages/DashboardPage.tsx:905–930`
```tsx
<span>{achievement.progress}%</span>
...
style={{ width: `${achievement.progress}%` }}
```
- **Why it is a bug:** Server-supplied `progress` is displayed and used as a percentage width without clamping. A progress of `-5` or `250` produces a negative/overflowing bar and nonsense label. Compare `HealthScore.clampScore` which does clamp.
- **Fix:** `Math.min(100, Math.max(0, achievement.progress))`.

### M10. `HealthScoreGauge` arc geometry is inconsistent — ticks/labels misaligned with drawn arc
- **File:** `site/src/design-system/components/HealthScore.tsx:276–296`
```ts
const radius = createMemo(() => height() - strokeWidth());   // e.g. sm: 48-6=42
// chord = width() - 2*strokeWidth = 1.5*ring - 2*sw  ≠ 2*radius
```
- **Why it is a bug:** For a semicircular arc, the chord between endpoints must equal `2r`. Here chord = `1.5R − 2(s+2)` while the specified radius is `R − (s+2)`; SVG silently rescales an impossible arc down to `chord/2 = 0.75R − (s+2)`. The tick marks and drop-shadow, however, are computed with the unscaled radius, so tick lines float off the track and the glow ring is offset — visible at all four sizes (worst at `sm`: drawn r=30 vs tick r≈36).
- **Fix:** Derive `radius = (width - strokeWidth*2) / 2` and position ticks from the same value.

### M11. `copyLicenseKey` has no error handling for rejected Clipboard write
- **File:** `site/src/pages/DashboardPage.tsx:96–104`
```ts
await navigator.clipboard.writeText(key);
```
- **Why it is a bug:** `navigator.clipboard` is unavailable/rejects on non-secure origins, denied permission, or missing user gesture chains; the rejection is unhandled → uncaught promise rejection, no user feedback, and the checkmark state machinery never engages. Two call sites share this handler.
- **Fix:** try/catch with an error toast/fallback (`document.execCommand` shim or inline selectable text).

---

## LOW

### L1. `exportData` revokes the object URL synchronously after a detached-anchor click
- **File:** `site/src/pages/DashboardPage.tsx:113–127`
```ts
a.click();
URL.revokeObjectURL(url);
```
- **Why it is a bug:** Revoking immediately after `click()` races download initiation; Chrome generally tolerates it, but Firefox/Safari have historically cancelled downloads whose blob URL is revoked in the same task. Anchor is also never appended to the document (works today, not guaranteed).
- **Fix:** Append anchor, `setTimeout(() => URL.revokeObjectURL(url))`, then remove.

### L2. Copy feedback timer leaks past unmount / rapid clicks
- **File:** `site/src/pages/DashboardPage.tsx:100–102` (`setTimeout(() => setCopiedLicense(false), 2000)`)
- **Why it is a bug:** Timer isn't cleared on unmount or on subsequent copies; clicking copy twice quickly resets the flag mid-cycle (checkmark flickers off early). Minor Solid signal write after disposal is tolerated but sloppy.
- **Fix:** Store handle, clear previous timeout before setting a new one; clear in `onCleanup`.

### L3. Raw IP address rendered as "location"
- **Files:** `site/src/lib/dashboard-page.ts:getSessionLocation` consumed at `DashboardPage.tsx:1035` (`{getSessionLocation(session.ipAddress)}`)
- **Why it is a bug:** Full session IP addresses are shown verbatim in Settings; sensitive data surfaced in UI (shoulder-surfing/screen-share leak) with no masking, despite the file header's claim that session secrets are deliberately minimized.
- **Fix:** Mask (e.g. `203.0.xxx.xxx`) or geolocate server-side.

### L4. Session avatar `<img>` renders arbitrary OAuth image URL without constraints
- **File:** `site/src/pages/DashboardPage.tsx:388–394`
```tsx
<img src={props.session.user.image ?? undefined} ... />
```
- **Why it is a bug:** Provider-controlled URL loaded directly: mixed-content risk if http:, tracking pixel on every dashboard load, no `referrerpolicy="no-referrer"` / `loading="lazy"`, no error fallback if URL 404s (broken-image icon next to name).
- **Fix:** Proxy/validate image host, add `referrerpolicy`, `onerror` fallback to initials.

### L5. ProgressRing accepts negative values — negative dash-offset draws overshoot
- **File:** `site/src/design-system/components/Charts.tsx:300–303`
```ts
const progress = createMemo(() => Math.min(props.value / max(), 1));
```
- **Why it is a bug:** Only clamps the upper bound; `value < 0` yields `offset > circumference`, drawing the indicator wrapped past full-empty (browser-dependent rendering artifacts). `max()` also divides by caller-supplied `max` without guarding 0/negative (default only when falsy — `max={0}` falls back to 100, ok, but `max` prop of 0 intended as "no max" is indistinguishable).
- **Fix:** Clamp to `[0,1]` and guard `max > 0`.

### L6. Donut segments with tiny percentages render distorted by round line caps
- **File:** `site/src/design-system/components/Charts.tsx:549–561` (`stroke-linecap="round"` on all segments)
- **Why it is a bug:** Round caps extend half the stroke width beyond each arc end; slices <~thickness% visually overlap neighbors and zero-value segments with a nonzero dasharray rounding can paint dots. Legend values won't match perceived proportions.
- **Fix:** Use butt caps, or hide segments below a threshold.

### L7. Heatmap cell lookup is O(cells²)
- **File:** `site/src/design-system/components/Charts.tsx:106–109` (`getValue` linear `.find` per rendered cell)
- **Why it is a bug:** For a 24×7+ heatmap every cell scans the whole data array (~1176 finds). Wasteful though bounded; noticeable on admin dashboards re-rendering live.
- **Fix:** Build a `Map` keyed `${x}:${y}` in a `createMemo`.

### L8. Heatmap hardcodes "commands" in accessible labels/title regardless of dataset
- **File:** `site/src/design-system/components/Charts.tsx:160, 166` (`... - ${value} commands`)
- **Why it is a bug:** Component is generic (also used for sessions/heatmaps of other metrics per admin components); screen-reader users hear "commands" for non-command data — misinformation.
- **Fix:** Add a `valueLabel?: string` prop.

### L9. `HealthScoreRing` spreads unknown props onto wrapper div
- **File:** `site/src/design-system/components/HealthScore.tsx:150–156` — `splitProps` extracts only `['score','size','showLabel','animated','class']`, then `{...others}` spreads `variant`, `trend`, `showTrend` etc. as literal DOM attributes (`variant="gauge"`).
- **Why it is a bug:** Invalid attributes pollute the DOM; `showTrend`/`trend` are silently ignored in ring variant even though the public API suggests they apply.
- **Fix:** Split the full known-prop list; document trend support per variant.

### L10. Telemetry Geo latitude/longitude validated finite but not range-checked
- **File:** `site/src/hooks/telemetry-message.ts:19–26`
- **Why it is a bug:** `latitude: 999, longitude: 1e300` decodes fine and flows into maps/analytics. Boundary parsing should parse into domain-constrained types (per project standards).
- **Fix:** `Schema.Number.pipe(Schema.finite(), Schema.between(-90, 90))` etc.; likewise ISO-validate `timestamp` fields.

### L11. Malformed WS telemetry frames dropped completely silently
- **File:** `site/src/hooks/telemetry-message.ts:101–105`
- **Why it is a bug:** Returning `null` with no counter/log means systematic schema drift (e.g. the M8 'pro' tier mismatch) manifests as an empty admin realtime view with zero diagnostic signal.
- **Fix:** At least count/log rejects (without logging payload PII).

### L12. Dashboard tab strip lacks tabpanel association and keyboard roving focus
- **File:** `site/src/pages/DashboardPage.tsx:216–247` — `role="tablist"`/`role="tab"`/`aria-selected` present, but no `aria-controls`, no `id`, panels have no `role="tabpanel"`, and Arrow-key navigation (implemented properly in design-system `TabNavigation`) is absent here — duplicated, weaker tab implementation.
- **Why it is a bug:** Fails ARIA tabs pattern; keyboard users must tab through every tab button.
- **Fix:** Reuse `TabNavigation` from the design system (which already implements Home/End/arrows).

### L13. `Section` collapsible is ignored when no `title` is provided
- **File:** `site/design-system/components/layouts/DashboardLayout.tsx:150–196` — the collapse button lives inside `<Show when={props.title}>`, so `collapsible` without `title` leaves children permanently expanded with no indication.
- **Fix:** Render the toggle outside the title block or warn/type-error the combination.

### L14. Breadcrumb links bypass the router (full page reloads)
- **File:** `DashboardLayout.tsx:230–236` — plain `<a href={crumb.href}>` inside an SPA; loses client state and forces SSR round-trip.
- **Fix:** Use `@solidjs/router` `<A>`.

### L15. CSV export lacks escaping/formula-injection defense
- **File:** `src/lib/dashboard-page.ts:createTelemetryExport` (consumed by in-scope export buttons, DashboardPage.tsx:128–137, 1075+)
- **Why it is a bug:** Fields joined with bare `,` and no quoting; current fields are numeric/dates so exploitation is latent, but any future string field (hostname, package name from telemetry) enables cell-splitting corruption and Excel formula injection (`=cmd|...`). Defense should exist before the first string field lands.
- **Fix:** Proper CSV quoting and `=`/`+`/`-`/`@` prefix guard now.

### L16. Hardcoded sticky-bar background mismatches theme token
- **File:** `DashboardPage.tsx:205` — `bg-[rgba(8,11,9,0.9)]` vs `--paper:#090909`/`--bg-overlay`. Legacy color leaks through the flattened design system; visible seam against the page background.
- **Fix:** Use `bg-[color-mix(in_srgb,var(--paper)_90%,transparent)]` or a token.

### L17. Skeleton loader ignores telemetry-only loading states inconsistently
- **File:** `DashboardPage.tsx:255–283` — big skeleton shows for `loading() || telemetryLoading()`, but Quick Stats panel (`:455`) renders empty (no skeleton) whenever `telemetryLoading()` is true after initial load completes (refresh case), leaving a blank white-ish card during refresh.
- **Fix:** Per-panel loading placeholders keyed off `telemetryLoading()`.

### L18. Peak-day conditional shifts Key Insights grid layout
- **File:** `DashboardPage.tsx:520–534` — middle column of `md:grid-cols-3` exists only `when={peakDay()}`; without a peak day the remaining two items jump columns. Cosmetic/layout instability.
- **Fix:** Always render the slot with a placeholder.

### L19. `StatCard` defined inside parent component body
- **File:** `DashboardPage.tsx:147–176`
- **Why it is a bug:** Component identity changes every parent initialization; harmless in Solid's run-once model but prevents consistent reconciliation identity and bloats the parent closure. Style/perf nit.
- **Fix:** Hoist to module scope.

### L20. `toLocaleString()` output differs between SSR locale and browser locale
- **File:** `DashboardPage.tsx` multiple (`data().usage.total_commands.toLocaleString()` etc.)
- **Why it is a bug:** Node SSR locale vs user browser locale can produce different digit grouping → hydration text mismatch warnings/flicker.
- **Fix:** Format with an explicit fixed locale.

---

## INFO

### I1. Admin gating relies solely on client-side role check in-page
- **File:** `DashboardPage.tsx:952–955` (`telemetryData()?.user.role === 'admin'`). Correct only if the admin APIs themselves enforce authorization server-side (out of slice to verify). Flagged for cross-check with slice owning `routes/api/admin`.

### I2. License key rendered in full plaintext twice (overview + settings) and copied to clipboard — acceptable for owner-view, but ensure it never appears in admin-shared views/logs.

### I3. `electric-*` and `photon-*` token palettes are byte-identical duplicates of `plasma-*` (tokens.css:60–92). Dead duplication; three names for one ramp invites drift.

### I4. `AdvancedMetrics` (types/domain/metrics.ts) vs `AdminAdvancedMetrics` (types/api/admin.ts) drift: `total_users` vs `total_active_users`, optional-vs-required fields, `avg_days_inactive`/`potential_arr` present only in domain type. Two parallel contracts for the same payload will desynchronize.

### I5. `customerTag.color` default `#6366f1` (indigo) — legacy palette value; indigo now aliases to signal orange in tokens.css, so stored tag colors will not match any current palette swatch.

### I6. `app.css` reduced-motion block sets `animation-duration:0.01ms !important` globally — also neutralizes the `gauge-fill` end-state reliance? Verified safe: `both` fill-mode retains final keyframe; noted only for awareness.

### I7. `LiveIndicator` `ring` variant references `animate-[ring-expand_…]` and `bar` variant `data-pulse` keyframes; neither `@keyframes ring-expand` nor `@keyframes data-pulse` exists anywhere (tokens define `--animate-ring-expand: none`). The ring expansion and equalizer bar animations are silently static — dead visual variants (cosmetic, hence INFO not MEDIUM).

### I8. `parseTelemetryMessage` test coverage is good but lacks a case for extra unknown properties (Effect structs are exact-by-default) and for `session_end` discriminant.

### I9. `types/cloudflare.d.ts` augments `h3` context with optional `env` — every consumer must null-check `event.context.cloudflare?.env`; typed as optional so correct, but call sites elsewhere should be verified (out of slice).

### I10. `db/auth-schema.ts` session `token` stored as plaintext unique column — standard for Better Auth defaults, but consider storing only a hash of the token given DB dumps/SQL-log exposure.

---

**Totals:** 2 HIGH · 11 MEDIUM · 20 LOW · 10 INFO — **43 findings**


---

# SLICE 26

# Slice 26 — omg-web: site/shared, site/e2e, site/tools, site root configs

Audit agent: audit26 (read-only; no builds/commands executed beyond `ls`/`wc`/`read`).
Scope files fully read: `site/shared/*.ts` (5), `site/e2e/*` (6 incl. helpers + README skipped-lines check via wc only for README), `site/tools/*.mjs` (2), `site/playwright.config.ts`, `site/wrangler.toml`, `site/tsconfig.json`, `site/package.json`.

---

## MEDIUM

### M-1. `test.use({ contextOptions })` is not an effective way to set reduced motion — the option likely has no effect
- **File:** `/home/pyro1121/Documents/omg-web/site/e2e/billing-unconfigured.spec.ts`, lines 4–5
- ```ts
  test.use({ contextOptions: { reducedMotion: 'reduce' } });
  ```
- **Why a bug:** In `@playwright/test` the first-class per-test context option is `use: { reducedMotion: 'reduce' }`. `contextOptions` is only forwarded to `browserType.launchPersistentContext()` (persistent-context fixtures); it is not merged into the regular `browser.newContext()` used by standard tests. As written, the landing page's rAF canvas animation is probably *not* actually reduced, so the crash/determinism problem described in the comment above it remains live. The test may pass today by luck and fail intermittently on resource-constrained machines.
- **Fix:** `test.use({ reducedMotion: 'reduce' });` (and verify with a trace that `prefers-reduced-motion` is applied).

### M-2. `workers_dev = true` exposes the Worker on the `*.workers.dev` subdomain alongside the custom domain
- **File:** `/home/pyro1121/Documents/omg-web/site/wrangler.toml`, line 11
- ```toml
  workers_dev = true
  routes = [{ pattern = "omg.latham.cloud", custom_domain = true }]
  ```
- **Why a bug:** With both enabled, the full application (BFF, admin-session-consuming endpoints, Stripe webhook receiver) is also reachable at `omg-site.<account>.workers.dev`. That bypasses any custom-domain-level protections (WAF rules, rate limiting, bot management, cache settings) and gives attackers a second, less-monitored origin for credential-stuffing, webhook probing, and BFF abuse. It also weakens strict same-origin/CSP assumptions if any code pins the production origin but cookies are scoped loosely.
- **Fix:** Set `workers_dev = false` now that the custom domain route is active, unless the workers.dev URL is intentionally required.

### M-3. Empty-string `E2E_BASE_URL` silently produces an empty `baseURL`
- **File:** `/home/pyro1121/Documents/omg-web/site/playwright.config.ts`, lines 4–5, 12–17
- ```ts
  const externalBaseUrl = process.env['E2E_BASE_URL']?.trim();
  const baseURL = externalBaseUrl ?? 'http://localhost:3000';
  const localWebServer = externalBaseUrl === undefined ? { webServer: {...} } : {};
  ```
- **Why a bug:** If CI exports `E2E_BASE_URL=""` (a common misconfiguration), `.trim()` yields `''`, which is not `undefined`, so no local web server starts and Playwright's `baseURL` becomes `''`. Every `page.goto('/login')` then fails with a confusing protocol/URL error instead of a clear config message. Same class of issue in `e2e/staging-auth.spec.ts` line 4 where `baseUrl === undefined` gates `test.skip` — empty string passes the gate and runs against `''`.
- **Fix:** Treat falsy-after-trim as unset: `const externalBaseUrl = process.env['E2E_BASE_URL']?.trim() || undefined;`

### M-4. `typecheck` script destructively deletes the production build output
- **File:** `/home/pyro1121/Documents/omg-web/site/package.json`, line 24 (`"typecheck"`), and `/home/pyro1121/Documents/omg-web/site/tools/prepare-worker-assets.mjs`, lines 7–9
- ```json
  "typecheck": "node tools/prepare-worker-assets.mjs --clean && wrangler types ... && tsc --noEmit"
  ```
- **Why a bug:** Running the routine developer command `npm run typecheck` executes `rm -rf site/dist`. A developer who built locally (`npm run build`) to run `npm run preview` loses their artifacts with no warning; the subsequent `wrangler dev` preview then serves an empty assets directory or errors. Destructive cleanup should not be a side effect of a read-only check.
- **Fix:** Move the `--clean` reset into the CI-only path (or a dedicated `prebuild:check` step) rather than into `typecheck`.

---

## LOW

### L-1. Hydration-retry helper re-fires mutating checkout clicks (documented hazard violated)
- **File:** `/home/pyro1121/Documents/omg-web/site/e2e/helpers.ts`, lines 33–46 (doc warning), and `/home/pyro1121/Documents/omg-web/site/e2e/billing-unconfigured.spec.ts`, lines 34–41
- ```ts
  await clickUntilEffectHolds(
    () => continueButton.click(),
    ...
  ```
- **Why a bug:** The helper's own JSDoc says it "MUST be safe to repeat … a mutating action … can double-fire". `Continue to Checkout` POSTs `/api/billing/checkout`; if the first click succeeds slowly (checkout session created, response rendering delayed), the retry loop fires a second POST, creating duplicate Stripe Checkout sessions in staging. The spec partially acknowledges this ("Anonymous deployments reject…"), but nothing prevents double-fire when the backend is merely slow rather than rejecting.
- **Fix:** For non-idempotent buttons, click once inside `expect.poll`/manual wait, or guard with a one-shot flag so retries don't re-submit.

### L-2. Sitemap assertions hardcode the production origin, breaking portability of the anonymous suite
- **File:** `/home/pyro1121/Documents/omg-web/site/e2e/anonymous.spec.ts`, lines ~95–99
- ```ts
  expect(sitemapText).toContain('<loc>https://omg.latham.cloud/docs</loc>');
  ```
- **Why a bug:** This spec runs unconditionally (`test:e2e:anonymous`) including against local dev and arbitrary `E2E_BASE_URL` deployments, but pins absolute production URLs and the exact `<loc>` spelling (`/docs` without trailing slash while the page navigated to is `/docs/`). Any environment with a different domain, or a sitemap that emits `/docs/`, fails for environmental rather than behavioral reasons.
- **Fix:** Assert relative-path presence (`/docs<`) or derive the expected origin from `E2E_BASE_URL`/`page.url()`.

### L-3. `normalizeLicensingPath` does not collapse duplicate slashes or decode variants
- **File:** `/home/pyro1121/Documents/omg-web/site/shared/licensing-routes.ts`, lines 118–121
- ```ts
  export function normalizeLicensingPath(path: string): string {
    return path.endsWith('/') && path !== '/' ? path.slice(0, -1) : path;
  }
  ```
- **Why a bug:** Only a single trailing slash is stripped. `/api/dashboard//`, `/api%2Fdashboard` (if decoded upstream after resolution), or mixed-case paths fall through `resolveLicensingRoute` and `isSiteBffRoute` to whatever default handling exists downstream. Depending on how callers use `isSiteBffRoute` (allowlist gating), inconsistent normalization can cause a BFF-gated route to be treated as non-BFF (fail-closed, but breaks functionality) or vice versa if a default-open fallback exists elsewhere.
- **Fix:** Normalize aggressively (collapse `//+` → `/`, lowercase optional, reject encoded slashes before decoding) and unit-test the edge cases.

### L-4. Permissive email pattern accepts malformed addresses
- **File:** `/home/pyro1121/Documents/omg-web/site/shared/site-session.ts`, line 26
- ```ts
  const EMAIL_PATTERN = /^[^@\s]+@[^@\s]+\.[^@\s]+$/;
  ```
- **Why a bug:** Accepts consecutive dots (`a..b@x.com`), leading/trailing dots in labels, and multiple `@`-free garbage segments. Because `EmailAddress` doubles as the site-session lookup key minted by the licensing Worker, two visually distinct spellings of the same mailbox (`User@X.com` vs normalized) collapse after trim/lowercase — fine — but structurally invalid keys can be persisted and become unrecoverable/unmatchable rows later. Also note the regex runs against the *transformed* value only because of pipe ordering; a refactor could silently break that invariant since there's no test in scope pinning it.
- **Fix:** Use a stricter RFC-ish pattern (or a validated email library schema) and add round-trip tests.

### L-5. `D1Number` silently coerces NULL aggregates to 0
- **File:** `/home/pyro1121/Documents/omg-web/site/shared/d1-rows.ts`, lines 5–11
- ```ts
  export const D1Number = Schema.Union(Schema.Number, Schema.Null).pipe(
    Schema.transform(Schema.Number, { decode: (fromA) => (fromA === null ? 0 : fromA), ... })
  );
  ```
- **Why a bug:** A NULL from an unexpected column (schema drift, wrong query) is indistinguishable from a legitimate zero count, masking data-integrity bugs in dashboards (e.g., streaks, revenue metrics showing 0 instead of erroring). Documented intent mitigates severity, but there is no variant that distinguishes "NULL" from "0" for callers that care.
- **Fix:** Keep NULL→0 only for explicit aggregate schemas; for entity columns use `Schema.NullOr(Schema.Number)` and decide at the call site.

### L-6. Bundle budget checks CSS per-file only, never totals CSS
- **File:** `/home/pyro1121/Documents/omg-web/site/tools/check-bundle-budget.mjs`, lines 74–82
- **Why a bug:** JavaScript has a total budget (`MAX_TOTAL_JAVASCRIPT_GZIP_BYTES`) but stylesheets are checked per-file only; N CSS chunks each under 30 KB can total arbitrarily more with no failure. Regressions that split one CSS file into many silently bypass the stylesheet budget.
- **Fix:** Sum stylesheet gzip bytes and enforce a `MAX_TOTAL_STYLESHEET_GZIP_BYTES` analogously.

### L-7. `check-bundle-budget.mjs`: unreachable double-reporting / early-exit inconsistency
- **File:** `/home/pyro1121/Documents/omg-web/site/tools/check-bundle-budget.mjs`, lines 27–32
- ```js
  } catch {
    reportFailure(...);
    process.exit(1);
  }
  ```
- **Why a bug:** `reportFailure` already sets `process.exitCode = 1`; the immediate `process.exit(1)` is redundant but also skips any future cleanup hooks and makes the two failure paths behave differently (hard exit vs. soft exit-code). Minor, cosmetic/robustness only.
- **Fix:** Drop `process.exit(1)` and rely on `exitCode`, consistent with the rest of the script.

### L-8. Staging specs accept whitespace-only credentials
- **File:** `/home/pyro1121/Documents/omg-web/site/e2e/staging-auth.spec.ts`, lines 4–8, 19–22
- ```ts
  const userEmail = process.env['E2E_USER_EMAIL']?.trim();
  test.skip(baseUrl === undefined || userEmail === undefined || userPassword === undefined, ...)
  ```
- **Why a bug:** `E2E_USER_PASSWORD=" "` passes the skip gate; `performUiLogin(page, userEmail ?? '', userPassword ?? '')` then hammers the real staging login endpoint with empty/whitespace credentials for 30 s per attempt across retries, polluting auth logs and potentially tripping rate limits/lockouts on shared accounts.
- **Fix:** Skip unless all values are truthy after trim.

### L-9. `Origin` header spoofing in staging dashboard request weakens what the test proves
- **File:** `/home/pyro1121/Documents/omg-web/site/e2e/staging-auth.spec.ts`, lines 31–35
- ```ts
  const dashboardResponse = await page.request.get('/api/licensing/api/dashboard', {
    headers: { Origin: baseUrl ?? '' },
  });
  ```
- **Why a bug:** Manually injecting `Origin` means the test verifies the BFF accepts a *self-declared* same-origin header — exactly what an attacker's script can do outside a browser. Fetch Metadata / Origin checks are anti-CSRF heuristics for browser contexts only; the test documents this but presents it as exercising "the BFF's same-origin read policy," which overstates the protection. Not exploitable by itself, but the assertion could mask removal of genuine browser-side protections (Sec-Fetch-Site enforcement would never be caught here).
- **Fix:** Prefer driving the fetch through the page context (`page.evaluate(fetch(...))`) so real browser headers apply.

### L-10. `preview` script and Playwright dev server collide on port 3000
- **File:** `/home/pyro1121/Documents/omg-web/site/package.json`, lines 15 & 21
- ```json
  "dev": "vinxi dev",
  "preview": "wrangler dev --config wrangler.toml --port 3000",
  ```
- **Why a bug:** `playwright.config.ts` defaults to `http://localhost:3000` and spawns `npm run dev`. If a developer has `npm run preview` (wrangler, port 3000) running, the E2E run silently tests against the *Worker preview* (built assets, possibly stale) because of `reuseExistingServer: !CI` semantics — the URL responds, so vinxi never starts. Tests then exercise a different runtime than intended with no warning.
- **Fix:** Use distinct ports (e.g., wrangler on 8787) or make the webServer probe for the vinxi process specifically.

### L-11. `tsconfig.json` disables unused-symbol checks project-wide
- **File:** `/home/pyro1121/Documents/omg-web/site/tsconfig.json`, lines 18–19
- ```json
  "noUnusedLocals": false,
  "noUnusedParameters": false,
  ```
- **Why a bug:** Dead code (unused handlers, stale parsers) accumulates invisibly in shared boundary modules — precisely the drift-prone files in this slice. Everything else in the config is strict, making these two flags look like leftovers rather than decisions.
- **Fix:** Enable both and fix the (likely small) fallout; suppress case-by-case with `_`-prefixed params.

---

## INFO

### I-1. `licensing-routes.ts`: many sensitive routes declared `authentication: 'none'`
- **File:** `/home/pyro1121/Documents/omg-web/site/shared/licensing-routes.ts`, lines 39–77 (e.g. `validateLicensePost`, `reportUsage`, `cliEvent`, `cliBatch`, `analytics`, `siteAnalyticsTrack`)
- Unauthenticated write endpoints (usage reporting, CLI events, analytics ingestion) are spam/flood surface; the contract itself doesn't enforce any rate-limit metadata. Presumably guarded in the Worker; flagging so reviewers confirm ingestion endpoints have throttling/abuse controls.

### I-2. `LicensingDashboardSchema.invoices` typed `Schema.Array(Schema.Unknown)`
- **File:** `/home/pyro1121/Documents/omg-web/site/shared/licensing-dashboard.ts`, line ~86
- Untyped invoice rows defeat the boundary-parsing architecture; consumers must re-parse or will hit runtime shape errors. Suggest a minimal invoice struct (id, amount, status, created).

### I-3. `subscription.cancel_at_period_end` modeled as `Schema.Number` (0/1) rather than Boolean
- **File:** `/home/pyro1121/Documents/omg-web/site/shared/licensing-dashboard.ts`, lines ~80–85
- D1 INTEGER leakage into a UI-facing contract; every consumer must remember `=== 1`. A transform to Boolean would centralize it.

### I-4. `SiteSessionParseError` duplicates Effect Schema error info
- **File:** `/home/pyro1121/Documents/omg-web/site/shared/site-session.ts`, lines 10–19
- Wrapping parse failures in a custom Error discards field-level detail (which property failed) unless `cause` is inspected; log consumers get only generic messages like "invalid shape". Consider tagging with the failing path via `Schema.ParseError` tree formatting.

### I-5. Hardcoded D1 `database_id` in wrangler.toml
- **File:** `/home/pyro1121/Documents/omg-web/site/wrangler.toml`, line 31
- Database IDs are not secrets but are account-reconnaissance aids if the repo is public; acceptable, noted for completeness.

### I-6. `compatibility_date = "2026-08-21"` must not exceed deploy-time wrangler's supported date
- **File:** `/home/pyro1121/Documents/omg-web/site/wrangler.toml`, line 4
- If any environment pins an older wrangler, deploys fail with "compatibility_date is ahead". Version-pinned wrangler (4.125.0) makes this consistent today; flagged as upgrade friction only.

### I-7. Aggressive npm `overrides` pinning transitive deps
- **File:** `/home/pyro1121/Documents/omg-web/site/package.json`, lines 62–72
- Overriding `h3`, `nitropack`, `tar`, etc. can desynchronize from vinxi/SolidStart expectations after upgrades (build breakage or subtle behavior changes). Each override needs a documented review trigger; currently none annotated in-file.

### I-8. `marketing-offer.ts` expiry timestamp pattern is loose
- **File:** `/home/pyro1121/Documents/omg-web/site/shared/marketing-offer.ts`, line ~20
- `Schema.pattern(/^\d{4}-\d{2}-\d{2}T/u)` accepts `0000-00-00Tgarbage`. Cosmetic for display; parse with a date schema if the value drives logic.

### I-9. `performUiLogin` refills password fields on every retry pass
- **File:** `/home/pyro1121/Documents/omg-web/site/e2e/helpers.ts`, lines 60–76
- Intentional per comments, but each pass submits real credentials again; against staging with flaky hydration this can generate several rapid failed logins and trip lockout/rate-limit on the shared test account. Consider asserting a distinct "already submitted" state to stop early.

### I-10. e2e README not audited line-by-line for content accuracy
- **File:** `/home/pyro1121/Documents/omg-web/site/e2e/README.md`
- Documentation only; skimmed for existence. No code impact.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH     | 0 |
| MEDIUM   | 4 |
| LOW      | 11 |
| INFO     | 10 |
| **Total**| **25** |

Most notable items: ineffective `reducedMotion` test option (M-1), `workers_dev = true` origin bypass (M-2), destructive `typecheck` cleaning `dist` (M-4), and the double-fire checkout POST risk in the hydration helper (L-1).


---

# SLICE 27

# Audit slice-27 — omg-web `site/workers/src/handlers/`

Scope: `account-dashboard.ts`, `admin.ts`, `auth.ts`, `billing.ts`, `dashboard.ts` (read-only audit; supporting contracts in `contracts/`, `api.ts`, `admin-auth.ts`, `shared/d1-rows.ts` consulted for verification).

---

## CRITICAL

### C1. Webhook customer.created stores raw-case email → duplicate customer identities / lost entitlements
**File:** `billing.ts:828–860` (handleStripeWebhook, `customer.created` case), interacting with `auth.ts` `findOrCreateCustomer` and `lookupStripeCustomerId`.
```ts
await env.DB.prepare(
  `INSERT INTO customers (id, stripe_customer_id, email, company, created_at)
   VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)`
).bind(crypto.randomUUID(), stripeCustomer.id, stripeCustomer.email, ...)
```
Why it's a bug: the OTP auth path (`findOrCreateCustomer`) and every dashboard/billing lookup normalize emails to lowercase (EmailAddress schema trims+lowercases; `lookupStripeCustomerId` binds lowercased email). Stripe `customer.email` is stored **raw**, so a Stripe customer with e.g. `User@Example.com` will never match a lowercase lookup. Consequences:
- A paying user logging in via OTP gets a *second* customers row with the lowercase email; their paid license (attached to the raw-case row) is invisible to their dashboard session.
- `lookupStripeCustomerId(env.DB, email.toLowerCase())` misses → "No billing account found" / fresh checkout mints another pending Stripe customer.
Suggested fix: store `stripeCustomer.email.trim().toLowerCase()` on insert (and in admin sync UPDATE), or compare with `LOWER(email) = LOWER(?)`.

### C2. `resolveStripeCustomerId` can bind `undefined` → D1 throws → webhook event retries forever
**File:** `billing.ts:150–176` (`resolveStripeCustomerId`), called at `billing.ts:905` / `:1090`.
```ts
async function resolveStripeCustomerId(db, stripeCustomerId: string | null | undefined) {
  const customerRow = await db
    .prepare('SELECT id FROM customers WHERE stripe_customer_id = ?')
    .bind(stripeCustomerId) // undefined throws in D1
```
Why it's a bug: the webhook invoice shape (`contracts/stripe.ts:42`) declares `customer: Schema.optional(Schema.Union(Schema.Null, Schema.String))`. If an invoice event carries `parent.subscription_details.subscription` but no `customer` field, `.bind(undefined)` throws a TypeError. The throw escapes to the handler's catch and marks the event `failed`; Stripe redelivers, fails identically — a permanent poison-message retry loop (also burns the 5-minute inbox lease repeatedly).
Suggested fix: guard `stripeCustomerId == null` up front and return `{ ok: false, reason: 'unlinked' }`, or bind `stripeCustomerId ?? null`.

---

## HIGH

### H1. Uncaught D1/store exceptions escape most dashboard handlers (inconsistent error handling)
**Files:** `dashboard.ts` — `handleUpdateProfile` (:96–113), `handleRegenerateLicense` (:116–140), `deactivateMachine`/`handleRevokeMachine` (:143–193), `handleGetSessions` (:196–226), `handleRevokeSession` (:229–263), `handleGetAuditLog` (:438–476).
Why it's a bug: only `handleGetTeamMembers` wraps its body in try/catch. Every other handler awaits D1 calls bare; a transient D1 failure rejects the promise returned by `withDashboardSession`, escaping as an unhandled Worker exception (HTML 1101 error page instead of JSON, no audit trail, no observability report except generic worker logs). Also `logAudit` via `Effect.runPromise` can reject.
Suggested fix: route all handlers through a shared wrapper that catches, `reportError`s, and returns `errorResponse('Internal server error', 500)`.

### H2. Checkout-session status endpoint leaks payment status of other users' sessions when email is absent
**File:** `billing.ts:566–585` (`handleCheckoutSessionStatus`).
```ts
const email = session.customer_details?.email ?? session.customer_email ?? null;
if (email !== null && email.toLowerCase() !== auth.user.email.toLowerCase()) { ... 403 ... }
```
Why it's a bug: if the fetched session has no resolvable email (e.g., completed guest/Alias checkout where customer_details were not retained), the ownership check is skipped entirely and the endpoint happily reports `{ status: 'paid', license: <key> }` for any `cs_…` id whose license happens to link to the caller — but more importantly it confirms payment status of arbitrary sessions without proof of ownership (the license join does protect cross-account license disclosure, but status probing of third-party sessions is unauthenticated by identity). Session ids are high entropy so exploitability is low, but the check should be fail-closed.
Suggested fix: require positive proof of ownership (email match OR `session.client_reference_id`/metadata userId match); return 403 when neither is present.

### H3. `enforceIpRateLimit` limiter call is unwrapped — binding failure crashes instead of failing closed
**File:** `auth.ts:497–512`.
```ts
const { success } = await env.AUTH_RATE_LIMITER.limit({ key: `${scope}:${ip}` });
```
Why it's a bug: unlike `checkRateLimitBucket` (which wraps the same call in `Effect.tryPromise`), this adapter-level call has no try/catch. A rate-limiter binding error rejects out of `handleSendCode`/`handleVerifyCode`/etc. before the Effect pipeline runs, producing an unhandled Worker exception rather than the intended 503 fail-closed response.
Suggested fix: wrap in try/catch and return the 503 `errorResponse` used for the missing-binding case.

---

## MEDIUM

### M1. `longest_streak` is just the current streak
**File:** `account-dashboard.ts:362` (`longest_streak: streak`).
Why it's a bug: the API contract advertises a longest streak; returning the current streak understates/misstates history (resets to 0 after one missed day even if the user had a 200-day streak). Pure data defect surfaced to the UI.
Suggested fix: compute longest streak over stored daily rows (window/CTE query or scan), keep `current_streak` separate.

### M2. Streak capped silently at 60 days by LIMIT
**File:** `account-dashboard.ts:237–243` + query `LIMIT 60`.
Why it's a bug: streak loop iterates `streakDates` which is truncated to 60 rows, so any streak ≥ 61 days displays as exactly ~60. Off-by-design off-by-one style defect.
Suggested fix: iterate until first gap regardless of LIMIT, or raise LIMIT and stop early on gap.

### M3. Dashboard top-package/top-runtime swallow real store failures into fake defaults
**File:** `account-dashboard.ts:297–334`.
```ts
.pipe(Effect.catchAll(() => Effect.succeed(null)) ... 'ripgrep')
```
Why it's a bug: `DashboardStoreUnavailable` (a genuine outage) is indistinguishable from "no usage data"; during partial D1 failures users are shown fabricated "global stats" (ripgrep/node) presented as real. Also swallows parse errors of malformed persisted rows, hiding data corruption.
Suggested fix: return nullable fields (`top_package: null`) and let the client render "unavailable", or only default on empty-result, not on any error.

### M4. Percentile rank counts all licenses ever seen, including inactive/expired
**File:** `account-dashboard.ts:335–357`.
Why it's a bug: `usage_daily GROUP BY license_id` includes stale licenses with tiny totals, inflating everyone's percentile; combined with `total_users` from `COUNT(DISTINCT license_id)` the metric is marketing-grade fiction that also double-counts licenses belonging to the same customer. Not security, but a correctness defect in a user-facing number.
Suggested fix: bound both queries to active licenses and recent activity windows.

### M5. Leaderboard exposes cross-customer PII (3-char email prefixes)
**File:** `account-dashboard.ts:360–374`.
```sql
SELECT SUBSTR(c.email, 1, 3) || '***' as user ...
```
Why it's a bug: every authenticated user receives email prefixes of arbitrary other customers (joined across all licenses). Three chars plus domain knowledge makes identification trivial for small user bases; it is cross-tenant data exposure from an authenticated endpoint. Also `time_saved` per rival is leaked.
Suggested fix: gate behind opt-in, use fully opaque handles, or drop the leaderboard.

### M6. `loadLicenseId` picks an arbitrary license when a customer has several
**File:** `dashboard.ts:73–84` (`SELECT id FROM licenses WHERE customer_id = ?` — no ORDER BY), also `handleGetTeamMembers` :252 and `handleGetAuditLog` tier check.
Why it's a bug: with multiple license rows (webhook projection + manual admin grants can create them), machine revocation, team listing, and tier gating operate on a nondeterministic license. A free-tier license could shadow the paid tier for the audit-log/team checks (authz-relevant) or revoke machines of the wrong license.
Suggested fix: deterministic ordering (paid tiers / newest active first) or enforce single-active-license invariant.

### M7. Invoice upsert race: `ON CONFLICT (id)` doesn't cover the `stripe_invoice_id` unique key
**File:** `billing.ts:452–486` (`buildInvoiceUpsert`).
```sql
VALUES (COALESCE((SELECT id FROM invoices WHERE stripe_invoice_id = ?), ?), ...) ON CONFLICT (id) DO UPDATE ...
```
Why it's a bug: two concurrent ingestions (webhook + admin sync) can both miss the SELECT and insert different UUID ids for the same `stripe_invoice_id`. If the schema declares UNIQUE(stripe_invoice_id), the second insert raises an unhandled constraint error (webhook marked failed/retried; sync item errors). The conflict clause targets the wrong key because the id is computed in SQL, not derived from stripe id. Suggest `INSERT ... ON CONFLICT (stripe_invoice_id) DO UPDATE` with id defaulted, or a two-statement batch inside `db.batch` (implicit transaction).

### M8. `verifyCode` invalidates ALL unused codes for the email after success — multi-device login race
**File:** `auth.ts:395–403`.
```ts
env.DB.prepare(`UPDATE auth_codes SET used = 1 WHERE email = ? AND used = 0`)
```
Why it's a bug: a second concurrent legitimate login (or an in-flight resend) has its freshly minted code invalidated mid-flow; the user sees "Invalid or expired code". Minor UX/race defect; also means resend-then-use-old-tab flows break non-deterministically.

### M9. OTP pepper reuses `JWT_SECRET`
**File:** `auth.ts:186` (`digestOtpCode(body.email, code, env.JWT_SECRET)`).
Why it's a bug: one secret now guards two independent concerns. Rotating the JWT signing secret silently invalidates all outstanding OTPs (availability), and any future exposure path of JWT_SECRET (e.g., a signing-key debug feature) directly compromises stored OTP digests. Cheap fix, meaningful hygiene gain.
Suggested fix: dedicated `OTP_PEPPER` binding.

### M10. Send-code rate limit counts codes that were never delivered
**File:** `auth.ts:243–276`.
Why it's a bug: the code row is inserted into `auth_codes` *before* `mailer(...)` runs. If email delivery fails (EmailDeliveryFailed), the failed attempts still count toward the "≥3 in 10 minutes" throttle — a user with a flaky mailbox provider gets locked out of login entirely for 10 minutes after three delivery failures, with no admin signal distinguishing lockout from outage.
Suggested fix: decrement/purge the inserted row when mailer fails, or only count rows whose send succeeded.

### M11. `handleAdminStripeSync` has no rate limit / timeout budget
**File:** `billing.ts:1000–1105`.
Why it's a bug: unlike checkout/status endpoints (which use `API_RATE_LIMITER`), the admin sync performs unbounded pagination over three Stripe list endpoints plus per-item D1 round trips (N+1 pattern). Large accounts exceed the Workers CPU/duration budget mid-sync leaving partially synced state and an error-only result; repeated clicks parallelize full Stripe scans.
Suggested fix: add limiter + cursor-based incremental sync or background queue.

### M12. CSV formula-injection neutralization misses space-prefixed cells
**File:** `admin.ts:180–192` (`escapeCSV`).
```ts
const isFormulaLike = /^[=+\-@\t\r]/.test(text);
```
Why it's a bug: Excel/LibreOffice evaluate formulas after stripping leading whitespace in some locales/import paths (`"  =cmd|..."`), and leading `'` quoting itself is the documented mitigation — but cells beginning with a space then `=` are not flagged. Customer-controlled strings (company names, emails, note content) flow into exports. Low practical impact since fields are mostly admin-entered, but the guard is advertised as complete and isn't.
Suggested fix: test `/^\s*[=+\-@\t\r]/` (and consider Unicode `\u0009` variants).

---

## LOW

### L1. `max_machines` preference order is wrong (`max_seats` wins over `max_machines`)
**File:** `account-dashboard.ts:381` — `license.max_seats ?? license.max_machines ?? 1`.
Why it's odd: seats (users) and machines are different concepts elsewhere (`TIER_FEATURES.free.max_machines = 1` vs team 10); preferring `max_seats` can under-report machine capacity for seat-based tiers. Verify intent; likely should be `max_machines ?? max_seats`.

### L2. Session-prune SQL uses template-string interpolation (constant, but pattern risk)
**File:** `auth.ts:431–441`.
```ts
... LIMIT ${MAX_SESSIONS_PER_CUSTOMER}
```
Safe today (module constant integer), but string interpolation into SQL is the pattern the rest of the file carefully avoids. Use `.bind(limit)` or a literal constant comment-enforced.

### L3. `handleUpdateProfile` audits success even when nothing was written, and treats empty string as nulling company
**File:** `dashboard.ts:99–108`. `decoded.value.name || null` silently maps `""` to clearing the company name; audit log fires unconditionally. Minor semantics/UX.

### L4. Team compliance/version-drift math compares against lexicographically-"largest" version string, not actual latest release
**File:** `dashboard.ts:344–352`.
```ts
right.localeCompare(left, undefined, { numeric: true })
```
Why it's a bug: with prerelease tags ("1.10.0-beta" > "1.9.0" numerically but semantically older; "unknown" sorts above many numeric versions), `latest_version` and `compliance_rate` can be wrong, mislabeling a fully-updated fleet as drifting (or vice versa). Cosmetic-to-misleading analytics.

### L5. `roi_multiplier` type inconsistency: string `'0'` vs formatted string otherwise
**File:** `dashboard.ts:432–435`. Returns `(totalValueUSD / ROI_BASELINE_COST_USD).toFixed(1)` (string) or `'0'`; consumers must handle both; `'0'` lacks the `.0` formatting. Trivial but real schema wobble.

### L6. `checkoutIdentity` daily rotation can block a deliberate second purchase same-day and collides across offers sharing a price
**File:** `billing.ts:508–528`. Idempotency key is hash(userId, offer, promo, day): if the first attempt created a session the user abandoned, a genuine retry within the same day reuses the same (possibly expired) Checkout Session via Stripe idempotency, and the user cannot start a fresh one until tomorrow. Deliberate trade-off per comments, but worth flagging as UX-affecting.

### L7. `fetchStripeJson` decodes HTTP-error payloads against success schemas → all Stripe API errors collapse to generic 502/`null`
**File:** `billing.ts:70–98` with `handleCreateCheckout:569–600`. The `session.error` branch at `billing.ts:603–605` is effectively dead unless `StripeCheckoutSessionSchema` explicitly permits an `error` field on non-2xx bodies (it decodes the *success* schema); real causes (invalid price id, key revoked) surface as opaque "Failed to create checkout session", hurting diagnosability. Suggest decoding error envelopes for non-2xx statuses.

### L8. `cleanupStripeEvents` deletes by `processed_at` but processed rows also have `event_data=''`; failed rows accumulate forever if a poison event keeps failing
**File:** `billing.ts:415–431`. Failed events are intentionally kept, but there is no cap or alert on their growth; a permanently failing event retried by Stripe for 3 days is fine, yet events that fail *after* Stripe gives up stay in `failed` state indefinitely (never pruned, never re-driven). Add retention for aged failed rows or an operator alert.

### L9. Admin metrics MRR ignores non-USD subscriptions entirely while ARR derives from USD-only MRR
**File:** `billing.ts:1300–1320` (`monthlyNormalizedCents` + currency filter). Correctly avoids mixing currencies (good), but the payload presents `mrr`/`arr` unlabeled as USD-only while `active_subscriptions` includes all currencies — numbers that won't reconcile for anyone auditing. Label or convert.

### L10. `computeMrr` in admin store view silently prices unknown tiers at 0
**File:** `admin.ts:117–125`. Any new tier value added to the DB (e.g., via webhook projection drift) contributes zero revenue with no warning; dashboards under-report. Log/warn on unknown tier keys.

### L11. `parsePaginationParam` accepts absurd page sizes up to MAX_PAGE_SIZE but pages up to 10,000 × offset arithmetic can still be heavy
**File:** `admin.ts:80–95`. Clamps exist (good); noting OFFSET depth 10,000×100 remains a potentially expensive scan on D1 per request. Consider keyset pagination. INFO-level hardening.

### L12. `withAdminBody` discards decode-failure cause
**File:** `admin.ts:154–167`. All body-validation failures become identical "Invalid JSON body" 400s with nothing logged; legitimate client bugs vs. attack probes are indistinguishable. Log reason server-side.

### L13. Logout requires valid token before deleting it; expired-but-held tokens are never cleaned from DB
**File:** `auth.ts:656–689`. `validateSession` filters `expires_at > datetime('now')`, so calling logout with an already-expired token skips deletion; expired session rows linger until some other cleanup. Combine with the prune-on-login sweep.

### L14. `handleCheckoutSessionStatus` ownership relies on `OptionalStripeReferenceId` declared far below its use site
**File:** `billing.ts:589` vs declaration `billing.ts:655`. Works (TDZ resolved at call time after module eval) but fragile ordering; move the const above its consumers.

### L15. `escapeCSV` BOM + LF line endings
**File:** `admin.ts:195–206`. RFC 4180 expects CRLF; rows joined with `\n`. Most tools cope; strict parsers may not. Cosmetic spec deviation.

---

## INFO

### I1. `deactivateMachine` interpolates column name into SQL
**File:** `dashboard.ts:158–166`. `${whereColumn}` is from a closed union type ('machine_id'|'id') so not injectable today; keep it that way or use two static statements to make safety local.

### I2. `brandGeneratedId` throws synchronously outside any try — becomes an Effect defect mapped to generic 500
**File:** `auth.ts:157–160, 407`. Behavior is safe (defect channel), but the generic message hides the actual failure from ops.

### I3. `requireTurnstile` is enforced unconditionally on send-code
**File:** `auth.ts:213–240`. Fail-closed when TURNSTILE_SECRET_KEY missing (good); note this makes the whole auth flow dependent on Turnstile availability (mapped to 503) — intentional, documented.

### I4. `claimStripeEvent` 5-minute lease acknowledged in comments
**File:** `billing.ts:300–350`. Handler exceeding lease could double-process after reclaim; reconciliation paths are idempotent so impact contained. Keep handler runtimes well under 5 min.

### I5. Processed webhook rows retain `attempt_count`, `status` metadata forever until 90-day prune
**File:** `billing.ts:380–398, 415–431`. Fine; noted for capacity planning.

### I6. `secureJsonResponse` sets strong no-store headers on all admin responses
**File:** `admin.ts:11–24`. Good practice; no finding.

### I7. `handleGetSessions` returns peer session IPs/user-agents to account owner
**File:** `dashboard.ts:196–226`. Standard security-feature behavior (session review); acceptable.

### I8. Dead/stub endpoints: `handleGetTeamPolicies`, `handleGetNotifications` return 501
**File:** `dashboard.ts:520–538`. Intentional stubs; ensure clients don't poll them (wasted auth'd round trips).

### I9. `verifyStripeSignature` duplicate-`t` headers take first value; extra v1 candidates all compared timing-safely
**File:** `billing.ts:333–412`. Implementation is sound; noted for reviewers.

### I10. `sendVerificationCode` piggyback DELETE sweeps all expired auth codes on every request
**File:** `auth.ts:255–276`. Global write amplification under load; consider scheduled job.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH | 3 |
| MEDIUM | 12 |
| LOW | 15 |
| INFO | 10 |
| **Total** | **42** |

Highest-priority actions: fix raw-case Stripe email storage (C1) before it silently forks paying customers' accounts; guard the possibly-undefined `customer` bind in webhook invoice handling (C2); wrap remaining dashboard handlers in structured error handling (H1); make the checkout-status ownership check fail-closed (H2).


---

# SLICE 28

# Audit slice-28 — omg-web `site/workers/src/handlers/` (docs-analytics, firehose, github-proxy, license, marketing-offer)

Read-only audit. All findings verified against source in `/home/pyro1121/Documents/omg-web/site/workers/src/` plus supporting contracts (`contracts/http-bodies.ts`, `contracts/license-ops.ts`, `body.ts`, `api.ts`, `telemetry-policy.ts`, `worker.ts` routing).

---

## docs-analytics.ts

### DA-1 · MEDIUM — Unvalidated JSON property values bound directly as SQL parameters can abort the whole batch
- **File:** `site/workers/src/handlers/docs-analytics.ts:133–138`
```ts
props['utm']?.source || null,
props['utm']?.medium || null,
props['utm']?.campaign || null,
props['referrer'] || null,
props['url'] || null,
```
- **Why:** `DocsAnalyticsEventSchema.properties` is a free-form `JsonObject` (`contracts/http-bodies.ts:115`) whose values may be objects, arrays, booleans or numbers. If a client sends `"url": {"a":1}` (or an array), the raw object/array is passed to D1 `.bind()`, which throws on non-scalar binds. One poisoned event fails `env.DB.batch()` for the entire batch → 500 for all events in it. Same for `referrer`. The aggregate SQL later does `JSON_EXTRACT(properties,'$.url')` assuming a string path.
- **Fix:** Coerce with the existing `optionalStringField` helper (used elsewhere in this codebase) before binding: `optionalStringField(props['url']) ?? null`.

### DA-2 · MEDIUM — Client-controlled `timestamp` breaks aggregation and allows future/back-dated poisoning
- **File:** `site/workers/src/handlers/docs-analytics.ts:94, 111, 131–132, 150, 221`
- **Why:** `event.timestamp` is only checked for truthiness and max length 40 chars; format is never validated. It is inserted verbatim into `docs_analytics_events.timestamp` and `docs_analytics_sessions.first_seen_at/last_seen_at`. Aggregation filters `DATE(timestamp) = ?` where `?` is the *server's* today (line 150/221), so:
  1. Events legitimately timestamped near midnight UTC, or by clients with wrong clocks, are stored but never aggregated (silent data loss in dashboards).
  2. An attacker can submit arbitrary/future timestamps to pollute session tables (`first_seen_at` far past) and skew `docs_analytics_sessions` retention windows.
- **Fix:** Validate `timestamp` against an ISO-8601 schema at decode time (or ignore client timestamps and use server time / clamp to a sane window around now).

### DA-3 · MEDIUM — `docs_analytics_performance_daily` is queried but never written anywhere (dead dashboard feature)
- **File:** `site/workers/src/handlers/docs-analytics.ts:336–344` (dashboard performance query); ingest loop at lines ~100–145 handles only `pageview` and session upserts.
- **Why:** Grep across the whole worker confirms no code inserts into `docs_analytics_performance_daily`. The admin dashboard's "performance" section can therefore never show data — dead/broken feature, and the perf event type is silently accepted then dropped at ingest.
- **Fix:** Either implement web-vitals ingestion into that table or remove the query + `DocsPerformanceRowSchema` plumbing.

### DA-4 · MEDIUM — Aggregate refresh re-scans the full day's events on every ingest request
- **File:** `site/workers/src/handlers/docs-analytics.ts:151–229`
- **Why:** Every batch schedules five `GROUP BY` queries over all of today's events via `ctx.waitUntil`. Under real traffic this is O(requests × daily-row-count) D1 work per day and will degrade badly late in a high-traffic day (D1 row-read billing + latency). A safer design increments aggregates incrementally from the just-inserted rows.
- **Fix:** Incrementally upsert aggregates computed from `body.events`, or gate full-day recompute behind a schedule/cron.

### DA-5 · LOW — Shared `'unknown'` rate-limit bucket for requests without `CF-Connecting-IP`
- **File:** `site/workers/src/handlers/docs-analytics.ts:58`
```ts
const ip = request.headers.get('CF-Connecting-IP') || 'unknown';
```
- **Why:** Inconsistent with `handleValidateLicense` (license.ts:370) which correctly uses `crypto.randomUUID()` fallback so headerless clients can't exhaust one shared bucket. Here, if the header is ever absent (e.g., service-binding/internal calls, misconfig), all such traffic shares a single limiter key → mutual DoS.
- **Fix:** Use `?? crypto.randomUUID()` like license.ts does.

### DA-6 · LOW — Unweighted average-of-averages for `avg_time` on top pages
- **File:** `site/workers/src/handlers/docs-analytics.ts:296–300`
```sql
SELECT path, SUM(views) ..., AVG(avg_time_on_page_ms) as avg_time
FROM docs_analytics_pageviews_daily ... GROUP BY path
```
- **Why:** `avg_time_on_page_ms` is itself a per-day mean; `AVG()` over days gives each day equal weight regardless of view count — statistically wrong when traffic varies by day. Should be `SUM(views * avg_time_on_page_ms)/SUM(views)`.

### DA-7 · INFO — Public unauthenticated write endpoint
- **File:** `docs-analytics.ts:44` (`handleDocsAnalytics`); route at `worker.ts:250`.
- **Why:** Anyone can POST analytics events (only IP rate limiting). This appears intentional for a public docs site, but it means the entire docs-analytics dataset (sessions, pageviews, geo, referrers) is attacker-writable noise within rate limits. Consider origin/referer checks or a lightweight signed beacon token if dashboard fidelity matters.

### DA-8 · INFO — Stale comment in `cleanupDocsAnalytics`
- **File:** `docs-analytics.ts:462–464`
- **Why:** Comment explains "an ISO string cutoff deletes up to a day early due to ' ' vs 'T' ordering" while the code below correctly uses `datetime('now','-7 days')`. The rationale comment describes the bug it fixed; fine, but worth noting the ISO-vs-SQLite-text subtlety applies to any future editor reverting to ISO cutoffs.

---

## firehose.ts

### FH-1 · LOW — Lax `limit` parsing accepts trailing garbage
- **File:** `firehose.ts:47–49`
```ts
const requestedLimit = Number.parseInt(url.searchParams.get('limit') ?? '', 10);
const limit = Number.isFinite(requestedLimit) && requestedLimit >= 1 ? Math.min(requestedLimit, 100) : 50;
```
- **Why:** `limit=50abc` parses as 50 and is accepted rather than rejected; `limit=0x10` → 0 → default. Not exploitable (result still clamped ≤100), but inconsistent with strict parsing elsewhere. Note `Number.isFinite(parseInt(...))` is always true unless NaN, so the check reduces to NaN-only.
- **Fix:** Strictly validate `/^\d{1,3}$/` before parsing.

### FH-2 · INFO — No upper pagination cursor; `since` filter is strictly `>`
- **File:** `firehose.ts:22, 62`
- **Why:** Clients paginate backwards using `since=oldest_seen_created_at`; because `created_at` has second resolution and batches share timestamps, rows sharing a timestamp straddling a page boundary are skipped by `>`. Potential missed events in the admin feed. Fix: tie-break on `id` or use `>=` + dedupe.

(Auth is correctly enforced both by route gating and `forbiddenUnlessAdminSession`.)

---

## github-proxy.ts

### GP-1 · MEDIUM — Unauthenticated GitHub API call: shared 60 req/hour budget causes user-visible outages
- **File:** `github-proxy.ts:128–135`
```ts
ghResponse = await fetch(GITHUB_COMMIT_ACTIVITY_URL, { headers: { Accept: ..., 'User-Agent': ... } });
```
- **Why:** No `Authorization` header, so every Worker isolate worldwide shares Cloudflare egress IPs' anonymous GitHub limit (60/hr per IP). Once exhausted, all visitors get 502 until cache/stale windows expire. A repo-scoped fine-grained token (or GitHub Actions-generated static JSON served from R2/KV) would decouple the public site from GitHub rate limits.
- **Fix:** Add a stored secret token or serve precomputed stats.

### GP-2 · LOW — Upstream response body never cancelled on early-return paths (202 / !ok / invalid payload)
- **File:** `github-proxy.ts:146–172`
- **Why:** On `status === 202`, `!ghResponse.ok`, and JSON/schema failure branches, `ghResponse.body` is neither read nor `cancel()`ed. In Workers this keeps the upstream stream/socket alive until GC, wasting connections and potentially delaying the isolate. The codebase already demonstrates correct handling (`body.ts:60`: `await reader.cancel().catch(...)`).
- **Fix:** Call `ghResponse.body?.cancel().catch(() => {})` on every non-consumed path (use try/finally).

### GP-3 · LOW — Cache stampede on expiry/stale boundary
- **File:** `github-proxy.ts:88–101`
- **Why:** When the entry goes stale, every concurrent request across isolates sees STALE and each schedules its own `refreshCache` (plus synchronous refetch after `STALE_TTL`). No soft-lock/dedupe. With GP-1's tiny anonymous quota this multiplies wasted upstream calls exactly when they are scarcest.
- **Fix:** Use a cache-API lock key or `ctx.waitUntil` single-flight marker.

### GP-4 · INFO — `X-RateLimit-Remaining` exposed to clients on MISS
- **File:** `github-proxy.ts:206–212`
- **Why:** Minor internal-state disclosure (how close the proxy is to GitHub throttling); harmless but inconsistent with the care taken elsewhere not to leak provider state ("Keep the real upstream status server-side", line 167).

---

## marketing-offer.ts

### MO-1 · MEDIUM — Orphaned Stripe promotion codes on network-failure retries (idempotency key changes per attempt)
- **File:** `marketing-offer.ts:118–121, 168–171, 247–258`
- **Why:** Each claim attempt mints fresh `leadId = crypto.randomUUID()` (line 168) even when reclaiming an existing email's lead (the reclaim UPDATE sets `id = ?` to the new UUID). Stripe's `Idempotency-Key: marketing-offer:${leadId}` therefore differs on every retry, defeating idempotency. If `createStripePromotion` throws *after* Stripe processed the request (timeout, connection reset, JSON parse failure), the code is live in Stripe for 30 days (`expires_at`, `max_redemptions: 1`) but the local row is marked `failed`; the next attempt generates a *different* code (derived from the new leadId). Result: multiple active single-use 20% codes per email, redeemable by anyone who learns them — a discount-abuse vector and Stripe-object leak. Additionally the deterministic `codeForLead` changes when the lead id rotates, so previously issued-but-unredeemed codes stay valid in Stripe alongside the new one.
- **Fix:** Keep a stable per-email lead id on reclaim (don't rotate the PK), reuse one idempotency key per email, and/or deactivate prior promotion codes via the stored `stripe_promotion_code_id` before issuing a new one.

### MO-2 · LOW — Rate-limit key derived from attacker-influenced `X-Offer-Visitor-IP` header
- **File:** `marketing-offer.ts:170–177`
```ts
const visitorIp = request.headers.get('X-Offer-Visitor-IP') ?? 'unknown';
... rateLimiter.limit({ key: `marketing_offer:${visitorIp}` })
```
- **Why:** The value comes from a plain request header forwarded through the internal call. Any caller possessing `ADMIN_API_SECRET` (the only other gate) can vary this header per request to obtain unlimited limiter buckets. Defense rests entirely on the admin secret + trust that the frontend always sets it from the real client IP. Prefer deriving the key from the actual `CF-Connecting-IP` on the incoming request, or drop per-IP keying since the endpoint is admin-gated anyway.

### MO-3 · LOW — Reclaim UPDATE mutates the primary key of existing rows
- **File:** `marketing-offer.ts:200–216`
```sql
SET id = CASE WHEN status = 'ready' THEN ? ELSE id END, ...
```
- **Why:** Rotating `id` (a UUID PK) for expired-ready leads breaks referential integrity for anything referencing `marketing_offer_leads.id` (e.g., audit trails, the stored `stripe_promotion_code_id` linkage is nulled here too). Also makes the earlier `Idempotency-Key` instability (MO-1) worse. If id rotation isn't required, keep the original id and rotate only `claim_token`.

### MO-4 · LOW — Promotion-code collision space and modulo bias
- **File:** `marketing-offer.ts:63–72`
- **Why:** Suffix uses 8 bytes mod 32 (~40 bits, biased toward first alphabet halves since 256 % 32 == 0 actually makes it uniform — but only bytes 0..7 are used, giving 32^8 ≈ 1.1e12 codes; collision across leads yields a Stripe duplicate-code rejection and a spurious `failed` status for an unrelated lead). Practically negligible at expected volume; noting for completeness. (Modulo bias itself is absent here because 256 is divisible by 32.)

### MO-5 · INFO — `AdminUnauthorizedError` mapped to 404 "Not found"
- **File:** `marketing-offer.ts:340`
- **Why:** Deliberate endpoint obscurity; acceptable, but note genuine misconfiguration (`OfferConfigurationUnavailable`) returns 503 with message "Introductory offer is unavailable" which distinguishes configured vs unconfigured states to an authenticated caller. Fine given the admin-secret gate.

### MO-6 · INFO — Concurrent same-email claims produce 409 instead of queueing
- **File:** `marketing-offer.ts:186–219`
- **Why:** Double-submit within the 5-minute 'creating' window returns `OfferClaimBusy` (409). Correct behavior, but the second tab gets an error rather than the eventual code; consider polling the lead status instead. Cosmetic UX.

---

## license.ts

### LC-1 · MEDIUM — `lookupPublicLicense` returns an arbitrary license and cross-license machine totals when a customer has multiple licenses
- **File:** `license.ts:455–470, 480–492`
```sql
SELECT l.license_key, ... FROM licenses l JOIN customers c ON l.customer_id = c.id WHERE c.email = ?
```
and machine count:
```sql
SELECT COUNT(*) ... JOIN customers c ... WHERE c.email = ? AND m.is_active = 1
```
- **Why:** With multiple licenses per customer email (tiers/upgrades make this plausible), `queryFirst` returns whichever row D1 emits first (undefined order), while the machine count aggregates across *all* the customer's licenses. The dashboard can therefore display license A's masked key/tier with license B's seat usage, or `used_machines > max_machines`. The validate path scopes everything by `license.id`; this lookup doesn't.
- **Fix:** Pick the primary/latest license deterministically (`ORDER BY l.created_at DESC LIMIT 1`) and count machines scoped to that `l.id`.

### LC-2 · MEDIUM — Public lookup ignores the `max_machines` fallback used by validation
- **File:** `license.ts:456` vs `maxMachinesFor` at lines 96–104
```sql
SELECT ..., l.max_seats as max_machines FROM licenses ...
```
- **Why:** `maxMachinesFor` prefers `max_seats`, then falls back to `max_machines`, else 1. The public dashboard query aliases only `max_seats`; when a license sets only `max_machines`, users see `max_machines: null` while activation enforces the real cap. Data inconsistency between surfaces.
- **Fix:** `SELECT COALESCE(l.max_seats, l.max_machines, 1) AS max_machines`.

### LC-3 · LOW — Shared `'unknown'` rate-limit buckets in `handleReportUsage` and `handleInstallPing`
- **File:** `license.ts:690, 721`
```ts
const ip = request.headers.get('CF-Connecting-IP') ?? 'unknown';
```
- **Why:** Same defect class as DA-5 but worse impact: all CF-headerless clients share one `report_usage:` bucket, so one noisy internal caller can 429 legitimate CLI usage reports globally. `handleValidateLicense` and `ingestAnalytics` use the correct random-UUID fallback; these two don't.
- **Fix:** `?? crypto.randomUUID()`.

### LC-4 · LOW — Oversized analytics batches silently discarded with success semantics
- **File:** `license.ts:873`
```ts
if (requestedEvents.length === 0 || requestedEvents.length > 50) {
  return { success: true as const, processed: 0 };
}
```
- **Why:** Dead branch (schema `AnalyticsBatchSchema` already caps `maxItems(50)` and rejects bigger batches with 400), but if it were reachable it would silently drop data while reporting success. Either way the client cannot distinguish "rate-limited" (also returns `processed: 0` success) from "stored" — telemetry loss is invisible. Remove the dead condition and consider returning a distinct flag for dropped batches.

### LC-5 · LOW — `eddsaSign` assumes base64url-encoded PKCS8 inside the PEM body
- **File:** `license.ts:802–818`
```ts
const keyData = base64UrlDecode(privateKeyDer.replace(/-----BEGIN PRIVATE KEY-----|-----END PRIVATE KEY-----|\n/g, ''));
```
- **Why:** Standard tooling exports PEM bodies in *standard* base64 containing `+` and `/`. base64url-decoding such input corrupts the key or throws in `atob`, turning every license activation into a 500. Works only if the secret was deliberately stored base64url-encoded; there's no normalization for standard base64. Fragile secret-format coupling with no test visible in scope.
- **Fix:** Detect and accept both alphabets (replace `-/`→`+/_` heuristically or store explicit format metadata).

### LC-6 · INFO — Doc comment says GET|POST but handler rejects everything except POST with 400
- **File:** `license.ts:155–160, 187–190`
```ts
/**
 * HTTP adapter for `GET|POST /api/validate-license`. ...
 */
...
new LicenseHandlerError('InvalidRequestUrlError', 'Method not allowed', 400)
```
- **Why:** The comment is stale relative to the deliberate POST-only security decision documented in `decodeInput`; also "Method not allowed" should conventionally be 405 with `Allow: POST`. Cosmetic/documentation drift.

### LC-7 · INFO — JWT embeds the raw license key (`lic` claim)
- **File:** `license.ts:771`
- **Why:** The offline-validation token carries the full license key. The bearer already knows it, so no direct escalation, but any log/APM sink that captures JWT payloads captures live license keys. Consider hashing the claim if the CLI doesn't need the plaintext.

### LC-8 · INFO — `reportUsage` MAX()-watermark semantics conflate concurrent machines
- **File:** `license.ts:551–572`
- **Why:** Per-license daily counters use `MAX(existing, excluded)` rather than summing deltas. That is correct only if every CLI instance reports cumulative lifetime-today counters; two machines reporting independent cumulative counts under-report (max wins). Presumably intended (per-machine rows exist separately), documenting as design observation.

### LC-9 · INFO — Install-ping rate-limited responses fake success
- **File:** `license.ts:719–726`
- **Why:** When rate limited, handler returns `{success:true, message:'Install recorded'}` without recording. Intentional anti-enumeration choice presumably, but it lies about persistence; a neutral `{success:true}` would be cleaner.

### LC-10 · INFO — Seat-limit INSERT relies on D1 serializing the conditional INSERT…SELECT
- **File:** `license.ts:225–247`
- **Why:** Good TOCTOU mitigation, but correctness depends on D1 (SQLite) write serialization of the count subquery + insert; there is no UNIQUE guard mentioned for `(license_id, machine_id)` visible in scope, so two concurrent activations of the *same new* machine could double-register and consume two seats. Verify a unique index exists on `machines(license_id, machine_id)` in migrations.

---

## Cross-cutting

### XC-1 · LOW — `getCorsHeaders` ignores its parameter and hardcodes credentials:true with a fixed origin
- **File:** `api.ts:196–198` (used by `github-proxy.ts:41, 90, 156`)
- **Why:** `_origin` unused; `Access-Control-Allow-Credentials: true` combined with `Access-Control-Allow-Origin: https://omg.latham.cloud` is safe only because the origin is pinned. Any future refactor that starts reflecting the argument creates a credentialed-reflection hole. The parameter invites that mistake — remove it or assert it.

### XC-2 · INFO — Consistent missing-rate-limiter posture
- `handleValidateLicense`/`handleReportUsage`/`handleInstallPing` proceed unprotected when `env.API_RATE_LIMITER` is undefined (no warning logged), whereas `handleDocsAnalytics` logs a warning. Align behavior and alerting.

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 0 |
| MEDIUM | 8 (DA-1, DA-2, DA-3, DA-4, GP-1, MO-1, LC-1, LC-2) |
| LOW | 12 |
| INFO | 10 |

Total: 30 findings. Highest-priority fixes: MO-1 (Stripe orphan codes / discount abuse), DA-1 (unvalidated bind params fail whole batches), LC-1/LC-2 (public license lookup inconsistencies), DA-3 (dead performance dashboard).


---

# SLICE 29

# slice-29 — omg-web `site/workers/src/handlers/`: privacy.ts, site-analytics.ts, site-session.ts, telemetry.ts

Read-only audit. All findings verified against source; supporting files (`api.ts`, `body.ts`, `contracts/http-bodies.ts`, `contracts/cli-telemetry.ts`, `telemetry-policy.ts`, `shared/site-session.ts`) read for context only.

---

## HIGH

### H-1. Site session expiry comparison broken by ISO-vs-SQL timestamp format (site-session.ts:118-126)
```ts
`SELECT token, expires_at FROM sessions
 WHERE customer_id = ? AND expires_at > datetime('now')
 ORDER BY created_at DESC LIMIT 1`
```
`expires_at` is written as a JS ISO string with `T`/milliseconds/`Z` (`new Date(...).toISOString()`, line 141), but compared lexicographically against `datetime('now')` which yields `YYYY-MM-DD HH:MM:SS`. On the expiry day itself, `'T'` (0x54) > `' '` (0x20), so an already-expired session (e.g. expired at 00:05 today) still compares as "not expired" for up to ~24 hours. A minted token whose 7-day lifetime ended earlier that same UTC day is reused instead of rotated.
**Fix:** store/compare in the same format — either bind `datetime('now', '+7 days')`-style SQL timestamps, or compare against a JS-computed cutoff in the same ISO format.

### H-2. Session inflation: every pageview counted as a new session (site-analytics.ts:246-253)
```sql
INSERT INTO site_analytics_geo_daily (...) VALUES (?, ?, ?, 1, 1, 1)
ON CONFLICT(date, country_code, city) DO UPDATE SET
  pageviews = pageviews + 1,
  sessions = sessions + (CASE WHEN excluded.visitors = 1 THEN 1 ELSE 0 END)
```
`excluded.visitors` is the value from the INSERT's VALUES list, which is the literal `1` on every row — so the CASE is always true and `sessions` increments once per pageview event. Multi-page visits massively inflate `total_sessions` in `handleGetAnalyticsOverview`. To dedupe you need a per-(visitor,day) marker (e.g. check `site_analytics_realtime.first_seen_at` or a separate daily visitor set), not `excluded.visitors`.
**Fix:** track sessions properly, e.g. increment sessions only when no prior realtime row exists for the visitor today, or maintain a distinct `visitor_day` table.

---

## MEDIUM

### M-1. Spoofable internal-call gate (site-session.ts:163-166)
```ts
if (request.headers.get('X-Internal-Call') !== 'service-binding') {
  return errorResponse('Not found', 404);
}
```
Any external client can set `X-Internal-Call: service-binding`; Cloudflare service bindings do not strip caller-supplied headers of this name unless explicitly removed. The gate therefore provides no real isolation — real protection rests entirely on `requireAdminSecret`. Also note service bindings don't automatically add such a header; if the BFF doesn't set it, this check would break legitimate calls too. Verify both directions.
**Fix:** rely solely on the admin secret (constant-time compare), or validate on a binding-derived signal (e.g. `request.cf` / mTLS / a random shared header injected by the BFF *after* stripping inbound copies).

### M-2. Unbounded `properties` JSON persisted per event (site-analytics.ts:196-215)
`properties` is `Schema.Record(String, JsonValue)` — arbitrarily nested, unbounded strings. The handler spreads it verbatim into the stored JSON:
```ts
JSON.stringify({ ...properties, device, browser, os, referrer_domain: referrerDomain }),
```
The handler clamps `event_name`, `session_id`, and `path`, but not `properties`. With the declared cap bypassable (see M-3) and `decodeJsonBody`'s 1 MB fallback, one request can persist up to ~50 × 1 MB of attacker-chosen JSON into D1 rows — storage abuse and slow admin queries. Same class of issue in telemetry.ts: `metadata` record values and `packages` string items are length-unbounded (telemetry.ts:73-90 sanitizes scalars only).
**Fix:** bound serialized properties size (e.g. drop or truncate when `JSON.stringify(properties).length > N`) and cap individual string values.

### M-3. Content-Length gate bypassable via chunked/streaming bodies (telemetry.ts:33-47, site-analytics.ts:70-72)
```ts
const contentLength = request.headers.get('Content-Length');
if (contentLength === null) return undefined;
```
A request without `Content-Length` (chunked transfer encoding) skips the size gate entirely; only the downstream `decodeJsonBody` 1 MB cap catches it — and that cap (1 MB) exceeds the intended per-endpoint limits (100 KB single telemetry, 512 KB site track). The declared-size gates are advisory only.
**Fix:** enforce byte budget during body reading per endpoint (parameterize `decodeJsonBody` max bytes), not just the declared header.

### M-4. Rate limiting fails open (telemetry.ts:96-99)
```ts
} catch (error: unknown) {
  reportError('Rate limit check failed:', error);
}
```
If the rate-limiter binding throws, ingestion proceeds with no limit. Combined with the missing-binding path (`reportWarning` then continue, also in site-analytics.ts:76-79), all ingest endpoints are unprotected whenever the limiter errors.
**Fix:** fail closed (429/500) or use a bounded retry; at minimum document the deliberate fail-open decision.

### M-5. Visitor salt rotation destroys visitor identity every ≤90 seconds (site-analytics.ts:36, 168-190)
`SALT_WINDOW_MS = 90_000`: the latest salt inserted within the last 90 s is reused; otherwise a fresh random salt is generated and inserted. Consequently the HMAC visitor id changes at most every 90 s, so a visitor browsing for 10 minutes can appear as dozens of distinct `visitor_id`s. This corrupts: `COUNT(DISTINCT visitor_id)` realtime active-visitor counts (over-counted), the `site_analytics_realtime ON CONFLICT(visitor_id)` upsert (rows churn instead of accumulating `page_count`), and any visitor-based dedup assumptions. If short-lived pseudonymization is a privacy goal, the aggregation layer must not treat `visitor_id` as stable.
**Fix:** use a daily-rotating salt (or salt derived deterministically from date) so ids are stable within the aggregation window.

### M-6. Expired-session reuse plus duplicate-session race (site-session.ts:128-155)
The find-then-insert for sessions has no uniqueness guard: two concurrent mints for the same customer (double-click login) both see `sessionRow === null` and insert two live sessions. Low impact (both valid), but combined with H-1 the "reuse existing" path can hand out tokens whose actual expiry passed. Also, reused existing sessions never have their `expires_at` extended, so heavy BFF traffic keeps returning a token that will hard-expire mid-use with no refresh path here.
**Fix:** add a unique partial index or accept-and-log duplicates; consider sliding renewal.

### M-7. iOS user agents classified as macOS (site-analytics.ts:140-158)
```ts
if (/windows/i.test(ua)) ... else if (/mac os/i.test(ua)) { os = 'macOS'; }
... else if (/ios|iphone|ipad/i.test(ua)) { os = 'iOS'; }
```
iPhone/iPad UAs contain `like Mac OS X`, so the `mac os` branch matches first and iOS devices are recorded as macOS; the trailing `ios|iphone|ipad` branch is effectively dead code. Device split analytics are wrong for Apple mobile traffic (also modern iPadOS desktop-mode UAs classify as desktop/macOS).
**Fix:** test `/iphone|ipad|ipod/` before the mac os branch.

---

## LOW

### L-1. Deletion does not anonymize licenses despite documented retention claim (privacy.ts:44-46 vs 152-157)
Comment says "Retains … License records (anonymized)" but the code only sets `status = 'deleted_by_user'`; license rows keep `license_key`, machine linkage columns, etc., tied to the customer row (customer row itself is not deleted/anonymized either — email remains). GDPR erasure claim vs implementation mismatch.
**Fix:** hash/null the license key and PII-bearing columns, or amend the disclosure.

### L-2. Export error paths swallow decode failures without reporting (privacy.ts:82-101 `loadPrivacyRows`)
On schema-decode failure the function returns a bare generic 500 with no `reportError`/Sentry call, unlike every catch block in these files — corrupt-row incidents will be invisible operationally.
**Fix:** `reportError(invalidRowMessage, ...)` before returning the 500.

### L-3. No rate limiting / size gating on privacy endpoints (privacy.ts:66, 176, 300, 455)
`handleExportMyData` runs ~8 D1 queries (with `LIMIT 1000` scans) and serializes a large payload per call; `handleDeleteMyData` runs a 11-statement batch; neither passes through `API_RATE_LIMITER`. An authenticated user (or an attacker with a stolen token) can hammer these. Export responses also lack `Content-Disposition`-adjacent CSRF concerns but do include `corsHeaders` (fine); still worth rate-limiting expensive self-service endpoints.

### L-4. `deleted` counts omit licenses update and hide zero-change ops (privacy.ts:186-193)
Only operations with `changes > 0` appear in the response map, so clients cannot distinguish "nothing to delete" from "op missing"; the license status flip and audit insert aren't reported at all. Cosmetic/API-clarity defect.

### L-5. `|| null` / `|| 0` coercion collapses legitimate falsy values (telemetry.ts:131, 160, 170, 175; site-analytics.ts:222-223)
`event.duration_ms || 0` turns a legitimate `0` into 0 (ok) but `result_count || null` turns a real `0` count into NULL; `event.timestamp || now` in site-analytics treats timestamp `0` as absent. Use explicit `??` / `Number.isFinite` checks.

### L-6. NaN durations pass schema and blow up at bind time (telemetry.ts via cli-telemetry.ts `OptionalNumber`)
Effect's `Schema.Number` accepts `NaN`/`Infinity`; `duration_ms` NaN reaches D1 `.bind()` → driver error → 500 for the whole batch. Filter non-finite numbers during sanitize.

### L-7. Hourly rollup inconsistent with geo rollup (site-analytics.ts:255-260)
`site_analytics_hourly` conflict update increments only `pageviews`; `visitors`/`sessions` stay at their insert-time 1 forever, so hourly unique counts undercount relative to intent while geo_daily overcounts sessions (H-2). Whatever the intended semantics, the two tables disagree.

### L-8. Salt-insert race creates orphan/duplicate salts (site-analytics.ts:180-189)
Two concurrent cold requests both observe `_tag === 'missing'` and each insert a new salt; subsequent reads pick whichever is newest, silently re-keying some visitors mid-window. Harmless-ish but adds noise; make `(inserted_at)` inserts idempotent or tolerate it explicitly.

### L-9. Internal error messages leaked to clients (site-session.ts:172-184)
`CustomerStoreUnavailable`'s message (`"Customer store unavailable during findByEmail"`) is returned verbatim as the 500 body. Not secret-bearing, but it discloses internals; other handlers return generic messages.

### L-10. SQL interpolation of numeric constant (privacy.ts:432-435; site-analytics.ts:168-171)
`` `datetime('now', '-${AUDIT_LOG_RETENTION_DAYS} days')` `` and `${SALT_WINDOW_MS}` are safe today (compile-time constants) but establish an interpolation habit in SQL strings. Prefer bound parameters: `datetime('now', ?)` with `'-30 days'`.

### L-11. Duplicate column in export query (privacy.ts:216-218)
`SELECT tier, status, max_machines, created_at AS activated_at, expires_at, created_at FROM licenses` selects `created_at` twice; harmless but suggests copy-paste drift between the aliased and raw column.

### L-12. `name` written into `company` column (site-session.ts:69-75)
```ts
INSERT INTO customers (id, email, company, tier, admin) VALUES (?, ?, ?, 'free', ?)
 ... .bind(crypto.randomUUID(), body.email, body.name ?? null, admin)
```
The Better Auth display name is persisted as `company`. If intentional, it's undocumented and surprising; profile exports (privacy.ts:236) surface it as `company`.

---

## INFO

### I-1. `handlePrivacyStatus` returns different shapes for anonymous vs authenticated callers (privacy.ts:448-483) — intentional, but `email_on_file` masking logic (`separatorIndex > 0 && < len-1`) correctly rejects `@foo.com`/`foo@`; well done.

### I-2. `parseReportingDays` accepts garbage like `"5abc"`? No — `Number.parseInt('5abc')` = 5, so trailing junk is silently accepted and clamped (site-analytics.ts:26-29). Minor laxness, consistent behavior though.

### I-3. Browser detection ignores Chromium-based Opera/Brave/Arc (all match `/chrome/` → "Chrome") and mobile Safari vs desktop — acceptable coarse bucketing; noted only.

### I-4. `STORAGE_EVENT_TYPE` mapping means scroll/time/vitals/engagement events all land in the legacy `performance` category, losing semantic type except via `event_name` (site-analytics.ts:40-52). Documented tradeoff; verify reporting queries account for it.

### I-5. `authorizeTelemetry` checks the license key supplied per-event only against `status='active'` (telemetry-policy.ts:56-59); expired/suspended licenses silently get 401 "Invalid license key" — correct authz scoping (no IDOR: licenseId resolved server-side). Good.

### I-6. Realtime upsert keys on `visitor_id` alone (site-analytics.ts:230-240): multi-tab/multi-page concurrent events overwrite `page_path` and inflate `page_count` regardless of page; top-pages stats derive from `GROUP BY page_path` over one row per visitor, so "top pages" reflects last-visited page per visitor, not views. Design limitation worth documenting.

---

## Summary
- CRITICAL: 0
- HIGH: 2 (H-1 session expiry format mismatch, H-2 session inflation)
- MEDIUM: 7
- LOW: 12
- INFO: 6

Total: 27 findings.


---

# SLICE 30

# Audit slice-30 — omg-web `site/workers/src` (root files, store/, contracts/)

Scope: worker.ts, api.ts, prelude.ts, body.ts, otp.ts, observability.ts, telemetry-policy.ts,
stripe-reconciliation.ts, admin-auth.ts, admin-secret.ts, store/*, contracts/*. Read-only audit.
Line numbers refer to the files as of audit time.

---

## MEDIUM

### M1. Dead/broken leftover code: stray `});` inside an unterminated doc comment
**File:** `site/workers/src/store/admin-store.ts:696–699`
```ts
/** Load one page of enriched audit events, plus the total event count.
  });

/** Load advanced engagement, retention, adoption, and revenue metrics. */
export const loadAdvancedMetrics = (db: D1Database) =>
```
**Why a bug:** A duplicate of the `listAuditLog` doc comment was deleted incorrectly. The `/**`
opened on line 696 is never closed on its own line; the stray `});` (line 697) and the *next*
doc comment (`/** Load advanced engagement … */`, line 699) are swallowed by the block comment —
the first `*/` encountered is the one terminating line 699's comment. The file still parses only
by accident; the stray `});` is dead code and `loadAdvancedMetrics` has lost its documentation
comment. Any future edit that closes the first comment will introduce a syntax error, and the
orphan signals a botched deletion that should be cleaned up.
**Fix:** Delete lines 696–697 entirely and restore a proper doc comment for `loadAdvancedMetrics`.

### M2. `listCohorts` references SELECT alias `month_index` in WHERE
**File:** `site/workers/src/store/admin-store.ts:~558–570` (`listCohorts`)
```sql
SELECT c.cohort_month,
  CAST((julianday(a.active_month || '-01') - julianday(c.cohort_month || '-01')) / 30.44 AS INTEGER) as month_index,
  ...
WHERE month_index >= 0 AND month_index < 12 GROUP BY 1, 2 ORDER BY 1 DESC, 2 ASC
```
**Why a bug:** Referencing a result-column alias inside `WHERE` is non-standard SQL. SQLite's
documented name-resolution permits aliases in GROUP BY/ORDER BY/HAVING; alias resolution in
`WHERE` is not guaranteed across versions and fails on most other engines. If the D1/SQLite build
rejects it ("no such column: month_index"), the entire admin cohorts endpoint 500s. Even where it
works it is fragile.
**Fix:** Repeat the full expression in the WHERE clause or wrap the query in a subquery/CTE and
filter on `month_index` in the outer select.

### M3. Stripe subscription projection persists only `items.data[0]` price id
**File:** `site/workers/src/stripe-reconciliation.ts:~233–247`
(`applyStripeSubscriptionProjection`, subscriptions upsert)
```ts
subscription.items.data[0]?.price.id ?? null,
```
**Why a bug:** Entitlement validation (`resolveProjectedEntitlement`) deliberately iterates **all**
subscription items and rejects subscriptions containing multiple recognized billing prices, but the
persisted `subscriptions.stripe_price_id` records only the first item. For a multi-item
subscription whose first item is an unrecognized add-on price, the stored row gets the add-on price
id while entitlement was validated against the real plan price elsewhere in the array. All
subsequent aggregate tier SQL (`effectiveTierFor`, `activeTeam`/`activePro` EXISTS checks) matches
on `stripe_price_id`, so such customers silently lose pro/team tier on the next projection pass —
exactly the downgrade the aggregate design claims to prevent.
**Fix:** Store the recognized catalog price id (resolved by `resolveProjectedEntitlement`) rather
than blindly `items.data[0]`, or reject/project each recognized item explicitly.

### M4. `ensureBillingCustomer` race / uniqueness handling missing
**File:** `site/workers/src/stripe-reconciliation.ts:~78–140`
```ts
await db.prepare('UPDATE customers SET stripe_customer_id = ? WHERE id = ?')...
...
await db.prepare(`INSERT INTO customers (id, stripe_customer_id, email, tier) VALUES (?, ?, ?, 'free')`)...
```
**Why a bug:** Stripe does not guarantee webhook delivery order or exactly-once processing here;
two concurrent deliveries for the same new customer both observe "missing" and both INSERT →
duplicate customer rows (or a UNIQUE-constraint crash if a unique index exists on
`stripe_customer_id`/`email`), neither of which is caught. Likewise the email-based UPDATE can
violate a unique index on `stripe_customer_id` if another local row already holds it (e.g., email
address reuse after account deletion), failing the whole reconciliation with an opaque D1 error.
There is also no guard before overwriting an existing non-null `stripe_customer_id` on the
email-matched row.
**Fix:** Use `INSERT ... ON CONFLICT DO NOTHING/DO UPDATE` + re-read, catch unique-constraint
errors explicitly, and only set `stripe_customer_id` when currently NULL (or equal).

### M5. Admin CSV exports silently truncated to 1000 rows
**File:** `site/workers/src/store/admin-store.ts` (`exportUsage`, `exportAudit`, `exportUsers`)
```ts
'SELECT date, license_id, commands_run, time_saved_ms FROM usage_daily ORDER BY date DESC LIMIT 1000'
'SELECT created_at, action, customer_id, ip_address FROM audit_log ORDER BY created_at DESC LIMIT 1000'
... FROM customers c LEFT JOIN licenses l ... LIMIT 1000'
```
**Why a bug:** The doc comment says "capped at the newest 1000 days of data" / "1000 events", but
these are hard row caps on security/compliance-relevant exports. An operator exporting the audit
log receives the newest 1000 events with no indication that older rows exist — silent data loss in
a feature whose purpose is completeness. With real usage, usage_daily alone will exceed 1000 rows
quickly (rows are per license per day).
**Fix:** Paginate the export (cursor/keyset) into streamed CSV, or at minimum surface the total
count vs exported count in the response so truncation is visible.

---

## LOW

### L1. `getCorsHeaders` ignores its origin parameter
**File:** `site/workers/src/api.ts:~186–188`
```ts
export function getCorsHeaders(_origin: string | null) {
  return { ...corsHeaders, 'Access-Control-Allow-Credentials': 'true' };
}
```
**Why a bug:** The `_origin` parameter is dead; callers (handlers/github-proxy.ts) compute and pass
an origin that is discarded. Behavior is fixed-origin so it is not exploitable, but the signature
misleads reviewers into thinking per-origin CORS exists, and credentialed responses always carry
the pinned ACAO even when the request came from another origin (harmless, but noise).
**Fix:** Remove the parameter, or implement the intended origin check.

### L2. `LICENSE_STATUSES` includes `'cancelled'` unreachable via HTTP contract
**File:** `site/workers/src/store/admin-store.ts:86` vs `contracts/http-bodies.ts` `AdminLicenseStatus`
```ts
export const LICENSE_STATUSES = ['active', 'cancelled', 'inactive'] as const;
// http-bodies: Schema.Literal('active', 'inactive')
```
**Why a bug:** The HTTP body schema only accepts `active|inactive` while the store enum (used by
`matchUnion` in handlers/admin.ts:400) also advertises `cancelled`. That member can never be
selected through the API — dead value and drift between contract layers (note the DB/projection
layer writes `'cancelled'`, so admins cannot restore/set that state manually).
**Fix:** Align the two unions intentionally and document why they differ, or extend the wire schema.

### L3. Installs badge renders `0` on malformed row instead of surfacing staleness
**File:** `site/workers/src/worker.ts:~96–113` (`handleInstallsBadge`)
```ts
if (badgeLookup._tag === 'invalid') {
  Sentry.captureMessage('Installs badge row has an invalid shape');
}
const total = badgeLookup._tag === 'present' ? badgeLookup.value.total : 0;
return badgeResponse(total.toLocaleString());
```
**Why a bug:** A corrupt row yields a confident "0 installs" badge (cached publicly for 60s +
SWR 300s) instead of keeping the last known good value or an error state. Also
`Sentry.captureMessage` drops the actual parse cause, hampering diagnosis. Minor, but the badge is
public-facing marketing surface.
**Fix:** Return the previous cached value (or omit count) on invalid rows and include the parse
error detail in the report.

### L4. `updateNote` with no changed fields still bumps `updated_at` and reports success
**File:** `site/workers/src/store/crm.ts` (`updateNote`)
```ts
const updates: string[] = ['updated_at = CURRENT_TIMESTAMP'];
```
**Why a bug:** A PUT with `{}` (both fields optional in `AdminUpdateNoteBodySchema`) performs a
no-op write that touches `updated_at`, corrupting the "last edited" signal auditors rely on, and
reports `'updated'`.
**Fix:** Reject empty update bodies at decode time or short-circuit to `'not-found'`/validation
error when neither field is present.

### L5. CLI telemetry envelope leaves `machine_id`/`version`/`platform` uncapped
**File:** `site/workers/src/contracts/cli-telemetry.ts` (`TelemetryEnvelopeSchema`)
```ts
machine_id: Schema.String,
version: Schema.String,
platform: Schema.String,
```
**Why a bug:** Sibling contracts (`AnalyticsEventSchema` in license-ops.ts) cap these fields
explicitly because, per its own comment, "Length caps bound the upsert-key cardinality of
analytics_daily / analytics_errors: without them every request mints new aggregate rows." The
telemetry envelope schema omits those caps, so unbounded strings flow into the same class of
storage keys — unbounded row minting / cardinality growth risk from any client.
**Fix:** Apply the same Capped() bounds used by AnalyticsEventSchema.

### L6. `statusRank('paused')` collapses to the unknown-status rank
**File:** `site/workers/src/stripe-reconciliation.ts:~28–34`
```ts
if (status === 'active' || status === 'trialing' || status === 'incomplete') return 1;
return 0;
```
**Why a bug:** `paused` is an explicit member of `StripeSubscriptionStatusSchema` but ranks 0, the
same as garbage values. A paused snapshot arriving at equal period_end loses to every other status
(including unknown junk), which happens to be safe today, but the mapping is implicit; adding a new
Stripe status silently lands at rank 0 with no test or assertion. The comment already warns about
lockstep drift with migration 017.
**Fail-safe suggestion:** Give known statuses explicit ranks and treat unrecognized statuses as a
logged anomaly rather than rank 0.

### L7. `updateUser` TOCTOU between existence check and update
**File:** `site/workers/src/store/admin-store.ts` (`updateUser`)
```ts
const existing = ... 'SELECT id FROM licenses WHERE customer_id = ?' ...
if (existing === null) return { _tag: 'customer-not-found' };
yield* ... 'UPDATE licenses SET ... WHERE customer_id = ?' ...
```
**Why a bug:** Two separate round-trips without a batch: a license created/deleted between check
and update produces a misleading result ('customer-not-found' for a customer who now has a license,
or a silent no-op update). Not exploitable, just imprecise.
**Fix:** Single statement `UPDATE ... ; changes()` via batch, or `RETURNING` to detect no-op.

### L8. `key` field capped at 128 but `license_key` path brand has no max — inconsistent key rejection UX
**File:** `site/workers/src/contracts/validate-license.ts:22–29` and `license-key.ts`
```ts
key: Schema.optional(Capped(128)),
license_key: Schema.optional(Capped(64)),
// LicenseKey = Schema.String.pipe(Schema.minLength(1), Schema.brand('LicenseKey'))
```
**Why a bug:** `toValidateLicenseRequest` silently maps over-long/invalid keys to `null`, so a
malformed credential surfaces as "missing license key" rather than "invalid license key". The two
wire fields also have different caps (128 vs 64) for the same logical credential, and the domain
brand imposes no upper bound at all.
**Fix:** Unify caps in `LicenseKey` and return a distinct invalid-key error from
`toValidateLicenseRequest`.

### L9. CRM constraint classification relies on SQLite error-message string matching
**File:** `site/workers/src/store/crm.ts:26–40`
```ts
message.includes('unique constraint') || message.includes('primary key')
... message.includes('foreign key constraint')
```
**Why a bug:** Correctness depends on D1/SQLite error text staying stable and lowercased; a wording
change turns `already-assigned` idempotency into a 500, or target-missing detection into a generic
failure. Also `isUniqueConstraint` matching bare `'primary key'` can misclassify unrelated
messages containing that phrase.
**Fix:** Match on SQLite error codes (e.g., `SQLITE_CONSTRAINT` codes in `cause.code`) when
available, keeping message matching only as fallback.

### L10. `handleAdminNotesRoute`/`handleAdminCustomerTagsRoute` default branches are unreachable
**File:** `site/workers/src/worker.ts:~118–160`
```ts
default:
  return errorResponse('Not found', 404);
```
**Why a bug:** `route.method` is already constrained by `resolveLicensingRoute`, so the default arm
can never execute with a method not covered by the case list unless the route table and these
switches drift — in which case this 404 masks the mismatch (e.g., PATCH added to routes but not the
switch would 404 instead of 405).
**Fix:** Use `casesHandled(route.method)` (prelude helper exists precisely for this) so drift is a
compile-time/loud failure, or return 405 Method Not Allowed.

---

## INFO

### I1. Session tokens stored and compared in plaintext
**File:** `site/workers/src/api.ts` (`generateToken`, `validateSession`)
Tokens are 256-bit random hex (good) but persisted unhashed and selected directly by equality. A
D1 leak exposes live session credentials. Consider storing only a hash of the token.

### I2. Generated OTPs never begin with 0
**File:** `site/workers/src/otp.ts` (`generateOtpCode`)
Range 100000–999999 (~19.9 bits) vs the schema's advertised 10^6 space; leading-zero codes are
valid inputs the generator never emits. Harmless entropy reduction; document or allow leading zeros.

### I3. Duplicate row-decode helpers across contract modules
**Files:** `contracts/d1-extras.ts` (`decodeExtraRowArray`, `decodeOptionalExtraRow`) vs
`contracts/account-dashboard.ts` (`decodeRowArray`, `decodeRow`) vs `validate-license.ts` (`decodeRow`,
`decodeRowArray`). Three near-identical implementations differing only in error type invite drift;
consolidate on one parameterized helper.

### I4. Inline admin-flag schema duplicates `AdminFlagRowSchema`
**File:** `admin-auth.ts` `requireAdminSession` uses `Schema.Struct({ admin: Schema.Number })`
inline although `contracts/d1-extras.ts` exports `AdminFlagRowSchema` for the same purpose
(and `customerIsAdmin` exists). Two definitions of "what makes a customer admin" can drift.

### I5. `telemetry-policy` truthiness narrowness
`resolveTelemetryIngestion` opts out only on `=== true || === 1`; any other truthy number (e.g., 2)
counts as opted-in=false. Safe given current writers, but the wide union
`(Number | Boolean | Null)` invites misinterpretation — prefer normalizing to boolean at decode.

### I6. `respondFromEffect` ignores typed-failure status semantics
`api.ts` — fine as designed, but nothing prevents a caller mapping a failure to a 200 body;
consider centralizing status mapping. No defect observed in-scope.

### I7. `loadRevenue` MRR tiers use `status='active'` only
`store/admin-store.ts` `ACTIVE_TIER_COUNTS_SQL` excludes `trialing`, whereas billing projections
elsewhere treat `trialing` as paid. Revenue dashboards undercount trialing customers relative to
entitlement logic. Confirm intentional.

### I8. `readOptionalExtraRow` discards the parse cause
`contracts/d1-extras.ts` returns `{_tag:'invalid'}` with no diagnostic attached; callers then log
generic messages (see L3). Consider carrying the cause on the invalid variant.

---

## Verified non-issues (checked, correct)

- Bind-parameter counts for the two large SQL statements in
  `stripe-reconciliation.ts` (`UPDATE licenses …` 19 placeholders / 19 binds;
  `INSERT INTO licenses …` 26 placeholders / 26 binds) — verified programmatically.
- `body.ts` bounded read: stream-counted cap defeats lying Content-Length; cancel-on-exceed present.
- `prelude.timingSafeEqualUtf8`: padded compare plus length-equality conjunction is constant-time
  enough and correct.
- `otp.ts` rejection sampling bound (`UNBIASED_LIMIT`) is computed correctly for uniform digits.
- Route dispatch (`normalizeLicensingPath` + exact method/path match) prevents trailing-slash and
  method-confusion bypasses; admin gating is consistently applied inside handlers for all
  `/api/admin/*` routes (spot-checked firehose/billing/marketing-offer/site-session).
- `requireAdminSecret` fails closed on unset/empty secret; timing-safe comparison used.
- `listUsers` LIKE escaping handles `\ % _` correctly including backslash-first ordering.
- `loadDashboard` batch-index → decoder mapping verified correct (indices 0–12).

Total findings: 25 (5 MEDIUM, 10 LOW, 8 INFO, plus 8 verified non-issues documented).


---

# SLICE 31

# Slice 31 — Audit of `workers/router`, `workers/releases`, wrangler configs, and `tools/` scripts (omg-web)

Read-only audit. Files fully reviewed:

- `~/Documents/omg-web/workers/router/src/index.ts` (385 lines)
- `~/Documents/omg-web/workers/releases/src/index.ts` (116 lines)
- `~/Documents/omg-web/workers/router/wrangler.toml`
- `~/Documents/omg-web/workers/releases/wrangler.toml`
- `~/Documents/omg-web/tools/check-source-policy.mjs` (65 lines)
- `~/Documents/omg-web/tools/check-cloudflare-remote.mjs` (61 lines)
- `~/Documents/omg-web/tools/oxlint/sync-anti-slop.mjs` (238 lines)

---

## HIGH

### H-1. Rewritten docs responses can carry a stale/wrong `Content-Length` from the origin
**File:** `workers/router/src/index.ts`, lines ~150–166 (`rewriteDocsResponse`) and ~168–190 / 192–214 (`finalizeDocsResponse`, `docsResponseHeaders`)
```ts
const rewritten = rewriteContent(await response.text(), isCss, hostname, docsOrigin);
const body = new ReadableStream({ ... encode(rewritten) ... });
return new Response(body, response); // copies ALL origin headers, incl. Content-Length
```
and later:
```ts
const headers = new Headers(response.headers); // Content-Length preserved
...
return new Response(response.body, { status, statusText, headers });
```
**Why it's a bug:** When HTML/CSS/JS content is rewritten, its byte length almost always changes (every `https://omg-docs.pages.dev` → `/docs` substitution shortens the body), but the origin's `Content-Length` header is carried through both `new Response(body, response)` and `finalizeDocsResponse`. If the Workers runtime honors the declared length, clients get truncated or over-long bodies / protocol errors on every rewritten page whose length changed. The correct behavior is to delete `Content-Length` when replacing a body with a buffer of different size.
**Fix:** In `rewriteDocsResponse`, build headers explicitly and `headers.delete('Content-Length')` (and defensively `Content-Encoding`, `Content-Range`) whenever content was rewritten.

---

## MEDIUM

### M-1. Entire R2 releases bucket publicly readable by key guessing; no method restriction
**File:** `workers/releases/src/index.ts`, lines ~57–85 (`/download/:filename` handler)
```ts
if (path.startsWith('/download/')) {
  const filename = path.slice('/download/'.length);
  ...
  const object = await readReleaseObject(env, filename);
```
**Why it's a bug:** Every key in bucket `omg-releases` is served to anonymous callers with zero authentication, authorization, or rate limiting. Any object ever placed in this bucket (staging binaries, manifests, anything the pipeline writes) becomes world-readable if the key can be guessed. Additionally there is no method check: `POST /download/x`, `DELETE /latest-version`, etc. are all served as if GET.
**Fix:** Restrict to `GET`/`HEAD`; consider a key prefix namespace check (e.g. only serve keys matching `^[\w.-]+$` / an allowlist pattern such as release-artifact naming) so non-release objects in the shared bucket are unreachable.

### M-2. Client-supplied `X-Forwarded-*` and other spoofable headers forwarded verbatim to both origins
**File:** `workers/router/src/index.ts`, lines ~236–270 (`prepareOriginHeaders`), used for MAIN_SITE (~line 26) and DOCS_SITE (~line 100)
```ts
for (const [key, value] of headers.entries()) {
  if (!hopByHopHeaders.includes(key.toLowerCase())) newHeaders.set(key, value);
}
newHeaders.set('X-Forwarded-Proto', 'https');
```
**Why it's a bug:** Only hop-by-hop headers are stripped. A client can inject `X-Forwarded-Host`, `X-Forwarded-For`, `X-Real-IP`, `Forwarded`, `CF-Connecting-IP`, etc., which are passed untouched to `omg-site.pages.dev` / `omg-docs.pages.dev`. Origins that trust these headers for absolute-URL generation, rate limiting, or logging can be misled (classic host-header-injection surface). Also, existing client-supplied `X-Forwarded-Proto` gets overwritten (good) but `X-Forwarded-Host` does not — inconsistent.
**Fix:** Strip all `X-Forwarded-*` / `Forwarded` from the incoming set before adding worker-controlled values; set `X-Forwarded-Host` explicitly to the worker hostname.

### M-3. `check-cloudflare-remote.mjs` spawns wrangler with no timeout — CI hang risk
**File:** `tools/check-cloudflare-remote.mjs`, lines ~24–32
```ts
const result = spawnSync(process.execPath, [wranglerPath, ...check.arguments], { env: {...}, encoding: 'utf8' });
```
**Why it's a bug:** No `timeout` option and no kill handling. If wrangler hangs (network blackhole, credential prompt), the pre-deploy gate blocks forever instead of failing fast. `result.error` is only checked after a nonzero status; a timed-out/killed run would be reported, but only if a timeout existed at all.
**Fix:** Add `timeout: 60_000` (and `killSignal`) to `spawnSync`, treat `result.error` / null status as failure even when output exists.

### M-4. Releases worker declares `ANALYTICS_DB` D1 binding but never uses it — download analytics silently missing
**Files:** `workers/releases/wrangler.toml` lines ~20–27 (+ production duplicate ~33–38); `workers/releases/src/index.ts` line 1–4 (`Env` has only `BUCKET`)
```toml
[[d1_databases]]
binding = "ANALYTICS_DB"
database_name = "omg-analytics"
```
**Why it's a bug:** The config comment says "Analytics tracking for downloads", the database is provisioned and bound in both top-level and production env, but the code never touches `ANALYTICS_DB` and `Env` doesn't even declare it. Either download analytics was planned and dropped (dead binding; every download untracked despite infra spend) or the feature regressed silently. Dead config also misleads auditors about data flows (the threat model presumably counts this D1 as written-to).
**Fix:** Either implement the counter increment (fire-and-forget via `ctx.waitUntil`) or remove the bindings and the D1 resource.

### M-5. Stale-on-error serves stale success bodies for 404/410, hiding deleted docs indefinitely
**File:** `workers/router/src/index.ts`, lines ~118–136 (`readStaleDocsResponse`)
```ts
if (originResponse.ok || originResponse.status === 304 || request.method !== 'GET') return null;
const staleResponse = await caches.default.match(...);
```
**Why it's a bug:** The fallback triggers on *any* non-ok status, not just 5xx. A document removed from the docs site (origin 404/410) keeps being served from cache with `X-Cache: STALE-ON-ERROR` until the cached entry expires — and since cached entries were stored with `immutable` Cache-Control and the entry is refreshed... actually it is never refreshed because each miss re-hits origin, gets 404 again, and re-serves stale; the stale copy itself never expires out of `caches.default` while being repeatedly read. Deleted content effectively never disappears.
**Fix:** Restrict stale-on-error to `originResponse.status >= 500 || originResponse.status === 502/503/504` (or fetch exceptions).

---

## LOW

### L-1. CSS rewrite double-prefixes already-proxied `/docs/…` URLs
**File:** `workers/router/src/index.ts`, lines ~283–287 (`rewriteContent`)
```ts
rewritten = rewritten.replace(/url\(["']?\/([^)"']*)["']?\)/g, `url("/docs/$1")`);
```
**Why it's a bug:** A CSS rule like `url("/docs/logo.svg")` (already proxy-relative, e.g. hand-written or produced by the earlier `replaceAll(docsUrl.origin, proxyOrigin)` when the CSS contains an absolute docs URL followed later by a relative one) becomes `url("/docs/docs/logo.svg")` → broken asset. The regex lacks a negative lookbehind for a preceding `/docs`.
**Fix:** Use `/(?<!\/docs\/)url\(...\)/`-style guard or skip matches whose captured path already starts with `docs/`.

### L-2. Hashed-immutable asset TTL regex essentially never matches real hashed filenames
**File:** `workers/router/src/index.ts`, lines ~330–333 (`getCacheTtl`)
```ts
if (pathname.match(/[a-f0-9]{8,}\.[a-f0-9]{8,}\.(js|css)$/i)) return 31536000;
```
**Why it's a bug:** This requires *two* dot-separated hex segments (e.g. `abc12345.def67890.js`). Conventional bundler output (`index-B4fA0192.js`, `app.a1b2c3d4.css`) has one hash segment, so virtually all JS/CSS falls through to the 1-day TTL and the "immutable 1 year" branch is dead code in practice.
**Fix:** Match single-hash patterns, e.g. `/[.-][a-f0-9]{8,}\.(js|css)$/i`.

### L-3. JSON API responses cached with `immutable`
**File:** `workers/router/src/index.ts`, lines ~226–232 (`docsResponseHeaders`)
```ts
headers.set('Cache-Control', `public, max-age=${cacheTtl}, s-maxage=${cacheTtl}, immutable`);
```
**Why it's a bug:** `immutable` tells browsers never to revalidate during max-age. It's meaningful for content-hashed assets, but wrong for HTML (intended to update within 5 min) and especially JSON APIs (60 s), where clients may need freshness semantics; combined with L-2's dead branch, everything cacheable gets `immutable` indiscriminately.
**Fix:** Apply `immutable` only to hashed static assets; use plain `public, max-age=…, s-maxage=…` otherwise.

### L-4. Whole-body buffering of docs responses defeats streaming and raises memory pressure
**File:** `workers/router/src/index.ts`, lines ~152–158 (`rewriteDocsResponse`: `await response.text()`)
**Why it's a bug:** Every HTML/CSS/JS response is fully buffered into a string and re-emitted as a single-chunk stream. Large assets (sourcemaps, big bundles served as JS) inflate isolate memory and time-to-first-byte. Not a correctness break at typical sizes, but a scaling defect.
**Fix:** Only rewrite below a size threshold (check `Content-Length`), else stream through unrewritten; or use a streaming `TransformStream` with replacement applied per-chunk-safe boundaries.

### L-5. Docs cache key omits hostname — cross-host cache reuse serves mis-rewritten content
**File:** `workers/router/src/index.ts`, lines ~92–96 & ~186 (`new Request(targetUrl, request)` keyed on targetUrl only)
**Why it's a bug:** Cached bodies contain HTML rewritten to absolute `https://<hostname>/docs` links of whichever host first populated the entry. If the worker is reachable under more than one host (zone route + workers.dev preview), the second host serves links pointing at the first host.
**Fix:** Include `url.hostname` in the cache key (or rewrite links relatively).

### L-6. Catch-all in docs proxy swallows all errors without any log signal
**File:** `workers/router/src/index.ts`, lines ~63–66 (`handleDocsProxy` try/catch)
```ts
} catch { return docsUnavailableResponse(); }
```
**Why it's a bug:** Programming defects (TypeError in rewriting, etc.) are indistinguishable from origin outages; nothing is logged despite `[observability] enabled = true`. Debugging production incidents requires reproducing locally. Contrast with the releases worker, which deliberately lets unknown errors escape for platform observability.
**Fix:** At minimum `ctx.waitUntil(console-free reporting)` — e.g. record an exception marker header or use the observability binding; or let unexpected error types escape like the releases worker does.

### L-7. Releases: no method restriction on `/latest-version` either; HEAD works incidentally
**File:** `workers/releases/src/index.ts`, lines ~30–55
**Why it's a bug:** Same as M-1's method gap for the version endpoint (`POST /latest-version` returns the version). Minor protocol hygiene.
**Fix:** Early-return 405 unless `GET`/`HEAD`.

### L-8. Releases download responses have no explicit `Cache-Control`
**File:** `workers/releases/src/index.ts`, lines ~72–82
**Why it's a bug:** Metadata (`etag`) is set but caching policy is left to Cloudflare defaults; artifacts are immutable, so missing `Cache-Control: public, max-age=31536000, immutable` wastes bandwidth and makes behavior environment-dependent. Conversely, if a bad artifact were ever overwritten in place, default caching could serve the stale broken artifact.
**Fix:** Set an explicit immutable long TTL for artifacts (they are content-addressed by version filename).

### L-9. Releases: broad catch maps *all* R2 `get` failures to 503 "store unavailable"
**File:** `workers/releases/src/index.ts`, lines ~97–103
**Why it's a bug:** The doc comment claims only storage failures map to 503, but the catch wraps every throw, including programmer/type errors surfaced through the binding call. A code regression masquerades as an outage with a misleading message.
**Fix:** Narrow the wrap (check known R2 error tags) or include the cause tag/classification in the response path/logs.

### L-10. `check-source-policy.mjs` crashes with raw ENOENT if a source root is missing
**File:** `tools/check-source-policy.mjs`, lines ~13–21 + ~37 (`readdir` without existence guard)
```ts
async function sourceFiles(relativeDirectory) {
  const entries = await readdir(new URL(`${relativeDirectory}/`, workspaceRoot), {...});
```
**Why it's a bug:** If any listed root (e.g. `site/e2e`) is renamed/removed, the script throws an unhandled rejection with an opaque file-URL stack trace instead of a clear "[source-policy] missing root" message. It still exits nonzero (accidentally correct), but diagnostics are poor for a guardrail tool.
**Fix:** Check `existsSync` per root and report a named error.

### L-11. `sync-anti-slop.mjs` clones upstream repo into OS tmpdir (RAM-backed tmpfs here)
**File:** `tools/oxlint/sync-anti-slop.mjs`, lines ~76 & ~89 (`mkdtempSync(join(tmpdir(), 'anti-slop-upstream-'))`)
**Why it's a bug:** On this machine `/tmp` is a small RAM tmpfs; project policy forbids repo clones/scratch checkouts there. A shallow fetch is small, but repeated syncs pin memory and violate the standing workspace rule.
**Fix:** Use a directory under `~/.cache/build-targets/` or `os.tmpdir()` override honoring `TMPDIR` set by the npm script.

### L-12. `sync-anti-slop.mjs` git fetch/execFileSync without timeout
**File:** `tools/oxlint/sync-anti-slop.mjs`, lines ~91–101 (`execFileSync('git', ['fetch'...])`)
**Why it's a bug:** A stalled network hangs `npm run sync:anti-slop` (and any CI step using it) indefinitely; `execFileSync` supports `timeout` but it isn't set.
**Fix:** Pass `timeout: 120_000` to the exec calls.

### L-13. Unexpected-file scan is single-level; rogue nested `.ts` escapes both sync and check
**File:** `tools/oxlint/sync-anti-slop.mjs`, lines ~139–155 (`unexpectedManagedFiles` uses non-recursive `readdirSync` over `MANAGED_DIRECTORIES`)
**Why it's a bug:** `managedPaths()` likewise only reads one level, so the two are consistent — but any drift introduced in a subdirectory (e.g. `rules/nested/bad.ts`) is neither synced, nor removed, nor flagged by `--check`, silently escaping manifest integrity verification.
**Fix:** Make both walks recursive (or document/enforce flat layout with a directory-depth assertion).

### L-14. `runCheck` trusts manifest completeness — files dropped from the manifest pass silently
**File:** `tools/oxlint/sync-anti-slop.mjs`, lines ~113–137
**Why it's a bug:** `--check` verifies each manifest entry against disk but never verifies that the manifest covers the required files (`index.ts`, `effect/index.ts`, all managed rules). A truncated/tampered manifest yields "vendored plugins match manifest" while half the plugin is unverified. (Offline mode can't check upstream, but it can at least assert required filenames are present.)
**Fix:** Assert the manifest key set ⊇ the structural minimum (`index.ts`, `effect/index.ts`) in `--check` mode.

### L-15. Duplicate manifest paths silently collapse via Map
**File:** `tools/oxlint/sync-anti-slop.mjs`, lines ~68–77 (`parseManifest` returns `Map`, later entries overwrite)
**Why it's a bug:** A hand-edited or merged-badly manifest containing the same path twice hides the conflict; the last hash wins and drift under the first hash goes undetected.
**Fix:** Throw on duplicate paths in `parseManifest`.

---

## INFO

### I-1. `prepareOriginHeaders` manually sets `Host`, which the Workers runtime ignores on outbound fetch
**File:** `workers/router/src/index.ts`, lines ~258–264
```ts
newHeaders.set('Host', originUrl.hostname);
```
Workers derives the outbound Host from the URL; a manually set Host header is disallowed/ignored. Dead code implying control it doesn't have. Remove it and the comment ("Will set this manually").

### I-2. `check-cloudflare-remote.mjs` validates only `omg-saas`/`omg-site`/platform-D1 — not this repo's own workers/resources
**File:** `tools/check-cloudflare-remote.mjs`, lines ~11–29
The gate doesn't probe `omg-router`, `omg-releases`, the `omg-releases` R2 bucket, or `omg-analytics` D1, so "all required production resources are accessible" overstates coverage for the workers defined in this repo.

### I-3. Source-policy markers match inside comments/strings
**File:** `tools/check-source-policy.mjs`, lines ~22–31
`contents.includes(policy.marker)` will flag documentation comments mentioning e.g. `@ts-ignore`, causing false positives; conversely it cannot catch suppressions with alternate spellings (`// @typescript-eslint/...` variants are covered by 'eslint-disable' substring, OK). Acceptable tradeoff; worth noting.

### I-4. Symlinked "files" skipped silently by source-policy walker
**File:** `tools/check-source-policy.mjs`, lines ~43–48 — `entry.isFile()` is false for symlinks, so symlinked sources bypass the policy scan without notice.

### I-5. Releases: keys containing `?` or `#` are unreachable
**File:** `workers/releases/src/index.ts`, lines ~59–62 — `url.pathname` splits at `?`/`#`, so an R2 key literally containing those characters can never be requested. Harmless for well-named release artifacts; note for pipeline key hygiene.

### I-6. Router: `shouldCache` caches any 2xx GET regardless of query string, keyed including query
**File:** `workers/router/src/index.ts`, lines ~180–188 — arbitrary query variants create distinct cache entries (cache-fill vector, bounded by Cloudflare quota). No `Vary` normalization for `Accept-Encoding` is needed on Workers (automatic), but query-string normalization is absent.

### I-7. `docsRedirect` passes through non-docs-host redirect Locations unchanged
**File:** `workers/router/src/index.ts`, lines ~106–116 — correct behavior (external redirects preserved), noting it was checked for open-redirect/proxy abuse: path is confined to `/docs*` and scheme/host are pinned to `env.DOCS_SITE`, so no SSRF via `/docs//evil.com` style input (host comes from config, not the path).

### I-8. Router wrangler.toml routes include both `/docs/*` and exact `/docs` — valid but redundant-looking; zone_name hard-coded twice (fine for single-zone).
**File:** `workers/router/wrangler.toml` lines ~6–9. No issue found; recorded for completeness.

---

## Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 1 |
| MEDIUM | 5 |
| LOW | 15 |
| INFO | 8 |

Total findings: 29

Most impactful next actions:
1. Fix `Content-Length` preservation after body rewriting (H-1) — likely user-visible truncation today.
2. Decide the fate of the unused `ANALYTICS_DB` binding (M-4): implement download analytics or remove the dead infrastructure.
3. Tighten stale-on-error to 5xx-only (M-5) so deleted docs actually disappear.


---

# SLICE 32

# Slice 32 — omg-web pipeline & config audit

Scope (read-only, every line reviewed):
- `.github/workflows/ci.yml` (74 lines)
- `oxlint.config.ts` (56 lines)
- `prettier.config.js` (12 lines)
- `tsconfig.json` (26 lines)

Cross-referenced: root `package.json` scripts (`check`, `audit`, `format:check`, `lint:oxlint`, `check:deploy`), `site/package.json` (`check:bundle-budget`), lockfile paths.

## Overall security posture of CI (positive findings)

- No `pull_request_target` anywhere; only `pull_request` with default types. Fork PRs run with the job-level `permissions: contents: read` and no secrets are referenced in the workflow — no secret exposure path exists.
- All third-party actions pinned to full commit SHAs with version comments (`actions/checkout@3d3c42e… # v7.0.1`, `setup-node@2028fbc… # v6.0.0`, `upload-artifact@bbbca2d… # v7.0.0`) — good supply-chain hygiene.
- `persist-credentials: false` on both checkouts prevents the runtime token from leaking into subsequent steps/artifacts.
- No `${{ }}` expression interpolation into `run:` shell blocks; all `run` blocks are static strings. No script injection surface.
- Concurrency group keyed on `github.ref` with `cancel-in-progress` correctly deduplicates push vs pull_request runs per ref.
- Artifact upload is failure-gated and explicitly anonymous-only; retention limited to 7 days.

## Findings

### F-1 — LOW — e2e job runs `npm ci` before pinning npm via corepack
File: `.github/workflows/ci.yml:57-63`
```yaml
      - run: npm ci
        working-directory: site
      - run: corepack enable npm && npm --version
        working-directory: site
```
The `check` job enables corepack *before* any `npm ci` (line 39), honoring `"packageManager": "npm@12.0.2"` (root `package.json:6`). The `e2e-anonymous` job does it backwards: `npm ci` in `site/` executes with the runner's stock npm before corepack is enabled, so the install step ignores the packageManager pin. If a lockfile ever requires npm-12 behavior (lockfileVersion drift, npm bugfix affecting resolution/hoisting), this job can install a different tree than the checked-in lockfile intends, while the check job would not.
Fix: move the `corepack enable npm && npm --version` step above the `npm ci` step in `e2e-anonymous`.

### F-2 — LOW — `npx playwright install` resolves Playwright at network time, unpinned by SHA
File: `.github/workflows/ci.yml:65-67`
```yaml
      - run: npx playwright install --with-deps chromium
```
Unlike the actions (SHA-pinned) and dependencies (`npm ci` from lockfiles), `npx playwright` resolves against whatever `playwright` version is in `site/node_modules` — that part is fine — but if `site`'s devDependency were ever missing/renamed, npx silently falls back to fetching the latest `playwright` from the registry and running its install script. A silent-fallback fetch-and-execute in CI is a latent supply-chain risk.
Fix: use `npx --no-install playwright install …` (fails fast if not installed) or call the local binary directly (`./node_modules/.bin/playwright`). Note `--with-deps` also invokes apt/sudo; acceptable on ubuntu-latest but worth knowing.

### F-3 — LOW — tsconfig typechecks oxlint plugin loader but excludes the plugin implementation
File: `tsconfig.json:22-25`
```json
  "include": ["oxlint.config.ts", "tools/oxlint/**/*.ts"],
  "exclude": ["node_modules", "dist", "tools/oxlint/anti-slop"]
```
The two jsPlugins in `oxlint.config.ts:16-21` load `tools/oxlint/anti-slop/index.ts` and `tools/oxlint/anti-slop/effect/index.ts` at lint runtime, but those files are excluded from the project's strict TypeScript program. Any type error inside the custom rule implementations ships uncaught to CI (the `check` gate's `typecheck` never sees them); a broken rule can fail every developer's lint with an opaque runtime error rather than a compile-time one. Presumably excluded for performance or because the files rely on oxlint-injected globals, but that trade-off isn't documented in-file.
Fix: either re-include `tools/oxlint/anti-slop` under a dedicated tsconfig with ambient declarations for the oxlint plugin API, or add a comment in tsconfig.json documenting why the exclusion exists and how those files get verified (there is a `tools/oxlint/sync-anti-slop.mjs` sync check, but that checks sync, not types).

### F-4 — INFO — artifact upload depends on a comment-enforced invariant
File: `.github/workflows/ci.yml:69-73`
```yaml
      # This job must stay anonymous-only: traces/videos from authenticated
      # specs would land in downloadable artifacts. Run staging specs elsewhere.
```
The guarantee that uploaded playwright traces contain no authenticated-session data is enforced only by this comment plus whatever `test:e2e:anonymous` selects. Nothing structural (directory allowlist, grep gate, separate report dir) prevents someone from adding an authenticated spec that the script picks up, after which tokens/cookies could land in public artifacts on every failing PR. Low likelihood, high blast radius.
Fix: add a cheap guard (e.g. assert the playwright projects/grep used by `test:e2e:anonymous` cover only an `anonymous` directory) or upload only an isolated report directory dedicated to anonymous specs.

### F-5 — INFO — `npm audit` gates CI on unpinned advisory noise
File: `.github/workflows/ci.yml:41` → root `package.json:11-12`
```json
"check": "… && npm run audit && npm run check:deploy && npm test",
"audit": "npm audit && npm audit --prefix site && npm audit --prefix site/workers && npm audit --prefix workers/router"
```
Every PR fails if any transitive dependency has a new advisory between lockfile updates, including advisories with no exploitability in this codebase. Combined with Renovate (`renovate.json` present) this self-heals, but it means CI redness is not always a signal about the change under test. Consider `npm audit --omit=dev` where appropriate or an audit-ci allowlist. Not a security hole — arguably intentional strictness.

### F-6 — INFO — concurrency cancel may discard required status results mid-merge
File: `.github/workflows/ci.yml:10-12`
```yaml
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
```
For `pull_request` events `github.ref` is `refs/pull/N/merge`, so rapid pushes to one PR cancel earlier in-flight runs — expected. Edge case: if a maintainer merges based on a pending check and a new push lands concurrently, the cancelled run shows as cancelled (not failed) on the merge commit's ref history; branch protection must treat non-success as blocking (GitHub does). No action needed; documented for completeness since `cancel-in-progress` on main pushes (group `refs/heads/main`) can also hide which commit last had a green full check.

### F-7 — INFO — prettier `trailingComma: 'es5'` is legacy-flavored
File: `prettier.config.js:9`
```js
  trailingComma: 'es5',
```
Prettier's own recommendation is `'all'`; `'es5'` omits trailing commas in function args, producing larger diffs when signatures gain parameters and inconsistent style versus the rest of the modern toolchain (ES2024 target in tsconfig makes `'all'` fully safe). Cosmetic/consistency only.

### F-8 — INFO — oxlint config relies on TS-config loading of `.ts` plugin specifiers
File: `oxlint.config.ts:16-21`
```ts
  jsPlugins: [
    { name: 'anti-slop', specifier: './tools/oxlint/anti-slop/index.ts' },
    { name: 'anti-slop-effect', specifier: './tools/oxlint/anti-slop/effect/index.ts' },
  ],
```
Relative specifiers resolve against the config file location; correct here (`/omg-web/tools/oxlint/...` exists). Verified `sync-anti-slop.mjs` is wired as `check:anti-slop` inside `npm run check`. No bug — noted that a rename of `tools/oxlint` would break lint at runtime for everyone until CI catches it; a smoke import in the typecheck program (see F-3) would catch it locally.

## Explicitly checked and clean

- No `pull_request_target`, no `workflow_dispatch` with inputs interpolated into run blocks, no `secrets.*` usage at all in the workflow.
- No command injection: zero `${{ github.event.* }}` / `${{ inputs.* }}` inside `run:` blocks.
- All four `cache-dependency-path` entries exist on disk (`package-lock.json`, `site/package-lock.json`, `site/workers/package-lock.json`, `workers/router/package-lock.json`).
- All scripts invoked by CI exist in their respective `package.json` files (`check`, `check:bundle-budget`, `test:e2e:anonymous`).
- `timeout-minutes` present on both jobs; `permissions` minimized at workflow level (no job escalates).
- oxlint `ignorePatterns` cannot be abused to skip auditing real source (excludes only build output, editor dirs, and the plugin implementation itself — see F-3).
- `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `verbatimModuleSyntax`, `useUnknownInCatchVariables` enabled — strong compiler baseline; nothing unsafe disabled.

**Totals: 8 findings (0 CRITICAL / 0 HIGH / 3 LOW / 5 INFO).**


---

# SLICE 33

# slice-33 — Cross-cutting security sweep of `site/workers/src` (omg-web)

Agent: audit33 · Scope: every line of `/home/pyro1121/Documents/omg-web/site/workers/src` (~12.8k LOC), verified against `omg-web-threat-model.md`. Read-only audit; no builds/tests run.

## Verification of threat-model mitigations (summary)

| Threat | Status in current code |
|---|---|
| TM-001 email-binding takeover | Site-side (`site/src/lib/*`) — out of this slice's files; Worker still binds by bare email in `site-session.ts` / `auth.ts` (residual, secret-gated) |
| TM-002 seat bypass via optional `machine_id` | **STILL PRESENT** (F1) |
| TM-005 stale entitlement resurrection | Mitigated (status_rank + aggregate projection, conditional upsert WHERE clauses) |
| TM-006 tier/status drift | **STILL PRESENT** on team/audit endpoints (F2) |
| TM-008 OTP abuse | Partially improved (per-IP + per-email limiters); attempt gate is dead code (F3/F4) |
| TM-009 mixed timestamp formats | **STILL PRESENT** — sessions & OTP expiry comparisons (F5) |
| TM-010 get-license enumeration | Fixed (session + ownership check) |
| TM-011 fleet PII in validate-license | **STILL PRESENT** (F6) |
| TM-013 stale admin flag on OTP login | **STILL PRESENT** (F7) |
| TM-014 metric poisoning | Improved for `/api/analytics` (license required); install-ping dedupe still broken (F8); site-analytics sessions inflation (F9) |
| TM-015 plaintext credentials | Session tokens/license keys still plaintext (F10) |
| TM-016 retention not wired | **PARTIALLY UNFIXED** — `cleanupExpiredAuditLogs` never scheduled (F11) |

Admin authorization sweep: every `/api/admin/*` route is gated — `withAdminContext`/`validateAdmin` in admin.ts handlers, `forbiddenUnlessAdminSession` for firehose/docs-dashboard/site-analytics dashboards, `requireAdmin` for Stripe sync/metrics. No ungated admin route found. Secrets: `ADMIN_API_SECRET` compared timing-safe and fails closed when unset; Turnstile fails closed. CORS: fixed allowlist origin, no reflection; `getCorsHeaders` ignores its parameter but returns the fixed-origin set (safe).

---

## Findings

### F1 — HIGH — Paid-tier seat limit bypassed when `machine_id` omitted
`site/workers/src/handlers/license.ts:247-251` (`registerOrTouchMachine`)
```ts
const machineId = body.machineId;
if (machineId === null) {
  return Effect.succeed(null);
}
```
and contract `contracts/validate-license.ts:32` (`machine_id: Schema.optional(Capped(128))`). A caller holding one valid paid key can POST `/api/validate-license` without `machine_id`, skipping all seat accounting, and still receive a signed tier/features JWT plus full fleet machines+usage. This is threat-model TM-002 verbatim, still unmitigated.
Fix: require a non-null canonical machine id for paid tiers (fail 400 otherwise), or bind JWT issuance to a persisted machine row.

### F2 — MEDIUM — Tier-vs-status drift on team endpoints (TM-006)
`site/workers/src/handlers/dashboard.ts:294-321` (`handleGetTeamMembers`) checks only `isTeamOrEnterpriseTier(license.tier)`; same pattern in `handleGetAuditLog` (`dashboard.ts:~430`) and `account-dashboard.ts` features list (`account-dashboard.ts:~425`). After cancellation, reconciliation sets `status='cancelled'` while keeping historical `tier='team'`, so cancelled subscribers keep Team dashboard features and audit logs indefinitely.
Fix: single effective-entitlement guard `(tier AND status='active')` on all paid routes.

### F3 — HIGH/MEDIUM — OTP brute-force window: attempt counter never increments (TM-008)
`site/workers/src/handlers/auth.ts:394-431` (`verifyCode` claim UPDATE):
```sql
WHERE email = ? AND code = ? AND used = 0
  AND attempt_count < ? AND expires_at > datetime('now')
```
The comment admits failed guesses never increment `attempt_count`; nothing else does either (grep confirms no writer). The `MAX_OTP_ATTEMPTS = 5` gate is dead defense-in-depth that will never fire. Guesses are bounded only by the per-IP limiter (10/min) and the per-email hashed bucket (same 10/min binding shared with send/logout scopes). Combined with F5 (expiry comparison extends validity up to ~24 h), an attacker with a handful of IPs gets ~100+ guesses per code instead of 5.
Fix: increment `attempt_count` atomically on each failed claim keyed to the code row (or store attempts per code at insert and bump on miss by id lookup), and give verify-code its own tighter limiter namespace.

### F4 — MEDIUM — Send-code quota counts after invalidation, and count includes used rows inconsistently
`site/workers/src/handlers/auth.ts:293-306`: the "recent" count query counts ALL codes for the email in the last 10 minutes including invalidated ones (`used=1`) — this is fine as a send quota, but the subsequent batch marks prior codes used *before* inserting the new row inside the same batch, so the quota is effectively "3 sends / 10 min" — correct. However there is no Turnstile verification on the *second/third* send path beyond the first (each send re-requires a fresh Turnstile token — OK), and expired-code purge piggybacks here only; if send-code traffic ceases, expired rows persist (minor retention gap). Also the quota check races two concurrent sends (both read count=2 → both insert): LOW severity, bounded by limiter.

### F5 — HIGH — ISO-`T` vs SQLite space-format timestamp comparison extends session/OTP validity ~24 h (TM-009 unresolved)
- Sessions inserted with `.toISOString()` → `2026-08-23T12:00:00.000Z` (`auth.ts:479`, `site-session.ts:151`), then validated with `WHERE s.expires_at > datetime('now')` (`api.ts:validateSession`, `site-session.ts findSession`, `handlers/auth.ts` OTP claim).
SQLite compares TEXT: `'T'` (0x54) > `' '` (0x20), so any same-day-expired token still satisfies `expires_at > datetime('now')` until the calendar date rolls over. Worker bearer sessions (30-day TTL) and OTP codes (10-min TTL) both over-live their expiry by up to ~24 h. Same defect in `marketing-offer.ts` guard is handled correctly (`datetime(expires_at)`), showing the codebase knows the fix.
Fix: compare with `datetime(expires_at) > datetime('now')` everywhere, or store epoch integers.

### F6 — MEDIUM — validate-license discloses fleet PII to key holders (TM-011 unresolved)
`site/workers/src/handlers/license.ts:337-372`: successful validation returns all active machine rows (user_name, user_email via `machines` selection in `ValidateLicenseRowSchema` consumers — response includes `machines` array with hostname/os/user identity fields) and 30 days of usage for the whole license. Any holder of a shared Team key learns colleagues' names/emails/hostnames/activity.
Fix: return only the activating machine; move fleet data behind session-authenticated dashboard endpoints.

### F7 — MEDIUM — OTP login never resyncs `customers.admin` (TM-013 unresolved)
`site/workers/src/handlers/auth.ts` `findOrCreateCustomer` adopts/creates customers without consulting Better Auth role or clearing a stale `admin=1`. Only the BFF path (`site-session.ts mintSiteSession`) syncs role. A demoted insider whose flag was never manually cleared keeps admin API access forever via fresh OTP logins.
Fix: authoritative role source + deprovisioning workflow that clears `customers.admin` and revokes sessions.

### F8 — LOW — install-ping dedupe is ineffective; public badge inflatable
`site/workers/src/handlers/license.ts handleInstallPing`: `INSERT OR IGNORE INTO install_stats (id, ...)` with `id = crypto.randomUUID()` — the PK never conflicts, so `OR IGNORE` is dead and repeat POSTs (rate-limited only at 100/min/IP, rotating IPs trivially bypass) inflate the publicly displayed installs badge (`worker.ts handleInstallsBadge` counts rows).
Fix: make `install_id` the unique key (unique index) and keep `OR IGNORE`.

### F9 — MEDIUM — Site analytics `sessions` counter inflates on every pageview
`site/workers/src/handlers/site-analytics.ts` (`handleTrackEvent`, geo_daily upsert):
```sql
ON CONFLICT(date, country_code, city) DO UPDATE SET
  pageviews = pageviews + 1,
  sessions = sessions + (CASE WHEN excluded.visitors = 1 THEN 1 ELSE 0 END)
```
`excluded.visitors` is always the literal `1` from the INSERT VALUES, so `sessions += 1` fires on *every* pageview, not once per new visitor/session. All downstream dashboards (`handleGetGeoAnalytics`, overview totals, blended engagement ranking) are skewed. Similarly `site_analytics_hourly` never increments visitors/sessions on conflict (pageviews only) — inconsistent semantics between the two aggregates.
Fix: track per-(visitor,date) seen-set table, or compute unique sessions from `site_analytics_events` distinct `session_id` during aggregation.

### F10 — LOW/INFO — Plaintext credentials at rest (TM-015 residual, accepted risk?)
Session tokens stored verbatim in `sessions.token` and license keys verbatim in `licenses.license_key`; both are also copied into JWT payloads (`lic` claim) and audit metadata. HMAC'd OTPs show the pattern exists (`otp.ts hashOtpCode`); tokens/keys were never migrated.
Fix (long-term): store SHA-256 digests, reveal raw value once.

### F11 — MEDIUM — Promised audit-log retention never enforced (TM-016 partial)
`site/workers/src/handlers/privacy.ts cleanupExpiredAuditLogs` (30-day deletion, documented in `handleDeleteMyData`'s `retention_notice` and `data_retention.audit_logs`) is exported but **never called**: `worker.ts scheduled()` runs only `cleanupDocsAnalytics`, `cleanupStripeEvents`, `cleanupMarketingOfferLeads`. Audit rows containing IP addresses and user agents grow unboundedly, contradicting the privacy disclosure shown to users at deletion time.
Fix: add `cleanupExpiredAuditLogs(env.DB)` to `scheduled()`.

### F12 — LOW — Checkout-status ownership check skipped when session carries no email
`site/workers/src/handlers/billing.ts handleCheckoutSessionStatus:658-663`:
```ts
if (email !== null && email.toLowerCase() !== auth.user.email.toLowerCase()) {
  return errorResponse(...403);
}
```
If `customer_details.email` and `customer_email` are both absent, any authenticated user who learns/guesses a paid `cs_…` id can poll its payment status (license handout itself is still ownership-checked against `c.stripe_customer_id`). Entropy makes exploitation impractical; defense-in-depth fix: reject when `email === null` rather than skipping the check.

### F13 — LOW — Admin note/tag mutations audit success even when target missing
Handlers ignore `'not-found'` outcomes from stores:
- `handlers/admin.ts handleAdminUpdateNote` / `handleAdminDeleteNote` / `handleAdminRemoveTag` always return `{success:true}` and write success audit rows even though `crm.updateNote/deleteNote/removeTag` report `'not-found'`.
Consequence: misleading audit trail and false-success API responses (UX/API-correctness defect).
Fix: map `'not-found'` to 404 and skip (or mark failed) audit entries.

### F14 — LOW — Dead/corrupted code fragment in admin-store
`store/admin-store.ts:~700`:
```ts
/** Load one page of enriched audit events, plus the total event count.
  });
```
An orphaned doc-comment plus dangling `});` remnant wedged between `listAuditLog` and `loadAdvancedMetrics` (harmless because it is inside a comment, but clearly a botched merge leftover; confusing to future editors).

### F15 — LOW — Rate-limiter failure modes inconsistent; some paths fail open
- `enforceIpRateLimit` (auth.ts) fails closed on missing binding (503) but lets a throwing `limiter.limit()` propagate to the outer catch → 500.
- `handleValidateLicense`/`handleReportUsage`/`handleInstallPing`/`handleTrackEvent`/docs analytics **skip limiting entirely** with only a warning when `API_RATE_LIMITER` is undefined (threat model assumption says bindings deployed — acceptable, but the asymmetry vs auth is a footgun).
- `ADMIN_RATE_LIMITER` is declared in wrangler.toml and `Env` but never referenced anywhere in src — dead config; admin routes have no rate limiting at all (only logging of denials). Fix: use it in `adminGated` or delete the binding.

### F16 — INFO — Session-token entropy downgrade on internal mint path
`handlers/site-session.ts:148`: `Schema.decodeUnknownSync(SessionToken)(crypto.randomUUID())` mints 122-bit UUIDv4 bearer tokens, versus `generateToken()`'s 256-bit hex used by OTP login. Also reuses the customer's newest unexpired session if one exists (token re-disclosure to the internal caller). Both are acceptable behind the service-binding + ADMIN_API_SECRET gate but are an unnecessary entropy/rotation downgrade.
Fix: use `generateToken()` for internally minted sessions too.

### F17 — INFO — Stripe signature check: good; minor nits
`billing.ts verifyStripeSignature` correctly handles multi-v1 rotation, strict numeric timestamp parse, replay window, and constant-time compare. Nit: length-mismatch short-circuit `continue` leaks signature-length equality via timing (negligible); duplicate `t=` values take `[0]` silently.

### F18 — LOW — `escapeCSV` formula-neutralization prefix corrupts legitimate data
`handlers/admin.ts escapeCSV`: cells starting with `-` (e.g., negative numbers, ISO dates never start with `-`, but negative amounts do) get a leading `'` prepended, so exports of negative `amount_cents`-derived values render as text in spreadsheets. Acceptable tradeoff, worth documenting; alternatively prefix only `=+-@` when followed by non-numeric content.

### F19 — INFO — `getCorsHeaders(_origin)` ignores its argument and always enables `Access-Control-Allow-Credentials: true`
`api.ts:~150`. Safe today because ACAO is the fixed allowlisted origin, but the unused parameter invites future misuse (someone "fixing" it to reflect Origin would create a credentialed reflection hole). Consider deleting the parameter or asserting it.

### F20 — LOW — `handleLogout` accepts body-less requests and any token without IP throttling benefit; deletes session only after full validation
`auth.ts handleLogout`: logout requires a *valid* session to delete (validateSession first). An attacker holding an expired/stolen-but-invalidated token cannot force deletion (fine), but a user cannot invalidate a corrupted/expired session row that still matches `token` yet fails `expires_at > datetime('now')` — those rows linger until... there is **no sessions cleanup job at all** (grep of scheduled handler). Expired session rows accumulate indefinitely (privacy/retention issue, complements F11).
Fix: prune expired sessions in the cron.

### F21 — LOW — `handleRevokeTeamMember` lacks tier check and uses confusing id semantics
`dashboard.ts`: free/pro users may call `/api/team/revoke` and deactivate their own machines by internal row `id` (not `machine_id`), unlike `handleRevokeMachine`. Row ids are only exposed via team-tier endpoints, so practical impact is minimal, but the missing tier guard is inconsistent with `handleGetTeamMembers`.

### F22 — INFO — `reportUsage` MAX() upsert semantics
`handlers/license.ts`: daily usage columns take `MAX(existing, incoming)` — a machine whose counters reset (reinstall, clock skew) can never lower a day's numbers; intentional anti-regression choice but means `usage_member_daily` can diverge from reality. Documented behavior; noting for completeness.

### F23 — LOW — `analytics_active_users` insert loop outside batch
`license.ts ingestAnalytics`: after the main batch, one D1 round-trip per unique machine (`for...of runSql`) — N+1 cost on attacker-influenceable input (up to 50 events/batch → up to 50 extra queries). DoS-cost amplifier under the 100/min/IP limiter. Fix: fold into the batch with `INSERT OR IGNORE`.

### F24 — INFO — Docs analytics trust boundary
`docs-analytics.ts` persists arbitrary client `event_type/event_name/properties` strings (schema caps lengths at 64/128 via contract) into raw events later JSON_EXTRACT-ed into aggregates; interaction targets are unbounded-keyed (`COUNT GROUP BY target`) allowing high-cardinality row growth within the 7-day retention. Rate-limited and capped; residual metric-poisoning surface remains (TM-014 partial).

## Clean bill areas (explicitly verified, no findings)
- SQL injection: every query parameterized; only interpolations are compile-time constants (`MAX_SESSIONS_PER_CUSTOMER`, retention day constants, `whereColumn` union, saltHex from CSPRNG hex).
- Stripe webhook ingestion: signature + replay window + durable claim lease with claim_token-guarded state transitions; PII payload truncation post-processing.
- CSV export: formula injection neutralized; BOM + RFC4180 quoting correct.
- Body parsing: 1 MiB streamed cap with chunk counting (Content-Length lie-proof); Content-Type JSON enforcement blocks cross-origin form CSRF-style posts.
- Timing-safe comparisons for ADMIN_API_SECRET (padded) and Stripe signatures.
- Turnstile fail-closed when secret unset; AUTH_RATE_LIMITER fail-closed for auth endpoints.


---

# SLICE 34

# Slice 34 — Cross-cutting security sweep of OMG (Rust, ~/Documents/omg)

Read-only grep-driven audit: command injection (`Command::new` / `format!` / `sh -c`), unsafe blocks,
unwrap/panic paths reachable from CLI input, path traversal, symlink attacks, TOCTOU, privilege
escalation logic. 557 `.rs` files scanned; every finding verified by reading surrounding code.

**Overall note:** this repo is unusually well-hardened (visible `aud-*` regression comments for prior
audit waves): package names are validated via a strict allowlist (`src/core/security/validation.rs`),
AUR builds run in bubblewrap with hook/PATH isolation, daemon sockets use umask+0600+uid checks,
self-update has checksum + downgrade protection, and elevation scrubs LD_PRELOAD/PYTHONPATH/etc.
The findings below are the residual defects that survive that hardening.

---

## Findings

### 1. MEDIUM — Workspace config from CWD executes arbitrary shell with no trust gate; unvalidated `project.path` allows running outside the workspace
- **File:** `src/cli/workspace.rs:405-413` (`run_project_command`), `src/cli/workspace.rs:311-315`, `src/cli/workspace.rs:352-357`, load at `src/cli/workspace.rs:60-69`
- **Code:**
  ```rust
  if let Some(custom_cmd) = project.commands.get(command) {
      let status = std::process::Command::new("sh")
          .arg("-c")
          .arg(custom_cmd)
          .current_dir(path)
          .status()
          .context("Failed to execute command")?;
  ```
- **Why it's a bug:** `Workspace::load()` reads `omg-workspace.toml` from the current directory with
  no provenance check (unlike team configs in `src/core/env/team.rs:132-156`, which explicitly
  reject symlinked/untrusted paths). Any cloned repository containing a malicious
  `omg-workspace.toml` gets arbitrary shell execution the moment a user runs any
  `omg workspace <cmd>` inside it. Additionally, `project.path` is used directly as
  `current_dir(path)` (line 411, also 425, 458, 514) with no containment check against the
  workspace root — `"path = "../.."` or an absolute path makes omg execute commands anywhere on
  disk while printing the workspace banner.
- **Fix:** require an explicit opt-in (e.g. `omg workspace trust`) recorded outside the repo before
  executing `commands`; validate `project.path` resolves to a directory inside the workspace root
  (canonicalize + `starts_with`), and reject absolute/traversal paths at parse time.

### 2. MEDIUM — `SUDO_HOME` without a leading `/` produces a cwd-relative cache path as root
- **File:** `src/core/paths.rs:100-118` (`cache_dir`)
- **Code:**
  ```rust
  let home = match std::env::var("SUDO_HOME") {
      Ok(dir) if is_valid_username(&dir) => PathBuf::from(dir),
      ...
  };
  return home.join(".cache/omg");
  ```
- **Why it's a bug:** `is_valid_username` only rejects empty, `/`, `\0`, `..`, >256 chars. A value
  like `SUDO_HOME=foo` passes validation but `PathBuf::from("foo").join(".cache/omg")` is
  *relative*, so when omg runs elevated (via its own sudo re-exec, which preserves selected env),
  cache reads/writes land in `$PWD/foo/.cache/omg` of whatever directory the elevated process
  inherits — potentially attacker-influenced (e.g. a project dir). Same class applies to the
  `DOAS_USER` branch which hardcodes `/home/{user}` (fine) but to `SUDO_USER` fallback too.
- **Fix:** after validation, require `dir.starts_with('/')` (or canonicalize and verify it is a
  directory owned by `sudo_user`'s uid); otherwise warn and fall back to `/home/{sudo_user}`.

### 3. MEDIUM — task_runner rejects legitimate argv arguments containing shell metacharacters, contradicting its own security model (UX-breaking false failures)
- **File:** `src/core/task_runner.rs:831-842`
- **Code:**
  ```rust
  // SECURITY: Validate extra_args to prevent command injection
  for arg in extra_args {
      if arg.contains(';') || arg.contains('|') || arg.contains('&')
          || arg.contains('`') || arg.contains('$') || arg.contains('\n') {
          anyhow::bail!("Invalid argument '{arg}' - contains shell metacharacters");
      }
  ```
- **Why it's a bug:** eleven lines earlier (line 826-829) the code documents correctly that the
  command is spawned argv-directly so metacharacters are inert — yet `extra_args` are still
  hard-rejected for `;|&\`$`. Real usage breaks: `omg run test -- --filter foo|bar`,
  `omg run lint -- --select E501,W704`, `--grep 'foo$'`, awk/sed one-liners, etc. The comment
  "prevent command injection" is wrong for an argv spawn; the check is both dead security weight
  and a correctness defect.
- **Fix:** delete the metacharacter bail (keep only NUL/control-character rejection, matching
  `validate_executable_command`), or downgrade it to a warning.

### 4. LOW/MEDIUM — self-update integrity depends entirely on the same channel it is updating from (no signature/pinned digest); fetch-to-verify TOCTOU
- **File:** `src/cli/self_update.rs:73-77` and `297+` (`download_verified`)
- **Code:**
  ```rust
  let expected_digest = fetch_checksum(&artifact.checksum_url()).await?;
  let bytes = download_verified(&artifact.download_url(), &expected_digest).await?;
  ```
- **Why it's a bug:** digest and archive come from the same host/channel; a compromised release
  feed serves a consistent malicious pair and passes verification. There is no sigstore/PGP
  signature check even though the crate ships `security/slsa.rs` and `pgp.rs`. Also, between
  `fetch_checksum` and `download_verified` the feed can be swapped (TOCTOU is benign here since the
  digest pins the download, but it means the "integrity gate" protects only against transport
  errors, not supply-chain compromise).
- **Fix:** verify a detached signature (minisign/sequoia) made by a key pinned at build time, or
  pin digests via an independent channel.

### 5. LOW — self-update leaves a window where no binary exists; unconditional 0o755; stale `.old` backup
- **File:** `src/cli/self_update.rs:88-125`
- **Code:**
  ```rust
  fs::rename(&current_exe, &backup_path)...;
  match fs::rename(&new_binary, &current_exe) { ... }
  perms.set_mode(0o755);
  ```
- **Why it's a bug:** (a) if the process is killed between the two renames, `omg` is missing until
  manual restore; (b) mode 0o755 is forced regardless of the original umask/mode (group/other
  executable bit granted unconditionally); (c) the new binary's ownership is whatever the
  extracting user was (running under sudo would install a root-owned file into a user prefix).
  Failure restore path is handled well (error surfaces restore failure).
- **Fix:** copy mode from `backup_path` before renaming over; consider `renameat2(RENAME_EXCHANGE)`
  or write-then-symlink-swap; refuse self-update when euid differs from the target dir owner.

### 6. LOW — tool linking deletes any existing entry in the managed bin dir without checking ownership/type
- **File:** `src/cli/tool.rs:487-495` (`link_binaries`)
- **Code:**
  ```rust
  let dest = bin_dir.join(filename);
  if dest.exists() || dest.symlink_metadata().is_ok() {
      fs::remove_file(&dest)?;
  }
  symlink(&path, &dest).context("Failed to symlink binary")?;
  ```
- **Why it's a bug:** any pre-existing regular file, directory, or foreign symlink at
  `<data>/bin/<filename>` is silently removed (`fs::remove_file` on a *directory* errors, but a
  real user binary placed there — e.g. by an older omg version or manually — is destroyed without
  prompt or backup). Also `install_managed` at line 320-337 does `remove_dir_all(&install_dir)` on
  an existing tool dir: safe today because `pkg` passed `validate_package_name`, but the guard is
  only as strong as every future call site remembering to validate.
- **Fix:** only remove entries that are symlinks resolving into `tools_dir`; otherwise bail with a
  clear message. Add a debug_assert/explicit re-validation of `pkg` inside `install_managed`.

### 7. LOW — `create_systemd_service` writes through `$HOME` string concatenation, follows symlinks, and hardcodes a possibly-wrong ExecStart path
- **File:** `src/cli/init.rs:897-921`
- **Code:**
  ```rust
  let home = std::env::var("HOME")?;
  let service_dir = format!("{home}/.config/systemd/user");
  std::fs::create_dir_all(&service_dir)?;
  ...
  std::fs::write(format!("{service_dir}/omgd.service"), service_content)?;
  ```
- **Why it's a bug:** (a) `std::fs::write` follows a pre-planted symlink `omgd.service -> victim`,
  overwriting the target (same-user threat, low impact, inconsistent with the symlink hygiene
  elsewhere in the repo); (b) `ExecStart=%h/.local/bin/omgd` assumes that exact install location —
  if omgd lives elsewhere (cargo install default `~/.cargo/bin`, distro package), the enabled unit
  fails at start with a confusing error; (c) uses raw `$HOME` instead of `crate::core::paths`
  helpers.
- **Fix:** write atomically via tempfile + rename (which also refuses symlinked destinations when
  using `symlink_metadata` checks first), resolve the actual `current_exe()` path into the unit,
  reuse `paths::config_dir()`.

### 8. LOW — container base-image policy falls back silently instead of failing closed
- **File:** `src/core/container.rs:328-332` (and 360)
- **Code:**
  ```rust
  "Refusing unsafe base image {base_image:?}; falling back to ubuntu:24.04"
  ```
- **Why it's a bug:** a policy violation ("unsafe base image") downgrades to a *default image* and
  continues building rather than aborting. Users get a successful build of something different from
  what they asked for — a silent behavioral substitution in a security control. Fail-closed (bail)
  is the correct semantics for policy rejection; fallback should be opt-in.
- **Fix:** return an error for rejected images; make any fallback explicit via config flag.

### 9. LOW — mmap'd index/db files can SIGBUS if the underlying file shrinks concurrently
- **Files:** `src/package_managers/debian_db/db.rs:177,221,2380-2411,2608-2620`; `src/package_managers/aur_index.rs:66`
- **Code:** `let mmap = unsafe { Mmap::map(&file)? };` plus unchecked byte-slice reads at fixed offsets.
- **Why it's a bug:** the unsafe blocks establish the invariant that the mapped region stays valid;
  nothing enforces it. If another process (another omg instance mid-refresh) truncates/rewrites the
  db/index while it is mapped, the kernel raises SIGBUS → process crash, not an `Err`. The unsafe
  blocks themselves contain no comments documenting this precondition.
- **Fix:** document SAFETY, hold a shared lock while mapping, or copy small headers through
  `read()` and bounds-check all offset arithmetic against `mmap.len()` before slicing.

### 10. LOW — `USER` env var trusted for chown target during AUR ownership repair
- **File:** `src/package_managers/aur/client.rs:1288-1296`
- **Code:**
  ```rust
  let current_user = std::env::var("USER")
      .or_else(|_| whoami::username())
      .unwrap_or_else(|_| "nobody".to_string());
  ... .args(["chown", "-R", &format!("{current_user}:{current_user}")]).arg(pkg_dir)
  ```
- **Why it's a bug:** `USER` is arbitrary caller-controlled env; under `sudo -u <x>` contexts or a
  spoofed env, chown targets the wrong account (fails harmlessly in most cases, but can hand the
  build tree to an unintended local user if that username exists). No injection risk (argv), but
  the authoritative source (`whoami::username()` / `geteuid()`→passwd lookup) is demoted to
  fallback.
- **Fix:** prefer the uid-based passwd lookup; drop the `USER` fast-path.

### 11. INFO — Daemon socket startup TOCTOU window (well-mitigated, residual race remains)
- **File:** `src/bin/omgd.rs:121-197`
- **Why it's notable:** between the `metadata(&socket_path)` uid check, `remove_file`, and
  `UnixListener::bind`, an attacker with write access to the socket directory can swap the node.
  Mitigations already present (umask tightened around bind, 0600 chmod, uid==self-or-root check,
  RAII cleanup) reduce this to a narrow same-directory race; noting for completeness. Consider
  binding in a private parent dir or verifying inode identity post-bind.

### 12. INFO — `omg new` passes raw user arg to `cargo new` / `poetry new` / `go mod init`
- **File:** `src/cli/new.rs:65,80,176,206`
- **Why it's notable:** argv-spawned so no injection, but names starting with `-` are forwarded as
  option-looking operands (e.g. `omg new --help` manipulates cargo, not creates a project), and
  names containing path components create projects outside expectations. Every other surface in
  the repo runs `validate_package_name` first; this one does not.
- **Fix:** apply the shared name validator (allowing a leading `./` explicitly if relative paths
  should work).

### 13. INFO — maintainer scripts from archives executed with inherited privileges (inherent, but chmod/exec TOCTOU)
- **File:** `src/package_managers/debian_db/transaction.rs:1316-1338` (`run_maintainer_script`)
- **Why it's notable:** archive-controlled maintainer scripts are chmod'd 0o755 then executed with
  omg's privileges (root when elevated) — inherent to dpkg-style installs and documented behavior,
  but the chmod→exec pair has a classic TOCTOU gap if the staging directory is writable by another
  principal. Staging dirs appear to be root-owned temp paths, keeping this informational. Consider
  `O_NOFOLLOW`/openat + `fchmod` on the held fd, and executing via `/proc/self/fd`.

---

## Verified non-issues (checked, found sound)
- `task_runner.rs` command spawn is argv-direct; venv PATH prepend is scoped to the child.
- `privilege.rs`: elevation whitelist (`ALLOWED_ROOT_OPS`), argv marker instead of env for
  `OMG_ELEVATED`, askpass/LD_*/language-path env scrubbing, exit-code propagation via
  `run_self_sudo`.
- `aur/client.rs`: bubblewrap sandbox tests, `core.hooksPath=/dev/null` + `GIT_CONFIG_NOSYSTEM` +
  `protocol.file.allow=user` on pulls, `validate_build_dir`/`validate_path_inside` anti-symlink
  checks, `_pkgdest/_srcdest` 0o700.
- `validation.rs`: strict package-name/image-ref/version allowlists incl. traversal and
  option-prefix rejection.
- `bin/omgd.rs`: stale-socket uid checks, umask-scoped bind, RAII socket cleanup, panic capture.
- `secrets.rs` / `team.rs`: symlink-cycle-safe directory scans; symlinked config files/dirs
  rejected fail-closed.
- `dnf.rs`: repomd href location validation present.

## Summary
13 findings: 0 CRITICAL, 0 HIGH, 3 MEDIUM (workspace sh -c + path escape; SUDO_HOME relative path;
task_runner metachar rejection breaking legit args), 7 LOW, 3 INFO. Highest-value fixes: #1
(workspace trust gate + path containment), #2 (require absolute SUDO_HOME), #3 (delete the
contradictory metachar bail).


---

# SLICE 35

# Slice 35 — omg-web `site/src/components/` + `site/src/routes/` cross-cutting audit

Scope: every file under `/home/pyro1121/Documents/omg-web/site/src/components/` and `/home/pyro1121/Documents/omg-web/site/src/routes/` (~13.5k lines, 62 files). Read-only audit; no builds/tests executed. Supporting files in `~/lib`, `~/shared` were inspected only where components/routes consume them.

## Executive summary

No XSS sinks exist anywhere in scope: zero uses of `innerHTML`, `{@html}`, or `dangerouslySetInnerHTML`. The API layer (`lib/api.ts`) parses all responses through Effect Schema before state, and the two API routes (`api/dashboard.ts`, `api/licensing/[...path].ts`) are well-structured with typed error channels and opaque infrastructure failures. The dominant problems in this slice are (1) **fabricated analytics** presented to admins as real data in several dashboard tabs, (2) a **weekly-cohort dataset rendered as monthly** with an invalid date format, (3) client-side-only search that silently filters only the current page of server-paginated data, and (4) assorted UX/race/dead-code defects.

Findings: 0 CRITICAL, 2 HIGH, 9 MEDIUM, 12 LOW, 6 INFO.

---

## HIGH

### H-1. Segment metrics are largely fabricated numbers presented as real analytics
- File: `site/src/components/dashboard/admin/SegmentAnalytics.tsx` lines ~247–310 (`segmentMetrics` createMemo)
- Excerpt:
  ```ts
  if (segment.id === 'power_users') {
    userCount = Math.round(mau * 0.1);
    avgLtv = (ltvMap['enterprise'] || 0) * 0.8;
    churnRisk = 5;
    mrrContribution = currentMRR * 0.4;
  } ...
  churnRisk = 65; // at_risk — hardcoded
  ```
- Why it's a bug: "Power Users" count is invented as 10% of MAU, churn risk percentages are hardcoded constants (5/25/8/12/18/65), LTVs are arbitrary fractions of tier averages, MRR contributions are fixed splits (40/5/45/30/20% — which sum to >100%). These drive the segment summary cards ("At-Risk Revenue", "High-Value Users"), comparison charts, donut totals, and Venn diagram. An admin making pricing/churn decisions from this screen is reading fiction.
- Fix: derive each metric from actual queries (the API already exposes per-tier counts/LTV and churn-risk segments), or clearly label estimates as estimates. Do not render hardcoded ratios as measured values.

### H-2. Weekly cohort data rendered as "Monthly" retention with an unparseable date format
- Files: `site/src/components/dashboard/admin/insights/InsightsTab.tsx` lines ~276–286; consumer `site/src/components/dashboard/admin/analytics/CohortRetentionHeatmap.tsx` lines 60–63 (`formatMonth`) and 44–52
- Excerpt (InsightsTab):
  ```tsx
  <CohortRetentionHeatmap
    data={cohorts().map(c => ({ cohort_month: c.cohort_week, month_index: c.weeks_since_signup, active_users: c.active_users }))}
    maxMonths={12}
  />
  ```
  ```ts
  function formatMonth(monthStr: string): string {
    const date = new Date(monthStr + '-01');
    return date.toLocaleDateString('en-US', { month: 'short', year: '2-digit' });
  }
  ```
- Why it's a bug: the schema (`site/src/lib/contracts/worker-http.ts` line ~234) types `cohort_week: Str`; the sibling component `CohortAnalysis.tsx` renders it raw as e.g. `2024-W05`/date strings. Appending `-01` to a week string (e.g. `"2024-05"`) yields `"2024-05-01"` only by luck if the worker sends `YYYY-MM`; if it sends full dates or ISO-week labels, `new Date(...)` is `Invalid Date` and every row label renders "Invalid Date". Independently, labeling weekly retention columns `M0..M12` ("Monthly retention tracking") misrepresents the data by a factor of ~4 for business decisions.
- Fix: pass through a truthful label (`W0..W12`, "Weekly retention") or fetch true monthly cohorts, and parse the actual `cohort_week` format defensively (fall back to raw string when `isNaN(date.getTime())`).

---

## MEDIUM

### M-1. CRM search does not reset pagination → stale page renders empty results
- Files: `site/src/components/dashboard/AdminDashboard.tsx` lines ~694–703 (`onSearchChange={search => actions.setCRMSearch(search)}`); `site/src/components/dashboard/admin/tabs/CRMTab.tsx` lines ~54–56, 176–180
- Why: `crmUsersQuery` keys on `[page, 25, search]`. When the admin searches while on page N>1 and results shrink below N pages, TanStack Query fetches page N of the new result set → likely `users: []` and the table shows "No customers found" even though matches exist. No code resets `store.crm.page` on search change.
- Fix: have `setCRMSearch` also set page 1 (or clamp page in the query layer).

### M-2. Audit-log search filters only the current server page, silently
- File: `site/src/components/dashboard/admin/AuditLogTab.tsx` lines ~78–90 (`logs()` filter), 65 (`limit = 25`)
- Excerpt:
  ```ts
  return allLogs.filter(log => log.user_email?.toLowerCase().includes(query) || ...)
  ```
- Why: `useAdminAuditLog(page, limit, actionFilter)` paginates server-side; `searchQuery` is applied purely client-side over the 25 rows on the current page. A search for an email/IP that exists in the log but not on the current page returns "No audit logs found" — a false negative on a security surface.
- Fix: send `search` as a server query parameter like `actionFilter`, or label the field "filter this page".

### M-3. Revenue tab presents hardcoded fabricated breakdown and growth rate
- File: `site/src/components/dashboard/admin/RevenueTab.tsx` lines ~250–305
- Excerpt: `{formatCompactCurrency((revenue()?.mrr || 0) * 0.6)}` with `width: '60%'`, `<p class="text-lg font-black text-white">+12.5%</p>`
- Why: "Revenue by Tier" is a fixed 60/30/10 split of MRR and the growth rate is the literal string `+12.5%` — no data source at all. Same class of issue as H-1 but smaller blast radius.
- Fix: compute per-tier MRR from real data or remove the card; never render a constant growth figure.

### M-4. Firehose transform defaults unknown success to OK
- File: `site/src/components/dashboard/AdminDashboard.tsx` lines ~216–231 (`transformFirehoseEvents`)
- Excerpt: `success: e.success !== false`
- Why: when the upstream event omits `success`, failures become indistinguishable from successes in the live command feed ("OK"/green). Combined with the fallback `duration_ms: e.duration_ms || 0`, missing telemetry is silently coerced into optimistic values on an ops surface whose purpose is surfacing errors.
- Fix: default to `unknown` and render "—" rather than fabricating OK/0ms.

### M-5. `transformToCRMCustomer` fabricates health sub-scores and MRR
- File: `site/src/components/dashboard/AdminDashboard.tsx` lines ~262–292
- Excerpt:
  ```ts
  engagement_score: Math.min(100, score + 10),
  predicted_churn_probability: stage === 'at_risk' ? 0.6 : stage === 'churned' ? 0.9 : 0.1,
  mrr: user.tier === 'enterprise' ? 199 : ...,
  ```
- Why: health dimension scores are `score ± constants`, churn probabilities are lookup constants, and per-customer MRR is list price regardless of seats/discounts. Displayed in the drawer/CRM table as if computed.
- Fix: source these fields from the backend or annotate them as derived heuristics.

### M-6. Predictions tab churn MRR-at-risk understated after `.slice(0, 6)`
- File: `site/src/components/dashboard/admin/PredictiveInsights.tsx` lines ~104–108 and ~330–338
- Why: `churnPredictions()` truncates segments to 6 via `.slice(0, 6)`, but the header and "AI-Powered Summary" both sum `p.mrrAtRisk` over the truncated array, so displayed "$X MRR at risk" excludes high/critical segments 7+. Either sum before slicing or show "top 6 segments".
- Fix: compute total over the unsliced filtered set; keep the slice only for rendering cards.

### M-7. `RealTimeCommandCenter` heatmap is dead code and would produce NaN cells if wired
- File: `site/src/components/dashboard/premium/RealTimeCommandCenter.tsx` lines 15, 88–94, 236–247
- Excerpt:
  ```ts
  commandHeatmap?: AdvancedMetrics['command_heatmap'];
  const heatmapData = createMemo(() => (props.commandHeatmap ?? []).map(item => ({
    x: Number.parseInt(item.hour, 10), y: Number.parseInt(item.day_of_week, 10), ...
  ```
- Why: the only caller (`OverviewTab.tsx`) never passes `commandHeatmap`, so the entire section can never render (dead prop/code). If it ever were wired, `day_of_week` may be a weekday name; `Number.parseInt('mon')` → `NaN`, producing invisible/misplaced heatmap cells with no validation.
- Fix: delete the dead branch, or validate parsed coordinates (`Number.isNaN` guard) and map day names to indices before wiring.

### M-8. AdminDashboard `TabButton` component defined inside parent body
- File: `site/src/components/dashboard/AdminDashboard.tsx` lines ~395–435 (`const TabButton = (props ...) =>`)
- Why: In Solid, a component defined inside another component's body is a fresh function identity on every parent re-run, so all eight tab buttons remount whenever the parent re-renders — destroying focus state mid roving-tabindex keyboard navigation (focus is patched up afterwards via a `setTimeout` + `document.querySelector`, itself fragile). Same pattern in `CustomerDetailDrawer.tsx` (`SectionButton`, lines ~186–196).
- Fix: hoist these to module scope and pass reactivity through props/accessors.

### M-9. `offer.ts` accepts requests with no `Origin` header and forwards unvalidated body
- File: `site/src/routes/api/offer.ts` lines ~25–50
- Excerpt:
  ```ts
  const origin = event.request.headers.get('Origin');
  if (requestUrl === null || (origin !== null && origin !== requestUrl.origin)) { fail(403) }
  ...
  const body = yield* Effect.tryPromise(() => event.request.text());
  ```
- Why: (a) A request with `Origin` absent bypasses the CSRF check entirely; browsers always send `Origin` on cross-site POST form submissions, but some tooling/older clients do not — the check should require same-origin positively (compare against `Sec-Fetch-Site` too). (b) Body size is capped but content type / shape is not validated at this boundary; arbitrary bytes are forwarded to the internal service with the admin secret attached. The downstream service presumably parses JSON, but defense-in-depth at this public proxy is cheap.
- Fix: reject missing/non-same-origin origin explicitly; verify `Content-Type: application/json` before proxying.

---

## LOW

### L-1. `CustomerDetailDrawer` focus-trap reads DOM once per keydown and can trap nothing
- File: `site/src/components/dashboard/admin/CustomerDetailDrawer.tsx` lines ~48–77
- Why: `focusableElements[0]` may be `undefined` when the drawer shows only the loading skeleton; Tab handling then does nothing (`lastElement?.focus()`), and Tab can escape the modal into background content while loading. Also the effect registers `document.addEventListener` per open — correct via `onCleanup`, but the initial focus targets `drawerRef?.querySelector('button')` (first button = close button, fine) using a magic timeout rather than after content paints.
- Fix: guard the trap when zero focusable elements; prefer focusing the dialog element itself.

### L-2. Save-View modal lacks Escape/backdrop dismissal
- File: `site/src/components/dashboard/AdminDashboard.tsx` lines ~755–805
- Why: the save-view dialog is a bare overlay div; Escape and clicking the backdrop do nothing, inconsistent with every other modal in the app (Kobalte dialogs elsewhere).
- Fix: use the Kobalte Dialog like `UpgradeModal`/`LicenseSuccessModal`.

### L-3. Audit-log action filter badge double-counts combined filters
- File: `site/src/components/dashboard/admin/AuditLogTab.tsx` lines ~140–146
- Excerpt: `{(actionFilter() ? 1 : 0) + (searchQuery() ? 1 : 0)}`
- Why: minor cosmetic; the count reflects inputs, not applied filters, and updates on every keystroke. Not incorrect per se, but inconsistent with `hasActiveFilters` semantics used elsewhere.

### L-4. `EngagementMetrics` AnimatedCounter animates from previous value on data refresh
- File: `site/src/components/dashboard/admin/insights/EngagementMetrics.tsx` lines ~39–61
- Why: `createEffect` re-runs whenever `props.value` changes and animates `startValue → target` over 1200 ms. During polling refetches, every metric continuously re-animates, making numbers hard to read and CPU-spinning rAF callbacks. Also `untrack(() => displayValue())` still makes the effect depend only on `props.value` (intended), but there is no threshold to skip animation for small deltas.
- Fix: animate once on mount; update instantly on subsequent changes.

### L-5. `Tooltip` positions are not viewport-clamped
- File: `site/src/components/ui/Tooltip.tsx` lines ~36–72
- Why: tooltips anchored near screen edges overflow the viewport (no flip/clamp logic), and the tooltip stays visible until `mouseleave`/`focusout` — scrolling with the pointer stationary leaves a floating orphan tooltip at stale coordinates (position captured once at enter).
- Fix: clamp within viewport and recompute or hide on scroll.

### L-6. `NotesSection` delete button has no confirmation and is invisible without hover
- File: `site/src/components/dashboard/admin/NotesSection.tsx` lines ~150–157
- Why: destructive note deletion fires immediately on click (`opacity-0 … group-hover:opacity-100`), unreachable by keyboard users (opacity-0 buttons remain focusable but invisible) and prone to accidental deletion on touch devices where hover doesn't exist.
- Fix: add confirm step or undo; make the control focus-visible instead of hover-gated.

### L-7. `TagsSection` create-tag flow ignores mutation-in-flight states
- File: `site/src/components/dashboard/admin/CustomerDetailDrawer.tsx` (handlers ~118–134) + `TagsSection.tsx` `handleCreateTag`
- Why: `handleCreateTag` clears the form immediately (`setNewTagName(''); setShowNewTagForm(false)`), so if `createTagMutation` fails the typed tag name is lost and the error banner appears far above in the CRM section with no way to retry the same input. Same fire-and-forget pattern for assign/remove tag.
- Fix: clear form only on mutation success; keep input for retry.

### L-8. `DocsAnalytics` reimplements fetching outside TanStack Query with ad-hoc race guard
- File: `site/src/components/dashboard/admin/DocsAnalytics.tsx` lines ~21–43
- Why: manual `createEffect` + `isCurrent` flag duplicates what the project's `createQuery` convention already provides (caching, retries, dedupe, error typing). Not a bug today (guard is correct), but drifts from architecture standards and loses cache invalidation consistency with the rest of the dashboard.
- Fix: migrate to `useQuery` with a `['docs-analytics', days]` key.

### L-9. Raw provider error messages surfaced to admin UI
- Files: `site/src/routes/login.tsx` (~line 34 `result.error.message`), `AuditLogTab.tsx`/`InsightsTab.tsx` etc. `{query.error?.message}`
- Why: `signIn.email`/`signIn.social` provider messages and TanStack error messages are rendered verbatim. Auth endpoints should return stable generic messages to avoid account-enumeration hints; internal query errors (already parsed) may include upstream transport text. Low because surfaces are admin-facing and the auth client controls its own messages.
- Fix: map auth errors to fixed copy; keep details in Sentry.

### L-10. `AdminDashboard.handleExport` has no user-visible failure feedback
- File: `site/src/components/dashboard/AdminDashboard.tsx` lines ~300–320
- Why: export failures go only to `reportClientError` (Sentry); the button just stops spinning and nothing tells the admin the CSV wasn't downloaded.
- Fix: surface a toast/error state.

### L-11. `PredictiveInsights` refresh button disabled-state mismatch
- File: `site/src/components/dashboard/admin/PredictiveInsights.tsx` lines ~283–297
- Why: `disabled={metricsQuery.isRefetching || usersQuery.isRefetching}` — during the very first load (`isLoading`, not refetching) the Refresh button is clickable while skeletons show; clicking triggers parallel duplicate requests. Cosmetic/race-y but harmless due to query deduplication.

### L-12. Sticky column offsets in `CohortRetentionHeatmap` assume fixed pixel widths
- File: `site/src/components/dashboard/admin/analytics/CohortRetentionHeatmap.tsx` lines ~232, 262 (`sticky left-[100px]`)
- Why: the "Users" sticky column is offset a hardcoded 100px from the cohort-name column, but the name cell width is content-driven (`px-3` + formatted month text); with long month labels the Users column overlaps or gaps. Visual defect on narrow viewports.
- Fix: give the first column a fixed width matching the offset.

---

## INFO

### I-1. `churn_rate` KPI divides at-risk users by MAU
- File: `site/src/components/dashboard/AdminDashboard.tsx` lines ~110–117 (`transformToExecutiveKPI`)
- Why: `(atRiskUsers / (metrics?.engagement?.mau || 1)) * 100` conflates CLI-engagement MAU with billing churn denominators and hides division-by-zero behind `|| 1`. It's a heuristic labeled "Churn rate" on the executive index. Document or rename.

### I-2. `stickiness` parsing strips `%` then `parseFloat`
- File: `site/src/components/dashboard/AdminDashboard.tsx` line ~113
- Why: `parseFloat(stickiness?.replaceAll('%','') || '0')` accepts any junk prefix (`parseFloat('abc%')` → NaN propagates to `.toFixed`). Schema should deliver a number; parsing again downstream suggests weak contract. NaN renders as "NaN%" in the UI.

### I-3. Country-code flag emoji math assumes exactly 2 ASCII letters
- File: `site/src/components/dashboard/admin/tabs/AnalyticsTab.tsx` (`GeoDistributionCard`, ~line 466)
- Why: `geo.country_code.length === 2` guards regional-indicator construction, but lowercase codes produce wrong flags; non-country dimensions (e.g. `"unknown"` from DocsAnalytics-style feeds) fall back to 🌍 correctly. Fine, but uppercase before mapping.

### I-4. "AI-powered" labeling of deterministic heuristics
- Files: `PredictiveInsights.tsx` ("AI-Powered Summary", "AI-powered predictions"), thresholds like `upgradeProbability` are static tables.
- Why: marketing copy misdescribes simple arithmetic; consider honest wording ("rule-based").

### I-5. `sitemap.xml.ts` sets `X-Robots-Tag: noindex` on the sitemap
- File: `site/src/routes/sitemap.xml.ts` line ~74
- Why: harmless (sitemaps aren't indexed as documents), but unusual; some crawlers log it as a soft warning.

### I-6. `robots.txt.ts` blocks Anthropic/OpenAI crawlers wholesale
- File: `site/src/routes/robots.txt.ts`
- Why: intentional per comments; noted only because it also blocks `ChatGPT-User` (user-initiated browsing), affecting link previews in AI tools. Business choice, not a bug.

---

## Verified-clean areas (explicitly checked)

- **XSS sinks**: none. All dynamic output goes through Solid text interpolation; inline styles involving untrusted data are sanitized via `tag-color.ts` `parseTagColor` (`^#[0-9a-fA-F]{6}$`) — correctly closes the CSS-injection sink for tag colors. `JSON.stringify(entry.metadata)` in AuditLogTab renders as text, safe.
- **Effect-Schema boundaries**: `routes/api/dashboard.ts` decodes D1 rows (`readD1RowArray`) and re-validates outbound payloads (`parseAccountDashboard`); `InsightsTab.loadBookmarkedInsights` validates localStorage with `Schema.Array(Schema.String)`; `lib/api.ts` routes everything through schema-parsed `apiRequest`. Compliant with the SVELTE-5-EFFECT-TS standards' boundary rules.
- **AuthZ**: `admin.tsx` requires admin via server query with redirect ladder (401→login, 403→dashboard) and fails closed when D1 binding is missing; `licensing/[...path].ts` enforces session + verified email + role lookup per request and maps infra failures to opaque 500s.
- **Stripe URL validation**: `UpgradeModal.startCheckout` pins redirects to `https://checkout.stripe.com`; `CustomerDetailDrawer.handleOpenBillingPortal` pins portal URLs to `billing.stripe.com`; `LicenseSuccessModal` validates `cs_…` session-id shape and scrubs it from the URL. Good.
- **Race conditions**: `UpgradeModal` checkout attempt counter + timer cancellation is correct; `DocsAnalytics` fetch guard is correct.
- **Legacy patterns**: no `class:` directives, no legacy stores; this is a SolidJS codebase (createSignal/createMemo/createEffect), not Svelte, so rune rules don't apply; Solid idioms used are mostly correct apart from M-8.
- **Resource leaks**: rAF/timeouts/listeners cleaned up via `onCleanup` in CohortRetentionHeatmap, EngagementMetrics, TimeToValueMetrics, CustomerDetailDrawer, Header, Tooltip. `downloadCSV` revokes object URLs properly.

## Suggested fix order
1. H-1/H-2/M-3/M-5 (data integrity on admin decision surfaces)
2. M-1/M-2 (search/pagination correctness)
3. M-9 (origin hardening on public offer proxy)
4. M-6/M-7/M-8, then LOW items.
