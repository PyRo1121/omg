// Dashboard API handlers (all require authentication)
import {
  Env,
  jsonResponse,
  errorResponse,
  validateSession,
  getAuthToken,
  logAudit,
  generateId,
  generateToken,
  TIER_FEATURES,
  ACHIEVEMENTS,
} from '../api';

// Get current user's dashboard data
export async function handleGetDashboard(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  const { user } = auth;

  // Check if user is admin (server-side only - ADMIN_USER_ID is a Cloudflare secret)
  const isAdmin = env.ADMIN_USER_ID ? user.id === env.ADMIN_USER_ID : false;

  // Get license
  const license = await env.DB.prepare(
    `
    SELECT * FROM licenses WHERE customer_id = ?
  `
  )
    .bind(user.id)
    .first();

  if (!license) {
    return errorResponse('License not found', 404);
  }

  // Get machines
  const machines = await env.DB.prepare(
    `
    SELECT * FROM machines WHERE license_id = ? AND is_active = 1
    ORDER BY last_seen_at DESC
  `
  )
    .bind(license.id)
    .all();

  // Get usage stats (last 30 days aggregated)
  const usageStats = await env.DB.prepare(
    `
    SELECT
      SUM(commands_run) as total_commands,
      SUM(packages_installed) as total_packages_installed,
      SUM(packages_searched) as total_packages_searched,
      SUM(runtimes_switched) as total_runtimes_switched,
      SUM(sbom_generated) as total_sbom_generated,
      SUM(vulnerabilities_found) as total_vulnerabilities_found,
      SUM(time_saved_ms) as total_time_saved_ms
    FROM usage_daily
    WHERE license_id = ? AND date >= date('now', '-30 days')
  `
  )
    .bind(license.id)
    .first();

  // Get daily usage for chart (last 14 days)
  const dailyUsage = await env.DB.prepare(
    `
    SELECT date, commands_run, time_saved_ms
    FROM usage_daily
    WHERE license_id = ? AND date >= date('now', '-14 days')
    ORDER BY date ASC
  `
  )
    .bind(license.id)
    .all();

  // Get achievements
  const unlockedAchievements = await env.DB.prepare(
    `
    SELECT achievement_id, unlocked_at FROM achievements WHERE customer_id = ?
  `
  )
    .bind(user.id)
    .all();

  const achievementIds = new Set(unlockedAchievements.results?.map((a: any) => a.achievement_id) || []);
  const achievements = ACHIEVEMENTS.map(a => ({
    ...a,
    unlocked: achievementIds.has(a.id),
    unlocked_at: unlockedAchievements.results?.find((ua: any) => ua.achievement_id === a.id)?.unlocked_at,
  }));

  // Calculate streak
  const streakData = await env.DB.prepare(
    `
    SELECT date FROM usage_daily
    WHERE license_id = ? AND commands_run > 0
    ORDER BY date DESC LIMIT 60
  `
  )
    .bind(license.id)
    .all();

  let currentStreak = 0;
  let longestStreak = 0;
  if (streakData.results && streakData.results.length > 0) {
    const dates = streakData.results.map((r: any) => r.date as string);
    const today = new Date().toISOString().split('T')[0];
    const yesterday = new Date(Date.now() - 86400000).toISOString().split('T')[0];

    // Check if streak is active (used today or yesterday)
    if (dates[0] === today || dates[0] === yesterday) {
      currentStreak = 1;
      for (let i = 1; i < dates.length; i++) {
        const prevDate = new Date(dates[i - 1]);
        const currDate = new Date(dates[i]);
        const diffDays = (prevDate.getTime() - currDate.getTime()) / 86400000;
        if (diffDays === 1) {
          currentStreak++;
        } else {
          break;
        }
      }
    }
    longestStreak = Math.max(currentStreak, longestStreak);
  }

  // Get subscription info
  const subscription = await env.DB.prepare(
    `
    SELECT * FROM subscriptions WHERE customer_id = ? ORDER BY created_at DESC LIMIT 1
  `
  )
    .bind(user.id)
    .first();

  // Get recent invoices
  const invoices = await env.DB.prepare(
    `
    SELECT * FROM invoices WHERE customer_id = ? ORDER BY created_at DESC LIMIT 10
  `
  )
    .bind(user.id)
    .all();

  // Calculate MRR
  const tier = license.tier as keyof typeof TIER_FEATURES;
  const tierConfig = TIER_FEATURES[tier] || TIER_FEATURES.free;

  // Get command breakdown
  const commandBreakdown = await env.DB.prepare(`
    SELECT packages_installed, packages_searched, runtimes_switched, sbom_generated, vulnerabilities_found
    FROM usage_daily
    WHERE license_id = ? AND date >= date('now', '-30 days')
  `)
    .bind(license.id)
    .all();

  let installed = 0, searched = 0, switched = 0, sbom = 0, vulns = 0;
  for (const row of (commandBreakdown.results || []) as any[]) {
    installed += row.packages_installed || 0;
    searched += row.packages_searched || 0;
    switched += row.runtimes_switched || 0;
    sbom += row.sbom_generated || 0;
    vulns += row.vulnerabilities_found || 0;
  }

  // Get global stats for telemetry section
  const topPackage = await env.DB.prepare(`
    SELECT package_name FROM analytics_packages ORDER BY install_count DESC LIMIT 1
  `).first<{ package_name: string }>();

  const topRuntime = await env.DB.prepare(`
    SELECT dimension FROM analytics_daily WHERE metric = 'version' ORDER BY value DESC LIMIT 1
  `).first<{ dimension: string }>();

  // Calculate user percentile
  const userTotalCommands = Number(usageStats?.total_commands) || 0;
  const rankResult = await env.DB.prepare(`
    SELECT COUNT(*) as better_users FROM (
      SELECT SUM(commands_run) as total FROM usage_daily GROUP BY license_id HAVING total > ?
    )
  `).bind(userTotalCommands).first<{ better_users: number }>();

  const totalUsersResult = await env.DB.prepare(`SELECT COUNT(DISTINCT license_id) as count FROM usage_daily`).first<{ count: number }>();
  const totalUsers = Number(totalUsersResult?.count) || 1;
  const percentile = Math.round((1 - ((Number(rankResult?.better_users) || 0) / totalUsers)) * 100);

  return jsonResponse({
    user: {
      id: user.id,
      email: user.email,
      name: user.name,
      avatar_url: user.avatar_url,
      created_at: user.created_at,
    },
    license: {
      id: license.id,
      license_key: license.license_key,
      tier: license.tier,
      status: license.status,
      max_machines: license.max_seats || license.max_machines || 1,
      expires_at: license.expires_at,
      features: tierConfig.features,
    },
    machines: machines.results || [],
    usage: {
      total_commands: usageStats?.total_commands || 0,
      total_packages_installed: usageStats?.total_packages_installed || 0,
      total_packages_searched: usageStats?.total_packages_searched || 0,
      total_runtimes_switched: usageStats?.total_runtimes_switched || 0,
      total_sbom_generated: usageStats?.total_sbom_generated || 0,
      total_vulnerabilities_found: usageStats?.total_vulnerabilities_found || 0,
      total_time_saved_ms: usageStats?.total_time_saved_ms || 0,
      current_streak: currentStreak,
      longest_streak: longestStreak,
      daily: dailyUsage.results || [],
      breakdown: {
        installed,
        searched,
        switched,
        sbom,
        vulns
      }
    },
    achievements,
    subscription: subscription
      ? {
          status: subscription.status,
          current_period_start: subscription.current_period_start,
          cancel_at_period_end: subscription.cancel_at_period_end,
        }
      : null,
    invoices: invoices.results || [],
    is_admin: isAdmin,
    leaderboard: await env.DB.prepare(`
      SELECT SUBSTR(c.email, 1, 3) || '***' as user, SUM(u.time_saved_ms) as time_saved
      FROM usage_daily u
      JOIN licenses l ON u.license_id = l.id
      JOIN customers c ON l.customer_id = c.id
      GROUP BY c.id
      ORDER BY time_saved DESC
      LIMIT 3
    `).all().then(r => r.results || []),
    global_stats: {
      top_package: topPackage?.package_name || 'ripgrep',
      top_runtime: topRuntime?.dimension || 'node',
      percentile: percentile
    }
  });
}

