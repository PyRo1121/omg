# libscoop v0.1.0-beta.7 - Quick Reference

## Installation

Add to `Cargo.toml`:
```toml
[dependencies]
libscoop = "0.1.0-beta.7"
```

---

## Core API at a Glance

### Initialize Session
```rust
use libscoop::Session;
let session = Session::new();
```

### Install Package
```rust
use libscoop::{Session, operation, SyncOption};

operation::package_sync(
    &session,
    vec!["firefox"],
    vec![SyncOption::AssumeYes],
)?;
```

### Remove Package
```rust
operation::package_sync(
    &session,
    vec!["firefox"],
    vec![SyncOption::Remove, SyncOption::AssumeYes],
)?;
```

### List Installed
```rust
use libscoop::operation;

let packages = operation::package_query(
    &session,
    vec![],
    vec![],
    true,  // installed=true
)?;
```

### Search Packages
```rust
let packages = operation::package_query(
    &session,
    vec!["firefox"],
    vec![],
    false,  // installed=false
)?;
```

### Check Updates
```rust
use libscoop::QueryOption;

let upgradable = operation::package_query(
    &session,
    vec![],
    vec![QueryOption::Upgradable],
    true,
)?;
```

### Upgrade All
```rust
operation::package_sync(
    &session,
    vec![],
    vec![SyncOption::OnlyUpgrade, SyncOption::AssumeYes],
)?;
```

---

## Key Types

| Type | Purpose |
|------|---------|
| `Session` | Entry point, manages Scoop state |
| `Package` | Represents a package (has `.name()`, `.bucket()`) |
| `SyncOption` | Options for install/remove/upgrade operations |
| `QueryOption` | Options for search/list operations |
| `Error` | Error type for all operations |

---

## SyncOption Quick Reference

| Option | Use Case |
|--------|----------|
| `AssumeYes` | Auto-confirm all prompts |
| `Remove` | Uninstall package |
| `OnlyUpgrade` | Upgrade only, don't install |
| `Cascade` | Remove dependencies (with `Remove`) |
| `Purge` | Remove persistent data (with `Remove`) |
| `IgnoreCache` | Force re-download |
| `Offline` | Use only cached packages |
| `NoDependencies` | Don't install dependencies |

---

## QueryOption Quick Reference

| Option | Use Case |
|--------|----------|
| `Explicit` | Exact name match (no regex) |
| `Description` | Search in descriptions |
| `Binary` | Search in binary names |
| `Upgradable` | Check if upgradable (installed only) |

---

## Async Integration Pattern

Since libscoop is **sync-only**, wrap calls in `spawn_blocking`:

```rust
use libscoop::{Session, operation};

async fn my_operation() -> anyhow::Result<()> {
    tokio::task::spawn_blocking(|| {
        let session = Session::new();
        operation::package_sync(&session, vec!["pkg"], vec![])
            .map_err(|e| anyhow::anyhow!("{}", e))
    })
    .await?
}
```

---

## Error Handling

All operations return `Result<T, libscoop::Error>`:

```rust
match operation::package_sync(&session, vec!["pkg"], vec![]) {
    Ok(()) => println!("Success"),
    Err(e) => eprintln!("Error: {}", e),
}
```

Common errors:
- `PackageNotFound` - Package doesn't exist
- `PackageMultipleCandidates` - Multiple matches found
- `Regex` - Invalid regex pattern
- I/O errors - File system issues

---

## Complete Minimal Example

```rust
use libscoop::{Session, operation, SyncOption};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize
    let session = Session::new();
    
    // Install
    operation::package_sync(
        &session,
        vec!["firefox"],
        vec![SyncOption::AssumeYes],
    )?;
    
    // List
    let packages = operation::package_query(&session, vec![], vec![], true)?;
    println!("Installed: {} packages", packages.len());
    
    // Remove
    operation::package_sync(
        &session,
        vec!["firefox"],
        vec![SyncOption::Remove, SyncOption::AssumeYes],
    )?;
    
    Ok(())
}
```

---

## Comparison: Subprocess vs libscoop

