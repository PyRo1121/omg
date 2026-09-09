# OMG

**Your packages. Your runtimes. One CLI.**

Search your distro's packages, switch Node.js or Python versions, and run project tasks without remembering a different command for each tool. OMG combines package backends, runtime version management, and environment drift checks in a Rust CLI.

**Spend less time switching tools. [Start with one project](docs/quickstart.md).** Select its runtime and run its existing tasks with OMG. Keep your native package manager available.

> OMG is alpha software. Commands and on-disk formats can change. Try package mutations in a disposable VM or on a machine you can recover. Back up important data before upgrades or migrations.

[Installation](docs/installation.md) · [CLI reference](docs/cli.md) · [Security limits](docs/security.md) · [Report a bug](https://github.com/PyRo1121/omg/issues)

## Try it without changing system packages

Release installation requires GitHub CLI (`gh`) for archive attestation verification, plus `curl` and the platform dependencies listed in the [installation guide](docs/installation.md). Download the installer for review instead of piping it directly into a shell:

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://getomg.xyz/install.sh -o omg-install.sh
less omg-install.sh
OMG_NO_TELEMETRY=1 OMG_SKIP_SHELL=1 bash omg-install.sh
export PATH="$HOME/.local/bin:$PATH"
omg --version
omg --help
```

The installer writes to `~/.local/bin` by default. These options disable install telemetry and automatic shell edits. The downloaded script is executable code from a mutable endpoint. For a pinned bootstrap, review and run a checkout at a commit you trust. Do not bypass a failed checksum or attestation check.

In an existing Node.js project:

```bash
omg use node 22
omg run build
```

`omg use` downloads a runtime if needed and changes the selected version. Inspect the project's `package.json` scripts and run `build` only if it is defined. `omg run` requires a task name. Tasks execute project code, so use a repository you trust.

For directory-based switching, add the matching [shell hook](docs/shell-integration.md) after reviewing what it does.

## What you can do

- **Use one package command vocabulary.** Search, install, remove, and update through the backend built for your platform. On Arch, search official repositories and AUR, or pass `--no-aur` for official results only.
- **Select runtimes per project.** Manage Node.js, Python, Rust, Go, Bun, Ruby, Java, and Pi. Existing version files can guide selection. Providers and platform availability differ. See [runtimes](docs/runtimes.md).
- **Run existing tasks.** `omg run` discovers tasks from supported project manifests, including `package.json`, `Cargo.toml`, and `Makefile`.
- **Make environment drift visible.** `omg env capture` writes `omg.lock`; `omg env check` compares it with the current machine. Sharing a lockfile does not install dependencies or guarantee identical machines.
- **Inspect before changing packages.** Use `omg info ripgrep`, `omg why ripgrep`, and `omg install --dry-run ripgrep` before an installation. A dry run is not a sandbox or an approval of package code.

OMG does not replace every capability of pacman, APT, DNF, Homebrew, rustup, or language-specific tools. Some operations delegate to those tools.

## Choose the right platform build

The release workflow builds Linux x86_64 archives for Arch, Debian, Ubuntu, and Fedora, plus a macOS ARM64 archive. Availability of an archive does not establish feature parity.

- **Arch Linux.** The ALPM backend supports official packages and AUR workflows. AUR builds execute community code and need review.
- **Debian and Ubuntu.** The native APT backend supports package operations. Security inventory and policy capabilities differ from Arch.
- **Fedora.** The DNF backend exists, but the recorded `v0.1.218` package smoke tests failed. Fixed local candidates are not proof that a published release is fixed.
- **macOS.** The release target is Apple Silicon with a Homebrew backend. There is no Intel macOS release artifact in the current workflow.
- **Windows.** Use a supported Linux distribution inside WSL. Native Windows is not supported.

Check [installation and backend limits](docs/installation.md) before choosing a binary. Do not use an Arch build on another distribution.

## Security evidence, with explicit limits

The release workflow generates a CycloneDX dependency SBOM from Cargo metadata and attests release archives and the SBOM through GitHub Actions. The installer verifies the selected archive against the release tag and workflow.

The local `omg audit sbom` command is different. It generates an installed-package CycloneDX 1.5 inventory with Arch advisory matching. It does not reconstruct a complete dependency graph, and its current CLI path does not support Debian, Fedora, or macOS SBOM generation.

`omg audit slsa` can verify supported Rekor artifact signatures with an exact expected certificate identity. It does **not** verify SLSA build provenance or establish SLSA Levels 1–3. Local audit chains establish consistency, not authenticity. Exported inventories and audit evidence are plaintext, not encrypted compliance archives.

Read the [security reference](docs/security.md) and [enterprise export limits](docs/enterprise.md) before using reports as evidence. OMG does not certify HIPAA, SOC 2, ISO 27001, PCI DSS, or FedRAMP compliance.

## Performance you can inspect

The optional daemon keeps package indexes in memory. Its benefit depends on the backend, query, cache state, and enabled sources. We do not claim a universal speedup or equivalent output across competing package managers.

The [benchmark guide](benchmarks/README.md) links methodology and raw records. The [published-artifact smoke record](benchmarks/records/release-smoke-v0.1.218-local.json) and [local QEMU receipt](benchmarks/records/qemu-four-distros-20260905.json) identify their tested artifacts. Smoke durations are not CLI latency benchmarks, and local debug candidates are not released binaries.

## Take the next step

**[Follow the quickstart](docs/quickstart.md), then try OMG on a recoverable development machine.** If a command fails, [open an issue](https://github.com/PyRo1121/omg/issues) with `omg --version`, your distribution, the command, and redacted output. That gives maintainers a reproducible case.

- [Configuration](docs/configuration.md), [package operations](docs/packages.md), and [troubleshooting](docs/troubleshooting.md).
- [Contribute a focused fix or documentation correction](CONTRIBUTING.md).
- Report vulnerabilities privately to <olen@latham.cloud>. See [SECURITY.md](SECURITY.md).

## License

OMG is [MIT licensed](LICENSE). Copyright 2024–2026 Olen Latham.
