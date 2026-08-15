# OMG v0.1.215 - Patch Release

## Release Notes

### 🐛 Bug Fixes

**AUR Update Detection Fixed**
- Fixed a critical bug where `omg update` would incorrectly report "No updates in AUR" when AUR packages had newer versions available
- Root cause: The AUR binary index fallback logic was returning early without checking the RPC API when the cached index was stale
- Impact: Users with outdated AUR metadata cache no longer miss package updates
- Affected packages: Any AUR packages not present in the local binary index (e.g., newly added packages or packages with version changes)

### 🔧 Technical Details

The AUR update check uses a 4-stage fallback strategy:
1. Binary index (fast, cached)
2. Metadata archive (slower JSON)
3. RPC API (live query - always accurate)

**Before fix:** If binary index returned empty → return early (bug!)
**After fix:** If binary index returns empty → continue to fallback mechanisms

This ensures the system always falls back to querying the live AUR API when the cached metadata is stale, matching the behavior of yay/paru.

### ✅ Verification

- Build: `cargo build --release --bin omg` ✓
- Tests: `cargo test aur` - 1 test passed ✓
- Code: No LSP diagnostics ✓

---

**Download:** https://github.com/PyRo1121/omg/releases

**Install:**
```bash
# Via script
curl -fsSL https://pyro1121.com/install.sh | bash

# Via AUR
yay -S omg-bin
```
