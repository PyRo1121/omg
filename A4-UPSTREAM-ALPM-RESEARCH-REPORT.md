# TASK A4-RETRY — Upstream alpm.rs ecosystem version-alignment research

Date: 2026 (research session). Scope: upstream web research only — no cargo runs, no builds.
Local ground truth (pre-verified by sibling agent, not re-verified here): omg `--features arch`
compiles dev+release, links `/usr/lib/libalpm.so.16` (→ `libalpm.so.16.0.1`, pacman 7.1.0.r9),
runs and reads pacman's local DB (1451 packages).

---

## (a) Compatibility verdict: alpm 5.0.2 ↔ libalpm.so.16 / pacman 7.1 — ✅ CORRECT SERIES

The alpm.rs checkver gate pins one supported libalpm major per crate major (`alpm/build.rs`):

| alpm crate | published | `supported_current` in build.rs | libalpm soname | pacman |
|---|---|---|---|---|
| 3.0.x | 2024-03 | (pre-soname-16 era) | 13–14 | 6.1 |
| 4.0.0–4.0.2 | 2024-08 → 2024-12 | **15** (verified in published 4.0.2 crate) | libalpm.so.15 | 7.0 |
| 4.0.3 / 4.0.4 | 2025-05 / 2025-12 | **15** (verified in published 4.0.4 crate; 4.0.4 is a same-week backport for users not yet on pacman 7.1) | libalpm.so.15 | 7.0 |
| **5.0.0** | 2025-12-05 | **16** | libalpm.so.16 | 7.1 |
| **5.0.2** (current, newest) | 2026-01-08 | **16** (verified in published 5.0.2 crate and in current master clone) | libalpm.so.16 | 7.1 |

Evidence:
- Issue #59 "Create tag for release supporting libalpm.so.16" — closed by maintainer Morganamilo
  on 2025-12-17: "**v5 with alpm v16.0.0 support was released 2 weeks ago**" (= alpm 5.0.0, 2025-12-05).
  https://github.com/archlinux/alpm.rs/issues/59
- Current master `alpm/build.rs`: `let supported_current = 16;`
  (master clone of github.com/archlinux/alpm.rs, commit b22d8c2, 2026-06-19).
- Local soname check: `/usr/lib/libalpm.so.16 → libalpm.so.16.0.1`.
- Since omg's build used alpm's *default* `checkver` feature and compiled+ran, the runtime
  `alpm_version()` returned 16.x — runtime self-attestation of compatibility.

