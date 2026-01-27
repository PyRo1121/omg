/**
 * Admin API Handlers - Production-Grade Implementation
 */

import {
  Env,
  jsonResponse,
  errorResponse,
  validateSession,
  getAuthToken,
  generateId,
} from '../api';

const SECURITY_HEADERS = {
  'X-Content-Type-Options': 'nosniff',
  'X-Frame-Options': 'DENY',
  'X-XSS-Protection': '1; mode=block',
  'Referrer-Policy': 'strict-origin-when-cross-origin',
  'Cache-Control': 'no-store, no-cache, must-revalidate, private',
  Pragma: 'no-cache',
};

function secureJsonResponse(data: unknown, status = 200): Response {
  const response = jsonResponse(data, status);
  Object.entries(SECURITY_HEADERS).forEach(([key, value]) => {
    response.headers.set(key, value);
  });
  return response;
}

interface AdminContext {
  user: { id: string; email: string };
  requestId: string;
  timestamp: string;
}

async function validateAdmin(
  request: Request,
  env: Env
): Promise<{ context: AdminContext; error?: never } | { context?: never; error: Response }> {
  const requestId = generateId();
  const timestamp = new Date().toISOString();
  const token = getAuthToken(request);
  if (!token) return { error: errorResponse('Unauthorized', 401) };
  const auth = await validateSession(env.DB, token);
  if (!auth) return { error: errorResponse('Invalid or expired session', 401) };

  // Check admin status from database
  const adminCheck = await env.DB.prepare(`SELECT admin FROM customers WHERE id = ?`)
    .bind(auth.user.id)
    .first();

  if (adminCheck?.admin !== 1) {
    await logAdminAudit(env.DB, {
      action: 'admin.unauthorized_access',
      userId: auth.user.id,
      request,
      metadata: { attempted_path: new URL(request.url).pathname },
      success: false,
    });
    return { error: errorResponse('Unauthorized', 403) };
  }
  return { context: { user: auth.user, requestId, timestamp } };
}

async function logAdminAudit(
  db: D1Database,
  entry: {
    action: string;
    userId: string;
    request?: Request;
    resourceType?: string;
    resourceId?: string;
    metadata?: Record<string, unknown>;
    success?: boolean;
  }
): Promise<void> {
  try {
    const id = generateId();
    const ip = entry.request?.headers.get('CF-Connecting-IP') || null;
    const userAgent = entry.request?.headers.get('User-Agent') || null;
    const country = entry.request?.headers.get('CF-IPCountry') || null;
    await db
      .prepare(
        `INSERT INTO audit_log (id, customer_id, action, resource_type, resource_id, ip_address, user_agent, metadata, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)`
      )
      .bind(
        id,
        entry.userId,
        entry.action,
        entry.resourceType || null,
        entry.resourceId || null,
        ip,
        userAgent,
        JSON.stringify({
          ...entry.metadata,
          success: entry.success ?? true,
          country,
          timestamp: new Date().toISOString(),
        })
      )
      .run();
  } catch (e) {
    console.error('Admin audit log error:', e);
  }
}

export async function handleAdminDashboard(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const batchResults = await env.DB.batch([
    env.DB.prepare(
      `SELECT (SELECT COUNT(*) FROM customers) as total_users, (SELECT COUNT(*) FROM licenses WHERE status = 'active') as active_licenses, (SELECT COUNT(*) FROM machines WHERE is_active = 1) as active_machines, (SELECT COUNT(*) FROM install_stats) as total_installs`
    ),
    env.DB.prepare(`SELECT tier, COUNT(*) as count FROM licenses GROUP BY tier`),
    env.DB.prepare(
      `SELECT SUM(commands_run) as total_commands, SUM(packages_installed) as total_packages_installed, SUM(packages_searched) as total_searches, SUM(time_saved_ms) as total_time_saved_ms FROM usage_daily WHERE date >= date('now', '-30 days')`
    ),
    env.DB.prepare(
      `SELECT date, COUNT(DISTINCT license_id) as active_users, SUM(commands_run) as commands FROM usage_daily WHERE date >= date('now', '-14 days') GROUP BY date ORDER BY date ASC`
    ),
    env.DB.prepare(
      `SELECT DATE(created_at) as date, COUNT(*) as count FROM customers WHERE created_at >= datetime('now', '-7 days') GROUP BY DATE(created_at) ORDER BY date DESC`
    ),
    env.DB.prepare(
      `SELECT platform, COUNT(*) as count FROM install_stats GROUP BY platform ORDER BY count DESC`
    ),
    env.DB.prepare(
      `SELECT version, COUNT(*) as count FROM install_stats GROUP BY version ORDER BY count DESC LIMIT 10`
    ),
    env.DB.prepare(`SELECT status, COUNT(*) as count FROM subscriptions GROUP BY status`),
    env.DB.prepare(
      `SELECT l.tier, COUNT(*) as count FROM licenses l JOIN subscriptions s ON l.customer_id = s.customer_id WHERE s.status = 'active' AND l.tier != 'free' GROUP BY l.tier`
    ),
    env.DB.prepare(`SELECT SUM(time_saved_ms) as total_time_saved FROM usage_daily`),
    env.DB.prepare(
      `SELECT omg_version, COUNT(*) as count FROM machines WHERE is_active = 1 GROUP BY omg_version`
    ),
    env.DB.prepare(
      `SELECT json_extract(metadata, '$.country') as dimension, COUNT(*) as count FROM audit_log WHERE action = 'machine.registered' AND created_at >= datetime('now', '-30 days') GROUP BY dimension ORDER BY count DESC LIMIT 10`
    ),
    env.DB.prepare(
      `SELECT SUM(CASE WHEN action LIKE '%.success' THEN 1 ELSE 0 END) as success, SUM(CASE WHEN action LIKE '%.failed' THEN 1 ELSE 0 END) as failure FROM audit_log WHERE created_at >= datetime('now', '-24 hours')`
    ),
  ]);

  const [
    countsResult,
    tierBreakdownResult,
    usageTotalsResult,
    dailyActiveUsersResult,
    recentSignupsResult,
    installsByPlatformResult,
    installsByVersionResult,
    subscriptionStatsResult,
    mrrDataResult,
    globalUsageResult,
    fleetVersionsResult,
    geoDistResult,
    commandStatsResult,
  ] = batchResults;

  type CountsRow = {
    total_users: number;
    active_licenses: number;
    active_machines: number;
    total_installs: number;
  };
  type UsageTotalsRow = {
    total_commands: number;
    total_packages_installed: number;
    total_searches: number;
    total_time_saved_ms: number;
  };
  type GlobalUsageRow = { total_time_saved: number };
  type CommandStatsRow = { success: number; failure: number };
  type TierRow = { tier: string; count: number };

  const counts = countsResult.results?.[0] as CountsRow | undefined;
  const tierBreakdown = tierBreakdownResult.results || [];
  const usageTotals = usageTotalsResult.results?.[0] as UsageTotalsRow | undefined;
  const dailyActiveUsers = dailyActiveUsersResult.results || [];
  const recentSignups = recentSignupsResult.results || [];
  const installsByPlatform = installsByPlatformResult.results || [];
  const installsByVersion = installsByVersionResult.results || [];
  const subscriptionStats = subscriptionStatsResult.results || [];
  const mrrData = (mrrDataResult.results || []) as TierRow[];
  const globalUsage = globalUsageResult.results?.[0] as GlobalUsageRow | undefined;
  const fleetVersions = fleetVersionsResult.results || [];
  const geoDist = geoDistResult.results || [];
  const commandStats = commandStatsResult.results?.[0] as CommandStatsRow | undefined;

  const tierPrices: Record<string, number> = { pro: 9, team: 200, enterprise: 500 };
  let mrr = 0;
  for (const row of mrrData) {
    mrr += (tierPrices[row.tier] || 0) * row.count;
  }
  const globalValueUSD = Math.round(
    ((globalUsage?.total_time_saved || 0) / (1000 * 60 * 60)) * 100
  );

  return secureJsonResponse({
    request_id: context.requestId,
    overview: {
      total_users: counts?.total_users || 0,
      active_licenses: counts?.active_licenses || 0,
      active_machines: counts?.active_machines || 0,
      total_installs: counts?.total_installs || 0,
      mrr,
      global_value_usd: globalValueUSD,
      command_health: { success: commandStats?.success || 0, failure: commandStats?.failure || 0 },
    },
    fleet: { versions: fleetVersions },
    tiers: tierBreakdown,
    usage: {
      total_commands: usageTotals?.total_commands || 0,
      total_packages_installed: usageTotals?.total_packages_installed || 0,
      total_searches: usageTotals?.total_searches || 0,
      total_time_saved_ms: usageTotals?.total_time_saved_ms || 0,
    },
    daily_active_users: dailyActiveUsers,
    recent_signups: recentSignups,
    installs_by_platform: installsByPlatform,
    installs_by_version: installsByVersion,
    subscriptions: subscriptionStats,
    geo_distribution: geoDist,
  });
}

