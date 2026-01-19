# OMG Enhancement Roadmap Implementation Plan

This plan prioritizes high-impact improvements from the AI roadmap analysis, filtered against what's already implemented.

## Current State Assessment

### Already Implemented ✅
- **CI/CD:** GitHub Actions with check, test, clippy, fmt, debian-smoke, typos
- **Docs:** 15+ markdown files covering CLI, architecture, daemon, security, runtimes
- **UX:** indicatif spinners/progress bars, owo-colors styling, verbose/quiet flags
- **Telemetry:** Privacy-first opt-out system with analytics tracking
- **Testing:** Integration suite (72KB), unit tests in 17 modules
- **Multi-distro:** Arch + Debian backends (feature-gated)

### Priority Gaps (From AI Analysis)

| Gap | Impact | Effort | Priority |
|-----|--------|--------|----------|
| Migration guides | High | Low | **P1** |
| CONTRIBUTING.md | High | Low | **P1** |
| Benchmarks in CI | Medium | Low | **P1** |
| Human-readable error suggestions | High | Medium | **P2** |
| mdBook docs site | Medium | Medium | **P2** |
| Plugin system architecture | High | High | **P3** |
| macOS/Homebrew backend | High | High | **P3** |
| AI-assisted features | Medium | High | **P4** |

---

## Phase 1: Quick Wins (1-2 weeks)

### 1.1 Create Migration Guides
**Files:** `docs/migration-from-yay.md`, `docs/migration-from-nvm.md`, `docs/migration-from-pyenv.md`

Content:
- Command equivalents table (yay → omg, nvm → omg)
- Shell configuration changes
- Common workflows comparison
- Troubleshooting section

### 1.2 Add CONTRIBUTING.md
**File:** `CONTRIBUTING.md`

Content:
- Development setup
- PR guidelines
- Testing requirements
- Code style (reference AGENTS.md)
- Issue labels and workflow

### 1.3 Add Benchmark CI Job
**File:** `.github/workflows/ci.yml`

Add benchmark job:
```yaml
benchmark:
  name: Performance Benchmarks
  runs-on: ubuntu-latest
  container:
    image: archlinux:latest
  steps:
    - uses: actions/checkout@v4
    - name: Run Benchmarks
      run: |
        cargo build --release
        ./benchmark.sh --ci
    - name: Upload Results
      uses: actions/upload-artifact@v4
      with:
        name: benchmark-results
        path: benchmark_report.md
```

### 1.4 Enhanced Error Messages
**File:** `src/core/error.rs` (new or expand existing)

Add context-aware suggestions:
```rust
pub fn suggest_fix(error: &OmgError) -> Option<String> {
    match error {
        OmgError::PackageNotFound(name) => 
            Some(format!("Try: omg search {}", name)),
        OmgError::RuntimeNotFound(runtime) => 
            Some(format!("Install with: omg use {} latest", runtime)),
        // ... more cases
    }
}
```

---

## Phase 2: Documentation Site (2-4 weeks)

### 2.1 mdBook Setup
**Files:** `book.toml`, `docs/SUMMARY.md`

- Convert existing docs to mdBook structure
- Add search functionality
- Deploy to GitHub Pages
- Add code highlighting and copy buttons

### 2.2 Interactive Examples
- Add asciinema recordings for key workflows
- Embed in docs site

---

## Phase 3: Extensibility Foundation (4-8 weeks)

### 3.1 Plugin Architecture Design
**File:** `docs/plugin-architecture.md`, `src/plugins/mod.rs`

Options:
1. **WASM plugins** (wasmtime) - Sandboxed, cross-platform
2. **Dynamic libraries** (libloading) - Native performance
3. **Script plugins** (lua/rhai) - Easy authoring

Recommended: WASM for security + Lua for simple scripts

### 3.2 Plugin Trait Definition
```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    async fn on_search(&self, query: &str) -> Vec<Package>;
    async fn on_install(&self, package: &str) -> Result<()>;
}
```

---

## Phase 4: Platform Expansion (8-12 weeks)

### 4.1 macOS Support
- Homebrew integration via `brew` CLI wrapper
- Feature gate: `--features macos`
- Distro detection in `src/core/platform.rs`

### 4.2 Windows Exploration
- Chocolatey/Winget integration
- WSL detection and bridging
- Lower priority than macOS

---

## Phase 5: AI Features (12+ weeks)

### 5.1 Smart Suggestions
**Opt-in, local-first approach:**
- Dependency conflict resolution hints
- Package recommendations based on project type
- Use local embedding model (llama.cpp via llama-rs)

### 5.2 Implementation Strategy
- Start with rule-based suggestions (no ML)
- Graduate to local LLM for complex cases
- Never send code/data externally without explicit consent

---

## Metrics to Track

| Metric | Current | Target | Tool |
|--------|---------|--------|------|
| GitHub Stars | ~50 | 500+ | GitHub |
| Test Coverage | ~60% | 80%+ | cargo-tarpaulin |
| Benchmark Regressions | N/A | 0 | CI artifacts |
| Install Count | tracked | tracked | Telemetry |
| Command Usage | tracked | tracked | Analytics |

---

## Recommended Execution Order

1. **Week 1:** Migration guides + CONTRIBUTING.md
2. **Week 2:** Benchmark CI job + error message improvements
3. **Weeks 3-4:** mdBook docs site
4. **Weeks 5-8:** Plugin architecture design + POC
5. **Weeks 9-12:** macOS backend
6. **Weeks 13+:** AI features (optional based on demand)

---

## Questions for User

1. **mdBook vs Docusaurus:** Prefer Rust-native mdBook or JS-based Docusaurus?
2. **Plugin priority:** WASM (secure) or Lua (simple) first?
3. **macOS priority:** Is cross-platform expansion a near-term goal?
4. **AI scope:** Rule-based suggestions only, or explore local LLM integration?
