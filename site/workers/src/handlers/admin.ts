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
  if (!env.ADMIN_USER_ID || auth.user.id !== env.ADMIN_USER_ID) {
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

async function logAdminAudit(db: D1Database, entry: { action: string; userId: string; request?: Request; resourceType?: string; resourceId?: string; metadata?: Record<string, unknown>; success?: boolean }): Promise<void> {
  try {
    const id = generateId();
    const ip = entry.request?.headers.get('CF-Connecting-IP') || null;
    const userAgent = entry.request?.headers.get('User-Agent') || null;
    const country = entry.request?.headers.get('CF-IPCountry') || null;
    await db.prepare(`INSERT INTO audit_log (id, customer_id, action, resource_type, resource_id, ip_address, user_agent, metadata, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)`)
      .bind(id, entry.userId, entry.action, entry.resourceType || null, entry.resourceId || null, ip, userAgent, JSON.stringify({ ...entry.metadata, success: entry.success ?? true, country, timestamp: new Date().toISOString() }))
      .run();
  } catch (e) { console.error('Admin audit log error:', e); }
}

export async function handleAdminDashboard(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const counts = await env.DB.prepare(`SELECT (SELECT COUNT(*) FROM customers) as total_users, (SELECT COUNT(*) FROM licenses WHERE status = 'active') as active_licenses, (SELECT COUNT(*) FROM machines WHERE is_active = 1) as active_machines, (SELECT COUNT(*) FROM install_stats) as total_installs`).first();
  const tierBreakdown = await env.DB.prepare(`SELECT tier, COUNT(*) as count FROM licenses GROUP BY tier`).all();
  const usageTotals = await env.DB.prepare(`SELECT SUM(commands_run) as total_commands, SUM(packages_installed) as total_packages_installed, SUM(packages_searched) as total_searches, SUM(time_saved_ms) as total_time_saved_ms FROM usage_daily WHERE date >= date('now', '-30 days')`).first();
  const dailyActiveUsers = await env.DB.prepare(`SELECT date, COUNT(DISTINCT license_id) as active_users, SUM(commands_run) as commands FROM usage_daily WHERE date >= date('now', '-14 days') GROUP BY date ORDER BY date ASC`).all();
  const recentSignups = await env.DB.prepare(`SELECT DATE(created_at) as date, COUNT(*) as count FROM customers WHERE created_at >= datetime('now', '-7 days') GROUP BY DATE(created_at) ORDER BY date DESC`).all();
  const installsByPlatform = await env.DB.prepare(`SELECT platform, COUNT(*) as count FROM install_stats GROUP BY platform ORDER BY count DESC`).all();
  const installsByVersion = await env.DB.prepare(`SELECT version, COUNT(*) as count FROM install_stats GROUP BY version ORDER BY count DESC LIMIT 10`).all();
  const subscriptionStats = await env.DB.prepare(`SELECT status, COUNT(*) as count FROM subscriptions GROUP BY status`).all();
  const mrrData = await env.DB.prepare(`SELECT l.tier, COUNT(*) as count FROM licenses l JOIN subscriptions s ON l.customer_id = s.customer_id WHERE s.status = 'active' AND l.tier != 'free' GROUP BY l.tier`).all();
  const tierPrices: Record<string, number> = { pro: 9, team: 200, enterprise: 500 };
  let mrr = 0;
  for (const row of mrrData.results || []) {
    mrr += (tierPrices[row.tier as string] || 0) * (row.count as number);
  }
  const globalUsage = await env.DB.prepare(`SELECT SUM(time_saved_ms) as total_time_saved FROM usage_daily`).first();
  const globalValueUSD = Math.round(((Number(globalUsage?.total_time_saved) || 0) / (1000 * 60 * 60)) * 100);
  const fleetVersions = await env.DB.prepare(`SELECT omg_version, COUNT(*) as count FROM machines WHERE is_active = 1 GROUP BY omg_version`).all();
  const geoDist = await env.DB.prepare(`SELECT json_extract(metadata, '$.country') as dimension, COUNT(*) as count FROM audit_log WHERE action = 'machine.registered' AND created_at >= datetime('now', '-30 days') GROUP BY dimension ORDER BY count DESC LIMIT 10`).all();
  const commandStats = await env.DB.prepare(`SELECT SUM(CASE WHEN action LIKE '%.success' THEN 1 ELSE 0 END) as success, SUM(CASE WHEN action LIKE '%.failed' THEN 1 ELSE 0 END) as failure FROM audit_log WHERE created_at >= datetime('now', '-24 hours')`).first();

  return secureJsonResponse({
    request_id: context.requestId,
    overview: {
      total_users: counts?.total_users || 0,
      active_licenses: counts?.active_licenses || 0,
      active_machines: counts?.active_machines || 0,
      total_installs: counts?.total_installs || 0,
      mrr,
      global_value_usd: globalValueUSD,
      command_health: { success: commandStats?.success || 0, failure: commandStats?.failure || 0 }
    },
    fleet: { versions: fleetVersions.results || [] },
    tiers: tierBreakdown.results || [],
    usage: {
      total_commands: usageTotals?.total_commands || 0,
      total_packages_installed: usageTotals?.total_packages_installed || 0,
      total_searches: usageTotals?.total_searches || 0,
      total_time_saved_ms: usageTotals?.total_time_saved_ms || 0
    },
    daily_active_users: dailyActiveUsers.results || [],
    recent_signups: recentSignups.results || [],
    installs_by_platform: installsByPlatform.results || [],
    installs_by_version: installsByVersion.results || [],
    subscriptions: subscriptionStats.results || [],
    geo_distribution: geoDist.results || []
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

  const users = await env.DB.prepare(query).bind(...params).all();
  return secureJsonResponse({ request_id: context.requestId, users: users.results || [], pagination: { page, limit } });
}

export async function handleAdminAnalytics(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const topCommands = await env.DB.prepare(`SELECT json_extract(properties, '$.command') as command, COUNT(*) as count FROM analytics_events WHERE event_type = 'command' GROUP BY 1 ORDER BY 2 DESC LIMIT 10`).all();
  const topErrors = await env.DB.prepare(`SELECT json_extract(properties, '$.error_type') as error_type, COUNT(*) as count FROM analytics_events WHERE event_type = 'error' GROUP BY 1 ORDER BY 2 DESC LIMIT 10`).all();
  const growth = await env.DB.prepare(`SELECT (SELECT COUNT(*) FROM customers WHERE created_at >= datetime('now', '-7 days')) as new_users_7d, (SELECT COUNT(*) FROM subscriptions WHERE status = 'active' AND created_at >= datetime('now', '-7 days')) as new_paid_7d`).first();
  const timeSaved = await env.DB.prepare(`SELECT SUM(time_saved_ms) / 3600000.0 as total_hours FROM usage_daily`).first();
  const funnel = await env.DB.prepare(`SELECT (SELECT COUNT(*) FROM install_stats WHERE created_at >= datetime('now', '-30 days')) as installs, (SELECT COUNT(DISTINCT u.license_id) FROM usage_daily u WHERE u.date >= datetime('now', '-30 days') AND u.commands_run > 0) as activated, (SELECT COUNT(DISTINCT u.license_id) FROM usage_daily u WHERE u.date >= datetime('now', '-30 days') GROUP BY u.license_id HAVING SUM(u.commands_run) > 1000) as power_users`).first();
  const churnRisk = await env.DB.prepare(`SELECT COUNT(*) as at_risk_users FROM (SELECT l.customer_id, (SELECT SUM(commands_run) FROM usage_daily WHERE license_id = l.id AND date >= date('now', '-3 days')) as cmds_3d, (SELECT SUM(commands_run) FROM usage_daily WHERE license_id = l.id AND date >= date('now', '-10 days') AND date < date('now', '-3 days')) as cmds_prev_7d FROM licenses l WHERE l.status = 'active' HAVING (COALESCE(cmds_prev_7d, 0) > 10 AND (COALESCE(cmds_3d, 0) / 3.0) / (COALESCE(cmds_prev_7d, 0) / 7.0 + 0.001) < 0.2) OR (SELECT MAX(date) FROM usage_daily WHERE license_id = l.id) < date('now', '-7 days'))`).first();
  const retentionRate = await env.DB.prepare(`SELECT CASE WHEN (SELECT COUNT(*) FROM customers WHERE created_at >= datetime('now', '-90 days')) = 0 THEN 0 ELSE CAST((SELECT COUNT(DISTINCT u.license_id) FROM usage_daily u WHERE u.date >= datetime('now', '-7 days')) * 100.0 / (SELECT COUNT(*) FROM customers WHERE created_at >= datetime('now', '-90 days')) AS INTEGER) END as rate`).first();
  const performance = await env.DB.prepare(`SELECT AVG(duration_ms) as avg_ms, percentile(0.5) within (duration_ms) as p50_ms, percentile(0.95) within (duration_ms) as p95_ms, percentile(0.99) within (duration_ms) as p99_ms, COUNT(*) as count FROM analytics_events WHERE event_type = 'performance' AND created_at >= datetime('now', '-7 days')`).first();
  const sessions = await env.DB.prepare(`SELECT COUNT(DISTINCT session_id) as total_sessions, COUNT(CASE WHEN event_type = 'session_start' THEN 1 END) as sessions_started, COUNT(CASE WHEN event_type = 'heartbeat' THEN 1 END) as heartbeats_sent, AVG(CASE WHEN event_type = 'session_end' THEN json_extract(properties, '$.duration_seconds') END) as avg_duration_seconds, MAX(CASE WHEN event_type = 'session_end' THEN json_extract(properties, '$.duration_seconds') END) as max_duration_seconds FROM analytics_events WHERE event_type IN ('session_start', 'heartbeat', 'session_end') AND created_at >= datetime('now', '-30 days')`).first();
  const userJourney = await env.DB.prepare(`WITH latest_stages AS (SELECT customer_id, MAX(CASE json_extract(properties, '$.to_stage') WHEN 'installed' THEN 1 WHEN 'activated' THEN 2 WHEN 'first_command' THEN 3 WHEN 'exploring' THEN 4 WHEN 'engaged' THEN 5 WHEN 'power_user' THEN 6 WHEN 'at_risk' THEN 7 WHEN 'churned' THEN 8 ELSE 0 END) as stage_order FROM analytics_events WHERE event_type = 'feature' AND event_name = 'stage_transition' AND created_at >= datetime('now', '-30 days') GROUP BY customer_id) SELECT SUM(CASE WHEN stage_order = 1 THEN 1 END) as installed, SUM(CASE WHEN stage_order = 2 THEN 1 END) as activated, SUM(CASE WHEN stage_order = 3 THEN 1 END) as first_command, SUM(CASE WHEN stage_order = 4 THEN 1 END) as exploring, SUM(CASE WHEN stage_order = 5 THEN 1 END) as engaged, SUM(CASE WHEN stage_order = 6 THEN 1 END) as power_user FROM latest_stages`).first();
  const runtimeUsage = await env.DB.prepare(`SELECT json_extract(properties, '$.runtime') as runtime, COUNT(*) as count, COUNT(DISTINCT machine_id) as machines FROM analytics_events WHERE (event_name = 'runtime_switch' OR event_name = 'runtime_use') AND created_at >= datetime('now', '-30 days') GROUP BY 1 ORDER BY 2 DESC`).all();

  return secureJsonResponse({
    request_id: context.requestId,
    commands_by_type: topCommands.results || [],
    errors_by_type: topErrors.results || [],
    growth: { new_users_7d: growth?.new_users_7d || 0, new_paid_7d: growth?.new_paid_7d || 0, growth_rate: 15 },
    time_saved: { total_hours: timeSaved?.total_hours || 0 },
    funnel: { installs: funnel?.installs || 0, activated: funnel?.activated || 0, power_users: funnel?.power_users || 0 },
    churn_risk: { at_risk_users: churnRisk?.at_risk_users || 0 },
    retention_rate: retentionRate?.rate || 0,
    performance: { avg_latency_ms: performance?.avg_ms || 0, p50: performance?.p50_ms || 0, p95: performance?.p95_ms || 0, p99: performance?.p99_ms || 0, query_count: performance?.count || 0 },
    sessions: { total_30d: sessions?.total_sessions || 0, sessions_started: sessions?.sessions_started || 0, heartbeats_sent: sessions?.heartbeats_sent || 0, avg_duration_seconds: sessions?.avg_duration_seconds || 0, max_duration_seconds: sessions?.max_duration_seconds || 0 },
    user_journey: { funnel: { installed: userJourney?.installed || 0, activated: userJourney?.activated || 0, first_command: userJourney?.first_command || 0, exploring: userJourney?.exploring || 0, engaged: userJourney?.engaged || 0, power_user: userJourney?.power_user || 0 } },
    runtime_usage: runtimeUsage.results || []
  });
}

export async function handleAdminCohorts(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const cohorts = await env.DB.prepare(`
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
  `).all();

  return secureJsonResponse({ request_id: context.requestId, cohorts: cohorts.results || [] });
}

export async function handleAdminRevenue(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;
  const monthlyRevenue = await env.DB.prepare(`SELECT strftime('%Y-%m', created_at) as month, SUM(amount_cents) / 100.0 as revenue, COUNT(*) as transactions FROM invoices WHERE status = 'paid' GROUP BY month ORDER BY month DESC LIMIT 12`).all();
  const revenueByTier = await env.DB.prepare(`SELECT l.tier, SUM(i.amount_cents) / 100.0 as total_revenue, COUNT(DISTINCT l.customer_id) as customers FROM invoices i JOIN licenses l ON i.customer_id = l.customer_id WHERE i.status = 'paid' GROUP BY l.tier`).all();
  const mrrData = await env.DB.prepare(`SELECT l.tier, COUNT(*) as count FROM licenses l JOIN subscriptions s ON l.customer_id = s.customer_id WHERE s.status = 'active' AND l.tier != 'free' GROUP BY l.tier`).all();
  const tierPrices: Record<string, number> = { pro: 9, team: 200, enterprise: 500 };
  let mrr = 0;
  for (const row of mrrData.results || []) { mrr += (tierPrices[row.tier as string] || 0) * (row.count as number); }
  return secureJsonResponse({ request_id: context.requestId, mrr, arr: mrr * 12, monthly_revenue: monthlyRevenue.results || [], revenue_by_tier: revenueByTier.results || [] });
}

export async function handleAdminExportUsers(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const users = await env.DB.prepare(`SELECT c.id, c.email, c.company, c.created_at, l.tier, l.status, (SELECT COUNT(*) FROM machines m WHERE m.license_id = l.id AND m.is_active = 1) as active_machines, (SELECT SUM(commands_run) FROM usage_daily u WHERE u.license_id = l.id) as total_commands FROM customers c LEFT JOIN licenses l ON c.id = l.customer_id ORDER BY c.created_at DESC`).all();
  const headers = ['id', 'email', 'company', 'created_at', 'tier', 'status', 'active_machines', 'total_commands'];
  const csv = [headers.join(','), ...(users.results || []).map(u => headers.map(h => JSON.stringify(u[h] ?? '')).join(','))].join('\n');
  return new Response(csv, { headers: { 'Content-Type': 'text/csv', 'Content-Disposition': `attachment; filename="omg-users.csv"`, ...SECURITY_HEADERS } });
}

export async function handleAdminAuditLog(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;
  const url = new URL(request.url);
  const page = parseInt(url.searchParams.get('page') || '1');
  const limit = Math.min(parseInt(url.searchParams.get('limit') || '50'), 100);
  const logs = await env.DB.prepare(`SELECT a.id, a.customer_id, c.email as user_email, a.action, a.ip_address, a.metadata, a.created_at FROM audit_log a LEFT JOIN customers c ON a.customer_id = c.id ORDER BY a.created_at DESC LIMIT ? OFFSET ?`).bind(limit, (page - 1) * limit).all();
  return secureJsonResponse({ request_id: context.requestId, logs: logs.results || [] });
}
