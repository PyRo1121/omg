# Fedora/DNF Engine — Research Verdicts & Build Plan (2026-08 research)

Correct package operations come first. A Rust index and performance comparisons
remain future work. Do not infer transaction speedups or cross-distribution
performance from query timings.

## Research verdicts

1. RPM database headers and RPM package archives are different boundaries.
   SQLite `Packages.blob` starts with two big-endian lengths, then index entries
   and payload. It does not include archive magic/reserved bytes or an RPM lead.
   The reader uses existing zerocopy views and borrowed tag data. A real Fedora
   fixture protects this distinction. `rpmdb` 0.1.1 reads installed databases;
   `rpm` 0.27.1 parses package archives and signatures. These crates exist, but
   neither was adopted for this bounded database correction.
   Sources: https://docs.rs/rpmdb/0.1.1/rpmdb/ and
   https://docs.rs/rpm/0.27.1/rpm/.
2. Repo metadata: rpmrepo_metadata 0.7.0 (streaming repomd/primary.xml)
   is the only real crate option. Alternative: quick-xml streaming over
   primary.xml.gz — we already ship flate2/zstd/lz4 decompressors.
   RHEL 10 drops sqlite metadata => XML primary is the future-proof
   target; do NOT invest in sqlite reading.
3. createrepo_rs exists for GENERATION — not needed (omg consumes).
4. Competitive bar is dnf5 (C++), not dnf4-python. dnf5 pays metadata
   load on every invocation; our win condition is the daemon-held mmap
   index: build once from verified metadata, O(1) name lookup, zero
   per-invocation parse. Same pattern as debian_db (rkyv mmap) and
   pacman_db.

## Installed size queries

`omg size` reads installed package identities and byte sizes through DNF5.
It retains every installed version and architecture, including concurrent kernel
builds. RPM signing-key records named `gpg-pubkey` are not installed packages.

`omg size --tree PACKAGE` lists the selected package and installed providers of
its direct requirements. Alternative requirements can have several installed
providers. This is not a minimal or recursive dependency closure. An ambiguous
package selector fails instead of choosing an arbitrary installed build. A
version/architecture-qualified selector can select a specific build.

These read-only queries use `--setopt=disable_excludes=*` so transaction filters
do not hide installed packages. Available-package queries and transactions retain
the configured exclusion and signature policies. Native output is bounded to
64 MiB and 60 seconds. Invalid sizes and arithmetic overflow are errors, not zero.

Fedora 44 QEMU verification covers native package parity, configured exclusions,
missing packages, concurrent locally built RPM versions, ambiguous selectors,
and unchanged RPM inventories during queries. This does not establish exhaustive
Fedora CLI coverage or repair an already published artifact.