// Update user profile
export async function handleUpdateProfile(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  let body: { name?: string };
  try {
    body = (await request.json()) as { name?: string };
  } catch (e) {
    return errorResponse('Invalid JSON body', 400);
  }
  const { user } = auth;

  if (body.name !== undefined) {
    await env.DB.prepare(
      `
      UPDATE customers SET company = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?
    `
    )
      .bind(body.name || null, user.id)
      .run();
  }

  await logAudit(env.DB, user.id, 'user.profile_updated', 'customer', user.id, request);

  return jsonResponse({ success: true });
}

// Regenerate license key
export async function handleRegenerateLicense(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  const { user } = auth;

  // Get current license
  const license = await env.DB.prepare(
    `
    SELECT * FROM licenses WHERE customer_id = ?
  `
  )
    .bind(user.id)
    .first();

  if (!license) {
    return errorResponse('License not found', 404);
  }

  // Generate new key
  const newLicenseKey = crypto.randomUUID();

  await env.DB.prepare(
    `
    UPDATE licenses SET license_key = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?
  `
  )
    .bind(newLicenseKey, license.id)
    .run();

  // Deactivate all machines (they need to re-activate)
  await env.DB.prepare(
    `
    UPDATE machines SET is_active = 0 WHERE license_id = ?
  `
  )
    .bind(license.id)
    .run();

  await logAudit(env.DB, user.id, 'license.regenerated', 'license', license.id as string, request);

  return jsonResponse({
    success: true,
    license_key: newLicenseKey,
    message: 'License key regenerated. All machines need to re-activate.',
  });
}

