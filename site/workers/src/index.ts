// OMG API Worker - Main Router
// Clean consolidated routing file

import * as Sentry from '@sentry/cloudflare';
import { corsHeaders } from './api';

// License handlers (CLI activation & usage reporting)
import {
  handleValidateLicense,
  handleGetLicense,
  handleReportUsage,
  handleInstallPing,
  handleAnalytics,
} from './handlers/license';

// Auth handlers (OTP login)
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
} from './handlers/dashboard';

import {
  handleCreateCheckout,
  handleBillingPortal,
  handleStripeWebhook,
} from './handlers/billing';

// Admin handlers
import {
  handleAdminDashboard,
  handleAdminCRMUsers,
  handleAdminUserDetail,
  handleAdminUpdateUser,
  handleAdminActivity,
  handleAdminHealth,
  handleAdminCohorts,
  handleAdminRevenue,
  handleAdminAuditLog,
  handleAdminExportUsers,
  handleAdminExportUsage,
  handleAdminExportAudit,
  handleAdminAnalytics,
  handleAdminAdvancedMetrics,
} from './handlers/admin';

import { handleGetSmartInsights } from './handlers/insights';

// Team Controls handlers (Team/Enterprise)
import {
  handleGetPolicies,
  handleCreatePolicy,
  handleUpdatePolicy,
  handleDeletePolicy,
  handleGetNotificationSettings,
  handleUpdateNotificationSettings,
  handleRevokeMember,
  handleGetAuditLogs,
  handleGetTeamMembers as handleGetTeamMembersControl,
  handleUpdateAlertThreshold,
} from './handlers/team-controls';

import { handleFleetPush } from './handlers/fleet';
import { handleGetFirehose } from './handlers/firehose';

export interface Env {
  DB: D1Database;
  AI: any;
  STRIPE_SECRET_KEY: string;
  STRIPE_WEBHOOK_SECRET: string;
  JWT_SECRET: string;
  RESEND_API_KEY?: string;
  JWT_PRIVATE_KEY?: string;
  ADMIN_USER_ID?: string;
  STRIPE_TEAM_PRICE_ID?: string;
  STRIPE_ENT_PRICE_ID?: string;
  META_API_KEY?: string;
  ACCOUNT_ID?: string;
  ADMIN_RATE_LIMITER?: any;
  // Turnstile (Cloudflare CAPTCHA)
  TURNSTILE_SECRET_KEY?: string;
  // Sentry error tracking
  SENTRY_DSN?: string;
}

