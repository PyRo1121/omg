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
  let licenseKey: string | null = null;
  let machineId: string | null = null;
  let userName: string | null = null;
  let userEmail: string | null = null;

  if (request.method === 'POST') {
    try {
      const body = (await request.json()) as any;
      licenseKey = body.key || body.license_key;
      machineId = body.machine_id ?? null;
      userName = body.user_name ?? null;
      userEmail = body.user_email ?? null;
    } catch (e) {
      return errorResponse('Invalid JSON body');
    }
  } else {
    const url = new URL(request.url);
    licenseKey = url.searchParams.get('key');
    machineId = url.searchParams.get('machine_id');
    userName = url.searchParams.get('user_name');
    userEmail = url.searchParams.get('user_email');
  }

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

  if (machineId) {
    const existingMachine = await env.DB.prepare(
      `
      SELECT * FROM machines WHERE license_id = ? AND machine_id = ?
    `
    )
      .bind(license.id, machineId)
      .first();

    if (existingMachine) {
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

  const token = env.JWT_PRIVATE_KEY 
    ? await generateLicenseJWT(license, machineId, env.JWT_PRIVATE_KEY, 'EdDSA')
    : await generateLicenseJWT(license, machineId, env.JWT_SECRET, 'HS256');

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

  // Mask the license key for public lookup to prevent harvesting
  const maskKey = (key: string) => {
    if (key.length <= 8) return '****' + key.slice(-4);
    return key.slice(0, 4) + '••••' + key.slice(-4);
  };

  return jsonResponse({
    found: true,
    license_key: maskKey(result.license_key),
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
    installed_packages?: Record<string, number>;
    runtime_usage_counts?: Record<string, number>;
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

  // Process granular package stats
  if (body.installed_packages) {
    for (const [pkg, count] of Object.entries(body.installed_packages)) {
      await env.DB.prepare(`
        INSERT INTO analytics_packages (package_name, install_count, last_seen_at)
        VALUES (?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(package_name) DO UPDATE SET install_count = install_count + ?, last_seen_at = CURRENT_TIMESTAMP
      `).bind(pkg, count, count).run();
    }
  }

  // Process granular runtime stats
  if (body.runtime_usage_counts) {
    for (const [runtime, count] of Object.entries(body.runtime_usage_counts)) {
      await env.DB.prepare(`
        INSERT INTO analytics_daily (date, metric, dimension, value)
        VALUES (?, 'version', ?, ?)
        ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + ?
      `).bind(today, runtime, count, count).run();
    }
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
  secret: string,
  algorithm: 'HS256' | 'EdDSA' = 'HS256'
): Promise<string> {
  const header = { alg: algorithm, typ: 'JWT' };
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

  const headerB64 = base64UrlEncode(JSON.stringify(header));
  const payloadB64 = base64UrlEncode(JSON.stringify(payload));
  const data = `${headerB64}.${payloadB64}`;

  const signature = algorithm === 'EdDSA'
    ? await eddsaSign(secret, data)
    : await hmacSign(secret, data);

  return `${data}.${signature}`;
}

function base64UrlEncode(data: Uint8Array | string): string {
  if (typeof data === 'string') {
    return btoa(data).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }
  return btoa(String.fromCharCode(...data)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function base64UrlDecode(data: string): string {
  const padded = data + '==='.slice(0, (4 - (data.length % 4)) % 4);
  return atob(padded.replace(/-/g, '+').replace(/_/g, '/'));
}

async function hmacSign(secret: string, data: string): Promise<string> {
  const encoder = new TextEncoder();
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign']
  );
  const signature = await crypto.subtle.sign('HMAC', key, encoder.encode(data));
  return base64UrlEncode(new Uint8Array(signature));
}

async function eddsaSign(privateKeyDer: string, data: string): Promise<string> {
  const encoder = new TextEncoder();
  const keyData = base64UrlDecode(privateKeyDer.replace(/-----BEGIN PRIVATE KEY-----|-----END PRIVATE KEY-----|\n/g, ''));
  const keyBuffer = new Uint8Array(keyData.length);
  for (let i = 0; i < keyData.length; i++) {
    keyBuffer[i] = keyData.charCodeAt(i);
  }

  const key = await crypto.subtle.importKey(
    'pkcs8',
    keyBuffer,
    { name: 'Ed25519', namedCurve: 'Ed25519' },
    false,
    ['sign']
  );

  const signature = await crypto.subtle.sign('Ed25519', key, encoder.encode(data));
  return base64UrlEncode(new Uint8Array(signature));
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
    const statements: D1PreparedStatement[] = [];

    for (const event of events) {
      // Store event
      statements.push(env.DB.prepare(`
        INSERT INTO analytics_events (id, event_type, event_name, properties, timestamp, session_id, machine_id, license_key, version, platform, duration_ms, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
      `).bind(
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
      ));

      if (event.event_type === 'command') {
        statements.push(env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'commands', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, event.event_name));

        statements.push(env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'total_commands', 'all', 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today));

        statements.push(env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'platform', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, event.platform));

        statements.push(env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'version', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, event.version));
      }

      if (event.event_type === 'error') {
        const errorMsg = (event.properties?.message as string) || 'unknown error';
        statements.push(env.DB.prepare(`
          INSERT INTO analytics_errors (error_message, occurrences, last_occurred_at)
          VALUES (?, 1, CURRENT_TIMESTAMP)
          ON CONFLICT(error_message) DO UPDATE SET occurrences = occurrences + 1, last_occurred_at = CURRENT_TIMESTAMP
        `).bind(errorMsg));

        const errorType = (event.properties?.error_type as string) || 'unknown';
        statements.push(env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'errors', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, errorType));
      }
    }

    // Atomic batch execution
    if (statements.length > 0) {
      await env.DB.batch(statements);
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