// Revoke a machine
export async function handleRevokeMachine(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  let body: { machine_id?: string };
  try {
    body = (await request.json()) as { machine_id?: string };
  } catch (e) {
    return errorResponse('Invalid JSON body', 400);
  }
  const { user } = auth;

  if (!body.machine_id) {
    return errorResponse('Machine ID required');
  }

  // Get license
  const license = await env.DB.prepare(
    `
    SELECT * FROM licenses WHERE customer_id = ?
  `
  )
    .bind(user.id)
    .first();

  if (!license) {
    return errorResponse('License not found', 404);
  }

  // Deactivate machine
  const result = await env.DB.prepare(
    `
    UPDATE machines SET is_active = 0 WHERE license_id = ? AND id = ?
  `
  )
    .bind(license.id, body.machine_id)
    .run();

  if (result.meta.changes === 0) {
    return errorResponse('Machine not found', 404);
  }

  await logAudit(env.DB, user.id, 'machine.revoked', 'machine', body.machine_id, request);

  return jsonResponse({ success: true });
}

// Get active sessions
export async function handleGetSessions(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  const sessions = await env.DB.prepare(
    `
    SELECT id, ip_address, user_agent, created_at, expires_at
    FROM sessions
    WHERE customer_id = ? AND expires_at > datetime('now')
    ORDER BY created_at DESC
  `
  )
    .bind(auth.user.id)
    .all();

  return jsonResponse({
    sessions:
      sessions.results?.map((s: any) => ({
        ...s,
        is_current: s.id === auth.session.id,
      })) || [],
  });
}

// Revoke a session
export async function handleRevokeSession(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  let body: { session_id?: string };
  try {
    body = (await request.json()) as { session_id?: string };
  } catch (e) {
    return errorResponse('Invalid JSON body', 400);
  }

  if (!body.session_id) {
    return errorResponse('Session ID required');
  }

  // Can't revoke current session via this endpoint
  if (body.session_id === auth.session.id) {
    return errorResponse('Cannot revoke current session. Use logout instead.');
  }

  await env.DB.prepare(
    `
    DELETE FROM sessions WHERE id = ? AND customer_id = ?
  `
  )
    .bind(body.session_id, auth.user.id)
    .run();

  await logAudit(env.DB, auth.user.id, 'session.revoked', 'session', body.session_id, request);

  return jsonResponse({ success: true });
}

