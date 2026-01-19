---
title: CLI Internals
sidebar_position: 35
description: CLI implementation details and optimization
---

# CLI Internals

The OMG CLI (`omg`) is optimized for sub-10ms response times on cached operations. It uses a hybrid sync/async model with daemon fallback for maximum performance.

## Binary Entry Point (`src/bin/omg.rs`)

### Main Function Structure

```rust
fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    
    // Handle elevation for root commands
    let needs_root = matches!(&cli.command, Commands::Sync | Commands::Clean { .. });
    if needs_root && !is_root() {
        elevate_if_needed()?;
    }
    
    // Route to command handlers
    match cli.command {
        // ... command routing
    }
}
```

### Memory Optimization

The CLI uses mimalloc for improved performance:
```rust
#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

Benefits:
- **Faster Allocations**: 1.5-2x faster than default allocator
- **Better Cache Locality**: Improved memory access patterns
- **Reduced Fragmentation**: Better for long-running processes

## Command Execution Model

### Hybrid Sync/Async Architecture

The CLI uses three execution paths:

1. **Pure Sync**: Sub-10ms for cached operations
2. **Sync with Async Fallback**: For operations that might need AUR
3. **Pure Async**: For network-bound operations

#### Sync Fast Path

```rust
Commands::Search { query, detailed, interactive } => {
    // PURE SYNC PATH (Sub-10ms)
    if !packages::search_sync_cli(&query, detailed, interactive)? {
        // Fallback to async if needed
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(packages::search(&query, detailed, interactive))?;
    }
}
```

Fast path characteristics:
- **Latency**: &lt;10ms for cached searches
- **No Tokio**: Direct function calls
- **Daemon IPC**: Uses sync client for sub-millisecond calls
- **Fallback**: Async only when cache miss

#### Info Command Pattern

```rust
Commands::Info { package } => {
    // 1. Try SYNC PATH (Official + Local)
    if !packages::info_sync(&package)? {
        // 2. Fallback to ASYNC PATH (AUR)
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(packages::info_aur(&package))?;
    }
}
```

Two-stage approach:
1. **Sync Check**: Local cache and official index
2. **Async Fallback**: AUR network request if needed

### Runtime Management

Runtime commands use async for network operations:

```rust
Commands::Use { runtime, version } => {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(runtimes::use_version(&runtime, version.as_deref()))?;
}

Commands::List { runtime, available } => {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(runtimes::list_versions(runtime.as_deref(), available))?;
}
```

Runtime operations:
- **Use**: Download and switch versions
- **List**: Show installed/available versions
- **Network Bound**: Always async (downloads required)

## Daemon Integration

### Daemon Command Handling

```rust
pub fn daemon(foreground: bool) -> Result<()> {
    if foreground {
        println!("{} Run 'omgd' directly for daemon mode", style::info("→"));
    } else {
        // Start daemon in background
        let status = Command::new("omgd")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match status {
            Ok(_) => println!("{} Daemon started", style::success("✓")),
            Err(e) => println!("{} Failed to start daemon: {}", style::error("✗"), e),
        }
    }
    Ok(())
}
```

Daemon startup options:
- **Background**: Default behavior, detached process
- **Foreground**: User runs `omgd` directly
- **Output Suppression**: Background daemon is silent

### IPC Client Selection

The CLI automatically selects sync or async IPC:

```rust
// For cached operations - sync client
let mut client = DaemonClient::connect_sync()?;
let response = client.call_sync(Request::Status { id: 1 })?;

