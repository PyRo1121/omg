# SolidStart Migration: Comprehensive Checklist

## Pre-Migration Preparation

### Backup & Version Control
- [ ] Create feature branch: `git checkout -b feat/solidstart-migration`
- [ ] Ensure all uncommitted changes are committed
- [ ] Tag current production version: `git tag v1.0-pre-solidstart`
- [ ] Backup `dist/` folder locally
- [ ] Document current Lighthouse scores:
  - [ ] Desktop Performance: ____/100
  - [ ] Mobile Performance: ____/100
  - [ ] SEO: ____/100
  - [ ] Accessibility: ____/100
  - [ ] Best Practices: ____/100

### Environment Setup
- [ ] Node.js version: `node --version` (should be 20+)
- [ ] Bun version: `bun --version` (should be 1.0+)
- [ ] Disk space: `df -h` (ensure >5GB free)
- [ ] Clean `node_modules`: `rm -rf node_modules && bun install`

---

## Phase 1: Install Dependencies (30 min)

### Install SolidStart
- [ ] Run: `bun add @solidjs/start @solidjs/meta vinxi`
- [ ] Run: `bun add -D @solidjs/start-static`
- [ ] Verify installation: `ls node_modules/@solidjs/start`

### Remove Vite
- [ ] Run: `bun remove vite vite-plugin-solid`
- [ ] Verify removal: `ls node_modules/vite` (should fail)

### Verify Dependencies
- [ ] Check `package.json` for:
  - [ ] `"@solidjs/start": "^1.0.11"`
  - [ ] `"@solidjs/meta": "^0.29.4"`
  - [ ] `"vinxi": "^0.5.5"`
  - [ ] `"@solidjs/start-static": "^1.0.0"` (devDependencies)
  - [ ] No `vite` or `vite-plugin-solid`

---

## Phase 2: Configuration Files (1 hour)

### Create app.config.ts
- [ ] Create file: `touch app.config.ts`
- [ ] Add SolidStart config (see `MIGRATION_QUICKSTART.md`)
- [ ] Verify syntax: `bunx tsc --noEmit app.config.ts`

### Create src/entry-server.tsx
- [ ] Create file: `touch src/entry-server.tsx`
- [ ] Import `./index.css` at top
- [ ] Add `StartServer` component
- [ ] Include Cloudflare Analytics script
- [ ] Verify syntax: `bunx tsc --noEmit src/entry-server.tsx`

### Create src/entry-client.tsx
- [ ] Create file: `touch src/entry-client.tsx`
- [ ] Add `mount(() => <StartClient />)`
- [ ] Verify syntax: `bunx tsc --noEmit src/entry-client.tsx`

### Update package.json Scripts
- [ ] Change `"dev": "vinxi dev"`
- [ ] Change `"build": "vinxi build"`
- [ ] Change `"preview": "vinxi preview"`
- [ ] Keep other scripts unchanged (lint, typecheck, etc.)

### Update tsconfig.json
- [ ] Add `"types": ["vinxi/client"]` to compilerOptions
- [ ] Verify: `bun run typecheck`

---

## Phase 3: Create Routes (2 hours)

### Create Routes Directory
- [ ] Run: `mkdir -p src/routes/docs`
- [ ] Verify: `ls -la src/routes/`

### Migrate HomePage
- [ ] Create: `src/routes/index.tsx`
- [ ] Copy content from `src/pages/HomePage.tsx`
- [ ] Remove `Component` type, change to `export default function Home()`
- [ ] Import `Title` and `Meta` from `@solidjs/meta`
- [ ] Add SEO meta tags at top of return statement
- [ ] Wrap client-only code with `if (!isServer)`
- [ ] Test compile: `bunx tsc --noEmit src/routes/index.tsx`

### Migrate Dashboard
- [ ] Create: `src/routes/dashboard.tsx`
- [ ] Import `DashboardPage` with `lazy()`
- [ ] Add `<Title>` and `<Meta name="robots" content="noindex">`
- [ ] Wrap in `<Suspense fallback={<PageLoader />}>`
- [ ] Test compile: `bunx tsc --noEmit src/routes/dashboard.tsx`

### Migrate Docs
- [ ] Create: `src/routes/docs/index.tsx`
- [ ] Import `DocsPage` with `lazy()`
- [ ] Add `<Title>` and `<Meta>` tags
- [ ] Test compile: `bunx tsc --noEmit src/routes/docs/index.tsx`

### Add Dynamic Docs Route (Optional)
- [ ] Create: `src/routes/docs/[...slug].tsx`
- [ ] Use `useParams<{ slug: string }>()`
- [ ] Add dynamic title: `<Title>{params.slug} — OMG Docs</Title>`

---

## Phase 4: Handle Client-Only Code (1 hour)

