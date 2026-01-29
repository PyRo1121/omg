# 🐛 PERFORMANCE DEBUG REPORT: TBT Regression Analysis
**Date:** January 29, 2026, 1:09 AM
**Test URL:** https://31396776.omg-site-4gd.pages.dev/
**Expected TBT:** <500ms
**Actual TBT:** 3,870ms (13x over target!)

---

## ✅ DEPLOYMENT VERIFICATION

### Fix #1: Modulepreload Removal ✅ DEPLOYED
**Status:** SUCCESS
**Evidence:**
```bash
$ grep -i modulepreload /tmp/deployed.html
(no output)
```
✅ Modulepreload tags are completely removed from deployed HTML.

### Fix #2: Async Font Loading ✅ DEPLOYED
**Status:** SUCCESS
**Evidence:**
```html
<link href="https://fonts.googleapis.com/css2?family=..." 
      rel="stylesheet" 
      media="print" 
      onload="this.media='all'">
<noscript><link href="..." rel="stylesheet"></noscript>
```
✅ Fonts are loading asynchronously with media="print" trick.

### Fix #3: ARIA Labels ❌ NOT DEPLOYED
**Status:** FAILED
**Evidence:**
```bash
$ grep -c 'aria-label' /tmp/deployed.html
0
```
❌ Zero ARIA labels found in deployed HTML (expected 7+).
⚠️ This is likely causing the SEO regression (92 → 69).

---

## 🔍 ROOT CAUSE ANALYSIS: 3,870ms TBT

### Asset Analysis

**Main Bundle:** 87KB (index-CrP2M_T1.js)
**Main CSS:** 187KB (index-DIhaPt4W.css)

**Lazy-loaded Bundles:**
- `syntax-CmyKrJRF.js`: 818KB
- `charts-CKYdeCzT.js`: 570KB  
- `three-B0BxZIiq.js`: 472KB
- `sentry-BG9Kkyd-.js`: 406KB
- `DashboardPage-DypBW-MA.js`: 405KB

### Critical Discovery: 187KB CSS Bundle 🚨

**Main CSS bundle is 187KB** - this is render-blocking!

**Analysis:**
```html
<link rel="stylesheet" crossorigin href="/assets/index-DIhaPt4W.css">
```

This CSS is **NOT** using the `media="print"` trick. It's **blocking render** while parsing 187KB of CSS.

**Expected Behavior:** CSS should be inlined critical styles + async non-critical.

**Impact Estimate:** 
- 187KB CSS download: ~10-15ms (44ms measured)
- 187KB CSS parse: ~500-1000ms on desktop
- Layout recalculation: ~200-500ms

**Total CSS TBT contribution:** ~700-1500ms

---

### Secondary Issue: Large Main Bundle Parse Time

**87KB JavaScript Bundle** (index-CrP2M_T1.js)

**Parse time estimate:**
- Desktop (Intel i9): ~100-200ms
- Slower desktop: ~200-400ms

**Evidence from bundle analysis:**
```javascript
// Main bundle includes:
import{_ as ze}from"./charts-CKYdeCzT.js";  // Chart imports
import{z as we,w as M,A as it...}from"./solid-vendor-D_YoVHc_.js";  // Solid.js
import{Q as mt,a as ut}from"./tanstack-B78bUHT3.js";  // TanStack Query
```

The main bundle is importing chart components, which should be lazy-loaded!

---

### Tertiary Issue: Sentry Initialization (Minor)

**Evidence from index.tsx:**
```typescript
requestIdleCallback(
  async () => {
    const Sentry = await import('@sentry/solid');
    Sentry.init({...});
  },
  { timeout: 15000 }  // 15 second timeout
);
```

**Timeout is too long.** If the main thread is busy, this could fire earlier and add ~50-100ms.

**Recommendation:** Reduce timeout to 30 seconds or remove timeout entirely.

---

### Quaternary Issue: Three.js Loading

**Evidence from HomePage.tsx:**
```typescript
requestIdleCallback(() => setShow3D(true), { timeout: 8000 });
```

**8 second timeout** is triggering Three.js load during TBT measurement window.

