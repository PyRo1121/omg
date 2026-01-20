---
title: Security & Compliance
sidebar_position: 20
description: Vulnerability scanning, SBOM, PGP verification, and audit logging
---

# Security Model

OMG implements enterprise-grade security with defense-in-depth: vulnerability scanning, PGP verification, SLSA provenance, SBOM generation, secret scanning, tamper-proof audit logging, and configurable security policies. All operations are user-isolated with minimal privilege requirements.

## Quick Reference

| Command | Description |
|---------|-------------|
| `omg audit` | Vulnerability scan (default) |
| `omg audit scan` | Scan installed packages for CVEs |
| `omg audit sbom` | Generate CycloneDX 1.5 SBOM |
| `omg audit secrets` | Scan for leaked credentials |
| `omg audit log` | View audit log entries |
| `omg audit verify` | Verify audit log integrity |
| `omg audit policy` | Show security policy status |
| `omg audit slsa <pkg>` | Check SLSA provenance |

## Security Overview

### Threat Model

OMG protects against:
- **Malicious Packages**: PGP signatures and vulnerability scanning
- **Supply Chain Attacks**: SLSA provenance verification via Sigstore/Rekor
- **Leaked Credentials**: Secret scanning for 20+ credential types
- **Compliance Violations**: SBOM generation for FDA, FedRAMP, SOC2
- **Privilege Escalation**: User-level operations only
- **Network Attacks**: HTTPS-only with certificate validation
- **Data Tampering**: Checksum verification and hash-chained audit logs

### Security Grades

Packages are classified into four security grades to help you make informed decisions about your system's integrity:

| Grade | Description | Security Features |
|-------|-------------|-------------------|
| **Locked** | Mission-critical system packages | SLSA Level 3 + PGP + Signature Verification |
| **Verified** | Official repository packages | PGP Signature Verification + Checksum |
| **Community** | AUR and user-maintained packages | Checksum Verification (Unsigned) |
| **Risk** | Packages with known issues | Contains active CVEs or security advisories |

Grade definitions:
- **Locked**: Core system components like `glibc`, `linux`, and `pacman`. These are verified against the highest standards including SLSA build provenance.
- **Verified**: Packages from official Arch/Debian repositories that are signed by trusted maintainers.
- **Community**: Packages from the AUR or third-party sources. While they have checksums, they lack official cryptographic signatures.
- **Risk**: Any package with one or more known vulnerabilities in the ALSA or OSV databases.

## Vulnerability Scanning

### Architecture

The OMG vulnerability scanner is designed for high-performance, parallel analysis. It integrates multiple security databases to provide a comprehensive view of your system's risk profile.

1. **Arch Linux Security Advisory (ALSA)**: Provides real-time information on vulnerabilities specifically affecting Arch Linux packages.
2. **OSV.dev**: A high-speed, distributed vulnerability database that covers multiple ecosystems.

### ALSA Integration

OMG periodically fetches the complete set of ALSA issues. Each advisory includes:
- **CVE Identifier**: The unique ID for the vulnerability.
- **Affected Packages**: A list of specific package names and version ranges.
- **Status**: Whether the issue is fixed, currently being addressed, or not yet fixed.
- **Severity**: A rating from Low to Critical based on the potential impact.

### OSV.dev Integration

For broader coverage, OMG queries the Open Source Vulnerabilities (OSV) API. To maintain speed, results are cached locally:
- **Querying**: When you scan a package, OMG sends the name and version to the OSV API.
- **Processing**: The API returns detailed reports including CVSS scores and fix versions.
- **Caching**: Results are stored in an in-memory concurrent cache for 1 hour, ensuring that subsequent checks for the same package are instantaneous.

### Parallel Scanning

When you run a full system audit (`omg audit`), OMG employs a massively parallel scanning strategy:
1. **Chunking**: Your installed packages are divided into small chunks.
2. **Concurrent Execution**: Each chunk is scanned in parallel using all available CPU cores.
3. **Aggregation**: The results are merged, filtered by severity (CVSS >= 7.0 for high severity), and presented in a unified report.

## Native PGP Verification

OMG performs all cryptographic operations natively using a thread-safe implementation. Unlike standard managers that call an external binary (which is slow and difficult to parallelize), OMG leverages native logic to verify signatures for dozens of packages in parallel across all available CPU cores without process-spawning overhead.

- **Direct System Integration**: No dependency on external security binaries.
- **Keyring Awareness**: Automatically leverages official system keyrings for seamless verification.
- **Modern Standards**: Built to support advanced and future-proof cryptographic algorithms.

### Verification Lifecycle

When verifying a package, OMG follows a rigorous multi-step process:

1.  **Extract**: Retrieves cryptographic signatures from package artifacts.
2.  **Calculate**: Computes integrity hashes using modern secure algorithms.
3.  **Match**: Identifies the correct public keys within the system keyring.
4.  **Validate**: Performs the mathematical validation of the signature against the calculated hash.
5.  **Assess**: Confirms the signing authority is trusted and the certificate is current.