### Three.js Component
- [ ] Edit: `src/components/3d/BackgroundMesh.tsx`
- [ ] Import: `import { isServer } from 'solid-js/web'`
- [ ] Add guard: `if (isServer || !containerRef) return;` in `onMount()`
- [ ] Test compile: `bunx tsc --noEmit src/components/3d/BackgroundMesh.tsx`

### HomePage Three.js Loading
- [ ] Edit: `src/routes/index.tsx`
- [ ] Ensure `lazy(() => import('../components/3d/BackgroundMesh'))`
- [ ] Verify deferred load: `requestIdleCallback(() => setShow3D(true), { timeout: 8000 })`
- [ ] Wrap in: `if (!isServer)` check

### Sentry Integration
- [ ] Edit: `src/routes/index.tsx`
- [ ] Move Sentry init to `onMount(() => { if (import.meta.env.PROD) { ... } })`
- [ ] Use dynamic import: `const Sentry = await import('@sentry/solid')`
- [ ] Defer to 15s: `requestIdleCallback(async () => { ... }, { timeout: 15000 })`

---

## Phase 5: Testing (3 hours)

### Local Development Testing
- [ ] Run: `bun run dev`
- [ ] Test: Navigate to `http://localhost:3000`
  - [ ] Homepage loads without errors
  - [ ] Three.js background appears after 8s
  - [ ] Pricing modal opens
  - [ ] License fetch works
- [ ] Test: Navigate to `http://localhost:3000/dashboard`
  - [ ] Dashboard loads
  - [ ] No 404 errors
  - [ ] Admin components render
- [ ] Test: Navigate to `http://localhost:3000/docs`
  - [ ] Docs page loads
  - [ ] Markdown renders correctly
- [ ] Check browser console: **Zero errors**
- [ ] Check Network tab: No failed requests

### Build Testing
- [ ] Run: `bun run build`
- [ ] Check build output:
  - [ ] `dist/index.html` exists (>40KB)
  - [ ] `dist/dashboard/index.html` exists
  - [ ] `dist/docs/index.html` exists
  - [ ] `dist/assets/` contains JS and CSS files
- [ ] Run: `bun run preview`
- [ ] Test production build at `http://localhost:3000`
  - [ ] All routes work
  - [ ] No console errors
  - [ ] Three.js loads correctly

### SEO Validation
- [ ] Check meta tags in pre-rendered HTML:
  ```bash
  cat dist/index.html | grep -A 20 "<head>"
  ```
  - [ ] `<title>OMG — Fastest Linux Package Manager | 22x Faster</title>`
  - [ ] `<meta name="description" content="...">`
  - [ ] `<meta property="og:title" content="...">`
  - [ ] `<meta property="og:image" content="...">`
  - [ ] `<meta name="twitter:card" content="summary_large_image">`

### Lighthouse Audit (Local)
- [ ] Run: `lighthouse http://localhost:3000 --view`
- [ ] Record scores:
  - [ ] Performance: ____/100 (target: 85+)
  - [ ] SEO: ____/100 (target: 100)
  - [ ] Accessibility: ____/100
  - [ ] Best Practices: ____/100
  - [ ] TBT (Desktop): ____ms (target: <200ms)
  - [ ] TBT (Mobile): ____ms (target: <100ms)

### Regression Testing
- [ ] **Homepage:**
  - [ ] Hero section renders
  - [ ] Benchmarks display correctly
  - [ ] Pricing cards show
  - [ ] Installation section visible
  - [ ] Footer links work
- [ ] **Payment Flow:**
  - [ ] Add `?success=true` to URL
  - [ ] Payment success modal opens
  - [ ] Email input works
  - [ ] License fetch API call succeeds
  - [ ] License key displays
  - [ ] Copy to clipboard works
- [ ] **Dashboard:**
  - [ ] Protected route redirects (if auth enabled)
  - [ ] Admin UI renders
  - [ ] Charts display
  - [ ] Real-time data updates
- [ ] **Docs:**
  - [ ] Markdown renders
  - [ ] Code blocks highlighted
  - [ ] Navigation works
  - [ ] Table of contents displays
- [ ] **Mobile:**
  - [ ] Responsive design works
  - [ ] Touch interactions functional
  - [ ] No horizontal scroll
  - [ ] Hamburger menu works (if present)

---

## Phase 6: Deploy to Staging (1 hour)

### Pre-Deployment Checks
- [ ] All tests passing: `bun run typecheck && bun run lint`
- [ ] Build succeeds: `bun run build`
- [ ] Git status clean: `git status`
- [ ] Commit changes: `git commit -m "feat: migrate to SolidStart SSG"`

