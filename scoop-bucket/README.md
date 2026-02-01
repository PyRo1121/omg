# OMG Scoop Bucket

Official [Scoop](https://scoop.sh) bucket for OMG - the fastest unified package manager.

## Installation

### 1. Install Scoop (if not already installed)

```powershell
irm get.scoop.sh | iex
```

### 2. Add the OMG bucket

```powershell
scoop bucket add omg https://github.com/PyRo1121/scoop-omg
```

### 3. Install OMG

```powershell
scoop install omg
```

## Quick Start

```powershell
# Search packages
omg search firefox

# Install packages
omg install <package>

# Use specific runtime versions
omg use node 20
omg use python 3.12

# Run project tasks
omg run dev
```

## Shell Integration

Add to your PowerShell profile (`$PROFILE`):

```powershell
Invoke-Expression (& omg hook powershell)
```

## Update OMG

```powershell
scoop update omg
```

## Documentation

- 🌐 Website: https://pyro1121.com
- 📚 Docs: https://pyro1121.com/docs
- 📝 Changelog: https://github.com/PyRo1121/omg/blob/main/docs/changelog.md
- 🐛 Issues: https://github.com/PyRo1121/omg/issues

## License

AGPL-3.0-or-later
