---
title: History & Rollback
sidebar_position: 24
description: Transaction history and system rollback
---

# History & Rollback

OMG maintains a transaction history log that tracks package operations, enabling review of past changes and rollback to previous states.

## Quick Reference

```bash
# View recent transactions (default: last 20)
omg history

# View last 5 transactions
omg history --limit 5

# Rollback to a specific transaction
omg rollback <transaction-id>

# Interactive rollback selection
omg rollback
```

## Transaction History

### Storage Location

Transaction history is stored in JSON format:

```text
~/.local/share/omg/history.json
```

### Transaction Types

The history tracks four types of operations:

| Type | Description | Icon in TUI |
| ------ | ------------- | ------------- |
| `Install` | Package installations | Install |
| `Remove` | Package removals | Remove |
| `Update` | Package upgrades | Update |
| `Sync` | Database synchronization | Sync |

### Data Structure

Each transaction records the following information:

```rust
pub struct Transaction {
    pub id: String,                    // UUID v4 identifier
    pub timestamp: Timestamp,          // jiff::Timestamp (precise timing)
    pub transaction_type: TransactionType,
    pub changes: Vec<PackageChange>,
    pub success: bool,                 // Whether the operation succeeded
}

pub struct PackageChange {
    pub name: String,                  // Package name
    pub old_version: Option<String>,   // Previous version (for updates/removes)
    pub new_version: Option<String>,   // New version (for installs/updates)
    pub source: String,                // "official" or "aur"
}
```

### Recording failures

`omg audit fix` records successful package upgrades as `Update` transactions, with the old and target versions needed for rollback. Delegated updates have one history owner to avoid duplicate records.

For audit fixes and the fast update path, if an update succeeds but recording fails, OMG returns an error stating that the package operation succeeded but its history could not be persisted. The packages remain changed. Do not interpret this error as a failed or reversed package transaction.

### History limits

- The live history retains at most 1,000 transactions.
- Older entries are appended to `history.json.archive.jsonl`. Normal history, rollback, and rollback-cache protection read both files, including when the live file is absent.
- Identical transaction IDs and contents left by interrupted retirement are returned once. Conflicting records with the same ID fail explicitly rather than selecting one.
- Malformed live or archived history causes reads to fail without moving or replacing either file. Corruption is not reported as empty history. Archive reads currently collect the combined history in memory.
- When recording a new transaction, malformed live history is moved to a `history.json.corrupt-*` file for manual recovery before the fresh log is written. Preserved bytes are not automatically available to rollback.
- A failed transaction can include successful individual changes. The transaction-level status does not describe each package separately, and automatic rollback rejects failed transactions.

## Viewing History

### Command Usage

```bash
omg history
omg history --limit 10
```

### Output Format

```text
📋 Transaction History (last 20)

┌─────────────────────────────────────────────────────────────────┐
│ Transaction: abc123-def456...                                   │
│ Time: 2026-01-16 13:00:00                                       │
│ Type: Install  │  Status: ✓ Succeeded                           │
├─────────────────────────────────────────────────────────────────┤
│ Changes:                                                        │
│   + firefox 124.0-1 (official)                                  │
│   + neovim 0.9.5-1 (official)                                   │
└─────────────────────────────────────────────────────────────────┘
```

Symbols used:

- `+` New package installed
- `-` Package removed
- `↑` Package upgraded (shows old → new version)

## Rollback

### How Rollback Works

Rollback reverses a transaction by performing the opposite operation:

| Original Operation | Rollback Action |
| ------------------- | ----------------- |
| Install | Remove the installed packages |
| Remove | Restore the recorded previous versions when available |
| Update | Downgrade to previous versions (if available in cache) |
| Sync | No action (database sync cannot be rolled back) |

### Basic Rollback

```bash
# Rollback a specific transaction by ID (partial match supported)
omg rollback abc123
```

The rollback will:

1. Find the transaction matching the ID prefix
2. Reject failed or partially applied transactions and unsupported sources such as AUR updates
3. Display what will be rolled back
4. Ask for confirmation
5. Execute the reverse operation

On Arch Linux, restoring a removed or updated package requires the exact old package archive in a configured pacman cache (`CacheDir` entries in `pacman.conf`). OMG fails before changing anything if any required archive is missing. Packages listed in `HoldPkg` cannot be removed by rollback, and rollback restores only official-repository packages.

### Interactive Rollback

```bash
omg rollback
```

This presents an interactive selection of recent transactions using `dialoguer`:

1. Shows the last 10 transactions
2. Displays transaction details (type, time, packages)
3. Allows selection via arrow keys
4. Confirms before executing

### Rollback Implementation

