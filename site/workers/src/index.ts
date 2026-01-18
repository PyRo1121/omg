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
  sub: string;        // customer_id
  tier: string;       // license tier
  features: string[]; // enabled features
  exp: number;        // expiration timestamp
  iat: number;        // issued at
  mid?: string;       // machine_id (optional binding)
  lic: string;        // license_key for reference
}

// Base64URL encode/decode helpers
function base64UrlEncode(data: string): string {
  return btoa(data).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function base64UrlDecode(data: string): string {
  const padded = data + '==='.slice(0, (4 - data.length % 4) % 4);
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
  'free': 1,
  'pro': 1,
  'team': 25,       // Team tier: max 25 users
  'enterprise': 999, // Unlimited for enterprise
};

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

      // Health check
      if (url.pathname === '/health') {
        return new Response(JSON.stringify({ status: 'ok', timestamp: new Date().toISOString() }), {
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        });
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

  const license = await env.DB.prepare(`
    SELECT l.*, c.email, c.company, c.id as customer_id
    FROM licenses l 
    JOIN customers c ON l.customer_id = c.id 
    WHERE l.license_key = ? 
      AND l.status = 'active'
      AND (l.expires_at IS NULL OR l.expires_at > datetime('now'))
  `).bind(key).first();

  if (!license) {
    const response: LicenseResponse = { valid: false, error: 'Invalid or expired license' };
    return new Response(JSON.stringify(response), {
      status: 401,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  const tier = license.tier as string;
  const maxSeats = license.max_seats as number || getMaxSeatsForTier(tier);
  const usedSeats = license.used_seats as number || 0;

  // For Team tier: check if this machine is already registered or if we have seats available
  if (machineId) {
    // Check if this machine is already registered for this license
    const existingMachine = await env.DB.prepare(`
      SELECT * FROM usage 
      WHERE license_key = ? AND machine_id = ? 
      LIMIT 1
    `).bind(key, machineId).first();

    if (!existingMachine) {
      // New machine - check seat limit
      if (usedSeats >= maxSeats) {
        const response: LicenseResponse = { 
          valid: false, 
          error: `Seat limit reached (${usedSeats}/${maxSeats}). Upgrade to add more users or contact support.` 
        };
        return new Response(JSON.stringify(response), {
          status: 403,
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        });
      }

      // Register this machine and increment seat count
      await env.DB.prepare(`
        UPDATE licenses SET used_seats = used_seats + 1 WHERE license_key = ?
      `).bind(key).run();
    }
  }

  // Check machine binding for Pro tier (single machine only)
  const boundMachineId = license.machine_id as string | null;
  if (tier === 'pro' && boundMachineId && machineId && boundMachineId !== machineId) {
    const response: LicenseResponse = { 
      valid: false, 
      error: 'Pro license is bound to a different machine. Upgrade to Team for multiple users.' 
    };
    return new Response(JSON.stringify(response), {
      status: 403,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Bind machine on first use for Pro tier
  if (tier === 'pro' && !boundMachineId && machineId) {
    await env.DB.prepare(`
      UPDATE licenses SET machine_id = ? WHERE license_key = ?
    `).bind(machineId, key).run();
  }

  // Log usage
  await env.DB.prepare(`
    INSERT INTO usage (id, license_key, feature, timestamp, machine_id)
    VALUES (?, ?, 'validation', datetime('now'), ?)
  `).bind(crypto.randomUUID(), key, machineId || null).run();

  const features = getFeaturesForTier(tier);
  const customerId = license.customer_id as string;

  // Calculate expiration: 7 days for token, or license expiry (whichever is sooner)
  const now = Math.floor(Date.now() / 1000);
  const tokenExpiry = now + (7 * 24 * 60 * 60); // 7 days
  const licenseExpiry = license.expires_at 
    ? Math.floor(new Date(license.expires_at as string).getTime() / 1000)
    : tokenExpiry + (365 * 24 * 60 * 60); // 1 year if no expiry
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
    customer: license.company as string || license.email as string,
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
  const body = await request.json() as { email?: string; priceId?: string };
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
      'Authorization': `Bearer ${env.STRIPE_SECRET_KEY}`,
      'Content-Type': 'application/x-www-form-urlencoded',
    },
    body: new URLSearchParams({
      'mode': 'subscription',
      'customer_email': email,
      'line_items[0][price]': priceId,
      'line_items[0][quantity]': '1',
      'success_url': 'https://pyro1121.com/?success=true',
      'cancel_url': 'https://pyro1121.com/#pricing',
    }),
  });

  const session = await stripeResponse.json() as { id?: string; url?: string; error?: { message: string } };

  if (session.error) {
    return new Response(JSON.stringify({ error: session.error.message }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  if (!session.url) {
    return new Response(JSON.stringify({ error: 'Failed to create checkout session', details: session }), {
      status: 500,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
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
      let customer = await env.DB.prepare(
        'SELECT * FROM customers WHERE stripe_customer_id = ?'
      ).bind(customerId).first();

      if (!customer) {
        // Fetch customer email from Stripe
        const stripeCustomer = await fetch(`https://api.stripe.com/v1/customers/${customerId}`, {
          headers: { 'Authorization': `Bearer ${env.STRIPE_SECRET_KEY}` },
        }).then(r => r.json()) as { email: string };

        const newCustomerId = crypto.randomUUID();
        await env.DB.prepare(`
          INSERT INTO customers (id, stripe_customer_id, email, tier)
          VALUES (?, ?, ?, 'pro')
        `).bind(newCustomerId, customerId, stripeCustomer.email).run();

        customer = { id: newCustomerId, email: stripeCustomer.email };
      }

      // Update or create subscription
      await env.DB.prepare(`
        INSERT OR REPLACE INTO subscriptions (id, customer_id, stripe_subscription_id, status, current_period_end)
        VALUES (?, ?, ?, ?, datetime(?, 'unixepoch'))
      `).bind(
        crypto.randomUUID(),
        customer.id,
        subscription.id,
        status,
        subscription.current_period_end
      ).run();

      // Create license if active
      if (status === 'active') {
        const existingLicense = await env.DB.prepare(
          'SELECT * FROM licenses WHERE customer_id = ?'
        ).bind(customer.id).first();

        if (!existingLicense) {
          const licenseKey = crypto.randomUUID();
          await env.DB.prepare(`
            INSERT INTO licenses (id, customer_id, license_key, tier, expires_at)
            VALUES (?, ?, ?, 'pro', datetime(?, 'unixepoch'))
          `).bind(
            crypto.randomUUID(),
            customer.id,
            licenseKey,
            subscription.current_period_end
          ).run();

          // TODO: Send license key email to customer
          console.log(`License created for ${customer.email}: ${licenseKey}`);
        } else {
          // Update expiry
          await env.DB.prepare(`
            UPDATE licenses SET expires_at = datetime(?, 'unixepoch'), status = 'active'
            WHERE customer_id = ?
          `).bind(subscription.current_period_end, customer.id).run();
        }
      }
      break;
    }

    case 'customer.subscription.deleted': {
      const subscription = event.data.object;
      const customerId = subscription.customer;

      const customer = await env.DB.prepare(
        'SELECT * FROM customers WHERE stripe_customer_id = ?'
      ).bind(customerId).first();

      if (customer) {
        // Deactivate license
        await env.DB.prepare(`
          UPDATE licenses SET status = 'cancelled' WHERE customer_id = ?
        `).bind(customer.id).run();

        // Update customer tier
        await env.DB.prepare(`
          UPDATE customers SET tier = 'free' WHERE id = ?
        `).bind(customer.id).run();
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
  const customer = await env.DB.prepare(`
    SELECT c.*, l.license_key, l.tier, l.expires_at, l.status as license_status
    FROM customers c
    LEFT JOIN licenses l ON c.id = l.customer_id
    WHERE c.email = ?
    ORDER BY l.created_at DESC
    LIMIT 1
  `).bind(email).first();

  if (!customer || !customer.license_key) {
    return new Response(JSON.stringify({ 
      found: false, 
      message: 'No license found. It may take a moment to process after checkout.' 
    }), {
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  return new Response(JSON.stringify({
    found: true,
    license_key: customer.license_key,
    tier: customer.tier,
    expires_at: customer.expires_at,
    status: customer.license_status,
  }), {
    headers: { 'Content-Type': 'application/json', ...corsHeaders },
  });
}

// Refresh license token (get new JWT without changing the license key)
async function handleRefreshLicense(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = await request.json() as { license_key?: string };
  const { license_key } = body;

  if (!license_key) {
    return new Response(JSON.stringify({ success: false, error: 'Missing license_key' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Find the license
  const license = await env.DB.prepare(`
    SELECT l.*, c.email, c.company, c.id as customer_id
    FROM licenses l 
    JOIN customers c ON l.customer_id = c.id 
    WHERE l.license_key = ? AND l.status = 'active'
  `).bind(license_key).first();

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
  const tokenExpiry = now + (7 * 24 * 60 * 60); // 7 days

  const jwtPayload: JWTPayload = {
    sub: customerId,
    tier,
    features,
    exp: tokenExpiry,
    iat: now,
    lic: license_key,
  };

  const token = await createJWT(jwtPayload, env.JWT_SECRET);

  return new Response(JSON.stringify({
    success: true,
    license: {
      license_key,
      tier,
      expires_at: license.expires_at,
      status: license.status,
    },
    token,
  }), {
    headers: { 'Content-Type': 'application/json', ...corsHeaders },
  });
}

// Regenerate license key (creates new key, invalidates old one)
async function handleRegenerateLicense(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = await request.json() as { email?: string; old_license_key?: string };
  const { email, old_license_key } = body;

  if (!email || !old_license_key) {
    return new Response(JSON.stringify({ success: false, error: 'Missing email or old_license_key' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Verify the old license belongs to this email
  const license = await env.DB.prepare(`
    SELECT l.*, c.email, c.id as customer_id
    FROM licenses l 
    JOIN customers c ON l.customer_id = c.id 
    WHERE l.license_key = ? AND c.email = ?
  `).bind(old_license_key, email).first();

  if (!license) {
    return new Response(JSON.stringify({ success: false, error: 'License not found or email mismatch' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Generate new license key
  const newLicenseKey = crypto.randomUUID();

  // Update the license with new key and reset machine binding
  await env.DB.prepare(`
    UPDATE licenses 
    SET license_key = ?, machine_id = NULL, used_seats = 0
    WHERE license_key = ?
  `).bind(newLicenseKey, old_license_key).run();

  // Log the regeneration
  await env.DB.prepare(`
    INSERT INTO usage (id, license_key, feature, timestamp)
    VALUES (?, ?, 'key_regenerated', datetime('now'))
  `).bind(crypto.randomUUID(), newLicenseKey).run();

  return new Response(JSON.stringify({
    success: true,
    new_license_key: newLicenseKey,
    message: 'License key regenerated. All machines will need to re-activate.',
  }), {
    headers: { 'Content-Type': 'application/json', ...corsHeaders },
  });
}

// Revoke machine access (for Team tier seat management)
async function handleRevokeMachine(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = await request.json() as { license_key?: string; machine_id?: string };
  const { license_key, machine_id } = body;

  if (!license_key || !machine_id) {
    return new Response(JSON.stringify({ success: false, error: 'Missing license_key or machine_id' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Verify the license exists
  const license = await env.DB.prepare(`
    SELECT * FROM licenses WHERE license_key = ? AND status = 'active'
  `).bind(license_key).first();

  if (!license) {
    return new Response(JSON.stringify({ success: false, error: 'Invalid license' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // For Pro tier, clear the machine binding
  if (license.tier === 'pro') {
    await env.DB.prepare(`
      UPDATE licenses SET machine_id = NULL WHERE license_key = ?
    `).bind(license_key).run();
  }

  // Delete usage records for this machine and decrement seat count
  const deleted = await env.DB.prepare(`
    DELETE FROM usage WHERE license_key = ? AND machine_id = ?
  `).bind(license_key, machine_id).run();

  if (deleted.meta.changes > 0) {
    await env.DB.prepare(`
      UPDATE licenses SET used_seats = MAX(0, used_seats - 1) WHERE license_key = ?
    `).bind(license_key).run();
  }

  return new Response(JSON.stringify({
    success: true,
    message: 'Machine access revoked',
  }), {
    headers: { 'Content-Type': 'application/json', ...corsHeaders },
  });
}

// Create Stripe Customer Portal session for subscription management
async function handleBillingPortal(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  const body = await request.json() as { email?: string };
  const { email } = body;

  if (!email) {
    return new Response(JSON.stringify({ success: false, error: 'Missing email' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Find customer's Stripe customer ID
  const customer = await env.DB.prepare(`
    SELECT stripe_customer_id FROM customers WHERE email = ?
  `).bind(email).first();

  if (!customer || !customer.stripe_customer_id) {
    return new Response(JSON.stringify({ success: false, error: 'No billing account found for this email' }), {
      status: 404,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  // Create Stripe Customer Portal session
  const portalResponse = await fetch('https://api.stripe.com/v1/billing_portal/sessions', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${env.STRIPE_SECRET_KEY}`,
      'Content-Type': 'application/x-www-form-urlencoded',
    },
    body: new URLSearchParams({
      'customer': customer.stripe_customer_id as string,
      'return_url': 'https://pyro1121.com/?portal=closed',
    }),
  });

  const session = await portalResponse.json() as { url?: string; error?: { message: string } };

  if (session.error || !session.url) {
    return new Response(JSON.stringify({ 
      success: false, 
      error: session.error?.message || 'Failed to create portal session' 
    }), {
      status: 400,
      headers: { 'Content-Type': 'application/json', ...corsHeaders },
    });
  }

  return new Response(JSON.stringify({
    success: true,
    url: session.url,
  }), {
    headers: { 'Content-Type': 'application/json', ...corsHeaders },
  });
}
