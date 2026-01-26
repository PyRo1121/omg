# Rust 2026 Modernization Design

**Date**: 2026-01-26
**Status**: Approved - Ready for Implementation
**Timeline**: 3-4 week sprint

---

## 🎯 Executive Summary

Transform the OMG codebase from AI-generated technical debt into idiomatic, modern Rust 2026. This is a comprehensive modernization addressing unsafe code, panic points, async patterns, type ergonomics, and architectural inconsistencies.

**Key Metrics:**
- **Files to modernize**: 148 Rust files (~50K lines)
- **Unsafe code blocks**: 9 files requiring rewrites
- **Panic points**: 254 (210 unwrap + 44 expect)
- **Async functions**: 251 across 69 files
- **Modules**: 10 top-level (cli, core, daemon, package_managers, runtimes, etc.)

**Success Criteria:**
- ✅ Zero unsafe code (except FFI where unavoidable)
- ✅ Zero unwrap/expect in production code
- ✅ No blocking code in async contexts
- ✅ All clippy lints pass (pedantic + nursery)
- ✅ 100% test pass rate
- ✅ No performance regressions

---

## 🧭 Core Principles

### 1. Zero Unsafe
**Goal**: Eliminate all unsafe blocks using safe abstractions

**Current State:**
- 9 files with unsafe code
- Patterns: mmap access, rkyv unchecked deserialization, env var manipulation in tests

**Target State:**
- Use safe wrappers: `zerocopy` for serialization, `safe-transmute` where needed
- Test env vars: use `serial_test` with proper cleanup
- Memory mapping: use checked deserialization with proper error handling

### 2. Zero Panics
**Goal**: No unwrap/expect in production code paths

**Current State:**
- 210 `.unwrap()` calls
- 44 `.expect()` calls
- Scattered across all modules

**Target State:**
- Convert to `?` operator with proper error types
- Use `unwrap_or_default()` / `unwrap_or_else()` where safe
- Document any remaining test-only panics with `#[cfg(test)]`

### 3. Async-First Architecture
**Goal**: Proper structured concurrency, no blocking in async

**Current State:**
- 251 async functions across 69 files
- Unknown amount of blocking code in async contexts
- Mixed patterns (spawn, spawn_blocking, etc.)

**Target State:**
- All I/O operations use async variants
- Blocking work uses `tokio::task::spawn_blocking`
- Clear async/sync boundaries documented
- Use `tokio::select!` and structured concurrency patterns

### 4. Type-Driven Design
**Goal**: Use the type system to prevent bugs

**Current State:**
- Excessive cloning for simplicity
- Generic types without clear constraints
- Missing newtype wrappers for domain concepts

**Target State:**
- Use `Cow<'_, T>` for read-mostly data
- Use `Arc<T>` for shared ownership, not `Clone`
- Newtype wrappers for domain primitives (PackageName, Version, etc.)
- Generic constraints that express actual requirements

### 5. YAGNI Ruthlessly
**Goal**: Remove unnecessary complexity

**Patterns to Eliminate:**
- Abstraction layers with single implementations
- Generic types that are never used polymorphically
- Trait hierarchies that could be simple functions
- Over-engineered error handling for simple cases

### 6. Consistent Patterns
**Goal**: One way to do things

**Standardize:**
- Error handling: `anyhow` for applications, `thiserror` for libraries
- Async patterns: Always spawn_blocking for CPU work
- Logging: `tracing` everywhere (no mix of log/tracing)
- Testing: Common test utilities, fixtures, and patterns

---

## 📋 Phase 1: Safety First (Week 1)

### Priority: Eliminate Existential Risk

**Goal**: Remove all unsafe code and critical panic points

### 1.1 Audit & Document Unsafe Code

**Files to review:**
1. `src/package_managers/aur_index.rs` - mmap + rkyv unchecked
2. `src/package_managers/debian_db.rs` - mmap + rkyv unchecked
3. `src/package_managers/mock.rs` - unsafe blocks
4. `src/cli/style.rs` - env var manipulation
5. `src/cli/why.rs` - unsafe blocks
6. `src/config/settings.rs` - unsafe blocks
7. `src/core/env/fingerprint.rs` - unsafe blocks
8. `src/core/fast_status.rs` - already safe with zerocopy ✅
9. `src/package_managers/aur.rs` - "unsafe" in strings only ✅

