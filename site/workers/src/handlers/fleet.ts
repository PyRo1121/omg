import { Env, jsonResponse, errorResponse, validateSession, getAuthToken, License } from '../api';

export async function handleFleetPush(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  // Fetch license
  const license = await env.DB.prepare(
    `SELECT * FROM licenses WHERE customer_id = ? AND status = 'active'`
  ).bind(auth.user.id).first<License>();

  if (!license) {
    return errorResponse('No active license found', 403);
  }

  // Check tier
  if (!['team', 'enterprise'].includes(license.tier)) {
    return errorResponse('Fleet features require Team tier', 403);
  }

  try {
    const body = await request.json() as { team: string; message: string; lock_content: string; machine_count: number };
    
    // Validate payload
    if (!body.lock_content) {
      return errorResponse('Missing lock content');
    }

    // Store the push event in audit log
    await env.DB.prepare(
      `INSERT INTO audit_log (id, license_id, action, resource_type, metadata, created_at, user_email)
       VALUES (?, ?, ?, ?, ?, ?, ?)`
    ).bind(
      crypto.randomUUID(),
      license.id,
      'fleet.push',
      'config',
      JSON.stringify({ 
        team: body.team, 
        message: body.message, 
        machine_count: body.machine_count,
        size_bytes: body.lock_content.length 
      }),
      new Date().toISOString(),
      auth.user.email
    ).run();

    // In a real implementation, we would also:
    // 1. Store the lock file in R2/Storage
    // 2. Update the 'team_state' table to reference this new version
    // 3. Trigger webhooks for connected agents

    return jsonResponse({ 
      success: true, 
      pushed_at: new Date().toISOString(),
      version: crypto.randomUUID().slice(0, 8)
    });
  } catch (e) {
    console.error('Fleet push error:', e);
    return errorResponse('Failed to process fleet push', 500);
  }
}
