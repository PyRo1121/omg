# Cloudflare Free Tier Setup Guide

This document tracks all Cloudflare resources configured for the OMG project.

## Resources Created via Wrangler

### D1 Databases
| Database | ID | Purpose |
|----------|-----|---------|
| omg-licensing | `bcaf7781-a747-4637-92d9-94782e4fa1db` | License management, user data |
| omg-analytics | `e11296b5-1c01-437a-9d22-2e3786c20932` | Download tracking, usage analytics |

### KV Namespaces
| Namespace | ID | Purpose |
|-----------|-----|---------|
| omg-cache | `305ad768c4b94f72bbf2225721e73a4b` | API response caching |
| omg-sessions | `1c7724b4fa6b4f0ca528d510b5009dd2` | User session storage |
| omg-flags | `7017da41ba6a4b528dc3ea018cd997bd` | Feature flags |

### R2 Buckets
| Bucket | Purpose |
|--------|---------|
| omg-releases | Binary releases storage |
| omg-assets | Static assets (images, fonts) |
| omg-docs-assets | Documentation assets |

## Dashboard Configuration Required

The following features need to be enabled in the Cloudflare Dashboard:

### 1. Web Analytics (Free)
1. Go to: https://dash.cloudflare.com → Analytics & Logs → Web Analytics
2. Click "Add a site"
3. Add sites:
   - `pyro1121.com` (main site)
   - `docs.pyro1121.com` (documentation)
4. Copy the tracking snippet and add to:
   - `site/frontend/index.html`
   - `docs-site/docusaurus.config.js` (use scripts injection)

### 2. Turnstile (Free CAPTCHA) ✅ IMPLEMENTED
Turnstile is now integrated into the login flow at `/dashboard`.

**Backend Configuration:**
- Secret Key is stored in wrangler secrets: `TURNSTILE_SECRET_KEY`

**Frontend Configuration:**
- Site Key is in `site/src/pages/DashboardPage.tsx`
- Update the `TURNSTILE_SITE_KEY` constant with your actual site key from the dashboard

**To get your Site Key:**
1. Go to: https://dash.cloudflare.com → Turnstile
2. Click your widget (e.g., `omg-auth`)
3. Copy the "Site Key" (public, safe to expose)
4. Update in `DashboardPage.tsx`:
   ```typescript
   const TURNSTILE_SITE_KEY = 'YOUR_SITE_KEY_HERE';
   ```

**How it works:**
- User enters email on login page
- Turnstile widget verifies user is human (invisible/managed challenge)
- Token is sent with send-code request
- Backend verifies token with Cloudflare before sending OTP email
- Blocks bots and credential stuffing attacks

### 3. Security Features
1. **WAF Rules**: Security → WAF → Custom rules
   - Block requests with SQL injection patterns
   - Rate limit by IP

2. **Bot Fight Mode**: Security → Bots → Bot Fight Mode → Enable

3. **Security Level**: Security → Settings → Security Level → High

### 4. Performance Features
1. **Speed Brain**: Speed → Optimization → Speed Brain → Enable
2. **Early Hints**: Speed → Optimization → Early Hints → Enable
3. **Auto Minify**: Speed → Optimization → Enable for JS/CSS/HTML
4. **Brotli**: Speed → Optimization → Brotli → Enable

### 5. Page Rules (Optional)
Create page rules for caching:
- `*pyro1121.com/api/*` - Cache Level: Bypass
- `*pyro1121.com/static/*` - Cache Level: Cache Everything, Edge TTL: 1 month

## Wrangler Configuration Files

### Main API Worker (`site/workers/wrangler.toml`)
- D1: DB (licensing), ANALYTICS_DB
- KV: CACHE, SESSIONS, FLAGS
- R2: ASSETS
- AI: AI binding
- Rate limiters: ADMIN, AUTH, API

### Releases Worker (`workers/releases/wrangler.toml`)
- R2: BUCKET (releases)
- D1: ANALYTICS_DB

### Docs Pages (`docs-site/wrangler.toml`)
- Bindings configured via dashboard for Pages Functions

## Secrets Required

Set via `wrangler secret put <NAME>`:
- `STRIPE_SECRET_KEY` - Stripe API key
- `STRIPE_WEBHOOK_SECRET` - Stripe webhook signing secret
- `JWT_SECRET` - JWT signing key
- `ADMIN_USER_ID` - Admin user identifier
- `RESEND_API_KEY` - Email service API key
- `META_API_KEY` - Meta/Analytics API key
- `TURNSTILE_SITE_KEY` - Cloudflare Turnstile site key
- `TURNSTILE_SECRET_KEY` - Cloudflare Turnstile secret key

## Free Tier Limits

| Resource | Free Limit |
|----------|------------|
| Workers | 100,000 requests/day |
| D1 | 5M rows read, 100K writes/day |
| KV | 100,000 reads, 1,000 writes/day |
| R2 | 10GB storage, 10M Class A, 1M Class B ops/month |
| Pages | Unlimited sites, 500 builds/month |
| Web Analytics | Unlimited |
| Turnstile | Unlimited |
