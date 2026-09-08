---
title: Cheat Sheet
sidebar_position: 99
description: Common OMG commands and their limits
---

# OMG cheat sheet

Install using the reviewed-download procedure in [installation](./installation.md). Examples below assume OMG is already installed. Choose versions for your project; example versions are not a current security recommendation.

## Inspect before changing state

```bash
omg --version
omg --all-commands --help
omg doctor
omg status
omg daemon-status
omg search ripgrep
omg info ripgrep
omg explicit --count
omg outdated
omg history
```

Use `omg COMMAND --help` for exact flags. A global `--json` parser option does not guarantee that every command implements a stable JSON response.

## Package operations

```bash
omg install --dry-run ripgrep
omg remove --dry-run ripgrep
omg update --check
omg update --dry-run
omg clean --dry-run --all
```

After reviewing the preview, omit `--dry-run` to request the mutation. `omg sync` refreshes repository metadata. Backends differ; Arch-only options are not portable transaction controls. Cleanup can remove rollback artifacts. See [packages](./packages.md).

## Runtimes and tasks

```bash
omg list node
omg list node --available
omg which node
omg use node 22
omg run build
omg run test -- --verbose
```

`use` can download and install software; `run` executes project code. Review untrusted repositories first. Supported runtime names are `node`, `python`, `go`, `rust`, `ruby`, `java`, `bun`, and `pi`.

For Rust, `rust-toolchain.toml` must contain TOML, not a bare channel name:

```toml
[toolchain]
channel = "stable"
```

Follow [shell integration](./shell-integration.md) for your shell's hook and completions. Directory-change activation selects installed versions; it is not a dependency installer.

## Environment records

```bash
omg env capture
omg env check
omg env share
omg env sync https://gist.github.com/USER/GIST_ID
```

`capture` writes a lockfile; `share` uploads inventory and requires configured credentials; `sync` downloads the record and checks drift without installing packages. Review private inventory before sharing. An unlisted Gist is not encrypted. See [team workflows](./team.md).

## Security and recovery

```bash
omg audit policy
omg audit scan
omg audit verify
omg audit sbom -o sbom.json
```

These operations have [backend and evidence limits](./security.md). Scan findings alone do not cause failure. SBOM advisory matching requires Arch support; exports are plaintext. Local log verification proves neither authenticity nor completeness. The SLSA-named command does not verify a SLSA build level.

Inspect history before considering `omg rollback TRANSACTION_ID`; rollback changes packages and is not a complete machine restore. Never reset malformed history or audit logs. See [history](./history.md) and [troubleshooting](./troubleshooting.md).

## Configuration and optional interfaces

```bash
omg config path
omg config list
omg config validate
omg account status
omg container status
omg hooks status
omg hooks uninstall
omg dash
```

Configuration output may disclose private settings; review before sharing. Account linking accepts a token through standard input (`omg account link --token-stdin`), not a command-line token. Use a credential manager rather than putting tokens in shell startup files.

`hooks uninstall` removes only byte-exact current OMG hook templates. Custom, composed, historical, and nonregular files stay in place.

See [CLI reference](./cli.md), [configuration](./configuration.md), [containers](./containers.md), and [installation](./installation.md). CI needs a pinned, verified installer/artifact and a prepared backend-compatible environment; a moving `curl | bash` URL is not a release pin.