// For network operations - async client
let mut client = DaemonClient::connect().await?;
let response = client.call(Request::Search { id: 1, query, limit: Some(50) }).await?;
```

Selection criteria:
- **Sync**: Cached data, status checks, local info
- **Async**: Searches, AUR operations, installs

## Command Categories

### Package Management Commands

#### Search (`omg search`)
- **Fast Path**: Sync daemon cache check
- **Fallback**: Async if cache miss
- **Interactive**: Optional TUI interface
- **Detailed**: Show extended package info

#### Install (`omg install`)
- **Always Async**: Network downloads required
- **AUR Support**: Builds in user directory
- **Dependencies**: Automatic resolution
- **Progress**: Real-time progress bars

#### Info (`omg info`)
- **Two-stage**: Sync local, async AUR
- **Rich Output**: Dependencies, size, etc.
- **Sources**: Official repos + AUR

#### Clean (`omg clean`)
- **Root Required**: System-wide operations
- **Options**: Orphans, cache, AUR, all
- **Safe**: Confirmation prompts

### Runtime Management Commands

#### Use (`omg use`)
- **Download**: Fetch if not installed
- **Symlink**: Update 'current' link
- **PATH**: Shell hook updates
- **Project**: Version file detection

#### List (`omg list`)
- **Installed**: Local versions
- **Available**: Remote versions
- **Status**: Active version marked

### System Commands

#### Sync (`omg sync`)
- **Root Required**: System package sync
- **Databases**: Refresh all repos
- **Fast**: Direct libalpm calls

#### Status (`omg status`)
- **Sync Path**: Daemon cached status
- **System Info**: OS, kernel, arch
- **Packages**: Counts, vulnerabilities
- **Runtimes**: Active versions

## Performance Optimizations

### Sub-10ms Target

The CLI is optimized for sub-10ms response times:

1. **Sync First**: Avoid Tokio startup overhead
2. **Daemon Cache**: Pre-computed results
3. **Direct libalpm**: No subprocess overhead
4. **Minimal Allocation**: Reuse buffers

### Cold Path Optimization

When cache misses occur:
1. **Lazy Tokio**: Runtime created only when needed
2. **Parallel Operations**: Concurrent downloads
3. **Progress Feedback**: User sees activity
4. **Intelligent Fallback**: Try fastest sources first

### Memory Efficiency

- **Stack Allocation**: Prefer stack over heap
- **String Reuse**: Cow for shared strings
- **Buffer Pooling**: Reuse network buffers
- **Minimal Dependencies**: Reduce binary size

## Error Handling

### Command-Level Errors

Each command handles errors appropriately:

```rust
match packages::search_sync_cli(&query, detailed, interactive) {
    Ok(true) => return Ok(()),  // Success
    Ok(false) => {
        // Continue to async path
    }
    Err(e) => {
        eprintln!("{} Search failed: {}", style::error("✗"), e);
        return Err(e);
    }
}
```

Error categories:
- **User Errors**: Invalid arguments, permission denied
- **Network Errors**: AUR unavailable, download failed
- **System Errors**: Daemon not running, disk full
- **Package Errors**: Not found, conflicts

### Graceful Degradation

The CLI degrades gracefully:
1. **Daemon Unavailable**: Fall back to direct libalpm
2. **AUR Down**: Use official packages only
3. **Network Issues**: Use cached data
4. **Partial Failures**: Continue with available data

## Shell Integration

### Hook System

The CLI generates shell hooks for PATH management:

```rust
pub fn print_hook(shell: &str) -> Result<()> {
    match shell {
        "bash" | "zsh" => {
            println!("# OMG hook for {}", shell);
            println!("export PATH=\"{}:$PATH\"", get_omg_bin_path()?);
            println!("_omg_hook() {{ eval \"$(omg hook-env {})\"; }}", shell);
            println!("preexec_functions+=(_omg_hook)");
        }
        "fish" => {
            println!("# OMG hook for fish");
            println!("set -gx PATH \"{}\" $PATH", get_omg_bin_path()?);
            println!("function _omg_hook --on-variable PWD");
            println!("    omg hook-env fish | source");
            println!("end");
        }
        _ => bail!("Unsupported shell: {}", shell),
    }
    Ok(())
}
```

Hook features:
- **Automatic**: Detects version files
- **Fast**: Sub-millisecond execution
- **Non-intrusive**: Minimal impact on shell startup
- **Smart**: Only updates when needed

### Environment Updates

```rust
pub fn hook_env(shell: &str) -> Result<()> {
    let runtime_versions = detect_runtime_versions()?;
    
    match shell {
        "bash" | "zsh" => {
            for (runtime, version) in runtime_versions {
                let bin_path = get_runtime_bin_path(runtime, &version)?;
                println!("export PATH=\"{}:$PATH\"", bin_path);
            }
        }
        // ... other shells
    }
    Ok(())
}
```

## Configuration Management

### Config Command

```rust
pub fn config(key: Option<&str>, value: Option<&str>) -> Result<()> {
    match (key, value) {
        (Some(k), Some(v)) => {
            // Set configuration value
            config::set(k, v)?;
            println!("{} Set {} = {}", style::success("✓"), k, v);
        }
        (Some(k), None) => {
            // Get configuration value
            if let Some(v) = config::get(k)? {
                println!("{} {} = {}", style::info("→"), k, v);
            } else {
                println!("{} {} not set", style::warning("→"), k);
            }
        }
        (None, None) => {
            // Show all configuration
            for (k, v) in config::list()? {
                println!("{} {} = {}", style::info("→"), k, v);
            }
        }
    }
    Ok(())
}
```

Configuration options:
- **shims.enabled**: Use shims instead of PATH
- **data_dir**: Custom data directory
- **socket**: Daemon socket path
- **default_shell**: Default for hooks

## Output Formatting

### Colored Output

The CLI uses `colored` for user-friendly output:

```rust
use colored::*;
use omg_lib::cli::style;