**Action Items:**
- [ ] Read each file, identify all unsafe blocks
- [ ] Document why unsafe was used
- [ ] Research safe alternatives
- [ ] Create checklist for Phase 1 implementation

### 1.2 Eliminate Unsafe: Memory-Mapped Files

**Problem**: `aur_index.rs` and `debian_db.rs` use:
```rust
let mmap = unsafe { Mmap::map(&file)? };
unsafe { rkyv::access_unchecked::<T>(&self.mmap) }
```

**Solution**:
```rust
// Use safe checked deserialization
let mmap = Mmap::map(&file)?;  // Already safe in memmap2
let archived = rkyv::check_archived_root::<T>(&mmap)
    .context("Corrupted package index")?;
```

**Benefits:**
- Validates data integrity before access
- Catches corruption early
- Only ~5-10% slower than unchecked

**Files to modify:**
- `src/package_managers/aur_index.rs`
- `src/package_managers/debian_db.rs`

### 1.3 Eliminate Unsafe: Test Environment Variables

**Problem**: `src/cli/style.rs` uses unsafe env var manipulation in tests:
```rust
unsafe { env::set_var("NO_COLOR", "1") };
```

**Solution**: Use `serial_test` with proper isolation:
```rust
#[test]
#[serial]
fn test_no_color() {
    temp_env::with_var("NO_COLOR", Some("1"), || {
        // test logic
    });
}
```

**Files to modify:**
- `src/cli/style.rs`

**Add dependency:**
- `temp-env = "0.3"` (for scoped env var manipulation)

### 1.4 Eliminate Critical Panic Points

**Target**: Top 100 unwrap/expect calls in hot paths

**Priority files** (highest impact):
1. `src/daemon/handlers.rs` - daemon crashes affect all operations
2. `src/core/packages/service.rs` - core business logic
3. `src/package_managers/pacman_db.rs` - critical data access
4. `src/cli/commands.rs` - user-facing command dispatch

**Strategy:**
```rust
// BEFORE: Panic
let value = map.get(key).unwrap();

// AFTER: Proper error handling
let value = map.get(key)
    .ok_or_else(|| anyhow!("Missing key: {key}"))?;

// OR: Safe default
let value = map.get(key).unwrap_or_default();
```

**Action Items:**
- [ ] Generate list of all unwrap/expect by file
- [ ] Categorize: must-fix vs. can-document
- [ ] Fix top 100 in critical paths
- [ ] Document remaining with `// SAFETY:` comments

### 1.5 Phase 1 Verification

**Quality Gates:**
- [ ] Zero unsafe blocks (except FFI if needed)
- [ ] Zero unwrap in daemon/core modules
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Security audit passes
- [ ] Benchmark: no performance regression

---

## 📋 Phase 2: Async & Performance (Week 2)

### Priority: Correct Async Patterns & Type Efficiency

### 2.1 Audit Async/Await Patterns

**Goal**: Identify blocking code in async contexts

**Audit checklist:**
- [ ] Find all `std::fs::*` in async functions → use `tokio::fs::*`
- [ ] Find all `std::io::Read/Write` in async → use `tokio::io::*`
- [ ] Find CPU-intensive loops in async → move to `spawn_blocking`
- [ ] Find `thread::sleep` in async → use `tokio::time::sleep`
- [ ] Find synchronous channel ops → use `tokio::sync` channels

**Tools:**
```bash
# Find blocking I/O in async functions
rg "std::fs::" --type rust | grep "async fn"
rg "std::io::Read" --type rust | grep "async fn"
rg "thread::sleep" --type rust | grep "async fn"
```

### 2.2 Fix Blocking in Async

**Pattern 1: File I/O**
```rust
// BEFORE: Blocks async runtime
async fn read_config() -> Result<Config> {
    let content = std::fs::read_to_string("config.toml")?;
    toml::from_str(&content)?
}

// AFTER: Proper async
async fn read_config() -> Result<Config> {
    let content = tokio::fs::read_to_string("config.toml").await?;
    toml::from_str(&content)?
}
```

