# Specification - Debian Daemon Refactor

## Problem Statement
Currently, Debian/Ubuntu support in OMG relies on direct library calls or subprocesses that may not fully leverage the high-performance `omgd` daemon architecture. This results in inconsistent performance between Arch and Debian systems and lacks comprehensive TDD coverage for the IPC layer.

## Goals
- Integrate Debian package indexing into the `omgd` daemon.
- Implement zero-copy IPC for APT search queries using `bitcode` or `rkyv`.
- Ensure 100% test coverage for the refactored search paths.
- Maintain sub-30ms search performance on Debian systems.

## Requirements
- **Daemon Integration:** The `omgd` daemon must handle APT cache indexing and updates.
- **IPC Protocol:** Update the length-delimited binary protocol to support Debian-specific search results.
- **TDD:** All new logic must follow the Red-Green-Refactor cycle.
- **Compatibility:** Maintain support for existing Arch Linux operations.
