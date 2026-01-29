# 🚨 TELEMETRY BUG - IMMEDIATE FIX GUIDE

**Time Required:** < 1 hour  
**Goal:** Get telemetry dashboard working for users

---

## Step 1: Add Debugging (5 mins)

Edit `src/routes/api/telemetry/dashboard.ts`:

### A. Debug Environment Binding (Lines 4-21)

Replace the `getEnv()` function with this version:

```typescript
function getEnv(event: APIEvent): CloudflareEnv {
  const env = (event.nativeEvent as any).context?.cloudflare?.env;
  
  if (!env) {
    console.error('[TELEMETRY DEBUG] No cloudflare env found');
    console.error('[TELEMETRY DEBUG] Event keys:', Object.keys(event));
    console.error('[TELEMETRY DEBUG] NativeEvent context:', (event.nativeEvent as any).context);
    throw new Error("Cloudflare environment not available");
  }
  
  console.log('[TELEMETRY DEBUG] ✓ Environment found');
  console.log('[TELEMETRY DEBUG] Environment keys:', Object.keys(env));
  console.log('[TELEMETRY DEBUG] DB binding exists:', !!env.DB);
  console.log('[TELEMETRY DEBUG] DB binding type:', typeof env.DB);

  return {
    DB: env.DB,
    BETTER_AUTH_KV: env.BETTER_AUTH_KV,
    BETTER_AUTH_SECRET: env.BETTER_AUTH_SECRET,
    BETTER_AUTH_URL: env.BETTER_AUTH_URL,
    GITHUB_CLIENT_ID: env.GITHUB_CLIENT_ID,
    GITHUB_CLIENT_SECRET: env.GITHUB_CLIENT_SECRET,
    GOOGLE_CLIENT_ID: env.GOOGLE_CLIENT_ID,
    GOOGLE_CLIENT_SECRET: env.GOOGLE_CLIENT_SECRET,
  };
}
```

### B. Debug Database Query (After Line 48)

Add this block right after the `linkedLicense` query:

```typescript
console.log('[TELEMETRY DEBUG] Query result:', linkedLicense);
console.log('[TELEMETRY DEBUG] Query result type:', typeof linkedLicense);
console.log('[TELEMETRY DEBUG] Query result keys:', linkedLicense ? Object.keys(linkedLicense) : 'null');

// Check if table exists
try {
  const tableCheck = await env.DB.prepare(
    `SELECT name FROM sqlite_master WHERE type='table' AND name='user_license'`
  ).first();
  console.log('[TELEMETRY DEBUG] Table check result:', tableCheck);
  
  if (!tableCheck) {
    console.error('[TELEMETRY DEBUG] ❌ user_license table does NOT exist!');
  } else {
    console.log('[TELEMETRY DEBUG] ✓ user_license table exists');
    
    // List all rows
    const allRows = await env.DB.prepare(`SELECT * FROM user_license`).all();
    console.log('[TELEMETRY DEBUG] All user_license rows:', allRows.results);
    console.log('[TELEMETRY DEBUG] Row count:', allRows.results.length);
  }
} catch (dbError) {
  console.error('[TELEMETRY DEBUG] ❌ Database check error:', dbError);
}
```

---

## Step 2: Deploy and Test (10 mins)

```bash
cd /home/pyro1121/Documents/omg/site

# Deploy to production
npm run build
npm run deploy

# OR if using wrangler directly:
npx wrangler pages deploy dist
```

---

## Step 3: Check Logs (5 mins)

### Option A: Real-time Tail

```bash
npx wrangler pages deployment tail
```

### Option B: Cloudflare Dashboard

1. Go to https://dash.cloudflare.com
2. Select "Pages" → "omg-site"
3. Click "Functions" tab
4. Open "Logs" section
5. Filter for "TELEMETRY DEBUG"

---

## Step 4: Reproduce Bug (2 mins)

1. Open browser: https://pyro1121.com
2. Log in with your account
3. Navigate to `/telemetry`
4. Watch console logs in terminal

---

## Step 5: Analyze Logs

### ✅ Expected (Good) Logs:

```
[TELEMETRY DEBUG] ✓ Environment found
[TELEMETRY DEBUG] DB binding exists: true
[TELEMETRY DEBUG] ✓ user_license table exists
[TELEMETRY DEBUG] All user_license rows: [{ user_id: "jnU4Lg...", license_key: "922f..." }]
[TELEMETRY DEBUG] Query result: { license_key: "922f7fcd-fcd1-42e6-91ae-c635294a2dd2" }
```

**If you see this:** The bug is NOT in this endpoint. Check frontend or backend API.

---

### ❌ Scenario 1: Table Missing

```
[TELEMETRY DEBUG] ✓ Environment found
[TELEMETRY DEBUG] DB binding exists: true
[TELEMETRY DEBUG] ❌ user_license table does NOT exist!
```

**Solution:** Run database migration:

```bash
cd /home/pyro1121/Documents/omg/site

# Create migration
npx wrangler d1 execute omg-auth-db --remote --command "
CREATE TABLE IF NOT EXISTS user_license (
  user_id TEXT PRIMARY KEY,
  license_key TEXT NOT NULL,
  linked_at INTEGER NOT NULL
);
"

# Verify
npx wrangler d1 execute omg-auth-db --remote --command "
SELECT name FROM sqlite_master WHERE type='table';
"
```