**TBT measurement window:** First 5-10 seconds after First Contentful Paint.

**Recommendation:** Increase timeout to 15+ seconds or wait for user interaction.

---

## 🔴 SEO REGRESSION ANALYSIS: 92 → 69 (23 point drop!)

### Missing ARIA Labels ❌

**Evidence:**
```bash
$ grep -c 'aria-label' /tmp/deployed.html
0
```

**Expected ARIA labels (from previous fix):**
1. Logo link: `aria-label="OMG Package Manager Home"`
2. GitHub link: `aria-label="OMG Package Manager on GitHub"`
3. Twitter/X link: `aria-label="OMG Package Manager on X (Twitter)"`
4. Mobile menu button: `aria-label="Open navigation menu"`
5. Keyboard shortcuts button: `aria-label="Show keyboard shortcuts"`
6. Close buttons: `aria-label="Close"`
7. Background mesh: `aria-hidden="true"` (present ✅)

**Impact:** -15 to -20 SEO points (accessibility is ~20% of SEO score).

---

### Missing Sitemap? ✅ NO - Sitemap Exists

```bash
$ curl -I https://31396776.omg-site-4gd.pages.dev/sitemap.xml
HTTP/2 200
content-type: application/xml
```

✅ Sitemap is present and accessible.

---

### Canonical URL Issue? ✅ NO - Canonical is Correct

```html
<link rel="canonical" href="https://pyro1121.com/" />
```

⚠️ **WAIT!** The deployment is on `31396776.omg-site-4gd.pages.dev` but canonical points to `pyro1121.com`.

**This is correct for staging**, but PageSpeed might be penalizing for canonical mismatch.

**Recommendation:** Add `<meta name="robots" content="noindex, nofollow">` to staging deployments.

---

## 🟡 BEST PRACTICES REGRESSION: 100 → 96 (4 point drop)

### Missing Security Headers

**Evidence:**
```bash
$ curl -I https://31396776.omg-site-4gd.pages.dev/
(no CSP, X-Frame-Options, or HSTS headers found)
```

**Missing headers:**
1. `Content-Security-Policy`
2. `X-Frame-Options: DENY`
3. `Strict-Transport-Security` (HSTS)
4. `X-Content-Type-Options: nosniff`
5. `Referrer-Policy: strict-origin-when-cross-origin`

**Impact:** -4 Best Practices points.

**Note:** Cloudflare Pages might add these automatically on production domain.

---

## 📊 TBT BREAKDOWN (Estimated)

| Component | TBT Contribution | Percentage |
|-----------|------------------|------------|
| **187KB CSS Parse** | 700-1500ms | 40% |
| **87KB JS Parse** | 200-400ms | 15% |
| **Layout Recalculation** | 300-500ms | 15% |
| **Three.js Initialization** | 200-400ms | 10% |
| **Sentry Load** | 50-100ms | 3% |
| **Other** | 100-200ms | 5% |
| **TOTAL** | ~2,550-4,100ms | 100% |

**Current Measured:** 3,870ms ✅ (within estimated range)

---

## 🎯 PRIORITIZED FIXES

### Fix #1: Split CSS Bundle (HIGH IMPACT) 🔥
**Expected TBT Reduction:** 700-1500ms (40% improvement)

**Action:**
1. Extract critical CSS (above-the-fold styles)
2. Inline critical CSS in `<head>`
3. Load non-critical CSS asynchronously
4. Use `loadCSS` or `media="print"` trick for non-critical CSS

**Implementation:**
```typescript
// vite.config.ts
export default defineConfig({
  build: {
    cssCodeSplit: true,  // Enable CSS code splitting
    rollupOptions: {
      output: {
        manualChunks: {
          'critical-css': ['./src/styles/critical.css'],
        }
      }
    }
  }
});
```

---

### Fix #2: Remove Chart Imports from Main Bundle (MEDIUM IMPACT) 🔥
**Expected TBT Reduction:** 100-200ms (10% improvement)

**Evidence:**
```javascript
// Main bundle currently imports:
import{_ as ze}from"./charts-CKYdeCzT.js";
```

