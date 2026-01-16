# History & Rollback

OMG maintains a transaction history log that tracks package operations, enabling review of past changes and rollback to previous states.

## Quick Reference

```bash
# View recent transactions
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

Transaction history is stored in:
```
~/.local/share/omg/history.json
```

### Transaction Types

The history tracks four types of operations:

| Type | Description |
|------|-------------|
| `Install` | Package installations |
| `Remove` | Package removals |
| `Update` | Package upgrades |
| `Sync` | Database synchronization |

### Transaction Structure

Each transaction records:

```rust
pub struct Transaction {
    pub id: String,           // UUID v4 identifier
    pub timestamp: Timestamp,  // When the operation occurred
    pub transaction_type: TransactionType,
    pub changes: Vec<PackageChange>,
    pub success: bool,        // Whether the operation succeeded
}

pub struct PackageChange {
    pub name: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub source: String,       // "official" or "aur"
}
```

### History Limits

- **Maximum entries**: 1000 transactions
- **Automatic cleanup**: Oldest entries removed when limit reached
- **Persistence**: JSON format for human readability

## Viewing History

### Basic Usage

```bash
omg history
```

Displays a formatted list of recent transactions:

```
📋 Transaction History (last 20)

[2026-01-16 13:00:00] abc123...
  Type: Install
  Status: ✓ Success
  Changes:
    + firefox 124.0-1 (official)
    + neovim 0.9.5-1 (official)

[2026-01-16 12:30:00] def456...
  Type: Update
  Status: ✓ Success
  Changes:
    ↑ linux 6.18.2 → 6.18.3 (official)
```

### Limiting Results

```bash
# Show only last 5 transactions
omg history --limit 5
```

## Rollback

### How Rollback Works

Rollback reverses a transaction by:
1. **Install** → Removes the installed packages
2. **Remove** → Reinstalls the removed packages (at current version)
3. **Update** → Downgrades to previous versions (if available in cache)

### Basic Rollback

```bash
# Rollback a specific transaction
omg rollback abc123def456...
```

### Interactive Rollback

```bash
# Select from recent transactions
omg rollback
```

This presents an interactive selection of recent transactions to choose from.

### Rollback Limitations

> [!WARNING]
> Rollback currently has the following limitations:

1. **Official packages only**: AUR package rollback is not yet supported
2. **Downgrade availability**: Requires previous versions in pacman cache
3. **No dependency resolution**: May leave dependency inconsistencies
4. **Sync cannot be rolled back**: Database sync is informational only

### Downgrade Requirements

For update rollback to work, previous package versions must be available in:
```
/var/cache/pacman/pkg/
```

Configure pacman to keep old versions:
```ini
# /etc/pacman.conf
CleanMethod = KeepCurrent
```

## Implementation Details

### History Manager

The `HistoryManager` struct handles all history operations:

```rust
pub struct HistoryManager {
    log_path: PathBuf,  // ~/.local/share/omg/history.json
}

impl HistoryManager {
    pub fn new() -> Result<Self>;
    pub fn load(&self) -> Result<Vec<Transaction>>;
    pub fn save(&self, history: &[Transaction]) -> Result<()>;
    pub fn add_transaction(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        success: bool,
    ) -> Result<()>;
}
```

### Transaction Recording

Transactions are recorded automatically by package operations:

1. **Before operation**: Changes are prepared
2. **After operation**: Transaction is logged with success status
3. **On failure**: Transaction is still logged with `success: false`

### Rollback Process

```rust
pub fn rollback(id: Option<String>) -> Result<()> {
    let manager = HistoryManager::new()?;
    let history = manager.load()?;
    
    // Find the target transaction
    let transaction = match id {
        Some(id) => history.iter().find(|t| t.id.starts_with(&id)),
        None => interactive_select(&history),
    };
    
    // Execute reverse operations
    for change in &transaction.changes {
        match transaction.transaction_type {
            TransactionType::Install => {
                // Remove the package
                remove_package(&change.name)?;
            }
            TransactionType::Update => {
                // Downgrade to old_version
                if let Some(old) = &change.old_version {
                    downgrade_package(&change.name, old)?;
                }
            }
            // ... etc
        }
    }
}
```

## Best Practices

### Regular Backups

While history tracks changes, consider:
- **System snapshots**: Use Btrfs/ZFS snapshots for full recovery
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

### Troubleshooting Failed Rollbacks

If rollback fails:

1. **Check cache**: Ensure old packages exist
   ```bash
   ls /var/cache/pacman/pkg/ | grep <package>
   ```

2. **Manual downgrade**: Use pacman directly
   ```bash
   sudo pacman -U /var/cache/pacman/pkg/<package>-<version>.pkg.tar.zst
   ```

3. **Check dependencies**: Resolve any conflicts manually

## Future Enhancements

Planned improvements:
- **AUR rollback**: Support for AUR package rollback
- **Dependency resolution**: Automatic handling of dependencies
- **Selective rollback**: Choose specific packages from a transaction
- **Snapshot integration**: Integration with Btrfs/ZFS snapshots
- **Remote history**: Sync history across machines

## Source Files

- History manager: [core/history.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/core/history.rs)
- Rollback command: [cli/commands.rs](file:///home/pyro1121/Documents/code/filemanager/omg/src/cli/commands.rs)
