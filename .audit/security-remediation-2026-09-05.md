# Stopped Daybreak scan remediation

Source revision: `289f65d93dcfe3b03b49ab6a2062d3aacdbc3d45`.
Remediation base: `0d33b188` (merged main). The scan was canceled; its retained findings are not an exhaustive audit.

## Plan

1. Preserve one disposition per reported entry and inspect current code.
2. Close privilege lookup, executable identity, and immutable package handoffs.
3. Bind all AUR outputs to reviewed source and isolate build capabilities.
4. Enforce installer provenance, bounded remote inputs, consistent vulnerability scores, and signer identity.
5. Close policy/recording gaps and disable unsupported pure-Debian mutation authority.
6. Run focused regressions, backend compilation checks, an independent bypass/regression review, and commit the verified patch.

## Finding ledger

| Entry | Finding ID | Claim | Disposition / evidence |
| --- | --- | --- | --- |
| 1 | `csf_37567b58a56fedc12df20fa7` | Quick install can accept attacker-replaced release binaries when `gh` is unavailable | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 2 | `csf_b581ed2dafd9999348b999be` | Privilege flows can invoke a PATH-shadowed `sudo` or root package-manager backend | Fixed: fixed system PATH, root-owned executable/ancestor validation, sanitized sudo/backend commands; privilege lookup regression tests. |
| 3 | `csf_992085138140ed7ae625c97b` | Reviewed Arch/AUR and Debian package archives can be swapped before privileged installation | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 4 | `csf_5744b8a167089766fbe2ecd2` | A compressed AUR metadata response can exhaust memory during index construction | Fixed: expanded JSON byte budget, record/field bounds, atomic index publication. Compressed oversized-field regression preserves prior index. |
| 5 | `csf_a9eac9f930e0c02fb6c45b94` | Turbo guidance grants the entire package-manager CLI passwordless root authority | Fixed: removed broad NOPASSWD package-manager configuration guidance; credential priming remains available. |
| 6 | `csf_63f859d75634568213c96858` | Writable AUR checkout lets sandboxed PKGBUILDs plant Git filters executed outside the sandbox | Fixed: refresh deletes build-tainted checkouts and clones anew without running Git inside their metadata. Real Git regression covers dirty sources and planted filter configuration. |
| 7 | `csf_73ecc21ec9f6b6da637d0c86` | Sandboxed AUR builds can reach host-local and private network services | Fixed: default bubblewrap unshares network; host prefetch pins public HTTPS addresses and rechecks every redirect; unsupported offline chroot fails closed. Explicit network opt-in documented. |
| 8 | `csf_d31ebadb7e0479db4193ddbb` | Pure Debian transactions can hang while retaining global dpkg locks | Fixed: removed duplicate acquisition of process mutex and dpkg locks; retained one lock pair through transaction completion. |
| 9 | `csf_af17e9f55684171f8880bd00` | AUR review can approve a benign PKGBUILD while an unseen install hook later runs as root | Fixed: full local source manifest review/verification and shared final output authorization for fresh, cached, dependency and rollback artifacts; exact hook bytes, identity, architecture and singleton metadata checks. |
| 10 | `csf_52349892861d6f97a5d1c99e` | Compromised runtime metadata can supply both malicious executables and matching digests | Trust boundary clarified: checksum and executable share an upstream publisher. No independent authenticity claim is made against compromise of that publisher. SECURITY.md documents this limitation; it is not a checksum implementation bypass. |
| 11 | `csf_07900505a0bb9a61a5de3eeb` | Auditing an artifact without `--certificate-identity` accepts any Sigstore signer as trusted | Fixed: missing/blank certificate identity fails before artifact or network access; existing cryptographic signer checks remain mandatory. |
| 12 | `csf_784071d94588b41057589e03` | The default user-writable OMG executable is reopened as the root payload | Fixed on Linux: sudo executes /proc/<live-parent>/exe, preserving the running inode across pathname replacement. Other Unix platforms require a root-controlled executable. Real root sudo integration unavailable on this host. |
| 13 | `csf_51c66e51af92550860a57670` | Security policy checks omit dependency closures and official updates | Fixed: policy re-evaluates complete prepared ALPM additions for installs/upgrades and dependencies; inherited policy survives elevation. Service updates apply source/grade/bans; final plan uses actual licenses. APT/DNF/Homebrew fail closed with explicit policies. |
| 14 | `csf_1d636ea61e2daa3ea80a796d` | The audit verifier accepts history rewritten and rehashed by the audited user | Corrected assurance boundary: verifier reports local chain consistency, not authenticated/comprehensive history. Root-controlled operation records provide a separate system collection; root and user-owner tampering limits are explicit. |
| 15 | `csf_6fd80840fd7bbab9cc9477e1` | Runtime metadata responses are buffered without a size or total-time limit | Fixed: runtime metadata body size and total request/body deadlines; wire-response tests cover declared and streamed overflow with valid JSON control. |
| 16 | `csf_121d82dfc388c3afbf71d685` | Runtime archives can exhaust inodes because extraction does not limit entry count | Fixed: archive entry-count, depth and cumulative path-byte budgets include skipped entries; extraction flood regression and existing path/link controls. |
| 17 | `csf_d0bce2a8d447b19bffac861e` | Release provenance is not bound to the requested version or release workflow | Fixed: exact release tag/source ref and release workflow identity required by installer and self-update; explicit requested tag mismatch rejected before extraction. |
| 18 | `csf_44d70220b6bdf775a7ef5227` | Running the documented installer without `gh` can install release-tampered OMG binaries | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 19 | `csf_ae09421485dd9fe9b144bc62` | Privilege elevation resolves `sudo` from the caller-controlled PATH | Fixed: fixed system PATH, root-owned executable/ancestor validation, sanitized sudo/backend commands; privilege lookup regression tests. |
| 20 | `csf_9b85f3512f112023567be662` | Fresh AUR archives are installed as root without being bound to the reviewed inputs | Fixed: full local source manifest review/verification and shared final output authorization for fresh, cached, dependency and rollback artifacts; exact hook bytes, identity, architecture and singleton metadata checks. |
| 21 | `csf_5eb293b87d3772bf605b32b8` | User-owned package archives can be replaced after approval but before root opens them | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 22 | `csf_2eeeba722ad67005ea1cf799` | Running the documented installer without `gh` can install release-tampered OMG binaries | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 23 | `csf_169c22a57a0456af2621d997` | Bootstrap installation can execute release assets without independent provenance verification | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 24 | `csf_d4221805c413204356fbd052` | A compromised AUR metadata response can exhaust client memory during index construction | Fixed: expanded JSON byte budget, record/field bounds, atomic index publication. Compressed oversized-field regression preserves prior index. |
| 25 | `csf_5d4d3f5bbacb7d1e40395b0b` | A writable ancestor can swap a reviewed local package before privileged installation | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 26 | `csf_e92c8aacf34fec9a5c125a12` | User-controlled PATH can select the package manager executed through sudo | Fixed: fixed system PATH, root-owned executable/ancestor validation, sanitized sudo/backend commands; privilege lookup regression tests. |
| 27 | `csf_7207fe5bc462cb44aa34d168` | AUR review can approve a benign PKGBUILD while an unseen install hook later runs as root | Fixed: full local source manifest review/verification and shared final output authorization for fresh, cached, dependency and rollback artifacts; exact hook bytes, identity, architecture and singleton metadata checks. |
| 28 | `csf_04c92fd1aad9fa254c874b2a` | Installing without GitHub CLI accepts release binaries without independent provenance | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 29 | `csf_91ae6920ca7cf37911d3ff38` | Privilege flows execute `sudo` from a user-writable PATH | Fixed: fixed system PATH, root-owned executable/ancestor validation, sanitized sudo/backend commands; privilege lookup regression tests. |
| 30 | `csf_2cf623bd3242c540e07d9ecb` | AUR and local Arch archives are mutable after validation and before root installation | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 31 | `csf_7a9118c47c08b1265a214627` | A validated local Debian archive can be replaced before privileged apt installs it | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 32 | `csf_a5f666fb2730fc4e57e660af` | Quick install can accept attacker-replaced release binaries when `gh` is unavailable | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 33 | `csf_2cb0237580a9414b62bcef96` | Package operations can invoke a PATH-shadowed `sudo` and expose elevation credentials | Fixed: fixed system PATH, root-owned executable/ancestor validation, sanitized sudo/backend commands; privilege lookup regression tests. |
| 34 | `csf_3265024024d30b0defd62d9f` | A local `.deb` can be swapped after approval and execute maintainer code as root | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 35 | `csf_a2aafb30ea6ce43065a181d2` | A compressed AUR metadata response can exhaust memory during indexing | Fixed: expanded JSON byte budget, record/field bounds, atomic index publication. Compressed oversized-field regression preserves prior index. |
| 36 | `csf_29c5d8fabdb45902f209b1fa` | A local peer can race a validated local package path into a root install | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 37 | `csf_bbf68da35750028b910dfefb` | Passing a dashboard token on the command line exposes account access to local observers | Fixed: removed positional dashboard token; stdin/environment input supported and token size bounded. Updated account-link guidance. |
| 38 | `csf_51fa17111b8c00429c29aef4` | The documented bootstrap executes mutable main-branch code before authentication | Trust boundary clarified: initial bootstrap is executable publisher-controlled code. Documented reviewed commit checkout workflow and no retroactive bootstrap-authentication claim; mutable-main download still requires publisher/channel trust. |
| 39 | `csf_e0f3c1a7a9e24059bb082924` | The installer does not require release-bound provenance for downloaded executables | Fixed: exact release tag/source ref and release workflow identity required by installer and self-update; explicit requested tag mismatch rejected before extraction. |
| 40 | `csf_2073282cde8daf219aa25856` | Runtime installers trust checksums from the same mutable origin as executable content | Trust boundary clarified: checksum and executable share an upstream publisher. No independent authenticity claim is made against compromise of that publisher. SECURITY.md documents this limitation; it is not a checksum implementation bypass. |
| 41 | `csf_fffa65014a90e7bec7d78cf8` | Package policy does not cover complete transaction plans or all mutation entry points | Fixed: policy re-evaluates complete prepared ALPM additions for installs/upgrades and dependencies; inherited policy survives elevation. Service updates apply source/grade/bans; final plan uses actual licenses. APT/DNF/Homebrew fail closed with explicit policies. |
| 42 | `csf_9aa7957b99b20616da372b8a` | Package and privileged operations are absent from the tamper-evident audit log | Fixed: synchronous operation attempt/outcome records at ALPM/APT/DNF/Homebrew execution seams and privileged native launch; service/history completion records retained even without parent-owned history. |
| 43 | `csf_c70a3d3b4a67d3e7d42ff5da` | Forgeable re-exec markers bypass privileged-child policy, consent, and history controls | Defense in depth: markers require root and no longer bypass backend final-plan policy or durable operation recording. They are coordination metadata, not an authentication boundary against an already-authorized root caller. |
| 44 | `csf_7d402a4cac362b26f95a640a` | The piped installer can execute an ambient working directory as source | Fixed: source builds require explicit --from-source and an actual script checkout; piped/failed-release paths never use ambient Cargo source. Installer fixture covers hostile working directory. |
| 45 | `csf_00939f64e4eb8e51f5637d75` | OSV CVSS vectors are discarded before severity classification and remediation | Fixed: CVSS numeric and vector scores normalize consistently; highest valid severity retained for classification and remediation. Vector/invalid score regressions. |
| 46 | `csf_bbdf4856a313e4cb6d4cbd82` | Pure Debian transactions deterministically deadlock while holding dpkg locks | Fixed: removed duplicate acquisition of process mutex and dpkg locks; retained one lock pair through transaction completion. |
| 47 | `csf_0e78ce7288e2bcb38f5a3a71` | OSV advisory text is rendered without terminal-control sanitization | Fixed: OSV package/advisory IDs, summaries and score displays use terminal-control sanitization. |
| 48 | `csf_d8ad0f1cba6a80a7e74aae84` | Package policy does not cover complete transaction plans or all mutation entry points | Fixed: policy re-evaluates complete prepared ALPM additions for installs/upgrades and dependencies; inherited policy survives elevation. Service updates apply source/grade/bans; final plan uses actual licenses. APT/DNF/Homebrew fail closed with explicit policies. |
| 49 | `csf_6288cc1cc69d3734c4010098` | Official package updates bypass the configured security policy | Fixed: policy re-evaluates complete prepared ALPM additions for installs/upgrades and dependencies; inherited policy survives elevation. Service updates apply source/grade/bans; final plan uses actual licenses. APT/DNF/Homebrew fail closed with explicit policies. |
| 50 | `csf_41e540ac69f2f543e8aeeee1` | A local peer can race a configured AUR archive path into a root package install | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 51 | `csf_f017083bc1b9c59ac9c8b2fc` | Update routes can install packages that the configured security policy would block | Fixed: policy re-evaluates complete prepared ALPM additions for installs/upgrades and dependencies; inherited policy survives elevation. Service updates apply source/grade/bans; final plan uses actual licenses. APT/DNF/Homebrew fail closed with explicit policies. |
| 52 | `csf_7feb733db931a7362126af91` | Installing, removing, or updating packages leaves the security audit log incomplete | Fixed: synchronous operation attempt/outcome records at ALPM/APT/DNF/Homebrew execution seams and privileged native launch; service/history completion records retained even without parent-owned history. |
| 53 | `csf_99dad0c6ab0bc61f6d1054a0` | Rewriting the audit file can produce a forged history that still verifies as intact | Corrected assurance boundary: verifier reports local chain consistency, not authenticated/comprehensive history. Root-controlled operation records provide a separate system collection; root and user-owner tampering limits are explicit. |
| 54 | `csf_d883d0c13f54391f286e0b7d` | A local package archive can be replaced after validation but before privileged installation | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 55 | `csf_81cd147e93a5fc27351aed64` | Fresh installs accept release binaries without provenance when GitHub CLI is absent | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 56 | `csf_56f233c6d59870a3fa32351b` | A same-user client can force daemon security events out of the durable audit log | Fixed: queue/persistence failure writes a durable incompleteness marker; verification rejects affected collections. Marker cannot authenticate history against its owner; limitation documented. |
| 57 | `csf_2db2cd1f58dc54f885362d85` | Unsafe AUR builds run while a reusable global sudo credential is kept alive | Fixed: explicit unsafe native build uses trusted setpriv --no-new-privs and setsid; kernel NoNewPrivs regression confirms the boundary against cached global sudo tickets. |
| 58 | `csf_1d3e1566c384dd8cc688a62a` | Running an official update installs packages that the security policy would block | Fixed: policy re-evaluates complete prepared ALPM additions for installs/upgrades and dependencies; inherited policy survives elevation. Service updates apply source/grade/bans; final plan uses actual licenses. APT/DNF/Homebrew fail closed with explicit policies. |
| 59 | `csf_8cde677f74b891284c78c0ab` | Quick install can accept attacker-replaced release binaries when `gh` is unavailable | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 60 | `csf_fb58d59084d0f5c3d9ed3527` | Package operations can invoke a PATH-shadowed `sudo` and expose elevation credentials | Fixed: fixed system PATH, root-owned executable/ancestor validation, sanitized sudo/backend commands; privilege lookup regression tests. |
| 61 | `csf_589d1444dbde3a45005af682` | A reviewed local package can be swapped before privileged installation | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 62 | `csf_f96783d2efba94e7748e059f` | Quick install can accept attacker-replaced release binaries when `gh` is unavailable | Fixed: installer requires successful release-bound GitHub attestation; missing gh fails closed. Executed installer transport fixtures cover absent/rejected/accepted provenance. |
| 63 | `csf_fa77e13d5df99977bff8c06c` | Package operations can invoke a PATH-shadowed `sudo` and expose elevation credentials | Fixed: fixed system PATH, root-owned executable/ancestor validation, sanitized sudo/backend commands; privilege lookup regression tests. |
| 64 | `csf_0235d994796383bd100cc8b0` | A reviewed local package can be swapped before privileged installation | Fixed: sealed archive/signature snapshots retained through approval/elevation; root-owned staging retained through metadata validation and transaction commit. Replacement and in-place write regressions. |
| 65 | `csf_246db346e9d5432a7ea82886` | A root pure-Debian API call can install code selected by the sudo user’s cache | Fixed by fail-closed boundary: production pure-Debian mutations disabled at manager and underlying transaction executor until repository authority is authenticated; native APT remains available. |

