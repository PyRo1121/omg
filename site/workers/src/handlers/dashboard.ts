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

  const achievementIds = new Set(unlockedAchievements.results?.map(a => a.achievement_id) || []);
  const achievements = ACHIEVEMENTS.map(a => ({
    ...a,
    unlocked: achievementIds.has(a.id),
    unlocked_at: unlockedAchievements.results?.find(ua => ua.achievement_id === a.id)?.unlocked_at,
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
    const dates = streakData.results.map(r => r.date as string);
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
  const userTotalCommands = Number(totalUsage?.total_commands) || 0;
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
          current_period_end: subscription.current_period_end,
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

  const body = (await request.json()) as { name?: string };
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

  const body = (await request.json()) as { machine_id?: string };
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
      sessions.results?.map(s => ({
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

  const body = (await request.json()) as { session_id?: string };

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

  const membersWithUsage = (machines.results || []).map((m: Record<string, unknown>) => {
    const usage = usageMap.get(m.machine_id as string) || {};
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
  // Industry standard: $100/hr for engineering time
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
  const activeMachines = (machines.results || []).filter((m: Record<string, unknown>) => m.is_active === 1).length;
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
    productivity_score: Math.min(100, Math.round((totalCommands / 1000) * 100)), // Real score based on team output
    achievements: {
      unique_achievements: 0, // Placeholder
      total_unlocks: 0,
    },
    insights: {
      engagement_rate: Math.round((activeMachines / (totalMachines || 1)) * 100),
      roi_multiplier: totalValueUSD > 0 ? (totalValueUSD / 200).toFixed(1) : '0', // vs Team cost ($200/mo)
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

  const body = (await request.json()) as { machine_id?: string };
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

// Get admin analytics (comprehensive telemetry data)
export async function handleGetAdminAnalytics(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) {
    return errorResponse('Authorization required', 401);
  }

  const auth = await validateSession(env.DB, token);
  if (!auth) {
    return errorResponse('Invalid or expired session', 401);
  }

  // Admin only
  const isAdmin = env.ADMIN_USER_ID ? auth.user.id === env.ADMIN_USER_ID : false;
  if (!isAdmin) {
    return errorResponse('Admin access required', 403);
  }

  try {
    // Get date ranges
    const today = new Date().toISOString().split('T')[0];
    const sevenDaysAgo = new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0];
    const thirtyDaysAgo = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString().split('T')[0];

    // DAU/WAU/MAU
    const dauResult = await env.DB.prepare(`
      SELECT COUNT(DISTINCT machine_id) as count FROM analytics_active_users WHERE date = ?
    `).bind(today).first();

    const wauResult = await env.DB.prepare(`
      SELECT COUNT(DISTINCT machine_id) as count FROM analytics_active_users WHERE date >= ?
    `).bind(sevenDaysAgo).first();

    const mauResult = await env.DB.prepare(`
      SELECT COUNT(DISTINCT machine_id) as count FROM analytics_active_users WHERE date >= ?
    `).bind(thirtyDaysAgo).first();

    // Commands by type (top 10)
    const commandsByType = await env.DB.prepare(`
      SELECT dimension as command, SUM(value) as count
      FROM analytics_daily
      WHERE metric = 'commands' AND date >= ?
      GROUP BY dimension
      ORDER BY count DESC
      LIMIT 10
    `).bind(sevenDaysAgo).all();

    // Features by usage (top 10)
    const featuresByUsage = await env.DB.prepare(`
      SELECT dimension as feature, SUM(value) as count
      FROM analytics_daily
      WHERE metric = 'features' AND date >= ?
      GROUP BY dimension
      ORDER BY count DESC
      LIMIT 10
    `).bind(sevenDaysAgo).all();

    // Errors by type
    const errorsByType = await env.DB.prepare(`
      SELECT dimension as error_type, SUM(value) as count
      FROM analytics_daily
      WHERE metric = 'errors' AND date >= ?
      GROUP BY dimension
      ORDER BY count DESC
      LIMIT 10
    `).bind(sevenDaysAgo).all();

    // Daily active users trend (last 30 days)
    const dauTrend = await env.DB.prepare(`
      SELECT date, COUNT(DISTINCT machine_id) as active_users
      FROM analytics_active_users
      WHERE date >= ?
      GROUP BY date
      ORDER BY date ASC
    `).bind(thirtyDaysAgo).all();

    // Daily commands trend
    const commandsTrend = await env.DB.prepare(`
      SELECT date, SUM(value) as commands
      FROM analytics_daily
      WHERE metric = 'total_commands' AND date >= ?
      GROUP BY date
      ORDER BY date ASC
    `).bind(thirtyDaysAgo).all();

    // Sessions trend
    const sessionsTrend = await env.DB.prepare(`
      SELECT date, SUM(value) as sessions
      FROM analytics_daily
      WHERE metric = 'sessions' AND date >= ?
      GROUP BY date
      ORDER BY date ASC
    `).bind(thirtyDaysAgo).all();

    // Performance percentiles (last 7 days)
    const performanceData = await env.DB.prepare(`
      SELECT operation, duration_ms
      FROM analytics_performance
      WHERE created_at >= datetime('now', '-7 days')
      ORDER BY operation, duration_ms
    `).all();

    // Calculate percentiles per operation
    const performanceByOp: Record<string, number[]> = {};
    for (const row of (performanceData.results || []) as Array<{ operation: string; duration_ms: number }>) {
      if (!performanceByOp[row.operation]) {
        performanceByOp[row.operation] = [];
      }
      performanceByOp[row.operation].push(row.duration_ms);
    }

    const performancePercentiles: Record<string, { p50: number; p95: number; p99: number; count: number }> = {};
    for (const [op, durations] of Object.entries(performanceByOp)) {
      durations.sort((a, b) => a - b);
      const count = durations.length;
      performancePercentiles[op] = {
        p50: durations[Math.floor(count * 0.5)] || 0,
        p95: durations[Math.floor(count * 0.95)] || 0,
        p99: durations[Math.floor(count * 0.99)] || 0,
        count,
      };
    }

    // Version distribution
    const versionDist = await env.DB.prepare(`
      SELECT version, COUNT(DISTINCT machine_id) as count
      FROM analytics_events
      WHERE created_at >= datetime('now', '-7 days') AND version IS NOT NULL
      GROUP BY version
      ORDER BY count DESC
      LIMIT 10
    `).all();

    // Platform distribution
    const platformDist = await env.DB.prepare(`
      SELECT platform, COUNT(DISTINCT machine_id) as count
      FROM analytics_events
      WHERE created_at >= datetime('now', '-7 days') AND platform IS NOT NULL
      GROUP BY platform
      ORDER BY count DESC
    `).all();

    // Retention: users active this week who were also active last week
    const thisWeekUsers = await env.DB.prepare(`
      SELECT DISTINCT machine_id FROM analytics_active_users WHERE date >= ?
    `).bind(sevenDaysAgo).all();

    const fourteenDaysAgo = new Date(Date.now() - 14 * 24 * 60 * 60 * 1000).toISOString().split('T')[0];
    const lastWeekUsers = await env.DB.prepare(`
      SELECT DISTINCT machine_id FROM analytics_active_users WHERE date >= ? AND date < ?
    `).bind(fourteenDaysAgo, sevenDaysAgo).all();

    const thisWeekSet = new Set((thisWeekUsers.results || []).map((r) => (r as { machine_id: string }).machine_id));
    const lastWeekSet = new Set((lastWeekUsers.results || []).map((r) => (r as { machine_id: string }).machine_id));
    const retained = [...lastWeekSet].filter(id => thisWeekSet.has(id)).length;
    const retentionRate = lastWeekSet.size > 0 ? (retained / lastWeekSet.size) * 100 : 0;

    // Total events today
    const eventsToday = await env.DB.prepare(`
      SELECT COUNT(*) as count FROM analytics_events WHERE DATE(created_at) = ?
    `).bind(today).first();

    // Funnel analytics: Install → Activate → First Command → Engaged → Power User
    const funnelData = await env.DB.prepare(`
      SELECT 
        (SELECT COUNT(DISTINCT install_id) FROM install_stats) as installs,
        (SELECT COUNT(*) FROM licenses WHERE status = 'active') as activated,
        (SELECT COUNT(DISTINCT machine_id) FROM analytics_events WHERE event_name = 'search' OR event_name = 'install') as first_command,
        (SELECT COUNT(DISTINCT machine_id) FROM analytics_active_users WHERE date >= ?) as engaged_7d,
        (SELECT COUNT(DISTINCT machine_id) FROM (
          SELECT machine_id, COUNT(*) as cmd_count 
          FROM analytics_events 
          WHERE event_type = 'command' AND created_at >= datetime('now', '-30 days')
          GROUP BY machine_id 
          HAVING cmd_count >= 100
        )) as power_users
    `).bind(sevenDaysAgo).first();

    // Cohort analysis: Users by signup week and their retention
    const cohortData = await env.DB.prepare(`
      SELECT 
        strftime('%Y-%W', created_at) as cohort_week,
        COUNT(*) as users,
        (SELECT COUNT(DISTINCT au.machine_id) 
         FROM analytics_active_users au 
         JOIN machines m ON au.machine_id = m.machine_id
         WHERE strftime('%Y-%W', m.first_seen_at) = strftime('%Y-%W', customers.created_at)
         AND au.date >= date('now', '-7 days')
        ) as active_this_week
      FROM customers
      WHERE created_at >= datetime('now', '-90 days')
      GROUP BY cohort_week
      ORDER BY cohort_week DESC
      LIMIT 12
    `).all();

    // User journey stage distribution
    const stageDistribution = await env.DB.prepare(`
      SELECT dimension as stage, SUM(value) as count
      FROM analytics_daily
      WHERE metric = 'features' AND dimension LIKE 'stage_%' AND date >= ?
      GROUP BY dimension
      ORDER BY count DESC
    `).bind(sevenDaysAgo).all();

    // Geographic distribution (from timezone/locale)
    const geoDistribution = await env.DB.prepare(`
      SELECT 
        json_extract(properties, '$.timezone') as timezone,
        COUNT(DISTINCT machine_id) as users
      FROM analytics_events
      WHERE event_name = 'geo_info' AND created_at >= datetime('now', '-30 days')
      GROUP BY timezone
      ORDER BY users DESC
      LIMIT 20
    `).all();

    // Churn risk: Users who were active but haven't been seen in 7-14 days
    const churnRisk = await env.DB.prepare(`
      SELECT COUNT(DISTINCT machine_id) as at_risk_users
      FROM analytics_active_users
      WHERE date >= ? AND date < ?
      AND machine_id NOT IN (
        SELECT DISTINCT machine_id FROM analytics_active_users WHERE date >= ?
      )
    `).bind(fourteenDaysAgo, sevenDaysAgo, sevenDaysAgo).first();

    // Growth metrics
    const growthMetrics = await env.DB.prepare(`
      SELECT
        (SELECT COUNT(*) FROM customers WHERE created_at >= ?) as new_users_7d,
        (SELECT COUNT(*) FROM customers WHERE created_at >= ? AND created_at < ?) as new_users_prev_7d,
        (SELECT COUNT(*) FROM licenses WHERE status = 'active' AND tier != 'free' AND created_at >= ?) as new_paid_7d
    `).bind(sevenDaysAgo, fourteenDaysAgo, sevenDaysAgo, sevenDaysAgo).first();

    const newUsers7d = Number(growthMetrics?.new_users_7d) || 0;
    const newUsersPrev7d = Number(growthMetrics?.new_users_prev_7d) || 1;
    const userGrowthRate = ((newUsers7d - newUsersPrev7d) / newUsersPrev7d * 100);

    // Time saved metrics (aggregate across all users)
    const timeSavedMetrics = await env.DB.prepare(`
      SELECT
        SUM(time_saved_ms) as total_time_saved_ms,
        AVG(time_saved_ms) as avg_time_saved_ms,
        MAX(time_saved_ms) as max_time_saved_ms
      FROM usage_daily
      WHERE date >= ?
    `).bind(thirtyDaysAgo).first();

    const totalTimeSavedMs = Number(timeSavedMetrics?.total_time_saved_ms) || 0;
    const avgTimeSavedMs = Number(timeSavedMetrics?.avg_time_saved_ms) || 0;
    const maxTimeSavedMs = Number(timeSavedMetrics?.max_time_saved_ms) || 0;

    // Time saved trend
    const timeSavedTrend = await env.DB.prepare(`
      SELECT date, SUM(time_saved_ms) as time_saved
      FROM usage_daily
      WHERE date >= ?
      GROUP BY date
      ORDER BY date ASC
    `).bind(thirtyDaysAgo).all();

    // Achievement distribution
    const achievementStats = await env.DB.prepare(`
      SELECT
        achievement_id,
        COUNT(*) as unlock_count,
        MIN(unlocked_at) as first_unlock,
        MAX(unlocked_at) as latest_unlock
      FROM achievements
      WHERE unlocked_at >= ?
      GROUP BY achievement_id
      ORDER BY unlock_count DESC
    `).bind(thirtyDaysAgo).all();

    // Command success rates (calculate from events)
    const commandSuccessData = await env.DB.prepare(`
      SELECT
        json_extract(properties, '$.command') as command,
        SUM(CASE WHEN json_extract(properties, '$.success') = 'true' THEN 1 ELSE 0 END) as success_count,
        SUM(CASE WHEN json_extract(properties, '$.success') = 'false' THEN 1 ELSE 0 END) as failure_count,
        COUNT(*) as total_count,
        AVG(CAST(duration_ms as REAL)) as avg_duration_ms
      FROM analytics_events
      WHERE event_type = 'command' AND created_at >= datetime('now', '-7 days')
      GROUP BY command
      ORDER BY total_count DESC
      LIMIT 20
    `).all();

    const commandHealth = (commandSuccessData.results || []).map((row: any) => ({
      command: row.command,
      success_count: Number(row.success_count) || 0,
      failure_count: Number(row.failure_count) || 0,
      total_count: Number(row.total_count) || 0,
      success_rate: row.total_count > 0 ? ((row.success_count / row.total_count) * 100).toFixed(1) : '0',
      avg_duration_ms: Math.round(Number(row.avg_duration_ms) || 0),
    }));

    // Feature adoption timeline (first use per feature)
    const featureAdoption = await env.DB.prepare(`
      SELECT
        json_extract(properties, '$.feature') as feature,
        MIN(created_at) as first_used_at,
        COUNT(DISTINCT machine_id) as unique_users,
        COUNT(*) as total_uses
      FROM analytics_events
      WHERE event_type = 'feature' AND created_at >= datetime('now', '-90 days')
      GROUP BY feature
      ORDER BY first_used_at ASC
    `).all();

    // Streak analytics (from usage_daily)
    const streakStats = await env.DB.prepare(`
      SELECT
        l.id as license_id,
        c.email,
        MAX(ud.date) as last_active,
        COUNT(DISTINCT ud.date) as active_days,
        SUM(ud.commands_run) as total_commands
      FROM licenses l
      JOIN customers c ON l.customer_id = c.id
      LEFT JOIN usage_daily ud ON l.id = ud.license_id AND ud.date >= ?
      WHERE l.status = 'active'
      GROUP BY l.id, c.email
      HAVING total_commands > 0
      ORDER BY active_days DESC
      LIMIT 100
    `).bind(thirtyDaysAgo).all();

    // Top errors
    const topErrors = await env.DB.prepare(`
      SELECT error_message, occurrences, last_occurred_at
      FROM analytics_errors
      ORDER BY occurrences DESC
      LIMIT 10
    `).all();

    // Top packages
    const topPackages = await env.DB.prepare(`
      SELECT package_name, install_count, search_count
      FROM analytics_packages
      ORDER BY install_count DESC, search_count DESC
      LIMIT 10
    `).all();

    // Regional performance
    const regionalPerf = await env.DB.prepare(`
      SELECT region, operation, avg_duration_ms, count
      FROM analytics_regional_perf
      ORDER BY count DESC
      LIMIT 20
    `).all();

    return jsonResponse({
      dau: Number(dauResult?.count) || 0,
      wau: Number(wauResult?.count) || 0,
      mau: Number(mauResult?.count) || 0,
      events_today: Number(eventsToday?.count) || 0,
      retention_rate: Math.round(retentionRate * 10) / 10,
      commands_by_type: commandsByType.results || [],
      features_by_usage: featuresByUsage.results || [],
      errors_by_type: errorsByType.results || [],
      dau_trend: dauTrend.results || [],
      commands_trend: commandsTrend.results || [],
      sessions_trend: sessionsTrend.results || [],
      performance: performancePercentiles,
      version_distribution: versionDist.results || [],
      platform_distribution: platformDist.results || [],
      // New gold-tier analytics
      funnel: {
        installs: Number(funnelData?.installs) || 0,
        activated: Number(funnelData?.activated) || 0,
        first_command: Number(funnelData?.first_command) || 0,
        engaged_7d: Number(funnelData?.engaged_7d) || 0,
        power_users: Number(funnelData?.power_users) || 0,
      },
      cohorts: cohortData.results || [],
      stage_distribution: stageDistribution.results || [],
      geo_distribution: geoDistribution.results || [],
      churn_risk: {
        at_risk_users: Number(churnRisk?.at_risk_users) || 0,
      },
      growth: {
        new_users_7d: newUsers7d,
        new_users_prev_7d: newUsersPrev7d,
        growth_rate: Math.round(userGrowthRate * 10) / 10,
        new_paid_7d: Number(growthMetrics?.new_paid_7d) || 0,
      },
      // Enhanced metrics for more informative dashboards
      time_saved: {
        total_ms: totalTimeSavedMs,
        total_hours: Math.round(totalTimeSavedMs / (1000 * 60 * 60) * 10) / 10,
        avg_per_user_ms: avgTimeSavedMs,
        avg_per_user_hours: Math.round(avgTimeSavedMs / (1000 * 60 * 60) * 10) / 10,
        max_user_ms: maxTimeSavedMs,
        max_user_hours: Math.round(maxTimeSavedMs / (1000 * 60 * 60) * 10) / 10,
        trend: timeSavedTrend.results || [],
      },
      achievements: {
        distribution: achievementStats.results || [],
        total_unlocks: (achievementStats.results || []).reduce((sum: number, a: any) => sum + Number(a.unlock_count), 0),
        unique_achievements: (achievementStats.results || []).length,
      },
      command_health: commandHealth,
      feature_adoption: featureAdoption.results || [],
      user_engagement: {
        active_users: (streakStats.results || []).length,
        avg_active_days: (streakStats.results || []).reduce((sum: number, s: any) => sum + Number(s.active_days), 0) / ((streakStats.results || []).length || 1),
        top_users: (streakStats.results || []).slice(0, 10),
      },
      top_errors: topErrors.results || [],
      top_packages: topPackages.results || [],
      regional_performance: regionalPerf.results || [],
    });
  } catch (error) {
    console.error('Admin analytics error:', error);
    return errorResponse('Failed to fetch analytics', 500);
  }
}
