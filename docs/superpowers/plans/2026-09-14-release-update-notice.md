# Release and terminal update notice

Prepare 0.1.221 from the verified main branch, documenting every commit since
0.1.220 and the security, mise, AUR and operator compatibility changes.

The existing interactive Bash, Zsh and Fish integration will invoke a private
notice command once at startup. It reads only a bounded local cache, displays a
strictly parsed newer stable version at most once per day, and starts a detached
three-second background refresh when the daily check is due. The first result
appears on a subsequent terminal opening. No installation occurs automatically.
Root, CI, noninteractive sessions and OMG_NO_UPDATE_CHECK=1 remain silent.

Security boundaries: reuse the self-updater's fixed HTTPS marker and bounded
semantic-version parser; never render remote text as shell code; refuse symlink,
hardlink, shared-owner or oversized cache files; anchor cache lookup to directory
descriptors; acquire a nonblocking file lock to avoid startup stalls and duplicate
requests. A failed refresh is throttled and does not fail the shell.

Verification: cache policy and hostile-file tests, shell-hook syntax/invocation
tests, format/clippy and platform CI, all release prerequisites on the exact
release revision, attested publication and post-release smoke/QEMU verification.
