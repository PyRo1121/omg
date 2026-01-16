# OMG Documentation

Welcome to the OMG documentation hub. This set of docs expands on the README with focused guides, reference material, and architecture notes for day-to-day use.

## Audience
- **Daily users**: Learn how to install, search, install, and manage runtimes.
- **Platform engineers**: Understand daemon behavior, caches, and policy enforcement.
- **Contributors**: Navigate code structure and runtime subsystems.

## Getting Started
- [CLI Reference](./cli.md)
- [Configuration & Policy](./configuration.md)
- [Architecture](./architecture.md)

## Deep Dives (Granular)
- [Daemon Internals](./daemon.md)
- [IPC Protocol](./ipc.md)
- [Caching & Indexing](./cache.md)
- [Package Search Flow](./package-search.md)
- [Runtime Management](./runtimes.md)
- [CLI Internals](./cli-internals.md)
- [Security & Audit Pipeline](./security.md)

## Core Concepts
- Unified package + runtime management
- Daemon-optional fast path with cached searches
- Graded security and policy enforcement
- Environment lockfiles for team sync
- **Built-in mise**: 100+ runtimes without external installation
- **Pure Rust ALPM**: V1/V2 desc format support for complete package database parsing

## Conventions
- Commands are shown as `omg <command>` unless otherwise specified.
- Paths follow XDG defaults when available.
- Examples assume Linux + zsh unless noted.

## Source of Truth
The README remains the high-level overview for features and quickstart. These docs aim to deepen specific areas without duplicating the entire README.