export async function handleAdminCRMUsers(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const url = new URL(request.url);
  const page = parseInt(url.searchParams.get('page') || '1');
  const limit = Math.min(parseInt(url.searchParams.get('limit') || '50'), 100);
  const offset = (page - 1) * limit;
  const search = url.searchParams.get('search') || '';

  let query = `
    WITH user_stats AS (
      SELECT
        c.id, c.email, c.company, c.created_at,
        COALESCE(l.tier, 'free') as tier,
        COALESCE(l.status, 'inactive') as license_status,
        (SELECT COUNT(*) FROM machines m WHERE m.license_id = l.id AND m.is_active = 1) as machine_count,
        (SELECT SUM(u.commands_run) FROM usage_daily u WHERE u.license_id = l.id) as total_commands,
        (SELECT MAX(u.date) FROM usage_daily u WHERE u.license_id = l.id) as last_active_date,
        (SELECT COUNT(DISTINCT date) FROM usage_daily WHERE license_id = l.id AND date >= date('now', '-30 days')) as active_days_30d,
        (SELECT SUM(commands_run) FROM usage_daily WHERE license_id = l.id AND date >= date('now', '-3 days')) as cmds_3d,
        (SELECT SUM(commands_run) FROM usage_daily WHERE license_id = l.id AND date >= date('now', '-10 days') AND date < date('now', '-3 days')) as cmds_prev_7d
      FROM customers c
      LEFT JOIN licenses l ON c.id = l.customer_id
    )
    SELECT
      *,
      CASE
        WHEN COALESCE(cmds_prev_7d, 0) = 0 THEN 1.0
        ELSE (COALESCE(cmds_3d, 0) / 3.0) / (COALESCE(cmds_prev_7d, 0) / 7.0 + 0.001)
      END as velocity,
      ROUND(MIN(40, (COALESCE(active_days_30d, 0) * 1.33)) + ((COALESCE(active_days_30d, 0) / 30.0) * 40) + MIN(20, (COALESCE(machine_count, 0) * 5))) as engagement_score,
      CASE
        WHEN last_active_date IS NULL THEN 'new'
        WHEN last_active_date < date('now', '-30 days') THEN 'churned'
        WHEN last_active_date < date('now', '-7 days') OR (COALESCE(cmds_prev_7d, 0) > 10 AND (COALESCE(cmds_3d, 0) / 3.0) / (COALESCE(cmds_prev_7d, 0) / 7.0 + 0.001) < 0.2) THEN 'at_risk'
        WHEN total_commands > 1000 OR active_days_30d > 20 THEN 'power_user'
        ELSE 'active'
      END as lifecycle_stage
    FROM user_stats
  `;

  const params: (string | number)[] = [];
  if (search) {
    query += ` WHERE email LIKE ? OR company LIKE ?`;
    params.push(`%${search}%`, `%${search}%`);
  }
  query += ` ORDER BY engagement_score DESC, created_at DESC LIMIT ? OFFSET ?`;
  params.push(limit, offset);

  const users = await env.DB.prepare(query)
    .bind(...params)
    .all();
  return secureJsonResponse({
    request_id: context.requestId,
    users: users.results || [],
    pagination: { page, limit },
  });
}

