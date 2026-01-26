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
  }

  return new Response('OK');
}
