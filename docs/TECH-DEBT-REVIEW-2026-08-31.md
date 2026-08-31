# Extreme Technical Review — 2026-08-31

90 AI reviewers (18 waves × 5 agents, GLM via ollama-cloud, max thinking),
each with a distinct scope + lens, cross-checked against live upstream
research of the archlinux/alpm alpha-crate ecosystem. Full per-agent reports:
`~/omg-waves/wNN_M.md` (90 files, 1.8 MB; each finding carries file:line,
verbatim quote, failure scenario, fix, confidence).

**Totals: 2 P0 · 50 P1 · 121 P2 · 141 P3** (314 ranked findings).

---

## P0 — fix immediately

| # | Location | Finding |
|---|----------|---------|
| 1 | `src/package_managers/apt.rs:500–536` | **Root-path install of a local `.deb` is a silent no-op that reports success** (archive copied into the apt cache but never installed via the FFI transaction; `mark_install` results discarded). Users on the root/FFI path believe a package installed when it did not. Independently re-found by wave 9. |
| 2 | `src/package_managers/apt.rs:344, 593–608` | **`debian` feature no longer compiles**: `String` assigned to `Version`-typed fields (regression from a recent refactor; anything building `--features debian` — i.e. the whole Debian matrix — is red). |

## P1 highlights — by subsystem (50 total; the ones that break real user flows)

**Arch / alpm core**
- `alpm_ops.rs:555` — `omg remove --recursive` sets `RECURSE|UNNEEDED`; pacman's `-Rs` sets RECURSE only. With `UNNEEDED`, libalpm *silently drops still-needed targets* from the transaction and reports success: `omg remove --recursive bash nginx` may remove neither while claiming success. (Unit test enshrines the wrong flags.)
- `alpm_ops.rs:784+` — pacman.conf `SigLevel`/`LocalFileSigLevel`/`RemoteFileSigLevel` (global and per-repo) are parsed and then **silently ignored**; hardcoded siglevels override user security policy both ways (breaks air-gapped unsigned mirrors, silently weakens tightened repos).
- `alpm_ops.rs:489` — all libalpm warnings suppressed on remove/autoremove: `.pacnew`/`.pacsave` notices never surface.
- `alpm_ops.rs:595` — HoldPkg checked before libalpm expands the remove set: cascaded/recursive removal can delete a held package.
- `alpm_ops.rs:867` — `configure_mirrors` can return `Ok(())` with zero servers configured; mirror failures swallowed at `tracing::debug`.
- `pacman_db/db.rs` — one corrupt local desc hard-fails the entire local DB (sync path deliberately degrades per-entry — inconsistent policy, plus missing `catch_unwind` on the local parse); non-UTF-8 desc in a sync `.db` is fatal to the whole repository.
- `alpm_direct.rs`/`alpm_worker.rs` — worker-init failure swallowed so the handle looks healthy forever; partial syncdb registration yields "successfully wrong" results.

**AUR (the flagship differentiator)**
- `aur/client.rs` + `aur_deps.rs` — **transitive AUR dependency resolution is depth-1**: deps-of-deps never resolved (confirmed independently by waves 4, 6 and 18).
- `makedepends`/`checkdepends` **version constraints silently dropped** (`aur_deps.rs:44`, wave 18 pins `aur_deps.rs:44-52`).
- No per-pkgbase lock: concurrent `build_only` of a shared dep races one `pkg_dir`.
- Index reads serve arbitrarily stale data (no TTL, never triggers sync).
- Epoch silently dropped by the `.SRCINFO` history matcher → AUR rollback broken for every epoch'd package.

**Debian db engine**
- `transaction.rs` mutates dpkg/apt state with **no dpkg lock acquisition** (concurrent-writer clobber, read-modify-write TOCTOU on `/var/lib/dpkg/status`); no on-disk journal, so SIGKILL mid-install strands files with no recovery; power-loss ordering bug between fsync'd status and non-fsync'd `/var/lib/dpkg/info`.
- Three **different candidate-selection algorithms** in `resolver.rs:101` / `db.rs:340` / `debian_pure.rs:481` — same inputs can pick different packages depending on code path.
- `update()` uses different upgrade semantics on the two privilege paths.
- Blocking FFI cache walks run directly on tokio executor threads.

