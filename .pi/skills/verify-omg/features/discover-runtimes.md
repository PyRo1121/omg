# Discover runtimes

Users inspect runtime versions known to OMG and see which Node, Python, Rust, or other runtime version the current project would select.

## Sub-features

- `runtime-list-installed`: list versions installed in the isolated OMG data directory.
- `runtime-list-available`: list versions available for a named runtime.
- `runtime-which`: explain which runtime version would be selected by the current project.
- `runtime-empty`: present an understandable state when no isolated versions are installed.

## How to get to it (user POV)

- Run `omg list` to list installed runtime versions.
- Run `omg list node` for one runtime.
- Run `omg list node --available` to query available Node versions.
- Run `omg which node` to resolve the version selected by project files or OMG state.

## Driving it with verify-omg

Preconditions: the helper doctor passes. Installed-version commands use the harness's empty `OMG_DATA_DIR`; available-version queries require network access.

- Empty installed list: `./.pi/skills/verify-omg/bin/verify-omg drive runtime-list-installed -- list node`; require exit zero, a Node versions heading, and no claim that the host's real `~/.local/share/omg` versions are installed.
- Resolution: `./.pi/skills/verify-omg/bin/verify-omg drive runtime-which -- which node`; require exit zero and either the selected version with its source or the explicit `no version set` state.
- Available versions: `./.pi/skills/verify-omg/bin/verify-omg drive runtime-list-available -- list node --available`; require exit zero and version-shaped entries. Record network failure as blocked and retain the transcript.

## Gotchas

- The harness deliberately hides the user's real OMG runtime installation by redirecting `OMG_DATA_DIR`.
- `omg use` downloads and switches runtimes and is outside the read-only allowlist; do not use it as a shortcut for discovery proof.
- `which` depends on project files such as `.tool-versions` or `.nvmrc`, so both a selected version and `no version set` can be valid.
- Available-version results are network- and upstream-dependent; assert version shape and labels, not an exact list.
