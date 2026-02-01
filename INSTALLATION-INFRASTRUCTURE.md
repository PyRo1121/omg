# Installation Infrastructure - Complete Reference

> Created: 2026-02-01  
> Commit: ee753bc  
> Impact: Added Windows support, Scoop bucket, improved release workflow

## 📋 Quick Reference

### Installation Commands by Platform

| Platform | Command |
|----------|---------|
| **Linux (Universal)** | `curl -fsSL https://pyro1121.com/install.sh \| bash` |
| **Arch Linux** | `yay -S omg-bin` |
| **Debian/Ubuntu** | `curl -fsSL https://pyro1121.com/install.sh \| bash` |
| **Fedora/RHEL** | `curl -fsSL https://pyro1121.com/install.sh \| bash` |
| **macOS** | `curl -fsSL https://pyro1121.com/install.sh \| bash` |
| **Windows (PowerShell)** | `irm https://pyro1121.com/install.ps1 \| iex` |
| **Windows (Scoop)** | `scoop bucket add omg <url> && scoop install omg` |
| **Windows (WSL)** | `wsl -- curl -fsSL https://pyro1121.com/install.sh \| bash` |
| **From Source** | `cargo install omg-cli` |

---

## 📁 File Structure

### Installation Scripts

```
.
├── install.sh                          # Universal installer (Linux/macOS)
│   ├── Lines: 715
│   ├── Features: OS detection, binary download, source build fallback
│   ├── Serves: All Unix-like platforms
│   └── URL: https://pyro1121.com/install.sh
│
├── install.ps1                         # Windows PowerShell installer
│   ├── Lines: 229
│   ├── Features: Auto-download, SHA256 verify, PATH setup
│   ├── Serves: Windows 10/11 (x64)
│   └── URL: https://pyro1121.com/install.ps1
│
└── site/public/install.ps1             # Copy for web distribution
    └── Synced: Automatically via build process
```

### Scoop Bucket

```
scoop-bucket/
├── omg.json                            # Scoop manifest
│   ├── Purpose: Package definition for Scoop
│   ├── Auto-update: via Excavator workflow
│   └── Version: Synced with GitHub releases
│
├── README.md                           # Bucket documentation
│   └── Purpose: Installation instructions for users
│
├── .github/workflows/excavator.yml     # Auto-updater
│   ├── Runs: Every 6 hours
│   ├── Updates: Version + SHA256 hash
│   └── Requires: GITH_EMAIL, GITHUB_TOKEN secrets
│
└── [Future: Deploy to PyRo1121/scoop-omg repo]
```

### Documentation

```
docs/
├── installation.md                     # Comprehensive install guide
│   ├── Lines: 461
│   ├── Covers: All platforms, troubleshooting, CI/CD
│   └── Audience: New users, advanced users, ops teams
│
├── changelog.md                        # Auto-generated changelog
│   ├── Generator: scripts/update-changelog.sh
│   ├── Source: git-cliff (conventional commits)
│   └── Sync: Auto-syncs to omg-docs/
│
└── quickstart.md                       # Quick start guide
    └── Status: Updated with new install methods
```

### Scripts

```
scripts/
├── update-changelog.sh                 # Changelog regenerator
│   ├── Input: git history
│   ├── Output: docs/changelog.md, omg-docs/.../changelog.md
│   └── Features: Starlight frontmatter, MDX escaping
│
├── extract-release-notes.sh            # Version-specific extractor
│   ├── Input: Version number (e.g., 0.1.204)
│   ├── Output: Changelog for that version only
│   └── Used by: .github/workflows/release.yml
│
└── generate-changelog.sh               # Manual changelog tool
    └── Usage: Preview, latest, unreleased, specific tag
```

### CI/CD

```
.github/workflows/
└── release.yml                         # Release automation
    ├── Modified: Lines 274-346 (release notes section)
    ├── Uses: scripts/extract-release-notes.sh
    └── Output: GitHub release with full changelog content
```

### Website

```
site/
├── public/
│   ├── install.sh                      # Synced from root
│   └── install.ps1                     # Synced from root
│
└── src/components/
    └── Installation.tsx                # Installation UI component
        ├── Tabs: Linux/macOS, Windows, Arch, Scoop
        └── Commands: Displayed with copy buttons
```

---

## 🔄 How It All Works Together

### 1. User Installs OMG

**Linux/macOS:**
```
User → pyro1121.com/install.sh → install.sh → Downloads binary → Installs
```

**Windows (PowerShell):**
```
User → pyro1121.com/install.ps1 → install.ps1 → Downloads .zip → Extracts → Adds to PATH
```

**Windows (Scoop):**
```
User → scoop install omg → omg.json → Downloads .zip → Scoop manages
```

### 2. Developer Makes Changes

```
Developer commits → Git history → git-cliff → changelog.md → Synced to docs
```

### 3. Release Process

```mermaid
graph LR
    A[Create Git Tag] --> B[GitHub Release Workflow]
    B --> C[Build Binaries]
    C --> D[Extract Changelog]
    D --> E[Create Release]
    E --> F[Publish Assets]
    F --> G[Excavator Updates Scoop]
```

**Detailed Flow:**

