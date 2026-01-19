---
title: Architecture
sidebar_label: Architecture
sidebar_position: 10
description: System architecture and component overview
---

# Architecture

OMG is split into a fast CLI and a persistent daemon, both backed by a shared Rust library.
This page is the high-level map; each subsystem has its own deep dive.

## Components
- **`omg` (CLI)**: user-facing interface. Prefers sync, cached paths; falls back to async or direct operations when needed. Runs a task-specific Tokio runtime for async commands.
- **`omgd` (daemon)**: persistent background server with Unix socket IPC. Owns in-memory caches, redb persistence, and the official package index.
- **Shared library**: CLI commands, runtime managers, security scanner, and IPC protocol types.

### Distro Backends
- **Arch (default)**: uses libalpm for package operations and indexing.
- **Debian/Ubuntu (feature-gated)**: build with `--features debian`, backed by `rust-apt` and `libapt-pkg`.
- **AUR**: Arch-only (disabled on Debian/Ubuntu).

## Request Lifecycle
1. CLI receives a command and checks if it has a sync fast-path.
2. If the daemon is running, the CLI hits IPC for cached results.
3. If the daemon is unavailable, the CLI falls back to direct libalpm or runtime operations.
4. Responses are formatted and returned to the terminal.

## Daemon Lifecycle
- Startup initializes daemon state (cache, redb, package index, managers) and starts a background worker.
- The background worker refreshes runtime probes, system status, and vulnerability counts every 300s.
- The server accepts new IPC clients and handles each connection concurrently.
- Shutdown is coordinated via a broadcast channel and cleans up the socket.

## IPC
- Unix domain socket using length-delimited framing.
- Binary serialization (bincode) for low latency.
- Requests are paired with IDs; the client verifies response IDs.

## Caching & Persistence
- **redb** for status persistence across daemon restarts.
- **In-memory moka cache** for search results, info lookups, and system status.
- **In-memory index** for instant fuzzy search of official packages.
  - Arch: index built from libalpm sync DBs.
  - Debian/Ubuntu: index built from APT cache.

## Runtime Management
- **Native runtimes**: Pure Rust implementations for Node, Python, Go, Rust, Ruby, Java, Bun via `RuntimeManager` trait.
- **Extended runtimes**: Built-in `MiseManager` provides 100+ additional runtimes (Deno, Elixir, Zig, etc.) with automatic mise download.
- Auto-detection via version files (`.nvmrc`, `.python-version`, `.tool-versions`, `.mise.toml`, etc.).
- Active version probes are stored in daemon status caches.
- Mise binary is bundled at `~/.local/share/omg/mise/mise` - no external installation required.

## Security Model
- Audit workflows are served by the daemon for speed.
- Vulnerability scans are aggregated into status responses and audit output.

## Deep Dives
- [Daemon Internals](./daemon.md)
- [IPC Protocol](./ipc.md)
- [Caching & Indexing](./cache.md)
- [Package Search Flow](./package-search.md)
- [Runtime Management](./runtimes.md)
- [CLI Internals](./cli-internals.md)
- [Security & Audit Pipeline](./security.md)