**Security & infra**
- `tui/ui.rs:38-45` — control characters pass `truncate_width` and are written raw to the terminal (untrusted data → terminal escapes).
- `license.rs` — offline expiry fully defeated by clock rollback (no monotonic high-water mark).
- `telemetry`/`http` redaction and SSRF hardening gaps (details in `w13_*`).

## The alpm alpha-stack verdict (per the live research digest)

Upstream (gitlab archlinux/alpm) is at `alpm-types 0.11.2`, `alpm-srcinfo 0.6.3`,
`alpm-db 0.2.2`, `alpm-repo-db 0.1.2` — omg pins `=0.11.1`/`=0.6.2`/`=0.2.1`/`=0.1.1`
at rev `b5463fc` (crates.io `alpm 5.0.2` is current). Wave 18 findings:

1. **Bump to the .2 releases is low-risk and recommended**: 0.2.2/0.1.2
   "canonicalize error variants" (check exhaustive matches on their error enums),
   0.11.2 adds missing `PackageOption::PEStrip`; 0.2.1 upstream fixed
   "backup entries with null digest" — a class omg risks re-implementing wrong.
2. **Two divergent version-comparison stacks** (`alpm_ops.rs:141`,
   `alpm_direct.rs:309`, `types.rs:256-268`) vs `alpm-types`
   `VersionRequirement::is_satisfied_by` (whose pkgrel semantics changed upstream
   in 0.11.0). Consolidate on alpm-types; delete hand-rolled vercmp.
3. `aur_deps.rs:44` drops makedepends version constraints — use
   `OptionalDependency::package_relation` / `VersionRequirement` from
   `alpm-types 0.11.x` instead of hand-parsing.
4. `pacman_db/db.rs` hand-parses desc files the pinned `alpm-db`/`alpm-repo-db`
   crates already parse — divergence is already biting (the upstream null-digest
   fix exists because of exactly this).
5. Pinning by `rev` + `=` blocks Renovate vuln alerts for these crates — add an
   explicit quarterly review task or switch to versioned tags once upstream
   reaches stable 1.0 per crate.

## Cross-cutting tech-debt themes (waves 13-18)

1. **Blocking calls inside async / no `spawn_blocking`** — repeated in apt.rs FFI
   walks, debian transaction maintainer-script runs, archive ops.
2. **Swallowed results** as a house pattern (`let _ =`, ignored `mark_*`
   returns, `tracing::debug` for user-relevant failures).
3. **Missing fsync/journal discipline** in every home-grown db/cache writer
   (debian_db cache, daemon index, aur metadata) vs crash-safety claims.
4. **Three parallel implementations of the same concept** everywhere: candidate
   selection (debian ×3), version comparison (arch ×2), orphan detection
   (pure-Rust cache reads obsolete `%REQUIREDBY%` sections modern pacman never
   writes — massively overcounted orphans).
5. **Test debt**: tests assert implementation choices (e.g. the wrong
   `RECURSE|UNNEEDED` flags) rather than pacman-parity behavior; several
   `coverage_*.rs` files are execution-without-assertion.

## Recommended attack order (ship-blocking first)

1. P0 #1 + #2 (apt.rs): fix root-path `.deb` install + restore `debian` feature compile — then re-enable the Debian CI matrix which currently cannot be green.
2. AUR recursive deps (depth-1) + makedepends constraints + pkgbase lock.
3. Arch remove-flag semantics (`UNNEEDED`) + libalpm-warning surfacing + HoldPkg-after-expansion check.
4. pacman.conf SigLevel honoring (or hard-fail at config parse when customized).
5. dpkg lock acquisition + transaction journal in debian_db.
6. Terminal control-char sanitization in tui.
7. alpm alpha bumps (`0.11.2` / `0.6.3` / `0.2.2` / `0.1.2`) + consolidating on `alpm-types` for all relation math.
8. Zero-server mirror guard, stale-index TTLs, license clock pinning.

---
*Method: 90 read-only review agents (18 waves × 5 lenses × GLM/GLM-flash,
ollama cloud, max thinking), orchestrated in Herdr with one tab slot reused
per wave; findings verified by each agent against callers/callees and, for
alpm semantics, against vendored libalpm 5 sources and `alpm-sys-ll` docs.
Raw reports: `~/omg-waves/`. Upstream research: `.tmp/alpm-upstream-research.md`
(in-repo copy at time of run).*

**Last updated:** 2026-08-31