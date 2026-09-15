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

- [ ] Reproduce a failed setup before lifecycle result creation.
- [ ] Emit a bounded controller-owned HARNESS_ERROR receipt, preserving original failure.
- [ ] Include a reporting delivery receipt for the existing verifier.
- [ ] Reuse an existing lifecycle report instead of creating duplicate failure identities.
- [ ] Test failure/cancellation/success, absent configuration, malformed evidence and duplicate suppression.

## 3. Crash and final-health admission

Files: new `scripts/check-qemu-health.py`, tests, `scripts/benchmark-qemu.sh`, exporter allowlist.

- [ ] Define fatal kernel/process signatures and structured final-health inputs.
- [ ] Test clean logs, actual crash fixtures, missing/truncated/oversized inputs, and benign mentions.
- [ ] Collect bounded boot-scoped kernel and OMG/omgd crash summaries, not raw cores or environment data.
- [ ] Require live guest and controller health after test execution, and preserve crash evidence on failure.
- [ ] Prove an injected crash cannot produce a green release selection.

## 4. Explicit required-case and skip policy

Files: new reviewed JSON policy, admission helper/tests, inventory runner and workflow summary.

- [ ] Enumerate required cases and allowed skips for staged/published profiles and each distro.
- [ ] Report selected/executed/pass/fail/blocked/skip counts independently.
- [ ] Reject missing/duplicate/substituted rows, unexpected skips, empty execution and incomplete summaries.
- [ ] Make additions/removals/profile changes visible in policy diffs; keep unsupported scenarios explicit.

## 5. Disposable transaction fault tests

Files: new guest fault suite and tests, QEMU driver, required-case policy, exporter.

- [ ] Enable the existing independent reset transaction trials in the release profile with a bounded sample count.
- [ ] Add deterministic ENOSPC, read-only and interruption cases confined to guest-owned temporary storage.
- [ ] Verify fault activation and post-failure recovery/integrity, not merely a nonzero exit.
- [ ] Add bounded I/O-error/power-loss cases only with verified disposable disk ownership and positive controls.
- [ ] Validate all four guests; classify unavailable required capabilities as blocked, never passed.

## 6. Resource and egress confinement

Files: controller setup, network policy/fixtures, diagnostic metadata.

- [ ] Record controller OOM/exit state before teardown and bound process/log/storage resources.
- [ ] Separate offline/hermetic phases from explicitly networked tests.
- [ ] Build destination policy from observed official endpoints and required runtime repositories.
- [ ] Block private/metadata destinations and unneeded outbound traffic without breaking required package operations.
- [ ] Test forbidden destinations and legitimate package retrieval; never expose host credentials to the guest.

## 7. Signed image renewal and dependency maintenance

Files: versioned pin metadata, refresh/verification helper/tests, scheduled validation workflow.

- [ ] Verify publishers' signed checksum metadata against pinned trust roots where available.
- [ ] Record per-image publisher, date, digest, signature provenance and review expiry.
- [ ] Generate reviewable pin updates without automatically merging or trusting unsigned metadata.
- [ ] Enforce explicit handling where a publisher does not supply equivalent signatures.
- [ ] Monitor controller package advisories and ensure manifest/version receipts remain available.

## 8. Trusted failure issue reporting

Files: trusted `workflow_run` reporter and bounded JSON parser/tests.

- [ ] Report push/manual/nightly and PR failures without granting issue-write or telemetry secrets to PR execution.
- [ ] Bind artifact retrieval to run ID, repository, attempt and commit; never check out PR code in the privileged reporter.
- [ ] Include observed failure, affected case/distro, stage, exit/signal, run/evidence links and bounded diagnostic context.
- [ ] Deduplicate recurring cases and close only from later matching passing evidence.
- [ ] Test artifact poisoning, malformed/oversized data, unsupported event identities and no-result failures.

## 9. Repository enforcement

Files/state: GitHub branch/tag rules and documented policy receipts.

- [ ] Resolve solo-maintainer versus second-reviewer policy (question pending).
- [ ] Require PR/check gates on main without path-filter deadlocks.
- [ ] Prevent force-push/deletion of main and mutation/deletion of release tags.
- [ ] Verify rules through the API and dry policy evaluation where available; retain recovery documentation.

## 10. Integrate, document and release

- [ ] Update the research findings table with implementation and hosted proof links.
- [ ] Update release notes with every correction and all commits since v0.1.220.
- [ ] Review the final diff independently, without subagents, for bypasses and compatibility regressions.
- [ ] Pass focused fixtures, all required hosted checks, PR QEMU and exact-main QEMU.
- [ ] Merge using a Conventional Commit title; close #420 only with main-push evidence.
- [ ] Publish v0.1.221, verify all archive checksums/attestations and canonical update marker.
- [ ] Run published release smoke and QEMU; report released/verified state and any genuinely external blocker separately.