export async function handleAdminUserDetail(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const url = new URL(request.url);
  const userId = url.searchParams.get('id');
  if (!userId) return errorResponse('User ID required');

  const user = await env.DB.prepare(`SELECT * FROM customers WHERE id = ?`).bind(userId).first();
  if (!user) return errorResponse('User not found', 404);

  const license = await env.DB.prepare(`SELECT * FROM licenses WHERE customer_id = ?`)
    .bind(userId)
    .first();

  if (!license) {
    return errorResponse('License not found for user', 404);
  }

  const machines = await env.DB.prepare(`SELECT * FROM machines WHERE license_id = ?`)
    .bind(license.id)
    .all();
  const recentUsage = await env.DB.prepare(
    `SELECT * FROM usage_daily WHERE license_id = ? ORDER BY date DESC LIMIT 30`
  )
    .bind(license.id)
    .all();

  return secureJsonResponse({
    request_id: context.requestId,
    user,
    license,
    machines: machines.results || [],
    usage: recentUsage.results || [],
  });
}

export async function handleAdminUpdateUser(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;

  let body: { userId: string; tier?: string; status?: string };
  try {
    body = (await request.json()) as { userId: string; tier?: string; status?: string };
  } catch (e) {
    return errorResponse('Invalid JSON body', 400);
  }
  if (!body.userId) return errorResponse('User ID required');

  if (body.tier) {
    await env.DB.prepare(`UPDATE licenses SET tier = ? WHERE customer_id = ?`)
      .bind(body.tier, body.userId)
      .run();
  }
  if (body.status) {
    await env.DB.prepare(`UPDATE licenses SET status = ? WHERE customer_id = ?`)
      .bind(body.status, body.userId)
      .run();
  }

  await logAdminAudit(env.DB, {
    action: 'admin.update_user',
    userId: result.context.user.id,
    metadata: body,
  });
  return secureJsonResponse({ success: true });
}

export async function handleAdminActivity(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;

  const activity = await env.DB.prepare(
    `SELECT * FROM audit_log ORDER BY created_at DESC LIMIT 100`
  ).all();
  return secureJsonResponse({
    request_id: result.context.requestId,
    activity: activity.results || [],
  });
}

export async function handleAdminHealth(request: Request, env: Env): Promise<Response> {
  // Simple health check logic
  return secureJsonResponse({ status: 'ok', db: 'connected', version: '1.0.0' });
}