println!("{} Package installed", style::success("✓"));
println!("{} Error: {}", style::error("✗"), error);
println!("{} Information", style::info("→"));
println!("{} Warning", style::warning("!"));
```

Style guide:
- **Success**: Green with checkmark
- **Error**: Red with X mark
- **Info**: Blue with arrow
- **Warning**: Yellow with exclamation

### Progress Indicators

For long-running operations:
```rust
use indicatif::{ProgressBar, ProgressStyle};

let bar = ProgressBar::new(total);
bar.set_style(
    ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .progress_chars("#>-")
);

for item in items {
    process(item)?;
    bar.inc(1);
}
bar.finish();
```

## Runtime Display Fallback

If the daemon is down, the CLI probes active runtimes locally using `probe_version`:

```rust
pub fn probe_version(runtime: &str) -> Option<String> {
    let current_link = DATA_DIR.join("versions").join(runtime).join("current");
    
    std::fs::read_link(&current_link)
        .ok()
        .and_then(|p| p.file_name()
            .map(|n| n.to_string_lossy().to_string()))
}
```

This ensures the CLI remains functional even when the daemon is unavailable.

## Testing Infrastructure

### Command Testing

Commands are tested with:
- **Unit Tests**: Individual function testing
- **Integration Tests**: Full command execution
- **Mock Servers**: For network operations
- **Temp Directories**: Isolated test environments

### Performance Testing

Response time validation:
```rust
#[test]
fn test_search_performance() {
    let start = Instant::now();
    packages::search_sync_cli("test", false, false).unwrap();
    let duration = start.elapsed();
    assert!(duration < Duration::from_millis(10));
}
```

Benchmarks:
- **Search**: &lt;10ms cached, &lt;100ms uncached
- **Info**: &lt;5ms local, &lt;200ms AUR
- **Status**: &lt;5ms via daemon
- **Install**: Depends on package size

## Future Enhancements

### Performance Improvements

1. **Persistent Tokio Runtime**: Reuse across commands
2. **Smart Caching**: Predictive cache warming
3. **Parallel Commands**: Execute multiple operations
4. **Compression**: Reduce IPC payload size

### User Experience

1. **Interactive TUI**: Enhanced interactive mode
2. **Fuzzy Search**: Global package search
3. **Suggestions**: Smart command suggestions
4. **History**: Command history and recall

### Developer Features

1. **Plugin System**: Extensible command architecture
2. **Scripting API**: Programmatic access
3. **JSON Output**: Machine-readable results
4. **Completion**: Enhanced shell completion
