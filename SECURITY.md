# Security Policy

## Supported Versions

We provide security updates for the following versions of OMG:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in OMG, please report it privately.

### How to Report

**Do NOT open a public issue for security vulnerabilities.**

Instead, please email: **<olen@latham.cloud>**

Include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if you have one)

### What to Expect

- **Initial Response:** Within 48 hours
- **Status Update:** Within 7 days
- **Fix Timeline:** Depends on severity
  - Critical: 1-3 days
  - High: 1-2 weeks
  - Medium: 2-4 weeks
  - Low: Next release cycle

### Disclosure Policy

- We follow responsible disclosure
- We will credit you in the security advisory (unless you prefer to remain anonymous)
- We will notify you when the fix is released
- Public disclosure happens after the fix is available

## Security Features

OMG includes built-in security features:

### Package Security

- **PGP Verification:** Automatic package signature verification
- **Vulnerability Scanning:** CVE detection for installed packages
- **SBOM Generation:** Software Bill of Materials in CycloneDX format
- **Security Grading:** Risk assessment for every package install
- **Audit Logging:** Tamper-proof logs of all package operations

### System Security

- **Privilege Separation:** Minimal sudo usage with sudoloop
- **Sandbox Support:** AUR builds can use bubblewrap/chroot
- **Secret Scanning:** Detects leaked credentials before commit
- **Policy Enforcement:** Configurable security policies via `policy.toml`

### Supply Chain Security

- **Dependency Pinning:** Lockfiles for reproducible builds
- **SLSA Provenance:** Build attestations (where available)
- **Signature Verification:** PGP signatures on official packages
- **Mirror Verification:** Checksum validation on downloads

## Known Security Considerations

### Third-Party Dependencies

OMG relies on several third-party crates. We regularly audit dependencies for security issues using `cargo audit`.

**Current Known Issues:**

No known dependency vulnerabilities are accepted. Release gates run `cargo audit`; yanked transitive packages are tracked separately from security advisories.

### Key Trust on First Use

AUR package keys listed in `validpgpkeys` are fetched over HKPS and imported into the user's GnuPG home on first sight, with no fingerprint confirmation prompt. The import prints the key fingerprint to stderr so it is never silent, and the GnuPG home is created `0700` (pre-existing homes are re-validated for ownership and mode before import). Silent TOFU is the accepted model here, matching what `makepkg` itself does with an unfamiliar key.

Native Windows is not supported. Windows users should run OMG inside WSL, where the installed Linux distribution determines the package backend.

### Privilege Escalation

OMG requires sudo access for:

- Installing/removing system packages
- Modifying system files
- Running AUR builds (when not in user-space)

**Mitigation:**

- Sudoloop limits password prompts
- Dry-run mode (`--dry-run`) shows what would happen
- Policy enforcement prevents unauthorized operations
- Audit logs track all privileged operations

### AUR Package Security

AUR packages are community-maintained and not officially verified.

**Built-in Protections:**

- Security grading (COMMUNITY level)
- Optional PKGBUILD review before build
- Sandboxed builds (bubblewrap/chroot)
- PGP verification where available

**Best Practices:**

- Review PKGBUILDs before installation
- Use `--dry-run` to preview changes
- Enable `review_pkgbuild = true` under `[aur]` in `~/.config/omg/config.toml`
- Check package popularity and votes

## Security Best Practices

### For Users

1. **Keep OMG Updated:**

   ```bash
   omg self-update
   ```

2. **Enable Security Features:**

   ```toml
   # ~/.config/omg/policy.toml
   minimum_grade = "Verified"  # Require PGP signatures
   require_pgp = true
   allow_aur = false  # Disable AUR if not needed
   ```

3. **Review Audit Logs:**

   ```bash
   omg audit log
   omg audit verify  # Check local hash-chain consistency
   ```

4. **Scan for Vulnerabilities:**

   ```bash
   omg audit scan
   omg audit fix  # Auto-upgrade vulnerable packages
   ```

### For Developers

1. **Run Security Audits:**

   ```bash
   cargo audit
   cargo clippy -- -D warnings
   ```

2. **Review Dependencies:**

   ```bash
   cargo tree
   cargo machete  # Find unused dependencies
   ```

3. **Test Security Features:**

   ```bash
   cargo test --features arch security
   ```

4. **Follow Secure Coding Practices:**
   - Avoid `unsafe` blocks unless absolutely necessary
   - Use `#[must_use]` on query functions
   - Add context to all errors
   - Validate all user input

## Security Updates

Security updates are announced via:

- GitHub Security Advisories
- Release notes (CHANGELOG.md)
- Email to <olen@latham.cloud> subscribers

## Compliance

OMG supports compliance requirements for:

- SOC2
- ISO27001
- FedRAMP (future)

Features:

- Audit log export (`omg enterprise audit-export`)
- SBOM generation (`omg audit sbom`)
- Vulnerability reporting (`omg --json audit scan`)
- Policy enforcement (`policy.toml`)

## Release Incident Response

If a published release is (or may be) compromised, follow this runbook.