### Deploy to Cloudflare Pages (Preview)
- [ ] Push to feature branch: `git push origin feat/solidstart-migration`
- [ ] Cloudflare Pages creates preview deployment
- [ ] Copy preview URL: `https://feat-solidstart-migration.omg-site.pages.dev`
- [ ] Test preview deployment:
  - [ ] All routes work
  - [ ] No 404 errors
  - [ ] Meta tags correct (view source)
  - [ ] Three.js loads
  - [ ] Analytics tracking works

### Lighthouse Audit (Preview Deployment)
- [ ] Run: `lighthouse https://feat-solidstart-migration.omg-site.pages.dev --view`
- [ ] Record scores:
  - [ ] Performance: ____/100
  - [ ] SEO: ____/100
  - [ ] Accessibility: ____/100
  - [ ] Best Practices: ____/100

---

## Phase 7: Deploy to Production (30 min)

### Final Checks
- [ ] Preview deployment tested and approved
- [ ] All Lighthouse scores meet targets (SEO ≥ 100)
- [ ] No console errors in production build
- [ ] Stakeholder approval received

### Merge to Main
- [ ] Create PR: `gh pr create --title "feat: SolidStart SSG migration" --body "..."`
- [ ] Review code changes
- [ ] Merge PR: `gh pr merge --squash`
- [ ] Delete feature branch: `git branch -D feat/solidstart-migration`

### Monitor Production Deployment
- [ ] Cloudflare Pages auto-deploys `main` branch
- [ ] Wait for build to complete (~2-3 minutes)
- [ ] Check build logs: No errors
- [ ] Verify live site: `https://pyro1121.com`
  - [ ] Homepage loads
  - [ ] All routes work
  - [ ] Meta tags correct (view source)

---

## Phase 8: Post-Deployment Monitoring (1 week)

### Immediate (First Hour)
- [ ] Monitor Cloudflare Analytics: Traffic spike/drop
- [ ] Check Sentry for new errors: `https://sentry.io`
- [ ] Test critical user flows:
  - [ ] Homepage → Pricing → Payment
  - [ ] Dashboard login
  - [ ] Docs navigation
- [ ] Run Lighthouse audit on production
- [ ] Verify Google Analytics tracking

### Day 1
- [ ] Monitor Core Web Vitals in Cloudflare
- [ ] Check for 404 errors in server logs
- [ ] Test on 5 browsers:
  - [ ] Chrome (latest)
  - [ ] Firefox (latest)
  - [ ] Safari (latest)
  - [ ] Edge (latest)
  - [ ] Brave (latest)
- [ ] Test on mobile devices:
  - [ ] iOS Safari
  - [ ] Android Chrome

### Week 1
- [ ] Monitor Google Search Console:
  - [ ] New pages indexed
  - [ ] Crawl errors (should be 0)
  - [ ] Mobile usability issues (should be 0)
- [ ] Compare traffic to previous week:
  - [ ] Organic search traffic (expect increase)
  - [ ] Bounce rate (expect decrease)
  - [ ] Average session duration (expect increase)
- [ ] Submit new sitemap: `https://pyro1121.com/sitemap.xml`
- [ ] Request re-indexing for changed pages

### Week 2
- [ ] Analyze SEO rankings for target keywords:
  - [ ] "OMG package manager"
  - [ ] "Linux package manager"
  - [ ] "pacman alternative"
  - [ ] "nvm alternative"
- [ ] Compare conversion rates (signups, license purchases)
- [ ] Review user feedback (support tickets, social media)

---

## Rollback Plan (If Issues Arise)

### Immediate Rollback (Critical Errors)
If the site is broken or SEO tanks:

1. [ ] Revert Git commit:
   ```bash
   git revert HEAD
   git push origin main
   ```
2. [ ] Cloudflare Pages auto-deploys previous version
3. [ ] Monitor build logs
4. [ ] Verify site is restored
5. [ ] Investigate issue in feature branch

### Full Rollback (Migration Failed)
If SolidStart migration is abandoned:

1. [ ] Checkout previous commit:
   ```bash
   git checkout v1.0-pre-solidstart
   ```
2. [ ] Create revert branch:
   ```bash
   git checkout -b revert/solidstart-rollback
   ```
3. [ ] Restore Vite dependencies:
   ```bash
   bun add -D vite vite-plugin-solid
   bun remove @solidjs/start @solidjs/start-static vinxi
   ```
4. [ ] Restore files:
   ```bash
   git checkout HEAD -- vite.config.ts src/App.tsx src/index.tsx
   ```
5. [ ] Delete SolidStart files:
   ```bash
   rm -rf app.config.ts src/entry-*.tsx src/routes/
   ```
6. [ ] Rebuild:
   ```bash
   bun run build
   ```
7. [ ] Test locally:
   ```bash
   bun run dev
   ```
8. [ ] Push to production:
   ```bash
   git push origin revert/solidstart-rollback
   ```

---

## Success Criteria

