# Fedora/DNF Engine — Research Verdicts & Build Plan (2026-08 research)

Goal: reduce repeated metadata-loading work with a Rust index, then measure
search and information lookup against equivalent DNF5 operations. Do not infer
transaction speedups or cross-distribution performance from query timings.

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

## Build order (each slice gated + compose-verified)

S1. Correct the raw-Rust database-header reader using zerocopy views and a
    native SQLite fixture. Compare installed records with `rpm -qa`; preserve
    malformed-input rejection. Package archive/signature parsing is separate.
    Focused tests and native parity are required; fuzz coverage remains future work.
S2. InRelease-equivalent: verify repomd.xml signatures (sequoia, already
    in-tree) BEFORE any metadata use; bind primary.xml checksums to
    verified repomd (mirrors the Debian signature-chain blockers).
S3. Streaming primary.xml.gz -> internal CompactPackage records ->
    rkyv mmap index under ~/.cache/omg/dnf, format-versioned, atomic
    publish, daemon prewarm. This replaces the deleted dead scaffolding
    with the REAL implementation (wave: dnf repo-metadata deletion).
S4. search/info/list_updates/get_status over the mmap index (O(1)/scan);
    list_updates un-blocks (currently explicit fail-closed).
S5. Transactions stay on the dnf CLI (already correct); benchmark
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