**Pattern 2: CPU-Intensive Work**
```rust
// BEFORE: Blocks async runtime
async fn parse_packages() -> Result<Vec<Package>> {
    let data = fetch_data().await?;
    expensive_parse(data)  // Blocks!
}

// AFTER: Offload to blocking thread
async fn parse_packages() -> Result<Vec<Package>> {
    let data = fetch_data().await?;
    tokio::task::spawn_blocking(move || {
        expensive_parse(data)
    }).await?
}
```

**Priority files:**
- `src/package_managers/pacman_db.rs` - heavy parsing
- `src/package_managers/aur.rs` - build operations
- `src/daemon/handlers.rs` - request handling

### 2.3 Type Ergonomics: Reduce Cloning

**Goal**: Use Arc/Cow strategically instead of Clone

**Pattern 1: Shared State**
```rust
// BEFORE: Excessive cloning
#[derive(Clone)]
struct PackageService {
    cache: HashMap<String, Package>,  // Clone is expensive
}

// AFTER: Shared ownership
struct PackageService {
    cache: Arc<DashMap<String, Package>>,  // Lock-free shared access
}
```

**Pattern 2: Read-Mostly Data**
```rust
// BEFORE: Always clones
fn process(config: Config) {
    // config is owned but rarely modified
}

// AFTER: Borrow when possible
fn process(config: &Config) {
    // Or use Cow for conditional ownership
}
```

**Pattern 3: Large String Data**
```rust
// BEFORE: Clones strings everywhere
struct Package {
    name: String,
    description: String,  // Often 1KB+
}

// AFTER: Use Arc for large immutable strings
struct Package {
    name: String,
    description: Arc<str>,  // Shared, cheap to clone
}
```

**Action Items:**
- [ ] Profile clone hotspots with `cargo flamegraph`
- [ ] Convert shared caches to Arc<DashMap>
- [ ] Use Cow in APIs that sometimes need to modify
- [ ] Use Arc<str> for large immutable strings

### 2.4 Modern Trait Patterns

**Use `trait-variant` everywhere:**

```rust
// BEFORE: Manual async trait duplication
#[async_trait]
trait PackageManager {
    async fn install(&self, pkg: &str) -> Result<()>;
}

// AFTER: Use trait-variant (already in dependencies!)
#[trait_variant::make(Send)]
trait PackageManager {
    async fn install(&self, pkg: &str) -> Result<()>;
}
// Generates both sync and Send async versions
```

**Files to convert:**
- `src/package_managers/mod.rs` - main trait
- `src/runtimes/common.rs` - runtime traits
- `src/core/packages/service.rs` - service traits

### 2.5 Phase 2 Verification

**Quality Gates:**
- [ ] No std::fs/io in async functions
- [ ] CPU work uses spawn_blocking
- [ ] Clone count reduced by 50%+ in hot paths
- [ ] trait-variant used for all async traits
- [ ] All tests pass
- [ ] Benchmarks show 10-20% speedup

---

## 📋 Phase 3: Architecture & Consistency (Weeks 3-4)

### Priority: Clean Architecture & Remove AI Slop

### 3.1 Module Structure Refinement

**Current structure:**
```
src/
├── bin/          (4 binaries)
├── cli/          (24 files) - too flat
├── core/         (security, packages, testing, etc.)
├── daemon/       (server, handlers, protocol)
├── package_managers/ (15 managers)
├── runtimes/     (8 runtime managers)
└── ...
```

**Issues:**
- `cli/` is too flat (24 files at top level)
- `core/` mixes unrelated concerns
- No clear domain boundaries

**Proposed structure:**
```
src/
├── bin/
├── cli/
│   ├── commands/    (install, remove, search, etc.)
│   ├── tui/         (terminal UI)
│   ├── tea/         (Bubble Tea models)
│   └── util/        (shared CLI utilities)
├── domain/          (new: core business logic)
│   ├── package/     (Package types, validation)
│   ├── runtime/     (Runtime types, version)
│   └── repository/  (Package sources)
├── infra/           (new: infrastructure)
│   ├── pacman/      (Arch/pacman)
│   ├── apt/         (Debian/Ubuntu)
│   ├── aur/         (AUR)
│   └── cache/       (Caching layer)
├── daemon/          (unchanged)
└── core/            (reduce to: error, config, paths)
```

**Benefits:**
- Clear domain/infra separation (DDD)
- CLI organized by feature, not file type
- Easier to navigate and reason about

