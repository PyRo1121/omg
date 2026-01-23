# Implementation Plan - Debian Daemon Refactor

This plan follows the project's TDD protocol: Write Test -> Implement -> Refactor.

## Phase 1: Daemon Integration & IPC
- [ ] Task: Define Debian-specific IPC message types in `omg-lib`
    - [ ] Write failing unit tests for Debian search request/response serialization
    - [ ] Implement `DebianSearch` variants in the IPC protocol
- [ ] Task: Integrate `debian-packaging` indexing into `omgd`
    - [ ] Write failing tests for Debian index initialization in the daemon
    - [ ] Implement background indexing for APT packages in `omgd`
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Daemon Integration & IPC' (Protocol in workflow.md)

## Phase 2: Client Refactor
- [ ] Task: Update `omg search` to route Debian queries via the daemon
    - [ ] Write failing integration tests for `omg search` on Debian systems
    - [ ] Implement client-side routing to `omgd` for Debian searches
- [ ] Task: Implement result caching for Debian searches
    - [ ] Write failing tests for cache hits/misses in `omgd`
    - [ ] Implement caching logic using `moka` or `redb`
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Client Refactor' (Protocol in workflow.md)

## Phase 3: Verification & Benchmarking
- [ ] Task: Add comprehensive integration suite for Debian/Ubuntu
    - [ ] Implement Docker-based smoke tests for Debian search operations
- [ ] Task: Performance benchmarking
    - [ ] Run `benchmark.sh` and verify sub-30ms performance for Debian searches
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Verification & Benchmarking' (Protocol in workflow.md)
