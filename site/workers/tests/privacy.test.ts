/**
 * Privacy API Tests (GDPR/CCPA Compliance)
 * Tests for:
 * - GET /api/privacy/status
 * - POST /api/privacy/export
 * - POST /api/privacy/delete
 * - POST /api/privacy/opt-out
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { env, createExecutionContext, waitOnExecutionContext } from 'cloudflare:test';
import worker from '../src/worker';

describe('Privacy API', () => {
  // Test data
  const TEST_EMAIL = 'privacy-test@example.com';
  const TEST_LICENSE_KEY = 'privacy-test-key-123';
  const TEST_CUSTOMER_ID = 'privacy-customer-id';
  const TEST_LICENSE_ID = 'privacy-license-id';
  const TEST_MACHINE_ID = 'privacy-machine-123';

  beforeEach(async () => {
    // Set up test customer and license with telemetry data
    await env.DB.prepare(`
      INSERT INTO customers (id, email, company, tier, telemetry_opt_out, created_at)
      VALUES (?, ?, ?, ?, ?, datetime('now'))
    `).bind(TEST_CUSTOMER_ID, TEST_EMAIL, 'Privacy Test Corp', 'pro', 0).run();

    await env.DB.prepare(`
      INSERT INTO licenses (id, customer_id, license_key, tier, status, max_machines, created_at)
      VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
    `).bind(TEST_LICENSE_ID, TEST_CUSTOMER_ID, TEST_LICENSE_KEY, 'pro', 'active', 3).run();

    // Add some telemetry data
    await env.DB.prepare(`
      INSERT INTO command_event (id, license_id, machine_id, command, success, timestamp)
      VALUES (?, ?, ?, ?, ?, datetime('now'))
    `).bind('cmd-1', TEST_LICENSE_ID, TEST_MACHINE_ID, 'search', 1).run();

    await env.DB.prepare(`
      INSERT INTO session (id, license_id, machine_id, session_id, event_type, timestamp)
      VALUES (?, ?, ?, ?, ?, datetime('now'))
    `).bind('sess-1', TEST_LICENSE_ID, TEST_MACHINE_ID, 'sess-123', 'start').run();

    await env.DB.prepare(`
      INSERT INTO performance_metric (id, license_id, machine_id, metric_type, duration_ms, timestamp)
      VALUES (?, ?, ?, ?, ?, datetime('now'))
    `).bind('perf-1', TEST_LICENSE_ID, TEST_MACHINE_ID, 'search_latency', 10).run();

    await env.DB.prepare(`
      INSERT INTO feature_usage (id, license_id, machine_id, feature, enabled, timestamp)
      VALUES (?, ?, ?, ?, ?, datetime('now'))
    `).bind('feat-1', TEST_LICENSE_ID, TEST_MACHINE_ID, 'sbom', 1).run();
  });

  afterEach(async () => {
    // Clean up all test data
    await env.DB.prepare('DELETE FROM command_event WHERE license_id = ?').bind(TEST_LICENSE_ID).run();
    await env.DB.prepare('DELETE FROM session WHERE license_id = ?').bind(TEST_LICENSE_ID).run();
    await env.DB.prepare('DELETE FROM performance_metric WHERE license_id = ?').bind(TEST_LICENSE_ID).run();
    await env.DB.prepare('DELETE FROM feature_usage WHERE license_id = ?').bind(TEST_LICENSE_ID).run();
    await env.DB.prepare('DELETE FROM audit_log WHERE resource_id = ?').bind(TEST_CUSTOMER_ID).run();
    await env.DB.prepare('DELETE FROM licenses WHERE id = ?').bind(TEST_LICENSE_ID).run();
    await env.DB.prepare('DELETE FROM customers WHERE id = ?').bind(TEST_CUSTOMER_ID).run();
  });

  describe('GET /api/privacy/status', () => {
    it('should return privacy policy without license key', async () => {
      const request = new Request('http://localhost/api/privacy/status', {
        method: 'GET',
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();

      expect(body).toHaveProperty('privacy_policy_version');
      expect(body).toHaveProperty('data_retention');
      expect(body).toHaveProperty('your_rights');
      expect(body).toHaveProperty('available_globally', true);
      expect(body.your_rights).toContain('Right to access (GET /api/privacy/export)');
      expect(body.your_rights).toContain('Right to deletion (POST /api/privacy/delete)');
      expect(body.your_rights).toContain('Right to opt-out (POST /api/privacy/opt-out)');
    });

    it('should return user status with valid license key', async () => {
      const request = new Request(
        `http://localhost/api/privacy/status?license_key=${TEST_LICENSE_KEY}`,
        { method: 'GET' }
      );

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();

      expect(body).toHaveProperty('user_status');
      expect(body.user_status).toHaveProperty('telemetry_opt_out', false);
      expect(body.user_status.email_on_file).toContain('@example.com');
    });

    it('should return null user_status for invalid license key', async () => {
      const request = new Request(
        'http://localhost/api/privacy/status?license_key=invalid-key',
        { method: 'GET' }
      );

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();

      expect(body).toHaveProperty('user_status', null);
    });
  });

  describe('POST /api/privacy/export', () => {
    it('should export user data with valid license key', async () => {
      const request = new Request('http://localhost/api/privacy/export', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: TEST_LICENSE_KEY,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      expect(response.headers.get('Content-Type')).toBe('application/json');
      expect(response.headers.get('Content-Disposition')).toContain('attachment');
      expect(response.headers.get('Content-Disposition')).toContain('omg-data-export-');

      const body = await response.json();

      // Verify export structure
      expect(body).toHaveProperty('export_date');
      expect(body).toHaveProperty('export_format_version', '1.0');
      expect(body).toHaveProperty('profile');
      expect(body).toHaveProperty('licenses');
      expect(body).toHaveProperty('command_history');
      expect(body).toHaveProperty('sessions');
      expect(body).toHaveProperty('performance_summary');
      expect(body).toHaveProperty('feature_usage');

      // Verify profile data
      expect(body.profile.email).toBe(TEST_EMAIL);
      expect(body.profile.company).toBe('Privacy Test Corp');
      expect(body.profile.tier).toBe('pro');

      // Verify telemetry data
      expect(body.command_history).toHaveLength(1);
      expect(body.sessions).toHaveLength(1);
    });

    it('should export user data with email', async () => {
      const request = new Request('http://localhost/api/privacy/export', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: TEST_EMAIL,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();

      expect(body.profile.email).toBe(TEST_EMAIL);
    });

    it('should return 400 without email or license_key', async () => {
      const request = new Request('http://localhost/api/privacy/export', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(400);
      const body = await response.json();
      expect(body.error).toContain('Must provide email or license_key');
    });

    it('should return 404 for non-existent user', async () => {
      const request = new Request('http://localhost/api/privacy/export', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: 'nonexistent@example.com',
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(404);
      const body = await response.json();
      expect(body.error).toContain('No matching records found');
    });

    it('should create audit log entry for export', async () => {
      const request = new Request('http://localhost/api/privacy/export', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: TEST_LICENSE_KEY,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);

      // Verify audit log entry
      const audit = await env.DB.prepare(
        'SELECT * FROM audit_log WHERE action = ? AND resource_id = ?'
      ).bind('data_export_request', TEST_CUSTOMER_ID).first();

      expect(audit).toBeTruthy();
      expect(audit?.resource_type).toBe('customer');
    });
  });

  describe('POST /api/privacy/delete', () => {
    it('should require confirm: true', async () => {
      const request = new Request('http://localhost/api/privacy/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: TEST_LICENSE_KEY,
          confirm: false,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(400);
      const body = await response.json();
      expect(body.error).toContain('Deletion must be confirmed');
    });

    it('should delete all user data with license_key', async () => {
      const request = new Request('http://localhost/api/privacy/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: TEST_LICENSE_KEY,
          confirm: true,
          reason: 'Testing data deletion',
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();

      expect(body).toHaveProperty('success', true);
      expect(body).toHaveProperty('request_id');
      expect(body).toHaveProperty('deleted');
      expect(body).toHaveProperty('retention_notice');

      // Verify telemetry data was deleted
      const commands = await env.DB.prepare(
        'SELECT COUNT(*) as count FROM command_event WHERE license_id = ?'
      ).bind(TEST_LICENSE_ID).first();
      expect(commands?.count).toBe(0);

      const sessions = await env.DB.prepare(
        'SELECT COUNT(*) as count FROM session WHERE license_id = ?'
      ).bind(TEST_LICENSE_ID).first();
      expect(sessions?.count).toBe(0);

      const perf = await env.DB.prepare(
        'SELECT COUNT(*) as count FROM performance_metric WHERE license_id = ?'
      ).bind(TEST_LICENSE_ID).first();
      expect(perf?.count).toBe(0);

      const features = await env.DB.prepare(
        'SELECT COUNT(*) as count FROM feature_usage WHERE license_id = ?'
      ).bind(TEST_LICENSE_ID).first();
      expect(features?.count).toBe(0);
    });

    it('should delete data by email', async () => {
      const request = new Request('http://localhost/api/privacy/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: TEST_EMAIL,
          confirm: true,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();
      expect(body.success).toBe(true);
    });

    it('should delete data by machine_id', async () => {
      const request = new Request('http://localhost/api/privacy/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          machine_id: TEST_MACHINE_ID,
          confirm: true,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();
      expect(body.success).toBe(true);

      // Verify machine-specific data was deleted
      const commands = await env.DB.prepare(
        'SELECT COUNT(*) as count FROM command_event WHERE machine_id = ?'
      ).bind(TEST_MACHINE_ID).first();
      expect(commands?.count).toBe(0);
    });

    it('should create audit log entry for deletion', async () => {
      const request = new Request('http://localhost/api/privacy/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: TEST_LICENSE_KEY,
          confirm: true,
          reason: 'GDPR request',
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);

      // Verify audit log entry
      const audit = await env.DB.prepare(
        'SELECT * FROM audit_log WHERE action = ? AND resource_id = ?'
      ).bind('data_deletion_request', TEST_CUSTOMER_ID).first();

      expect(audit).toBeTruthy();
      expect(audit?.resource_type).toBe('customer');
      const details = JSON.parse(audit?.details as string);
      expect(details.reason).toBe('GDPR request');
    });

    it('should anonymize license record', async () => {
      const request = new Request('http://localhost/api/privacy/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: TEST_LICENSE_KEY,
          confirm: true,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);

      // Verify license status was updated to deleted_by_user
      const license = await env.DB.prepare(
        'SELECT status FROM licenses WHERE id = ?'
      ).bind(TEST_LICENSE_ID).first();

      expect(license?.status).toBe('deleted_by_user');
    });

    it('should return 400 without email, license_key, or machine_id', async () => {
      const request = new Request('http://localhost/api/privacy/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          confirm: true,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(400);
      const body = await response.json();
      expect(body.error).toContain('Must provide email, license_key, or machine_id');
    });

    it('should return 404 for non-existent user', async () => {
      const request = new Request('http://localhost/api/privacy/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: 'nonexistent@example.com',
          confirm: true,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(404);
      const body = await response.json();
      expect(body.error).toContain('No matching records found');
    });
  });

  describe('POST /api/privacy/opt-out', () => {
    it('should opt-out of telemetry', async () => {
      const request = new Request('http://localhost/api/privacy/opt-out', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: TEST_LICENSE_KEY,
          opt_out: true,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();

      expect(body).toHaveProperty('success', true);
      expect(body).toHaveProperty('telemetry_opt_out', true);
      expect(body.message).toContain('Telemetry disabled');

      // Verify opt-out flag was set in database
      const customer = await env.DB.prepare(
        'SELECT telemetry_opt_out FROM customers WHERE id = ?'
      ).bind(TEST_CUSTOMER_ID).first();

      expect(customer?.telemetry_opt_out).toBe(1);
    });

    it('should opt-in to telemetry', async () => {
      // First opt-out
      await env.DB.prepare(
        'UPDATE customers SET telemetry_opt_out = 1 WHERE id = ?'
      ).bind(TEST_CUSTOMER_ID).run();

      // Then opt-in
      const request = new Request('http://localhost/api/privacy/opt-out', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: TEST_LICENSE_KEY,
          opt_out: false,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(200);
      const body = await response.json();

      expect(body).toHaveProperty('success', true);
      expect(body).toHaveProperty('telemetry_opt_out', false);
      expect(body.message).toContain('Telemetry re-enabled');

      // Verify opt-out flag was unset
      const customer = await env.DB.prepare(
        'SELECT telemetry_opt_out FROM customers WHERE id = ?'
      ).bind(TEST_CUSTOMER_ID).first();

      expect(customer?.telemetry_opt_out).toBe(0);
    });

    it('should return 400 without license_key', async () => {
      const request = new Request('http://localhost/api/privacy/opt-out', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          opt_out: true,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(400);
      const body = await response.json();
      expect(body.error).toContain('License key required');
    });

    it('should return 404 for invalid license_key', async () => {
      const request = new Request('http://localhost/api/privacy/opt-out', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          license_key: 'invalid-key-xyz',
          opt_out: true,
        }),
      });

      const ctx = createExecutionContext();
      const response = await worker.fetch(request, env, ctx);
      await waitOnExecutionContext(ctx);

      expect(response.status).toBe(404);
      const body = await response.json();
      expect(body.error).toContain('Invalid license key');
    });
  });
});
