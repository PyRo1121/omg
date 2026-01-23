# Implementation Plan - Debian Daemon Refactor

This plan follows the project's TDD protocol: Write Test -> Implement -> Refactor.

## Phase 1: Daemon Integration & IPC [checkpoint: 76108d5]
- [x] Task: Define Debian-specific IPC message types in `omg-lib` [82b3e6b]
    - [ ] Write failing unit tests for Debian search request/response serialization
    - [x] Implement `DebianSearch` variants in the IPC protocol [dcfef33]
- [x] Task: Integrate `debian-packaging` indexing into `omgd` [1cff35e]
    - [ ] Write failing tests for Debian index initialization in the daemon
    - [ ] Implement background indexing for APT packages in `omgd`
- [x] Task: Conductor - User Manual Verification 'Phase 1: Daemon Integration & IPC' (Protocol in workflow.md) [76108d5]

## Phase 2: Client Refactor [checkpoint: f2ebdaa]
- [x] Task: Update `omg search` to route Debian queries via the daemon [f95f015]
    - [x] Write failing integration tests for `omg search` on Debian systems
    - [x] Implement client-side routing to `omgd` for Debian searches
- [x] Task: Implement result caching for Debian searches [b809706]
    - [x] Write failing tests for cache hits/misses in `omgd`
    - [x] Implement caching logic using `moka` or `redb`
- [x] Task: Conductor - User Manual Verification 'Phase 2: Client Refactor' (Protocol in workflow.md) [f2ebdaa]

## Phase 3: Verification & Benchmarking
- [x] Task: Add comprehensive integration suite for Debian/Ubuntu [2509a55]
    - [x] Implement Docker-based smoke tests for Debian search operations
- [~] Task: Performance benchmarking
    - [ ] Run `benchmark.sh` and verify sub-30ms performance for Debian searches
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Verification & Benchmarking' (Protocol in workflow.md)
