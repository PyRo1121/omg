# Docusaurus Documentation Site

Create a modern, ultra-polished Docusaurus documentation site that integrates with the existing SolidJS marketing site, using the current `docs/` markdown files as content source.

---

## Architecture Decision

| Option | Pros | Cons |
|--------|------|------|
| **Docusaurus in `docs-site/`** | Full-featured, versioning, search, i18n | Separate build, different stack |
| Starlight (Astro) | Modern, fast, lightweight | Less ecosystem than Docusaurus |
| VitePress | Vue-based, fast | Different ecosystem |

**Recommendation:** Docusaurus 3.x - best docs ecosystem, Algolia search, versioning, MDX support

---

## Implementation Plan

### Phase 1: Docusaurus Setup (~30 min)

1. **Create `docs-site/` directory** with Docusaurus 3.x
   ```bash
   cd /home/pyro1121/Documents/code/filemanager/omg
   npx create-docusaurus@latest docs-site classic --typescript
   ```

2. **Configure to read from `../docs/`** (symlink or copy strategy)
   - Option A: Symlink `docs-site/docs` → `../docs` (cleaner)
   - Option B: Build script copies docs (safer for deployment)

3. **Theme customization** to match main site:
   - Dark mode default (match SolidJS site)
   - Indigo/purple color scheme
   - OMG branding (logo, favicon)

### Phase 2: Content Migration (~1 hour)

1. **Convert existing docs to Docusaurus format:**
   - Add frontmatter (`title`, `sidebar_position`, `description`)
   - Organize into categories (Getting Started, Core Concepts, API Reference)
   - Add `_category_.json` files for sidebar organization

2. **Sidebar structure:**
   ```
   docs/
   ├── intro.md (index.md → intro.md)
   ├── getting-started/
   │   ├── installation.md
   │   ├── quickstart.md
   │   └── migration-guides/
   │       ├── from-yay.md
   │       ├── from-nvm.md
   │       └── from-pyenv.md
   ├── core-concepts/
   │   ├── architecture.md
   │   ├── daemon.md
   │   ├── cache.md
   │   └── security.md
   ├── cli-reference/
   │   ├── cli.md
   │   ├── cli-internals.md
   │   └── commands/
   ├── runtimes/
   │   └── runtimes.md
   └── advanced/
       ├── ipc.md
       ├── workflows.md
       └── troubleshooting.md
   ```

### Phase 3: Branding & Polish (~30 min)

1. **Custom CSS** matching main site:
   ```css
   :root {
     --ifm-color-primary: #6366f1; /* Indigo */
     --ifm-color-primary-dark: #4f46e5;
     --ifm-background-color: #0f172a; /* Slate-900 */
   }
   ```

2. **Homepage** with hero matching main site style

3. **Navbar** with link back to main site + dashboard

4. **Footer** matching main site footer

### Phase 4: Search & Features (~30 min)

1. **Local search** (docusaurus-search-local) - free, works offline
2. **Code blocks** with copy button, line highlighting
3. **Tabs** for multi-platform instructions
4. **Admonitions** (tip, warning, info, danger)

### Phase 5: Deployment Config (prep only)

1. **GitHub Actions workflow** (commented out, ready to enable):
   ```yaml
   # .github/workflows/docs.yml
   name: Deploy Docs
   on:
     push:
       branches: [main]
       paths: ['docs/**', 'docs-site/**']
   jobs:
     deploy:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         - uses: actions/setup-node@v4
         - run: cd docs-site && npm ci && npm run build
         - uses: peaceiris/actions-gh-pages@v4
           with:
             github_token: ${{ secrets.GITHUB_TOKEN }}
             publish_dir: ./docs-site/build
   ```

2. **Cloudflare Pages config** (alternative):
   - Build command: `cd docs-site && npm run build`
   - Output: `docs-site/build`

---

## File Structure After Implementation

```
omg/
├── docs/                    # Source markdown (stays here)
│   ├── index.md
│   ├── cli.md
│   └── ...
├── docs-site/               # Docusaurus project
│   ├── docusaurus.config.ts
│   ├── sidebars.ts
│   ├── src/
│   │   ├── css/custom.css
│   │   └── pages/index.tsx
│   ├── static/
│   │   └── img/
│   └── docs -> ../docs      # Symlink
├── site/                    # Existing SolidJS site
└── .github/
    └── workflows/
        └── docs.yml         # Commented out, ready to enable
```

---

## Integration with Main Site

1. **Header link:** Add "Docs" link in main site Header.tsx pointing to `/docs`
2. **Subdomain or path:** 
   - Option A: `docs.omg.dev` (separate deployment)
   - Option B: `omg.dev/docs` (unified, needs proxy config)

**Recommendation:** Separate subdomain `docs.omg.dev` - simpler deployment

---

## Migration Guides to Create

| Guide | Content |
|-------|---------|
| `from-yay.md` | yay → omg command mapping, config migration |
| `from-nvm.md` | nvm → omg use node, .nvmrc support |
| `from-pyenv.md` | pyenv → omg use python, .python-version support |
| `from-asdf.md` | asdf → omg, .tool-versions support |

---

## Timeline

| Phase | Time | Output |
|-------|------|--------|
| 1. Setup | 30 min | Working Docusaurus skeleton |
| 2. Content | 1 hour | All docs migrated with frontmatter |
| 3. Branding | 30 min | Matching theme |
| 4. Features | 30 min | Search, code blocks |
| 5. Deploy prep | 15 min | Workflow ready |

**Total: ~3 hours**

---

## Questions

~~Questions resolved - proceeding with implementation~~