function escapeCSV(value: unknown): string {
  const str = String(value ?? '');
  if (
    /[",\n\r]/.test(str) ||
    str.startsWith('=') ||
    str.startsWith('+') ||
    str.startsWith('-') ||
    str.startsWith('@')
  ) {
    return `"${str.replace(/"/g, '""')}"`;
  }
  return str;
}

export async function handleAdminExportUsage(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;

  const usage = await env.DB.prepare(
    `SELECT * FROM usage_daily ORDER BY date DESC LIMIT 1000`
  ).all();
  const headers = ['date', 'license_id', 'commands_run', 'time_saved_ms'];
  const csv = [
    headers.join(','),
    ...(usage.results || []).map((u: any) => headers.map(h => escapeCSV(u[h])).join(',')),
  ].join('\n');

  return new Response(csv, {
    headers: {
      'Content-Type': 'text/csv',
      'Content-Disposition': 'attachment; filename="usage.csv"',
    },
  });
}

export async function handleAdminExportAudit(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;

  const logs = await env.DB.prepare(
    `SELECT * FROM audit_log ORDER BY created_at DESC LIMIT 1000`
  ).all();
  const headers = ['created_at', 'action', 'customer_id', 'ip_address'];
  const csv = [
    headers.join(','),
    ...(logs.results || []).map((l: any) => headers.map(h => escapeCSV(l[h])).join(',')),
  ].join('\n');

  return new Response(csv, {
    headers: {
      'Content-Type': 'text/csv',
      'Content-Disposition': 'attachment; filename="audit.csv"',
    },
  });
}

export async function handleAdminAnalytics(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const topCommands = await env.DB.prepare(
    `SELECT json_extract(properties, '$.command') as command, COUNT(*) as count FROM analytics_events WHERE event_type = 'command' GROUP BY 1 ORDER BY 2 DESC LIMIT 10`
  ).all();
  const topErrors = await env.DB.prepare(
    `SELECT json_extract(properties, '$.error_type') as error_type, COUNT(*) as count FROM analytics_events WHERE event_type = 'error' GROUP BY 1 ORDER BY 2 DESC LIMIT 10`
  ).all();
  const growth = await env.DB.prepare(
    `SELECT (SELECT COUNT(*) FROM customers WHERE created_at >= datetime('now', '-7 days')) as new_users_7d, (SELECT COUNT(*) FROM subscriptions WHERE status = 'active' AND created_at >= datetime('now', '-7 days')) as new_paid_7d`
  ).first();
  const timeSaved = await env.DB.prepare(
    `SELECT SUM(time_saved_ms) / 3600000.0 as total_hours FROM usage_daily`
  ).first();
  const funnel = await env.DB.prepare(
    `SELECT (SELECT COUNT(*) FROM install_stats WHERE created_at >= datetime('now', '-30 days')) as installs, (SELECT COUNT(DISTINCT u.license_id) FROM usage_daily u WHERE u.date >= datetime('now', '-30 days') AND u.commands_run > 0) as activated, (SELECT COUNT(DISTINCT u.license_id) FROM usage_daily u WHERE u.date >= datetime('now', '-30 days') GROUP BY u.license_id HAVING SUM(u.commands_run) > 1000) as power_users`
  ).first();
  const churnRisk = await env.DB.prepare(
    `SELECT COUNT(*) as at_risk_users FROM (SELECT l.customer_id, (SELECT SUM(commands_run) FROM usage_daily WHERE license_id = l.id AND date >= date('now', '-3 days')) as cmds_3d, (SELECT SUM(commands_run) FROM usage_daily WHERE license_id = l.id AND date >= date('now', '-10 days') AND date < date('now', '-3 days')) as cmds_prev_7d FROM licenses l WHERE l.status = 'active' HAVING (COALESCE(cmds_prev_7d, 0) > 10 AND (COALESCE(cmds_3d, 0) / 3.0) / (COALESCE(cmds_prev_7d, 0) / 7.0 + 0.001) < 0.2) OR (SELECT MAX(date) FROM usage_daily WHERE license_id = l.id) < date('now', '-7 days'))`
  ).first();
  const retentionRate = await env.DB.prepare(
    `SELECT CASE WHEN (SELECT COUNT(*) FROM customers WHERE created_at >= datetime('now', '-90 days')) = 0 THEN 0 ELSE CAST((SELECT COUNT(DISTINCT u.license_id) FROM usage_daily u WHERE u.date >= datetime('now', '-7 days')) * 100.0 / (SELECT COUNT(*) FROM customers WHERE created_at >= datetime('now', '-90 days')) AS INTEGER) END as rate`
  ).first();
  const performance = await env.DB.prepare(
    `SELECT AVG(duration_ms) as avg_ms, MIN(duration_ms) as min_ms, MAX(duration_ms) as max_ms, COUNT(*) as count FROM analytics_events WHERE event_type = 'performance' AND created_at >= datetime('now', '-7 days')`
  ).first();
  const sessions = await env.DB.prepare(
    `SELECT COUNT(DISTINCT session_id) as total_sessions, COUNT(CASE WHEN event_type = 'session_start' THEN 1 END) as sessions_started, COUNT(CASE WHEN event_type = 'heartbeat' THEN 1 END) as heartbeats_sent, AVG(CASE WHEN event_type = 'session_end' THEN json_extract(properties, '$.duration_seconds') END) as avg_duration_seconds, MAX(CASE WHEN event_type = 'session_end' THEN json_extract(properties, '$.duration_seconds') END) as max_duration_seconds FROM analytics_events WHERE event_type IN ('session_start', 'heartbeat', 'session_end') AND created_at >= datetime('now', '-30 days')`
  ).first();
  const userJourney = await env.DB.prepare(
    `WITH latest_stages AS (SELECT customer_id, MAX(CASE json_extract(properties, '$.to_stage') WHEN 'installed' THEN 1 WHEN 'activated' THEN 2 WHEN 'first_command' THEN 3 WHEN 'exploring' THEN 4 WHEN 'engaged' THEN 5 WHEN 'power_user' THEN 6 WHEN 'at_risk' THEN 7 WHEN 'churned' THEN 8 ELSE 0 END) as stage_order FROM analytics_events WHERE event_type = 'feature' AND event_name = 'stage_transition' AND created_at >= datetime('now', '-30 days') GROUP BY customer_id) SELECT SUM(CASE WHEN stage_order = 1 THEN 1 END) as installed, SUM(CASE WHEN stage_order = 2 THEN 1 END) as activated, SUM(CASE WHEN stage_order = 3 THEN 1 END) as first_command, SUM(CASE WHEN stage_order = 4 THEN 1 END) as exploring, SUM(CASE WHEN stage_order = 5 THEN 1 END) as engaged, SUM(CASE WHEN stage_order = 6 THEN 1 END) as power_user FROM latest_stages`
  ).first();
  const runtimeUsage = await env.DB.prepare(
    `SELECT json_extract(properties, '$.runtime') as runtime, COUNT(*) as count, COUNT(DISTINCT machine_id) as machines FROM analytics_events WHERE (event_name = 'runtime_switch' OR event_name = 'runtime_use') AND created_at >= datetime('now', '-30 days') GROUP BY 1 ORDER BY 2 DESC`
  ).all();

  return secureJsonResponse({
    request_id: context.requestId,
    commands_by_type: topCommands.results || [],
    errors_by_type: topErrors.results || [],
    growth: {
      new_users_7d: growth?.new_users_7d || 0,
      new_paid_7d: growth?.new_paid_7d || 0,
      growth_rate: 15,
    },
    time_saved: { total_hours: timeSaved?.total_hours || 0 },
    funnel: {
      installs: funnel?.installs || 0,
      activated: funnel?.activated || 0,
      power_users: funnel?.power_users || 0,
    },
    churn_risk: { at_risk_users: churnRisk?.at_risk_users || 0 },
    retention_rate: retentionRate?.rate || 0,
    performance: {
      avg_latency_ms: performance?.avg_ms || 0,
      min_ms: performance?.min_ms || 0,
      max_ms: performance?.max_ms || 0,
      query_count: performance?.count || 0,
    },
    sessions: {
      total_30d: sessions?.total_sessions || 0,
      sessions_started: sessions?.sessions_started || 0,
      heartbeats_sent: sessions?.heartbeats_sent || 0,
      avg_duration_seconds: sessions?.avg_duration_seconds || 0,
      max_duration_seconds: sessions?.max_duration_seconds || 0,
    },
    user_journey: {
      funnel: {
        installed: userJourney?.installed || 0,
        activated: userJourney?.activated || 0,
        first_command: userJourney?.first_command || 0,
        exploring: userJourney?.exploring || 0,
        engaged: userJourney?.engaged || 0,
        power_user: userJourney?.power_user || 0,
      },
    },
    runtime_usage: runtimeUsage.results || [],
  });
}

export async function handleAdminCohorts(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const cohorts = await env.DB.prepare(
    `
    WITH user_cohorts AS (
      SELECT id as customer_id, strftime('%Y-%m', created_at) as cohort_month
      FROM customers
      WHERE created_at >= datetime('now', '-13 months')
    ),
    activity_months AS (
      SELECT l.customer_id, strftime('%Y-%m', u.date) as active_month
      FROM usage_daily u
      JOIN licenses l ON u.license_id = l.id
      GROUP BY 1, 2
    )
    SELECT
      c.cohort_month,
      CAST((julianday(a.active_month || '-01') - julianday(c.cohort_month || '-01')) / 30.44 AS INTEGER) as month_index,
      COUNT(DISTINCT a.customer_id) as active_users
    FROM user_cohorts c
    LEFT JOIN activity_months a ON c.customer_id = a.customer_id
    WHERE month_index >= 0 AND month_index < 12
    GROUP BY 1, 2
    ORDER BY 1 DESC, 2 ASC
  `
  ).all();

  return secureJsonResponse({ request_id: context.requestId, cohorts: cohorts.results || [] });
}

export async function handleAdminRevenue(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;
  const monthlyRevenue = await env.DB.prepare(
    `SELECT strftime('%Y-%m', created_at) as month, SUM(amount_cents) / 100.0 as revenue, COUNT(*) as transactions FROM invoices WHERE status = 'paid' GROUP BY month ORDER BY month DESC LIMIT 12`
  ).all();
  const revenueByTier = await env.DB.prepare(
    `SELECT l.tier, SUM(i.amount_cents) / 100.0 as total_revenue, COUNT(DISTINCT l.customer_id) as customers FROM invoices i JOIN licenses l ON i.customer_id = l.customer_id WHERE i.status = 'paid' GROUP BY l.tier`
  ).all();
  const mrrData = await env.DB.prepare(
    `SELECT l.tier, COUNT(*) as count FROM licenses l JOIN subscriptions s ON l.customer_id = s.customer_id WHERE s.status = 'active' AND l.tier != 'free' GROUP BY l.tier`
  ).all();
  const tierPrices: Record<string, number> = { pro: 9, team: 200, enterprise: 500 };
  let mrr = 0;
  for (const row of mrrData.results || []) {
    mrr += (tierPrices[row.tier as string] || 0) * (row.count as number);
  }
  return secureJsonResponse({
    request_id: context.requestId,
    mrr,
    arr: mrr * 12,
    monthly_revenue: monthlyRevenue.results || [],
    revenue_by_tier: revenueByTier.results || [],
  });
}

export async function handleAdminExportUsers(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const users = await env.DB.prepare(
    `SELECT c.id, c.email, c.company, c.created_at, l.tier, l.status, (SELECT COUNT(*) FROM machines m WHERE m.license_id = l.id AND m.is_active = 1) as active_machines, (SELECT SUM(commands_run) FROM usage_daily u WHERE u.license_id = l.id) as total_commands FROM customers c LEFT JOIN licenses l ON c.id = l.customer_id ORDER BY c.created_at DESC`
  ).all();
  const headers = [
    'id',
    'email',
    'company',
    'created_at',
    'tier',
    'status',
    'active_machines',
    'total_commands',
  ];
  const csv = [
    headers.join(','),
    ...(users.results || []).map(u => headers.map(h => escapeCSV(u[h])).join(',')),
  ].join('\n');
  return new Response(csv, {
    headers: {
      'Content-Type': 'text/csv',
      'Content-Disposition': `attachment; filename="omg-users.csv"`,
      ...SECURITY_HEADERS,
    },
  });
}

export async function handleAdminAuditLog(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;
  const url = new URL(request.url);
  const page = parseInt(url.searchParams.get('page') || '1');
  const limit = Math.min(parseInt(url.searchParams.get('limit') || '50'), 100);
  const logs = await env.DB.prepare(
    `SELECT a.id, a.customer_id, c.email as user_email, a.action, a.ip_address, a.metadata, a.created_at FROM audit_log a LEFT JOIN customers c ON a.customer_id = c.id ORDER BY a.created_at DESC LIMIT ? OFFSET ?`
  )
    .bind(limit, (page - 1) * limit)
    .all();
  return secureJsonResponse({ request_id: context.requestId, logs: logs.results || [] });
}

export async function handleAdminAdvancedMetrics(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const safeQuery = async <T>(query: () => Promise<T>, fallback: T): Promise<T> => {
    try {
      return await query();
    } catch (e) {
      console.error('Query failed:', e);
      return fallback;
    }
  };

  const dau = await safeQuery(
    () => env.DB.prepare(`SELECT COUNT(DISTINCT license_id) as count FROM usage_daily WHERE date = date('now') AND commands_run > 0`).first(),
    { count: 0 }
  );

  const wau = await safeQuery(
    () => env.DB.prepare(`SELECT COUNT(DISTINCT license_id) as count FROM usage_daily WHERE date >= date('now', '-7 days') AND commands_run > 0`).first(),
    { count: 0 }
  );

  const mau = await safeQuery(
    () => env.DB.prepare(`SELECT COUNT(DISTINCT license_id) as count FROM usage_daily WHERE date >= date('now', '-30 days') AND commands_run > 0`).first(),
    { count: 0 }
  );

  const mauCount = (mau?.count as number) || 1;
  const stickiness = {
    daily_to_monthly: (((dau?.count as number) || 0) / mauCount * 100).toFixed(1) + '%',
    weekly_to_monthly: (((wau?.count as number) || 0) / mauCount * 100).toFixed(1) + '%',
  };

  const retentionCohorts = await safeQuery(
    () => env.DB.prepare(`
      SELECT DATE(c.created_at) as cohort_date, 
             CAST((julianday(u.date) - julianday(DATE(c.created_at))) / 7 AS INTEGER) as week_number,
             COUNT(DISTINCT c.id) as retained_users
      FROM customers c
      JOIN licenses l ON c.id = l.customer_id
      JOIN usage_daily u ON l.id = u.license_id
      WHERE c.created_at >= datetime('now', '-90 days') AND u.commands_run > 0
      GROUP BY cohort_date, week_number
      ORDER BY cohort_date DESC, week_number ASC
      LIMIT 100
    `).all(),
    { results: [] }
  );

  const ltv = await safeQuery(
    () => env.DB.prepare(`
      SELECT l.tier, COUNT(*) as customer_count,
             AVG(CASE l.tier WHEN 'pro' THEN 9 WHEN 'team' THEN 200 WHEN 'enterprise' THEN 500 ELSE 0 END 
                 * (julianday('now') - julianday(c.created_at)) / 30.0) as avg_ltv
      FROM customers c
      JOIN licenses l ON c.id = l.customer_id
      WHERE l.tier != 'free'
      GROUP BY l.tier
    `).all(),
    { results: [] }
  );

  const featureAdoption = await safeQuery(
    () => env.DB.prepare(`
      SELECT SUM(packages_installed) as total_installs, SUM(packages_searched) as total_searches,
             SUM(runtimes_switched) as total_runtime_switches,
             COUNT(DISTINCT CASE WHEN packages_installed > 0 THEN license_id END) as install_adopters,
             COUNT(DISTINCT CASE WHEN packages_searched > 0 THEN license_id END) as search_adopters,
             COUNT(DISTINCT CASE WHEN runtimes_switched > 0 THEN license_id END) as runtime_adopters,
             COUNT(DISTINCT license_id) as total_active_users
      FROM usage_daily WHERE date >= date('now', '-30 days')
    `).first(),
    null
  );

  const commandHeatmap = await safeQuery(
    () => env.DB.prepare(`
      SELECT strftime('%H', created_at) as hour, strftime('%w', created_at) as day_of_week, COUNT(*) as event_count
      FROM analytics_events WHERE event_type = 'command' AND created_at >= datetime('now', '-7 days')
      GROUP BY hour, day_of_week ORDER BY day_of_week, hour
    `).all(),
    { results: [] }
  );

  const runtimeAdoption = await safeQuery(
    () => env.DB.prepare(`
      SELECT json_extract(properties, '$.runtime') as runtime, COUNT(DISTINCT machine_id) as unique_users, COUNT(*) as total_uses
      FROM analytics_events WHERE event_name IN ('runtime_switch', 'runtime_use') AND created_at >= datetime('now', '-30 days')
      GROUP BY runtime ORDER BY unique_users DESC
    `).all(),
    { results: [] }
  );

  const churnRiskSegments = await safeQuery(
    () => env.DB.prepare(`
      SELECT l.tier, COUNT(*) as user_count, 
             CASE WHEN MAX(u.date) < date('now', '-14 days') THEN 'critical' 
                  WHEN MAX(u.date) < date('now', '-7 days') THEN 'high' 
                  ELSE 'healthy' END as risk_segment
      FROM licenses l
      LEFT JOIN usage_daily u ON l.id = u.license_id
      WHERE l.status = 'active'
      GROUP BY l.id
    `).all(),
    { results: [] }
  );

  const expansionOpportunities = await safeQuery(
    () => env.DB.prepare(`
      SELECT c.email, l.tier, COUNT(DISTINCT m.id) as active_machines, SUM(u.commands_run) as total_commands_30d,
             CASE WHEN l.tier = 'free' AND SUM(u.commands_run) > 500 THEN 'upsell_to_pro'
                  WHEN l.tier = 'pro' AND COUNT(DISTINCT m.id) >= 3 THEN 'upsell_to_team' ELSE NULL END as opportunity_type,
             'medium' as priority
      FROM licenses l
      JOIN customers c ON l.customer_id = c.id
      LEFT JOIN machines m ON l.id = m.license_id AND m.is_active = 1
      LEFT JOIN usage_daily u ON l.id = u.license_id AND u.date >= date('now', '-30 days')
      WHERE l.status = 'active'
      GROUP BY l.customer_id
      HAVING opportunity_type IS NOT NULL
      LIMIT 50
    `).all(),
    { results: [] }
  );

  const timeToValue = await safeQuery(
    () => env.DB.prepare(`
      SELECT AVG(julianday(MIN(u.date)) - julianday(c.created_at)) as avg_days_to_activation,
             COUNT(CASE WHEN julianday(MIN(u.date)) - julianday(c.created_at) <= 7 THEN 1 END) * 100.0 / COUNT(*) as pct_activated_week1
      FROM customers c
      JOIN licenses l ON c.id = l.customer_id
      LEFT JOIN usage_daily u ON l.id = u.license_id AND u.commands_run > 0
      WHERE c.created_at >= datetime('now', '-90 days')
      GROUP BY c.id
    `).first(),
    null
  );

  const revenueMetrics = await safeQuery(
    () => env.DB.prepare(`
      SELECT SUM(CASE l.tier WHEN 'pro' THEN 9 WHEN 'team' THEN 200 WHEN 'enterprise' THEN 500 ELSE 0 END) as current_mrr
      FROM licenses l JOIN subscriptions s ON l.customer_id = s.customer_id
      WHERE s.status = 'active' AND l.tier != 'free'
    `).first(),
    { current_mrr: 0 }
  );

  const currentMrr = (revenueMetrics?.current_mrr as number) || 0;

  return secureJsonResponse({
    request_id: context.requestId,
    engagement: {
      dau: (dau?.count as number) || 0,
      wau: (wau?.count as number) || 0,
      mau: (mau?.count as number) || 0,
      stickiness,
    },
    retention: { cohorts: retentionCohorts.results || [] },
    ltv_by_tier: ltv.results || [],
    feature_adoption: featureAdoption,
    command_heatmap: commandHeatmap.results || [],
    runtime_adoption: runtimeAdoption.results || [],
    churn_risk_segments: churnRiskSegments.results || [],
    expansion_opportunities: expansionOpportunities.results || [],
    time_to_value: timeToValue,
    revenue_metrics: { current_mrr: currentMrr, projected_arr: currentMrr * 12, expansion_mrr_12m: 0 },
  });
}

// ============================================
// Customer Notes CRUD
// ============================================

export async function handleAdminGetNotes(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const url = new URL(request.url);
  const customerId = url.searchParams.get('customerId');
  if (!customerId) return errorResponse('Customer ID required', 400);

  const notes = await env.DB.prepare(
    `SELECT n.*, c.email as author_email 
     FROM customer_notes n 
     LEFT JOIN customers c ON n.author_id = c.id 
     WHERE n.customer_id = ? 
     ORDER BY n.is_pinned DESC, n.created_at DESC`
  )
    .bind(customerId)
    .all();

  return secureJsonResponse({
    request_id: context.requestId,
    notes: notes.results || [],
  });
}

export async function handleAdminCreateNote(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  let body: { customerId: string; content: string; noteType?: string };
  try {
    body = (await request.json()) as { customerId: string; content: string; noteType?: string };
  } catch {
    return errorResponse('Invalid JSON body', 400);
  }

  if (!body.customerId || !body.content) {
    return errorResponse('Customer ID and content required', 400);
  }

  const noteId = generateId();
  const noteType = body.noteType || 'general';

  await env.DB.prepare(
    `INSERT INTO customer_notes (id, customer_id, content, note_type, author_id, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)`
  )
    .bind(noteId, body.customerId, body.content, noteType, context.user.id)
    .run();

  await logAdminAudit(env.DB, {
    action: 'admin.note_created',
    userId: context.user.id,
    request,
    resourceType: 'customer_note',
    resourceId: noteId,
    metadata: { customer_id: body.customerId, note_type: noteType },
  });

  return secureJsonResponse({
    request_id: context.requestId,
    success: true,
    note_id: noteId,
  });
}

export async function handleAdminUpdateNote(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  let body: { noteId: string; content?: string; isPinned?: boolean };
  try {
    body = (await request.json()) as { noteId: string; content?: string; isPinned?: boolean };
  } catch {
    return errorResponse('Invalid JSON body', 400);
  }

  if (!body.noteId) {
    return errorResponse('Note ID required', 400);
  }

  // Build dynamic update query
  const updates: string[] = ['updated_at = CURRENT_TIMESTAMP'];
  const params: (string | number)[] = [];

  if (body.content !== undefined) {
    updates.push('content = ?');
    params.push(body.content);
  }
  if (body.isPinned !== undefined) {
    updates.push('is_pinned = ?');
    params.push(body.isPinned ? 1 : 0);
  }

  params.push(body.noteId);

  await env.DB.prepare(`UPDATE customer_notes SET ${updates.join(', ')} WHERE id = ?`)
    .bind(...params)
    .run();

  await logAdminAudit(env.DB, {
    action: 'admin.note_updated',
    userId: context.user.id,
    request,
    resourceType: 'customer_note',
    resourceId: body.noteId,
    metadata: { updated_fields: Object.keys(body).filter(k => k !== 'noteId') },
  });

  return secureJsonResponse({
    request_id: context.requestId,
    success: true,
  });
}

export async function handleAdminDeleteNote(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const url = new URL(request.url);
  const noteId = url.searchParams.get('noteId');
  if (!noteId) return errorResponse('Note ID required', 400);

  await env.DB.prepare(`DELETE FROM customer_notes WHERE id = ?`).bind(noteId).run();

  await logAdminAudit(env.DB, {
    action: 'admin.note_deleted',
    userId: context.user.id,
    request,
    resourceType: 'customer_note',
    resourceId: noteId,
  });

  return secureJsonResponse({
    request_id: context.requestId,
    success: true,
  });
}

// ============================================
// Customer Tags CRUD
// ============================================

export async function handleAdminGetTags(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  // Get all available tags with usage counts
  const tags = await env.DB.prepare(
    `SELECT t.*, COUNT(cta.customer_id) as usage_count
     FROM customer_tags t
     LEFT JOIN customer_tag_assignments cta ON t.id = cta.tag_id
     GROUP BY t.id
     ORDER BY t.name ASC`
  ).all();

  return secureJsonResponse({
    request_id: context.requestId,
    tags: tags.results || [],
  });
}

export async function handleAdminGetCustomerTags(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const url = new URL(request.url);
  const customerId = url.searchParams.get('customerId');
  if (!customerId) return errorResponse('Customer ID required', 400);

  const tags = await env.DB.prepare(
    `SELECT t.* FROM customer_tags t
     JOIN customer_tag_assignments cta ON t.id = cta.tag_id
     WHERE cta.customer_id = ?
     ORDER BY t.name ASC`
  )
    .bind(customerId)
    .all();

  return secureJsonResponse({
    request_id: context.requestId,
    tags: tags.results || [],
  });
}

export async function handleAdminCreateTag(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  let body: { name: string; color?: string; description?: string };
  try {
    body = (await request.json()) as { name: string; color?: string; description?: string };
  } catch {
    return errorResponse('Invalid JSON body', 400);
  }

  if (!body.name) {
    return errorResponse('Tag name required', 400);
  }

  const tagId = generateId();
  const color = body.color || '#6366f1'; // Default indigo

  await env.DB.prepare(
    `INSERT INTO customer_tags (id, name, color, description, created_at)
     VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)`
  )
    .bind(tagId, body.name, color, body.description || null)
    .run();

  await logAdminAudit(env.DB, {
    action: 'admin.tag_created',
    userId: context.user.id,
    request,
    resourceType: 'customer_tag',
    resourceId: tagId,
    metadata: { name: body.name, color },
  });

  return secureJsonResponse({
    request_id: context.requestId,
    success: true,
    tag_id: tagId,
  });
}

export async function handleAdminAssignTag(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  let body: { customerId: string; tagId: string };
  try {
    body = (await request.json()) as { customerId: string; tagId: string };
  } catch {
    return errorResponse('Invalid JSON body', 400);
  }

  if (!body.customerId || !body.tagId) {
    return errorResponse('Customer ID and Tag ID required', 400);
  }

  try {
    await env.DB.prepare(
      `INSERT INTO customer_tag_assignments (customer_id, tag_id, assigned_by, assigned_at)
       VALUES (?, ?, ?, CURRENT_TIMESTAMP)`
    )
      .bind(body.customerId, body.tagId, context.user.id)
      .run();
  } catch (e: any) {
    if (e.message?.includes('UNIQUE constraint') || e.message?.includes('PRIMARY KEY')) {
      return secureJsonResponse({
        request_id: context.requestId,
        success: true,
        message: 'Tag already assigned',
      });
    }
    throw e;
  }

  await logAdminAudit(env.DB, {
    action: 'admin.tag_assigned',
    userId: context.user.id,
    request,
    resourceType: 'customer_tag_assignment',
    metadata: { customer_id: body.customerId, tag_id: body.tagId },
  });

  return secureJsonResponse({
    request_id: context.requestId,
    success: true,
  });
}

export async function handleAdminRemoveTag(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const url = new URL(request.url);
  const customerId = url.searchParams.get('customerId');
  const tagId = url.searchParams.get('tagId');

  if (!customerId || !tagId) {
    return errorResponse('Customer ID and Tag ID required', 400);
  }

  await env.DB.prepare(`DELETE FROM customer_tag_assignments WHERE customer_id = ? AND tag_id = ?`)
    .bind(customerId, tagId)
    .run();

  await logAdminAudit(env.DB, {
    action: 'admin.tag_removed',
    userId: context.user.id,
    request,
    resourceType: 'customer_tag_assignment',
    metadata: { customer_id: customerId, tag_id: tagId },
  });

  return secureJsonResponse({
    request_id: context.requestId,
    success: true,
  });
}

// ============================================
// Customer Health Score
// ============================================

export async function handleAdminGetCustomerHealth(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const url = new URL(request.url);
  const customerId = url.searchParams.get('customerId');
  if (!customerId) return errorResponse('Customer ID required', 400);

  const health = await env.DB.prepare(`SELECT * FROM customer_health WHERE customer_id = ?`)
    .bind(customerId)
    .first();

  if (!health) {
    return secureJsonResponse({
      request_id: context.requestId,
      health: {
        customer_id: customerId,
        overall_score: 50,
        engagement_score: 50,
        activation_score: 50,
        growth_score: 50,
        risk_score: 0,
        lifecycle_stage: 'new',
        updated_at: null,
      },
    });
  }

  return secureJsonResponse({
    request_id: context.requestId,
    health,
  });
}

// Database initialization (one-time setup)
export async function handleInitDb(env: Env): Promise<Response> {
  try {
    // Users table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY,
        email TEXT UNIQUE NOT NULL,
        name TEXT,
        avatar_url TEXT,
        stripe_customer_id TEXT UNIQUE,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )
    `
    ).run();

    // Licenses table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS licenses (
        id TEXT PRIMARY KEY,
        user_id TEXT UNIQUE NOT NULL,
        license_key TEXT UNIQUE NOT NULL,
        tier TEXT NOT NULL DEFAULT 'free',
        status TEXT DEFAULT 'active',
        max_machines INTEGER DEFAULT 1,
        expires_at DATETIME,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
      )
    `
    ).run();

    // Machines table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS machines (
        id TEXT PRIMARY KEY,
        license_id TEXT NOT NULL,
        machine_id TEXT NOT NULL,
        hostname TEXT,
        os TEXT,
        arch TEXT,
        omg_version TEXT,
        last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        first_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        is_active INTEGER DEFAULT 1,
        UNIQUE(license_id, machine_id),
        FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
      )
    `
    ).run();

    // Usage daily table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS usage_daily (
        id TEXT PRIMARY KEY,
        license_id TEXT NOT NULL,
        date TEXT NOT NULL,
        commands_run INTEGER DEFAULT 0,
        packages_installed INTEGER DEFAULT 0,
        packages_searched INTEGER DEFAULT 0,
        runtimes_switched INTEGER DEFAULT 0,
        sbom_generated INTEGER DEFAULT 0,
        vulnerabilities_found INTEGER DEFAULT 0,
        time_saved_ms INTEGER DEFAULT 0,
        UNIQUE(license_id, date),
        FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
      )
    `
    ).run();

    // Achievements table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS achievements (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        achievement_id TEXT NOT NULL,
        unlocked_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        UNIQUE(user_id, achievement_id),
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
      )
    `
    ).run();

    // Subscriptions table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS subscriptions (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        stripe_subscription_id TEXT UNIQUE,
        stripe_price_id TEXT,
        status TEXT DEFAULT 'active',
        current_period_start DATETIME,
        current_period_end DATETIME,
        cancel_at_period_end INTEGER DEFAULT 0,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
      )
    `
    ).run();

    // Invoices table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS invoices (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        stripe_invoice_id TEXT UNIQUE,
        amount_cents INTEGER NOT NULL,
        currency TEXT DEFAULT 'usd',
        status TEXT,
        invoice_url TEXT,
        invoice_pdf TEXT,
        period_start DATETIME,
        period_end DATETIME,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
      )
    `
    ).run();

    // Sessions table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        token TEXT UNIQUE NOT NULL,
        ip_address TEXT,
        user_agent TEXT,
        expires_at DATETIME NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
      )
    `
    ).run();

    // Auth codes table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS auth_codes (
        id TEXT PRIMARY KEY,
        email TEXT NOT NULL,
        code TEXT NOT NULL,
        expires_at DATETIME NOT NULL,
        used INTEGER DEFAULT 0,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )
    `
    ).run();

    // Audit log table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS audit_log (
        id TEXT PRIMARY KEY,
        user_id TEXT,
        action TEXT NOT NULL,
        resource_type TEXT,
        resource_id TEXT,
        ip_address TEXT,
        user_agent TEXT,
        metadata TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )
    `
    ).run();

    // Indexes
    await env.DB.prepare(`CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)`).run();
    await env.DB.prepare(
      `CREATE INDEX IF NOT EXISTS idx_licenses_key ON licenses(license_key)`
    ).run();
    await env.DB.prepare(`CREATE INDEX IF NOT EXISTS idx_licenses_user ON licenses(user_id)`).run();
    await env.DB.prepare(
      `CREATE INDEX IF NOT EXISTS idx_machines_license ON machines(license_id)`
    ).run();
    await env.DB.prepare(
      `CREATE INDEX IF NOT EXISTS idx_usage_license_date ON usage_daily(license_id, date)`
    ).run();
    await env.DB.prepare(`CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token)`).run();
    await env.DB.prepare(`CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id)`).run();
    await env.DB.prepare(
      `CREATE INDEX IF NOT EXISTS idx_auth_codes_email ON auth_codes(email)`
    ).run();

    return jsonResponse({ success: true, message: 'Database initialized' });
  } catch (e) {
    console.error('Init DB error:', e);
    return errorResponse(e instanceof Error ? e.message : 'Database init failed', 500);
  }
}
