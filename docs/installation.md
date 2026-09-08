# Install OMG

OMG is alpha software. Use a recoverable development machine or disposable VM for package mutations. Keep your native package manager available.

## Choose a supported release target

The [release workflow](../.github/workflows/release.yml) builds:

- Linux x86_64 for Arch, Debian, Ubuntu, and Fedora, each with a separate backend archive.
- macOS ARM64 for Apple Silicon, with the Homebrew backend.

There is no current Intel macOS or Linux ARM64 release artifact. Rosetta does not run ARM64 binaries on Intel Macs. Native Windows is unsupported; use a supported Linux distribution inside WSL. Fedora support does not establish RHEL compatibility.

Arch uses ALPM and supports AUR builds. Debian and Ubuntu use the native APT backend. Fedora uses DNF, with database reads and subprocess fallbacks. macOS package operations require Homebrew; OMG itself is not packaged as a Homebrew formula here.

These backends do not have identical policy, audit, or runtime coverage. The recorded `v0.1.218` Fedora package smoke tests failed. Later local candidate results do not establish that a published archive is fixed. Check the [artifact-specific evidence](../benchmarks/README.md) before choosing a release.

## Review and run the installer

Downloaded releases require `curl`, archive tools, and GitHub CLI (`gh`) for build-provenance verification. Install those prerequisites through your trusted platform tools first.

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://omg.latham.cloud/install.sh -o omg-install.sh
less omg-install.sh
OMG_NO_TELEMETRY=1 OMG_SKIP_SHELL=1 bash omg-install.sh
export PATH="$HOME/.local/bin:$PATH"
omg --version
omg --help
```

The default destination is `~/.local/bin`. The options above disable installer telemetry and shell edits. Without `OMG_SKIP_SHELL=1`, the installer can modify shell startup files. Review any dependency-installation prompt before approving it.

The installer checks the archive checksum and verifies GitHub attestation against the requested release tag and OMG release workflow. A missing `gh` or failed attestation stops release installation. There is no supported checksum-only opt-out in the current script.

Archive verification does not authenticate the bootstrap script retroactively. The download URL is mutable. For a reproducible bootstrap, review a repository checkout at a trusted commit and run its `install.sh` instead. See [security boundaries](../SECURITY.md#security-boundaries-and-retained-trust).

## Installation options

Apply environment variables to the shell running the script:

```bash
OMG_VERSION=v0.1.218 OMG_NO_TELEMETRY=1 OMG_SKIP_SHELL=1 bash omg-install.sh
INSTALL_DIR="$HOME/.omg/bin" OMG_SKIP_SHELL=1 bash omg-install.sh
```

The version above is an example pin, not a recommendation that its artifacts pass all tests. Select a release after reviewing its notes and evidence. A custom directory must be added to `PATH`.

## Install from AUR on Arch

Review the AUR package recipe before building or installing:

```bash
yay -S omg-bin
```

Use `yay -S omg` for the source package. AUR packaging has its own build and trust path; do not assume it executes the standalone installer's verification sequence.

## Build from source

Use the toolchain pinned in `rust-toolchain.toml`, currently Rust 1.95.0. Build prerequisites depend on the backend. Arch needs libalpm and its development dependencies. Native Debian builds need `libapt-pkg-dev`, `clang`, `cmake`, `pkg-config`, and OpenSSL development headers. macOS needs Xcode Command Line Tools. See [contributing](../CONTRIBUTING.md) for development setup.

From a reviewed checkout, select exactly one backend:

```bash
# Arch
cargo build --release --locked --no-default-features --features arch,pgp,license
# Debian or Ubuntu
cargo build --release --locked --no-default-features --features debian,pgp,license
# Fedora
cargo build --release --locked --no-default-features --features fedora,pgp,license
# Apple Silicon macOS
cargo build --release --locked --no-default-features --features macos,pgp,license
```

Do not run every command. Cargo features are additive, so `--features debian` alone does not remove the default Arch backend. The `license` feature compiles account-linking support; it is not a local CLI paywall.

Inspect `target/release/omg --help` before installing a built binary. An explicit source-install path is `bash ./install.sh --from-source` from a trusted checkout. Source builds are not release-attested binaries.

## Set up your shell

Start with [the quickstart](./quickstart.md). For automatic directory-based runtime selection, add the appropriate line to your shell configuration once:

```bash
# Bash
eval "$(omg hook bash)"
# Zsh
eval "$(omg hook zsh)"
```

For Fish:

```fish
omg hook fish | source
```

Use only the line for your shell. Review [shell integration](./shell-integration.md) before combining OMG with another runtime manager.

Generate completions with `omg completions bash`, `omg completions zsh`, or `omg completions fish`. The command installs the completion file and reports its location. Use `--stdout` only when you want the script itself.

## Daemon requirements

Most package queries can use direct fallback paths. Vulnerability scans, Unix SOC 2 export, and metrics require the daemon. The current SBOM CLI also requires the Arch backend and advisory access, independently of daemon availability.

```bash
omg daemon-status
omg daemon
```

`omg daemon` launches the separate `omgd` binary in the background. Use `omg daemon --foreground` to see its output, or configure the optional [user service](./configuration.md). The current Arch archive includes `omgd`; the other release packaging steps include only `omg`. Those archives alone cannot provide daemon-dependent commands. A matching `omgd` build must also be installed.

## Update or uninstall

For a standalone release installation:

```bash
omg self-update
```

For an AUR-managed installation, update through your AUR package manager instead. Do not mix installation methods without checking which binary `command -v omg` selects.

To uninstall a script-managed installation, review and run the installer's `--uninstall` mode. Review its handling of shell entries and data before confirming. For AUR, use `yay -R omg-bin` or the package name you installed.

Do not delete `~/.local/share/omg` as a routine binary-uninstall step. It can contain runtimes, audit history, and other durable data. Back up and remove such data separately only when intended.

## CI setup

Pin the installer source and OMG release used by your pipeline. Ensure `gh` can perform the required attestation verification. Set `OMG_NO_TELEMETRY=1` and `OMG_SKIP_SHELL=1`, and add the chosen binary directory to the CI job's `PATH` explicitly. Shell startup edits are not a substitute for CI environment setup.

Install named system packages or select required runtimes before running project tasks. Bare `omg install` is an interactive package picker, not a project-dependency installer. `omg env check` reports drift; it does not fix it.

## Next steps

[Run your first project task](./quickstart.md), read [backend security limits](./security.md), or [troubleshoot installation](./troubleshooting.md).
