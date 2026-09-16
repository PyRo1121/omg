# CI Streamlining Implementation Plan

**Goal:** Reduce unnecessary compilation while preserving exact-commit release gates and QEMU coverage.
**Architecture:** Conservative documentation-only classification for PRs, with always-running aggregate checks. Restore Quick Gate's actual Cargo output directory. Record compiler timing artifacts for the existing QEMU builds before consolidating distinct test configurations.
**Spec:** Exa research approved in this task; baseline main 58e7e15b.
**Constraints:** GitHub runners only; no test removal, privilege changes, or release-gate bypass. Work inline without subagents.

- [ ] Add and test a fail-closed PR classifier using NUL-separated Git paths; renames include old and new paths. Only README.md, CHANGELOG.md and Markdown under docs/ are documentation. Non-PR events always require full checks.
- [ ] Use classification in CI and Coverage. Required coverage aggregate fails if classification fails or required execution does not succeed.
- [ ] Point Quick Gate's CI_LOCAL_TARGET_DIR at the target directory restored by rust-cache; retain the local developer default.
- [ ] Add Cargo --timings and bounded, per-distro timing artifacts to staged QEMU builds without changing tests or compiler profiles.
- [ ] Run classifier, CI gate, release-boundary and QEMU workflow regressions, inspect the diff, then push a conventionally named PR and inspect hosted checks.

Follow-up batches: prove equivalent test configurations before deduplication; per-distro build/guest dependencies; digest-verified base-image caching and prebuilt controller. Keep these separate from this change so regressions remain attributable.
