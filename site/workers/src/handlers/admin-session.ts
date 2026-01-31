import { Env, jsonResponse, errorResponse, generateId, logAudit } from '../api';

export async function handleCreateAdminSession(request: Request, env: Env): Promise<Response> {
  try {
    const adminSecret = request.headers.get('X-Admin-Secret');
    
    if (!adminSecret || adminSecret !== env.ADMIN_API_SECRET) {
      return errorResponse('Unauthorized', 401);
    }

    const body = await request.json() as { 
      email: string; 
      name?: string;
      betterAuthUserId?: string;
    };
    
    if (!body.email) {
      return errorResponse('Email required', 400);
    }

    let customer = await env.DB.prepare(`SELECT * FROM customers WHERE email = ?`)
      .bind(body.email)
      .first();

    if (!customer) {
      const customerId = generateId();
      
      await env.DB.prepare(
        `INSERT INTO customers (id, email, company, tier, admin) VALUES (?, ?, ?, 'free', 1)`
      ).bind(customerId, body.email, body.name || null).run();

      const licenseKey = crypto.randomUUID();
      await env.DB.prepare(
        `INSERT INTO licenses (id, customer_id, license_key, tier, status, max_seats)
         VALUES (?, ?, ?, 'free', 'active', 1)`
      ).bind(generateId(), customerId, licenseKey).run();

      customer = { id: customerId, email: body.email, admin: 1 };
      
      await logAudit(env.DB, customerId, 'admin.session_created', 'customer', customerId, request);
    }

    if (customer.admin !== 1) {
      return errorResponse('User is not an admin', 403);
    }

    const existingSession = await env.DB.prepare(
      `SELECT * FROM sessions WHERE customer_id = ? AND expires_at > datetime('now') ORDER BY created_at DESC LIMIT 1`
    ).bind(customer.id).first();

    if (existingSession) {
      return jsonResponse({
        token: existingSession.token,
        expiresAt: existingSession.expires_at,
        customerId: customer.id,
      });
    }

    const sessionId = generateId();
    const token = crypto.randomUUID();
    const expiresAt = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString();

    await env.DB.prepare(
      `INSERT INTO sessions (id, customer_id, token, expires_at) VALUES (?, ?, ?, ?)`
    ).bind(sessionId, customer.id, token, expiresAt).run();

    await logAudit(env.DB, customer.id as string, 'admin.session_created', 'session', sessionId, request);

    return jsonResponse({
      token,
      expiresAt,
      customerId: customer.id,
    });
  } catch (error) {
    console.error('Create admin session error:', error);
    return errorResponse('Internal server error', 500);
  }
}
