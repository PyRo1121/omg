# libscoop v0.1.0-beta.7 API Research

## Overview

**libscoop** is a pure Rust reimplementation of Scoop (Windows package manager). It provides a library API for programmatic package management without subprocess calls.

- **Repository**: https://github.com/chawyehsu/hok
- **Crate**: https://crates.io/crates/libscoop
- **Docs**: https://docs.rs/libscoop/0.1.0-beta.7
- **License**: MIT OR Apache-2.0
- **Version**: 0.1.0-beta.7

## Key Characteristics

✅ **Sync-only API** - No async/await support (uses `futures::executor::ThreadPool` internally)
✅ **Pure Rust** - No subprocess calls needed
✅ **Event-driven** - Emits events during operations via event bus
✅ **Error handling** - Uses `thiserror` for custom error types
✅ **Windows-focused** - Designed for Windows package management

## Core API Structure

### 1. Session Initialization

**Type**: `libscoop::Session`

```rust
use libscoop::Session;

// Create a new session with default config path
let session = Session::new();

// Or create with custom config path
let session = Session::new_with("/path/to/config")?;

// Access configuration
let config = session.config();
println!("{}", config.root_path().display());

// Access event bus for monitoring operations
let event_bus = session.event_bus();
```

**Key Methods**:
- `Session::new()` - Creates session with default config
- `Session::new_with(path)` - Creates session with custom config path (returns `Result`)
- `session.config()` - Returns `Ref<'_, Config>` (immutable borrow)
- `session.event_bus()` - Returns `&EventBus` for event monitoring
- `session.set_user_agent(agent)` - Set custom user agent (can only be set once)

---

## 2. Package Operations

### A. Install Package

**Function**: `libscoop::operation::package_sync()`

```rust
use libscoop::{Session, operation, SyncOption};

let session = Session::new();

// Install a single package
let result = operation::package_sync(
    &session,
    vec!["firefox"],  // package names to install
    vec![],           // no special options
);

match result {
    Ok(()) => println!("Installation successful"),
    Err(e) => eprintln!("Installation failed: {}", e),
}
```

**With Options**:
```rust
use libscoop::{Session, operation, SyncOption};

let session = Session::new();

// Install with options
let result = operation::package_sync(
    &session,
    vec!["firefox", "vscode"],
    vec![
        SyncOption::AssumeYes,      // Auto-confirm prompts
        SyncOption::IgnoreCache,    // Force re-download
    ],
);
```

**Signature**:
```rust
pub fn package_sync(
    session: &Session,
    queries: Vec<&str>,
    options: Vec<SyncOption>,
) -> Result<(), Error>
```

---

### B. Remove/Uninstall Package

**Function**: `libscoop::operation::package_sync()` with `SyncOption::Remove`

```rust
use libscoop::{Session, operation, SyncOption};

let session = Session::new();

// Uninstall a package
let result = operation::package_sync(
    &session,
    vec!["firefox"],
    vec![SyncOption::Remove],
);

match result {
    Ok(()) => println!("Uninstall successful"),
    Err(e) => eprintln!("Uninstall failed: {}", e),
}
```

**With Additional Options**:
```rust
use libscoop::{Session, operation, SyncOption};

let session = Session::new();

// Uninstall with cascade (remove dependencies) and purge (remove persistent data)
let result = operation::package_sync(
    &session,
    vec!["firefox"],
    vec![
        SyncOption::Remove,
        SyncOption::Cascade,        // Remove dependencies
        SyncOption::Purge,          // Remove persistent data
        SyncOption::AssumeYes,      // Auto-confirm
    ],
);
```

---

### C. List Installed Packages

**Function**: `libscoop::operation::package_query()` with `installed=true`

```rust
use libscoop::{Session, operation, QueryOption};

let session = Session::new();

// List all installed packages
let packages = operation::package_query(
    &session,
    vec![],           // empty query = all packages
    vec![],           // no special options
    true,             // installed=true
)?;

for pkg in packages {
    println!("Package: {}", pkg.name());
}
```

**Signature**:
```rust
pub fn package_query(
    session: &Session,
    queries: Vec<&str>,
    options: Vec<QueryOption>,
    installed: bool,
) -> Result<Vec<Package>, Error>
```

**Returns**: `Vec<Package>` sorted by package name

