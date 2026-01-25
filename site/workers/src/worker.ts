// OMG API Worker - Main Entry Point
// Clean, modular architecture with authenticated endpoints

import { Env, corsHeaders, jsonResponse, errorResponse, sendEmail } from './api';
import {
  handleSendCode,
  handleVerifyCode,
  handleVerifySession,
  handleLogout,
} from './handlers/auth';
import {
  handleGetDashboard,
  handleUpdateProfile,
  handleRegenerateLicense,
  handleRevokeMachine,
  handleGetSessions,
  handleRevokeSession,
  handleGetAuditLog,
  handleGetTeamMembers,
  handleRevokeTeamMember,
  handleGetTeamPolicies,
  handleGetNotifications,
} from './handlers/dashboard';
import {
  handleValidateLicense,
  handleGetLicense,
  handleReportUsage,
  handleInstallPing,
  handleAnalytics,
} from './handlers/license';
import {
  handleAdminDashboard,
  handleAdminUsers,
  handleAdminUserDetail,
  handleAdminUpdateUser,
  handleAdminActivity,
  handleAdminHealth,
  handleAdminCohorts,
  handleAdminRevenue,
  handleAdminExportUsers,
  handleAdminExportUsage,
  handleAdminExportAudit,
  handleAdminAuditLog,
} from './handlers/admin';
import { handleGetSmartInsights } from './handlers/insights';
import { handleGetFirehose } from './handlers/firehose';

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    const url = new URL(request.url);
    const path = url.pathname;

    try {
      // ============================================
      // Public endpoints (no auth required)
      // ============================================

      // Health check
      if (path === '/health') {
        return jsonResponse({ status: 'ok', timestamp: new Date().toISOString() });
      }

      // Auth: Send OTP code
      if (path === '/api/auth/send-code' && request.method === 'POST') {
        return handleSendCode(request, env);
      }

      // Auth: Verify OTP code
      if (path === '/api/auth/verify-code' && request.method === 'POST') {
        return handleVerifyCode(request, env);
      }

      // Auth: Verify session token
      if (path === '/api/auth/verify-session' && request.method === 'POST') {
        return handleVerifySession(request, env);
      }

      // Auth: Logout
      if (path === '/api/auth/logout' && request.method === 'POST') {
        return handleLogout(request, env);
      }

      // License: Validate (for CLI activation)
      if (path === '/api/validate-license' && request.method === 'GET') {
        return handleValidateLicense(request, env);
      }

      // License: Get by email (for pre-auth lookup)
      if (path === '/api/get-license' && request.method === 'GET') {
        return handleGetLicense(request, env);
      }

      // License: Report usage (from CLI)
      if (path === '/api/report-usage' && request.method === 'POST') {
        return handleReportUsage(request, env);
      }

      // Install ping (anonymous telemetry)
      if (path === '/api/install-ping' && request.method === 'POST') {
        return handleInstallPing(request, env);
      }

      // Analytics events (batch from CLI)
      if (path === '/api/analytics' && request.method === 'POST') {
        return handleAnalytics(request, env);
      }

      // ============================================
      // Authenticated endpoints (require Bearer token)
      // ============================================

      // Dashboard: Get all dashboard data
      if (path === '/api/dashboard' && request.method === 'GET') {
        return handleGetDashboard(request, env);
      }

      // User: Update profile
      if (path === '/api/user/profile' && request.method === 'PUT') {
        return handleUpdateProfile(request, env);
      }

      // License: Regenerate key
      if (path === '/api/license/regenerate' && request.method === 'POST') {
        return handleRegenerateLicense(request, env);
      }

      // Machine: Revoke
      if (path === '/api/machines/revoke' && request.method === 'POST') {
        return handleRevokeMachine(request, env);
      }

      // Sessions: List
      if (path === '/api/sessions' && request.method === 'GET') {
        return handleGetSessions(request, env);
      }

      // Sessions: Revoke
      if (path === '/api/sessions/revoke' && request.method === 'POST') {
        return handleRevokeSession(request, env);
      }

      // Audit: Get log (Team+ only)
      if (path === '/api/audit-log' && request.method === 'GET') {
        return handleGetAuditLog(request, env);
      }

      // Team: Get members and usage (Team+ only)
      if (path === '/api/team/members' && request.method === 'GET') {
        return handleGetTeamMembers(request, env);
      }

      // Fleet: Status (Alias for team members, used by dashboard)
      if (path === '/api/fleet/status' && request.method === 'GET') {
        return handleGetTeamMembers(request, env);
      }

      // Team: Analytics (Alias for dashboard data for now)
      if (path === '/api/team/analytics' && request.method === 'GET') {
        return handleGetDashboard(request, env);
      }

      // Team: Policies (Placeholder)
      if (path === '/api/team/policies' && request.method === 'GET') {
        return handleGetTeamPolicies(request, env);
      }

      // Team: Notifications (Placeholder)
      if (path === '/api/team/notifications' && request.method === 'GET') {
        return handleGetNotifications(request, env);
      }

      // Team: Audit Logs (Alias)
      if (path === '/api/team/audit-logs' && request.method === 'GET') {
        return handleGetAuditLog(request, env);
      }

      // Team: Revoke member access (Team+ only)
      if (path === '/api/team/revoke' && request.method === 'POST') {
        return handleRevokeTeamMember(request, env);
      }

      // ============================================
      // Admin endpoints (require admin validation)
      // ============================================

      // Admin: Dashboard overview
      if (path === '/api/admin/dashboard' && request.method === 'GET') {
        return handleAdminDashboard(request, env);
      }

      // Admin: List users
      if (path === '/api/admin/users' && request.method === 'GET') {
        return handleAdminUsers(request, env);
      }

      // Admin: User detail
      if (path === '/api/admin/user' && request.method === 'GET') {
        return handleAdminUserDetail(request, env);
      }

      // Admin: Update user
      if (path === '/api/admin/user' && request.method === 'PUT') {
        return handleAdminUpdateUser(request, env);
      }

      // Admin: Activity feed
      if (path === '/api/admin/activity' && request.method === 'GET') {
        return handleAdminActivity(request, env);
      }

      // Admin: Health metrics
      if (path === '/api/admin/health' && request.method === 'GET') {
        return handleAdminHealth(request, env);
      }

      // Admin: Cohort analysis
      if (path === '/api/admin/cohorts' && request.method === 'GET') {
        return handleAdminCohorts(request, env);
      }

      // Admin: Revenue analytics
      if (path === '/api/admin/revenue' && request.method === 'GET') {
        return handleAdminRevenue(request, env);
      }

      // Admin: Analytics (comprehensive telemetry)
      if (path === '/api/admin/analytics' && request.method === 'GET') {
        return handleGetAdminAnalytics(request, env);
      }

      // Admin: Export users (CSV)
      if (path === '/api/admin/export/users' && request.method === 'GET') {
        return handleAdminExportUsers(request, env);
      }

      // Admin: Export usage (JSON)
      if (path === '/api/admin/export/usage' && request.method === 'GET') {
        return handleAdminExportUsage(request, env);
      }

      // Admin: Export audit log (JSON)
      if (path === '/api/admin/export/audit' && request.method === 'GET') {
        return handleAdminExportAudit(request, env);
      }

      // Admin: View audit log
      if (path === '/api/admin/audit-log' && request.method === 'GET') {
        return handleAdminAuditLog(request, env);
      }

      // Admin: Real-time event firehose
      if (path === '/api/admin/firehose' && request.method === 'GET') {
        return handleGetFirehose(request, env);
      }

      // Insights: AI-powered recommendations
      if (path === '/api/insights' && request.method === 'GET') {
        return handleGetSmartInsights(request, env);
      }

      // ============================================
      // Stripe webhooks
      // ============================================
      if (path === '/api/stripe/webhook' && request.method === 'POST') {
        return handleStripeWebhook(request, env);
      }

      // Billing portal
      if (path === '/api/billing/portal' && request.method === 'POST') {
        return handleBillingPortal(request, env);
      }

      // Create checkout
      if (path === '/api/billing/checkout' && request.method === 'POST') {
        return handleCreateCheckout(request, env);
      }

      // ============================================
      // Database init (one-time setup)
      // ============================================
      if (path === '/api/init-db' && request.method === 'POST') {
        return handleInitDb(env);
      }

      return errorResponse('Not found', 404);
    } catch (error) {
      console.error('Worker error:', error);
      return errorResponse('Internal server error', 500);
    }
  },
};