### Signals

- `sync-r2` round-trip verification failure (byte-identical re-download check)
- A gitleaks / TruffleHog alert on any commit
- Client reports of checksum or attestation verification failures
- Unexpected commits to `.github/workflows/` on `main`

### Immediate containment (first hour)

1. **Stop exposure:** remove the affected archives, `.sha256` sidecars, and the SBOM from the GitHub Release, and delete the corresponding `omg-releases/` objects in R2. If a good earlier version exists, run `./scripts/r2-rollback.sh <previous-version>` so update checks resolve to it.
2. **Revoke tokens:** delete `CLOUDFLARE_API_TOKEN` immediately — do not wait for analysis. Rotate per the procedure in [docs/release-operations.md](docs/release-operations.md).
3. **Freeze releases:** no new tags until containment is confirmed.

### Assessment

4. Determine blast radius: compare attestation provenance (`gh attestation verify … -R PyRo1121/omg`) and checksums for every published archive against the CI-generated artifacts (upload artifacts in the release run, plus `attest-build-provenance` records).
5. If CI itself is suspect, review workflow diffs, runner logs, and the vendored npm lockfile integrity (`.github/deps/release-tools/package-lock.json` has registered integrity hashes for every transitive dependency).

### Recovery

6. Fix the root cause on `main` and let CI go green.
7. Re-tag and let the full pipeline rebuild, attest, verify, and publish.
8. Publish a security advisory (and a changelog entry) describing the incident, affected versions, and remediation.
9. File a post-mortem; add regression tests where the failure slipped through.

Full release procedures and credential rotation: see [docs/release-operations.md](docs/release-operations.md).

## Contact

Security Team: **<olen@latham.cloud>**
General Support: **GitHub Issues**

## Acknowledgments

We thank security researchers who responsibly disclose vulnerabilities. Credits will be listed here upon disclosure.

---

**Last Updated:** 2026-02-01

## Security boundaries and retained trust

Release installation requires GitHub CLI (`gh`) verification of the archive's
attestation against the requested tag and `.github/workflows/release.yml` in
`PyRo1121/omg`. A missing verifier or rejected attestation stops the installer.
Source builds require explicitly running a checked-out `install.sh --from-source`;
piped execution never builds the current directory. The initial bootstrap script
is executable code: fetching it from mutable `main` trusts the repository owner
and delivery channel before any archive verification runs. For reproducibility,
review a checkout at a commit you trust and run that copy of the script. Archive
attestation does not retroactively authenticate the bootstrap script.

Runtime downloads trust their documented upstream publishers. A digest delivered
by that publisher detects corruption but cannot protect against compromise of the
publisher and its metadata together. OMG does not claim independent provenance
for every runtime. SLSA artifact verification requires an explicit certificate
identity; a valid signature from an arbitrary signer is insufficient.

AUR review covers the complete local regular-file source manifest, including
`.SRCINFO`, patches and install hooks. Source symlinks are rejected; replace them
with reviewed regular files. All cached, fresh, dependency and rollback archives
must match the reviewed output identity and exact install-hook bytes. Package
archives are snapshotted into sealed Linux files before approval and copied into
private root staging for the complete privileged transaction.

Bubblewrap builds are offline by default. The host prefetcher accepts only public
HTTPS destinations, checks every redirect and pins resolved addresses. Sources
that require build-time networking (including unsupported VCS prefetches) require
an explicit `[aur] allow_network = true` setting. That setting exposes local and
private services to the build; it is a trust decision. Chroot devtools cannot
promise this offline boundary and require the same explicit setting. Native
builds remain an explicit unsafe option and execute with `no_new_privs` so they
cannot gain root through a cached sudo ticket.

Explicit package policies are enforced again against ALPM's prepared install or
upgrade plan, including dependencies. Native APT/DNF/Homebrew install and upgrade paths
refuse explicit policies because OMG cannot guarantee their final plan matches a
separate precheck. Use their default policy or an enforceable ALPM transaction.
Pure-Debian production mutations are disabled until independently authenticated
repository authority is implemented; the native APT backend remains available.

Local audit verification establishes internal hash-chain consistency only. The
owner can rewrite and rehash a user-owned collection, remove entries, or delete
its incompleteness marker; successful verification is not proof of authenticity
or completeness. Operations executed inside privileged OMG backends additionally persist attempt
and outcome records synchronously under root-controlled `/var/log/omg`. Direct
external native launches record their attempt/outcome in the invoking user’s
collection. An
abruptly terminated operation may have an attempt without a completion record.
Daemon queue overflow or persistence failure leaves a durable `audit/incomplete`
marker and verification refuses to describe that collection as complete. Root
can still alter system logs. Independently retained or remotely collected logs
are required for evidence against the machine's administrator.

Dashboard account linking accepts `omg account link --token-stdin` or
`OMG_DASHBOARD_TOKEN`, avoiding a token in process arguments. Prefer stdin from
a secret manager; environment variables remain visible to sufficiently privileged
processes and should not be entered literally in shell history.