**Verdict: alpm 5.0.2 is exactly the right series for libalpm.so.16 / pacman 7.1. It is also the
newest release on crates.io (nothing newer exists; GitHub has no releases and git tags stop at
alpm-v4.0.1 — the maintainer explicitly says crates.io versions are the source of truth, issue #59).**
Downgrading to 4.0.4 would be wrong (it asserts libalpm 15 and would fail checkver on this system).

## (b) Pin consistency at rev b5463fcd4b677e55792dfb0c708da9659db0368a — ✅ CONSISTENT

Verified via GitLab raw API at the exact rev (paths are top-level crates in the monorepo):

- `alpm-types/Cargo.toml` at rev → `version = "0.11.1"` ✅
- `alpm-srcinfo/Cargo.toml` at rev → `version = "0.6.2"` ✅
- `alpm-db/Cargo.toml` at rev → `version = "0.2.1"` ✅
- `alpm-repo-db/Cargo.toml` at rev → `version = "0.1.1"` ✅

All four upstream tags for these versions were cut the same day (2026-01-11:
alpm-types/0.11.1, alpm-srcinfo/0.6.2, alpm-db/0.2.1, alpm-repo-db/0.1.1), and the pinned rev
b5463fcd is the `chore(deps): Update Rust crate colored to v3.1.1` commit sitting exactly in that
release chain. Because the GitLab repo is a single cargo workspace, all four crates at one rev are
internally consistent by construction; the committed omg `Cargo.lock` resolves all four from that
exact rev with the same hash `b5463fc…` and cargo enforced the exact `=` requirements on build.
No inconsistency found.

## (c) Recommended newer pin — OPTIONAL bump; safe, no breaking changes for omg's use

Newer tagged releases exist in the monorepo (2026-03-15..17):

| crate | pinned | newest tag | changes (from crate CHANGELOG at alpm-srcinfo/0.6.3 tag) |
|---|---|---|---|
| alpm-types | 0.11.1 | **0.11.2** | `Fixed`: **Add missing `PackageOption::PEStrip`**; i18n file renames; `time` bump |
| alpm-db | 0.2.1 | **0.2.2** | error-variant canonicalization (enum shape refactor); i18n renames |
| alpm-srcinfo | 0.6.2 | **0.6.3** | i18n file renames only (no parser/validation changes) |
| alpm-repo-db | 0.1.1 | **0.1.2** | error-variant canonicalization; i18n renames |

Interim note: your pinned rev already *includes* alpm-srcinfo 0.6.1–0.6.2 parser fixes
(`PackageVersion` charset per spec, tab handling) and alpm-db 0.2.1's
"Ignore backup entries with null digest in alpm-db-files" fix — i.e. the pin is on a bug-free-for-
these-classes snapshot. The 0.11.x→0.2.x bumps since then fix one real API gap
(PackageOption::PEStrip) relevant only if omg sets pacman package options, plus a monorepo-wide
**RUSTSEC-2026-0007 (bytes crate)** transitive bump that happened between your rev and the
March batch (commit 5dc54871).

The March batch is the **newest tagged release set**. The monorepo `main` HEAD is now
`7e06d1ee3236` (2026-08-27: "fix: Incorrect json serialization of x86_64_vX variants" plus serde
`FromStr` deserialization fixes) — untagged; per your exact-rev supply-chain policy, prefer a tag
batch over HEAD.

**Recommendation: hold the current pin (zero observed breakage; it is verified-good).**
If/when bumping (e.g. to pick up PackageOption::PEStrip and the RUSTSEC bytes fix), move all five
alpm-* git entries together to the newest fully-tagged commit:

```toml
alpm-types = { version = "=0.11.2", git = "https://gitlab.archlinux.org/archlinux/alpm/alpm.git", package = "alpm-types", rev = "bd241fb0b8690f4f2a0c62a3f9cfbd6f0b2ebd1a", optional = true }
alpm-srcinfo = { version = "=0.6.3", git = "https://gitlab.archlinux.org/archlinux/alpm/alpm.git", package = "alpm-srcinfo", rev = "bd241fb0b8690f4f2a0c62a3f9cfbd6f0b2ebd1a", optional = true }
alpm-db = { version = "=0.2.2", git = "https://gitlab.archlinux.org/archlinux/alpm/alpm.git", package = "alpm-db", rev = "bd241fb0b8690f4f2a0c62a3f9cfbd6f0b2ebd1a", optional = true }
alpm-repo-db = { version = "=0.1.2", git = "https://gitlab.archlinux.org/archlinux/alpm/alpm.git", package = "alpm-repo-db", rev = "bd241fb0b8690f4f2a0c62a3f9cfbd6f0b2ebd1a", optional = true }
```

(`bd241fb0…` = the `alpm-srcinfo/0.6.3` tag commit, 2026-03-17, the chronologically last release
commit of the batch, so it contains alpm-types 0.11.2 + alpm-db 0.2.2 + alpm-repo-db 0.1.2 +
alpm-srcinfo 0.6.3 simultaneously. **Re-verify the full 40-char SHAs and run a lockfile diff +
build before committing** — this report used truncated SHAs from the API and I made no builds
per task constraints. The 0.2.2/0.1.2 "canonicalize error variants" change can shift error enum
shapes; check any exhaustive `match` on alpm-db/alpm-repo-db errors.)

## (d) Known upstream bug classes that could hit an otherwise-correct 5.x caller

GitHub issue triage (31 total issues on archlinux/alpm.rs; no open issues about soname mismatch,
handle-init failures, or errno misreporting — searched "errno" = 0 hits, handle/init issues = none
recent):

1. **Panic (not Err) on NUL bytes in DB strings** — issue #69 (2026-04, closed as user-data
   upstream; observed on libalpm v16.0.1 / pacman 7.1.0): `Called Result::unwrap()` on
   `NulError` at `alpm/src/db.rs:144` when DB strings contain embedded NULs (paru AUR-completion
   garbage triggered it). Class to know about: alpm.rs *panics* rather than returning `Err` when
   converting libalpm C strings containing NUL. If omg sees recurring panics while reading DBs,
   sanitize/route through a catch or validate strings first. https://github.com/archlinux/alpm.rs/issues/69
