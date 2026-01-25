---
title: CLI Internals
sidebar_position: 35
description: CLI implementation details and optimization
---

# CLI Internals

The OMG CLI (`omg`) is a high-performance, statically linked binary designed for sub-10ms response times. It uses a sophisticated hybrid execution model to ensure that common operations are instant while complex tasks remain non-blocking.

## ⚡ Execution Model

OMG employs three distinct execution paths based on the nature of the requested command:

1. **The Fast Sync Path (Sub-10ms)**: Used for cached searches and status checks. It bypasses the overhead of an asynchronous runtime, making direct sub-millisecond calls to the daemon's IPC interface.
2. **Hybrid Path**: For operations like `omg info`, the CLI first attempts a fast sync check for local data. If a network fallback (like the AUR) is required, it dynamically spins up a task-based runtime.
3. **Dedicated Async Path**: Used for network-bound operations like `omg install` or runtime management, where parallel downloading and real-time progress tracking are essential.

---

## 🛰️ Daemon Integration

The CLI acts as a thin but intelligent client for the background daemon (`omgd`). It automatically selects the most efficient communication channel:
- **Sync Client**: Optimized for minimal latency on "read" operations (status, cache hits).
- **Async Client**: Optimized for "write" operations and long-running "read" tasks (AUR searches).

If the daemon is not running, the CLI can automatically attempt to start it in the background or fall back to direct system queries, ensuring the tool remains functional even in edge cases.

---

## 🛠️ Command Architecture

Commands are logically grouped to provide a consistent user experience:

| Category | Typical Command | Primary Path | Hardware Focus |
|----------|-----------------|--------------|----------------|
| **Package Ops** | `search`, `info` | Sync/Hybrid | Memory & IPC |
| **System Ops** | `status`, `audit`| Sync | Local Persistence |
| **Installation** | `install`, `sync`| Async | Network & Disk |
| **Runtimes** | `use`, `list`   | Async | Network & Symlinks |

---

## 🐚 Shell Integration

OMG provides deeply integrated shell hooks to manage your environment variables (like `PATH`) automatically as you navigate between projects.

### Dynamic Environment Refresh
When you enter a directory containing a version file (e.g., `.nvmrc` or `.python-version`), the OMG shell hook:
1. Detects the required runtime and version.
2. Updates your `PATH` in real-time to point to the OMG-managed installation.
3. Performs this check in less than 1ms, ensuring no perceptible lag in your shell prompt.

---

## 📈 Performance Engineering

- **Memory Efficiency**: The CLI uses a zero-allocation philosophy where possible, reusing buffers and preferring stack allocation over heap allocation.
- **Minimal Footprint**: Statically linked for zero external dependencies, resulting in a small binary that starts instantly.
- **Rich Feedback**: Uses high-performance progress indicators and concurrent output streams to keep you informed during long operations without impacting throughput.
