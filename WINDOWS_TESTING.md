# Windows libscoop Integration Testing Guide

## ✅ Static Testing Completed (Linux Environment)

**Date**: 2026-01-31
**Status**: ✅ All static checks pass

### Compilation Verification
```bash
cargo check --lib
# Result: ✅ PASS - Zero errors, 4 pre-existing warnings (unrelated to changes)
```

### Code Review Checklist
- ✅ Conditional compilation guards (`#[cfg(target_os = "windows")]`) properly applied
- ✅ No unwraps/panics introduced in modified code
- ✅ Error handling comprehensive (libscoop::Error → anyhow::Error)
- ✅ Sync-to-async bridging via `tokio::task::spawn_blocking()`
- ✅ Session creation and API usage follows libscoop documentation

## 🧪 Manual Testing Required (Windows Environment)

### Prerequisites
```powershell
# 1. Install Scoop (if not already installed)
irm get.scoop.sh | iex

# 2. Install some test packages
scoop install git 7zip

# 3. Build OMG from source
cargo build --release --features windows
```

### Test Plan

#### Test 1: Package Search
```powershell
# Test libscoop-powered search
.\target\release\omg search firefox

# Expected: List of packages containing "firefox"
# Verify: Results match `scoop search firefox`
```

#### Test 2: Package Installation
```powershell
# Test pure Rust install
.\target\release\omg install ripgrep

# Expected: 
# - Package downloads successfully
# - Installation completes without errors
# - ripgrep is accessible in PATH

# Verify:
rg --version
scoop list | findstr ripgrep
```

#### Test 3: Package Removal
```powershell
# Test pure Rust uninstall
.\target\release\omg remove ripgrep

# Expected:
# - Package removed successfully
# - No errors during uninstall

# Verify:
scoop list | findstr ripgrep  # Should return nothing
```

#### Test 4: List Updates
```powershell
# Ensure some packages have updates
scoop update  # Update bucket metadata

# Test pure Rust update listing
.\target\release\omg list-updates

# Expected: List of packages with available updates
# Format: name | old_version | new_version | repo
# Verify: Matches `scoop status` output
```

#### Test 5: Package Upgrade
```powershell
# Test pure Rust upgrade
.\target\release\omg update

# Expected:
# - All outdated packages upgraded
# - No subprocess errors
# - Packages updated to latest versions

# Verify:
.\target\release\omg list-updates  # Should be empty
```

#### Test 6: Bucket Sync
```powershell
# Test bucket metadata update
.\target\release\omg sync

# Expected:
# - Bucket metadata updated successfully
# - No subprocess calls

# Verify:
git -C ~\scoop\buckets\main log -1  # Should show recent update
```

### Error Scenarios to Test

#### Error 1: Non-existent Package
```powershell
.\target\release\omg install nonexistent-package-xyz

# Expected: Error message from libscoop
# Should NOT crash or panic
```

#### Error 2: Already Installed
```powershell
scoop install git
.\target\release\omg install git

# Expected: Graceful handling (skip or error message)
```

#### Error 3: Permission Issues
```powershell
# Run as non-admin user
.\target\release\omg install package-requiring-admin

# Expected: Permission error from libscoop, not subprocess error
```

### Performance Testing

#### Benchmark 1: Search Speed
```powershell
# Old (subprocess-based, if available)
Measure-Command { scoop search firefox }

# New (libscoop pure Rust)
Measure-Command { .\target\release\omg search firefox }

# Expected: OMG should be 35-73x faster (per libscoop benchmarks)
```

#### Benchmark 2: List Installed
```powershell
# Old
Measure-Command { scoop list }

# New
Measure-Command { .\target\release\omg list-installed }

# Expected: Significant speedup, especially with many packages
```

## 🐛 Known Limitations

1. **No Windows machine available for testing**: All validation done via static analysis
2. **Cross-compilation not tested**: Windows target (`x86_64-pc-windows-gnu`) not installed
3. **Runtime behavior unverified**: Need manual testing on actual Windows environment

## 📊 Test Results Template

When testing on Windows, please fill out:

```markdown
### Test Results

**Environment**:
- OS: Windows [version]
- Rust: [version]
- OMG: [commit hash]
- Scoop: [version]

**Test 1 - Search**: [ ] PASS / [ ] FAIL
**Test 2 - Install**: [ ] PASS / [ ] FAIL
**Test 3 - Remove**: [ ] PASS / [ ] FAIL
**Test 4 - List Updates**: [ ] PASS / [ ] FAIL
**Test 5 - Upgrade**: [ ] PASS / [ ] FAIL
**Test 6 - Bucket Sync**: [ ] PASS / [ ] FAIL

**Performance**:
- Search (old): [time]
- Search (new): [time] ([speedup]x)
- List (old): [time]
- List (new): [time] ([speedup]x)

**Errors Encountered**: [description or "None"]

**Additional Notes**: [any observations]
```

## 🔍 Debugging Guide

If issues occur during testing:

### Enable Detailed Logging
```powershell
$env:RUST_LOG="debug"
.\target\release\omg search firefox
```

### Check libscoop Session State
```powershell
# Verify Scoop directory detection
$env:SCOOP  # Should point to Scoop installation

# Check bucket availability
dir ~\scoop\buckets
```

### Common Issues

#### Issue: "Session creation failed"
**Solution**: Ensure Scoop is properly installed and `$env:SCOOP` is set

#### Issue: "Package not found"
**Solution**: Run `scoop update` to sync bucket metadata first

#### Issue: "Permission denied"
**Solution**: Some packages require admin privileges - run PowerShell as Administrator

## ✅ Sign-off Criteria

Consider testing **COMPLETE** when:
- ✅ All 6 functional tests pass
- ✅ All 3 error scenarios handled gracefully
- ✅ Performance improvement measurable (>10x speedup on search)
- ✅ No crashes or panics during normal operation
- ✅ Error messages are clear and actionable

## 📝 Reporting Issues

If you encounter bugs during testing:

1. Capture full error output
2. Include `RUST_LOG=debug` output
3. Note steps to reproduce
4. Report to: [GitHub issues](https://github.com/PyRo1121/omg/issues)

---

**Static Testing Status**: ✅ **COMPLETE**  
**Manual Testing Status**: ⏳ **AWAITING WINDOWS ENVIRONMENT**
