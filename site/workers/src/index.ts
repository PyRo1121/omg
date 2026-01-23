/**
 * OMG SaaS - Cloudflare Workers Backend
 *
 * Handles:
 * - License validation with signed JWTs
 * - Stripe webhooks for subscription management
 * - Checkout session creation
 * - Machine-bound license tokens
 */

export interface Env {
  DB: D1Database;
  STRIPE_SECRET_KEY: string;
  STRIPE_WEBHOOK_SECRET: string;
  JWT_SECRET: string; // HMAC-SHA256 secret for signing JWTs
  RESEND_API_KEY?: string; // For sending OTP emails
}

interface LicenseResponse {
  valid: boolean;
  tier?: string;
  features?: string[];
  customer?: string;
  expires_at?: string;
  token?: string; // Signed JWT for offline validation
  error?: string;
}

interface JWTPayload {
  sub: string; // customer_id
  tier: string; // license tier
  features: string[]; // enabled features
  exp: number; // expiration timestamp
  iat: number; // issued at
  mid?: string; // machine_id (optional binding)
  lic: string; // license_key for reference
}

// Base64URL encode/decode helpers
function base64UrlEncode(data: string): string {
  return btoa(data).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function base64UrlDecode(data: string): string {
  const padded = data + '==='.slice(0, (4 - (data.length % 4)) % 4);
  return atob(padded.replace(/-/g, '+').replace(/_/g, '/'));
}

// HMAC-SHA256 signing
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
  return base64UrlEncode(String.fromCharCode(...new Uint8Array(signature)));
}

// Create a signed JWT
async function createJWT(payload: JWTPayload, secret: string): Promise<string> {
  const header = { alg: 'HS256', typ: 'JWT' };
  const headerB64 = base64UrlEncode(JSON.stringify(header));
  const payloadB64 = base64UrlEncode(JSON.stringify(payload));
  const signature = await hmacSign(secret, `${headerB64}.${payloadB64}`);
  return `${headerB64}.${payloadB64}.${signature}`;
}

// Verify and decode a JWT
async function verifyJWT(token: string, secret: string): Promise<JWTPayload | null> {
  try {
    const parts = token.split('.');
    if (parts.length !== 3) return null;

    const [headerB64, payloadB64, signature] = parts;
    const expectedSig = await hmacSign(secret, `${headerB64}.${payloadB64}`);

    if (signature !== expectedSig) return null;

    const payload = JSON.parse(base64UrlDecode(payloadB64)) as JWTPayload;

    // Check expiration
    if (payload.exp && payload.exp < Math.floor(Date.now() / 1000)) {
      return null;
    }

    return payload;
  } catch {
    return null;
  }
}

// Feature definitions by tier
const FREE_FEATURES = ['packages', 'runtimes', 'container', 'env-capture', 'env-share'];
const PRO_FEATURES = ['sbom', 'audit', 'secrets'];
const TEAM_FEATURES = ['team-sync', 'team-config', 'audit-log'];
const ENTERPRISE_FEATURES = ['policy', 'slsa', 'sso', 'priority-support'];

// Seat limits by tier
const TIER_SEAT_LIMITS: Record<string, number> = {
  free: 1,
  pro: 1,
  team: 25, // Team tier: max 25 users
  enterprise: 999, // Unlimited for enterprise
};

