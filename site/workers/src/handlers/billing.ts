import { Env, jsonResponse, errorResponse, validateSession, getAuthToken, logAudit } from '../api';

/**
 * Verify Stripe webhook signature using HMAC-SHA256
 * This is CRITICAL for security - prevents webhook spoofing
 */
async function verifyStripeSignature(
  payload: string,
  signature: string,
  secret: string
): Promise<boolean> {
  try {
    const encoder = new TextEncoder();

    // Parse the Stripe signature header (format: t=timestamp,v1=signature)
    const parts = signature.split(',').reduce(
      (acc, part) => {
        const [key, value] = part.split('=');
        if (key && value) acc[key] = value;
        return acc;
      },
      {} as Record<string, string>
    );

    const timestamp = parts['t'];
    const expectedSig = parts['v1'];

    if (!timestamp || !expectedSig) {
      console.error('Stripe signature missing timestamp or v1 signature');
      return false;
    }

    // Check timestamp to prevent replay attacks (5 minute tolerance)
    const timestampNum = parseInt(timestamp, 10);
    const now = Math.floor(Date.now() / 1000);
    if (Math.abs(now - timestampNum) > 300) {
      console.error('Stripe webhook timestamp too old or in future');
      return false;
    }

    // Compute expected signature: HMAC-SHA256(timestamp.payload)
    const signedPayload = `${timestamp}.${payload}`;
    const key = await crypto.subtle.importKey(
      'raw',
      encoder.encode(secret),
      { name: 'HMAC', hash: 'SHA-256' },
      false,
      ['sign']
    );

    const signatureBytes = await crypto.subtle.sign('HMAC', key, encoder.encode(signedPayload));

    // Convert to hex string
    const computedSig = Array.from(new Uint8Array(signatureBytes))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');

    // Timing-safe comparison to prevent timing attacks
    if (computedSig.length !== expectedSig.length) {
      return false;
    }

    let result = 0;
    for (let i = 0; i < computedSig.length; i++) {
      result |= computedSig.charCodeAt(i) ^ expectedSig.charCodeAt(i);
    }

    return result === 0;
  } catch (error) {
    console.error('Stripe signature verification error:', error);
    return false;
  }
}

export async function handleCreateCheckout(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const body = (await request.json()) as { email?: string; priceId?: string };
  const { email, priceId } = body;

  if (!email || !priceId) {
    return errorResponse('Missing email or priceId');
  }

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
      success_url: 'https://pyro1121.com/dashboard?success=true',
      cancel_url: 'https://pyro1121.com/#pricing',
    }),
  });

  const session = (await stripeResponse.json()) as {
    id?: string;
    url?: string;
    error?: { message: string };
  };

  if (session.error) {
    return errorResponse(session.error.message);
  }

  if (!session.url) {
    return errorResponse('Failed to create checkout session', 500);
  }

  await logAudit(
    env.DB,
    auth.user.id,
    'billing.checkout_created',
    'checkout',
    session.id,
    request,
    { priceId }
  );

  return jsonResponse({ sessionId: session.id, url: session.url });
}

