# OMG CLI Test Coverage Matrix

**Last Updated**: 2026-02-01  
**Total Commands**: 43  
**Unit Tests**: 345 passing  
**Integration Tests**: ~112 passing

## Coverage Status Legend
- ✅ **Covered**: Has comprehensive e2e tests (success + error cases)
- 🟡 **Partial**: Has basic tests but missing error/edge cases
- ❌ **Missing**: No e2e tests exist

---

## Core Package Management (8 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `search` | ✅ | Basic search, AUR search, detailed, regex | Performance benchmarks |
| `install` | 🟡 | Help text only | Actual install flow, error cases, security grading |
| `remove` | 🟡 | Help text only | Actual remove flow, dependency handling, orphan cleanup |
| `update` | 🟡 | `--check` flag | Full update flow, `--yes` mode, conflict resolution |
| `info` | ✅ | pacman, firefox, nonexistent | Multiple sources (AUR, official), JSON output |
| `why` | ✅ | Basic, reverse deps | Deep dependency chains, circular deps |
| `outdated` | ✅ | Basic, security, JSON | Large package sets, performance |
| `sync` | ✅ | Basic sync | Parallel sync, error recovery |

**Priority**: Add `install`, `remove`, `update` full flows

---

## Package Operations (5 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `explicit` | ✅ | List, count | Large systems (1000+ packages) |
| `clean` | 🟡 | Help only | Actual cleanup, dry-run, selective clean |
| `pin` | ❌ | None | Pin package, unpin, list pinned |
| `size` | ✅ | Basic, limit, tree | Cache size, breakdown by repo |
| `blame` | ✅ | Basic | Multiple packages, history depth |

**Priority**: Add `pin` and `clean` full flows

---

## Runtime Management (4 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `use` | 🟡 | Basic node switch | Python, Go, Rust, Bun, Ruby, Java; global vs local |
| `list` | ✅ | Installed runtimes | `--available`, filtering, JSON output |
| `hook` | ❌ | None | Shell hooks (bash, zsh, fish), integration tests |
| `which` | ✅ | Basic | Fallback behavior, .tool-versions precedence |

**Priority**: Add `hook` tests and multi-runtime `use` tests

---

## Project Workflows (4 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `run` | ❌ | None | package.json, Cargo.toml, Makefile, pyproject.toml detection |
| `new` | ❌ | None | All templates (react, rust, python, etc.) |
| `tool` | ❌ | None | Install, remove, list, update cross-ecosystem tools |
| `workspace` | ❌ | None | Monorepo detection, workspace commands |

**Priority**: Critical user-facing features - add full coverage

---

## Environment & Team (4 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `env` | ❌ | None | Capture, restore, check, share |
| `team` | ❌ | None | Init, join, push, pull, status, diff |
| `hooks` | ❌ | None | Git hook installation, pre-commit, post-checkout |
| `snapshot` | ❌ | None | Create, restore, list, delete snapshots |

**Priority**: Team collaboration features - add comprehensive tests

---

## Container & CI (2 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `container` | ❌ | None | shell, build, init, run commands |
| `ci` | ❌ | None | GitHub Actions, GitLab CI, CircleCI generation |

**Priority**: DevOps workflows - add end-to-end tests

---

## Security & Compliance (2 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `audit` | 🟡 | scan, policy | sbom, secrets, licenses, slsa, fix, export, eol |
| `license` | ❌ | None | activate, status, deactivate, tiers |

**Priority**: Security is critical - add all audit subcommands

---

## System Management (8 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `status` | ✅ | Basic, concurrent | Detailed status, JSON output |
| `doctor` | ❌ | None | Health checks, diagnostics, fix suggestions |
| `config` | ❌ | None | get, set, list, unset |
| `daemon` | ❌ | None | start, stop, restart, logs |
| `daemon-status` | ❌ | None | Detailed daemon info |
| `history` | ❌ | None | Transaction log, filtering, rollback |
| `rollback` | ❌ | None | Rollback to previous state |
| `migrate` | ❌ | None | Cross-distro migration workflows |

**Priority**: `doctor` and `config` are high-value

---

## UI & Utilities (6 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `dash` | ❌ | None | TUI launch, navigation, keybindings |
| `stats` | ❌ | None | Usage stats, time saved, command frequency |
| `metrics` | ❌ | None | Prometheus metrics export |
| `completions` | ✅ | zsh generation | bash, fish, PowerShell |
| `generate-man` | ❌ | None | Man page generation for all commands |
| `diff` | ❌ | None | Lock file comparison |

**Priority**: `completions` needs expansion

---

## Enterprise & Fleet (2 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `fleet` | ❌ | None | Multi-machine management |
| `enterprise` | ❌ | None | Reports, policies, compliance exports |

**Priority**: Lower (Pro/Enterprise features)

---

## Meta Commands (2 commands)

| Command | Status | Tests | Missing |
|---------|--------|-------|---------|
| `self-update` | ❌ | None | Update check, download, install, verify |
| `init` | ❌ | None | Interactive setup wizard |

**Priority**: User onboarding - add `init` tests

---

## Summary

### Current Coverage
- **Well Tested (✅)**: 10/43 commands (23%)
- **Partially Tested (🟡)**: 7/43 commands (16%)
- **Not Tested (❌)**: 26/43 commands (60%)

### Priority Test Additions (Next 2 hours)

**Tier 1 (Critical - User-Facing)**:
1. `install`, `remove`, `update` - Core package operations
2. `run`, `new` - Project workflows
3. `use` (multi-runtime), `hook` - Runtime management
4. `doctor`, `config` - System management

**Tier 2 (Important - Team Features)**:
5. `env`, `team` - Environment sync
6. `audit` (all subcommands) - Security
7. `tool` - Dev tool management

**Tier 3 (Nice-to-Have)**:
8. `container`, `ci` - DevOps
9. `dash`, `stats` - UI/UX
10. `init` - Onboarding

---

## Test Quality Standards

### ✅ Comprehensive Test Requirements

Each command should have:

1. **Success Cases**
   - Basic happy path
   - With all major flags/options
   - JSON output (if supported)
   - Verbose mode

2. **Error Cases**
   - Invalid arguments
   - Nonexistent resources
   - Permission errors
   - Network failures (if applicable)

3. **Edge Cases**
   - Empty results
   - Very large datasets
   - Special characters in input
   - Concurrent operations

4. **Performance**
   - Execution time < target
   - Memory usage reasonable
   - No hangs/deadlocks

---

## Test Implementation Checklist

When adding a new command test:

- [ ] Create test file or add to existing
- [ ] Test success case
- [ ] Test with `--help`
- [ ] Test error handling (invalid input)
- [ ] Test JSON output (if supported)
- [ ] Add performance assertion
- [ ] Document expected behavior
- [ ] Add to this coverage matrix

---

## Running Specific Test Suites

```bash
# Core package management
cargo test --test cli_integration

# Security
cargo test --test security_tests

# Performance
cargo test --test benchmarks --release

# All tests
cargo test --all

# With system access
OMG_RUN_SYSTEM_TESTS=1 cargo test --features arch
```
