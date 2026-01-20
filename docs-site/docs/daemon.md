---
title: Daemon Internals
sidebar_position: 31
description: Background service lifecycle, IPC, and state management
---

# Daemon Internals (omgd)

The OMG daemon (`omgd`) is the central services engine that manages system state, package indices, and background synchronization.

## 🚀 Daemon Lifecycle

### 1. Initialization and Socket Setup
When the daemon starts, it resolves its operating environment and establishes a secure communication channel:
- **Socket Resolution**: It identifies the optimal path for the Unix socket, prioritizing the user's runtime directory (`$XDG_RUNTIME_DIR`) and falling back to systemic shared locations.
- **Cleanup and Bind**: It ensures a fresh start by removing any stale socket files and binding with strict `0600` permissions (user read/write only).
- **Background Forking**: By default, it detaches from the terminal to run as a persistent background service, logging its activity to the system journal.

### 2. State Management
The daemon maintains a comprehensive, thread-safe view of the system's package and runtime environment:
- **In-Memory Cache (moka)**: A high-speed cache for recent search queries, package metadata, and system status results.
- **Persistent Store (redb)**: An embedded, ACID-compliant database that ensures system status and audit logs survive reboots.
- **Package Index**: A highly optimized, searchable index built from official repository databases, enabling sub-millisecond lookups.
- **Runtime Registry**: A dynamic list of all installed language runtimes and their active versions.

### 3. Background Synchronization
A dedicated worker thread handles ongoing system maintenance without interrupting user operations:
- **Periodic Refresh**: Every 5 minutes, the daemon performs a background "vital signs" check.
- **Vulnerability Scanning**: Continuous background analysis of installed packages against the ALSA and OSV databases.
- **Prompt Optimization**: Updates the binary status file used by `omg-fast` to ensure shell prompts are always accurate and instant.

---

## 🛰️ Request Processing

Every request from the CLI or third-party tools is processed through a structured execution pipeline:

### Concurrent Handling
The daemon uses an asynchronous, task-based architecture. Every incoming connection is assigned its own isolated task, ensuring that a long-running search or complex dependency resolution doesn't block other users or system queries.

### Request Routing
Requests are automatically routed to specialized handlers based on their type:
- **Package Operations**: Handled by the official repository or AUR backends.
- **Security Audits**: Processed by the vulnerability and policy engines.
- **System Status**: Reading from either the in-memory cache or the persistent database.

---

## 🔒 Reliability and Failure Recovery

The daemon is designed for 100% uptime with graceful degradation:

- **Graceful Shutdown**: Upon receiving a termination signal (SIGINT/SIGTERM), the daemon signals all background tasks to complete their current work, saves its persistent state, and cleanly removes the socket file.
- **Self-Healing Index**: If the package index becomes corrupted or outdated, the daemon automatically rebuilds it from the underlying system databases.
- **Network Resilience**: If the internet connection is lost, the daemon seamlessly falls back to local-only mode, serving results from its cache and system databases while queuing non-critical network tasks.

### Performance Profile
- **Baseline Memory**: ~40MB (including full repository index)
- **Idle CPU**: <1%
- **IPC Latency**: ~100μs
- **Recovery Time**: <1s from cold start