export async function handleBillingPortal(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const body = (await request.json()) as { email?: string };
  const email = body.email || auth.user.email;

  const customer = await env.DB.prepare(`SELECT stripe_customer_id FROM customers WHERE email = ?`)
    .bind(email)
    .first();

  if (!customer || !customer.stripe_customer_id) {
    return errorResponse('No billing account found for this email', 404);
  }

  const portalResponse = await fetch('https://api.stripe.com/v1/billing_portal/sessions', {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.STRIPE_SECRET_KEY}`,
      'Content-Type': 'application/x-www-form-urlencoded',
    },
    body: new URLSearchParams({
      customer: customer.stripe_customer_id as string,
      return_url: 'https://pyro1121.com/dashboard?portal=closed',
    }),
  });

  const session = (await portalResponse.json()) as { url?: string; error?: { message: string } };

  if (session.error || !session.url) {
    return errorResponse(session.error?.message || 'Failed to create portal session');
  }

  await logAudit(env.DB, auth.user.id, 'billing.portal_opened', 'portal', null, request);

  return jsonResponse({ success: true, url: session.url });
}

export async function handleStripeWebhook(request: Request, env: Env): Promise<Response> {
  const signature = request.headers.get('stripe-signature');
  if (!signature || !env.STRIPE_WEBHOOK_SECRET) {
    return new Response('Missing signature or secret', { status: 400 });
  }

  const body = await request.text();

  // Verify Stripe signature
  const isValid = await verifyStripeSignature(body, signature, env.STRIPE_WEBHOOK_SECRET);
  if (!isValid) {
    return new Response('Invalid signature', { status: 401 });
  }

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

      let customer = await env.DB.prepare('SELECT * FROM customers WHERE stripe_customer_id = ?')
        .bind(customerId)
        .first();

      if (!customer) {
        const stripeCustomer = (await fetch(`https://api.stripe.com/v1/customers/${customerId}`, {
          headers: { Authorization: `Bearer ${env.STRIPE_SECRET_KEY}` },
        }).then(r => r.json())) as { email: string };

        const newCustomerId = crypto.randomUUID();
        await env.DB.prepare(
          `INSERT INTO customers (id, stripe_customer_id, email, tier) VALUES (?, ?, ?, 'pro')`
        )
          .bind(newCustomerId, customerId, stripeCustomer.email)
          .run();

        customer = { id: newCustomerId, email: stripeCustomer.email };
      }

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
        } else {
          await env.DB.prepare(
            `
            UPDATE licenses SET expires_at = datetime(?, 'unixepoch'), status = 'active' WHERE customer_id = ?
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
        await env.DB.prepare(`UPDATE licenses SET status = 'cancelled' WHERE customer_id = ?`)
          .bind(customer.id)
          .run();

        await env.DB.prepare(`UPDATE customers SET tier = 'free' WHERE id = ?`)
          .bind(customer.id)
          .run();
      }
      break;
    }

    case 'invoice.paid': {
      const invoice = event.data.object;
      const customerId = invoice.customer;

      const customer = await env.DB.prepare('SELECT * FROM customers WHERE stripe_customer_id = ?')
        .bind(customerId)
        .first();

      if (customer) {
        // Store invoice in database for revenue tracking
        await env.DB.prepare(
          `INSERT OR REPLACE INTO invoices (id, customer_id, stripe_invoice_id, amount_cents, currency, status, invoice_url, invoice_pdf, period_start, period_end, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime(?, 'unixepoch'), datetime(?, 'unixepoch'), CURRENT_TIMESTAMP)`
        )
          .bind(
            crypto.randomUUID(),
            customer.id,
            invoice.id,
            invoice.amount_paid,
            invoice.currency,
            invoice.status,
            invoice.hosted_invoice_url || null,
            invoice.invoice_pdf || null,
            invoice.period_start,
            invoice.period_end
          )
          .run();
      }
      break;
    }

    case 'invoice.payment_failed': {
      const invoice = event.data.object;
      const customerId = invoice.customer;

      const customer = await env.DB.prepare('SELECT * FROM customers WHERE stripe_customer_id = ?')
        .bind(customerId)
        .first();

      if (customer) {
        // Mark subscription as past_due
        await env.DB.prepare(`UPDATE subscriptions SET status = 'past_due' WHERE customer_id = ?`)
          .bind(customer.id)
          .run();

        // Log for admin visibility
        await env.DB.prepare(
          `INSERT INTO audit_log (id, customer_id, action, metadata, created_at)
           VALUES (?, ?, 'billing.payment_failed', ?, CURRENT_TIMESTAMP)`
        )
          .bind(crypto.randomUUID(), customer.id, JSON.stringify({ invoice_id: invoice.id, amount: invoice.amount_due }))
          .run();
      }
      break;
    }

    case 'customer.created': {
      const stripeCustomer = event.data.object;
      
      // Check if customer already exists
      const existing = await env.DB.prepare('SELECT * FROM customers WHERE stripe_customer_id = ? OR email = ?')
        .bind(stripeCustomer.id, stripeCustomer.email)
        .first();

      if (!existing) {
        await env.DB.prepare(
          `INSERT INTO customers (id, stripe_customer_id, email, name, company, created_at)
           VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)`
        )
          .bind(
            crypto.randomUUID(),
            stripeCustomer.id,
            stripeCustomer.email,
            stripeCustomer.name || null,
            stripeCustomer.metadata?.company || null
          )
          .run();
      } else if (!existing.stripe_customer_id) {
        // Link existing customer to Stripe
        await env.DB.prepare(`UPDATE customers SET stripe_customer_id = ? WHERE email = ?`)
          .bind(stripeCustomer.id, stripeCustomer.email)
          .run();
      }
      break;
    }
  }

  return new Response('OK');
}

/**
 * Admin: Sync all Stripe data (customers, subscriptions, invoices)
 * This is useful for initial setup or data recovery
 */
export async function handleAdminStripeSync(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  // Check admin status
  const adminCheck = await env.DB.prepare(`SELECT admin FROM customers WHERE id = ?`)
    .bind(auth.user.id)
    .first();

  if (adminCheck?.admin !== 1) {
    return errorResponse('Unauthorized', 403);
  }

  const results = {
    customers_synced: 0,
    subscriptions_synced: 0,
    invoices_synced: 0,
    errors: [] as string[],
  };

  try {
    // Sync customers
    let hasMore = true;
    let startingAfter: string | undefined;

    while (hasMore) {
      const url = new URL('https://api.stripe.com/v1/customers');
      url.searchParams.set('limit', '100');
      if (startingAfter) url.searchParams.set('starting_after', startingAfter);

      const response = await fetch(url.toString(), {
        headers: { Authorization: `Bearer ${env.STRIPE_SECRET_KEY}` },
      });
      const data = (await response.json()) as { data: any[]; has_more: boolean };

      for (const customer of data.data) {
        try {
          await env.DB.prepare(
            `INSERT OR REPLACE INTO customers (id, stripe_customer_id, email, name, company, created_at)
             VALUES (COALESCE((SELECT id FROM customers WHERE stripe_customer_id = ? OR email = ?), ?), ?, ?, ?, ?, CURRENT_TIMESTAMP)`
          )
            .bind(
              customer.id, customer.email, crypto.randomUUID(),
              customer.id, customer.email, customer.name, customer.metadata?.company
            )
            .run();
          results.customers_synced++;
        } catch (e) {
          results.errors.push(`Customer ${customer.email}: ${e}`);
        }
      }

      hasMore = data.has_more;
      if (data.data.length > 0) {
        startingAfter = data.data[data.data.length - 1].id;
      }
    }

    // Sync subscriptions
    hasMore = true;
    startingAfter = undefined;

    while (hasMore) {
      const url = new URL('https://api.stripe.com/v1/subscriptions');
      url.searchParams.set('limit', '100');
      url.searchParams.set('status', 'all');
      if (startingAfter) url.searchParams.set('starting_after', startingAfter);

      const response = await fetch(url.toString(), {
        headers: { Authorization: `Bearer ${env.STRIPE_SECRET_KEY}` },
      });
      const data = (await response.json()) as { data: any[]; has_more: boolean };

      for (const sub of data.data) {
        try {
          const customer = await env.DB.prepare('SELECT id FROM customers WHERE stripe_customer_id = ?')
            .bind(sub.customer)
            .first();

          if (customer) {
            await env.DB.prepare(
              `INSERT OR REPLACE INTO subscriptions (id, customer_id, stripe_subscription_id, status, current_period_end, created_at)
               VALUES (COALESCE((SELECT id FROM subscriptions WHERE stripe_subscription_id = ?), ?), ?, ?, ?, datetime(?, 'unixepoch'), CURRENT_TIMESTAMP)`
            )
              .bind(sub.id, crypto.randomUUID(), customer.id, sub.id, sub.status, sub.current_period_end)
              .run();
            results.subscriptions_synced++;
          }
        } catch (e) {
          results.errors.push(`Subscription ${sub.id}: ${e}`);
        }
      }

      hasMore = data.has_more;
      if (data.data.length > 0) {
        startingAfter = data.data[data.data.length - 1].id;
      }
    }

    // Sync invoices (last 12 months)
    hasMore = true;
    startingAfter = undefined;
    const twelveMonthsAgo = Math.floor(Date.now() / 1000) - (365 * 24 * 60 * 60);

    while (hasMore) {
      const url = new URL('https://api.stripe.com/v1/invoices');
      url.searchParams.set('limit', '100');
      url.searchParams.set('created[gte]', twelveMonthsAgo.toString());
      if (startingAfter) url.searchParams.set('starting_after', startingAfter);

      const response = await fetch(url.toString(), {
        headers: { Authorization: `Bearer ${env.STRIPE_SECRET_KEY}` },
      });
      const data = (await response.json()) as { data: any[]; has_more: boolean };

      for (const invoice of data.data) {
        if (invoice.status !== 'paid') continue;

        try {
          const customer = await env.DB.prepare('SELECT id FROM customers WHERE stripe_customer_id = ?')
            .bind(invoice.customer)
            .first();

          if (customer) {
            await env.DB.prepare(
              `INSERT OR REPLACE INTO invoices (id, customer_id, stripe_invoice_id, amount_cents, currency, status, invoice_url, invoice_pdf, period_start, period_end, created_at)
               VALUES (COALESCE((SELECT id FROM invoices WHERE stripe_invoice_id = ?), ?), ?, ?, ?, ?, ?, ?, ?, datetime(?, 'unixepoch'), datetime(?, 'unixepoch'), datetime(?, 'unixepoch'))`
            )
              .bind(
                invoice.id, crypto.randomUUID(),
                customer.id, invoice.id, invoice.amount_paid, invoice.currency, invoice.status,
                invoice.hosted_invoice_url, invoice.invoice_pdf, invoice.period_start, invoice.period_end, invoice.created
              )
              .run();
            results.invoices_synced++;
          }
        } catch (e) {
          results.errors.push(`Invoice ${invoice.id}: ${e}`);
        }
      }

      hasMore = data.has_more;
      if (data.data.length > 0) {
        startingAfter = data.data[data.data.length - 1].id;
      }
    }

  } catch (error) {
    results.errors.push(`Sync error: ${error}`);
  }

  await logAudit(env.DB, auth.user.id, 'admin.stripe_sync', 'stripe', null, request, results);

  return jsonResponse(results);
}

/**
 * Admin: Get real-time Stripe metrics (MRR, subscriber counts, etc.)
 */
export async function handleAdminStripeMetrics(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  // Check admin status
  const adminCheck = await env.DB.prepare(`SELECT admin FROM customers WHERE id = ?`)
    .bind(auth.user.id)
    .first();

  if (adminCheck?.admin !== 1) {
    return errorResponse('Unauthorized', 403);
  }

  // Fetch active subscriptions from Stripe for accurate MRR
  const subsResponse = await fetch('https://api.stripe.com/v1/subscriptions?status=active&limit=100', {
    headers: { Authorization: `Bearer ${env.STRIPE_SECRET_KEY}` },
  });
  const subsData = (await subsResponse.json()) as { data: any[] };

  let mrr = 0;
  const tierCounts: Record<string, number> = { pro: 0, team: 0, enterprise: 0 };

  for (const sub of subsData.data) {
    for (const item of sub.items.data) {
      const amount = item.price.unit_amount || 0;
      const interval = item.price.recurring?.interval;
      const intervalCount = item.price.recurring?.interval_count || 1;

      // Convert to monthly
      let monthlyAmount = amount;
      if (interval === 'year') {
        monthlyAmount = amount / (12 * intervalCount);
      } else if (interval === 'month') {
        monthlyAmount = amount / intervalCount;
      }

      mrr += monthlyAmount;

      // Categorize by tier based on price
      if (monthlyAmount >= 50000) {
        tierCounts.enterprise++;
      } else if (monthlyAmount >= 20000) {
        tierCounts.team++;
      } else {
        tierCounts.pro++;
      }
    }
  }

  // Fetch recent balance (available + pending)
  const balanceResponse = await fetch('https://api.stripe.com/v1/balance', {
    headers: { Authorization: `Bearer ${env.STRIPE_SECRET_KEY}` },
  });
  const balance = (await balanceResponse.json()) as { available: any[]; pending: any[] };

  const availableBalance = balance.available.reduce((sum: number, b: any) => sum + b.amount, 0);
  const pendingBalance = balance.pending.reduce((sum: number, b: any) => sum + b.amount, 0);

  return jsonResponse({
    mrr: Math.round(mrr),
    arr: Math.round(mrr * 12),
    active_subscriptions: subsData.data.length,
    tier_breakdown: tierCounts,
    balance: {
      available: availableBalance,
      pending: pendingBalance,
      currency: 'usd',
    },
  });
}
