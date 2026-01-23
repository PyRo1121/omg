// License validation handlers (for CLI activation)
import {
  Env,
  jsonResponse,
  errorResponse,
  generateId,
  generateToken,
  logAudit,
  TIER_FEATURES,
} from '../api';

// Validate license key (called by CLI during activation)
export async function handleValidateLicense(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  const licenseKey = url.searchParams.get('key');
  const machineId = url.searchParams.get('machine_id');
  const userName = url.searchParams.get('user_name');
  const userEmail = url.searchParams.get('user_email');

  if (!licenseKey) {
    return errorResponse('License key required');
  }

  // Find license (using existing schema with customers table)
  const license = await env.DB.prepare(
    `
    SELECT l.*, c.email, c.company as customer_name
    FROM licenses l
    JOIN customers c ON l.customer_id = c.id
    WHERE l.license_key = ?
  `
  )
    .bind(licenseKey)
    .first();

  if (!license) {
    return jsonResponse({ valid: false, error: 'Invalid license key' });
  }

  // Check status
  if (license.status !== 'active') {
    return jsonResponse({ valid: false, error: `License is ${license.status}` });
  }

  // Check expiration
  if (license.expires_at) {
    const expiresAt = new Date(license.expires_at as string);
    if (expiresAt < new Date()) {
      return jsonResponse({ valid: false, error: 'License has expired' });
    }
  }

  // Handle machine registration if machine_id provided
  if (machineId) {
    // Check if machine already registered
    const existingMachine = await env.DB.prepare(
      `
      SELECT * FROM machines WHERE license_id = ? AND machine_id = ?
    `
    )
      .bind(license.id, machineId)
      .first();

    if (existingMachine) {
      // Update last seen and user info if provided
      if (userName || userEmail) {
        await env.DB.prepare(
          `
          UPDATE machines SET last_seen_at = CURRENT_TIMESTAMP, user_name = COALESCE(?, user_name), user_email = COALESCE(?, user_email) WHERE id = ?
        `
        )
          .bind(userName, userEmail, existingMachine.id)
          .run();
      } else {
        await env.DB.prepare(
          `
          UPDATE machines SET last_seen_at = CURRENT_TIMESTAMP WHERE id = ?
        `
        )
          .bind(existingMachine.id)
          .run();
      }
    } else {
      // Check machine limit
      const machineCount = await env.DB.prepare(
        `
        SELECT COUNT(*) as count FROM machines WHERE license_id = ? AND is_active = 1
      `
      )
        .bind(license.id)
        .first();

      const maxMachines = (license.max_machines as number) || 1;
      if ((machineCount?.count as number) >= maxMachines) {
        return jsonResponse({
          valid: false,
          error: `Machine limit reached (${maxMachines}). Revoke a machine in your dashboard or upgrade.`,
        });
      }

      // Register new machine with user info
      await env.DB.prepare(
        `
        INSERT INTO machines (id, license_id, machine_id, user_name, user_email, is_active)
        VALUES (?, ?, ?, ?, ?, 1)
      `
      )
        .bind(generateId(), license.id, machineId, userName, userEmail)
        .run();

      await logAudit(
        env.DB,
        license.customer_id as string,
        'machine.registered',
        'machine',
        machineId,
        request
      );
    }
  }

  // Generate JWT token for offline validation
  const token = await generateLicenseJWT(license, machineId, env.JWT_SECRET);

  const tier = license.tier as keyof typeof TIER_FEATURES;
  const tierConfig = TIER_FEATURES[tier] || TIER_FEATURES.free;

  return jsonResponse({
    valid: true,
    tier: license.tier,
    features: tierConfig.features,
    customer: license.customer_name || license.email,
    expires_at: license.expires_at,
    token,
  });
}

// Get license info by email (for dashboard lookup before auth)
export async function handleGetLicense(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  const email = url.searchParams.get('email')?.toLowerCase().trim();

  if (!email) {
    return errorResponse('Email required');
  }

  const result = await env.DB.prepare(
    `
    SELECT l.license_key, l.tier, l.status, l.expires_at, l.max_seats as max_machines
    FROM licenses l
    JOIN customers c ON l.customer_id = c.id
    WHERE c.email = ?
  `
  )
    .bind(email)
    .first();

  if (!result) {
    return jsonResponse({ found: false });
  }

  // Get machine count
  const machineCount = await env.DB.prepare(
    `
    SELECT COUNT(*) as count FROM machines m
    JOIN licenses l ON m.license_id = l.id
    JOIN customers c ON l.customer_id = c.id
    WHERE c.email = ? AND m.is_active = 1
  `
  )
    .bind(email)
    .first();

  return jsonResponse({
    found: true,
    license_key: result.license_key,
    tier: result.tier,
    status: result.status,
    expires_at: result.expires_at,
    max_machines: result.max_machines,
    used_machines: machineCount?.count || 0,
  });
}