```rust
pub async fn rollback(id: Option<String>) -> Result<()> {
    let manager = HistoryManager::new()?;
    let history = manager.load()?;
    
    // Find target transaction
    let transaction = match id {
        Some(prefix) => {
            history.iter()
                .find(|t| t.id.starts_with(&prefix))
                .ok_or_else(|| anyhow!("Transaction not found"))?
        }
        None => {
            // Interactive selection
            let selection = Select::with_theme(&ColorfulTheme::default())
                .items(&history)
                .interact()?;
            &history[selection]
        }
    };
    
    // Execute reverse operations based on transaction type
    match transaction.transaction_type {
        TransactionType::Install => {
            // Remove installed packages
            for change in &transaction.changes {
                remove_package(&change.name)?;
            }
        }
        TransactionType::Update => {
            // Downgrade to old versions
            for change in &transaction.changes {
                if let Some(old_ver) = &change.old_version {
                    downgrade_package(&change.name, old_ver)?;
                }
            }
        }
        // ... etc
    }
}
```

## Limitations

> [!WARNING]
> Current rollback limitations:

1. **Official packages only**: AUR package rollback is not yet fully supported
2. **Downgrade availability**: Update rollback requires old package versions in pacman cache
3. **No automatic dependency resolution**: May leave dependency inconsistencies
4. **Sync cannot be rolled back**: Database sync is informational only
5. **Failed transactions are not automatic**: Partially applied operations require manual recovery

### Downgrade Requirements

For update rollback to work, previous versions must exist in:

```text
/var/cache/pacman/pkg/
```

Configure pacman to retain old versions:

```ini
# /etc/pacman.conf
CleanMethod = KeepCurrent
```

Or use `paccache` to manage cache retention:

```bash
# Keep last 3 versions of each package
paccache -rk3
```

## HistoryManager API

The `HistoryManager` struct provides the public API for history operations:

```rust
pub struct HistoryManager {
    log_path: PathBuf,  // ~/.local/share/omg/history.json
}

impl HistoryManager {
    /// Create a new manager (creates directory if needed)
    pub fn new() -> Result<Self>;
    
    /// Load all transactions from disk
    pub fn load(&self) -> Result<Vec<Transaction>>;
    
    /// Save transaction list to disk
    pub fn save(&self, history: &[Transaction]) -> Result<()>;
    
    /// Add a new transaction (archives entries beyond the 1000-entry live limit)
    pub fn add_transaction(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        success: bool,
    ) -> Result<()>;
}
```

### Recording Transactions

Transactions are recorded by package operations in `packages.rs`:

```rust
// After successful install
let history = HistoryManager::new()?;
let changes = packages.iter()
    .map(|p| PackageChange {
        name: p.name.clone(),
        old_version: None,
        new_version: Some(p.version.clone()),
        source: p.source.clone(),
    })
    .collect();
history.add_transaction(TransactionType::Install, changes, true)?;
```

## Integration with TUI

The TUI dashboard displays the last 10 transactions in the "Recent Activity" panel:

```rust
// In app.rs
if let Ok(entries) = history_mgr.load() {
    self.history = entries.into_iter().rev().take(10).collect();
}
```

Each entry shows:

- Timestamp (time only: HH:MM:SS)
- Transaction type
- Success/failure status
- First 3 affected packages

## Best Practices

### Regular Backups

While history tracks changes, consider additional safeguards:

- **System snapshots**: Use Btrfs/ZFS snapshots before major updates
- **Package list export**: `pacman -Qqe > packages.txt`
- **Config backups**: Keep `/etc` in version control

### Before Major Updates

```bash
# Check current state
omg status

# Review recent history
omg history --limit 5

# Proceed with update
omg update
```

### After a Failed Update

```bash
# View what changed
omg history --limit 1

# Rollback if needed
omg rollback
```

## Troubleshooting

### History file not found

The file is created automatically on first package operation. If missing:

```bash
# Ensure directory exists
mkdir -p ~/.local/share/omg

# Verify permissions
ls -la ~/.local/share/omg/
```

### Rollback fails

1. **Check package cache**:

   ```bash
   ls /var/cache/pacman/pkg/ | grep <package>
   ```

2. **Manual downgrade** (if cache has old version):

   ```bash
   sudo pacman -U /var/cache/pacman/pkg/<package>-<version>.pkg.tar.zst
   ```

3. **Dependency conflicts**: Resolve manually with pacman

## Source Files

| File | Purpose |
| ------ | --------- |
| [src/core/history.rs](../src/core/history.rs) | HistoryManager, Transaction, PackageChange structs |
| [src/cli/commands.rs](../src/cli/commands.rs) | `history` and `rollback` command implementations |
| [src/cli/tui/app.rs](../src/cli/tui/app.rs) | History loading for TUI display |
