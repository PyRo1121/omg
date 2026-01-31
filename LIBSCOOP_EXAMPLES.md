# libscoop Integration Examples for OMG

## Context: Replacing Subprocess Calls

**Current locations in codebase**:
- `src/package_managers/windows.rs` line 819 (install)
- `src/package_managers/windows.rs` line 1053 (remove/update)

---

## Example 1: Install Package (Replaces line 819)

### Current Code (Subprocess)
```rust
// Current: Using tokio::process::Command
let output = tokio::process::Command::new("scoop")
    .args(&["install", package_name])
    .output()
    .await?;

if !output.status.success() {
    return Err(anyhow::anyhow!("Failed to install package"));
}
```

### New Code (libscoop)
```rust
use libscoop::{Session, operation, SyncOption};
use anyhow::Result;

async fn install_package(package_name: &str) -> Result<()> {
    // Run in blocking context since libscoop is sync-only
    let package_name = package_name.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        operation::package_sync(
            &session,
            vec![&package_name],
            vec![SyncOption::AssumeYes],  // Auto-confirm prompts
        ).map_err(|e| anyhow::anyhow!("Failed to install package: {}", e))
    })
    .await?
}
```

### With Error Context
```rust
use libscoop::{Session, operation, SyncOption};
use anyhow::{Result, Context};

async fn install_package(package_name: &str) -> Result<()> {
    let package_name = package_name.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        operation::package_sync(
            &session,
            vec![&package_name],
            vec![SyncOption::AssumeYes],
        )
        .with_context(|| format!("Failed to install package '{}'", package_name))
    })
    .await?
}
```

---

## Example 2: Remove/Uninstall Package (Replaces line 1053)

### Current Code (Subprocess)
```rust
// Current: Using tokio::process::Command
let output = tokio::process::Command::new("scoop")
    .args(&["uninstall", package_name])
    .output()
    .await?;

if !output.status.success() {
    return Err(anyhow::anyhow!("Failed to uninstall package"));
}
```

### New Code (libscoop)
```rust
use libscoop::{Session, operation, SyncOption};
use anyhow::Result;

async fn remove_package(package_name: &str) -> Result<()> {
    let package_name = package_name.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        operation::package_sync(
            &session,
            vec![&package_name],
            vec![
                SyncOption::Remove,      // Uninstall
                SyncOption::AssumeYes,   // Auto-confirm
            ],
        ).map_err(|e| anyhow::anyhow!("Failed to remove package: {}", e))
    })
    .await?
}
```

### With Cascade and Purge Options
```rust
use libscoop::{Session, operation, SyncOption};
use anyhow::Result;

async fn remove_package_with_deps(
    package_name: &str,
    cascade: bool,
    purge: bool,
) -> Result<()> {
    let package_name = package_name.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        let mut options = vec![
            SyncOption::Remove,
            SyncOption::AssumeYes,
        ];
        
        if cascade {
            options.push(SyncOption::Cascade);  // Remove dependencies
        }
        if purge {
            options.push(SyncOption::Purge);    // Remove persistent data
        }
        
        operation::package_sync(
            &session,
            vec![&package_name],
            options,
        ).map_err(|e| anyhow::anyhow!("Failed to remove package: {}", e))
    })
    .await?
}
```

---

## Example 3: List Installed Packages

### Basic Implementation
```rust
use libscoop::{Session, operation};
use anyhow::Result;

async fn list_installed_packages() -> Result<Vec<String>> {
    tokio::task::spawn_blocking(|| {
        let session = Session::new();
        
        operation::package_query(
            &session,
            vec![],  // Empty query = all packages
            vec![],  // No special options
            true,    // installed=true
        )
        .map(|packages| {
            packages
                .into_iter()
                .map(|pkg| pkg.name().to_string())
                .collect()
        })
        .map_err(|e| anyhow::anyhow!("Failed to list packages: {}", e))
    })
    .await?
}
```

### With Filtering
```rust
use libscoop::{Session, operation};
use anyhow::Result;

async fn list_installed_packages_matching(pattern: &str) -> Result<Vec<String>> {
    let pattern = pattern.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        operation::package_query(
            &session,
            vec![&pattern],  // Regex pattern
            vec![],
            true,
        )
        .map(|packages| {
            packages
                .into_iter()
                .map(|pkg| pkg.name().to_string())
                .collect()
        })
        .map_err(|e| anyhow::anyhow!("Failed to list packages: {}", e))
    })
    .await?
}
```

---

## Example 4: Search Packages

### Basic Search
```rust
use libscoop::{Session, operation};
use anyhow::Result;

async fn search_packages(query: &str) -> Result<Vec<(String, String)>> {
    let query = query.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        operation::package_query(
            &session,
            vec![&query],
            vec![],
            false,  // installed=false (search all available)
        )
        .map(|packages| {
            packages
                .into_iter()
                .map(|pkg| (pkg.name().to_string(), pkg.bucket().to_string()))
                .collect()
        })
        .map_err(|e| anyhow::anyhow!("Search failed: {}", e))
    })
    .await?
}
```