// Report usage from CLI
export async function handleReportUsage(request: Request, env: Env): Promise<Response> {
  const body = (await request.json()) as {
    license_key?: string;
    machine_id?: string;
    hostname?: string;
    os?: string;
    arch?: string;
    omg_version?: string;
    commands_run?: number;
    packages_installed?: number;
    packages_searched?: number;
    runtimes_switched?: number;
    sbom_generated?: number;
    vulnerabilities_found?: number;
    time_saved_ms?: number;
    current_streak?: number;
    achievements?: string[];
  };

  if (!body.license_key) {
    return errorResponse('License key required');
  }

  // Find license
  const license = await env.DB.prepare(
    `
    SELECT id, customer_id FROM licenses WHERE license_key = ? AND status = 'active'
  `
  )
    .bind(body.license_key)
    .first();

  if (!license) {
    return errorResponse('Invalid license', 401);
  }

  const today = new Date().toISOString().split('T')[0];

  // Upsert daily usage
  await env.DB.prepare(
    `
    INSERT INTO usage_daily (id, license_id, date, commands_run, packages_installed, packages_searched, runtimes_switched, sbom_generated, vulnerabilities_found, time_saved_ms)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(license_id, date) DO UPDATE SET
      commands_run = commands_run + excluded.commands_run,
      packages_installed = packages_installed + excluded.packages_installed,
      packages_searched = packages_searched + excluded.packages_searched,
      runtimes_switched = runtimes_switched + excluded.runtimes_switched,
      sbom_generated = sbom_generated + excluded.sbom_generated,
      vulnerabilities_found = vulnerabilities_found + excluded.vulnerabilities_found,
      time_saved_ms = time_saved_ms + excluded.time_saved_ms
  `
  )
    .bind(
      generateId(),
      license.id,
      today,
      body.commands_run || 0,
      body.packages_installed || 0,
      body.packages_searched || 0,
      body.runtimes_switched || 0,
      body.sbom_generated || 0,
      body.vulnerabilities_found || 0,
      body.time_saved_ms || 0
    )
    .run();

  // Upsert per-member usage for team intelligence
  if (body.machine_id) {
    await env.DB.prepare(
      `
      INSERT INTO usage_member_daily (id, license_id, machine_id, date, commands_run, packages_installed, runtimes_switched, time_saved_ms)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(license_id, machine_id, date) DO UPDATE SET
        commands_run = commands_run + excluded.commands_run,
        packages_installed = packages_installed + excluded.packages_installed,
        runtimes_switched = runtimes_switched + excluded.runtimes_switched,
        time_saved_ms = time_saved_ms + excluded.time_saved_ms
    `
    )
      .bind(
        generateId(),
        license.id,
        body.machine_id,
        today,
        body.commands_run || 0,
        body.packages_installed || 0,
        body.runtimes_switched || 0,
        body.time_saved_ms || 0
      )
      .run();

    // Update machine info if provided
    await env.DB.prepare(
      `
      UPDATE machines SET 
        last_seen_at = CURRENT_TIMESTAMP,
        hostname = COALESCE(?, hostname),
        os = COALESCE(?, os),
        arch = COALESCE(?, arch),
        omg_version = COALESCE(?, omg_version)
      WHERE license_id = ? AND machine_id = ?
    `
    )
      .bind(
        body.hostname || null,
        body.os || null,
        body.arch || null,
        body.omg_version || null,
        license.id,
        body.machine_id
      )
      .run();
  }

  // Sync achievements if provided
  if (body.achievements && body.achievements.length > 0) {
    for (const achievement of body.achievements) {
      await env.DB.prepare(
        `
        INSERT OR IGNORE INTO achievements (id, customer_id, achievement_id)
        VALUES (?, ?, ?)
      `
      )
        .bind(generateId(), license.customer_id, achievement)
        .run();
    }
  }

  return jsonResponse({ success: true });
}

// Handle install ping (anonymous telemetry)
export async function handleInstallPing(request: Request, env: Env): Promise<Response> {
  const body = (await request.json()) as {
    install_id?: string;
    timestamp?: string;
    version?: string;
    platform?: string;
    backend?: string;
  };

  if (!body.install_id) {
    return errorResponse('Install ID required');
  }

  // Record install in install_stats table
  await env.DB.prepare(
    `
    INSERT OR IGNORE INTO install_stats (id, install_id, version, platform, backend, created_at)
    VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
  `
  )
    .bind(
      generateId(),
      body.install_id,
      body.version || 'unknown',
      body.platform || 'unknown',
      body.backend || 'unknown'
    )
    .run();

  return jsonResponse({ success: true, message: 'Install recorded' });
}

// Generate JWT for offline license validation
async function generateLicenseJWT(
  license: Record<string, unknown>,
  machineId: string | null,
  secret: string
): Promise<string> {
  const header = { alg: 'HS256', typ: 'JWT' };
  const now = Math.floor(Date.now() / 1000);
  const payload = {
    sub: license.customer_id,
    tier: license.tier,
    features: TIER_FEATURES[license.tier as keyof typeof TIER_FEATURES]?.features || [],
    exp: now + 7 * 24 * 60 * 60, // 7 days
    iat: now,
    mid: machineId,
    lic: license.license_key,
  };

  const encoder = new TextEncoder();
  const headerB64 = btoa(JSON.stringify(header))
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');
  const payloadB64 = btoa(JSON.stringify(payload))
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');

  const data = encoder.encode(`${headerB64}.${payloadB64}`);
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign']
  );
  const signature = await crypto.subtle.sign('HMAC', key, data);
  const signatureB64 = btoa(String.fromCharCode(...new Uint8Array(signature)))
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');

  return `${headerB64}.${payloadB64}.${signatureB64}`;
}

