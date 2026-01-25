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
```
~/.local/share/omg/history.json
```

### Transaction Types

The history tracks four types of operations:

| Type | Description | Icon in TUI |
|------|-------------|-------------|
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

### History Limits

- **Maximum entries**: 1000 transactions
- **Automatic pruning**: When limit reached, oldest entries are removed
- **Persistence**: JSON format for human readability and easy backup
- **Corruption handling**: Gracefully handles corrupted files (returns empty history)

## Viewing History

### Command Usage

```bash
omg history
omg history --limit 10
```

### Output Format

```
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
|-------------------|-----------------|
| Install | Remove the installed packages |
| Remove | Reinstall the removed packages (at current version) |
| Update | Downgrade to previous versions (if available in cache) |
| Sync | No action (database sync cannot be rolled back) |

### Basic Rollback

```bash
# Rollback a specific transaction by ID (partial match supported)
omg rollback abc123
```

The rollback will:
1. Find the transaction matching the ID prefix
2. Display what will be rolled back
3. Ask for confirmation
4. Execute the reverse operations

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
5. **Remove reinstalls at current version**: Cannot restore the exact removed version

### Downgrade Requirements

For update rollback to work, previous versions must exist in:
```
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
    
    /// Add a new transaction (auto-prunes to 1000 entries)
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

### Corrupted history file

If `history.json` is corrupted, OMG gracefully handles it:
```rust
let history: Vec<Transaction> = serde_json::from_str(&content)
    .unwrap_or_default();  // Returns empty Vec on error
```

To manually reset:
```bash
rm ~/.local/share/omg/history.json
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
|------|---------|
| [core/history.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/core/history.rs) | HistoryManager, Transaction, PackageChange structs |
| [cli/commands.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/commands.rs) | `history` and `rollback` command implementations |
| [cli/tui/app.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/tui/app.rs) | History loading for TUI display |
