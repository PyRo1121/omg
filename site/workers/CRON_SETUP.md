# Setting Up Cron Triggers for Analytics Cleanup

The analytics system includes an automated cleanup task that removes raw events older than 7 days (keeping only the aggregates). This needs to run daily.

## Option 1: Cloudflare Dashboard (Recommended)

1. Go to [Cloudflare Dashboard](https://dash.cloudflare.com/)
2. Select your account → Workers & Pages
3. Click on `omg-saas` worker
4. Go to "Triggers" tab
5. Scroll to "Cron Triggers"
6. Click "Add Cron Trigger"
7. Enter cron expression: `0 2 * * *` (daily at 2 AM UTC)
8. Click "Save"

## Option 2: Manual Cleanup (If Cron Not Set)

Run this command manually as needed:

```bash
cd /home/pyro1121/Documents/code/filemanager/omg/site/workers

bunx wrangler d1 execute omg-licensing --remote --command="
  DELETE FROM docs_analytics_events WHERE created_at < datetime('now', '-7 days');
  DELETE FROM docs_analytics_sessions WHERE first_seen_at < datetime('now', '-30 days');
"
```

## Verification

After setting up the cron trigger, you can verify it's working by checking the worker logs:

```bash
bunx wrangler tail --format=pretty
```

On the next scheduled run (2 AM UTC), you should see:
```
Running scheduled cleanup tasks
Successfully cleaned up N old docs analytics events
```

## Cron Schedule Explanation

`0 2 * * *` means:
- Minute: 0 (at the top of the hour)
- Hour: 2 (2 AM)
- Day of month: * (every day)
- Month: * (every month)
- Day of week: * (every day of the week)

Result: Runs daily at 2:00 AM UTC

## Why This Matters

Without cleanup:
- Raw events table grows indefinitely (~50MB/day)
- Database queries slow down
- Storage costs increase

With cleanup:
- Only 7 days of raw events kept
- Aggregates kept forever (much smaller)
- Fast queries, low storage costs