2. **`trans_add_pkg` returning `WrongArgs` for local-DB packages** — issue #53 (open, 2025-07):
   libalpm itself asserts `pkg->origin != ALPM_PKG_FROM_LOCALDB` in `alpm_add_pkg`, so
   `trans_add_pkg(pkg_from_localdb())` → `Error::WrongArgs`. Not an alpm.rs bug, but a misleading
   error for a seemingly-correct caller; for installs you must pass sync-DB (repo) packages or
   load a package file. https://github.com/archlinux/alpm.rs/issues/53
3. **`configure_alpm` / `set_hookdirs` clobbers the system hookdir** — issue #65 (open, 2026-02):
   if omg sets HookDir explicitly, the default system hookdir is replaced rather than appended.
   https://github.com/archlinux/alpm.rs/issues/65
4. **Historical UB/fixed ones (already closed, shipped fixed ≥ 4.0.x–5.0.x):**
   - #51 `unreachable_unchecked!()` UB (closed 2024-12) — fixed before 5.x.
   - #55 "[bug] wrong install reason" (closed 2025-12-05) — fixed before the 5.0.x you pin.
   - #48 libalpm v15 support (closed 2024-09) → alpm 4.x.
   - 5.0.0→5.0.2 itself contains exactly two changes (verified by diffing published crates
     5.0.0 vs 5.0.2): `list_mut.rs` replaced libc `strndup` with calloc+memcpy
     (allocation-safety/portability fix) and a `NonNull::new_unchecked` clippy fix in `mtree.rs`.
     **No API/behavioral breaking changes between 5.0.x patch versions.** (alpm 5.0.1 was never
     published for the `alpm` crate; only alpm-sys 5.0.1 exists on crates.io.)

No errno-misreporting class exists upstream (0 search hits).

---

## Bottom line

1. **alpm 5.0.2 ↔ libalpm.so.16: correct and current.** 5.x is the libalpm-16 series (maintainer
   statement + build.rs `supported_current = 16`); 5.0.2 is the newest crates.io release (2026-01-08).
2. **Pin b5463fcd is internally consistent** — all four crate Cargo.tomls at that rev declare
   exactly 0.11.1 / 0.6.2 / 0.2.1 / 0.1.1, matching omg's `=` requirements and Cargo.lock.
3. **No bump is required.** If you want the March-2026 release batch (PEStrip option +
   RUSTSEC-2026-0007 bytes fix), bump all four entries together to rev `bd241fb0…`
   (alpm-srcinfo/0.6.3 tag) with versions =0.11.2/​=0.6.3/​=0.2.2/​=0.1.2; verify full SHA and
   re-diff the lockfile first.
4. **Recurring-runtime-error suspects for a correct caller:** NUL-byte panics in DB string
   conversion (db.rs:144 unwrap), WrongArgs from adding local-DB packages to a transaction,
   and hookdir replacement when configuring HookDir. Nothing else in the 5.x era.

### Source URLs
- https://github.com/archlinux/alpm.rs (README, master tree; build.rs `supported_current = 16`)
- https://github.com/archlinux/alpm.rs/issues/59 (v5 = libalpm 16.0.0, maintainer statement)
- https://github.com/archlinux/alpm.rs/issues/69 (NUL panic on libalpm 16.0.1 / pacman 7.1)
- https://github.com/archlinux/alpm.rs/issues/53 (WrongArgs / trans_add_pkg)
- https://github.com/archlinux/alpm.rs/issues/65 (hookdirs), /issues/55, /issues/51, /issues/48
- https://crates.io/api/v1/crates/alpm (version timeline), https://crates.io/crates/alpm-sys
- Published crate sources (static.crates.io): alpm-4.0.2 / 4.0.4 (supported_current=15),
  alpm-5.0.0 vs 5.0.2 (=16, only list_mut.rs/mtree.rs diffs)
- https://gitlab.archlinux.org/archlinux/alpm/alpm — tags
  alpm-{types/0.11.1,srcinfo/0.6.2,db/0.2.1,repo-db/0.1.1} (2026-01-11) and
  alpm-{types/0.11.2,db/0.2.2,srcinfo/0.6.3,repo-db/0.1.2} (2026-03-15..17); raw Cargo.toml
  at rev b5463fcd; crate CHANGELOGs at tag alpm-srcinfo/0.6.3
- Local: /home/pyro1121/Documents/omg/Cargo.toml + Cargo.lock (locked alpm 5.0.2 checksum
  4c7b56a…, git deps hash b5463fcd…, exactly as pinned); /usr/lib/libalpm.so.16 → libalpm.so.16.0.1