### Subprocess (Current)
```rust
let output = tokio::process::Command::new("scoop")
    .args(&["install", "firefox"])
    .output()
    .await?;

if !output.status.success() {
    return Err(anyhow::anyhow!("Failed"));
}
```

**Cons**:
- Subprocess overhead
- Parse text output for errors
- No structured error types
- No progress monitoring

### libscoop (New)
```rust
tokio::task::spawn_blocking(|| {
    let session = Session::new();
    operation::package_sync(
        &session,
        vec!["firefox"],
        vec![SyncOption::AssumeYes],
    )
}).await?
```

**Pros**:
- Direct library calls
- Typed errors
- Event monitoring
- Dependency management
- Atomic transactions

---

## API Stability

⚠️ **Beta Version**: libscoop 0.1.0-beta.7 is in beta
- API may change in future versions
- Consider pinning to exact version: `libscoop = "=0.1.0-beta.7"`
- Monitor releases for breaking changes

---

## Platform Support

✅ **Windows** - Primary target
❌ **Linux/macOS** - Not supported (Scoop is Windows-only)

---

## Documentation Links

- **Crates.io**: https://crates.io/crates/libscoop
- **Docs.rs**: https://docs.rs/libscoop/0.1.0-beta.7
- **GitHub**: https://github.com/chawyehsu/hok
- **Scoop**: https://scoop.sh/

---

## Common Operations Reference

### Install with Dependencies
```rust
operation::package_sync(&session, vec!["pkg"], vec![SyncOption::AssumeYes])?;
```

### Install Without Dependencies
```rust
operation::package_sync(
    &session,
    vec!["pkg"],
    vec![SyncOption::NoDependencies, SyncOption::AssumeYes],
)?;
```

### Uninstall with Cleanup
```rust
operation::package_sync(
    &session,
    vec!["pkg"],
    vec![SyncOption::Remove, SyncOption::Purge, SyncOption::AssumeYes],
)?;
```

### Uninstall with Cascade
```rust
operation::package_sync(
    &session,
    vec!["pkg"],
    vec![SyncOption::Remove, SyncOption::Cascade, SyncOption::AssumeYes],
)?;
```

### Search with Regex
```rust
operation::package_query(&session, vec!["^python.*"], vec![], false)?;
```

### Search Exact Name
```rust
operation::package_query(
    &session,
    vec!["python"],
    vec![QueryOption::Explicit],
    false,
)?;
```

### List with Upgrade Check
```rust
operation::package_query(
    &session,
    vec![],
    vec![QueryOption::Upgradable],
    true,
)?;
```

### Batch Install
```rust
operation::package_sync(
    &session,
    vec!["firefox", "vscode", "git"],
    vec![SyncOption::AssumeYes],
)?;
```

### Batch Upgrade
```rust
operation::package_sync(
    &session,
    vec![],  // Empty = all packages
    vec![SyncOption::OnlyUpgrade, SyncOption::AssumeYes],
)?;
```

---

## Troubleshooting

### "Session not found"
- Ensure Scoop is installed on Windows
- Check Scoop configuration exists

### "Package not found"
- Verify package name is correct
- Check if bucket is added: `operation::bucket_list(&session)`

### "Multiple candidates found"
- Use `QueryOption::Explicit` for exact match
- Or use `SyncOption::AssumeYes` to auto-select

### Blocking the async runtime
- Always use `tokio::task::spawn_blocking()` for libscoop calls
- Never call libscoop directly in async context

---

## Next Steps

1. **Add dependency**: Update `Cargo.toml`
2. **Wrap operations**: Use `spawn_blocking()` for async integration
3. **Replace subprocess**: Update `src/package_managers/windows.rs`
4. **Test thoroughly**: Verify all operations work
5. **Monitor updates**: Watch for libscoop releases

---

## Files Generated

1. **LIBSCOOP_RESEARCH.md** - Comprehensive API documentation
2. **LIBSCOOP_EXAMPLES.md** - Practical code examples for all operations
3. **LIBSCOOP_SUMMARY.md** - This quick reference guide