---

### ❌ Scenario 2: Wrong Database

```
[TELEMETRY DEBUG] ✓ Environment found
[TELEMETRY DEBUG] DB binding exists: true
[TELEMETRY DEBUG] ✓ user_license table exists
[TELEMETRY DEBUG] All user_license rows: []  ← EMPTY!
[TELEMETRY DEBUG] Query result: null
```

**Solution:** Check database binding in `wrangler.toml`:

```toml
[[d1_databases]]
binding = "DB"
database_name = "omg-auth-db"  ← Must match production DB
database_id = "871b70ca-79f7-4bb0-bfba-0f9f9aca4de9"  ← Verify this ID
```

Verify correct DB:

```bash
# List all D1 databases
npx wrangler d1 list

# Check data in production DB
npx wrangler d1 execute omg-auth-db --remote --command "SELECT * FROM user_license;"
```

---

### ❌ Scenario 3: Session User ID Mismatch

```
[TELEMETRY DEBUG] Query result: null
[TELEMETRY DEBUG] All user_license rows: [
  { user_id: "ABC123...", license_key: "..." }  ← Different user_id!
]
```

**Solution:** Add session debugging:

```typescript
console.log('[TELEMETRY DEBUG] Session user ID:', session.user.id);
console.log('[TELEMETRY DEBUG] Session user email:', session.user.email);

const linkedLicense = await env.DB.prepare(
  `SELECT * FROM user_license WHERE user_id = ?`
).bind(session.user.id).first();
```

If `session.user.id` doesn't match database `user_id`:
1. Check if Better Auth is creating consistent user IDs
2. Verify session token is valid
3. Check if user logged in with different provider

---

### ❌ Scenario 4: No Environment

```
[TELEMETRY DEBUG] ❌ No cloudflare env found
[TELEMETRY DEBUG] Event keys: ['request', 'nativeEvent', ...]
[TELEMETRY DEBUG] NativeEvent context: undefined
```

**Solution:** Fix route file structure:

Move `/api/telemetry/dashboard.ts` to `/api/telemetry-dashboard.ts` (no nested folder):

```bash
cd /home/pyro1121/Documents/omg/site/src/routes/api
mv telemetry/dashboard.ts telemetry-dashboard.ts
```

Update frontend call:

```typescript
// BEFORE:
const response = await fetch('/api/telemetry/dashboard');

// AFTER:
const response = await fetch('/api/telemetry-dashboard');
```

---

## Step 6: Apply Fix

Based on logs, apply the appropriate fix above.

---

## Step 7: Verify Fix

```bash
# Rebuild and deploy
npm run build
npm run deploy

# Test again
curl -H "Cookie: session=YOUR_SESSION_TOKEN" https://pyro1121.com/api/telemetry/dashboard

# Should return:
{
  "usage": { ... },
  "achievements": [ ... ],
  "daily_usage": [ ... ]
}
```

---

## Step 8: Clean Up Debug Logs

Once working, remove debug logs:

```typescript
// Remove all lines starting with:
console.log('[TELEMETRY DEBUG] ...');
console.error('[TELEMETRY DEBUG] ...');
```

Keep only essential logs:

```typescript
console.log('[Telemetry API] User ID:', session.user.id);
console.log('[Telemetry API] Found license:', linkedLicense?.license_key || 'null');
```

---

## Quick Reference: Useful Commands

```bash
# Check production database
npx wrangler d1 execute omg-auth-db --remote --command "SELECT * FROM user_license;"

# List all tables
npx wrangler d1 execute omg-auth-db --remote --command "SELECT name FROM sqlite_master WHERE type='table';"

# Check table schema
npx wrangler d1 execute omg-auth-db --remote --command "PRAGMA table_info(user_license);"

# View real-time logs
npx wrangler pages deployment tail

# List all D1 databases
npx wrangler d1 list

# Deploy
npm run deploy
```

---

## If Still Broken After All Fixes

**Nuclear Option:** Copy working endpoint logic:

```bash
# Copy the working provision-license logic
cp src/routes/api/provision-license.ts src/routes/api/telemetry/dashboard-backup.ts

# Edit dashboard.ts to use EXACT same patterns as provision-license.ts
```

**Compare side-by-side:**

```bash
diff src/routes/api/provision-license.ts src/routes/api/telemetry/dashboard.ts
```

Look for differences in:
- `getEnv()` function
- Session extraction
- Database query pattern
- Error handling

---

## Success Criteria

✅ `/api/telemetry/dashboard` returns data (not "No license linked")  
✅ Frontend shows telemetry metrics (commands, time saved, achievements)  
✅ No errors in console logs  
✅ Response time < 500ms  

**You should see:**
```json
{
  "usage": {
    "commands_run": 1247,
    "time_saved_ms": 9240000
  },
  "achievements": [
    { "id": "first_command", "unlocked": true }
  ]
}
```

---

**Questions?** Check the full analysis: `/home/pyro1121/Documents/omg/TELEMETRY_CODE_REVIEW.md`
