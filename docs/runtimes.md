# Runtime Management

OMG provides pure Rust implementations for managing multiple language runtimes with sub-millisecond switching. It supports version files, auto-detection, and isolated installations.

## Supported Runtimes

### Native Runtimes (Pure Rust)

OMG natively supports seven major runtimes with pure Rust implementations:

| Runtime | Manager | Version Files | Binary Names |
|---------|---------|---------------|--------------|
| Node.js | `NodeManager` | `.nvmrc`, `.node-version` | `node`, `npm`, `npx` |
| Python | `PythonManager` | `.python-version` | `python3`, `pip` |
| Go | `GoManager` | `.go-version` | `go` |
| Rust | `RustManager` | `.rust-version`, `rust-toolchain.toml` | `rustc`, `cargo` |
| Ruby | `RubyManager` | `.ruby-version` | `ruby`, `gem` |
| Java | `JavaManager` | `.java-version` | `java`, `javac` |
| Bun | `BunManager` | `.bun-version` | `bun`, `bunx` |

### Extended Runtimes (via Built-in Mise)

For runtimes not natively supported, OMG includes a **built-in mise manager** that automatically downloads and manages mise when needed. This provides access to 100+ additional runtimes:

| Category | Runtimes |
|----------|----------|
| **JavaScript/TypeScript** | Deno, Bun (alternative) |
| **Functional** | Elixir, Erlang, Haskell, OCaml, Clojure |
| **Systems** | Zig, Nim, Crystal, D |
| **Mobile/Native** | Swift, Kotlin, Flutter, Dart |
| **Enterprise** | .NET, PHP, Perl, Lua |
| **Data Science** | Julia, R |
| **And many more...** | 100+ runtimes supported |

**How it works:**
1. When you run `omg use <runtime>` for a non-native runtime, OMG checks if mise is available
2. If mise isn't installed, OMG automatically downloads it to `~/.local/share/omg/mise/`
3. OMG then uses mise to install and manage the requested runtime
4. No manual mise installation required - it's completely seamless

### Runtime Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Node,
    Bun,
    Python,
    Go,
    Rust,
    Ruby,
    Java,
}
```

## Runtime Manager Trait

All runtime managers implement the common `RuntimeManager` trait:

```rust
#[async_trait]
pub trait RuntimeManager: Send + Sync {
    /// Get the runtime type
    fn runtime(&self) -> Runtime;
    
    /// List available versions for download
    async fn list_available(&self) -> Result<Vec<String>>;
    
    /// List installed versions
    fn list_installed(&self) -> Result<Vec<RuntimeVersion>>;
    
    /// Install a specific version
    async fn install(&self, version: &str) -> Result<()>;
    
    /// Uninstall a specific version
    fn uninstall(&self, version: &str) -> Result<()>;
    
    /// Use a specific version (update 'current' symlink)
    fn use_version(&self, version: &str) -> Result<()>;
}
```

### Trait Implementation Pattern

Each manager follows this structure:
```rust
impl RuntimeManager for NodeManager {
    fn runtime(&self) -> Runtime { Runtime::Node }
    
    async fn list_available(&self) -> Result<Vec<String>> {
        // Fetch from Node.js dist API
    }
    
    fn list_installed(&self) -> Result<Vec<RuntimeVersion>> {
        // Scan versions directory
    }
    
    async fn install(&self, version: &str) -> Result<()> {
        // Download, extract, verify
    }
    
    fn uninstall(&self, version: &str) -> Result<()> {
        // Remove directory, update symlinks
    }
    
