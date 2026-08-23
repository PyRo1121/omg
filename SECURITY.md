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
   omg audit verify  # Check for tampering
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
- Email to <security@pyro1121.com> subscribers

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

## Contact

Security Team: **<olen@latham.cloud>**
General Support: **GitHub Issues**

## Acknowledgments

We thank security researchers who responsibly disclose vulnerabilities. Credits will be listed here upon disclosure.

---

**Last Updated:** 2026-02-01
