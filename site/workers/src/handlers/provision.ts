import { Env, jsonResponse, errorResponse, generateId, logAudit } from '../api';

export async function handleProvisionUser(request: Request, env: Env): Promise<Response> {
  try {
    const body = await request.json() as { email: string; name?: string };
    
    if (!body.email) {
      return errorResponse('Email required', 400);
    }

    let customer = await env.DB.prepare(`SELECT * FROM customers WHERE email = ?`)
      .bind(body.email)
      .first();

    let licenseKey: string;

    if (customer) {
      const existingLicense = await env.DB.prepare(
        `SELECT license_key FROM licenses WHERE customer_id = ? AND status = 'active' LIMIT 1`
      ).bind(customer.id).first();

      if (existingLicense) {
        return jsonResponse({
          success: true,
          customerId: customer.id,
          licenseKey: existingLicense.license_key,
          message: 'Customer already exists',
        });
      }

      licenseKey = crypto.randomUUID();
      await env.DB.prepare(
        `INSERT INTO licenses (id, customer_id, license_key, tier, status, max_seats)
         VALUES (?, ?, ?, 'free', 'active', 1)`
      ).bind(generateId(), customer.id, licenseKey).run();
    } else {
      const customerId = generateId();
      
      await env.DB.prepare(
        `INSERT INTO customers (id, email, company, tier) VALUES (?, ?, ?, 'free')`
      ).bind(customerId, body.email, body.name || null).run();

      licenseKey = crypto.randomUUID();
      
      await env.DB.prepare(
        `INSERT INTO licenses (id, customer_id, license_key, tier, status, max_seats)
         VALUES (?, ?, ?, 'free', 'active', 1)`
      ).bind(generateId(), customerId, licenseKey).run();

      customer = { id: customerId, email: body.email };
      
      await logAudit(env.DB, customerId, 'user.provisioned', 'customer', customerId, request);
    }

    return jsonResponse({
      success: true,
      customerId: customer.id,
      licenseKey,
    });
  } catch (error) {
    console.error('Provision user error:', error);
    return errorResponse('Internal server error', 500);
  }
}
