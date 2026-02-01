# Security Policy

## 🛡️ Security Commitment

OMG is designed with **enterprise-grade security** as a core principle. We implement SLSA provenance, PGP verification, SBOM generation, and tamper-proof audit logging to ensure supply chain security and package integrity.

This document outlines our security practices, known vulnerabilities, and reporting procedures.

---

## 📋 Supported Versions

We provide security updates for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | ✅ Active development |
| < 0.1.0 | ❌ Not supported    |

---

## 🚨 Reporting a Vulnerability

**DO NOT** open a public GitHub issue for security vulnerabilities.

### Reporting Procedure

1. **Email:** Send vulnerability details to **olen@latham.cloud**
2. **Subject:** `[SECURITY] Brief description`
3. **Include:**
   - Vulnerability description
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if available)

### Response Timeline

- **Initial Response:** Within 48 hours
- **Status Update:** Within 7 days
- **Fix Timeline:** Varies by severity
  - **Critical:** 24-48 hours
  - **High:** 7 days
  - **Medium:** 14 days
  - **Low:** Next release cycle

---

## 🔍 Known Security Considerations

### Platform-Specific Dependency Issues

OMG uses optional dependencies for platform-specific package manager integrations. Some third-party dependencies have known advisories that affect specific platforms only.

#### **1. Windows Builds (RUSTSEC-2023-0018) - Medium Risk**

**Status:** ⚠️ **Tracked, Low Impact**

- **Affected Component:** `libscoop` → `remove_dir_all 0.7.0`
- **Vulnerability:** Race condition enabling TOCTOU (Time-of-check Time-of-use)
- **RUSTSEC ID:** [RUSTSEC-2023-0018](https://rustsec.org/advisories/RUSTSEC-2023-0018)
- **Platforms Affected:** ❌ Windows only
- **Platforms NOT Affected:** ✅ Linux, macOS, BSD

**Context:**
- `libscoop` is only compiled on Windows builds (`target_os = "windows"`)
- Linux and macOS builds do NOT include this dependency
- The vulnerability requires specific race condition timing that is difficult to exploit in OMG's use case

**Mitigation:**
- Tracked upstream: Waiting for `libscoop` to update to `remove_dir_all >= 0.8.0`
- Not exploitable on Linux/macOS (90%+ of OMG users)
- Windows users: Use WSL for enhanced security posture

**Fix Timeline:** Blocked on upstream `libscoop` release

---

#### **2. Debian/Ubuntu Builds (Unmaintained Dependencies) - Low Risk**

**Status:** ⚠️ **Tracked, Low Impact**

**Affected when building with `--features debian`:**
- `async-std 1.13.2` - Discontinued (RUSTSEC-2025-0052)
- `ring 0.16.20` - Unmaintained, need 0.17+ (RUSTSEC-2025-0010)
- `rusoto_*` - Rusoto ecosystem unmaintained (RUSTSEC-2022-0071)
- `rustls-pemfile 1.0.4` - Unmaintained (RUSTSEC-2025-0134)

**Context:**
- All come from `debian-packaging` crate (Pure Rust APT implementation)
- Only affect Debian/Ubuntu builds with `--features debian`
- Default Arch Linux builds do NOT include these dependencies
- No known exploitable vulnerabilities, just maintenance status warnings

**Mitigation:**
- Tracked upstream: Monitoring `debian-packaging` for dependency updates
- Consider migrating to `rust-apt` FFI bindings (faster, maintained)
- Default Arch build unaffected

**Fix Timeline:** Monitoring upstream; considering migration in Q2 2026

---

## ✅ Security Best Practices

### For OMG Users

1. **Verify Installation:**
   ```bash
   # Verify PGP signature on download
   curl -fsSL https://pyro1121.com/install.sh.sig -o install.sh.sig
   gpg --verify install.sh.sig install.sh
   ```

2. **Enable Audit Logging:**
   ```toml
   # ~/.config/omg/config.toml
   [security]
   audit_log_enabled = true
   audit_log_path = "~/.local/share/omg/audit.log"
   ```

3. **Use Policy Files:**
   ```toml
   # policy.toml - Restrict installations
   [security.policy]
   allowed_sources = ["official", "aur"]
   require_pgp_verification = true
   block_unverified = true
   ```

4. **Regular Security Scans:**
   ```bash
   # Scan installed packages for vulnerabilities
   omg audit scan
   
   # Generate SBOM for supply chain tracking
   omg sbom generate --format cyclonedx
   ```

### For OMG Contributors

1. **Dependency Hygiene:**
   - Run `cargo audit` before every PR
   - Run `cargo deny check` for license/security compliance
   - Minimize new dependencies

2. **Security Testing:**
   ```bash
   # Run security-focused tests
   cargo test --features proptest
   
   # Check for common vulnerabilities
   cargo clippy -- -D warnings
   ```

3. **Code Review Focus Areas:**
   - Authentication/authorization logic
   - File system operations (`std::fs`, path handling)
   - Network requests (HTTPS only, certificate validation)
   - Subprocess execution (avoid shell injection)

---

## 🔐 Security Features

OMG implements the following security features:

### ✅ Supply Chain Security

- **SLSA Provenance:** Build attestations for verifiable supply chain
- **PGP Verification:** GPG signature verification on all packages
- **SBOM Generation:** CycloneDX SBOM for dependency tracking
- **Vulnerability Scanning:** Built-in CVE database scanning

### ✅ Runtime Security

- **Sandbox Execution:** Optional sandboxing for untrusted packages
- **Audit Logging:** Tamper-proof cryptographic audit logs
- **Secret Scanning:** Prevent accidental credential commits
- **Policy Enforcement:** Mandatory security policies via `policy.toml`

### ✅ Package Security

- **Security Grading:** A-F grade on every package install
- **Dependency Graph:** Full transitive dependency analysis
- **Reproducible Builds:** Lock files for deterministic builds
- **Rollback Support:** Atomic transactions with rollback

---

## 📊 Security Audit History

| Date | Auditor | Scope | Findings | Status |
|------|---------|-------|----------|--------|
| 2026-02-01 | Internal | Dependency audit | 1 medium (Windows), 4 low (Debian) | ✅ Documented |
| TBD | External | Full security audit | Pending | Planned Q2 2026 |

---

## 🔗 Resources

- **Security Docs:** [docs/security.md](docs/security.md)
- **RustSec Advisory DB:** https://rustsec.org
- **SBOM Format:** [CycloneDX](https://cyclonedx.org)
- **SLSA Framework:** https://slsa.dev

---

## 📜 License

This security policy is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).

---

**Last Updated:** 2026-02-01  
**Next Review:** 2026-03-01
