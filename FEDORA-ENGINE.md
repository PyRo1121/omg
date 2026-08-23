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
