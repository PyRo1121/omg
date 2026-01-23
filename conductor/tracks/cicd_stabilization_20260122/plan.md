# Implementation Plan - CI/CD Stabilization and Code Quality

## Phase 1: Workflow Analysis & Quick Fixes [checkpoint: 227d2ad]
- [x] Task: Audit existing GitHub Actions workflows
    - [x] Analyze `ci.yml`, `test-matrix.yml`, `benchmark.yml`, and `release.yml` for recent failures.
    - [x] Identify and document specific failure points and flaky tests.
        - **Findings:**
            - `Qual & Sec`: Typos in `cargo_tree_debian.txt`.
            - `Qual & Sec`: Clippy warnings in `src/cli/enterprise.rs` (unused mut) and `src/cli/packages/search.rs` (unused vars).
            - `Ubuntu/Debian/Arch Builds`: Unreachable code in `src/cli/packages/explicit.rs`.
- [x] Task: Stabilize core CI/Test Matrix
    - [x] Fix typo errors in `cargo_tree_debian.txt`.
    - [x] Fix clippy warnings in `src/cli/enterprise.rs` and `src/cli/packages/search.rs`.
    - [x] Fix unreachable code in `src/cli/packages/explicit.rs`.
    - [x] Fix immediate errors preventing `ci.yml` and `test-matrix.yml` from passing.
    - [x] Implement retry logic or fix race conditions in flaky tests identified during audit.
    - Commit: 20e90e5
- [x] Task: Conductor - User Manual Verification 'Phase 1: Workflow Analysis & Quick Fixes' (Protocol in workflow.md)

## Phase 2: Enhanced Quality Gates
- [x] Task: Integrate `cargo-audit` into CI
    - [x] Create a new workflow or job to run `cargo audit`.
    - [x] Configure it to fail on vulnerability findings.
- [ ] Task: Enforce Code Coverage
    - [ ] Update `ci.yml` to generate coverage reports (e.g., using `tarpaulin`).
    - [ ] Add a step to fail the build if coverage is below 80%.
- [ ] Task: Strict Linting Enforcement
    - [ ] Update CI to run `cargo clippy --all-targets --all-features -- -D warnings`.
    - [ ] Ensure `cargo fmt --check` is running and enforced.
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Enhanced Quality Gates' (Protocol in workflow.md)

## Phase 3: Production-Readiness & Stub Implementation
- [ ] Task: Codebase Audit for Stubs
    - [ ] Search for `TODO`, `FIXME`, `unimplemented!()`, and stubbed functions.
    - [ ] Create a prioritized list of incomplete features.
- [ ] Task: Implement Stubbed Features (Iterative)
    - [ ] For each identified stub, write failing tests (Red).
    - [ ] Implement the missing functionality (Green).
    - [ ] Refactor and ensure `clippy` compliance.
- [ ] Task: Resolve Clippy Warnings
    - [ ] systematic pass to fix all `clippy` warnings in the codebase.
    - [ ] Enable `clippy::pedantic` locally and address high-value suggestions.
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Production-Readiness & Stub Implementation' (Protocol in workflow.md)

## Phase 4: Release & Benchmark Stabilization
- [ ] Task: Fix Benchmark Workflow
    - [ ] Update `benchmark.yml` to run reliably.
    - [ ] Ensure it reports results correctly without failing the pipeline on minor regressions (unless critical).
- [ ] Task: Verify Release Workflow
    - [ ] Test the release drafter and release publication process (dry run if possible).
    - [ ] Ensure artifacts are built and attached correctly for all targets.
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Release & Benchmark Stabilization' (Protocol in workflow.md)