---

### D. Search Packages

**Function**: `libscoop::operation::package_query()` with `installed=false`

```rust
use libscoop::{Session, operation, QueryOption};

let session = Session::new();

// Search for packages matching a pattern
let packages = operation::package_query(
    &session,
    vec!["firefox"],  // search query (regex by default)
    vec![],           // no special options
    false,            // installed=false (search all available)
)?;

for pkg in packages {
    println!("Found: {} ({})", pkg.name(), pkg.bucket());
}
```

**With Query Options**:
```rust
use libscoop::{Session, operation, QueryOption};

let session = Session::new();

// Search with explicit mode (no regex, exact name match)
let packages = operation::package_query(
    &session,
    vec!["firefox"],
    vec![QueryOption::Explicit],  // Disable regex, match name only
    false,
)?;

// Search including description and binaries
let packages = operation::package_query(
    &session,
    vec!["web"],
    vec![
        QueryOption::Description,  // Search in descriptions
        QueryOption::Binary,       // Search in binary names
    ],
    false,
)?;
```

---

### E. Check for Updates (List Upgradable Packages)

**Function**: `libscoop::operation::package_query()` with `QueryOption::Upgradable`

```rust
use libscoop::{Session, operation, QueryOption};

let session = Session::new();

// List all upgradable packages
let upgradable = operation::package_query(
    &session,
    vec![],           // empty query = all packages
    vec![QueryOption::Upgradable],  // Check if upgradable
    true,             // installed=true
)?;

for pkg in upgradable {
    println!("Upgradable: {}", pkg.name());
}
```

**Upgrade Packages**:
```rust
use libscoop::{Session, operation, SyncOption};

let session = Session::new();

// Upgrade all packages
let result = operation::package_sync(
    &session,
    vec![],           // empty = upgrade all
    vec![SyncOption::OnlyUpgrade],  // Only upgrade, don't install new
)?;
```

---

## 3. SyncOption Enum

Options for `package_sync()` operation:

| Option | Purpose |
|--------|---------|
| `AssumeYes` | Auto-confirm all prompts, suppress candidate selection |
| `DownloadOnly` | Download packages without installing |
| `EscapeHold` | Force operations on held packages |
| `IgnoreCache` | Force re-download, ignore local cache |
| `IgnoreFailure` | Continue on failure (no rollback) |
| `NoDependencies` | Don't install dependencies |
| `NoHashCheck` | Skip integrity verification (NOT recommended) |
| `NoUpgrade` | Don't upgrade packages |
| `NoReplace` | Don't replace packages from different buckets |
| `Offline` | Use only cached packages |
| `OnlyUpgrade` | Upgrade packages only |
| `Remove` | Uninstall packages |
| `Purge` | Remove persistent data (with `Remove`) |
| `Cascade` | Remove dependencies (with `Remove`) |
| `NoDependentCheck` | Skip dependent check on removal |

---

## 4. QueryOption Enum

Options for `package_query()` operation:

| Option | Purpose |
|--------|---------|
| `Binary` | Search through package binaries |
| `Description` | Search through package descriptions |
| `Explicit` | Exact name match (regex disabled) |
| `Upgradable` | Check if package is upgradable (installed only) |

---

## 5. Package Struct

**Type**: `libscoop::Package`

```rust
pub struct Package {
    bucket: String,    // Bucket name
    name: String,      // Package name
    // ... other fields
}
```

**Key Methods** (inferred from usage):
- `pkg.name()` - Get package name
- `pkg.bucket()` - Get bucket name

---

## 6. Error Handling

**Type**: `libscoop::Error`

```rust
use libscoop::{Session, operation};

let session = Session::new();

match operation::package_query(&session, vec!["firefox"], vec![], false) {
    Ok(packages) => {
        // Handle success
    }
    Err(e) => {
        // Handle error
        eprintln!("Error: {}", e);
        
        // Error variants (from docs):
        // - PackageNotFound
        // - PackageMultipleCandidates
        // - Regex (invalid regex pattern)
        // - I/O errors
    }
}
```

---

## 7. Event Bus

**Type**: `libscoop::EventBus`

```rust
use libscoop::{Session, Event};

let session = Session::new();
let event_bus = session.event_bus();

// Events are emitted during operations
// Event type: libscoop::Event (enum)
```