1. **Tag creation**: `git tag v0.1.205 && git push --tags`
2. **GitHub Actions triggers**: `.github/workflows/release.yml`
3. **Builds for all platforms**: Arch, Debian, Ubuntu, Fedora, macOS, Windows
4. **Extracts changelog**: `scripts/extract-release-notes.sh v0.1.205`
5. **Creates release**: With actual changelog content
6. **Scoop auto-updates**: Excavator workflow runs every 6 hours

---

## 🛠️ Maintenance Tasks

### Update Changelog

```bash
# Regenerate from git history
./scripts/update-changelog.sh

# Preview unreleased changes
./scripts/generate-changelog.sh --preview

# Extract specific version
./scripts/extract-release-notes.sh 0.1.204
```

### Update Installation Scripts

**To modify install.sh:**
1. Edit `install.sh` in root
2. Test locally: `./install.sh`
3. Commit and push
4. Sync to website: `cp install.sh site/public/`

**To modify install.ps1:**
1. Edit `install.ps1` in root
2. Test on Windows: `powershell -File install.ps1`
3. Commit and push
4. Sync to website: `cp install.ps1 site/public/`
5. Deploy website to make it live

### Update Scoop Bucket

**Automatic (Excavator):**
- Runs every 6 hours
- Detects new releases
- Updates version + hash
- Commits to bucket repo

**Manual:**
1. Edit `scoop-bucket/omg.json`
2. Update version
3. Download release, calculate SHA256
4. Update hash
5. Commit to `PyRo1121/scoop-omg`

---

## 🧪 Testing Checklist

### Before Each Release

- [ ] Run `./scripts/update-changelog.sh`
- [ ] Verify `docs/changelog.md` is updated
- [ ] Verify `omg-docs/.../changelog.md` is synced
- [ ] Test install.sh on Linux VM
- [ ] Test install.ps1 on Windows (if changed)
- [ ] Check README.md installation section renders correctly

### After Each Release

- [ ] Verify GitHub release has full changelog content
- [ ] Check all platform binaries are attached
- [ ] Verify install.sh downloads correct version
- [ ] Verify install.ps1 downloads correct version
- [ ] Wait 6 hours, check Scoop bucket auto-updated
- [ ] Test: `scoop update omg` works

---

## 📊 Analytics & Metrics

### Installation Method Distribution (Target)

| Method | Target % | Notes |
|--------|----------|-------|
| Linux curl | 40% | Primary method |
| AUR (Arch) | 20% | Active Arch users |
| Windows PS | 15% | Native Windows |
| Scoop | 10% | Windows power users |
| Cargo | 10% | Rust developers |
| WSL | 5% | Windows+Linux hybrid |

### Success Metrics

- **Install success rate**: >95%
- **Time to install**: <30 seconds
- **User retention**: >80% (still using after 30 days)
- **Windows adoption**: 20%+ of new installs

---

## 🔐 Security Considerations

### Binary Verification

**Linux/macOS (install.sh):**
- Downloads from GitHub releases
- Verifies SHA256 checksums (built-in)
- Uses HTTPS only

**Windows (install.ps1):**
- Downloads from GitHub releases
- Verifies SHA256 checksums (explicit check)
- Uses HTTPS only
- Does NOT require admin (installs to user directory)

**Scoop:**
- Scoop verifies SHA256 automatically
- Manifest is in version control
- Excavator is official Scoop tool

### Telemetry

Both installers:
- Ask for telemetry consent (opt-in)
- Allow opt-out via environment variable
- Document what data is collected
- No personal information collected

---

## 🚀 Future Improvements

### Short-term (1-3 months)

- [ ] Homebrew tap for macOS
- [ ] APT repository for Debian/Ubuntu
- [ ] DNF repository for Fedora
- [ ] Chocolatey package (secondary Windows option)
- [ ] Add installation badges to README

### Long-term (3-6 months)

- [ ] Windows Store listing
- [ ] Snap package (Ubuntu)
- [ ] Flatpak package (universal Linux)
- [ ] Docker official image
- [ ] Automated dependency updates

---

## 📞 Support

### User-Facing

- **Documentation**: https://pyro1121.com/docs/installation
- **Issues**: https://github.com/PyRo1121/omg/issues
- **Discussions**: https://github.com/PyRo1121/omg/discussions

### Developer-Facing

- **This document**: `INSTALLATION-INFRASTRUCTURE.md`
- **Scoop setup**: `SCOOP-BUCKET-SETUP.md`
- **Script reference**: `scripts/README.md` (if exists)

---

## 📝 Change History

| Date | Version | Change | Author |
|------|---------|--------|--------|
| 2026-02-01 | Initial | Created comprehensive installation infrastructure | Claude + User |
| 2026-02-01 | ee753bc | Added Windows installer, Scoop bucket, improved releases | Claude + User |

---

## ✅ Quick Verification

After deployment, verify:

```bash
# Linux
curl -fsSL https://pyro1121.com/install.sh | head -5

# Windows
irm https://pyro1121.com/install.ps1 | Select-Object -First 5

# Scoop (after bucket creation)
scoop bucket list | grep omg
scoop search omg

# Release notes
gh release view v0.1.204 --json body
```

All should work without errors.
