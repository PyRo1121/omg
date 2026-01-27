# Phase 3: Error Handling Strategy

## Overview

This document describes the standardized error handling approach for OMG, following Rust best practices and ensuring consistent, user-friendly error reporting.

## Core Principles

### 1. Library vs Application Error Handling

**Library Code** (src/core, src/package_managers, src/runtimes):
- Use `thiserror::Error` for structured error types
- Return `Result<T, E>` where E is a specific error type
- Provide detailed error context for library consumers
- No unwrap() or expect() in production code paths

**Application Code** (src/cli, src/bin):
- Use `anyhow::Result<T>` for flexible error handling
- Add context with `.context()` for user-friendly messages
- Convert library errors to anyhow with automatic conversion
- Display helpful suggestions using `core::error::suggest_for_anyhow()`

### 2. Error Type Hierarchy

```rust
// Library error (core/error.rs)
#[derive(Error, Debug)]
pub enum OmgError {
    #[error("[OMG-E001] Package not found: {0}")]
    PackageNotFound(String),

    #[error("[OMG-E101] Version not found: {runtime} {version}")]
    VersionNotFound { runtime: String, version: String },

    // ... more variants with error codes
}

// Application usage (CLI)
use anyhow::{Context, Result};

pub async fn install_package(name: &str) -> Result<()> {
    let pkg = pm.info(name)
        .await
        .context(format!("Failed to find package: {name}"))?;

    pm.install(&[name.to_string()])
        .await
        .context("Installation failed")?;

    Ok(())
}
```

## Error Codes

Error codes follow the pattern `OMG-Ennn`:
- **E001-E099**: Package-related errors
- **E100-E199**: Runtime/version errors
- **E200-E299**: System/IO errors
- **E300-E399**: Network errors
- **E400-E499**: Configuration errors
- **E500-E599**: Daemon errors

## Allowed unwrap/expect Usage

### Acceptable Cases

1. **Test Code**: `#[cfg(test)]` modules can use unwrap()
   ```rust
   #[cfg(test)]
   mod tests {
       #[test]
       fn test_something() {
           let result = some_call().unwrap();
           assert_eq!(result, expected);
       }
   }
   ```

2. **Static Template Strings**: With `#[allow(clippy::expect_used)]`
   ```rust
   #[allow(clippy::expect_used)]
   pub fn progress_style() -> ProgressStyle {
       ProgressStyle::default_bar()
           .template("{bar:40}")
           .expect("valid template")
   }
   ```

3. **Documented Safety**: When proven safe with comment + allow attribute
   ```rust
   #[allow(clippy::unwrap_used)]
   pub async fn download_with_checksum() -> Result<()> {
       let hasher = if needs_checksum { Some(Sha256::new()) } else { None };

       // ... code that conditionally initializes hasher ...

       if needs_checksum {
           // Safe: hasher was initialized above
           let hash = hasher.unwrap().finalize();
       }
   }
   ```

4. **Mock/Testing Infrastructure**: With `#![allow(clippy::unwrap_used)]` at module level
   ```rust
   //! Mock package manager for testing
   #![allow(clippy::unwrap_used)]

   use std::sync::{Arc, Mutex};

   impl MockDb {
       pub fn add_package(&self, name: &str) {
           self.packages.lock().unwrap().insert(name.to_string(), ...);
       }
   }
   ```

### Prohibited Cases

1. **Production Library Code**: Never unwrap in public library APIs
   ```rust
   // ❌ BAD
   pub fn get_package(name: &str) -> Package {
       db.get(name).unwrap()  // Will panic on missing package!
   }

   // ✅ GOOD
   pub fn get_package(name: &str) -> Result<Package, OmgError> {
       db.get(name).ok_or_else(|| OmgError::PackageNotFound(name.to_string()))
   }
   ```

2. **User Input Handling**: Always validate and provide helpful errors
   ```rust
   // ❌ BAD
   let version: u32 = user_input.parse().unwrap();

   // ✅ GOOD
   let version: u32 = user_input.parse()
       .context("Invalid version number. Expected format: X.Y.Z")?;
   ```

