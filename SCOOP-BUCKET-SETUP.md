# Scoop Bucket Setup Guide

This guide explains how to set up and maintain the OMG Scoop bucket for Windows package distribution.

## 📁 Repository Structure

The Scoop bucket should be a **separate GitHub repository**:

```
PyRo1121/scoop-omg/
├── .github/
│   └── workflows/
│       └── excavator.yml    # Auto-updates manifests
├── omg.json                 # Scoop manifest for OMG
└── README.md                # Installation instructions
```

## 🚀 Setup Steps

### 1. Create the Scoop Bucket Repository

```bash
# Create a new GitHub repository
gh repo create PyRo1121/scoop-omg --public --description "Official Scoop bucket for OMG"

# Clone and initialize
git clone https://github.com/PyRo1121/scoop-omg.git
cd scoop-omg

# Copy bucket files from main repo
cp ../omg/scoop-bucket/* .
cp ../omg/scoop-bucket/.github/workflows/excavator.yml .github/workflows/

# Initial commit
git add .
git commit -m "Initial Scoop bucket setup"
git push origin main
```

### 2. Configure Excavator (Auto-updater)

Excavator automatically updates the manifest when new releases are published.

**Required GitHub Secrets:**
1. Go to `https://github.com/PyRo1121/scoop-omg/settings/secrets/actions`
2. Add:
   - `GITH_EMAIL`: Your GitHub email
   - `GITHUB_TOKEN`: Personal access token with `repo` scope

Excavator runs every 6 hours and checks for new releases.

### 3. Test the Bucket

```powershell
# Add bucket
scoop bucket add omg https://github.com/PyRo1121/scoop-omg

# Install OMG
scoop install omg

# Verify
omg --version
```

## 📝 Updating the Manifest

### Manual Update (for testing)

1. Update version in `omg.json`:
   ```json
   {
     "version": "0.1.205",
     "architecture": {
       "64bit": {
         "url": "https://github.com/PyRo1121/omg/releases/download/v0.1.205/omg-v0.1.205-x86_64-windows.zip"
       }
     }
   }
   ```

2. Calculate SHA256 hash:
   ```powershell
   # Download the release
   $url = "https://github.com/PyRo1121/omg/releases/download/v0.1.205/omg-v0.1.205-x86_64-windows.zip"
   Invoke-WebRequest -Uri $url -OutFile omg.zip
   
   # Get hash
   (Get-FileHash -Path omg.zip -Algorithm SHA256).Hash
   ```

3. Add hash to manifest:
   ```json
   "hash": "abc123..."
   ```

4. Test locally:
   ```powershell
   scoop install ./omg.json
   ```

### Automatic Update (Excavator)

Excavator will automatically:
1. Detect new releases from GitHub
2. Download the asset
3. Calculate SHA256 hash
4. Update `omg.json`
5. Commit and push changes

No manual intervention needed after initial setup!

## 🔄 Release Workflow Integration

To fully integrate Scoop into your release process:

### Option A: Manual Trigger After Release

After publishing a GitHub release:
```powershell
# Manually trigger Excavator workflow
gh workflow run excavator.yml --repo PyRo1121/scoop-omg
```

### Option B: Automatic via GitHub Actions

Add to `.github/workflows/release.yml` in the main OMG repo:

```yaml
- name: Trigger Scoop Bucket Update
  if: github.event_name == 'push'
  run: |
    gh workflow run excavator.yml --repo PyRo1121/scoop-omg
  env:
    GITHUB_TOKEN: ${{ secrets.PERSONAL_ACCESS_TOKEN }}
```

## 📊 User Installation Experience

Once set up, Windows users can install OMG via:

### First-time Setup:
```powershell
# 1. Install Scoop (if needed)
irm get.scoop.sh | iex

# 2. Add OMG bucket
scoop bucket add omg https://github.com/PyRo1121/scoop-omg

# 3. Install OMG
scoop install omg
```

### Updates:
```powershell
scoop update omg
```

### Benefits:
- ✅ Automatic PATH management
- ✅ Clean uninstall: `scoop uninstall omg`
- ✅ Version management: `scoop list`
- ✅ Auto-updates with Excavator
- ✅ No admin rights required

## 🔍 Verification

Test the complete flow:

```powershell
# Remove if already installed
scoop uninstall omg

# Fresh install
scoop bucket add omg https://github.com/PyRo1121/scoop-omg
scoop install omg

# Verify
omg --version
omg search vim

# Update
scoop update omg
```

## 📚 Resources

- [Scoop Documentation](https://scoop.sh)
- [App Manifest Specification](https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests)
- [Excavator Documentation](https://github.com/ScoopInstaller/GithubActions)

## ⚡ Quick Checklist

- [ ] Create `PyRo1121/scoop-omg` repository
- [ ] Add Excavator workflow
- [ ] Configure GitHub secrets (GITH_EMAIL, GITHUB_TOKEN)
- [ ] Copy `omg.json` manifest
- [ ] Add README with installation instructions
- [ ] Test locally: `scoop install ./omg.json`
- [ ] Push to GitHub
- [ ] Verify Excavator runs successfully
- [ ] Update main OMG docs with Scoop instructions
- [ ] Add Scoop badge to README: `[![Scoop Version](https://img.shields.io/scoop/v/omg)](https://scoop.sh)`

---

**Next Steps:**
1. Create the repository: `gh repo create PyRo1121/scoop-omg --public`
2. Copy files: `cp -r scoop-bucket/* /path/to/scoop-omg/`
3. Configure secrets in GitHub repo settings
4. Test installation
5. Announce to Windows users! 🎉
