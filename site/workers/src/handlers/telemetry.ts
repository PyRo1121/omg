// CLI telemetry event handlers
import { Env, jsonResponse, errorResponse, generateId } from '../api';

// ========== Payload Size Limits ==========
const MAX_EVENT_PAYLOAD_BYTES = 100 * 1024; // 100 KB
const MAX_BATCH_PAYLOAD_BYTES = 1024 * 1024; // 1 MB
const MAX_BATCH_SIZE = 500; // events

// ========== Field Length Limits ==========
const MAX_STRING_LENGTH = 1000; // characters
const MAX_ERROR_LENGTH = 5000; // characters
const MAX_ARRAY_LENGTH = 100; // items

// ========== Rate Limiting ==========
const RATE_LIMIT_EVENTS_PER_MINUTE = 100;
const RATE_LIMIT_WINDOW_SECONDS = 60;

// ========== Validation Helpers ==========

/**
 * Check Content-Length header before parsing JSON
 */
function validateContentLength(request: Request, maxBytes: number): { valid: boolean; error?: Response } {
  const contentLength = request.headers.get('Content-Length');

  if (!contentLength) {
    return { valid: false, error: errorResponse('Content-Length header required', 411) };
  }

  const bytes = parseInt(contentLength, 10);
  if (isNaN(bytes) || bytes > maxBytes) {
    const response = errorResponse('Payload too large', 413);
    response.headers.set('Content-Type', 'application/json');
    return { valid: false, error: response };
  }

  return { valid: true };
}

/**
 * Truncate string to max length (don't reject)
 */
function truncateString(value: any, maxLength: number): string | null {
  if (value === null || value === undefined) return null;
  const str = String(value);
  return str.length > maxLength ? str.slice(0, maxLength) : str;
}

/**
 * Truncate array to max length (don't reject)
 */
function truncateArray<T>(value: any, maxLength: number): T[] {
  if (!Array.isArray(value)) return [];
  return value.slice(0, maxLength);
}

/**
 * Sanitize and validate event payload
 */
function sanitizeEvent(event: any): any {
  return {
    ...event,
    command: truncateString(event.command, MAX_STRING_LENGTH),
    subcommand: truncateString(event.subcommand, MAX_STRING_LENGTH),
    error: truncateString(event.error, MAX_ERROR_LENGTH),
    packages: truncateArray(event.packages || [], MAX_ARRAY_LENGTH),
    session_id: truncateString(event.session_id, MAX_STRING_LENGTH),
    metric_type: truncateString(event.metric_type, MAX_STRING_LENGTH),
    context: truncateString(event.context, MAX_STRING_LENGTH),
    feature: truncateString(event.feature, MAX_STRING_LENGTH),
    event_type: truncateString(event.event_type, MAX_STRING_LENGTH),
    start_time: truncateString(event.start_time, MAX_STRING_LENGTH),
    end_time: truncateString(event.end_time, MAX_STRING_LENGTH),
  };
}

/**
 * Rate limit by license key using Cloudflare Rate Limiting API
 */
async function checkRateLimit(
  env: Env,
  licenseKey: string
): Promise<{ allowed: boolean; retryAfter?: number }> {
  if (!env.API_RATE_LIMITER) {
    console.warn('API_RATE_LIMITER binding not available, skipping rate limit');
    return { allowed: true };
  }

  try {
    const rateLimitKey = `telemetry:${licenseKey}`;
    const { success } = await env.API_RATE_LIMITER.limit({ key: rateLimitKey });

    if (!success) {
      // Calculate retry-after based on rate limit window
      return { allowed: false, retryAfter: RATE_LIMIT_WINDOW_SECONDS };
    }

    return { allowed: true };
  } catch (e) {
    console.error('Rate limit check failed:', e);
    // Fail open - allow request if rate limiter fails
    return { allowed: true };
  }
}