3. **External Resources**: Network, filesystem, database operations
   ```rust
   // ❌ BAD
   let content = std::fs::read_to_string(path).unwrap();

   // ✅ GOOD
   let content = std::fs::read_to_string(path)
       .with_context(|| format!("Failed to read file: {}", path.display()))?;
   ```

## Error Context Best Practices

### Add Context at Each Layer

```rust
// Low-level: Core library function
pub fn read_database() -> Result<Database, OmgError> {
    let content = std::fs::read(DB_PATH)
        .map_err(OmgError::IoError)?;

    serde_json::from_slice(&content)
        .map_err(|e| OmgError::DatabaseError(e))
}

// Mid-level: Package manager function
pub async fn get_package_info(name: &str) -> Result<Package> {
    let db = read_database()
        .context("Failed to open package database")?;

    db.packages.get(name)
        .ok_or_else(|| anyhow!("Package '{}' not found", name))
        .context("Package lookup failed")
}

// High-level: CLI command
pub async fn install_command(name: &str) -> Result<()> {
    let info = get_package_info(name)
        .await
        .context(format!("Cannot install '{}': package not found", name))?;

    println!("Installing {} v{}", info.name, info.version);
    Ok(())
}
```

## Error Display and User Experience

### Helpful Error Messages

1. **Include Error Codes**: For searchability
   ```
   Error: [OMG-E001] Package not found: nonexistent-pkg

   💡 Try: omg search <query> to find available packages
   ```

2. **Provide Suggestions**: Use `OmgError::suggestion()` or `suggest_for_anyhow()`
   ```rust
   if let Err(err) = result {
       eprintln!("{}", err);
       if let Some(suggestion) = suggest_for_anyhow(&err) {
           eprintln!("\n💡 {}", suggestion);
       }
   }
   ```

3. **Context Chain**: Show full error context
   ```
   Error: Installation failed

   Caused by:
       0: Failed to download package
       1: Network error: connection timeout

   💡 Check your internet connection and try again
   ```

## Migration Checklist

- [x] Core error types use `thiserror::Error`
- [x] CLI/application code uses `anyhow::Result`
- [ ] Review all unwrap/expect calls in production code
- [ ] Add proper error context to library functions
- [ ] Test error messages for user-friendliness
- [ ] Document error handling patterns in contributing guide

## Current Status

### Library Modules (thiserror)
- ✅ `core::error` - Central error type with codes and suggestions
- ✅ Package managers use proper Result types
- ✅ Runtime managers use anyhow (application-facing)

### Application Modules (anyhow)
- ✅ CLI commands use anyhow::Result
- ✅ Binary entry points handle errors with suggestions
- ✅ Error context added with .context()

### Known Issues (Fixed in This Phase)

1. **http.rs Client Building**: Uses `.expect()` on static initialization
   - **Impact**: Could panic on startup with malformed config
   - **Priority**: Medium (unlikely to fail with default config)
   - **Fix**: Return Result from build_client() or document safety

2. **Mock Testing Code**: Uses unwrap() extensively
   - **Status**: Acceptable - has `#![allow(clippy::unwrap_used)]`
   - **Justification**: Testing infrastructure only

3. **Template Strings**: Multiple `.expect("valid template")`
   - **Status**: Acceptable - compile-time constants with allow attributes
   - **Justification**: Static strings that cannot fail at runtime

## Future Improvements

1. **Structured Error Responses**: Add machine-readable error format
2. **Error Recovery**: Implement retry logic for transient failures
3. **Error Telemetry**: Track error patterns in production (via Sentry)
4. **Localization**: Support translated error messages

## References

- [The Rust Programming Language - Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [thiserror documentation](https://docs.rs/thiserror/)
- [anyhow documentation](https://docs.rs/anyhow/)
- [Error Handling Patterns in Rust](https://nick.groenen.me/posts/rust-error-handling/)
