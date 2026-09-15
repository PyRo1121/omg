# QEMU assurance completion implementation plan

> Execute sequentially in this task; the user explicitly prohibits subagents. Use the existing isolated checkout and retain unrelated work. The user authorized fixing every concrete gap before release.

**Goal:** Close the concrete release-assurance gaps in the primary-source QEMU assessment, then publish and verify the documented release with terminal update notices.

**Architecture:** Keep the existing controller, inventory executor and exporter. Add small, explicit admission and evidence helpers. The host validates complete evidence; guest faults run only in disposable guests; reporting consumes data and never executes downloaded artifacts.

**Tech stack:** GitHub Actions, Python standard library, Bash, Debian package verification, QEMU/KVM, existing Sentry reporting.

**Spec:** `.audit/qemu-security-research-2026-09-14.md` in the original OMG checkout. Sources and confirmed limitations are recorded there.

## Constraints

- Security before speed; never weaken verification or convert required failures to skips.
- No raw dumps, secrets, guest keys or arbitrary filesystem traversal in published evidence.
- No host fault injection, root QEMU fallback or TCG fallback.
- Keep architecture-specific controller pins and the QEMU package floor `1:10.0.11+ds-0+deb13u1`.
- Preserve explicit supported/unsupported platform distinctions. No claim of universal bug detection or formal SLSA certification.
- Keep v0.1.221 unpublished until all new required checks pass.

## 1. Controller and release-routing corrections

- [x] Resolve staged artifact mode once and use it at download and execution.
- [x] Ensure every exact-commit main prerequisite can run.
- [x] Verify official controller manifests and pin both architectures.
- [x] Enforce patched installed package versions before image parsing.
- [x] Migrate to QEMU 10's supported privilege-drop option.
- [x] Pass published-artifact QEMU on all four x86_64 guests (run 34919650424).
- [ ] Pass latest candidate PR and exact merged-main gates.

## 2. Complete setup-failure evidence

Files: `scripts/ci-smoke-report.py`, `scripts/test_ci_smoke_report.py`, `.github/workflows/qemu-matrix.yml`, workflow-boundary fixtures.

- [x] Reproduce a failed setup before lifecycle result creation.
- [x] Emit a bounded controller-owned HARNESS_ERROR receipt, preserving original failure.
- [x] Include a reporting delivery receipt for the existing verifier.
- [x] Reuse an existing lifecycle report instead of creating duplicate failure identities.
- [x] Test failure/cancellation/success, absent configuration, malformed evidence and duplicate suppression.

## 3. Crash and final-health admission

Files: new `scripts/check-qemu-health.py`, tests, `scripts/benchmark-qemu.sh`, exporter allowlist.

- [x] Define fatal kernel/process signatures and structured final-health inputs.
- [x] Test clean logs, crash-signature fixtures, missing/truncated/oversized inputs, and benign mentions.
- [x] Collect bounded boot-scoped kernel and OMG/omgd crash summaries, not raw cores or environment data.
- [x] Require live guest and controller health after test execution, and preserve crash evidence on failure.
- [x] Prove injected serial-crash/OOM/missing-health evidence cannot produce a green lifecycle fixture. All four real guests passed health admission in run 34921961271.

## 4. Explicit required-case and skip policy

Files: new reviewed JSON policy, admission helper/tests, inventory runner and workflow summary.

- [x] Enumerate required cases and allowed skips for staged/published profiles and each distro.
- [x] Report selected/executed/pass/fail/blocked/skip counts independently.
- [x] Reject missing/duplicate/substituted rows, unexpected skips, empty execution and incomplete summaries.
- [x] Make additions/removals/profile changes visible in policy diffs; keep unsupported scenarios explicit. Published v0.1.220 and current v0.1.221 inventories have separate pinned identities.

## 5. Disposable transaction fault tests

Files: new guest fault suite and tests, QEMU driver, required-case policy, exporter.

