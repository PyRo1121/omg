# Screenshots TODO

This document tracks visual assets needed for OMG documentation.

## Priority 1: README.md

### 1. TUI Dashboard Screenshot
**Location:** README.md, after "Interactive Dashboard" section (line ~147)
**Command to capture:** `omg dash`
**Purpose:** Show the full-screen TUI with:
- System status overview
- Installed packages list
- Runtime versions
- Security alerts
- Real-time activity log

**Markdown to add:**
```markdown
![OMG Dashboard](./docs/assets/omg-dash.png)
*Interactive TUI showing system status, packages, and security alerts*
```

### 2. Security Grading Display
**Location:** README.md, "Enterprise Security" section (line ~245)
**Command to capture:** `omg install ripgrep` (showing security grade output)
**Purpose:** Demonstrate security grading feature

**Markdown to add:**
```markdown
![Security Grading](./docs/assets/security-grade.png)
*Package security grade with vulnerability scan results*
```

### 3. Benchmark Comparison Graph
**Location:** README.md, after benchmark tables (line ~315)
**Tool:** Create chart with matplotlib/gnuplot from benchmark data
**Purpose:** Visual comparison of OMG vs pacman/yay/apt performance

**Markdown to add:**
```markdown
![Performance Comparison](./docs/assets/benchmark-comparison.png)
*OMG performance compared to pacman, yay, and apt-cache*
```

---

## Priority 2: docs/tui.md

### 4. Full Dashboard View
**Location:** docs/tui.md (create sections if needed)
**Command:** `omg dash`
**Purpose:** Show all dashboard panels

### 5. Interactive Search Demo
**Location:** docs/tui.md
**Command:** `omg dash` → Press `/` → Type search query
**Purpose:** Demonstrate interactive search with fuzzy matching

### 6. Package Details View
**Location:** docs/tui.md
**Command:** `omg dash` → Select a package → Press Enter
**Purpose:** Show package detail panel with dependencies, description, etc.

---

## Priority 3: docs/security.md

### 7. SBOM Generation Output
**Location:** docs/security.md, "SBOM Generation" section
**Command:** `omg sbom generate --format json`
**Purpose:** Show generated SBOM structure

### 8. Vulnerability Scan Results
**Location:** docs/security.md, "Vulnerability Scanning" section
**Command:** `omg audit scan`
**Purpose:** Show vulnerability scan output with CVE details

### 9. Audit Log Viewer
**Location:** docs/security.md, "Audit Logging" section
**Command:** `omg audit log --tail 20`
**Purpose:** Show tamper-proof audit log entries

---

## Priority 4: docs/quickstart.md

### 10. Installation Process
**Location:** docs/quickstart.md, "Install OMG" section
**Command:** Capture terminal output during installation
**Purpose:** Show what users see during install

### 11. First Command Output
**Location:** docs/quickstart.md, "Your First 5 Minutes" section
**Command:** `omg search neovim`
**Purpose:** Show expected search results with timing

### 12. Version Switching
**Location:** docs/quickstart.md, Step 4
**Command:** `omg use node 20` with progress bar
**Purpose:** Show runtime installation progress

---

## How to Create Screenshots

### Terminal Screenshots (Recommended: Termshot)

```bash
# Install termshot (or use asciinema + agg)
cargo install termshot

# Capture command output
termshot capture --command "omg dash" --output docs/assets/omg-dash.png

# Or use asciinema + agg
asciinema rec demo.cast -c "omg dash"
agg demo.cast docs/assets/omg-dash.gif
```

### Chart Generation (Benchmark Comparison)

```python
# benchmark-chart.py
import matplotlib.pyplot as plt
import numpy as np

categories = ['Search', 'Info', 'Status', 'Explicit']
omg = [6, 6.5, 7, 1.2]
pacman = [133, 138, None, 14]
yay = [150, 300, None, 27]

x = np.arange(len(categories))
width = 0.25

fig, ax = plt.subplots(figsize=(10, 6))
ax.bar(x - width, omg, width, label='OMG', color='#4CAF50')
ax.bar(x, [p if p else 0 for p in pacman], width, label='pacman', color='#FF9800')
ax.bar(x + width, [y if y else 0 for y in yay], width, label='yay', color='#F44336')

ax.set_ylabel('Time (ms)', fontsize=12)
ax.set_title('OMG Performance vs Traditional Package Managers', fontsize=14, fontweight='bold')
ax.set_xticks(x)
ax.set_xticklabels(categories)
ax.legend()
ax.set_ylim(0, 160)
ax.grid(axis='y', alpha=0.3)

plt.tight_layout()
plt.savefig('docs/assets/benchmark-comparison.png', dpi=300, bbox_inches='tight')
print("Chart saved to docs/assets/benchmark-comparison.png")
```

---

## Directory Structure

Create the following structure:

```
docs/
├── assets/
│   ├── omg-dash.png              # Priority 1
│   ├── security-grade.png        # Priority 1
│   ├── benchmark-comparison.png  # Priority 1
│   ├── tui-search.png           # Priority 2
│   ├── package-details.png      # Priority 2
│   ├── sbom-output.png          # Priority 3
│   ├── vulnerability-scan.png   # Priority 3
│   ├── audit-log.png            # Priority 3
│   ├── install-process.png      # Priority 4
│   ├── search-results.png       # Priority 4
│   └── version-switching.png    # Priority 4
└── SCREENSHOTS-TODO.md          # This file
```

---

## Checklist

### Priority 1 (README.md) - Do First
- [ ] omg-dash.png
- [ ] security-grade.png
- [x] benchmark-comparison.png ✅ (Generated + Added to README)

### Priority 2 (TUI Documentation)
- [ ] tui-search.png
- [ ] package-details.png

### Priority 3 (Security Documentation)
- [ ] sbom-output.png
- [ ] vulnerability-scan.png
- [ ] audit-log.png

### Priority 4 (Quick Start)
- [ ] install-process.png
- [ ] search-results.png
- [ ] version-switching.png

---

## Notes

- **Format:** Use PNG for screenshots (better compatibility)
- **Size:** Optimize images (<500KB each) with `optipng` or similar
- **Accessibility:** Add descriptive alt text to all images
- **Dark/Light Mode:** Capture in dark mode (matches most developer terminals)
- **Annotations:** Add arrows/highlights if needed using `imagemagick`:
  ```bash
  convert input.png -fill red -draw "circle 100,100 110,110" output.png
  ```

---

## After Screenshots Are Added

1. Update this checklist
2. Test all image links in docs
3. Optimize images: `optipng docs/assets/*.png`
4. Commit with message: `docs: add screenshots for [section]`
5. Update README.md and relevant docs with image references
