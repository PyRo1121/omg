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

- **No publish from a red commit.** `gate-on-ci` requires successful `CI` and
  `Benchmark` runs for the exact release commit; in-progress runs are *watched
  to completion* rather than raced. The only bypass is a no-op dry run.
- **Reproducible dependency set.** Every build uses `--locked` against
  `Cargo.lock`; the SBOM job fails if generation mutates the lockfile.
- **Artifact allowlist.** `scripts/collect-release-artifacts.sh` refuses
  anything that is not exactly the five platform archives, their `.sha256`
  sidecars, and the CycloneDX SBOM.
- **Fail-closed publish.** If any uploaded R2 object cannot be re-downloaded
  byte-identical, the job aborts *before* the `latest-version` marker moves.
- **Cache policy matches mutability.** Version-addressed archives and `.sha256`
  sidecars are uploaded with `Cache-Control: public, max-age=31536000,
  immutable`; the only mutable pointer, the `latest-version` marker, is
  published with `Cache-Control: no-store` by both `release.yml` and
  `scripts/r2-rollback.sh`, so subsequent version checks fetch the current marker.
- **Remote R2 only.** Wrangler 4's `r2 object` commands default to local
  Miniflare storage. `release.yml` and `scripts/r2-rollback.sh` pass `--remote`
  so publishes hit the production `omg-releases` bucket.
- **Pinned release tooling.** All actions are SHA-pinned (Renovate with a
  7-day dwell time), containers are digest-pinned, and `wrangler` is installed
  with `npm ci` from a committed lockfile (`.github/deps/release-tools`), so
  downloaded npm packages are checked against their locked integrity hashes.

## Recovering an R2 sync

If GitHub publication succeeds but `sync-r2` does not, dispatch the `Release`
workflow from `main` with `sync_existing_tag` set to the published tag and
`dry_run` set to `false`. The default dry run does not sync or publish. This
path does not rebuild or edit the release. It downloads exactly five archives
and five checksum sidecars, verifies every checksum and GitHub attestation
against the tag's commit, then runs the normal production R2 upload,
round-trip verification, and latest-marker sequence.

## Client-side verification chain

Clients never trust the release bucket alone:

| Layer | Installer (`install.sh`) | `omg self-update` |
|---|---|---|
| TLS | HTTPS-only pinned hosts (`releases.omg.latham.cloud`) | HTTPS + redacted error URLs |
| Latest resolution | R2 `latest-version` marker only: bounded bare-SemVer text, fail-closed; exact `OMG_VERSION` installs use the given tag verbatim | R2 `latest-version` marker only (R2-only, fail-closed) |
| Checksum | mandatory `.sha256` sidecar | pinned digest verified before extraction |
| Downgrade protection | installs only the resolved latest or the exact requested version | refuses older versions without `--force` |
| Build provenance | `gh attestation verify`; missing `gh` causes refusal unless `OMG_INSTALL_ALLOW_UNVERIFIED_PROVENANCE=1` explicitly opts out | `gh attestation verify`; missing `gh` causes refusal unless `OMG_SELF_UPDATE_ALLOW_UNVERIFIED_PROVENANCE=1` explicitly opts out |
| Size bounds | curl + disk constraints | 256 MiB streaming cap, 16 MiB prealloc cap |

The provenance layer exists precisely because a compromise of the R2
credentials could rewrite binaries **and** checksum sidecars together;
Sigstore attestations are anchored outside the bucket and cannot be forged
without the CI runner itself.

Both clients download archives and sidecars from the R2 release domain
(`https://releases.omg.latham.cloud`). GitHub Releases remains the documented
mirror of the same immutable objects, and the installer pins each archive to
the version it already resolved. The installer resolves "latest" from the R2
marker only. Repointing the marker changes version selection, not the client's
downgrade policy.

## Rollback

`latest-version` is the only intentionally mutable pointer; release operations
must treat version-addressed archives as immutable. To select a previously
published version for new downloads:

```bash
export CLOUDFLARE_API_TOKEN=... CLOUDFLARE_ACCOUNT_ID=...
./scripts/r2-rollback.sh 0.1.214           # verify + re-point marker
./scripts/r2-rollback.sh 0.1.214 --dry-run # preview only
```

The script refuses to move the marker to a version whose archives are not
fully present in R2 (all five platforms). Fresh installations use the selected
version. Installed clients resolve the same marker, but a normal `omg self-update`
refuses a version older than the installed binary. Users must run
`omg self-update --force` to accept that downgrade. Checksum and provenance
verification still apply. The script does not modify installed binaries.

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
  users are supported through WSL only. A native
  port requires a new package-manager feature and should add a
  `build-windows` CI leg at the same time the feature lands.
- **Linux aarch64:** no release artifact exists; `self_update` fails with an
  explicit error directing users to the manual path rather than
  installing a wrong-arch binary.

**Last Updated:** 2026-09-04