# Fedora/DNF Engine — Research Verdicts & Build Plan (2026-08 research)

Goal: beat dnf5 (C++) on every user-visible latency, using the same
mmap-index architecture that made our Debian/Arch paths fast.

## Research verdicts

1. RPM header format: NO maintained pure-Rust parser crate exists.
   -> Write raw Rust: we ALREADY hand-parse RPM headers in dnf.rs
   (parse_rpm_header). Extend with zerocopy (already a dependency!)
   U16/U32<BigEndian> borrowed views over the 96-byte lead + signature
   + header regions: zero-copy, bounds-checked, no new crate.
   (rpm crate exists w/ sig verification but is heavyweight; only its
   OpenPGP check matters, and sequoia is already in-tree for that.)
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

S1. Raw-Rust RPM lead/signature/header reader via zerocopy views,
    bounds-checked, hostile-input fuzzed; replaces ad-hoc blob parser.
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
+ RPM reader: zerocopy fixed-layout views + checked Rust for variable data;
  reject bytemuck/nom. Steal librpm validation invariants (lead magic check,
  tag types 1..9, STRING count-one). rpm crate = differential-test oracle
  only. No drop-in pure-Rust parser exists (proven across #2/#11/#12).
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