// Helper to send emails via Resend
export async function sendEmail(env: Env, to: string, subject: string, html: string): Promise<boolean> {
  if (!env.RESEND_API_KEY) {
    console.error('RESEND_API_KEY not configured');
    return false;
  }

  try {
    const res = await fetch('https://api.resend.com/emails', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${env.RESEND_API_KEY}`,
      },
      body: JSON.stringify({
        from: 'OMG Package Manager <no-reply@pyro1121.com>',
        to: [to],
        subject,
        html,
      }),
    });

    return res.ok;
  } catch (e) {
    console.error('Failed to send email:', e);
    return false;
  }
}

// Get max seats for a tier
function getMaxSeatsForTier(tier: string): number {
  return TIER_SEAT_LIMITS[tier] || 1;
}

// Get features for a tier (includes all lower tiers)
function getFeaturesForTier(tier: string): string[] {
  const features = [...FREE_FEATURES];
  if (['pro', 'team', 'enterprise'].includes(tier)) {
    features.push(...PRO_FEATURES);
  }
  if (['team', 'enterprise'].includes(tier)) {
    features.push(...TEAM_FEATURES);
  }
  if (tier === 'enterprise') {
    features.push(...ENTERPRISE_FEATURES);
  }
  return features;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    };

    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    try {
      // License validation endpoint
      if (url.pathname === '/api/validate-license') {
        return await handleValidateLicense(request, env, corsHeaders);
      }

      // Create checkout session
      if (url.pathname === '/api/create-checkout' && request.method === 'POST') {
        return await handleCreateCheckout(request, env, corsHeaders);
      }

      // Stripe webhook
      if (url.pathname === '/webhook/stripe' && request.method === 'POST') {
        return await handleStripeWebhook(request, env);
      }

      // Get license by email (for post-checkout)
      if (url.pathname === '/api/get-license' && request.method === 'GET') {
        return await handleGetLicense(request, env, corsHeaders);
      }

      // Refresh license token (get new JWT without changing key)
      if (url.pathname === '/api/refresh-license' && request.method === 'POST') {
        return await handleRefreshLicense(request, env, corsHeaders);
      }

      // Regenerate license key (new key, invalidates old one)
      if (url.pathname === '/api/regenerate-license' && request.method === 'POST') {
        return await handleRegenerateLicense(request, env, corsHeaders);
      }

      // Revoke machine access
      if (url.pathname === '/api/revoke-machine' && request.method === 'POST') {
        return await handleRevokeMachine(request, env, corsHeaders);
      }

      // Create Stripe Customer Portal session (for upgrade/downgrade/cancel)
      if (url.pathname === '/api/billing-portal' && request.method === 'POST') {
        return await handleBillingPortal(request, env, corsHeaders);
      }

      // Register free account
      if (url.pathname === '/api/register-free' && request.method === 'POST') {
        return await handleRegisterFree(request, env, corsHeaders);
      }

      // Send OTP code to email
      if (url.pathname === '/api/auth/send-code' && request.method === 'POST') {
        return await handleSendCode(request, env, corsHeaders);
      }

      // Verify OTP code and create session
      if (url.pathname === '/api/auth/verify-code' && request.method === 'POST') {
        return await handleVerifyCode(request, env, corsHeaders);
      }

      // Verify session token
      if (url.pathname === '/api/auth/verify-session' && request.method === 'POST') {
        return await handleVerifySession(request, env, corsHeaders);
      }

      // Install telemetry ping
      if (url.pathname === '/api/install-ping' && request.method === 'POST') {
        return await handleInstallPing(request, env, corsHeaders);
      }

      // Analytics events (batch)
      if (url.pathname === '/api/analytics' && request.method === 'POST') {
        return await handleAnalytics(request, env, corsHeaders);
      }

      // Report usage (legacy, still supported)
      if (url.pathname === '/api/report-usage' && request.method === 'POST') {
        return await handleReportUsage(request, env, corsHeaders);
      }

      // Install badge endpoint (shields.io format)
      if (url.pathname === '/api/badge/installs' && request.method === 'GET') {
        return await handleInstallsBadge(env, corsHeaders);
      }

      // Health check
      if (url.pathname === '/health') {
        return new Response(JSON.stringify({ status: 'ok', timestamp: new Date().toISOString() }), {
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        });
      }

      // Database init endpoint (one-time setup)
      if (url.pathname === '/api/init-db' && request.method === 'POST') {
        return await handleInitDb(request, env, corsHeaders);
      }

      // Get license members (Team/Enterprise)
      if (url.pathname === '/api/license/members' && request.method === 'GET') {
        return await handleGetLicenseMembers(request, env, corsHeaders);
      }

      // Get license policies (Enterprise)
      if (url.pathname === '/api/license/policies' && request.method === 'GET') {
        return await handleGetLicensePolicies(request, env, corsHeaders);
      }

      // Get license audit logs (Team/Enterprise)
      if (url.pathname === '/api/license/audit' && request.method === 'GET') {
        return await handleGetLicenseAudit(request, env, corsHeaders);
      }

      // Team Proposals (Team/Enterprise)
      if (url.pathname === '/api/team/propose' && request.method === 'POST') {
        return await handleTeamPropose(request, env, corsHeaders);
      }

      if (url.pathname === '/api/team/review' && request.method === 'POST') {
        return await handleTeamReview(request, env, corsHeaders);
      }

      if (url.pathname === '/api/team/proposals' && request.method === 'GET') {
        return await handleGetTeamProposals(request, env, corsHeaders);
      }

      return new Response('Not found', { status: 404, headers: corsHeaders });
    } catch (error) {
      console.error('Error:', error);
      return new Response(JSON.stringify({ error: 'Internal server error' }), {
        status: 500,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }
  },
};

async function handleGetLicenseMembers(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const url = new URL(request.url);
  const key = url.searchParams.get('key');

  if (!key) {
    return new Response(JSON.stringify({ error: 'Missing license key' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Find the license
  const license = await env.DB.prepare(
    `SELECT id, tier FROM licenses WHERE license_key = ? AND status = 'active'`
  )
    .bind(key)
    .first();

  if (!license) {
    return new Response(JSON.stringify({ error: 'Invalid or inactive license' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Only Team and Enterprise can see all members
  if (!['team', 'enterprise'].includes(license.tier as string)) {
    return new Response(
      JSON.stringify({ error: 'Fleet features require Team or Enterprise tier' }),
      { status: 403, headers: { 'Content-Type': 'application/json', ...corsHeaders } }
    );
  }

  // Get all machines for this license
  const machines = await env.DB.prepare(
    `SELECT machine_id, hostname, os, arch, omg_version, last_seen_at, is_active FROM machines WHERE license_id = ?`
  )
    .bind(license.id)
    .all();

  return new Response(JSON.stringify(machines.results || []), {
    headers: { 'Content-Type': 'application/json', ...corsHeaders },
  });
}

async function handleGetLicensePolicies(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const url = new URL(request.url);
  const key = url.searchParams.get('key');

  if (!key) {
    return new Response(JSON.stringify({ error: 'Missing license key' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  const license = await env.DB.prepare(
    `SELECT id, tier FROM licenses WHERE license_key = ? AND status = 'active'`
  )
    .bind(key)
    .first();

  if (!license) {
    return new Response(JSON.stringify({ error: 'Invalid or inactive license' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  if (license.tier !== 'enterprise') {
    return new Response(JSON.stringify({ error: 'Policy features require Enterprise tier' }), {
      status: 403,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

    const policies = await env.DB.prepare(`SELECT scope, rule, enforced FROM policies WHERE license_id = ?`)

      .bind(license.id)

      .all();

  

    return new Response(JSON.stringify(policies.results || []), {

      headers: { 'Content-Type': 'application/json', ...corsHeaders },

    });

  }

  

  async function handleGetLicenseAudit(

    request: Request,

    env: Env,

    corsHeaders: Record<string, string>

  ): Promise<Response> {

    const url = new URL(request.url);

    const key = url.searchParams.get('key');

  

    if (!key) {

      return new Response(JSON.stringify({ error: 'Missing license key' }), {

        status: 400,

        headers: { 'Content-Type': 'application/json', ...corsHeaders },

      });

    }

  

    const license = await env.DB.prepare(

      `SELECT id, customer_id, tier FROM licenses WHERE license_key = ? AND status = 'active'`

    )

      .bind(key)

      .first();

  

    if (!license) {

      return new Response(JSON.stringify({ error: 'Invalid or inactive license' }), {

        status: 401,

        headers: { 'Content-Type': 'application/json', ...corsHeaders },

      });

    }

  

    if (!['team', 'enterprise'].includes(license.tier as string)) {

      return new Response(JSON.stringify({ error: 'Audit logs require Team or Enterprise tier' }), {

        status: 403,

        headers: { 'Content-Type': 'application/json', ...corsHeaders },

      });

    }

  

    const logs = await env.DB.prepare(

      `SELECT action, resource_type, resource_id, ip_address, created_at FROM audit_log WHERE user_id = ? ORDER BY created_at DESC LIMIT 50`

    )

      .bind(license.customer_id)

      .all();

  

    return new Response(JSON.stringify(logs.results || []), {

      headers: { 'Content-Type': 'application/json', ...corsHeaders },

    });

  }

  

async function handleValidateLicense(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const url = new URL(request.url);
  const key = url.searchParams.get('key');
  const machineId = url.searchParams.get('machine_id'); // Optional machine binding

  if (!key) {
    const response: LicenseResponse = { valid: false, error: 'Missing license key' };
    return new Response(JSON.stringify(response), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  const license = await env.DB.prepare(
    `
    SELECT l.*, c.email, c.company, c.id as customer_id
    FROM licenses l 
    JOIN customers c ON l.customer_id = c.id 
    WHERE l.license_key = ? 
      AND l.status = 'active'
      AND (l.expires_at IS NULL OR l.expires_at > datetime('now'))
  `
  )
    .bind(key)
    .first();

  if (!license) {
    const response: LicenseResponse = { valid: false, error: 'Invalid or expired license' };
    return new Response(JSON.stringify(response), {
      status: 401,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  const tier = license.tier as string;
  const maxSeats = (license.max_seats as number) || getMaxSeatsForTier(tier);
  const usedSeats = (license.used_seats as number) || 0;

  // For Team tier: check if this machine is already registered or if we have seats available
  if (machineId) {
    // Check if this machine is already registered for this license
    const existingMachine = await env.DB.prepare(
      `
      SELECT * FROM usage 
      WHERE license_key = ? AND machine_id = ? 
      LIMIT 1
    `
    )
      .bind(key, machineId)
      .first();

    if (!existingMachine) {
      // New machine - check seat limit
      if (usedSeats >= maxSeats) {
        const response: LicenseResponse = {
          valid: false,
          error: `Seat limit reached (${usedSeats}/${maxSeats}). Upgrade to add more users or contact support.`,
        };
        return new Response(JSON.stringify(response), {
          status: 403,
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        });
      }

      // Register this machine and increment seat count
      await env.DB.prepare(
        `
        UPDATE licenses SET used_seats = used_seats + 1 WHERE license_key = ?
      `
      )
        .bind(key)
        .run();
    }
  }

  // Check machine binding for Pro tier (single machine only)
  const boundMachineId = license.machine_id as string | null;
  if (tier === 'pro' && boundMachineId && machineId && boundMachineId !== machineId) {
    const response: LicenseResponse = {
      valid: false,
      error: 'Pro license is bound to a different machine. Upgrade to Team for multiple users.',
    };
    return new Response(JSON.stringify(response), {
      status: 403,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Bind machine on first use for Pro tier
  if (tier === 'pro' && !boundMachineId && machineId) {
    await env.DB.prepare(
      `
      UPDATE licenses SET machine_id = ? WHERE license_key = ?
    `
    )
      .bind(machineId, key)
      .run();
  }

  // Log usage
  await env.DB.prepare(
    `
    INSERT INTO usage (id, license_key, feature, timestamp, machine_id)
    VALUES (?, ?, 'validation', datetime('now'), ?)
  `
  )
    .bind(crypto.randomUUID(), key, machineId || null)
    .run();

  const features = getFeaturesForTier(tier);
  const customerId = license.customer_id as string;

  // Calculate expiration: 7 days for token, or license expiry (whichever is sooner)
  const now = Math.floor(Date.now() / 1000);
  const tokenExpiry = now + 7 * 24 * 60 * 60; // 7 days
  const licenseExpiry = license.expires_at
    ? Math.floor(new Date(license.expires_at as string).getTime() / 1000)
    : tokenExpiry + 365 * 24 * 60 * 60; // 1 year if no expiry
  const exp = Math.min(tokenExpiry, licenseExpiry);

  // Create signed JWT for offline validation
  const jwtPayload: JWTPayload = {
    sub: customerId,
    tier,
    features,
    exp,
    iat: now,
    mid: machineId || undefined,
    lic: key,
  };

  const token = await createJWT(jwtPayload, env.JWT_SECRET);

  const response: LicenseResponse = {
    valid: true,
    tier,
    features,
    customer: (license.company as string) || (license.email as string),
    expires_at: license.expires_at as string,
    token, // Signed JWT for offline validation
  };

  return new Response(JSON.stringify(response), {
    headers: { 'Content-Type': 'application/json', ...corsHeaders },
  });
}

async function handleCreateCheckout(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = (await request.json()) as { email?: string; priceId?: string };
  const { email, priceId } = body;

  if (!email || !priceId) {
    return new Response(JSON.stringify({ error: 'Missing email or priceId' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Create Stripe checkout session
  const stripeResponse = await fetch('https://api.stripe.com/v1/checkout/sessions', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.STRIPE_SECRET_KEY}`,
      'Content-Type': 'application/x-www-form-urlencoded',
    },
    body: new URLSearchParams({
      mode: 'subscription',
      customer_email: email,
      'line_items[0][price]': priceId,
      'line_items[0][quantity]': '1',
      success_url: 'https://pyro1121.com/?success=true',
      cancel_url: 'https://pyro1121.com/#pricing',
    }),
  });

  const session = (await stripeResponse.json()) as {
    id?: string;
    url?: string;
    error?: { message: string };
  };

  if (session.error) {
    return new Response(JSON.stringify({ error: session.error.message }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  if (!session.url) {
    return new Response(
      JSON.stringify({ error: 'Failed to create checkout session', details: session }),
      {
        status: 500,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }

  return new Response(JSON.stringify({ sessionId: session.id, url: session.url }), {
    headers: { 'Content-Type': 'application/json', ...corsHeaders },
  });
}

async function handleStripeWebhook(request: Request, env: Env): Promise<Response> {
  const body = await request.text();
  const signature = request.headers.get('stripe-signature');

  // In production, verify the webhook signature
  // For now, we'll parse the event directly
  let event;
  try {
    event = JSON.parse(body);
  } catch {
    return new Response('Invalid JSON', { status: 400 });
  }

  switch (event.type) {
    case 'customer.subscription.created':
    case 'customer.subscription.updated': {
      const subscription = event.data.object;
      const customerId = subscription.customer;
      const status = subscription.status;

      // Get or create customer
      let customer = await env.DB.prepare('SELECT * FROM customers WHERE stripe_customer_id = ?')
        .bind(customerId)
        .first();

      if (!customer) {
        // Fetch customer email from Stripe
        const stripeCustomer = (await fetch(`https://api.stripe.com/v1/customers/${customerId}`, {
          headers: { Authorization: `Bearer ${env.STRIPE_SECRET_KEY}` },
        }).then(r => r.json())) as { email: string };

        const newCustomerId = crypto.randomUUID();
        await env.DB.prepare(
          `
          INSERT INTO customers (id, stripe_customer_id, email, tier)
          VALUES (?, ?, ?, 'pro')
        `
        )
          .bind(newCustomerId, customerId, stripeCustomer.email)
          .run();

        customer = { id: newCustomerId, email: stripeCustomer.email };
      }

      // Update or create subscription
      await env.DB.prepare(
        `
        INSERT OR REPLACE INTO subscriptions (id, customer_id, stripe_subscription_id, status, current_period_end)
        VALUES (?, ?, ?, ?, datetime(?, 'unixepoch'))
      `
      )
        .bind(
          crypto.randomUUID(),
          customer.id,
          subscription.id,
          status,
          subscription.current_period_end
        )
        .run();

      // Create license if active
      if (status === 'active') {
        const existingLicense = await env.DB.prepare('SELECT * FROM licenses WHERE customer_id = ?')
          .bind(customer.id)
          .first();

        if (!existingLicense) {
          const licenseKey = crypto.randomUUID();
          await env.DB.prepare(
            `
            INSERT INTO licenses (id, customer_id, license_key, tier, expires_at)
            VALUES (?, ?, ?, 'pro', datetime(?, 'unixepoch'))
          `
          )
            .bind(crypto.randomUUID(), customer.id, licenseKey, subscription.current_period_end)
            .run();

          // Send license key email to customer
          await sendEmail(
            env,
            customer.email,
            'Your OMG License Key',
            `
            <h1>Welcome to OMG!</h1>
            <p>Your ${subscription.plan.nickname || 'Pro'} license has been activated.</p>
            <p><strong>License Key:</strong> <code>${licenseKey}</code></p>
            <p>Activate it on your machine with:</p>
            <pre>omg license activate ${licenseKey}</pre>
            <p>Visit your <a href="https://pyro1121.com/dashboard">dashboard</a> to manage your machines.</p>
          `
          );
          console.log(`License created and emailed for ${customer.email}: ${licenseKey}`);
        } else {
          // Update expiry
          await env.DB.prepare(
            `
            UPDATE licenses SET expires_at = datetime(?, 'unixepoch'), status = 'active'
            WHERE customer_id = ?
          `
          )
            .bind(subscription.current_period_end, customer.id)
            .run();
        }
      }
      break;
    }

    case 'customer.subscription.deleted': {
      const subscription = event.data.object;
      const customerId = subscription.customer;

      const customer = await env.DB.prepare('SELECT * FROM customers WHERE stripe_customer_id = ?')
        .bind(customerId)
        .first();

      if (customer) {
        // Deactivate license
        await env.DB.prepare(
          `
          UPDATE licenses SET status = 'cancelled' WHERE customer_id = ?
        `
        )
          .bind(customer.id)
          .run();

        // Update customer tier
        await env.DB.prepare(
          `
          UPDATE customers SET tier = 'free' WHERE id = ?
        `
        )
          .bind(customer.id)
          .run();
      }
      break;
    }
  }

  return new Response('OK');
}

async function handleGetLicense(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const url = new URL(request.url);
  const email = url.searchParams.get('email');

  if (!email) {
    return new Response(JSON.stringify({ error: 'Missing email parameter' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Find customer by email
  const customer = await env.DB.prepare(
    `
    SELECT c.*, l.license_key, l.tier, l.expires_at, l.status as license_status
    FROM customers c
    LEFT JOIN licenses l ON c.id = l.customer_id
    WHERE c.email = ?
    ORDER BY l.created_at DESC
    LIMIT 1
  `
  )
    .bind(email)
    .first();

  if (!customer || !customer.license_key) {
    return new Response(
      JSON.stringify({
        found: false,
        message: 'No license found. It may take a moment to process after checkout.',
      }),
      {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }

  return new Response(
    JSON.stringify({
      found: true,
      license_key: customer.license_key,
      tier: customer.tier,
      expires_at: customer.expires_at,
      status: customer.license_status,
    }),
    {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    }
  );
}

// Refresh license token (get new JWT without changing the license key)
async function handleRefreshLicense(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = (await request.json()) as { license_key?: string };
  const { license_key } = body;

  if (!license_key) {
    return new Response(JSON.stringify({ success: false, error: 'Missing license_key' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Find the license
  const license = await env.DB.prepare(
    `
    SELECT l.*, c.email, c.company, c.id as customer_id
    FROM licenses l 
    JOIN customers c ON l.customer_id = c.id 
    WHERE l.license_key = ? AND l.status = 'active'
  `
  )
    .bind(license_key)
    .first();

  if (!license) {
    return new Response(JSON.stringify({ success: false, error: 'Invalid or inactive license' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  const tier = license.tier as string;
  const features = getFeaturesForTier(tier);
  const customerId = license.customer_id as string;

  // Create new JWT token
  const now = Math.floor(Date.now() / 1000);
  const tokenExpiry = now + 7 * 24 * 60 * 60; // 7 days

  const jwtPayload: JWTPayload = {
    sub: customerId,
    tier,
    features,
    exp: tokenExpiry,
    iat: now,
    lic: license_key,
  };

  const token = await createJWT(jwtPayload, env.JWT_SECRET);

  return new Response(
    JSON.stringify({
      success: true,
      license: {
        license_key,
        tier,
        expires_at: license.expires_at,
        status: license.status,
      },
      token,
    }),
    {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    }
  );
}

// Regenerate license key (creates new key, invalidates old one)
async function handleRegenerateLicense(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = (await request.json()) as { email?: string; old_license_key?: string };
  const { email, old_license_key } = body;

  if (!email || !old_license_key) {
    return new Response(
      JSON.stringify({ success: false, error: 'Missing email or old_license_key' }),
      {
        status: 400,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }

  // Verify the old license belongs to this email
  const license = await env.DB.prepare(
    `
    SELECT l.*, c.email, c.id as customer_id
    FROM licenses l 
    JOIN customers c ON l.customer_id = c.id 
    WHERE l.license_key = ? AND c.email = ?
  `
  )
    .bind(old_license_key, email)
    .first();

  if (!license) {
    return new Response(
      JSON.stringify({ success: false, error: 'License not found or email mismatch' }),
      {
        status: 401,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }

  // Generate new license key
  const newLicenseKey = crypto.randomUUID();

  // Update the license with new key and reset machine binding
  await env.DB.prepare(
    `
    UPDATE licenses 
    SET license_key = ?, machine_id = NULL, used_seats = 0
    WHERE license_key = ?
  `
  )
    .bind(newLicenseKey, old_license_key)
    .run();

  // Log the regeneration
  await env.DB.prepare(
    `
    INSERT INTO usage (id, license_key, feature, timestamp)
    VALUES (?, ?, 'key_regenerated', datetime('now'))
  `
  )
    .bind(crypto.randomUUID(), newLicenseKey)
    .run();

  return new Response(
    JSON.stringify({
      success: true,
      new_license_key: newLicenseKey,
      message: 'License key regenerated. All machines will need to re-activate.',
    }),
    {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    }
  );
}

// Revoke machine access (for Team tier seat management)
async function handleRevokeMachine(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = (await request.json()) as { license_key?: string; machine_id?: string };
  const { license_key, machine_id } = body;

  if (!license_key || !machine_id) {
    return new Response(
      JSON.stringify({ success: false, error: 'Missing license_key or machine_id' }),
      {
        status: 400,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }

  // Verify the license exists
  const license = await env.DB.prepare(
    `
    SELECT * FROM licenses WHERE license_key = ? AND status = 'active'
  `
  )
    .bind(license_key)
    .first();

  if (!license) {
    return new Response(JSON.stringify({ success: false, error: 'Invalid license' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // For Pro tier, clear the machine binding
  if (license.tier === 'pro') {
    await env.DB.prepare(
      `
      UPDATE licenses SET machine_id = NULL WHERE license_key = ?
    `
    )
      .bind(license_key)
      .run();
  }

  // Delete usage records for this machine and decrement seat count
  const deleted = await env.DB.prepare(
    `
    DELETE FROM usage WHERE license_key = ? AND machine_id = ?
  `
  )
    .bind(license_key, machine_id)
    .run();

  if (deleted.meta.changes > 0) {
    await env.DB.prepare(
      `
      UPDATE licenses SET used_seats = MAX(0, used_seats - 1) WHERE license_key = ?
    `
    )
      .bind(license_key)
      .run();
  }

  return new Response(
    JSON.stringify({
      success: true,
      message: 'Machine access revoked',
    }),
    {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    }
  );
}

// Create Stripe Customer Portal session for subscription management
async function handleBillingPortal(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = (await request.json()) as { email?: string };
  const { email } = body;

  if (!email) {
    return new Response(JSON.stringify({ success: false, error: 'Missing email' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Find customer's Stripe customer ID
  const customer = await env.DB.prepare(
    `
    SELECT stripe_customer_id FROM customers WHERE email = ?
  `
  )
    .bind(email)
    .first();

  if (!customer || !customer.stripe_customer_id) {
    return new Response(
      JSON.stringify({ success: false, error: 'No billing account found for this email' }),
      {
        status: 404,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }

  // Create Stripe Customer Portal session
  const portalResponse = await fetch('https://api.stripe.com/v1/billing_portal/sessions', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.STRIPE_SECRET_KEY}`,
      'Content-Type': 'application/x-www-form-urlencoded',
    },
    body: new URLSearchParams({
      customer: customer.stripe_customer_id as string,
      return_url: 'https://pyro1121.com/?portal=closed',
    }),
  });

  const session = (await portalResponse.json()) as { url?: string; error?: { message: string } };

  if (session.error || !session.url) {
    return new Response(
      JSON.stringify({
        success: false,
        error: session.error?.message || 'Failed to create portal session',
      }),
      {
        status: 400,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }

  return new Response(
    JSON.stringify({
      success: true,
      url: session.url,
    }),
    {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    }
  );
}

// Handle free account registration
async function handleRegisterFree(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = (await request.json()) as { email?: string };
  const { email } = body;

  if (!email) {
    return new Response(JSON.stringify({ success: false, error: 'Missing email' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Check if customer already exists
  let customer = await env.DB.prepare(
    `
    SELECT c.*, l.license_key, l.tier, l.status as license_status
    FROM customers c
    LEFT JOIN licenses l ON c.id = l.customer_id
    WHERE c.email = ?
    LIMIT 1
  `
  )
    .bind(email)
    .first();

  if (customer && customer.license_key) {
    // Already registered - return existing license
    return new Response(
      JSON.stringify({
        success: true,
        license_key: customer.license_key,
        tier: customer.tier || 'free',
        already_registered: true,
        usage: {
          time_saved_ms: 0,
          total_commands: 0,
          current_streak: 0,
          achievements: [],
        },
      }),
      {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }

  // Create new free customer
  const customerId = crypto.randomUUID();
  const licenseKey = `free-${crypto.randomUUID()}`;

  try {
    // Create customer
    await env.DB.prepare(
      `
      INSERT INTO customers (id, email, tier, created_at)
      VALUES (?, ?, 'free', datetime('now'))
    `
    )
      .bind(customerId, email)
      .run();

    // Create free license (never expires)
    await env.DB.prepare(
      `
      INSERT INTO licenses (id, customer_id, license_key, tier, status, expires_at, created_at)
      VALUES (?, ?, ?, 'free', 'active', NULL, datetime('now'))
    `
    )
      .bind(crypto.randomUUID(), customerId, licenseKey)
      .run();

    return new Response(
      JSON.stringify({
        success: true,
        license_key: licenseKey,
        tier: 'free',
        usage: {
          time_saved_ms: 0,
          total_commands: 0,
          current_streak: 0,
          achievements: [],
        },
      }),
      {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  } catch (e) {
    console.error('Registration error:', e);
    return new Response(
      JSON.stringify({ success: false, error: 'Registration failed. Please try again.' }),
      {
        status: 500,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }
}

// Handle install telemetry ping
async function handleInstallPing(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    const body = (await request.json()) as {
      install_id?: string;
      timestamp?: string;
      version?: string;
      platform?: string;
      backend?: string;
    };

    const { install_id, timestamp, version, platform, backend } = body;

    if (!install_id || !timestamp || !version) {
      return new Response(JSON.stringify({ success: false, error: 'Missing required fields' }), {
        status: 400,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }

    // Store install details
    await env.DB.prepare(
      `
      INSERT OR IGNORE INTO install_details (id, timestamp, version, platform, backend)
      VALUES (?, ?, ?, ?, ?)
    `
    )
      .bind(install_id, timestamp, version, platform || 'unknown', backend || 'unknown')
      .run();

    // Increment daily counter
    const today = new Date().toISOString().split('T')[0];
    await env.DB.prepare(
      `
      INSERT INTO install_stats (id, date, count)
      VALUES (?, ?, 1)
      ON CONFLICT(date) DO UPDATE SET count = count + 1
    `
    )
      .bind(`day-${today}`, today)
      .run();

    return new Response(JSON.stringify({ success: true }), {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  } catch (e) {
    console.error('Install ping error:', e);
    return new Response(JSON.stringify({ success: false, error: 'Internal error' }), {
      status: 500,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }
}

// Handle install badge endpoint (shields.io format)
async function handleInstallsBadge(
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    // Get total install count
    const result = await env.DB.prepare(
      `
      SELECT COUNT(DISTINCT id) as total FROM install_details
    `
    ).first();

    const total = (result?.total as number) || 0;

    // Return shields.io endpoint JSON format
    return new Response(
      JSON.stringify({
        schemaVersion: 1,
        label: 'installs',
        message: total.toLocaleString(),
        color: 'blue',
      }),
      {
        headers: {
          'Content-Type': 'application/json',
          'Cache-Control': 'public, max-age=300', // Cache for 5 minutes
          ...corsHeaders,
        },
      }
    );
  } catch (e) {
    console.error('Badge error:', e);
    return new Response(
      JSON.stringify({
        schemaVersion: 1,
        label: 'installs',
        message: 'error',
        color: 'red',
      }),
      {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }
}

// Generate a 6-digit OTP code
function generateOTP(): string {
  return Math.floor(100000 + Math.random() * 900000).toString();
}

// Generate a secure session token
function generateSessionToken(): string {
  const array = new Uint8Array(32);
  crypto.getRandomValues(array);
  return Array.from(array, b => b.toString(16).padStart(2, '0')).join('');
}

// Send OTP code via email
async function sendOTPEmail(
  email: string,
  code: string,
  apiKey: string
): Promise<{ success: boolean; error?: string }> {
  try {
    const response = await fetch('https://api.resend.com/emails', {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${apiKey}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        from: 'OMG <noreply@pyro1121.com>',
        to: [email],
        subject: 'Your OMG verification code',
        html: `
          <div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 480px; margin: 0 auto; padding: 40px 20px;">
            <div style="text-align: center; margin-bottom: 30px;">
              <h1 style="color: #1a1a2e; margin: 0; font-size: 28px;">🚀 OMG</h1>
              <p style="color: #666; margin: 5px 0 0;">Package Manager</p>
            </div>
            <div style="background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); border-radius: 16px; padding: 30px; text-align: center;">
              <p style="color: rgba(255,255,255,0.9); margin: 0 0 15px; font-size: 16px;">Your verification code is:</p>
              <div style="background: rgba(255,255,255,0.95); border-radius: 12px; padding: 20px; margin: 0 auto; max-width: 200px;">
                <span style="font-size: 36px; font-weight: bold; letter-spacing: 8px; color: #1a1a2e;">${code}</span>
              </div>
              <p style="color: rgba(255,255,255,0.8); margin: 20px 0 0; font-size: 14px;">This code expires in 10 minutes.</p>
            </div>
            <p style="color: #999; font-size: 13px; text-align: center; margin-top: 30px;">
              If you didn't request this code, you can safely ignore this email.
            </p>
          </div>
        `,
      }),
    });

    if (!response.ok) {
      const errorData = (await response.json()) as { message?: string; name?: string };
      console.error('Resend API error:', errorData);
      return { success: false, error: errorData.message || 'Email send failed' };
    }
    return { success: true };
  } catch (e) {
    console.error('Email send error:', e);
    return { success: false, error: e instanceof Error ? e.message : 'Unknown error' };
  }
}

// Handle sending OTP code
async function handleSendCode(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    const body = (await request.json()) as { email?: string };
    const { email } = body;

    if (!email || !email.includes('@')) {
      return new Response(JSON.stringify({ success: false, error: 'Valid email required' }), {
        status: 400,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }

    // Rate limit: max 3 codes per email per 10 minutes
    const recentCodes = await env.DB.prepare(
      `
      SELECT COUNT(*) as count FROM auth_codes 
      WHERE email = ? AND created_at > datetime('now', '-10 minutes')
    `
    )
      .bind(email.toLowerCase())
      .first();

    if ((recentCodes?.count as number) >= 3) {
      return new Response(
        JSON.stringify({
          success: false,
          error: 'Too many requests. Please wait a few minutes.',
        }),
        {
          status: 429,
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        }
      );
    }

    // Generate OTP
    const code = generateOTP();
    const expiresAt = new Date(Date.now() + 10 * 60 * 1000).toISOString(); // 10 minutes

    // Store code
    await env.DB.prepare(
      `
      INSERT INTO auth_codes (id, email, code, expires_at)
      VALUES (?, ?, ?, ?)
    `
    )
      .bind(crypto.randomUUID(), email.toLowerCase(), code, expiresAt)
      .run();

    // Send email - REQUIRED for security
    if (!env.RESEND_API_KEY) {
      console.error('RESEND_API_KEY not configured');
      return new Response(
        JSON.stringify({
          success: false,
          error: 'Email service not configured. Please contact support.',
        }),
        {
          status: 500,
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        }
      );
    }

    const result = await sendOTPEmail(email, code, env.RESEND_API_KEY);
    if (!result.success) {
      return new Response(
        JSON.stringify({
          success: false,
          error: result.error || 'Failed to send email. Please try again.',
        }),
        {
          status: 500,
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        }
      );
    }

    return new Response(
      JSON.stringify({
        success: true,
        message: 'Verification code sent to your email',
      }),
      {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  } catch (e) {
    console.error('Send code error:', e);
    return new Response(JSON.stringify({ success: false, error: 'Internal error' }), {
      status: 500,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }
}

// Handle verifying OTP code
async function handleVerifyCode(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    const body = (await request.json()) as { email?: string; code?: string };
    const { email, code } = body;

    if (!email || !code) {
      return new Response(JSON.stringify({ success: false, error: 'Email and code required' }), {
        status: 400,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }

    // Find valid code
    const authCode = await env.DB.prepare(
      `
      SELECT * FROM auth_codes 
      WHERE email = ? AND code = ? AND used = 0 AND expires_at > datetime('now')
      ORDER BY created_at DESC
      LIMIT 1
    `
    )
      .bind(email.toLowerCase(), code)
      .first();

    if (!authCode) {
      return new Response(
        JSON.stringify({
          success: false,
          error: 'Invalid or expired code',
        }),
        {
          status: 401,
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        }
      );
    }

    // Mark code as used
    await env.DB.prepare(
      `
      UPDATE auth_codes SET used = 1 WHERE id = ?
    `
    )
      .bind(authCode.id)
      .run();

    // Create session token (valid for 30 days)
    const sessionToken = generateSessionToken();
    const sessionExpires = new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString();

    await env.DB.prepare(
      `
      INSERT INTO sessions (id, email, token, expires_at)
      VALUES (?, ?, ?, ?)
    `
    )
      .bind(crypto.randomUUID(), email.toLowerCase(), sessionToken, sessionExpires)
      .run();

    // Clean up old sessions for this email (keep only last 5)
    await env.DB.prepare(
      `
      DELETE FROM sessions WHERE email = ? AND id NOT IN (
        SELECT id FROM sessions WHERE email = ? ORDER BY created_at DESC LIMIT 5
      )
    `
    )
      .bind(email.toLowerCase(), email.toLowerCase())
      .run();

    return new Response(
      JSON.stringify({
        success: true,
        token: sessionToken,
        expires_at: sessionExpires,
      }),
      {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  } catch (e) {
    console.error('Verify code error:', e);
    return new Response(JSON.stringify({ success: false, error: 'Internal error' }), {
      status: 500,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }
}

// Handle verifying session token
async function handleVerifySession(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    const body = (await request.json()) as { token?: string };
    const { token } = body;

    if (!token) {
      return new Response(JSON.stringify({ valid: false, error: 'Token required' }), {
        status: 400,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }

    // Find valid session
    const session = await env.DB.prepare(
      `
      SELECT * FROM sessions 
      WHERE token = ? AND expires_at > datetime('now')
      LIMIT 1
    `
    )
      .bind(token)
      .first();

    if (!session) {
      return new Response(JSON.stringify({ valid: false, error: 'Invalid or expired session' }), {
        status: 401,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }

    return new Response(
      JSON.stringify({
        valid: true,
        email: session.email,
        expires_at: session.expires_at,
      }),
      {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  } catch (e) {
    console.error('Verify session error:', e);
    return new Response(JSON.stringify({ valid: false, error: 'Internal error' }), {
      status: 500,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }
}

// Handle database initialization (one-time setup)
async function handleInitDb(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    // Create auth_codes table
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

    // Create sessions table
    await env.DB.prepare(
      `
      CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        email TEXT NOT NULL,
        token TEXT UNIQUE NOT NULL,
        expires_at DATETIME NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )
    `
    ).run();

    // Create indexes
    await env.DB.prepare(
      `CREATE INDEX IF NOT EXISTS idx_auth_codes_email ON auth_codes(email)`
    ).run();
    await env.DB.prepare(`CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token)`).run();

    return new Response(
      JSON.stringify({
        success: true,
        message: 'Database tables created successfully',
      }),
      {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  } catch (e) {
    console.error('Init DB error:', e);
    return new Response(
      JSON.stringify({
        success: false,
        error: e instanceof Error ? e.message : 'Unknown error',
      }),
      {
        status: 500,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      }
    );
  }
}

// Handle analytics events (batch)
async function handleAnalytics(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    const body = await request.json() as { events?: Array<{
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
    }> };

    const events = body.events || [];
    if (events.length === 0) {
      return new Response(JSON.stringify({ success: true, processed: 0 }), {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }

    // Process events in batch
    const today = new Date().toISOString().split('T')[0];

    for (const event of events) {
      // Store event in analytics_events table
      await env.DB.prepare(`
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
      ).run();

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
      }

      if (event.event_type === 'session_start') {
        // Track unique sessions
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'sessions', 'all', 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today).run();
      }

      if (event.event_type === 'error') {
        // Track errors
        const errorType = (event.properties?.error_type as string) || 'unknown';
        await env.DB.prepare(`
          INSERT INTO analytics_daily (date, metric, dimension, value)
          VALUES (?, 'errors', ?, 1)
          ON CONFLICT(date, metric, dimension) DO UPDATE SET value = value + 1
        `).bind(today, errorType).run();
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
        // Track performance metrics (store for percentile calculation)
        await env.DB.prepare(`
          INSERT INTO analytics_performance (id, operation, duration_ms, created_at)
          VALUES (?, ?, ?, CURRENT_TIMESTAMP)
        `).bind(crypto.randomUUID(), event.event_name, event.duration_ms).run();
      }
    }

    // Track unique active machines today
    const uniqueMachines = [...new Set(events.map(e => e.machine_id))];
    for (const machineId of uniqueMachines) {
      await env.DB.prepare(`
        INSERT OR IGNORE INTO analytics_active_users (date, machine_id)
        VALUES (?, ?)
      `).bind(today, machineId).run();
    }

    return new Response(JSON.stringify({ success: true, processed: events.length }), {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  } catch (e) {
    console.error('Analytics error:', e);
    return new Response(JSON.stringify({ success: false, error: 'Failed to process analytics' }), {
      status: 500,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }
}

// Handle usage reporting (legacy endpoint, still supported)
async function handleReportUsage(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    const body = await request.json() as {
      license_key: string;
      machine_id: string;
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

    if (!body.license_key || !body.machine_id) {
      return new Response(JSON.stringify({ success: false, error: 'Missing required fields' }), {
        status: 400,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }

    // Get license
    const license = await env.DB.prepare(`
      SELECT id FROM licenses WHERE license_key = ? AND status = 'active'
    `).bind(body.license_key).first();

    if (!license) {
      return new Response(JSON.stringify({ success: false, error: 'Invalid license' }), {
        status: 401,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }

    const today = new Date().toISOString().split('T')[0];

    // Update or insert daily usage
    await env.DB.prepare(`
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
    `).bind(
      crypto.randomUUID(),
      license.id,
      today,
      body.commands_run || 0,
      body.packages_installed || 0,
      body.packages_searched || 0,
      body.runtimes_switched || 0,
      body.sbom_generated || 0,
      body.vulnerabilities_found || 0,
      body.time_saved_ms || 0
    ).run();

    // Update machine info
    await env.DB.prepare(`
      UPDATE machines SET
        hostname = COALESCE(?, hostname),
        os = COALESCE(?, os),
        arch = COALESCE(?, arch),
        omg_version = COALESCE(?, omg_version),
        last_seen_at = CURRENT_TIMESTAMP
      WHERE license_id = ? AND machine_id = ?
    `).bind(
      body.hostname || null,
      body.os || null,
      body.arch || null,
      body.omg_version || null,
      license.id,
      body.machine_id
    ).run();

    return new Response(JSON.stringify({ success: true }), {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  } catch (e) {
    console.error('Report usage error:', e);
        return new Response(JSON.stringify({ success: false, error: 'Failed to report usage' }), {
            status: 500,
            headers: { 'Content-Type': 'application/json', ...corsHeaders },
          });
      }
    }
    
    async function handleTeamPropose(
      request: Request,
      env: Env,
      corsHeaders: Record<string, string>
    ): Promise<Response> {
      const body = await request.json() as { key: string, message: string, state: any };
      const { key, message, state } = body;
    
      const license = await env.DB.prepare(
        `SELECT id, user_id, tier FROM licenses WHERE license_key = ? AND status = 'active'`
      ).bind(key).first();
    
      if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
        return new Response(JSON.stringify({ error: 'Proposals require Team or Enterprise tier' }), {
          status: 403,
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        });
      }
    
      const result = await env.DB.prepare(
        `INSERT INTO team_proposals (license_id, creator_id, message, state_json) VALUES (?, ?, ?, ?)`
      ).bind(license.id, license.user_id, message, JSON.stringify(state)).run();
    
      return new Response(JSON.stringify({ success: true, proposal_id: result.meta.last_row_id }), {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }
    
    async function handleTeamReview(
      request: Request,
      env: Env,
      corsHeaders: Record<string, string>
    ): Promise<Response> {
      const body = await request.json() as { key: string, proposal_id: number, status: string };
      const { key, proposal_id, status } = body;
    
      const license = await env.DB.prepare(
        `SELECT id, tier FROM licenses WHERE license_key = ? AND status = 'active'`
      ).bind(key).first();
    
      if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
        return new Response(JSON.stringify({ error: 'Unauthorized' }), { status: 403 });
      }
    
      await env.DB.prepare(
        `UPDATE team_proposals SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND license_id = ?`
      ).bind(status, proposal_id, license.id).run();
    
      return new Response(JSON.stringify({ success: true }), {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }
    
    async function handleGetTeamProposals(
      request: Request,
      env: Env,
      corsHeaders: Record<string, string>
    ): Promise<Response> {
      const url = new URL(request.url);
      const key = url.searchParams.get('key');
    
      const license = await env.DB.prepare(
        `SELECT id, tier FROM licenses WHERE license_key = ? AND status = 'active'`
      ).bind(key).first();
    
      if (!license || !['team', 'enterprise'].includes(license.tier as string)) {
        return new Response(JSON.stringify({ error: 'Unauthorized' }), { status: 403 });
      }
    
      const proposals = await env.DB.prepare(
        `SELECT p.*, u.email as creator_email FROM team_proposals p JOIN users u ON p.creator_id = u.id WHERE p.license_id = ? ORDER BY p.created_at DESC`
      ).bind(license.id).all();
    
      return new Response(JSON.stringify(proposals.results || []), {
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }
    