## SLSA Provenance

### Architecture

SLSA (Supply-chain Levels for Software Artifacts) provides a framework for ensuring the integrity of the entire software supply chain.

- **Provenance**: Cryptographic proof of how and where a package was built.
- **Integrity**: Guarantees that the binary you download is exactly what was produced by the build system.
- **Traceability**: Allows you to trace a binary back to its exact source code commit.

### Verification

OMG integrates with **Sigstore** and the **Rekor** transparency log to verify build provenance. When checking a package, OMG ensures that the package hash is recorded in a tamper-proof public log and matches the signed attestation from a trusted build environment (like GitHub Actions or GitLab CI).

### Hash-Based Integrity

Beyond signatures, OMG performs a streaming SHA-256 hash validation on every downloaded artifact. This ensures that the data on your disk matches the exact byte sequence reported by the upstream repository, protecting against bit rot and man-in-the-middle attacks during the download process.

## Security Policy

### Policy Configuration

You can define custom security policies in your `omg.toml` or via the CLI to enforce organizational standards.

Key policy options:
- **Minimum Grade**: Reject any package that doesn't meet a specific security level (e.g., "Verified only").
- **AUR Restrictions**: Toggle whether community-contributed packages are allowed.
- **PGP Requirement**: Force signature verification for all installations.
- **License Whitelist**: Only allow packages with approved licenses (e.g., MIT, Apache-2.0).
- **Blacklist**: Explicitly ban specific packages from being installed.

### Enforcement

The policy engine acts as a gatekeeper for every operation. Before a package is installed or updated, OMG checks it against your policy rules. If a package is banned, has too low a grade, or violates license requirements, the operation is blocked with a detailed explanation of the violation.

## Security Grading Logic

The grading system uses a strictly hierarchical check to determine a package's safety level:

1.  **Risk Assessment**: If a package has any known vulnerabilities in the security databases, it is immediately graded as **Risk**, regardless of its source or signature.
2.  **Core Integrity**: Packages that form the system's foundation (such as the kernel or core libraries) are graded as **Locked** if they meet both the SLSA Level 3 provenance and official PGP standards.
3.  **Official Verification**: Packages from official repositories that carry a valid PGP signature from a trusted maintainer are graded as **Verified**.
4.  **Community Standards**: Packages from the AUR or third-party sources that provide valid checksums but lack official cryptographic signatures are graded as **Community**.

## Verification Pipeline

When a package is integrated into the system, it passes through a rigorous 5-stage native verification process:

*   **Extraction**: Cryptographic signatures are retrieved from detached signature artifacts.
*   **Integrity Calculation**: A streaming hash is computed using secure modern algorithms to ensure the file matches the expected byte sequence.
*   **Key Identification**: The system keyring is searched for the specific public key associated with the signature.
*   **Mathematical Validation**: The public key and calculated hash are used to perform a native cryptographic validation of the signature.
*   **Trust Evaluation**: The system confirms the signing certificate is active, trusted by the root authority, and has not expired.

### Network Security

- **HTTPS Only**: All network traffic uses TLS
- **Certificate Pinning**: Known certificates for critical endpoints
- **Timeout Protection**: 5-second timeout for vulnerability API
- **Retry Logic**: Exponential backoff for failures

### File System Security

- **User Isolation**: All operations as current user
- **Permission Checks**: Validate permissions before operations
- **Atomic Operations**: Use atomic writes where possible
- **Cleanup**: Remove temporary files securely

## Monitoring and Auditing

### Security Events

All security-relevant operations—such as signature verifications, vulnerability detections, and policy rejections—are captured by the system's logging backend. These events provided detailed context for audits and real-time monitoring.

### Audit Trail

- **Package Installs**: Full audit log with checksums
- **Security Scans**: Timestamp and results
- **Policy Violations**: All rejections logged
- **Configuration Changes**: Policy updates tracked

### Metrics Collection

Security metrics available:
- **Vulnerability Count**: Total and by severity
- **Verification Rate**: PGP verification success
- **Policy Violations**: Rejection reasons
- **Scan Performance**: Time per package

## Best Practices

### For Users

1. **Enable PGP Verification**: Always verify signatures
2. **Regular Scans**: Run security audits weekly
3. **Policy Configuration**: Set appropriate minimum grades
4. **Update Keyring**: Keep GPG keys current
5. **Review Logs**: Monitor security events

### For Organizations

1. **Central Policies**: Distribute security policies
2. **License Compliance**: Configure allowed licenses
3. **Package Blacklist**: Block problematic packages
4. **Regular Audits**: Automated security scanning
5. **Incident Response**: Plan for vulnerability disclosures

### For Developers

1. **Sign Packages**: Always sign custom packages
2. **SLSA Attestations**: Provide build provenance
3. **Vulnerability Disclosure**: Report security issues
4. **Secure Defaults**: Enable security by default
5. **Documentation**: Document security features

