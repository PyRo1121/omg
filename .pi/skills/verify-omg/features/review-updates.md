# Review available updates

Users inspect packages that would be updated without starting an update transaction, either as terminal output or machine-readable JSON.

## Sub-features

- `updates-human`: show an understandable list or explicit up-to-date state.
- `updates-json`: emit valid JSON suitable for automation.
- `updates-content`: include package identity and installed/available version information when updates exist.

## How to get to it (user POV)

- Run `omg outdated` for terminal output.
- Run `omg outdated --json` for automation-friendly output.

## Driving it with verify-omg

Preconditions: the helper doctor passes and the host pacman database is readable. The host may have zero or many updates.

- Human-readable view: `./.pi/skills/verify-omg/bin/verify-omg drive updates-human -- outdated`; require exit zero and either a clearly labeled package list or an explicit no-updates state.
- JSON view: `./.pi/skills/verify-omg/bin/verify-omg drive updates-json -- outdated --json`; require exit zero, extract the JSON payload from the retained transcript as documented in `../SKILL.md`, and require `jq -e 'type == "array"'` to pass.
- JSON update row: when the array is non-empty, require each sampled object to contain a package name and installed/available version information. When empty, prove only the empty-array state and do not claim row formatting.

## Gotchas

- Available updates change as mirrors and the host package database change; never assert a fixed count.
- This feature must not call `omg update`, `omg sync`, pacman, or sudo.
- The helper transcript contains metadata around JSON. Parse the extracted payload, not the whole transcript.
- A zero-length update list is a valid user state, not a skipped test.