## Independent review

The required read-only review identified five residual issues: private-address
prefetch/chroot networking, duplicate archive singleton metadata, cached artifacts
using fresh-build version relaxation, update license precheck rejection, and
source-manifest additions. All five were addressed before final verification.
No stopped scan artifacts or workbench finding states were rewritten.

## Verification

- Combined security/boundary unit suite: **349 passed, 0 failed, 1 ignored**.
  Filters covered security, privilege, HTTP, AUR client/index/source downloads,
  runtime archive handling, package service, history, self-update and install CLI.
- `cargo test --test security_privilege_escalation_tests --no-default-features
  --features arch,license,pgp -- --test-threads=2`: **51 passed**.
- Pure-Debian `dpkg_lock_files_are_created_and_locked` and
  `cancelled_configuration_keeps_locks_until_rollback_finishes`: **2 passed**.
- `cargo check --all-targets --no-default-features --features <backend>,license,pgp`:
  **passed separately for `arch`, `fedora`, `debian`, `debian-pure`, and `macos`**.
  The Homebrew/macOS feature was checked on Linux, not on an Apple host.
  Native APT C++ compilation used isolated Debian 3.0.3 headers in
  `/tmp/omg-apt-check/usr/include`; no host packages were installed.
- `bash -n install.sh` and `bash tests/installer_security.sh`: **passed**.
- `cargo fmt --all -- --check` and `git diff --check`: **passed**.
- A compiled, benign Linux fixture replaced its executable pathname while live,
  then forked and executed `/proc/<live-parent>/exe`: **original inode executed**.
  Actual root sudo integration was unavailable (no cached credentials; Docker
  socket access also unavailable). No host package transactions were executed.

Rust commands used `RUSTC_WRAPPER='' TMPDIR=/tmp` to avoid an unrelated stale
shared sccache temporary directory. The single ignored test is the existing
`cancelling_logged_sandbox_build_terminates_descendants`, which requires bubblewrap
and permission to create PID namespaces. The two pure-Debian lock tests used
isolated fixture paths, not the host dpkg database.

This remediation is not a completed rescan: the source scan was canceled with
incomplete coverage. Findings about compromise of an upstream publisher or the
initial bootstrap trust root are documented trust limitations, not claims of
new independent provenance. Local audit hash consistency cannot authenticate
history against the file owner or the system administrator.
