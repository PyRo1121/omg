import { Env, jsonResponse, errorResponse, validateSession, getAuthToken, logAudit } from '../api';

interface PolicyRule {
  id: string;
  scope: 'runtime' | 'package' | 'security' | 'network';
  rule: string;
  value: string;
  enforced: boolean;
  created_at: string;
}

interface NotificationSetting {
  type: string;
  enabled: boolean;
  threshold?: number;
  channels: string[];
}

export async function handleGetPolicies(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
    return errorResponse('Policies require Team or Enterprise tier', 403);
  }

  const policies = await env.DB.prepare(
    `SELECT id, scope, rule, value, enforced, created_at FROM policies WHERE license_id = ? ORDER BY scope, rule`
  ).bind(license.id).all();

  return jsonResponse({ policies: policies.results || [] });
}

export async function handleCreatePolicy(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || license.tier !== 'enterprise') {
    return errorResponse('Policy management requires Enterprise tier', 403);
  }

  const body = await request.json() as { scope: string; rule: string; value: string; enforced?: boolean };
  const { scope, rule, value, enforced = true } = body;

  if (!scope || !rule || value === undefined) {
    return errorResponse('Missing required fields: scope, rule, value', 400);
  }

  const validScopes = ['runtime', 'package', 'security', 'network'];
  if (!validScopes.includes(scope)) {
    return errorResponse(`Invalid scope. Must be one of: ${validScopes.join(', ')}`, 400);
  }

  const policyId = crypto.randomUUID();
  await env.DB.prepare(
    `INSERT INTO policies (id, license_id, scope, rule, value, enforced, created_at) VALUES (?, ?, ?, ?, ?, ?, datetime('now'))`
  ).bind(policyId, license.id, scope, rule, value, enforced ? 1 : 0).run();

  await logAudit(env.DB, auth.user.id, 'policy.create', 'policy', policyId, request, { scope, rule, value });

  return jsonResponse({ success: true, policy: { id: policyId, scope, rule, value, enforced } });
}

export async function handleUpdatePolicy(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || license.tier !== 'enterprise') {
    return errorResponse('Policy management requires Enterprise tier', 403);
  }

  const body = await request.json() as { id: string; value?: string; enforced?: boolean };
  const { id, value, enforced } = body;

  if (!id) return errorResponse('Missing policy id', 400);

  const existing = await env.DB.prepare(
    `SELECT * FROM policies WHERE id = ? AND license_id = ?`
  ).bind(id, license.id).first();

  if (!existing) return errorResponse('Policy not found', 404);

  const updates: string[] = [];
  const values: any[] = [];

  if (value !== undefined) {
    updates.push('value = ?');
    values.push(value);
  }
  if (enforced !== undefined) {
    updates.push('enforced = ?');
    values.push(enforced ? 1 : 0);
  }

  if (updates.length === 0) return errorResponse('No updates provided', 400);

  values.push(id, license.id);
  await env.DB.prepare(
    `UPDATE policies SET ${updates.join(', ')} WHERE id = ? AND license_id = ?`
  ).bind(...values).run();

  await logAudit(env.DB, auth.user.id, 'policy.update', 'policy', id, request, { value, enforced });

  return jsonResponse({ success: true });
}

export async function handleDeletePolicy(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || license.tier !== 'enterprise') {
    return errorResponse('Policy management requires Enterprise tier', 403);
  }

  const body = await request.json() as { id: string };
  const { id } = body;

  if (!id) return errorResponse('Missing policy id', 400);

  await env.DB.prepare(
    `DELETE FROM policies WHERE id = ? AND license_id = ?`
  ).bind(id, license.id).run();

  await logAudit(env.DB, auth.user.id, 'policy.delete', 'policy', id, request);

  return jsonResponse({ success: true });
}

export async function handleGetNotificationSettings(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
    return errorResponse('Notifications require Team or Enterprise tier', 403);
  }

  const settings = await env.DB.prepare(
    `SELECT type, enabled, threshold, channels FROM notification_settings WHERE license_id = ?`
  ).bind(license.id).all();

  const defaultSettings: NotificationSetting[] = [
    { type: 'vulnerability_critical', enabled: true, channels: ['email', 'dashboard'] },
    { type: 'vulnerability_high', enabled: true, channels: ['dashboard'] },
    { type: 'member_inactive', enabled: true, threshold: 7, channels: ['email'] },
    { type: 'seat_quota_warning', enabled: true, threshold: 80, channels: ['email', 'dashboard'] },
    { type: 'policy_violation', enabled: true, channels: ['email', 'dashboard'] },
    { type: 'license_expiring', enabled: true, threshold: 30, channels: ['email'] },
  ];

  const existingMap = new Map((settings.results || []).map((s: any) => [s.type, s]));
  const merged = defaultSettings.map(def => {
    const existing = existingMap.get(def.type);
    if (existing) {
      return {
        ...def,
        enabled: !!existing.enabled,
        threshold: existing.threshold ?? def.threshold,
        channels: existing.channels ? JSON.parse(existing.channels as string) : def.channels,
      };
    }
    return def;
  });

  return jsonResponse({ settings: merged });
}