// Handle analytics events (batch)
export async function handleAnalytics(request: Request, env: Env): Promise<Response> {
  try {
    const body = await request.json() as {
      events?: Array<{
        event_type: string;
        event_name: string;
        properties?: Record<string, unknown>;
        timestamp: string;
        session_id: string;
        machine_id: string;
        license_key?: string;
        version: string;
        platform: string;
        duration_ms?: number;
      }>;
    };

    const events = body.events || [];
    if (events.length === 0) {
      return jsonResponse({ success: true, processed: 0 });
    }

    // Process events in batch
    const today = new Date().toISOString().split('T')[0];

    for (const event of events) {
      // Store event in analytics_events table
      await env.DB.prepare(`
        INSERT INTO analytics_events (id, event_type, event_name, properties, timestamp, session_id, machine_id, license_key, version, platform, duration_ms, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
      `)
        .bind(
          crypto.randomUUID(),
          event.event_type,
          event.event_name,
          JSON.stringify(event.properties || {}),
          event.timestamp,
          event.session_id,
          event.machine_id,
          event.license_key || null,
          event.version,
          event.platform,
          event.duration_ms || null
        )
        .run();

      // Update aggregated stats
      if (event.event_type === 'command') {
        // Update daily command stats
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'commands', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, event.event_name).run();

        // Update total commands
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'total_commands', 'all', 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today).run();

        // Update platform distribution
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'platform', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, event.platform).run();

        // Update version distribution
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'version', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, event.version).run();
        
        // Track specific packages if provided in properties
        if (event.event_name === 'install' && event.properties?.package) {
          await env.DB.prepare(`
            INSERT INTO analytics_packages (package_name, install_count, last_seen_at)
            VALUES (?, 1, CURRENT_TIMESTAMP)
            ON CONFLICT(package_name) DO UPDATE SET install_count = install_count + 1, last_seen_at = CURRENT_TIMESTAMP
          `).bind(event.properties.package).run();
        }

        if (event.event_name === 'search' && event.properties?.query) {
          await env.DB.prepare(`
            INSERT INTO analytics_packages (package_name, search_count, last_seen_at)
            VALUES (?, 1, CURRENT_TIMESTAMP)
            ON CONFLICT(package_name) DO UPDATE SET search_count = search_count + 1, last_seen_at = CURRENT_TIMESTAMP
          `).bind(event.properties.query).run();
        }
      }

      if (event.event_type === 'error') {
        const errorMsg = (event.properties?.message as string) || 'unknown error';
        await env.DB.prepare(`
          INSERT INTO analytics_errors (error_message, occurrences, last_occurred_at)
          VALUES (?, 1, CURRENT_TIMESTAMP)
          ON CONFLICT(error_message) DO UPDATE SET occurrences = occurrences + 1, last_occurred_at = CURRENT_TIMESTAMP
        `).bind(errorMsg).run();

        // Also track error in analytics_daily
        const errorType = (event.properties?.error_type as string) || 'unknown';
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'errors', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, errorType).run();
      }

      if (event.event_type === 'session_start') {
        // Track unique sessions
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'sessions', 'all', 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today).run();
      }

      if (event.event_type === 'feature') {
        // Track feature usage
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'features', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, event.event_name).run();
      }

      if (event.event_type === 'performance' && event.duration_ms) {
        // Regional performance
        const country = request.headers.get('CF-IPCountry') || 'Unknown';
        await env.DB.prepare(`
          INSERT INTO analytics_regional_perf (region, operation, avg_duration_ms, count)
          VALUES (?, ?, ?, 1)
          ON CONFLICT(region, operation) DO UPDATE SET 
            avg_duration_ms = (avg_duration_ms * count + excluded.avg_duration_ms) / (count + 1),
            count = count + 1
        `).bind(country, event.event_name, event.duration_ms).run();

        // Track performance metrics (store for percentile calculation)
        await env.DB.prepare(`
          INSERT INTO analytics_performance (id, operation, duration_ms, created_at)
          VALUES (?, ?, ?, CURRENT_TIMESTAMP)
        `)
          .bind(crypto.randomUUID(), event.event_name, event.duration_ms)
          .run();
      }
    }

    // Track unique active machines today
    const uniqueMachines = [...new Set(events.map((e) => e.machine_id))];
    for (const machineId of uniqueMachines) {
      await env.DB.prepare(`
        INSERT OR IGNORE INTO analytics_active_users (date, machine_id)
        VALUES (?, ?)
      `)
        .bind(today, machineId)
        .run();
    }

    return jsonResponse({ success: true, processed: events.length });
  } catch (e) {
    console.error('Analytics error:', e);
    return errorResponse('Failed to process analytics', 500);
  }
}
