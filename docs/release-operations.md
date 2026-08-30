# Release Operations

How releases are built, published, verified, and rolled back.

## Pipeline overview

```
tag push (v*) or manual dispatch
        │
        ▼
gate-on-ci ──▶ build-arch / build-debian / build-ubuntu / build-fedora / build-macos
   (1)                  │  (digest-pinned distro containers, 1.93.1 toolchain)
                        ▼
                   release (SBOM → attest → checksum-verified collection → GitHub Release)
                        │
                        ▼
                   sync-r2  (environment: production)
                         1. upload archives + .sha256 sidecars to R2
                         2. round-trip verify every uploaded object
                         3. publish `latest-version` marker (LAST)
```

Hard guarantees enforced in `.github/workflows/release.yml`:

- **No publish from a red commit.** `gate-on-ci` requires a successful `CI`
  run for the exact release commit; in-progress runs are *watched to
  completion* rather than raced. The only bypass is a no-op dry run.
- **Reproducible dependency set.** Every build uses `--locked` against
  `Cargo.lock`; the SBOM job fails if generation mutates the lockfile.
- **Artifact allowlist.** `scripts/collect-release-artifacts.sh` refuses
  anything that is not exactly the five platform archives, their `.sha256`
  sidecars, and the CycloneDX SBOM.
- **Fail-closed publish.** If any uploaded R2 object cannot be re-downloaded
  byte-identical, the job aborts *before* the `latest-version` marker moves.
- **Pinned release tooling.** All actions are SHA-pinned (Renovate with a
  7-day dwell time), containers are digest-pinned, and `wrangler` is installed
  with `npm ci` from a committed lockfile (`.github/deps/release-tools`), so
  downloaded npm packages are checked against their locked integrity hashes.

## Client-side verification chain

Clients never trust the release bucket alone:

| Layer | Installer (`install.sh`) | `omg self-update` |
|---|---|---|
| TLS | HTTPS-only pinned hosts | HTTPS + redacted error URLs |
| Checksum | mandatory `.sha256` sidecar | pinned digest verified before extraction |
| Downgrade protection | version check in release metadata | refuses older versions without `--force` |
| Build provenance | `gh attestation verify` when `gh` is installed | `gh attestation verify`, fail-closed; skipped with warning only if `gh` missing |
| Size bounds | curl + disk constraints | 256 MiB streaming cap, 16 MiB prealloc cap |

The provenance layer exists precisely because a compromise of the R2
credentials could rewrite binaries **and** checksum sidecars together;
Sigstore attestations are anchored outside the bucket and cannot be forged
without the CI runner itself.

## Rollback

`latest-version` is the only intentionally mutable pointer; release operations
must treat version-addressed archives as immutable. To roll clients back to a
previously published version:

```bash
export CLOUDFLARE_API_TOKEN=... CLOUDFLARE_ACCOUNT_ID=...
./scripts/r2-rollback.sh 0.1.214           # verify + re-point marker
./scripts/r2-rollback.sh 0.1.214 --dry-run # preview only
```

The script refuses to move the marker to a version whose archives are not
fully present in R2 (all five platforms). Installed clients update on their
next `omg self-update` check; nothing is uninstalled or downgraded server-side.

**To withdraw (rather than roll back) a version with a bad or malicious
binary:** delete its objects from the R2 `omg-releases/` prefix and from
GitHub Releases, rotate credentials if compromise is suspected, then publish
a fixed version and cut a security advisory (see SECURITY.md).

## Credential rotation

Rotate **immediately and unconditionally** if any of the following trigger:

- `CLOUDFLARE_API_TOKEN` appears in a log, PR, or a gitleaks/trufflehog alert
- A release job fails round-trip verification without an obvious cause
- Any unexpected commit to `.github/workflows/` on `main`
- A maintainer account shows unrecognized sessions

Rotation procedure:

1. Create a fresh token scoped to **only** the `omg-releases` R2 bucket (object read/write, no account-level roles).
2. Update the `CLOUDFLARE_API_TOKEN` secret in the `production` environment (release.yml).
3. Delete the old token in the Cloudflare dashboard; verify the new token fails with the old value.
4. Review the Cloudflare audit log for the window since the last known-good release.

## Known platform gaps (documented, not hidden)

- **Windows native:** there is no `windows` cargo feature or backend. Windows
  users are supported through WSL only (see `WINDOWS_TESTING.md`). A native
  port requires a new package-manager feature and should add a
  `build-windows` CI leg at the same time the feature lands.
- **Linux aarch64:** no release artifact exists; `self_update` fails with an
  explicit error directing users to the manual path rather than
  installing a wrong-arch binary.

**Last Updated:** 2026-08-30