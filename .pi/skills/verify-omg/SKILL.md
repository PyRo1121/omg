---
name: verify-omg
description: Drive and verify OMG's Rust command-line package and runtime manager when changing user-facing CLI behavior, capturing reproducible terminal proof without mutating the host package state.
---

# Verify OMG

OMG's primary user surface is the `omg` CLI. `omgd` is a supporting daemon, `omg-fast` is a fast-path binary, and `omg dash` is an interactive TUI; this initial skill drives the non-interactive CLI because it is the stable public surface covered by `docs/cli.md`.

Run every command from the repository root. Never substitute the installed `~/.local/bin/omg`: verification must drive the binary built from the checkout.

## Launch

OMG is a short-lived CLI, so there is no server or shared process to keep alive. Build once, then run each drive through the supplied shell harness:

```bash
cd "$(git rev-parse --show-toplevel)"
export CARGO_TARGET_DIR="$HOME/.cache/build-targets/omg-verification"
export OMG_VERIFY_RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
./.pi/skills/verify-omg/bin/verify-omg build
./.pi/skills/verify-omg/bin/verify-omg paths
```

The launch is ready when `build` exits zero and prints the `omg` version from `Cargo.toml`. The harness creates disposable state at `~/.cache/omg-verification/$OMG_VERIFY_RUN_ID` and evidence at `.pi/verification-evidence/omg/$OMG_VERIFY_RUN_ID`.

Each run ID has independent OMG data, cache, config, daemon data, and socket paths, so multiple drives can run side by side. The read-only package features share the host's pacman database as an input. Never run two build steps against the shared Cargo target directory concurrently.

Teardown is the `cleanup` command in the Cleanup section.

## Doctor

Run this before driving whenever the build, environment, or output looks wrong:

```bash
./.pi/skills/verify-omg/bin/verify-omg doctor
```

The doctor rejects a missing binary and a binary version that does not match `Cargo.toml`, records the source Git revision and all run paths, and then runs the real `omg doctor` command with isolated application state. It must exit zero. In the disposable environment, `Daemon is not running` and `OMG bin directory not in PATH` are expected observations; dependency, distro, or connectivity failures are not.

## Drive

Use the helper's `drive` command. It captures command, revision, combined stdout/stderr, and exit status in one transcript:

```bash
./.pi/skills/verify-omg/bin/verify-omg drive search-official -- search zsh --no-aur --limit 3
./.pi/skills/verify-omg/bin/verify-omg drive inspect-why -- why glibc
./.pi/skills/verify-omg/bin/verify-omg drive updates-json -- outdated --json
./.pi/skills/verify-omg/bin/verify-omg drive runtime-which -- which node
```

Read `features/README.md`, then the matching feature file before driving. Treat literal command names and flags as the stable handles; do not drive by terminal coordinates or tab order.

The wrapper intentionally allows only these read-only top-level commands: `search`, `info`, `why`, `outdated`, `explicit`, `list`, `which`, `status`, `doctor`, `size`, `blame`, `daemon-status`, `history`, and `stats`. It refuses package mutations, runtime installation/switching, configuration writes, project creation, daemon launch, and TUI launch. Do not bypass that refusal during routine verification. Installing/removing packages is not fully isolated from the host privilege boundary, and `omg dash` requires a dedicated PTY recipe that this skill does not yet provide.

## Evidence

Proof artifacts are written to:

```text
.pi/verification-evidence/omg/$OMG_VERIFY_RUN_ID/
```

`doctor.txt` proves build identity and launch health. Each drive creates a UTC-timestamped `<feature-id>.txt` transcript containing the source HEAD, exact checkout-built command, disposable state path, host pacman-database fingerprints before and after, stdout/stderr, and exit code. The helper fails if that read-only database fingerprint changes.

A valid proof must:

- exercise the public `omg` command a user would type, not a Rust function, test-only override, or the globally installed binary;
- capture the action and resulting output together, including exit status;
- use a mapped feature ID and cover every entry point claimed in that proof;
- check semantic output, not merely that the command returned zero (for example, search results contain the query or package source; JSON parses and has the documented fields);
- retain stdout for human-facing output and a parser check for JSON output;
- keep telemetry and daemon use disabled through the harness;
- verify mutations separately if a future feature adds a truly isolated mutation path; never trust `--dry-run` without observing package DB, filesystem, network, and Git side effects.

For JSON proof, add a parser check after the captured drive, without replacing the user-facing transcript:

```bash
latest=$(find ".pi/verification-evidence/omg/$OMG_VERIFY_RUN_ID" -name '*-updates-json.txt' -print0 | xargs -0 ls -1t | head -1)
awk '/^--- stdout\+stderr ---$/ { capture=1; next } /^--- exit:/ { capture=0 } capture' "$latest" \
  | jq -e 'type == "array"' >/dev/null
```

The current helper drives only read-only features and redirects all writable OMG state into the sandbox. If proving a future mutation, capture both the visible result and a second read-only observation of the isolated state it changed.

## Cleanup

Remove only the sandbox owned by the current run ID:

```bash
./.pi/skills/verify-omg/bin/verify-omg cleanup
```

The helper never kills by process name and never deletes evidence. After cleanup, prove both conditions:

```bash
sandbox="$HOME/.cache/omg-verification/$OMG_VERIFY_RUN_ID"
evidence=".pi/verification-evidence/omg/$OMG_VERIFY_RUN_ID"
test ! -e "$sandbox"
test -d "$evidence" && find "$evidence" -type f -size +0c -print
```

Do not run broad `rm` commands against `~/.cache/omg-verification`, `.pi`, or Cargo target directories. Remove a verification build target separately only when no active run needs it.

## Helpers

The executable helper is `.pi/skills/verify-omg/bin/verify-omg`.

```bash
./.pi/skills/verify-omg/bin/verify-omg --help
```

Supported actions are `build`, `doctor`, `drive`, `paths`, and `cleanup`. Except for `build`, they require `OMG_VERIFY_RUN_ID`. `drive` requires a feature ID followed by `--` and literal OMG arguments. The helper's read-only allowlist is a safety boundary; extending it requires first proving where every write lands and updating the feature map and cleanup recipe.