// Handle single telemetry event
export async function handleCliEvent(request: Request, env: Env): Promise<Response> {
  try {
    // Validate payload size BEFORE parsing JSON
    const lengthCheck = validateContentLength(request, MAX_EVENT_PAYLOAD_BYTES);
    if (!lengthCheck.valid) {
      return lengthCheck.error!;
    }

    const body = await request.json() as {
      event: {
        type: 'command' | 'session' | 'performance' | 'feature';
        [key: string]: any;
      };
      timestamp: string;
      machine_id: string;
      version: string;
      platform: string;
      license_key?: string;
      retries?: number;
    };

    if (!body.license_key) {
      return errorResponse('License key required', 401);
    }

    // Rate limit by license key
    const rateLimit = await checkRateLimit(env, body.license_key);
    if (!rateLimit.allowed) {
      const response = errorResponse('Rate limit exceeded', 429);
      if (rateLimit.retryAfter) {
        response.headers.set('Retry-After', String(rateLimit.retryAfter));
      }
      return response;
    }

    // Validate license key
    const license = await env.DB.prepare(
      'SELECT id, customer_id FROM licenses WHERE license_key = ? AND status = "active"'
    )
      .bind(body.license_key)
      .first();

    if (!license) {
      return errorResponse('Invalid license key', 401);
    }

    const today = new Date().toISOString().split('T')[0];
    const eventId = generateId();

    // Store based on event type (sanitize all fields)
    switch (body.event.type) {
      case 'command': {
        const cmd = sanitizeEvent(body.event as any);
        await env.DB.prepare(`
          INSERT INTO command_event (
            id, license_id, machine_id, session_id, command, subcommand,
            packages, duration_ms, success, error, result_count, updated_count, timestamp
          )
          VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `)
          .bind(
            eventId,
            license.id,
            truncateString(body.machine_id, MAX_STRING_LENGTH),
            cmd.session_id,
            cmd.command,
            cmd.subcommand,
            JSON.stringify(cmd.packages),
            cmd.duration_ms || 0,
            cmd.success ? 1 : 0,
            cmd.error,
            cmd.result_count || null,
            cmd.updated_count || null,
            truncateString(body.timestamp, MAX_STRING_LENGTH)
          )
          .run();
        break;
      }

      case 'session': {
        const sess = sanitizeEvent(body.event as any);
        await env.DB.prepare(`
          INSERT INTO session (
            id, license_id, machine_id, session_id, event_type,
            start_time, end_time, commands_run, duration_secs, timestamp
          )
          VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `)
          .bind(
            eventId,
            license.id,
            truncateString(body.machine_id, MAX_STRING_LENGTH),
            sess.session_id,
            sess.event_type,
            sess.start_time,
            sess.end_time,
            sess.commands_run || null,
            sess.duration_secs || null,
            truncateString(body.timestamp, MAX_STRING_LENGTH)
          )
          .run();
        break;
      }

      case 'performance': {
        const perf = sanitizeEvent(body.event as any);
        await env.DB.prepare(`
          INSERT INTO performance_metric (
            id, license_id, machine_id, metric_type, duration_ms, context, timestamp
          )
          VALUES (?, ?, ?, ?, ?, ?, ?)
        `)
          .bind(
            eventId,
            license.id,
            truncateString(body.machine_id, MAX_STRING_LENGTH),
            perf.metric_type,
            perf.duration_ms,
            perf.context,
            truncateString(body.timestamp, MAX_STRING_LENGTH)
          )
          .run();
        break;
      }

      case 'feature': {
        const feat = sanitizeEvent(body.event as any);
        await env.DB.prepare(`
          INSERT INTO feature_usage (
            id, license_id, machine_id, feature, enabled, metadata, timestamp
          )
          VALUES (?, ?, ?, ?, ?, ?, ?)
        `)
          .bind(
            eventId,
            license.id,
            truncateString(body.machine_id, MAX_STRING_LENGTH),
            feat.feature,
            feat.enabled ? 1 : 0,
            JSON.stringify(feat.metadata || {}),
            truncateString(body.timestamp, MAX_STRING_LENGTH)
          )
          .run();
        break;
      }

      default:
        return errorResponse('Invalid event type', 400);
    }

    return jsonResponse({ success: true, event_id: eventId });
  } catch (e) {
    console.error('CLI event error:', e);
    return errorResponse('Failed to process event', 500);
  }
}

