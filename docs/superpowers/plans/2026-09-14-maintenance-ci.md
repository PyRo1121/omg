# Review and improve the complete OMG delivery pipeline

Scope: every OMG and OMG-web workflow, supporting scripts, QEMU controller and
guest lifecycle, reporting, release provenance, and deployment. Passing jobs
are in scope. Preserve concurrent security work and perform this work without
subagents. The maintenance fixes below are only the first work package.

## Review and implementation sequence

1. Inventory triggers, permissions, toolchain and container pins, cache keys,
   artifacts, dependencies, timeouts, and required checks for every workflow.
   Record the baseline run IDs, tested commits, job durations, and failures.
2. Research current primary documentation from GitHub Actions, QEMU, Rust,
   nextest, git-cliff, SvelteKit, Alchemy, and Cloudflare. Record exact sources
   and distinguish current documentation from versions installed by the jobs.
3. Trace QEMU from release selection through download verification, controller
   setup, image validation, guest boot/reboot, lifecycle and inventory tests,
   cancellation, evidence export, Sentry, and issue filing. Check failure paths
   and missing-report handling as carefully as successful execution.
4. Build a coverage matrix before removing duplication. Different backends,
   root/non-root behavior, coverage instrumentation, and packaged binaries
   are distinct coverage. Prefer compatible compiled-output reuse over deleting
   useful assertions. Keep cache trust separate from artifact provenance.
5. Repair confirmed failures and privilege gaps with focused regressions.
   Improve cache freshness, dependencies, and filters where evidence supports
   them. Require unchanged coverage and observable skipped-work decisions.
6. Inspect release gates for the exact source revision and ensure security and
   staged guest evidence are required before publication. Validate attestations,
   installer synchronization, R2 round trips, and the latest-version marker.
7. Run focused checks, hosted full CI, staged QEMU, maintenance campaigns, and
   website checks. Review logs and per-case artifacts, including rerun attempts.
   Compare cold/warm durations without claiming universal speed superiority.
8. Merge passing changes with conventional PR titles, release the security fixes,
   verify published artifacts and installed-version smoke checks, and close only
   issues whose resolution is supported by a linked PR and passing evidence.

## Initial evidence and open review items

- Security PR #414 at b6a967ac5f91be82d21e0eb48d39e3e423c71bdd is failing
  across platforms. CI run 34901546495 reports the dangling-destination-symlink
  test expects a preview but passes an absolute path now rejected by policy.
  Inspect the fixture and production contract before deciding the correction.
- Release Smoke configures the Sentry secret in both PR execution jobs. Apply
  the same PR exclusion already used in QEMU; test that scheduled reporting stays.
- QEMU staged builds cache downloads but not compiled outputs. Evaluate explicit
  distro/architecture/toolchain/profile keys and bounded refresh rather than
  sharing incompatible or untrusted release outputs.
- Release currently gates publication on CI and Benchmark only. Determine how
  to require complete security and guest coverage on the release revision without
  waiting on unrelated historical PR merge revisions or introducing a cycle.
- The controller launches QEMU as container root. Container isolation and KVM
  ACLs are useful but do not meet QEMU's recommended non-root process model.
  Test a narrowly scoped runtime user and seccomp compatibility before rollout.

## Primary research sources

Reviewed online on 2026-09-14; this is a working source register, not a claim
that the comprehensive review is finished.

- GitHub, [Secure use reference](https://docs.github.com/en/actions/reference/security/secure-use):
  minimum token permissions, immutable action pins, untrusted checkout boundaries.
- GitHub, [Dependency caching reference](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching):
  branch/PR cache scope, key matching, and cache access controls. Check support
  before adopting recently documented workflow syntax.
- GitHub, [Artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations):
  signed provenance and consumer verification requirements.
- QEMU, [Security](https://www.qemu.org/docs/master/system/security.html):
  non-root processes, least privilege, resource limits, and seccomp. Master docs
  identify version 11.1.50; verify options against the controller's actual QEMU.
- Rust Fuzz Book, [Setup](https://rust-fuzz.github.io/book/cargo-fuzz/setup.html):
  nightly sanitizer requirements.
- git-cliff, [Remote configuration](https://git-cliff.org/docs/configuration/remote/):
  offline generation when remote metadata is unnecessary.
- GitHub, [CodeQL build modes](https://docs.github.com/en/code-security/concepts/code-scanning/codeql/codeql-for-compiled-languages)
  and [supported languages](https://codeql.github.com/docs/codeql-overview/supported-languages-and-frameworks/):
  Rust supports build mode none; GitHub Actions has its own extractor and queries.
  Baseline run 34901546491 spent 370 seconds building release binaries before
  the Rust extractor ran independently in the analysis step. Remove that build,
  retain the pinned Rust installation and existing Rust queries, and compare
  extraction diagnostics on the hosted replacement. Add a separate Actions scan
  with its own category and include all workflow/action changes in trigger paths.

## Maintenance repairs already prepared

Observed failures: changelog generation panics on an unauthenticated GitHub API
403; scheduled fuzzing invokes stable Rust with nightly sanitizer flags and
selects a musl target from the prebuilt cargo-fuzz executable.

1. Keep changelog generation offline in cliff.toml because its templates use
   local commit/tag data and literal links, not remote metadata. This avoids
   editing the changelog workflow while Daybreak changes its privilege boundary.
2. Select the installed pinned nightly explicitly for fuzz dependency resolution
   and execution, and select the hosted runner's GNU target explicitly.
3. Add focused regressions, run actual changelog generation and a bounded hosted
   fuzz campaign, then merge after the relevant checks pass.
4. Independently resolve the companion site's adapter compatibility failure and
   verify its build and CI. Retry the main site install only for its documented
   transient registry connection reset.