// Stripe webhook handler
async function handleStripeWebhook(request: Request, env: Env): Promise<Response> {
  const signature = request.headers.get('stripe-signature');
  if (!signature) {
    return errorResponse('Missing stripe-signature', 400);
  }

  try {
    const body = await request.text();
    // In production, you'd verify the signature here with env.STRIPE_WEBHOOK_SECRET
    // For now, we'll parse and handle the events
    const event = JSON.parse(body) as { type: string; data: { object: any } };

    switch (event.type) {
      case 'checkout.session.completed': {
        const session = event.data.object;
        const customerEmail = session.customer_details?.email?.toLowerCase();
        const priceId = session.line_items?.data?.[0]?.price?.id;

        if (customerEmail) {
          // Map priceId to tier
          let tier: 'pro' | 'team' | 'enterprise' = 'pro';
          if (priceId === env.STRIPE_TEAM_PRICE_ID) tier = 'team';
          if (priceId === env.STRIPE_ENT_PRICE_ID) tier = 'enterprise';

          // Update user license
          await env.DB.prepare(`
            UPDATE licenses 
            SET tier = ?, status = 'active', updated_at = CURRENT_TIMESTAMP
            WHERE user_id = (SELECT id FROM users WHERE email = ?)
          `).bind(tier, customerEmail).run();

          // Create subscription record
          await env.DB.prepare(`
            INSERT INTO subscriptions (id, user_id, stripe_subscription_id, status, created_at)
            VALUES (?, (SELECT id FROM users WHERE email = ?), ?, 'active', CURRENT_TIMESTAMP)
            ON CONFLICT(stripe_subscription_id) DO UPDATE SET status = 'active'
          `).bind(crypto.randomUUID(), customerEmail, session.subscription).run();

          // Send "Gold Standard" Welcome Email
          await sendEmail(
            env, 
            customerEmail, 
            `Welcome to OMG ${tier.toUpperCase()}!`, 
            `
            <div style="font-family: sans-serif; max-width: 600px; margin: auto; padding: 20px; border: 1px solid #eee; border-radius: 10px;">
              <h2 style="color: #4f46e5;">You're now a Pro, ${customerEmail}!</h2>
              <p>Your OMG account has been upgraded to the <strong>${tier}</strong> tier.</p>
              <p>You can now manage your fleet and view your productivity ROI in your <a href="https://pyro1121.com/dashboard">Dashboard</a>.</p>
              <hr style="border: 0; border-top: 1px solid #eee; margin: 20px 0;" />
              <p style="font-size: 12px; color: #666;">OMG Package Manager - The world's fastest way to manage your stack.</p>
            </div>
            `
          );
        }
        break;
      }

      case 'customer.subscription.deleted': {
        const subscription = event.data.object;
        await env.DB.prepare(`
          UPDATE licenses 
          SET tier = 'free', updated_at = CURRENT_TIMESTAMP
          WHERE user_id = (SELECT user_id FROM subscriptions WHERE stripe_subscription_id = ?)
        `).bind(subscription.id).run();

        await env.DB.prepare(`
          UPDATE subscriptions SET status = 'cancelled' WHERE stripe_subscription_id = ?
        `).bind(subscription.id).run();
        break;
      }

      case 'invoice.paid': {
        const invoice = event.data.object;
        await env.DB.prepare(`
          INSERT INTO invoices (id, user_id, stripe_invoice_id, amount_cents, currency, status, invoice_url, created_at)
          VALUES (?, (SELECT id FROM users WHERE stripe_customer_id = ?), ?, ?, ?, 'paid', ?, CURRENT_TIMESTAMP)
        `).bind(
          crypto.randomUUID(),
          invoice.customer,
          invoice.id,
          invoice.amount_paid,
          invoice.currency,
          invoice.hosted_invoice_url
        ).run();
        break;
      }
    }

    return jsonResponse({ received: true });
  } catch (e) {
    console.error('Webhook error:', e);
    return errorResponse('Webhook handler failed', 500);
  }
}

