---
title: Snapshots
sidebar_position: 47
description: Local environment version control
---

# Environment Snapshots

**Time Travel for Your System**

Snapshots allow you to save the exact state of your OMG environment (runtimes + packages) and restore it later. Think of it like `git commit` for your system configuration.

## 📸 Creating a Snapshot

Capture your current state before making risky changes.

```bash
omg snapshot create --message "Before upgrading to Node 22"
```

This saves:
- All active runtime versions.
- The list of installed packages.
- A cryptographic hash of the state.

## 📜 Listing Snapshots

View your snapshot history.

```bash
omg snapshot list
```

```text
ID                   Date                 Hash         Message
snap-2024-01-20-a1b2 2024-01-20 14:30     7f8a9d...    Before upgrading to Node 22
snap-2024-01-15-c3d4 2024-01-15 09:00     3e4f5g...    Stable setup
```

## ⏪ Restoring a Snapshot

Roll back your system to a previous state.

```bash
# Preview changes
omg snapshot restore snap-2024-01-20-a1b2 --dry-run

# Apply restore
omg snapshot restore snap-2024-01-20-a1b2
```

OMG will calculate the difference and:
1. Downgrade/Upgrade runtimes to match the snapshot.
2. Install missing packages.
3. Remove extra packages (optional, prompts for confirmation).

## 🗑️ Managing Snapshots

Clean up old snapshots to save space.

```bash
omg snapshot delete snap-2024-01-15-c3d4
```

## Where are snapshots stored?

Snapshots are stored as JSON files in `~/.local/share/omg/snapshots/`. You can back up this directory to save your history.
