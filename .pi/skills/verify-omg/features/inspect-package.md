# Inspect a package

Users inspect installed package metadata and understand why it is present, how much space it consumes, and when it entered the system.

## Sub-features

- `inspect-info`: show package identity, version, repository, description, and installation state.
- `inspect-why`: show dependency-chain or explicit-install reasoning.
- `inspect-size`: show installed-size information.
- `inspect-blame`: show installation-history information when available.

## How to get to it (user POV)

- Run `omg info <package>` for package metadata.
- Run `omg why <package>` to explain why it is installed.
- Run `omg size --tree <package>` for that package's installed size and dependency footprint.
- Run `omg blame <package>` for installation timing and reason.

## Driving it with verify-omg

Preconditions: the helper doctor passes and `glibc` is present in the host's read-only pacman database.

- Metadata: `./.pi/skills/verify-omg/bin/verify-omg drive inspect-info -- info glibc`; require exit zero and output that identifies `glibc`, its installed version, and its repository or installation state.
- Dependency reasoning: `./.pi/skills/verify-omg/bin/verify-omg drive inspect-why -- why glibc`; require exit zero, the `Package Analysis` heading, package name `glibc`, and a package-information or dependency-reason section.
- Size: `./.pi/skills/verify-omg/bin/verify-omg drive inspect-size -- size --tree glibc`; require exit zero, the `Package Size Tree` heading, package name `glibc`, and a numeric size with an explicit unit.
- Installation history: `./.pi/skills/verify-omg/bin/verify-omg drive inspect-blame -- blame glibc`; when transaction history exists, require a timestamp or reason tied to `glibc`; otherwise retain and report the explicit unavailable-history state.

## Gotchas

- `glibc` is a stable Arch fixture but still verify the precondition rather than assuming it.
- `why` can legitimately report explicit installation instead of a dependency chain.
- `omg size` takes `--tree <package>` or `--limit`, not a positional package name; `omg size glibc` is invalid.
- `blame` depends on local package transaction history and may not reconstruct an old install.
- Do not replace these commands with direct reads of `/var/lib/pacman`; the user-facing formatter and exit behavior are part of the feature.
