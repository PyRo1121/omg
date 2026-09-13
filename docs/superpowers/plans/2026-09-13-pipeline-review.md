# QEMU and CI/CD review and verification

User authorization: scrutinize and improve the pipeline before running the
pending OMG security patches; use five reviewers at medium reasoning where
the runtime permits. Preserve existing source fixes and audit evidence.

## Review domains

1. QEMU orchestration: selection, staged revision, test coverage, evidence,
   artifact identity and failure propagation.
2. Reporting: configure the existing Sentry reporter securely, cover early
   failures, retain delivery evidence, and preserve original test status.
3. OMG CI: required checks, platform and security regression coverage,
   permissions and false-success paths.
4. Release and smoke: release gates, artifact provenance, isolation, pinning,
   smoke coverage and failure handling.
5. OMG-Web CI/CD: checks before deployment, environment boundaries,
   revision identity, permissions and observability.

Parent review additionally traces QEMU guest lifecycle and inventory scripts.
Agents own disjoint files; overlapping integration stays with the parent.

## Implementation and validation

- Record concrete findings with file evidence before changing behavior.
- Fix observed gaps within existing scripts/workflows; retain pinned images,
  action revisions, KVM requirements and explicit mutation boundaries.
- Add focused regression checks for changed orchestration/reporting behavior.
- Run Bash syntax, Python/workflow checks and available fixture suites locally.
- Review final diffs and map remaining security regression tests to actual jobs.
- Put reviewable changes on a dedicated branch and run staged QEMU plus CI
  against that exact revision after pipeline improvements are complete.
- Collect job conclusions, per-case evidence, skips and Sentry delivery logs.
  A green lifecycle run alone does not prove every security regression passed.
- Do not merge, publish releases or deploy production as part of verification.

## Known starting gaps

- QEMU artifact upload uses an unset Actions `env.HOME` and uploads no evidence.
- QEMU never configures the existing Sentry reporter despite an available DSN
  secret. Pre-harness and build failures do not reach cleanup reporting.
- QEMU staged builds do not run lib/bin unit tests.
- Local patches are uncommitted: published-release runs cannot verify them.

Findings, corrections and execution results will be recorded under
`security-review-2026-09-13/pipeline-*.md` and the remediation report.