### Explicit Search (No Regex)
```rust
use libscoop::{Session, operation, QueryOption};
use anyhow::Result;

async fn search_packages_exact(query: &str) -> Result<Vec<(String, String)>> {
    let query = query.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        operation::package_query(
            &session,
            vec![&query],
            vec![QueryOption::Explicit],  // Exact name match
            false,
        )
        .map(|packages| {
            packages
                .into_iter()
                .map(|pkg| (pkg.name().to_string(), pkg.bucket().to_string()))
                .collect()
        })
        .map_err(|e| anyhow::anyhow!("Search failed: {}", e))
    })
    .await?
}
```

### Search with Description
```rust
use libscoop::{Session, operation, QueryOption};
use anyhow::Result;

async fn search_packages_with_description(query: &str) -> Result<Vec<(String, String)>> {
    let query = query.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        operation::package_query(
            &session,
            vec![&query],
            vec![
                QueryOption::Description,  // Search descriptions
                QueryOption::Binary,       // Search binary names
            ],
            false,
        )
        .map(|packages| {
            packages
                .into_iter()
                .map(|pkg| (pkg.name().to_string(), pkg.bucket().to_string()))
                .collect()
        })
        .map_err(|e| anyhow::anyhow!("Search failed: {}", e))
    })
    .await?
}
```

---

## Example 5: Check for Updates (scoop status)

### List Upgradable Packages
```rust
use libscoop::{Session, operation, QueryOption};
use anyhow::Result;

async fn list_upgradable_packages() -> Result<Vec<String>> {
    tokio::task::spawn_blocking(|| {
        let session = Session::new();
        
        operation::package_query(
            &session,
            vec![],  // Empty query = all packages
            vec![QueryOption::Upgradable],  // Check if upgradable
            true,    // installed=true
        )
        .map(|packages| {
            packages
                .into_iter()
                .map(|pkg| pkg.name().to_string())
                .collect()
        })
        .map_err(|e| anyhow::anyhow!("Failed to check updates: {}", e))
    })
    .await?
}
```

### Upgrade All Packages
```rust
use libscoop::{Session, operation, SyncOption};
use anyhow::Result;

async fn upgrade_all_packages() -> Result<()> {
    tokio::task::spawn_blocking(|| {
        let session = Session::new();
        
        operation::package_sync(
            &session,
            vec![],  // Empty = upgrade all
            vec![
                SyncOption::OnlyUpgrade,  // Only upgrade, don't install new
                SyncOption::AssumeYes,    // Auto-confirm
            ],
        ).map_err(|e| anyhow::anyhow!("Failed to upgrade packages: {}", e))
    })
    .await?
}
```

### Upgrade Specific Packages
```rust
use libscoop::{Session, operation, SyncOption};
use anyhow::Result;

async fn upgrade_packages(packages: &[&str]) -> Result<()> {
    let packages: Vec<String> = packages.iter().map(|s| s.to_string()).collect();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        let package_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
        
        operation::package_sync(
            &session,
            package_refs,
            vec![
                SyncOption::OnlyUpgrade,
                SyncOption::AssumeYes,
            ],
        ).map_err(|e| anyhow::anyhow!("Failed to upgrade packages: {}", e))
    })
    .await?
}
```

---

## Example 6: Wrapper Struct for Windows Package Manager