// Billing portal handler
async function handleBillingPortal(request: Request, env: Env): Promise<Response> {
  const body = (await request.json()) as { email?: string };

  if (!body.email) {
    return errorResponse('Email required');
  }

  // Find user's Stripe customer ID
  const user = await env.DB.prepare(
    `
    SELECT stripe_customer_id FROM users WHERE email = ?
  `
  )
    .bind(body.email.toLowerCase())
    .first();

  if (!user?.stripe_customer_id) {
    return errorResponse('No billing account found');
  }

  // Create Stripe billing portal session
  const response = await fetch('https://api.stripe.com/v1/billing_portal/sessions', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.STRIPE_SECRET_KEY}`,
      'Content-Type': 'application/x-www-form-urlencoded',
    },
    body: new URLSearchParams({
      customer: user.stripe_customer_id as string,
      return_url: 'https://pyro1121.com/dashboard',
    }),
  });

  const session = (await response.json()) as { url?: string; error?: { message: string } };

  if (session.error) {
    return errorResponse(session.error.message);
  }

  return jsonResponse({ success: true, url: session.url });
}

// Create checkout session
async function handleCreateCheckout(request: Request, env: Env): Promise<Response> {
  const body = (await request.json()) as { email?: string; priceId?: string };

  if (!body.email || !body.priceId) {
    return errorResponse('Email and priceId required');
  }

  // Find or create Stripe customer
  let user = await env.DB.prepare(
    `
    SELECT id, stripe_customer_id FROM users WHERE email = ?
  `
  )
    .bind(body.email.toLowerCase())
    .first();

  let customerId = user?.stripe_customer_id as string | null;

  if (!customerId) {
    // Create Stripe customer
    const customerResponse = await fetch('https://api.stripe.com/v1/customers', {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${env.STRIPE_SECRET_KEY}`,
        'Content-Type': 'application/x-www-form-urlencoded',
      },
      body: new URLSearchParams({ email: body.email }),
    });

    const customer = (await customerResponse.json()) as { id: string };
    customerId = customer.id;

    // Update user with Stripe customer ID
    if (user) {
      await env.DB.prepare(
        `
        UPDATE users SET stripe_customer_id = ? WHERE id = ?
      `
      )
        .bind(customerId, user.id)
        .run();
    }
  }

  // Create checkout session
  const checkoutResponse = await fetch('https://api.stripe.com/v1/checkout/sessions', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.STRIPE_SECRET_KEY}`,
      'Content-Type': 'application/x-www-form-urlencoded',
    },
    body: new URLSearchParams({
      customer: customerId,
      'line_items[0][price]': body.priceId,
      'line_items[0][quantity]': '1',
      mode: 'subscription',
      success_url: 'https://pyro1121.com/dashboard?success=true',
      cancel_url: 'https://pyro1121.com/dashboard?cancelled=true',
    }),
  });

  const session = (await checkoutResponse.json()) as { url?: string; error?: { message: string } };

  if (session.error) {
    return errorResponse(session.error.message);
  }

  return jsonResponse({ url: session.url });
}

// Database initialization
async function handleInitDb(env: Env): Promise<Response> {
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