    fn use_version(&self, version: &str) -> Result<()> {
        // Update 'current' symlink
    }
}
```

## Active Version Probing

### Zero-Allocation Probing

The daemon probes active versions efficiently:

```rust
pub fn probe_version(runtime: &str) -> Option<String> {
    let current_link = DATA_DIR.join("versions").join(runtime).join("current");
    
    std::fs::read_link(&current_link)
        .ok()
        .and_then(|p| p.file_name()
            .map(|n| n.to_string_lossy().to_string()))
}
```

Probing characteristics:
- **Performance**: O(1) syscall, no allocations
- **Method**: Read symlink target
- **Location**: `<data_dir>/versions/<runtime>/current`
- **Fallback**: None if not installed

### Directory Structure

Each runtime follows this layout:
```
<XDG_DATA_DIR>/omg/versions/
├── node/
│   ├── 18.17.0/
│   │   ├── bin/
│   │   │   ├── node
│   │   │   ├── npm
│   │   │   └── npx
│   │   └── lib/
│   ├── 20.5.0/
│   └── current -> 20.5.0
├── python/
│   ├── 3.11.4/
│   │   ├── bin/
│   │   │   ├── python3
│   │   │   └── pip
│   │   └── lib/
│   └── current -> 3.11.4
└── ...
```

## Version Files

### Supported Version Files

Each runtime supports project-specific version files:

```rust
impl Runtime {
    pub const fn version_file(&self) -> &'static str {
        match self {
            Self::Node => ".nvmrc",
            Self::Bun => ".bun-version",
            Self::Python => ".python-version",
            Self::Go => ".go-version",
            Self::Rust => ".rust-version",
            Self::Ruby => ".ruby-version",
            Self::Java => ".java-version",
        }
    }
}
```

### Version File Detection

The CLI detects version files in this order:
1. **Current directory**: `.nvmrc`, `.python-version`, etc.
2. **Parent directories**: Walk up to project root
3. **Global default**: Falls back to system or configured version

### Version File Formats

#### Node.js (.nvmrc)
```
18.17.0
lts/*
lts/hydrogen
```

#### Python (.python-version)
```
3.11.4
3.10
```

#### Go (.go-version)
```
1.21.0
1.20.8
```

#### Rust (.rust-version)
```
1.72.0
stable
nightly
```

## Runtime Managers

### Node.js Manager

#### Features
- **Source**: Node.js official dist server
- **Formats**: tar.xz (Linux), tar.gz (macOS), zip (Windows)
- **Verification**: SHASUM256 signature verification
- **Binary Names**: `node`, `npm`, `npx`

#### Implementation Details
```rust
pub struct NodeManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}
```

#### Installation Process
1. Download from `https://nodejs.org/dist/v{version}/`
2. Verify SHASUM256 signature
3. Extract to versions directory
4. Update permissions
5. Update `current` symlink if requested

### Python Manager

#### Features
- **Source**: Python-build-standalone releases
- **Formats**: tar.gz (Linux), zip (macOS/Windows)
- **Variants**: CPython, PyPy support
- **Binary Names**: `python3`, `pip`

#### Implementation Details
```rust
pub struct PythonManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}
```

#### Installation Process
1. Fetch from GitHub releases
2. Verify GPG signature
3. Extract with proper permissions
4. Install pip if needed
5. Update symlinks

### Go Manager

#### Features
- **Source**: Go official downloads
- **Formats**: tar.gz (all platforms)
- **Architecture**: Multiple arch support (amd64, arm64)
- **Binary Names**: `go`

#### Implementation Details
```rust
pub struct GoManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}
```

#### Installation Process
1. Download from `https://go.dev/dl/`
2. Extract to `go{version}` directory
3. Create `go` symlink in bin/
4. Update `current` symlink

### Rust Manager

#### Features
- **Source**: Rust official releases
- **Formats**: tar.gz (Linux/macOS), exe (Windows)
- **Components**: rustc, cargo, rust-std
- **Binary Names**: `rustc`, `cargo`
- **Toolchain Files**: `rust-toolchain.toml` with channels, components, targets, and profiles

#### Implementation Details
```rust
pub struct RustManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}
```

#### Installation Process
1. Download toolchain archives from `https://static.rust-lang.org/dist/`
2. Extract components directly (pure Rust, no rustup)
3. Track installed components/targets in `.omg-toolchain.toml`
4. Update `current` symlink for active toolchain

### Ruby Manager

#### Features
- **Source**: Ruby-builder releases
- **Formats**: tar.gz (Linux/macOS), zip (Windows)
- **Variants**: MRI, JRuby support
- **Binary Names**: `ruby`, `gem`

#### Implementation Details
```rust
pub struct RubyManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}
```

### Java Manager

#### Features
- **Source**: Adoptium (Eclipse Temurin)
- **Formats**: tar.gz (Linux), tar.gz (macOS), zip (Windows)
- **JVM**: HotSpot implementation
- **Binary Names**: `java`, `javac`

#### Implementation Details
```rust
pub struct JavaManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}
```

### Bun Manager

#### Features
- **Source**: Bun releases
- **Formats**: tar.gz (Linux/macOS), zip (Windows)
- **Performance**: JavaScript runtime and bundler
- **Binary Names**: `bun`, `bunx`

#### Implementation Details
```rust
pub struct BunManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}
```

## Runtime Metadata

### RuntimeVersion Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeVersion {
    pub runtime: Runtime,
    pub version: String,
    pub installed: bool,
    pub active: bool,
    pub path: Option<std::path::PathBuf>,
}
```

### Version Information

Each version provides:
- **Runtime Type**: Enum value
- **Version String**: SemVer format
- **Installation Status**: Whether installed
- **Active Status**: Currently in use
- **Path**: Installation directory

### Version Comparison

Runtime managers implement version comparison:
```rust
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<u32> = a.split('.')
        .filter_map(|p| p.parse().ok())
        .collect();
    let b_parts: Vec<u32> = b.split('.')
        .filter_map(|p| p.parse().ok())
        .collect();
    
    // Compare major.minor.patch
    for i in 0..3 {
        let a_part = a_parts.get(i).unwrap_or(&0);
        let b_part = b_parts.get(i).unwrap_or(&0);
        if a_part != b_part {
            return a_part.cmp(&b_part);
        }
    }
    
    std::cmp::Ordering::Equal
}
```

## Performance Optimizations

### Sub-millisecond Switching

Version switching is optimized for speed:
- **Symlink Updates**: O(1) atomic operation
- **No Environment Changes**: Uses PATH modification
- **Cache Friendly**: Version probes cached
- **Parallel Installs**: Async downloads

### Memory Efficiency

- **Zero-copy Probing**: Direct filesystem reads
- **Lazy Loading**: Load runtimes on demand
- **Shared Storage**: Common directories for all runtimes
- **Minimal Overhead**: <1MB per runtime

### Download Optimization

- **Parallel Downloads**: Multiple runtimes concurrently
- **Resume Support**: Continue interrupted downloads
- **Compression**: Reduced bandwidth usage
- **Caching**: Local mirror of available versions

## Integration Points

### Shell Integration

OMG modifies PATH via shell hooks:
```bash
# Generated by OMG hook
export PATH="$HOME/.local/share/omg/versions/node/current/bin:$PATH"
export PATH="$HOME/.local/share/omg/versions/python/current/bin:$PATH"
```

### IDE Integration

Version files are automatically detected:
- **VS Code**: Detects `.nvmrc`, `.python-version`
- **IntelliJ**: Supports `.java-version`
- **Vim/Neovim**: Custom plugins available

### CI/CD Integration

Runtime installation in CI:
```bash
# Install specific versions
omg use node 18.17.0
omg use python 3.11.4

# Auto-detect from version files
omg install  # Reads .nvmrc, .python-version, etc.
```

## Error Handling

### Installation Errors

1. **Network Failures**: Retry with exponential backoff
2. **Checksum Mismatch**: Abort with clear error
3. **Permission Errors**: Suggest sudo or fix permissions
4. **Disk Space**: Check before download

### Version Errors

1. **Invalid Version**: Validate against available versions
2. **Not Installed**: Offer to install
3. **Already Installed**: Skip or force reinstall
4. **In Use**: Prevent uninstalling active version

### Symlink Errors

1. **Broken Symlinks**: Detect and repair
2. **Permission Denied**: Check directory permissions
3. **Race Conditions**: Atomic operations prevent
4. **Cleanup**: Remove stale symlinks

## Security Considerations

### Download Verification

All downloads are verified:
- **Checksums**: SHA256 verification
- **Signatures**: GPG for critical packages
- **HTTPS**: TLS for all downloads
- **Mirrors**: Official sources only

### Isolation

- **User Installation**: No system-wide changes
- **Sandboxed**: Each version isolated
- **Permissions**: Minimal required permissions
- **Cleanup**: Complete removal on uninstall

### Path Security

- **Safe PATH**: Controlled modification
- **No Sudo**: User-level installation
- **Verification**: Check binary integrity
- **Audit Trail**: Log all operations

## MiseManager

The `MiseManager` handles the built-in mise integration:

```rust
pub struct MiseManager {
    bin_dir: PathBuf,      // ~/.local/share/omg/mise/
    mise_bin: PathBuf,     // ~/.local/share/omg/mise/mise
    client: reqwest::Client,
}

impl MiseManager {
    /// Check if mise is available (bundled or system)
    pub fn is_available(&self) -> bool;
    
    /// Get path to mise binary
    pub fn mise_path(&self) -> PathBuf;
    
    /// Download and install mise if not available
    pub async fn ensure_installed(&self) -> Result<()>;
    
    /// Use a specific version of a runtime via mise
    pub fn use_version(&self, runtime: &str, version: &str) -> Result<()>;
    
    /// Get current version of a runtime
    pub fn current_version(&self, runtime: &str) -> Result<Option<String>>;
    
    /// List installed runtimes via mise
    pub fn list_installed(&self) -> Result<Vec<String>>;
}
```

### Auto-Installation Flow

When a user requests a non-native runtime:

```
omg use deno 1.40.0
     │
     ▼
┌─────────────────────┐
│ Is runtime native?  │──Yes──▶ Use native manager
└─────────────────────┘
     │ No
     ▼
┌─────────────────────┐
│ Is mise available?  │──Yes──▶ Use mise
└─────────────────────┘
     │ No
     ▼
┌─────────────────────┐
│ Download mise from  │
│ GitHub releases     │
└─────────────────────┘
     │
     ▼
┌─────────────────────┐
│ Install to          │
│ ~/.local/share/omg/ │
│ mise/mise           │
└─────────────────────┘
     │
     ▼
┌─────────────────────┐
│ Use mise to install │
│ requested runtime   │
└─────────────────────┘
```

## Future Enhancements

### Additional Native Runtimes

With mise built-in, all 100+ runtimes are now available. Future work focuses on:
- **Performance**: Native implementations for popular runtimes
- **Integration**: Deeper mise configuration support

### Advanced Features

1. **Project Isolation**: Per-project environments
2. **Version Locking**: Pin exact versions
3. **Automatic Updates**: Security patching
4. **Telemetry**: Usage analytics

### Performance Improvements

1. **Lazy Loading**: Load runtimes on first use
2. **Compression**: Reduce disk usage
3. **Deduplication**: Shared files between versions
4. **Caching**: Aggressive version caching