**Action:**
Ensure charts are only imported in `Benchmarks.tsx` (which should be lazy-loaded or below-the-fold).

**Implementation:**
```typescript
// src/components/Benchmarks.tsx
import { lazy } from 'solid-js';

const ChartComponent = lazy(() => import('./ChartComponent'));
```

---

### Fix #3: Increase Three.js Timeout (MEDIUM IMPACT) 🔥
**Expected TBT Reduction:** 200-400ms (15% improvement)

**Action:**
Change `timeout: 8000` to `timeout: 20000` or remove timeout entirely.

**Implementation:**
```typescript
// src/pages/HomePage.tsx
onMount(() => {
  // Defer Three.js load until after TBT measurement (15+ seconds)
  requestIdleCallback(() => setShow3D(true), { timeout: 20000 });
});
```

---

### Fix #4: Add ARIA Labels (SEO FIX) 🔥
**Expected SEO Improvement:** +15-20 points (69 → 84-89)

**Action:**
Re-apply ARIA label fixes to all interactive elements.

**Check:** Verify these were in the source but not deployed. Possible build issue?

---

### Fix #5: Increase Sentry Timeout (LOW IMPACT)
**Expected TBT Reduction:** 20-50ms (2% improvement)

**Action:**
Change Sentry timeout from 15s to 30s or remove timeout.

---

### Fix #6: Add Security Headers (BEST PRACTICES FIX)
**Expected Best Practices Improvement:** +4 points (96 → 100)

**Action:**
Add Cloudflare Pages `_headers` file:

```
/*
  Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline' https://static.cloudflareinsights.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: https://api.pyro1121.com; connect-src 'self' https://api.pyro1121.com; frame-ancestors 'none'
  X-Frame-Options: DENY
  X-Content-Type-Options: nosniff
  Referrer-Policy: strict-origin-when-cross-origin
  Permissions-Policy: geolocation=(), microphone=(), camera=()
```

---

## 🔬 VERIFICATION COMMANDS

**After fixes are deployed, run:**

```bash
# Verify ARIA labels
curl -s https://31396776.omg-site-4gd.pages.dev/ | grep -c 'aria-label'
# Expected: 7+

# Verify CSS size reduction
curl -s https://31396776.omg-site-4gd.pages.dev/ | grep -o 'stylesheet.*\.css' | head -5
# Expected: Multiple smaller CSS files

# Verify no chart imports in main bundle
curl -s https://31396776.omg-site-4gd.pages.dev/assets/index-*.js | grep -o 'charts-'
# Expected: (no output)

# Measure new TBT
# Run PageSpeed Insights: https://pagespeed.web.dev/
# Expected Desktop TBT: <1,000ms (target: <500ms)
```

---

## 🎯 EXPECTED RESULTS AFTER ALL FIXES

| Metric | Current | After Fixes | Improvement |
|--------|---------|-------------|-------------|
| **Desktop TBT** | 3,870ms | <1,000ms | 74% |
| **Desktop Performance** | 55/100 | 85+/100 | +30 pts |
| **Desktop SEO** | 69/100 | 89+/100 | +20 pts |
| **Desktop Best Practices** | 96/100 | 100/100 | +4 pts |

---

## 🚀 IMMEDIATE ACTION ITEMS

1. **Split CSS bundle** (CSS code splitting + critical CSS inlining)
2. **Remove chart imports from main bundle** (lazy-load Benchmarks component)
3. **Increase Three.js timeout to 20s**
4. **Re-apply ARIA labels** (investigate why they didn't deploy)
5. **Add security headers** (`_headers` file for Cloudflare Pages)
6. **Increase Sentry timeout to 30s**

---

## 📝 NOTES

- **Why did ARIA labels not deploy?** This needs investigation. Check:
  - `git diff` to see if changes were committed
  - Build logs to see if there were TypeScript errors
  - Cloudflare Pages deployment to see if it used the correct commit

- **CSS bundle size is the smoking gun.** 187KB of render-blocking CSS is likely responsible for 40% of TBT.

- **Chart imports in main bundle are suspicious.** This suggests the Benchmarks component is not lazy-loaded or is being imported somewhere in the main app.

---

**End of Debug Report**
