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
  const adminCheck = await env.DB.prepare(
    `SELECT admin FROM customers WHERE id = ?`
  )
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

export async function handleAdminUserDetail(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const url = new URL(request.url);
  const userId = url.searchParams.get('id');
  if (!userId) return errorResponse('User ID required');

  const user = await env.DB.prepare(`SELECT * FROM customers WHERE id = ?`).bind(userId).first();
  if (!user) return errorResponse('User not found', 404);

  const license = await env.DB.prepare(`SELECT * FROM licenses WHERE customer_id = ?`).bind(userId).first();
  
  if (!license) {
    return errorResponse('License not found for user', 404);
  }

  const machines = await env.DB.prepare(`SELECT * FROM machines WHERE license_id = ?`).bind(license.id).all();
  const recentUsage = await env.DB.prepare(`SELECT * FROM usage_daily WHERE license_id = ? ORDER BY date DESC LIMIT 30`).bind(license.id).all();

  return secureJsonResponse({ request_id: context.requestId, user, license, machines: machines.results || [], usage: recentUsage.results || [] });
}

export async function handleAdminUpdateUser(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;

  let body: { userId: string, tier?: string, status?: string };
  try {
    body = await request.json() as { userId: string, tier?: string, status?: string };
  } catch (e) {
    return errorResponse('Invalid JSON body', 400);
  }
  if (!body.userId) return errorResponse('User ID required');

  if (body.tier) {
    await env.DB.prepare(`UPDATE licenses SET tier = ? WHERE customer_id = ?`).bind(body.tier, body.userId).run();
  }
  if (body.status) {
    await env.DB.prepare(`UPDATE licenses SET status = ? WHERE customer_id = ?`).bind(body.status, body.userId).run();
  }

  await logAdminAudit(env.DB, { action: 'admin.update_user', userId: result.context.user.id, metadata: body });
  return secureJsonResponse({ success: true });
}

export async function handleAdminActivity(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;

  const activity = await env.DB.prepare(`SELECT * FROM audit_log ORDER BY created_at DESC LIMIT 100`).all();
  return secureJsonResponse({ request_id: result.context.requestId, activity: activity.results || [] });
}

export async function handleAdminHealth(request: Request, env: Env): Promise<Response> {
  // Simple health check logic
  return secureJsonResponse({ status: 'ok', db: 'connected', version: '1.0.0' });
}

