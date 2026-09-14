# Native mise compatibility implementation plan

The user approved implementing native mise support on September 13, 2026. Execute in this task without subagents.

**Goal:** Expand actual mise project compatibility while preserving OMG's installer and automatic-hook boundaries.

**Architecture:** Shared ordered configuration documents feed environment resolution, tool selection, and task discovery. A separate task model represents dependencies before execution. The existing native runtime and managed-tool installers remain the execution boundary.

**Baseline:** mise v2026.9.7, upstream source `0db3fbe9efcee1944bc4990434c1a5ebcb480fca`. Current OMG inspection and broader backlog are recorded in `docs/mise-compatibility.md`.

## Implementation sequence

- [x] Add a portable Cargo harness compiling production configuration modules by path. Demonstrate failures for local overrides, selected environment files, and ancestor precedence before implementation.
- [x] Add `src/config/mise_config.rs`: ordered configuration discovery, explicit environment selection, bounded parsing, source identity, and declaring project root. Keep discovery separate from script execution.
- [x] Replace the environment module's private file enumeration with shared discovery. Check child base versus parent selected environment, local versus base, and invalid files.
- [ ] Feed shared documents into runtime pin parsing. Preserve existing dedicated-pin behavior while allowing mise variants to override earlier mise declarations. Document intentional differences from upstream version-file defaults.
- [ ] Add a native task definition and planner module. Cover shorthand, dependency-only groups, cycles, missing prerequisites, diamond deduplication, working directories, and malformed commands before wiring execution.
- [ ] Integrate definitions into the existing task runner without changing other ecosystem selection. Execute planned dependencies with their own environment and declaring root; stop on failure.
- [ ] Run the portable production-module tests and formatter. Push a focused PR for Linux build and existing regressions, then use the staged QEMU workflow for guest execution.
- [ ] Continue the inventory's remaining file-task, template, backend, lockfile, and plugin workstreams; report unsupported behavior honestly until implemented and tested.

## Verification contract

The local harness uses the real production modules, not a second implementation. Tests use temporary files and literal expected output. No workstation environment mutation or real package install is needed for configuration tests. Native Linux tests validate actual shell execution; a Windows parser pass is not a Linux integration pass.

Do not remove regression tests, weaken archive/signature checks, reintroduce an implicit mise subprocess fallback, or enable automatic project environment sourcing to obtain parity. Keep remaining incompatibilities visible rather than asserting full coverage after this first increment.