export default Sentry.withSentry(
  (env: Env) => ({
    dsn: env.SENTRY_DSN,
    tracesSampleRate: 0.1, // Sample 10% of requests for performance monitoring
    environment: 'production',
  }),
  {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;
    const method = request.method;

    // Handle CORS preflight
    if (method === 'OPTIONS') {
      return new Response(null, { 
        headers: {
          ...corsHeaders,
          'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
        }
      });
    }

    try {
      // ============================================
      // Health Check
      // ============================================
      if (path === '/health') {
        return new Response(JSON.stringify({ 
          status: 'ok', 
          timestamp: new Date().toISOString() 
        }), {
          headers: { 'Content-Type': 'application/json', ...corsHeaders },
        });
      }

      // ============================================
      // License / CLI Endpoints (public)
      // ============================================
      if (path === '/api/validate-license' && (method === 'GET' || method === 'POST')) {
        return handleValidateLicense(request, env);
      }

      if (path === '/api/get-license' && method === 'GET') {
        return handleGetLicense(request, env);
      }

      if (path === '/api/report-usage' && method === 'POST') {
        return handleReportUsage(request, env);
      }

      if (path === '/api/install-ping' && method === 'POST') {
        return handleInstallPing(request, env);
      }

      if (path === '/api/analytics' && method === 'POST') {
        return handleAnalytics(request, env);
      }

      // ============================================
      // Auth Endpoints (public)
      // ============================================
      if (path === '/api/auth/send-code' && method === 'POST') {
        return handleSendCode(request, env);
      }

      if (path === '/api/auth/verify-code' && method === 'POST') {
        return handleVerifyCode(request, env);
      }

      if (path === '/api/auth/verify-session' && method === 'POST') {
        return handleVerifySession(request, env);
      }

      if (path === '/api/auth/logout' && method === 'POST') {
        return handleLogout(request, env);
      }

      // ============================================
      // Dashboard Endpoints (authenticated)
      // ============================================
      if (path === '/api/dashboard' && method === 'GET') {
        return handleGetDashboard(request, env);
      }

      if (path === '/api/user/profile' && method === 'PUT') {
        return handleUpdateProfile(request, env);
      }

      if (path === '/api/license/regenerate' && method === 'POST') {
        return handleRegenerateLicense(request, env);
      }

      if (path === '/api/machines/revoke' && method === 'POST') {
        return handleRevokeMachine(request, env);
      }

      if (path === '/api/sessions' && method === 'GET') {
        return handleGetSessions(request, env);
      }

      if (path === '/api/sessions/revoke' && method === 'POST') {
        return handleRevokeSession(request, env);
      }

      if (path === '/api/audit-log' && method === 'GET') {
        return handleGetAuditLog(request, env);
      }

      // ============================================
      // Team Endpoints (Team/Enterprise)
      // ============================================
      if (path === '/api/team/members' && method === 'GET') {
        return handleGetTeamMembers(request, env);
      }

      if (path === '/api/team/revoke' && method === 'POST') {
        return handleRevokeTeamMember(request, env);
      }

      // ============================================
      // Team Controls API (Team/Enterprise)
      // ============================================
      
      // Policies (Enterprise only)
      if (path === '/api/team/policies' && method === 'GET') {
        return handleGetPolicies(request, env);
      }
      if (path === '/api/team/policies' && method === 'POST') {
        return handleCreatePolicy(request, env);
      }
      if (path === '/api/team/policies' && method === 'PUT') {
        return handleUpdatePolicy(request, env);
      }
      if (path === '/api/team/policies' && method === 'DELETE') {
        return handleDeletePolicy(request, env);
      }

      // Notification Settings (Team/Enterprise)
      if (path === '/api/team/notifications' && method === 'GET') {
        return handleGetNotificationSettings(request, env);
      }
      if (path === '/api/team/notifications' && method === 'POST') {
        return handleUpdateNotificationSettings(request, env);
      }

      // Member Management (Team/Enterprise)
      if (path === '/api/team/members/revoke' && method === 'POST') {
        return handleRevokeMember(request, env);
      }
      if (path === '/api/team/members/list' && method === 'GET') {
        return handleGetTeamMembersControl(request, env);
      }

      // Audit Logs (Team/Enterprise)
      if (path === '/api/team/audit-logs' && method === 'GET') {
        return handleGetAuditLogs(request, env);
      }

      // Alert Thresholds (Team/Enterprise)
      if (path === '/api/team/thresholds' && method === 'POST') {
        return handleUpdateAlertThreshold(request, env);
      }

      // Fleet Operations (Team/Enterprise)
      if (path === '/api/fleet/push' && method === 'POST') {
        return handleFleetPush(request, env);
      }

      if (path === '/api/insights' && method === 'GET') {
        return handleGetSmartInsights(request, env);
      }

      // ============================================
      // Billing Endpoints (authenticated)
      // ============================================
      if (path === '/api/billing/checkout' && method === 'POST') {
        return handleCreateCheckout(request, env);
      }

      if (path === '/api/billing/portal' && method === 'POST') {
        return handleBillingPortal(request, env);
      }

      if (path === '/webhook/stripe' && method === 'POST') {
        return handleStripeWebhook(request, env);
      }

      // ============================================
      // Admin Endpoints (admin only)
      // ============================================
      if (path === '/api/admin/dashboard' && method === 'GET') {
        return handleAdminDashboard(request, env);
      }

      if (path === '/api/admin/users' && method === 'GET') {
        return handleAdminCRMUsers(request, env);
      }

      if (path === '/api/admin/user' && method === 'GET') {
        return handleAdminUserDetail(request, env);
      }

      if (path === '/api/admin/user' && method === 'PUT') {
        return handleAdminUpdateUser(request, env);
      }

      if (path === '/api/admin/activity' && method === 'GET') {
        return handleAdminActivity(request, env);
      }

      if (path === '/api/admin/health' && method === 'GET') {
        return handleAdminHealth(request, env);
      }

      if (path === '/api/admin/cohorts' && method === 'GET') {
        return handleAdminCohorts(request, env);
      }

      if (path === '/api/admin/revenue' && method === 'GET') {
        return handleAdminRevenue(request, env);
      }

      if (path === '/api/admin/analytics' && method === 'GET') {
        return handleAdminAnalytics(request, env);
      }

      if (path === '/api/admin/advanced-metrics' && method === 'GET') {
        return handleAdminAdvancedMetrics(request, env);
      }

      if (path === '/api/admin/audit-log' && method === 'GET') {
        return handleAdminAuditLog(request, env);
      }

      if (path === '/api/admin/firehose' && method === 'GET') {
        return handleGetFirehose(request, env);
      }

      if (path === '/api/admin/crm/users' && method === 'GET') {
        return handleAdminCRMUsers(request, env);
      }

      if (path === '/api/admin/export/users' && method === 'GET') {
        return handleAdminExportUsers(request, env);
      }

      if (path === '/api/admin/export/usage' && method === 'GET') {
        return handleAdminExportUsage(request, env);
      }

      if (path === '/api/admin/export/audit' && method === 'GET') {
        return handleAdminExportAudit(request, env);
      }

      // ============================================
      // Badge Endpoint (public)
      // ============================================
      if (path === '/api/badge/installs' && method === 'GET') {
        const result = await env.DB.prepare(
          `SELECT COUNT(DISTINCT install_id) as total FROM install_stats`
        ).first();
        const total = (result?.total as number) || 0;
        return new Response(JSON.stringify({
          schemaVersion: 1,
          label: 'installs',
          message: total.toLocaleString(),
          color: 'blue',
        }), {
          headers: {
            'Content-Type': 'application/json',
            'Cache-Control': 'public, max-age=300',
            ...corsHeaders,
          },
        });
      }

      // 404 Not Found
      return new Response(JSON.stringify({ error: 'Not found' }), {
        status: 404,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });

    } catch (error) {
      // Capture error in Sentry
      Sentry.captureException(error);
      console.error('Worker error:', error);
      return new Response(JSON.stringify({
        error: 'Internal server error',
        message: error instanceof Error ? error.message : 'Unknown error'
      }), {
        status: 500,
        headers: { 'Content-Type': 'application/json', ...corsHeaders },
      });
    }
  },
} satisfies ExportedHandler<Env>);
