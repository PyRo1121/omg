// Firehose Handler - Streaming real-time events to Admin Dashboard
import { Env, jsonResponse, errorResponse, validateSession, getAuthToken } from '../api';

export async function handleGetFirehose(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  // Strictly Admin Only
  const isAdmin = env.ADMIN_USER_ID ? auth.user.id === env.ADMIN_USER_ID : false;
  if (!isAdmin) return errorResponse('Forbidden', 403);

  const url = new URL(request.url);
  const limit = Math.min(Number(url.searchParams.get('limit')) || 50, 100);
  const since = url.searchParams.get('since'); // ISO timestamp

  try {
    let query = `
      SELECT
        id,
        event_type,
        event_name,
        properties,
        timestamp,
        session_id,
        machine_id,
        version,
        platform,
        duration_ms,
        created_at
      FROM analytics_events
    `;

    const params: any[] = [];

    if (since) {
      query += ` WHERE created_at > ?`;
      params.push(since);
    }

    query += ` ORDER BY created_at DESC LIMIT ?`;
    params.push(limit);

    const { results } = await env.DB.prepare(query).bind(...params).all();

    // Parse properties JSON for frontend convenience
    const events = results.map((event: any) => ({
      ...event,
      properties: event.properties ? JSON.parse(event.properties as string) : {},
    }));

    return jsonResponse({
      events,
      count: events.length,
      timestamp: new Date().toISOString()
    });
  } catch (e) {
    console.error('Firehose error:', e);
    return errorResponse('Failed to fetch firehose data', 500);
  }
}