```rust
use libscoop::{Session, operation, SyncOption, QueryOption};
use anyhow::Result;

pub struct ScoopPackageManager;

impl ScoopPackageManager {
    pub async fn install(&self, package: &str) -> Result<()> {
        let package = package.to_string();
        
        tokio::task::spawn_blocking(move || {
            let session = Session::new();
            operation::package_sync(
                &session,
                vec![&package],
                vec![SyncOption::AssumeYes],
            ).map_err(|e| anyhow::anyhow!("Install failed: {}", e))
        })
        .await?
    }
    
    pub async fn remove(&self, package: &str) -> Result<()> {
        let package = package.to_string();
        
        tokio::task::spawn_blocking(move || {
            let session = Session::new();
            operation::package_sync(
                &session,
                vec![&package],
                vec![SyncOption::Remove, SyncOption::AssumeYes],
            ).map_err(|e| anyhow::anyhow!("Remove failed: {}", e))
        })
        .await?
    }
    
    pub async fn list_installed(&self) -> Result<Vec<String>> {
        tokio::task::spawn_blocking(|| {
            let session = Session::new();
            operation::package_query(&session, vec![], vec![], true)
                .map(|pkgs| pkgs.iter().map(|p| p.name().to_string()).collect())
                .map_err(|e| anyhow::anyhow!("List failed: {}", e))
        })
        .await?
    }
    
    pub async fn search(&self, query: &str) -> Result<Vec<(String, String)>> {
        let query = query.to_string();
        
        tokio::task::spawn_blocking(move || {
            let session = Session::new();
            operation::package_query(&session, vec![&query], vec![], false)
                .map(|pkgs| {
                    pkgs.iter()
                        .map(|p| (p.name().to_string(), p.bucket().to_string()))
                        .collect()
                })
                .map_err(|e| anyhow::anyhow!("Search failed: {}", e))
        })
        .await?
    }
    
    pub async fn list_upgradable(&self) -> Result<Vec<String>> {
        tokio::task::spawn_blocking(|| {
            let session = Session::new();
            operation::package_query(
                &session,
                vec![],
                vec![QueryOption::Upgradable],
                true,
            )
            .map(|pkgs| pkgs.iter().map(|p| p.name().to_string()).collect())
            .map_err(|e| anyhow::anyhow!("Check updates failed: {}", e))
        })
        .await?
    }
    
    pub async fn upgrade_all(&self) -> Result<()> {
        tokio::task::spawn_blocking(|| {
            let session = Session::new();
            operation::package_sync(
                &session,
                vec![],
                vec![SyncOption::OnlyUpgrade, SyncOption::AssumeYes],
            ).map_err(|e| anyhow::anyhow!("Upgrade failed: {}", e))
        })
        .await?
    }
}

// Usage
#[tokio::main]
async fn main() -> Result<()> {
    let scoop = ScoopPackageManager;
    
    // Install
    scoop.install("firefox").await?;
    
    // List
    let installed = scoop.list_installed().await?;
    println!("Installed: {:?}", installed);
    
    // Search
    let results = scoop.search("python").await?;
    println!("Search results: {:?}", results);
    
    // Check updates
    let upgradable = scoop.list_upgradable().await?;
    println!("Upgradable: {:?}", upgradable);
    
    // Upgrade
    scoop.upgrade_all().await?;
    
    // Remove
    scoop.remove("firefox").await?;
    
    Ok(())
}
```

---

## Error Handling Patterns

### Pattern 1: Simple Error Propagation
```rust
use libscoop::{Session, operation};
use anyhow::Result;

async fn simple_operation() -> Result<()> {
    tokio::task::spawn_blocking(|| {
        let session = Session::new();
        operation::package_sync(&session, vec!["pkg"], vec![])
            .map_err(|e| anyhow::anyhow!("{}", e))
    })
    .await?
}
```

### Pattern 2: Detailed Error Context
```rust
use libscoop::{Session, operation};
use anyhow::{Result, Context};

async fn operation_with_context(pkg: &str) -> Result<()> {
    let pkg = pkg.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        operation::package_sync(&session, vec![&pkg], vec![])
            .with_context(|| format!("Failed to sync package '{}'", pkg))
    })
    .await?
}
```

### Pattern 3: Match on Error Type
```rust
use libscoop::{Session, operation, Error};
use anyhow::Result;

async fn operation_with_error_handling(pkg: &str) -> Result<()> {
    let pkg = pkg.to_string();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        
        match operation::package_sync(&session, vec![&pkg], vec![]) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("Operation failed: {}", e);
                Err(anyhow::anyhow!("Package operation failed: {}", e))
            }
        }
    })
    .await?
}
```

---

## Performance Considerations

### Blocking Context
```rust
// libscoop is sync-only, so use spawn_blocking to avoid blocking the async runtime
tokio::task::spawn_blocking(|| {
    let session = Session::new();
    // ... libscoop operations
})
.await?
```

### Batch Operations
```rust
use libscoop::{Session, operation, SyncOption};

async fn install_multiple(packages: &[&str]) -> anyhow::Result<()> {
    let packages: Vec<String> = packages.iter().map(|s| s.to_string()).collect();
    
    tokio::task::spawn_blocking(move || {
        let session = Session::new();
        let package_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
        
        // Single operation for multiple packages is more efficient
        operation::package_sync(
            &session,
            package_refs,
            vec![SyncOption::AssumeYes],
        ).map_err(|e| anyhow::anyhow!("{}", e))
    })
    .await?
}
```

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_install_package() {
        let result = install_package("7zip").await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_list_installed() {
        let packages = list_installed_packages().await.unwrap();
        assert!(!packages.is_empty());
    }
    
    #[tokio::test]
    async fn test_search_packages() {
        let results = search_packages("python").await.unwrap();
        assert!(!results.is_empty());
    }
}
```

---

## Migration Checklist

- [ ] Add `libscoop = "0.1.0-beta.7"` to `Cargo.toml`
- [ ] Replace subprocess calls in `src/package_managers/windows.rs` line 819
- [ ] Replace subprocess calls in `src/package_managers/windows.rs` line 1053
- [ ] Wrap libscoop calls in `tokio::task::spawn_blocking()`
- [ ] Update error handling to use `anyhow::Result`
- [ ] Add tests for each operation
- [ ] Verify all operations work on Windows
- [ ] Remove subprocess-related code
- [ ] Update documentation
