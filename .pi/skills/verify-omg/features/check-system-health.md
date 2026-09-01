# Check system health

Users see concise OMG system status or run a fuller doctor that checks distro support, network reachability, required tools, daemon state, PATH setup, and shell integration.

## Sub-features

- `health-status`: show a concise system-status summary.
- `health-status-fast`: show the fast status path without starting services.
- `health-doctor`: run all default health checks and summarize actionable issues.
- `health-network`: include explicit network checks when requested.

## How to get to it (user POV)

- Run `omg status` or `omg status --fast` for a status overview.
- Run `omg doctor` for the standard health report.
- Run `omg doctor --network` for network-focused checks.

## Driving it with verify-omg

Preconditions: the helper doctor passes. The disposable state intentionally has no daemon and its binary directory is not added to PATH.

- Fast status: `./.pi/skills/verify-omg/bin/verify-omg drive health-status-fast -- status --fast`; require exit zero and an OMG/system identity plus status fields.
- Full status: `./.pi/skills/verify-omg/bin/verify-omg drive health-status -- status`; require exit zero and a broader status report than the fast path.
- Doctor report: `./.pi/skills/verify-omg/bin/verify-omg drive health-doctor -- doctor`; require exit zero, Arch Linux detection, dependency checks, and a final issue summary.
- Network doctor: `./.pi/skills/verify-omg/bin/verify-omg drive health-network -- doctor --network`; require an explicit connectivity result. Record environmental network failure as blocked rather than rewriting the expected result.

## Gotchas

- `Daemon is not running` and `OMG bin directory not in PATH` are expected in the disposable verification environment.
- Doctor warnings are user-visible findings and can coexist with exit zero; assert the summary text as well as status.
- Do not launch the daemon to silence its expected warning.
- Network checks can fail because of the environment rather than OMG; retain the transcript and report the precondition.