// Handle batched telemetry events
export async function handleCliBatch(request: Request, env: Env): Promise<Response> {
  try {
    // Validate payload size BEFORE parsing JSON
    const lengthCheck = validateContentLength(request, MAX_BATCH_PAYLOAD_BYTES);
    if (!lengthCheck.valid) {
      return lengthCheck.error!;
    }

    const body = await request.json() as {
      events: Array<{
        event: {
          type: 'command' | 'session' | 'performance' | 'feature';
          [key: string]: any;
        };
        timestamp: string;
        machine_id: string;
        version: string;
        platform: string;
        license_key?: string;
        retries?: number;
      }>;
      batch_timestamp: string;
      machine_id: string;
    };

    if (!body.events || body.events.length === 0) {
      return jsonResponse({ success: true, processed: 0 });
    }

    // Enforce batch size limit
    if (body.events.length > MAX_BATCH_SIZE) {
      return errorResponse(`Batch size exceeds limit of ${MAX_BATCH_SIZE} events`, 413);
    }

    // Extract license key from first event (all should have same license)
    const licenseKey = body.events[0].license_key;
    if (!licenseKey) {
      return errorResponse('License key required', 401);
    }

    // Rate limit by license key (batch counts as multiple events)
    const rateLimit = await checkRateLimit(env, licenseKey);
    if (!rateLimit.allowed) {
      const response = errorResponse('Rate limit exceeded', 429);
      if (rateLimit.retryAfter) {
        response.headers.set('Retry-After', String(rateLimit.retryAfter));
      }
      return response;
    }

    // Validate license key
    const license = await env.DB.prepare(
      'SELECT id, customer_id FROM licenses WHERE license_key = ? AND status = "active"'
    )
      .bind(licenseKey)
      .first();

    if (!license) {
      return errorResponse('Invalid license key', 401);
    }

    const statements: D1PreparedStatement[] = [];

    // Process each event
    for (const item of body.events) {
      const eventId = generateId();

      switch (item.event.type) {
        case 'command': {
          const cmd = sanitizeEvent(item.event as any);
          statements.push(
            env.DB.prepare(`
              INSERT INTO command_event (
                id, license_id, machine_id, session_id, command, subcommand,
                packages, duration_ms, success, error, result_count, updated_count, timestamp
              )
              VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            `).bind(
              eventId,
              license.id,
              truncateString(item.machine_id, MAX_STRING_LENGTH),
              cmd.session_id,
              cmd.command,
              cmd.subcommand,
              JSON.stringify(cmd.packages),
              cmd.duration_ms || 0,
              cmd.success ? 1 : 0,
              cmd.error,
              cmd.result_count || null,
              cmd.updated_count || null,
              truncateString(item.timestamp, MAX_STRING_LENGTH)
            )
          );
          break;
        }

        case 'session': {
          const sess = sanitizeEvent(item.event as any);
          statements.push(
            env.DB.prepare(`
              INSERT INTO session (
                id, license_id, machine_id, session_id, event_type,
                start_time, end_time, commands_run, duration_secs, timestamp
              )
              VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            `).bind(
              eventId,
              license.id,
              truncateString(item.machine_id, MAX_STRING_LENGTH),
              sess.session_id,
              sess.event_type,
              sess.start_time,
              sess.end_time,
              sess.commands_run || null,
              sess.duration_secs || null,
              truncateString(item.timestamp, MAX_STRING_LENGTH)
            )
          );
          break;
        }

        case 'performance': {
          const perf = sanitizeEvent(item.event as any);
          statements.push(
            env.DB.prepare(`
              INSERT INTO performance_metric (
                id, license_id, machine_id, metric_type, duration_ms, context, timestamp
              )
              VALUES (?, ?, ?, ?, ?, ?, ?)
            `).bind(
              eventId,
              license.id,
              truncateString(item.machine_id, MAX_STRING_LENGTH),
              perf.metric_type,
              perf.duration_ms,
              perf.context,
              truncateString(item.timestamp, MAX_STRING_LENGTH)
            )
          );
          break;
        }

        case 'feature': {
          const feat = sanitizeEvent(item.event as any);
          statements.push(
            env.DB.prepare(`
              INSERT INTO feature_usage (
                id, license_id, machine_id, feature, enabled, metadata, timestamp
              )
              VALUES (?, ?, ?, ?, ?, ?, ?)
            `).bind(
              eventId,
              license.id,
              truncateString(item.machine_id, MAX_STRING_LENGTH),
              feat.feature,
              feat.enabled ? 1 : 0,
              JSON.stringify(feat.metadata || {}),
              truncateString(item.timestamp, MAX_STRING_LENGTH)
            )
          );
          break;
        }
      }
    }

    // Atomic batch execution
    if (statements.length > 0) {
      await env.DB.batch(statements);
    }

    return jsonResponse({ success: true, processed: body.events.length });
  } catch (e) {
    console.error('CLI batch error:', e);
    return errorResponse('Failed to process batch', 500);
  }
}