### 3.2 Eliminate Over-Engineering

**Pattern: Unnecessary Traits**

Find and remove:
```rust
// BEFORE: Trait with single impl
trait Configurable {
    fn load() -> Result<Self>;
}
impl Configurable for Config { ... }

// AFTER: Just implement on the type
impl Config {
    fn load() -> Result<Self> { ... }
}
```

**Pattern: Excessive Generics**

Find and simplify:
```rust
// BEFORE: Generic that's never parameterized
fn process<T: PackageManager>(mgr: T, pkg: &str) -> Result<()>
// Only ever called with one concrete type!

// AFTER: Use concrete type
fn process(mgr: &PacmanManager, pkg: &str) -> Result<()>
```

**Action Items:**
- [ ] Find traits with single implementation
- [ ] Find generic functions with single call site
- [ ] Find abstraction layers that don't abstract anything
- [ ] Remove and simplify

### 3.3 Consistent Error Handling

**Current state**: Mix of anyhow, thiserror, custom errors

**Target state**:
- **Applications/Binaries**: `anyhow::Result<T>`
- **Library APIs**: `thiserror::Error` enums
- **Internal modules**: `anyhow` for flexibility

**Pattern:**
```rust
// Library API (omg_lib)
#[derive(thiserror::Error, Debug)]
pub enum PackageError {
    #[error("Package not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// Binary (omg)
fn main() -> anyhow::Result<()> {
    // Use anyhow for flexibility
    let pkg = install_package("firefox")
        .context("Failed to install firefox")?;
    Ok(())
}
```

### 3.4 Consistent Logging

**Current state**: Mix of println!, eprintln!, tracing

**Target state**: 100% tracing

**Migration:**
```rust
// BEFORE
println!("Installing package: {}", name);
eprintln!("Error: {}", err);

// AFTER
tracing::info!("Installing package: {}", name);
tracing::error!("Failed to install: {}", err);
```

**Configure spans:**
```rust
#[tracing::instrument(skip(self))]
async fn install(&self, pkg: &str) -> Result<()> {
    tracing::debug!("Checking dependencies");
    // All logs automatically tagged with pkg name
}
```

### 3.5 Dependency Audit & Upgrades

**Review all dependencies:**
- [ ] Remove unused crates (cargo-machete)
- [ ] Upgrade outdated crates
- [ ] Replace deprecated patterns
- [ ] Consolidate duplicate functionality

**Specific upgrades:**
- ✅ Already using tokio 1.49 (latest)
- ✅ Already using clap 4.5 (latest)
- ✅ Already using trait-variant (modern)
- [ ] Review: Any crates with better alternatives?

### 3.6 Testing Modernization

**Consistent test patterns:**

```rust
// Use fixtures
use crate::core::testing::fixtures::*;

#[test]
fn test_package_install() {
    let ctx = TestContext::new();
    let pkg = PackageFixture::rust_latest().build();

    ctx.install(&pkg).expect("Should install");
    assert!(ctx.is_installed(&pkg.name));
}

// Use helpers
use crate::core::testing::helpers::*;

#[test]
fn test_with_timeout() {
    with_timeout(Duration::from_secs(5), async {
        slow_operation().await
    }).expect("Should complete in time");
}
```

**Action Items:**
- [ ] Standardize on test fixtures (already have some)
- [ ] Create common test utilities
- [ ] Use proptest for property-based tests
- [ ] Add integration test patterns

### 3.7 Phase 3 Verification

**Quality Gates:**
- [ ] Module structure follows DDD principles
- [ ] Zero unnecessary abstractions
- [ ] Consistent error handling (anyhow/thiserror)
- [ ] 100% tracing (no println/eprintln)
- [ ] Zero unused dependencies
- [ ] All tests modernized
- [ ] Documentation updated

---

## 🔧 Implementation Strategy

### Workflow

**1. Pause All Feature Work**
- No new features during sprint
- Only critical bugfixes allowed
- Full team focus on modernization

**2. Work in Phases**
- Complete Phase 1 before starting Phase 2
- Each phase ends with verification
- Can pause after any phase if needed

**3. Quality Gates**
- Every commit must:
  - Pass all tests
  - Pass clippy (pedantic + nursery)
  - Have no TODO comments
  - Update relevant docs