export async function handleUpdateNotificationSettings(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
    return errorResponse('Notifications require Team or Enterprise tier', 403);
  }

  const body = await request.json() as { settings: NotificationSetting[] };
  const { settings } = body;

  if (!settings || !Array.isArray(settings)) {
    return errorResponse('Missing settings array', 400);
  }

  for (const setting of settings) {
    await env.DB.prepare(
      `INSERT INTO notification_settings (id, license_id, type, enabled, threshold, channels)
       VALUES (?, ?, ?, ?, ?, ?)
       ON CONFLICT(license_id, type) DO UPDATE SET enabled = ?, threshold = ?, channels = ?`
    ).bind(
      crypto.randomUUID(),
      license.id,
      setting.type,
      setting.enabled ? 1 : 0,
      setting.threshold ?? null,
      JSON.stringify(setting.channels),
      setting.enabled ? 1 : 0,
      setting.threshold ?? null,
      JSON.stringify(setting.channels)
    ).run();
  }

  await logAudit(env.DB, auth.user.id, 'notifications.update', 'settings', null, request);

  return jsonResponse({ success: true });
}

export async function handleRevokeMember(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
    return errorResponse('Member management requires Team or Enterprise tier', 403);
  }

  const body = await request.json() as { machine_id: string };
  const { machine_id } = body;

  if (!machine_id) return errorResponse('Missing machine_id', 400);

  const machine = await env.DB.prepare(
    `SELECT * FROM machines WHERE machine_id = ? AND license_id = ?`
  ).bind(machine_id, license.id).first();

  if (!machine) return errorResponse('Machine not found', 404);

  await env.DB.prepare(
    `UPDATE machines SET is_active = 0, revoked_at = datetime('now') WHERE machine_id = ? AND license_id = ?`
  ).bind(machine_id, license.id).run();

  await env.DB.prepare(
    `UPDATE licenses SET used_seats = MAX(0, used_seats - 1) WHERE id = ?`
  ).bind(license.id).run();

  await logAudit(env.DB, auth.user.id, 'member.revoke', 'machine', machine_id, request, { hostname: machine.hostname });

  return jsonResponse({ success: true, message: 'Machine access revoked' });
}

export async function handleGetAuditLogs(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
    return errorResponse('Audit logs require Team or Enterprise tier', 403);
  }

  const url = new URL(request.url);
  const limit = Math.min(parseInt(url.searchParams.get('limit') || '50'), 200);
  const offset = parseInt(url.searchParams.get('offset') || '0');
  const action = url.searchParams.get('action');
  const resource_type = url.searchParams.get('resource_type');

  let query = `SELECT id, action, resource_type, resource_id, ip_address, user_agent, metadata, created_at 
               FROM audit_log WHERE customer_id = ?`;
  const params: any[] = [auth.user.id];

  if (action) {
    query += ` AND action LIKE ?`;
    params.push(`%${action}%`);
  }
  if (resource_type) {
    query += ` AND resource_type = ?`;
    params.push(resource_type);
  }

  query += ` ORDER BY created_at DESC LIMIT ? OFFSET ?`;
  params.push(limit, offset);

  const logs = await env.DB.prepare(query).bind(...params).all();

  const countResult = await env.DB.prepare(
    `SELECT COUNT(*) as total FROM audit_log WHERE customer_id = ?`
  ).bind(auth.user.id).first();

  return jsonResponse({
    logs: (logs.results || []).map((log: any) => ({
      ...log,
      metadata: log.metadata ? JSON.parse(log.metadata) : null,
    })),
    total: countResult?.total || 0,
    limit,
    offset,
  });
}

export async function handleGetTeamMembers(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier, l.max_seats, l.used_seats FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
    return errorResponse('Team members require Team or Enterprise tier', 403);
  }

  const members = await env.DB.prepare(`
    SELECT 
      m.machine_id,
      m.hostname,
      m.os,
      m.arch,
      m.omg_version,
      m.last_seen_at,
      m.first_seen_at,
      m.is_active,
      COALESCE(SUM(u.commands_run), 0) as total_commands,
      COALESCE(SUM(u.time_saved_ms), 0) as total_time_saved_ms,
      COALESCE(SUM(CASE WHEN u.date >= date('now', '-7 days') THEN u.commands_run ELSE 0 END), 0) as commands_last_7d
    FROM machines m
    LEFT JOIN usage_daily u ON m.machine_id = u.machine_id AND m.license_id = u.license_id
    WHERE m.license_id = ?
    GROUP BY m.machine_id
    ORDER BY m.last_seen_at DESC
  `).bind(license.id).all();

  return jsonResponse({
    members: members.results || [],
    seats: {
      used: license.used_seats || 0,
      max: license.max_seats || 25,
    },
  });
}

export async function handleUpdateAlertThreshold(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const license = await env.DB.prepare(
    `SELECT l.id, l.tier FROM licenses l WHERE l.customer_id = ? AND l.status = 'active'`
  ).bind(auth.user.id).first();

  if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
    return errorResponse('Alert thresholds require Team or Enterprise tier', 403);
  }

  const body = await request.json() as { threshold_type: string; value: number };
  const { threshold_type, value } = body;

  if (!threshold_type || value === undefined) {
    return errorResponse('Missing threshold_type or value', 400);
  }

  await env.DB.prepare(
    `INSERT INTO alert_thresholds (id, license_id, threshold_type, value)
     VALUES (?, ?, ?, ?)
     ON CONFLICT(license_id, threshold_type) DO UPDATE SET value = ?`
  ).bind(crypto.randomUUID(), license.id, threshold_type, value, value).run();

  await logAudit(env.DB, auth.user.id, 'threshold.update', 'alert', threshold_type, request, { value });

  return jsonResponse({ success: true });
}
