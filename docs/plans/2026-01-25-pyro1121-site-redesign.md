# Pyro1121.com "Terminal Hero" Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform `pyro1121.com` into a modern, unique product site for the `omg` CLI, featuring a 3D glass terminal hero, abstract mesh background, and integrated documentation.

**Architecture:** SolidJS + Vite single-page app. Content-driven documentation using Markdown. Visuals powered by Three.js (background) and CSS 3D (terminal).

**Tech Stack:** SolidJS, Tailwind CSS, Three.js, Solid Router, Unified/Remark (Markdown).

---

## Phase 1: Content Migration & Cleanup

### Task 1: Move Docs Content
**Goal:** Consolidate documentation into the main site structure.

**Files:**
- Move: `docs-site/docs/` -> `site/src/content/docs/`
- Delete: `docs-site/`

**Step 1: Create destination directory**
```bash
mkdir -p site/src/content
```

**Step 2: Move files**
```bash
cp -r docs-site/docs site/src/content/
```

**Step 3: Remove old project**
```bash
rm -rf docs-site
```

**Step 4: Commit**
```bash
git add site/src/content
git rm -r docs-site
git commit -m "chore: migrate docs content to site/src/content and remove docs-site"
```

---

## Phase 2: Dependencies & Configuration

### Task 2: Install Design & 3D Dependencies
**Goal:** Set up the tools needed for the visual effects and markdown rendering.

**Files:**
- Modify: `site/package.json`

**Step 1: Install Dependencies**
```bash
cd site
npm install three @types/three clsx tailwind-merge solid-markdown remark-gfm rehype-highlight
```
*(Note: Using `solid-markdown` for easy component rendering of MD)*

**Step 2: Configure Tailwind for 3D & Glass**
- Modify: `site/src/index.css` / `site/tailwind.config.js` (if exists, or use v4 CSS config)
- Add utility classes for `perspective`, `transform-style-3d`, `backface-hidden`.

**Step 3: Commit**
```bash
git add package.json package-lock.json src/index.css
git commit -m "chore: add dependencies for 3D, styling, and markdown"
```

---

## Phase 3: The "Abstract Mesh" Background

### Task 3: Create BackgroundMesh Component
**Goal:** Implement the glowing, flowing 3D mesh background using Three.js.

**Files:**
- Create: `site/src/components/3d/BackgroundMesh.tsx`

**Step 1: Scaffold Component**
Create a component that initializes a Three.js scene in `onMount`.

**Step 2: Implement Mesh Logic**
- Scene, Camera, Renderer (alpha: true).
- Geometry: `PlaneGeometry` with many segments.
- Material: `MeshPhongMaterial` or `ShaderMaterial` for the "wireframe/glowing" look.
- Animation: Verify vertex positions in `useFrame` (requestAnimationFrame) loop to create "flowing" wave effect.

**Step 3: Add to App Layout**
- Modify: `site/src/App.tsx` (or main layout) to include `<BackgroundMesh />` as a fixed background layer (`z-[-1]`).

**Step 4: Commit**
```bash
git add site/src/components/3d/BackgroundMesh.tsx site/src/App.tsx
git commit -m "feat: implement 3d abstract mesh background"
```

---

## Phase 4: The Glass Terminal Hero

### Task 4: Create GlassContainer & Terminal Component
**Goal:** The centerpiece 3D-tilted frosted glass window.

**Files:**
- Create: `site/src/components/ui/GlassCard.tsx`
- Create: `site/src/components/hero/HeroTerminal.tsx`

**Step 1: Create GlassCard**
- detailed CSS/Tailwind for: `backdrop-blur-xl`, `bg-white/5` (or dark equivalent), `border-white/10`, `shadow-2xl`.

**Step 2: Implement HeroTerminal**
- Use `GlassCard` as container.
- Add "Traffic Lights" (window controls).
- Add "Typewriter" effect logic:
  - State: `lines` (array of strings).
  - Effect: type characters one by one.
  - Content: A demo script showing `omg` commands (e.g., `omg install node`, `omg list`).

**Step 3: Apply 3D Tilt**
- Wrap `HeroTerminal` in a container with `perspective`.
- Apply `rotateX` / `rotateY` based on mouse position (parallax) or static tilt for elegance.

**Step 4: Commit**
```bash
git add site/src/components/ui/GlassCard.tsx site/src/components/hero/HeroTerminal.tsx
git commit -m "feat: add glass terminal component with typewriter effect"
```

---

## Phase 5: Landing Page Assembly

### Task 5: Build Landing Page
**Goal:** Assemble the Hero and feature sections.

**Files:**
- Modify: `site/src/pages/Home.tsx` (create if needed)
- Create: `site/src/components/landing/FeatureGrid.tsx`

**Step 1: Hero Section**
- Center `HeroTerminal` on screen.
- Add Headline ("The Package Manager for the Future", etc.) with "High-End Motion" fade-in.

**Step 2: Feature Grid**
- Create cards using `GlassCard` variant.
- Content: "Universal", "Sandboxed", "Fast".

**Step 3: Commit**
```bash
git add site/src/pages/Home.tsx site/src/components/landing/FeatureGrid.tsx
git commit -m "feat: assemble landing page with hero and features"
```

---

## Phase 6: Documentation Engine

### Task 6: Markdown Routing & Rendering
**Goal:** Serve the docs from `site/src/content/docs`.

**Files:**
- Create: `site/src/lib/docs.ts` (helper to load/parse MD)
- Create: `site/src/pages/Docs.tsx`
- Modify: `site/src/index.tsx` (routes)

**Step 1: Setup Routes**
- Add `/docs` and `/docs/*slug` routes.

**Step 2: Implement Docs Layout**
- Sidebar (left) listing all files in `content/docs`.
- Main content area (right) rendering the Markdown.

**Step 3: Connect Content**
- Import markdown files (using Vite's `import.meta.glob` or raw import).
- Render using `SolidMarkdown`.

**Step 4: Commit**
```bash
git add site/src/lib/docs.ts site/src/pages/Docs.tsx site/src/index.tsx
git commit -m "feat: implement documentation routing and rendering"
```