**4. Review Process**
- Phase reviews before moving forward
- Code review for major rewrites
- Performance benchmarks before/after

### Tools & Automation

**Linting:**
```toml
# .cargo/config.toml
[alias]
lint = "clippy --all-targets --all-features -- -D warnings -D clippy::pedantic -D clippy::nursery"
```

**Pre-commit hooks:**
```bash
#!/bin/bash
# .git/hooks/pre-commit
cargo fmt --check
cargo lint
cargo test
```

**Automation:**
- [ ] cargo-machete for unused deps
- [ ] cargo-audit for security
- [ ] cargo-deny for supply chain
- [ ] cargo-flamegraph for profiling

---

## 📊 Success Metrics

### Quantitative

**Before:**
- 9 unsafe blocks
- 254 panic points (unwrap/expect)
- Unknown clone count
- Unknown async blocking

**After:**
- 0 unsafe blocks (except FFI)
- 0 unwrap/expect in production code
- 50%+ reduction in clones
- 0 blocking code in async

### Qualitative

**Before:**
- Inconsistent error handling
- Mix of logging approaches
- Over-engineered abstractions
- Unclear module boundaries

**After:**
- Consistent anyhow/thiserror pattern
- 100% tracing with spans
- Removed unnecessary complexity
- Clear DDD architecture

### Performance

**Targets:**
- No regressions in hot paths
- 10-20% speedup from reduced cloning
- Better async throughput

---

## 🚨 Risk Mitigation

### High-Risk Changes

1. **Removing unsafe code**
   - Risk: Performance regression
   - Mitigation: Benchmark before/after, keep safe version if <10% slower

2. **Module reorganization**
   - Risk: Breaking internal APIs
   - Mitigation: Use rust-analyzer rename, verify with tests

3. **Async pattern changes**
   - Risk: Deadlocks or race conditions
   - Mitigation: Extensive testing, use tokio-console for debugging

### Rollback Plan

- Each phase in separate branch
- Can revert phase if issues found
- Keep phase 1 (safety) at minimum

---

## 📚 Documentation Updates

**After each phase:**
- [ ] Update ARCHITECTURE.md
- [ ] Update CONTRIBUTING.md (new patterns)
- [ ] Add migration guide (for API changes)
- [ ] Update rustdoc comments

---

## ✅ Checklist

### Phase 1: Safety First
- [ ] Audit all 9 files with unsafe code
- [ ] Replace unsafe mmap access with checked deserialization
- [ ] Replace unsafe env vars with temp-env
- [ ] Fix top 100 panic points
- [ ] Document remaining panics
- [ ] Pass all quality gates

### Phase 2: Async & Performance
- [ ] Audit async/await patterns
- [ ] Replace blocking I/O with tokio::fs/io
- [ ] Move CPU work to spawn_blocking
- [ ] Reduce cloning by 50%
- [ ] Convert traits to trait-variant
- [ ] Pass all quality gates

### Phase 3: Architecture
- [ ] Reorganize modules (DDD)
- [ ] Remove unnecessary abstractions
- [ ] Standardize error handling
- [ ] Convert to 100% tracing
- [ ] Audit dependencies
- [ ] Modernize tests
- [ ] Pass all quality gates

---

## 📅 Timeline

**Week 1: Phase 1 (Safety)**
- Day 1-2: Audit unsafe code
- Day 3-4: Eliminate unsafe, fix critical panics
- Day 5: Verification, benchmarks

**Week 2: Phase 2 (Async/Performance)**
- Day 1-2: Audit async patterns, fix blocking code
- Day 3-4: Type ergonomics (Arc/Cow)
- Day 5: Verification, benchmarks

**Week 3-4: Phase 3 (Architecture)**
- Days 1-3: Module reorganization
- Days 4-6: Remove over-engineering
- Days 7-8: Error handling, logging, tests
- Days 9-10: Final verification, docs

---

## 🎯 Success Definition

**This modernization succeeds when:**
1. Codebase is idiomatic Rust 2026
2. Zero unsafe code (except necessary FFI)
3. Zero production panics
4. Consistent patterns throughout
5. Clear architecture (DDD)
6. All tests pass, no regressions
7. Team velocity improved (easier to work with)

**This is world-class Rust.**