**Event Types** (from docs):
- Various operation progress events
- Download progress
- Installation/removal status

---

## Complete Example: Full Workflow

```rust
use libscoop::{Session, operation, SyncOption, QueryOption};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize session
    let session = Session::new();
    
    // 2. Search for a package
    println!("=== Searching for packages ===");
    let search_results = operation::package_query(
        &session,
        vec!["firefox"],
        vec![QueryOption::Description],
        false,
    )?;
    
    for pkg in &search_results {
        println!("Found: {}", pkg.name());
    }
    
    // 3. Install a package
    println!("\n=== Installing package ===");
    operation::package_sync(
        &session,
        vec!["firefox"],
        vec![SyncOption::AssumeYes],
    )?;
    println!("Installation complete");
    
    // 4. List installed packages
    println!("\n=== Listing installed packages ===");
    let installed = operation::package_query(
        &session,
        vec![],
        vec![],
        true,
    )?;
    
    println!("Installed packages: {}", installed.len());
    for pkg in installed.iter().take(5) {
        println!("  - {}", pkg.name());
    }
    
    // 5. Check for updates
    println!("\n=== Checking for updates ===");
    let upgradable = operation::package_query(
        &session,
        vec![],
        vec![QueryOption::Upgradable],
        true,
    )?;
    
    if !upgradable.is_empty() {
        println!("Upgradable packages: {}", upgradable.len());
        
        // Upgrade all
        operation::package_sync(
            &session,
            vec![],
            vec![SyncOption::OnlyUpgrade, SyncOption::AssumeYes],
        )?;
        println!("Upgrade complete");
    }
    
    // 6. Uninstall a package
    println!("\n=== Uninstalling package ===");
    operation::package_sync(
        &session,
        vec!["firefox"],
        vec![SyncOption::Remove, SyncOption::AssumeYes],
    )?;
    println!("Uninstall complete");
    
    Ok(())
}
```

---

## Error Handling Pattern

```rust
use libscoop::{Session, operation, Error};

fn install_package(name: &str) -> Result<(), Error> {
    let session = Session::new();
    
    operation::package_sync(
        &session,
        vec![name],
        vec![],
    ).map_err(|e| {
        eprintln!("Failed to install {}: {}", name, e);
        e
    })
}

// Usage
match install_package("firefox") {
    Ok(()) => println!("Success"),
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## Integration Notes for OMG

### Replacing Subprocess Calls

**Current Code** (subprocess):
```rust
let output = tokio::process::Command::new("scoop")
    .args(&["install", "firefox"])
    .output()
    .await?;
```

**New Code** (libscoop):
```rust
use libscoop::{Session, operation, SyncOption};

let session = Session::new();
operation::package_sync(
    &session,
    vec!["firefox"],
    vec![SyncOption::AssumeYes],
)?;
```

### Key Advantages

1. **No subprocess overhead** - Direct library calls
2. **Better error handling** - Typed errors instead of parsing output
3. **Event monitoring** - Real-time progress via event bus
4. **Dependency management** - Automatic dependency resolution
5. **Atomic operations** - Transactions with rollback support

### Compatibility

- **Sync-only**: No async/await, but can be wrapped in `tokio::task::spawn_blocking()`
- **Windows-only**: Designed for Windows Scoop
- **Pure Rust**: No external dependencies beyond what's in Cargo.toml

---

## Dependencies

libscoop uses these key dependencies:
- `chrono` - Date/time handling
- `curl` - HTTP downloads
- `git2` - Git operations for bucket management
- `serde`/`serde_json` - Serialization
- `thiserror` - Error types
- `tracing` - Structured logging
- `regex` - Pattern matching
- `winreg` - Windows registry access
- `junction` - Windows junction/symlink handling

---

## Version Compatibility

- **libscoop**: 0.1.0-beta.7 (beta, API may change)
- **Rust Edition**: 2021
- **MSRV**: Not explicitly stated, but likely 1.56+

---

## References

- **Crates.io**: https://crates.io/crates/libscoop
- **Docs.rs**: https://docs.rs/libscoop/0.1.0-beta.7
- **GitHub**: https://github.com/chawyehsu/hok
- **Hok CLI**: Reference implementation using libscoop
