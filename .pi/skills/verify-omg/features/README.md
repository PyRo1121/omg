# OMG verification map

This directory is the maintained source for verifying OMG's user-facing CLI behavior. Read this index before driving the app, then use the matching feature file as the literal recipe.

## Baseline preconditions

- Run from `/home/pyro1121/Documents/omg`.
- Set a unique `OMG_VERIFY_RUN_ID` and use `.pi/skills/verify-omg/bin/verify-omg` for every build, doctor, drive, and cleanup action.
- Build the checkout once and require the helper doctor to pass before driving.
- Keep telemetry and daemon use disabled through the helper-provided environment.
- Use the host pacman database only as a read-only input. Never drive package mutation, runtime installation/switching, configuration writes, project generation, daemon launch, or `omg dash` through this initial map.
- Never substitute the installed `~/.local/bin/omg` for the checkout-built binary.

## Driving conventions

- Start every recipe from the baseline state unless its preconditions say otherwise.
- Treat command names, package names, and flags as literal stable handles.
- Use `--no-aur` when proof does not require AUR so the result is local and deterministic.
- Use known host packages such as `glibc` for installed-package inspection; record a blocked precondition rather than silently changing the fixture.
- Give each transcript the feature ID named in its recipe.
- Run cleanup after every attempt, including failed attempts. Cleanup must retain evidence.

## Proof and skip reporting

- Capture the command and resulting stdout/stderr and exit code in one helper transcript.
- Assert semantic output: expected package names, headings, source labels, or JSON shape.
- For JSON, retain the transcript and run a separate parser assertion.
- Read-only proof must use the helper's isolated writable paths and must not invoke sudo.
- Report an unreachable command with its attempted invocation and unmet precondition.
- Do not claim an untested entry point is covered by a nearby successful command.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then uses exactly four H2 sections in this order.

1. `Sub-features`
2. `How to get to it (user POV)`
3. `Driving it with verify-omg`
4. `Gotchas`

## Features

- [Search packages](./search-packages.md) covers official-repository search, result limits, and detailed/AUR search.
- [Inspect a package](./inspect-package.md) covers package metadata, dependency reasons, installed size, and installation history.
- [Review available updates](./review-updates.md) covers human-readable and JSON outdated-package views.
- [Discover runtimes](./discover-runtimes.md) covers installed version lists, available version lists, and runtime selection resolution.
- [Check system health](./check-system-health.md) covers concise status and the full doctor report.