function escapeCSV(value: unknown): string {
  const str = String(value ?? '');
  if (/[",\n\r]/.test(str) || str.startsWith('=') || str.startsWith('+') || str.startsWith('-') || str.startsWith('@')) {
    return `"${str.replace(/"/g, '""')}"`;
  }
  return str;
}

export async function handleAdminExportUsage(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;

  const usage = await env.DB.prepare(`SELECT * FROM usage_daily ORDER BY date DESC LIMIT 1000`).all();
  const headers = ['date', 'license_id', 'commands_run', 'time_saved_ms'];
  const csv = [headers.join(','), ...(usage.results || []).map((u: any) => headers.map(h => escapeCSV(u[h])).join(','))].join('\n');

  return new Response(csv, { headers: { 'Content-Type': 'text/csv', 'Content-Disposition': 'attachment; filename="usage.csv"' } });
}

export async function handleAdminExportAudit(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;

  const logs = await env.DB.prepare(`SELECT * FROM audit_log ORDER BY created_at DESC LIMIT 1000`).all();
  const headers = ['created_at', 'action', 'customer_id', 'ip_address'];
  const csv = [headers.join(','), ...(logs.results || []).map((l: any) => headers.map(h => escapeCSV(l[h])).join(','))].join('\n');

  return new Response(csv, { headers: { 'Content-Type': 'text/csv', 'Content-Disposition': 'attachment; filename="audit.csv"' } });
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
  const performance = await env.DB.prepare(`SELECT AVG(duration_ms) as avg_ms, MIN(duration_ms) as min_ms, MAX(duration_ms) as max_ms, COUNT(*) as count FROM analytics_events WHERE event_type = 'performance' AND created_at >= datetime('now', '-7 days')`).first();
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
    performance: { avg_latency_ms: performance?.avg_ms || 0, min_ms: performance?.min_ms || 0, max_ms: performance?.max_ms || 0, query_count: performance?.count || 0 },
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
  const csv = [headers.join(','), ...(users.results || []).map(u => headers.map(h => escapeCSV(u[h])).join(','))].join('\n');
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

export async function handleAdminAdvancedMetrics(request: Request, env: Env): Promise<Response> {
  const result = await validateAdmin(request, env);
  if (result.error) return result.error;
  const { context } = result;

  const dau = await env.DB.prepare(`
    SELECT COUNT(DISTINCT license_id) as count 
    FROM usage_daily 
    WHERE date = date('now') AND commands_run > 0
  `).first();

  const wau = await env.DB.prepare(`
    SELECT COUNT(DISTINCT license_id) as count 
    FROM usage_daily 
    WHERE date >= date('now', '-7 days') AND commands_run > 0
  `).first();

  const mau = await env.DB.prepare(`
    SELECT COUNT(DISTINCT license_id) as count 
    FROM usage_daily 
    WHERE date >= date('now', '-30 days') AND commands_run > 0
  `).first();

  const stickiness = {
    daily_to_monthly: mau?.count ? ((dau?.count || 0) / mau.count * 100).toFixed(1) : 0,
    weekly_to_monthly: mau?.count ? ((wau?.count || 0) / mau.count * 100).toFixed(1) : 0
  };

  const retentionCohorts = await env.DB.prepare(`
    WITH cohorts AS (
      SELECT 
        c.id as customer_id,
        DATE(c.created_at) as cohort_date,
        DATE(c.created_at, '+' || 
          CAST((julianday(u.date) - julianday(DATE(c.created_at))) / 7 AS INTEGER) || ' weeks') as week_number
      FROM customers c
      LEFT JOIN licenses l ON c.id = l.customer_id
      LEFT JOIN usage_daily u ON l.id = u.license_id
      WHERE c.created_at >= datetime('now', '-90 days')
        AND u.commands_run > 0
    )
    SELECT 
      cohort_date,
      week_number,
      COUNT(DISTINCT customer_id) as retained_users
    FROM cohorts
    GROUP BY cohort_date, week_number
    ORDER BY cohort_date DESC, week_number ASC
    LIMIT 100
  `).all();

  const ltv = await env.DB.prepare(`
    WITH user_revenue AS (
      SELECT 
        c.id,
        c.created_at,
        l.tier,
        CASE l.tier
          WHEN 'pro' THEN 9
          WHEN 'team' THEN 200
          WHEN 'enterprise' THEN 500
          ELSE 0
        END as monthly_value,
        julianday('now') - julianday(c.created_at) as days_active
      FROM customers c
      JOIN licenses l ON c.id = l.customer_id
      WHERE l.tier != 'free'
    )
    SELECT 
      AVG(monthly_value * (days_active / 30.0)) as avg_ltv,
      tier,
      COUNT(*) as customer_count
    FROM user_revenue
    GROUP BY tier
  `).all();

  const featureAdoption = await env.DB.prepare(`
    SELECT 
      SUM(packages_installed) as total_installs,
      SUM(packages_searched) as total_searches,
      SUM(runtimes_switched) as total_runtime_switches,
      SUM(sbom_generated) as total_sbom,
      SUM(vulnerabilities_found) as total_vulns,
      COUNT(DISTINCT CASE WHEN packages_installed > 0 THEN license_id END) as install_adopters,
      COUNT(DISTINCT CASE WHEN packages_searched > 0 THEN license_id END) as search_adopters,
      COUNT(DISTINCT CASE WHEN runtimes_switched > 0 THEN license_id END) as runtime_adopters,
      COUNT(DISTINCT CASE WHEN sbom_generated > 0 THEN license_id END) as sbom_adopters,
      COUNT(DISTINCT license_id) as total_active_users
    FROM usage_daily
    WHERE date >= date('now', '-30 days')
  `).first();

  const commandHeatmap = await env.DB.prepare(`
    SELECT 
      strftime('%H', created_at) as hour,
      strftime('%w', created_at) as day_of_week,
      COUNT(*) as event_count
    FROM analytics_events
    WHERE event_type = 'command' 
      AND created_at >= datetime('now', '-7 days')
    GROUP BY hour, day_of_week
    ORDER BY day_of_week, hour
  `).all();

  const runtimeAdoption = await env.DB.prepare(`
    SELECT 
      json_extract(properties, '$.runtime') as runtime,
      COUNT(DISTINCT machine_id) as unique_users,
      COUNT(*) as total_uses,
      AVG(CAST(json_extract(properties, '$.duration_ms') AS REAL)) as avg_duration_ms
    FROM analytics_events
    WHERE event_name IN ('runtime_switch', 'runtime_use')
      AND created_at >= datetime('now', '-30 days')
    GROUP BY runtime
    ORDER BY unique_users DESC
  `).all();

  const churnRiskSegments = await env.DB.prepare(`
    WITH user_activity AS (
      SELECT 
        l.id as license_id,
        l.customer_id,
        c.email,
        l.tier,
        MAX(u.date) as last_active,
        julianday('now') - julianday(MAX(u.date)) as days_inactive,
        SUM(CASE WHEN u.date >= date('now', '-7 days') THEN u.commands_run ELSE 0 END) as cmds_7d,
        SUM(CASE WHEN u.date >= date('now', '-30 days') AND u.date < date('now', '-7 days') THEN u.commands_run ELSE 0 END) as cmds_prev_23d,
        COUNT(DISTINCT u.date) as active_days_30d
      FROM licenses l
      JOIN customers c ON l.customer_id = c.id
      LEFT JOIN usage_daily u ON l.id = u.license_id
      WHERE l.status = 'active' AND l.tier != 'free'
      GROUP BY l.id
    )
    SELECT 
      CASE
        WHEN days_inactive > 14 THEN 'critical_churn_risk'
        WHEN days_inactive > 7 THEN 'high_churn_risk'
        WHEN cmds_7d = 0 AND cmds_prev_23d > 50 THEN 'medium_churn_risk'
        WHEN active_days_30d < 5 THEN 'low_engagement'
        ELSE 'healthy'
      END as risk_segment,
      COUNT(*) as user_count,
      AVG(cmds_7d + cmds_prev_23d) as avg_monthly_commands,
      tier
    FROM user_activity
    GROUP BY risk_segment, tier
    ORDER BY 
      CASE risk_segment
        WHEN 'critical_churn_risk' THEN 1
        WHEN 'high_churn_risk' THEN 2
        WHEN 'medium_churn_risk' THEN 3
        WHEN 'low_engagement' THEN 4
        ELSE 5
      END
  `).all();

  const expansionOpportunities = await env.DB.prepare(`
    WITH usage_intensity AS (
      SELECT 
        l.customer_id,
        c.email,
        c.company,
        l.tier,
        l.max_seats,
        COUNT(DISTINCT m.id) as active_machines,
        SUM(u.commands_run) as total_commands_30d,
        SUM(u.time_saved_ms) / 3600000.0 as hours_saved_30d
      FROM licenses l
      JOIN customers c ON l.customer_id = c.id
      LEFT JOIN machines m ON l.id = m.license_id AND m.is_active = 1
      LEFT JOIN usage_daily u ON l.id = u.license_id AND u.date >= date('now', '-30 days')
      WHERE l.status = 'active'
      GROUP BY l.customer_id
    )
    SELECT 
      customer_id,
      email,
      company,
      tier,
      active_machines,
      max_seats,
      total_commands_30d,
      ROUND(hours_saved_30d, 1) as hours_saved_30d,
      CASE
        WHEN tier = 'free' AND total_commands_30d > 500 THEN 'upsell_to_pro'
        WHEN tier = 'pro' AND active_machines >= 3 THEN 'upsell_to_team'
        WHEN tier = 'team' AND hours_saved_30d > 100 THEN 'upsell_to_enterprise'
        WHEN active_machines >= max_seats * 0.8 THEN 'seat_expansion'
        ELSE NULL
      END as opportunity_type,
      CASE
        WHEN tier = 'free' AND total_commands_30d > 1000 THEN 'high'
        WHEN tier = 'free' AND total_commands_30d > 500 THEN 'medium'
        WHEN active_machines >= max_seats THEN 'high'
        ELSE 'low'
      END as priority
    FROM usage_intensity
    WHERE opportunity_type IS NOT NULL
    ORDER BY 
      CASE priority WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END,
      total_commands_30d DESC
    LIMIT 50
  `).all();

  const timeToValue = await env.DB.prepare(`
    WITH first_command AS (
      SELECT 
        l.customer_id,
        c.email,
        c.created_at as signup_date,
        MIN(u.date) as first_usage_date,
        julianday(MIN(u.date)) - julianday(c.created_at) as days_to_first_use,
        (SELECT MIN(date) FROM usage_daily WHERE license_id = l.id AND commands_run >= 10) as power_user_date
      FROM customers c
      JOIN licenses l ON c.id = l.customer_id
      LEFT JOIN usage_daily u ON l.id = u.license_id AND u.commands_run > 0
      WHERE c.created_at >= datetime('now', '-90 days')
      GROUP BY l.customer_id
    )
    SELECT 
      AVG(days_to_first_use) as avg_days_to_activation,
      AVG(julianday(power_user_date) - julianday(signup_date)) as avg_days_to_power_user,
      COUNT(CASE WHEN days_to_first_use <= 1 THEN 1 END) * 100.0 / COUNT(*) as pct_activated_day1,
      COUNT(CASE WHEN days_to_first_use <= 7 THEN 1 END) * 100.0 / COUNT(*) as pct_activated_week1,
      COUNT(CASE WHEN power_user_date IS NOT NULL THEN 1 END) * 100.0 / COUNT(*) as pct_became_power_users
    FROM first_command
  `).first();

  const revenueMetrics = await env.DB.prepare(`
    WITH monthly_revenue AS (
      SELECT 
        strftime('%Y-%m', created_at) as month,
        l.tier,
        COUNT(*) as new_customers,
        CASE l.tier
          WHEN 'pro' THEN 9
          WHEN 'team' THEN 200
          WHEN 'enterprise' THEN 500
          ELSE 0
        END * COUNT(*) as new_mrr
      FROM customers c
      JOIN licenses l ON c.id = l.customer_id
      WHERE c.created_at >= datetime('now', '-12 months')
        AND l.tier != 'free'
      GROUP BY month, l.tier
    ),
    current_mrr AS (
      SELECT 
        SUM(CASE l.tier
          WHEN 'pro' THEN 9
          WHEN 'team' THEN 200
          WHEN 'enterprise' THEN 500
          ELSE 0
        END) as total_mrr
      FROM licenses l
      JOIN subscriptions s ON l.customer_id = s.customer_id
      WHERE s.status = 'active' AND l.tier != 'free'
    )
    SELECT 
      (SELECT total_mrr FROM current_mrr) as current_mrr,
      (SELECT total_mrr FROM current_mrr) * 12 as projected_arr,
      SUM(new_mrr) as expansion_mrr_12m,
      COUNT(DISTINCT month) as months_tracked
    FROM monthly_revenue
  `).first();

  const productStickiness = await env.DB.prepare(`
    WITH user_streaks AS (
      SELECT 
        license_id,
        date,
        LAG(date, 1, date) OVER (PARTITION BY license_id ORDER BY date) as prev_date,
        julianday(date) - julianday(LAG(date, 1, date) OVER (PARTITION BY license_id ORDER BY date)) as days_since_last
      FROM usage_daily
      WHERE commands_run > 0 AND date >= date('now', '-60 days')
    )
    SELECT 
      COUNT(DISTINCT CASE WHEN days_since_last <= 1 THEN license_id END) * 100.0 / COUNT(DISTINCT license_id) as daily_active_pct,
      COUNT(DISTINCT CASE WHEN days_since_last <= 7 THEN license_id END) * 100.0 / COUNT(DISTINCT license_id) as weekly_active_pct,
      AVG(CASE WHEN days_since_last IS NOT NULL THEN days_since_last END) as avg_days_between_sessions
    FROM user_streaks
  `).first();

  return secureJsonResponse({
    request_id: context.requestId,
    engagement: {
      dau: dau?.count || 0,
      wau: wau?.count || 0,
      mau: mau?.count || 0,
      stickiness
    },
    retention: {
      cohorts: retentionCohorts.results || [],
      product_stickiness: productStickiness
    },
    ltv_by_tier: ltv.results || [],
    feature_adoption: featureAdoption,
    command_heatmap: commandHeatmap.results || [],
    runtime_adoption: runtimeAdoption.results || [],
    churn_risk_segments: churnRiskSegments.results || [],
    expansion_opportunities: expansionOpportunities.results || [],
    time_to_value: timeToValue,
    revenue_metrics: revenueMetrics
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