DNF5 documents [provider queries](https://dnf5.readthedocs.io/en/latest/commands/repoquery.8.html)
and [exclusion configuration](https://dnf5.readthedocs.io/en/latest/dnf5.conf.5.html).

## Recorded installation reasons

`omg why PACKAGE` displays DNF5's recorded reason for a specific installed build.
It preserves labels such as `Group`, `User`, `Weak Dependency`, and `External User`.
It does not infer an account identity or reconstruct the original transaction.

`omg why --reverse PACKAGE` lists other installed packages whose direct
requirements match that package through DNF5's native provider resolution. Each
entry retains its full identity and native installation reason. Current
requirements are not proof of historical cause or removal safety.

Both modes reject missing or ambiguous package selectors. These installed-fact
queries ignore exclusion filters without changing transaction policy. Reason-only
mode does not load reverse requirements. Terminal commands are constructed after
native asynchronous queries have finished.

## Package history diagnostics

`omg blame PACKAGE` combines native installed identity, EVR, installation reason,
and current direct requiring packages with the selected user's OMG history.
Qualified selectors use the canonical native package name for history lookup.
History is name-scoped, not proof about a particular installed architecture or
build. Failed records are displayed as failed actions.

The diagnostic uses a locked, read-only history snapshot. Missing history is
reported as missing. Malformed history fails without quarantine or rewriting.
Existing package-operation recovery remains separate. Earlier native DNF history
is not backfilled, and absent OMG records do not establish an installation date
or actor.

An earlier lifecycle exposed missing installation records and a SQLite warning.
The SQLite warning was isolated to the translated `gnat-srpm-macros` header. The reader now accepts I18NSTRING
arrays and validates every declared string terminator within the payload.
The captured native header passes through the SQLite reader without fallback;
installed summaries retain the default-locale string.

## Native transaction recording

CLI install, remove, update, and orphan cleanup attach a unique DNF5 transaction comment.
Bounded journal reads select that comment rather than assuming the last native
transaction belongs to OMG. Successful records retain canonical names and exact
RPM EVRs. Replacement versions pair only within the same name and architecture.
Failed native transactions do not turn planned versions into committed changes.

The caller supplies the history destination. PackageService uses its configured
manager, honors disabled history, and does not add a second predicted record.
The validated privileged-parent ownership flag also suppresses child recording.
Unrecorded backend methods remain available to callers that own their history.
A successful no-op adds no transaction. A completed operation with a persistence
failure returns an error that says the package operation succeeded.

Fedora 44 checks cover unprivileged install, no-op, remove, real blame history,
custom and disabled destinations, parent ownership, an unrelated later native
transaction, a native command failure, and a persistence failure. The lifecycle
restores the RPM inventory. Cache cleanup and sync do not record package-version
changes. Partial RPM failure recovery and exhaustive command coverage remain
unverified.

Fedora cleanup uses native DNF elevation rather than re-executing OMG through
sudo, so the caller retains its history destination. Native orphan confirmation
remains in place. A declined command records failure without committed package
changes. Preview, empty cleanup, and cache cleanup add no package history.
The ignored orphan fixture verifies these cases and removes only its own tree
package; it rejects pre-existing orphans.

The ignored `dnf_operations::test_update_all_packages` test upgrades a disposable
VM through the real CLI. It compares every recorded old and new version with
independent RPM inventory differences, then checks a no-op update. The Fedora 44
candidate matched 196 removed builds and 204 added builds. Reboot checks passed,
and an offline disk snapshot restored that guest's original package inventory.
The test requires an external VM snapshot and rollback; it is not a host test.

## Future index build order

S1. Correct the raw-Rust database-header reader using zerocopy views and a
    native SQLite fixture. Compare installed records with `rpm -qa`; preserve
    malformed-input rejection. Package archive/signature parsing is separate.
    Focused tests and native parity are required; fuzz coverage remains future work.
S2. Preserve Fedora's configured repository trust policy. The pinned Fedora
    image has repo_gpgcheck=0 and gpgcheck=1: metadata signatures and package
    signatures are separate checks. Resolve metalinks and bind primary metadata
    to the selected repomd checksums. Do not require a detached metadata signature
    that the configured repository does not publish, or silently bypass package
    signature verification. Test corrupt metadata and mirror changes.
S3. Stream verified primary metadata into compact records and a versioned,
    atomically published index under ~/.cache/omg/dnf. Real DNF5 cache inspection
    found primary.xml.zck for Fedora and updates, and primary.xml.zst for openh264.
    Select the supported primary representation from repomd explicitly; do not
    assume gzip or mistake a .solv cache for XML. Include enabled repository set,
    architecture, metadata digests, and policy in cache identity.
S4. Move search/info/list_updates/get_status from native DNF queries to the
    proposed index only after equivalent behavior is verified.
S5. Transactions stay on the dnf CLI; benchmark
    S3/S4 vs `dnf5 info/search` cold+warm in Dockerfile.fedora compose.

## R&D synthesis (wave: /tmp/omg-fleet13, 15 citation-backed reports)
+ RPM reader: zerocopy fixed-layout views and checked Rust for variable data.
  Apply validation to the representation actually read. Do not impose archive
  lead/magic checks on database blobs. Compare against native librpm output and
  real fixtures; synthetic headers alone previously concealed the format mismatch.
+ Index envelope: content-addressed generations named
  `(verified repomd digest, schema version, arch, repo set)` with magic/
  writer-version/arch/repomd-digest fields (#6/#8) — Nix-style identity,
  libdnf-style separation of authoritative bytes vs derived index.
+ dnf5: steal session->Goal->Transaction split as Rust traits; reject
  hydrate-per-query (#14).
+ Generators: steal projection-specific parsing, bounded pipelines (#13).
+ Benchmark protocol: identical RPMDB snapshots, pinned network profile,
  separate cold-mmap/warm/solve/download/txn numbers, publish bytes+counts.
+ UX: adoption driver = reduced decision anxiety; lead with grouped previews,
  dependency reasons, honest undo (#5).

## SpacetimeDB evaluation (backend option)

What it is: BSL-1.1 source-available relational DB + Rust reducer modules;
Maincloud managed free tier (~2,500 TeV/mo ~= 3M calls, 12.5GB egress,
1GB storage, pauses when idle); self-host via `spacetime start`/Docker.

Fit for omg:
+ Package-manager core: NOT an option — local mmap indexes win on latency
  and offline use; a network DB cannot serve `omg search` offline.
+ Remote backend (license JWT issuance, telemetry aggregation, team sync,
  future AI features): STRONG fit — Rust reducers replace our Cloudflare
  Worker + D1/KV spread with one typed deploy; free Maincloud tier covers
  current telemetry volume; scale-to-zero suits bursty CLI traffic.
Caveats: BSL license (fine as consumer), in-memory capacity bounds, free
tier idle-pause adds cold latency to first license check.
Decision: prototype the telemetry/license backend as a SpacetimeDB module
on Maincloud free tier behind the existing endpoint contract; keep
Cloudflare Workers as the fallback. Local engine stays mmap.
