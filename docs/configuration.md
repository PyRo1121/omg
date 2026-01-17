# Configuration & Policy

OMG uses XDG defaults when available, with clear fallbacks for portability.

## Locations
- **Config**: `~/.config/omg/config.toml`
- **Policy**: `~/.config/omg/policy.toml`
- **Data**: `~/.local/share/omg/` (fallback: `~/.omg/`)
- **Socket**: `$XDG_RUNTIME_DIR/omg.sock` (fallback: `/tmp/omg.sock`)
- **History**: `~/.local/share/omg/history.json`

## Example: config.toml
```toml
shims_enabled = false
data_dir = "/home/you/.local/share/omg"
socket_path = "/run/user/1000/omg.sock"
default_shell = "zsh"
auto_update = false

[aur]
build_concurrency = 8
makeflags = "-j8"
pkgdest = "/home/you/.cache/omg/pkgdest"
srcdest = "/home/you/.cache/omg/srcdest"
cache_builds = true
enable_ccache = false
ccache_dir = "/home/you/.cache/ccache"
enable_sccache = false
sccache_dir = "/home/you/.cache/sccache"
```

## Security Policy
`policy.toml` controls what is allowed to install.

```toml
minimum_grade = "Verified"
allow_aur = false
require_pgp = true
allowed_licenses = ["AGPL-3.0-or-later", "Apache-2.0"]
banned_packages = ["example-bad-package"]
```

### Grades
- **LOCKED**: SLSA + PGP verified
- **VERIFIED**: PGP / checksum verified
- **COMMUNITY**: AUR / unsigned sources
- **RISK**: known vulnerabilities found

## Tips
- Keep `policy.toml` in version control for teams.
- Start permissive, then tighten policy with real-world usage data.