### Performance Metrics
- [ ] SEO Score: **100/100** (was 92/100)
- [ ] Desktop Performance: **≥85/100** (was 76/100)
- [ ] Mobile Performance: **≥95/100** (was 90/100)
- [ ] First Contentful Paint: **<1.0s** (was ~1.2s)
- [ ] Total Blocking Time (Desktop): **<200ms** (was 390ms)
- [ ] Total Blocking Time (Mobile): **<100ms** (was 110ms)
- [ ] Bundle Size: **≤3.8MB** (same as before)

### SEO Validation
- [ ] All routes have unique `<title>` tags
- [ ] All routes have unique `<meta name="description">` tags
- [ ] Open Graph tags present on all routes
- [ ] Twitter Card tags present on all routes
- [ ] Structured data (JSON-LD) preserved
- [ ] Canonical URLs correct
- [ ] No duplicate content issues

### User Experience
- [ ] Zero console errors on any page
- [ ] All interactive elements functional
- [ ] Three.js background animates smoothly
- [ ] Payment flow works end-to-end
- [ ] Mobile responsive design intact
- [ ] No layout shifts (CLS < 0.1)

### Business Metrics (Week 1)
- [ ] Organic traffic increased by ≥10%
- [ ] Bounce rate decreased by ≥5%
- [ ] Average session duration increased
- [ ] Conversion rate maintained or improved
- [ ] No increase in support tickets

---

## Troubleshooting Guide

### Issue: "vinxi: command not found"
**Symptoms:** Build fails with "vinxi: command not found"  
**Cause:** Vinxi not installed or not in PATH  
**Fix:**
```bash
bun add -D vinxi
bun run build
```

---

### Issue: Three.js crashes with "window is not defined"
**Symptoms:** Build fails during SSR with ReferenceError  
**Cause:** Three.js tries to access browser APIs during server rendering  
**Fix:**
```tsx
import { isServer } from 'solid-js/web';

onMount(() => {
  if (isServer) return; // Add this guard
  // Three.js code...
});
```

---

### Issue: Hydration mismatch warning
**Symptoms:** Console warning: "Hydration mismatch between server and client"  
**Cause:** Server-rendered HTML differs from client render  
**Fix:**
```tsx
const [clientOnlyState, setClientOnlyState] = createSignal(null);

createEffect(() => {
  if (!isServer) {
    setClientOnlyState(window.localStorage.getItem('key'));
  }
});
```

---

### Issue: Tailwind styles missing
**Symptoms:** Site loads with no styles  
**Cause:** CSS not imported in entry-server.tsx  
**Fix:**
```tsx
// src/entry-server.tsx
import "./index.css"; // Add this line at top
```

---

### Issue: Cloudflare Pages build fails
**Symptoms:** Build fails with Node.js version error  
**Cause:** Cloudflare Pages using wrong Node.js version  
**Fix:**
1. Go to Cloudflare Pages dashboard
2. Settings → Build Configuration → Environment Variables
3. Add: `NODE_VERSION = 20`
4. Retry deployment

---

### Issue: Routes return 404 in production
**Symptoms:** Direct navigation to `/dashboard` returns 404  
**Cause:** Cloudflare Pages needs redirect rules for SPA  
**Fix:**
Create `public/_redirects` file:
```
/*    /index.html   200
```

---

### Issue: SEO score still 92/100
**Symptoms:** Lighthouse SEO score didn't improve  
**Cause:** Missing meta tags or incorrect Open Graph tags  
**Fix:**
1. View source: `curl https://pyro1121.com`
2. Verify all meta tags present
3. Check structured data: `https://validator.schema.org/`
4. Run: `lighthouse https://pyro1121.com --only-categories=seo --view`

---

## Contacts & Resources

### Documentation
- SolidStart: https://start.solidjs.com/
- SolidJS: https://www.solidjs.com/
- Tailwind CSS v4: https://tailwindcss.com/docs
- Cloudflare Pages: https://developers.cloudflare.com/pages/

### Support
- SolidStart Discord: https://discord.com/invite/solidjs
- Migration Guide: `SOLIDSTART_MIGRATION_PLAN.md`
- Quick Start: `MIGRATION_QUICKSTART.md`
- Architecture: `ARCHITECTURE_DIAGRAM.md`

---

## Sign-Off

**Migration Checklist Completed By:**
- [ ] Developer: ________________
- [ ] Reviewer: ________________
- [ ] QA: ________________
- [ ] Approved for Production: ________________

**Date:** ________________

**Production URL:** https://pyro1121.com  
**Preview URL:** https://feat-solidstart-migration.omg-site.pages.dev  

**Notes:**
_____________________________________________________________
_____________________________________________________________
_____________________________________________________________

---

**Document Version:** 1.0  
**Last Updated:** 2026-01-29  
**Next Review Date:** 2026-02-05 (1 week post-deployment)