// Get team members and their usage (for Team/Enterprise tiers)
export async function handleGetTeamMembers(request: Request, env: Env): Promise<Response> {
  try {
    const token = getAuthToken(request);
    if (!token) {
      return errorResponse('Authorization required', 401);
    }

    const auth = await validateSession(env.DB, token);
    if (!auth) {
      return errorResponse('Invalid or expired session', 401);
    }

    // Get license and check tier
    const license = await env.DB.prepare(`
      SELECT * FROM licenses WHERE customer_id = ?
    `)
      .bind(auth.user.id)
      .first();

    if (!license) {
      return errorResponse('License not found', 404);
    }

    if (!['team', 'enterprise'].includes(license.tier as string)) {
      return errorResponse('Team management requires Team or Enterprise tier', 403);
    }

  // Get all machines (team members)
  const machines = await env.DB.prepare(`
    SELECT
      m.id,
      m.machine_id,
      m.hostname,
      m.os,
      m.arch,
      m.omg_version,
      m.user_name,
      m.user_email,
      m.is_active,
      m.first_seen_at,
      m.last_seen_at
    FROM machines m
    WHERE m.license_id = ?
    ORDER BY m.last_seen_at DESC
  `)
    .bind(license.id)
    .all();

  // Get real per-member usage stats
  const memberUsage = await env.DB.prepare(`
    SELECT
      machine_id,
      SUM(commands_run) as total_commands,
      SUM(packages_installed) as total_packages,
      SUM(time_saved_ms) as total_time_saved_ms,
      MAX(date) as last_active
    FROM usage_member_daily
    WHERE license_id = ?
    GROUP BY machine_id
  `)
    .bind(license.id)
    .all();

  const usageMap = new Map(memberUsage.results?.map((u: any) => [u.machine_id, u]) || []);

  // Get last 7 days usage
  const recentUsage = await env.DB.prepare(`
    SELECT
      machine_id,
      SUM(commands_run) as commands_last_7d
    FROM usage_member_daily
    WHERE license_id = ? AND date >= date('now', '-7 days')
    GROUP BY machine_id
  `)
    .bind(license.id)
    .all();

  const recentMap = new Map(recentUsage.results?.map((u: any) => [u.machine_id, u.commands_last_7d]) || []);

  const totalUsage = await env.DB.prepare(`
    SELECT
      SUM(commands_run) as total_commands,
      SUM(packages_installed) as total_packages,
      SUM(time_saved_ms) as total_time_saved_ms
    FROM usage_daily
    WHERE license_id = ?
  `).bind(license.id).first();

  const membersWithUsage = (machines.results || []).map((m: any) => {
    const usage = usageMap.get(m.machine_id as string) as any || {};
    const recent = recentMap.get(m.machine_id as string) || 0;
    return {
      ...m,
      total_commands: Number(usage.total_commands || 0),
      total_packages: Number(usage.total_packages || 0),
      total_time_saved_ms: Number(usage.total_time_saved_ms || 0),
      commands_last_7d: Number(recent),
      last_active: usage.last_active || m.last_seen_at,
    };
  });

  // Calculate fleet compliance (version drift)
  const versions = (machines.results || []).map((m: any) => m.omg_version || 'unknown');
  const uniqueVersions = [...new Set(versions)];
  const latestVersion = uniqueVersions.sort().reverse()[0] || 'unknown';
  const complianceRate = (versions.filter(v => v === latestVersion).length / (versions.length || 1)) * 100;

  // Calculate ROI (Return on Investment)
  const totalHoursSaved = (Number(totalUsage?.total_time_saved_ms) || 0) / (1000 * 60 * 60);
  const totalValueUSD = Math.round(totalHoursSaved * 100);

  // Get daily usage breakdown (last 14 days)
  const dailyUsage = await env.DB.prepare(`
    SELECT
      date,
      commands_run,
      time_saved_ms
    FROM usage_daily
    WHERE license_id = ? AND date >= date('now', '-14 days')
    ORDER BY date DESC
  `)
    .bind(license.id)
    .all();

  // Get team totals
  const totalMachines = machines.results?.length || 0;
  const activeMachines = (machines.results || []).filter((m: any) => m.is_active === 1).length;
  const totalCommands = Number(totalUsage?.total_commands) || 0;
  const totalTimeSaved = Number(totalUsage?.total_time_saved_ms) || 0;

  return jsonResponse({
    license: {
      tier: license.tier,
      max_seats: license.max_seats,
      status: license.status,
    },
    members: membersWithUsage,
    daily_usage: dailyUsage.results || [],
    totals: {
      total_machines: totalMachines,
      active_machines: activeMachines,
      total_commands: totalCommands,
      total_time_saved_ms: totalTimeSaved,
      total_time_saved_hours: Math.round(totalTimeSaved / (1000 * 60 * 60) * 10) / 10,
      total_value_usd: totalValueUSD,
    },
    fleet_health: {
      compliance_rate: Math.round(complianceRate),
      latest_version: latestVersion,
      version_drift: uniqueVersions.length > 1,
    },
    productivity_score: Math.min(100, Math.round((totalCommands / 1000) * 100)),
    insights: {
      engagement_rate: Math.round((activeMachines / (totalMachines || 1)) * 100),
      roi_multiplier: totalValueUSD > 0 ? (totalValueUSD / 200).toFixed(1) : '0',
    },
  });
  } catch (error) {
    console.error('handleGetTeamMembers error:', error);
    return errorResponse('Failed to load team data', 500);
  }
}

