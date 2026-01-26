# Security Policy

## Supported Versions

We only provide security updates for the latest stable release.

| Version | Supported          |
| ------- | ------------------ |
| latest  | :white_check_mark: |
| < latest| :x:                |

## Reporting a Vulnerability

We take the security of OMG very seriously. If you believe you have found a security vulnerability, please report it to us responsibly.

**Do not open a public GitHub issue for security vulnerabilities.**

Please send security reports to: `olen@latham.cloud`

### What we need from you

- A detailed description of the vulnerability.
- Steps to reproduce the issue (a Proof of Concept script is highly appreciated).
- Any potential impact you've identified.

### What you can expect from us

- A response acknowledging your report within 48 hours.
- An estimated timeframe for a fix.
- Notification once the vulnerability has been patched.
- Credit in our security advisories (if desired).

## Security Hardening in OMG

OMG is built with several security-first principles:
- **Pure Rust Logic**: Minimizing memory safety issues by avoiding C dependencies where possible.
- **Sandboxed Execution**: Daemon-client architecture allows for privilege separation.
- **Supply Chain Security**: We use automated tools (`cargo-deny`, `cargo-audit`) to monitor dependencies.
- **Binary Hardening**: Releases are verified with `checksec` to ensure modern OS protections are enabled.
