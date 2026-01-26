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
  handleAdminCRMUsers,
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
  handleAdminAnalytics,
  handleInitDb,
} from './handlers/admin';
import { handleGetSmartInsights } from './handlers/insights';
import { handleGetFirehose } from './handlers/firehose';
import {
  handleCreateCheckout,
  handleBillingPortal,
  handleStripeWebhook,
} from './handlers/billing';
import {
  handleDocsAnalytics,
  handleDocsAnalyticsDashboard,
  cleanupDocsAnalytics,
} from './handlers/docs-analytics';

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    // Add execution context to env for waitUntil support
    (env as any).ctx = ctx;
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

      // Docs analytics (batch from docs site)
      if (path === '/api/docs/analytics' && request.method === 'POST') {
        return handleDocsAnalytics(request, env);
      }

      // Docs analytics dashboard (admin view)
      if (path === '/api/docs/analytics/dashboard' && request.method === 'GET') {
        return handleDocsAnalyticsDashboard(request, env);
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
        return handleAdminCRMUsers(request, env);
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
      if ((path === '/api/admin/activity' || path === '/api/admin/events') && request.method === 'GET') {
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
        return handleAdminAnalytics(request, env);
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

  // Scheduled cron handler for cleanup tasks
  async scheduled(event: ScheduledEvent, env: Env, ctx: ExecutionContext): Promise<void> {
    console.log('Running scheduled cleanup tasks');
    ctx.waitUntil(cleanupDocsAnalytics(env.DB));
  },
};