## SBOM Generation

OMG generates CycloneDX 1.5 compliant Software Bill of Materials for enterprise compliance.

### Usage

```bash
# Generate SBOM with vulnerabilities
omg audit sbom --vulns

# Export to specific file
omg audit sbom -o /path/to/sbom.json

# Generate without vulnerability data
omg audit sbom --vulns=false
```

### SBOM Contents

The generated SBOM includes:
- **All installed packages** with PURL identifiers
- **Version information** for each component
- **Vulnerability data** (optional) from ALSA
- **Metadata** including generation timestamp and tool version

### Compliance Standards

OMG's SBOM generation supports:
- **FDA Cybersecurity Requirements** for medical devices
- **FedRAMP** for federal systems
- **SOC2** for enterprise compliance
- **NTIA Minimum Elements** for software transparency

## Secret Scanning

OMG detects leaked credentials before they're committed.

### Usage

```bash
# Scan current directory
omg audit secrets

# Scan specific path
omg audit secrets -p /path/to/project
```

### Detected Secret Types

| Type | Pattern | Severity |
|------|---------|----------|
| AWS Access Key | `AKIA...` | Critical |
| AWS Secret Key | `aws_secret_access_key=...` | Critical |
| GitHub Token | `ghp_...`, `github_pat_...` | Critical |
| GitLab Token | `glpat-...` | Critical |
| Private Key | `-----BEGIN PRIVATE KEY-----` | Critical |
| Stripe Key | `sk_live_...` | Critical |
| Slack Token | `xoxb-...` | High |
| Google API Key | `AIza...` | High |
| NPM Token | `npm_...` | High |
| JWT Token | `eyJ...` | Medium |
| Generic API Key | `api_key=...` | Medium |
| Generic Password | `password=...` | Medium |

### Placeholder Detection

The scanner automatically ignores common placeholders:
- `your_api_key_here`
- `example_token`
- `<API_KEY>`
- `${SECRET}`

## Audit Logging

OMG maintains tamper-proof audit logs for compliance and forensics.

### Usage

```bash
# View recent entries
omg audit log

# View last 50 entries
omg audit log -l 50

# Filter by severity
omg audit log -s error

# Verify log integrity
omg audit verify
```

### Event Types

| Event | Description |
|-------|-------------|
| `PackageInstall` | Package installation |
| `PackageRemove` | Package removal |
| `PackageUpgrade` | Package upgrade |
| `SecurityAudit` | Security scan performed |
| `VulnerabilityDetected` | CVE found |
| `SignatureVerified` | PGP verification success |
| `SignatureFailed` | PGP verification failure |
| `PolicyViolation` | Policy rule triggered |
| `SbomGenerated` | SBOM created |

### Tamper Detection

Each audit entry includes:
- **SHA-256 hash** of entry contents
- **Previous entry hash** for chain integrity
- **Timestamp** in ISO 8601 format
- **User** who performed the action

The `omg audit verify` command validates:
1. Each entry's hash matches its contents
2. The hash chain is unbroken
3. No entries have been modified or deleted

### Log Location

Audit logs are stored at:
```
~/.local/share/omg/audit/audit.jsonl
```

## SLSA Verification

OMG verifies SLSA provenance via Sigstore/Rekor.

### Usage

```bash
# Check SLSA provenance for a package file
omg audit slsa /path/to/package.pkg.tar.zst
```

### SLSA Levels

| Level | Requirements | OMG Support |
|-------|--------------|-------------|
| Level 1 | Build process documented | ✅ |
| Level 2 | Hosted build, signed provenance | ✅ |
| Level 3 | Hardened build, non-falsifiable | ✅ |

### Rekor Integration

OMG queries the Sigstore Rekor transparency log to verify:
- Package hash is recorded in the log
- Build attestation is present
- Signature is valid

### Package SLSA Levels

| Package Type | Default Level |
|--------------|---------------|
| Core packages (glibc, linux, pacman) | Level 3 |
| Official repo packages | Level 2 |
| AUR packages | None |

## Future Security Enhancements

### Planned Features

1. **Policy-as-Code**: OPA/Rego integration for complex policies
2. **Runtime Security**: Monitor package behavior post-install
3. **Machine Learning**: Anomaly detection for suspicious packages
5. **Zero-Trust**: Enhanced verification

### Emerging Threats

1. **Supply Chain Attacks**: Enhanced provenance
2. **Deep Package Inspection**: Static analysis
3. **Behavioral Analysis**: Runtime monitoring
4. **Threat Intelligence**: CVE database integration
5. **Compliance**: Industry standard support

### Cryptographic Improvements

1. **Post-Quantum**: Prepare for quantum computing
2. **Multi-Sig**: Multiple signature support
3. **Key Rotation**: Automated key management
4. **Hardware Tokens**: YubiKey integration
5. **Secure Enclaves**: TPM integration
