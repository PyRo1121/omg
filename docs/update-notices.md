# Terminal update notices

After installing this version, existing `omg hook bash`, `omg hook zsh`, and
`omg hook fish` integrations check for an OMG update when an interactive terminal
opens. Run `omg init` once if shell integration is not installed.

The first terminal starts a background check; a subsequent terminal can show:

```text
OMG 0.1.222 is available (installed: 0.1.221). Update with your package manager or `omg self-update`. Notes: https://github.com/PyRo1121/omg/releases/tag/v0.1.222
```

Checks and notices are limited to once per day per user cache, including failed
network attempts. Startup only reads local state; a separate process has at most
three seconds to fetch the same HTTPS release marker used by `omg self-update`.
Offline errors are silent. The check sends no install identifier or telemetry,
although the HTTPS server receives normal request/network metadata.
Noninteractive shells, CI and root sessions do not check or display notices.
Versions older than the installed build, prereleases and malformed metadata are
not advertised. Cached results older than seven days are not shown.

No packages are installed automatically. Package-managed installations should
update through their package manager. Direct installations can use
`omg self-update`, which retains checksum and provenance verification.

To disable both checking and notices, put this **before** the OMG hook:

```bash
# Bash or Zsh
export OMG_NO_UPDATE_CHECK=1
```

```fish
# Fish
set -gx OMG_NO_UPDATE_CHECK 1
```

Remove that variable to enable the feature again. State lives in
`$XDG_CACHE_HOME/omg/update-notice.json` (normally `~/.cache/omg`), or the explicitly
configured `OMG_CACHE_DIR`. Unsafe or unreadable state is ignored. A damaged cache
can be removed to let the next terminal recreate it; it is not required to use OMG.