- [x] Enable the existing independent reset transaction trials with one sample per tool/operation. Debian and Ubuntu passed in run 34923934513; Fedora trials passed but final serial parsing exposed a torn UTF-8 character, fixed with a real-evidence regression.
- [x] Implement deterministic ENOSPC, read-only and fsync-interruption cases confined to guest-owned temporary storage. Hosted validation pending.
- [x] Require fault activation and post-failure export recovery/integrity, not merely a nonzero exit. Hosted validation pending.
- [ ] Add bounded I/O-error/power-loss cases only with verified disposable disk ownership and positive controls.
- [ ] Validate all four guests; classify unavailable required capabilities as blocked, never passed.

## 6. Resource and egress confinement

Files: controller setup, network policy/fixtures, diagnostic metadata.

- [ ] Record controller OOM/exit state before teardown and bound process/log/storage resources.
- [x] Separate offline phases from explicitly networked tests using reviewed per-case scope. Run 34923408618 exposed mislabeled legacy tiers; preserve expectations and correct scope instead of skipping cases.
- [ ] Build destination policy from observed official endpoints and required runtime repositories.
- [x] Implement controller firewall blocking private/metadata/runner destinations and unneeded outbound ports, with a positive reject-counter probe. Hosted validation pending.
- [ ] Test forbidden destinations and legitimate package retrieval; never expose host credentials to the guest.

## 7. Signed image renewal and dependency maintenance

Files: versioned pin metadata, refresh/verification helper/tests, scheduled validation workflow.

- [x] Verify publishers' signed checksum metadata against pinned trust roots where available. All four actual x64 images passed in run 34922660853, including Arch detached image verification.
- [x] Record per-image publisher, digest, signature provenance and review expiry.
- [x] Document the review PR procedure and implement daily expiry warnings without automatically changing pins or trusting new keys. Activation requires merge.
- [x] Enforce explicit handling where a publisher does not supply equivalent signatures. Debian cloud-image exception follows Debian's own documentation.
- [ ] Monitor controller package advisories and ensure manifest/version receipts remain available.

## 8. Trusted failure issue reporting

Files: trusted `workflow_run` reporter and bounded JSON parser/tests.

- [x] Implement push/manual/nightly reporting. Daybreak commit 3f2d005e explicitly excludes PR runs from privileged reporting and limits case identities to default-branch policy. Activation and delivery still require merging the default-branch workflow.
- [x] Bind artifact retrieval to run ID, repository, attempt and commit; never check out PR code in the privileged reporter.
- [x] Include observed failure, affected case/distro, stage, exit/signal, run/evidence links and bounded diagnostic context.
- [x] Deduplicate recurring cases and close only from later matching current-main passing evidence.
- [x] Test artifact poisoning, malformed/oversized data and unsupported event identities; implement no-result fallback.

## 9. Repository enforcement

Files/state: GitHub branch/tag rules and documented policy receipts.

- [x] Preserve solo-maintainer policy: zero external approvals; PR and checks required.
- [x] Require PR/check gates on main without path-filter deadlocks.
- [x] Prevent force-push/deletion of main and mutation/deletion of release tags.
- [x] Verify active rulesets 23399194/23399197 through the API; retain recovery documentation. Tag creation now waits for all independent prerequisites before consuming an immutable version.

## 10. Integrate, document and release

- [ ] Update the research findings table with implementation and hosted proof links.
- [ ] Update release notes with every correction and all commits since v0.1.220.
- [ ] Review the final diff independently, without subagents, for bypasses and compatibility regressions.
- [ ] Pass focused fixtures, all required hosted checks, PR QEMU and exact-main QEMU.
- [ ] Merge using a Conventional Commit title; close #420 only with main-push evidence.
- [ ] Publish v0.1.221, verify all archive checksums/attestations and canonical update marker.
- [ ] Run published release smoke and QEMU; report released/verified state and any genuinely external blocker separately.