// Revoke a team member's machine access
export async function handleRevokeTeamMember(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  let body: { machine_id?: string };
  try {
    body = (await request.json()) as { machine_id?: string };
  } catch (e) {
    return errorResponse('Invalid JSON body', 400);
  }
  if (!body.machine_id) {
    return errorResponse('Machine ID required');
  }

  // Get license
  const license = await env.DB.prepare(`
    SELECT * FROM licenses WHERE customer_id = ?
  `)
    .bind(auth.user.id)
    .first();

  if (!license) {
    return errorResponse('License not found', 404);
  }

  // Deactivate the machine
  const result = await env.DB.prepare(`
    UPDATE machines SET is_active = 0 WHERE license_id = ? AND id = ?
  `)
    .bind(license.id, body.machine_id)
    .run();

  if (result.meta.changes === 0) {
    return errorResponse('Machine not found', 404);
  }

  await logAudit(
    env.DB,
    auth.user.id,
    'team.member_revoked',
    'machine',
    body.machine_id,
    request
  );

  return jsonResponse({ success: true });
}

// Get audit log
export async function handleGetAuditLog(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  // Only team+ tiers can access audit logs
  const license = await env.DB.prepare(
    `
    SELECT tier FROM licenses WHERE customer_id = ?
  `
  )
    .bind(auth.user.id)
    .first();

  if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
    return errorResponse('Audit logs require Team or Enterprise tier', 403);
  }

  const logs = await env.DB.prepare(
    `
    SELECT id, action, resource_type, resource_id, ip_address, created_at
    FROM audit_log
    WHERE customer_id = ?
    ORDER BY created_at DESC
    LIMIT 100
  `
  )
    .bind(auth.user.id)
    .all();

  return jsonResponse({ logs: logs.results || [] });
}

// Placeholder for policies
export async function handleGetTeamPolicies(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);
  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  // Return empty list for now (Production-ready placeholder)
  return jsonResponse({ policies: [] });
}

// Placeholder for notifications
export async function handleGetNotifications(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);
  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  return jsonResponse({ settings: [] });
}
