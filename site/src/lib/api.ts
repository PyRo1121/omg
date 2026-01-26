// API Client for OMG Dashboard
// All authenticated requests include Bearer token

const API_BASE = 'https://api.pyro1121.com';

// Get stored session token
export function getSessionToken(): string | null {
  return localStorage.getItem('omg_session_token');
}

// Set session token
export function setSessionToken(token: string): void {
  localStorage.setItem('omg_session_token', token);
}

// Clear session
export function clearSession(): void {
  localStorage.removeItem('omg_session_token');
}

// API request helper with auth
export async function apiRequest<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
  const token = getSessionToken();

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...((options.headers as Record<string, string>) || {}),
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const response = await fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers,
  });

  const data = await response.json();

  if (!response.ok) {
    throw new ApiError(data.error || 'Request failed', response.status);
  }

  return data as T;
}

// Generic HTTP helpers
export async function get<T>(endpoint: string): Promise<T> {
  return apiRequest<T>(endpoint, { method: 'GET' });
}

export async function post<T>(endpoint: string, body?: any): Promise<T> {
  return apiRequest<T>(endpoint, {
    method: 'POST',
    body: body ? JSON.stringify(body) : undefined,
  });
}

export async function put<T>(endpoint: string, body?: any): Promise<T> {
  return apiRequest<T>(endpoint, {
    method: 'PUT',
    body: body ? JSON.stringify(body) : undefined,
  });
}

export async function del<T>(endpoint: string, body?: any): Promise<T> {
  return apiRequest<T>(endpoint, {
    method: 'DELETE',
    body: body ? JSON.stringify(body) : undefined,
  });
}

// Custom error class
export class ApiError extends Error {
  constructor(
    message: string,
    public status: number
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

// ============================================
// Auth API
// ============================================

export interface SendCodeResponse {
  success: boolean;
  status?: string;
  message?: string;
  error?: string;
}

export async function sendCode(email: string, turnstileToken?: string): Promise<SendCodeResponse> {
  return apiRequest('/api/auth/send-code', {
    method: 'POST',
    body: JSON.stringify({ email, turnstileToken }),
  });
}

export interface VerifyCodeResponse {
  success: boolean;
  token?: string;
  expires_at?: string;
  user?: {
    id: string;
    email: string;
    name: string | null;
  };
  error?: string;
}

export async function verifyCode(email: string, code: string): Promise<VerifyCodeResponse> {
  return apiRequest('/api/auth/verify-code', {
    method: 'POST',
    body: JSON.stringify({ email, code }),
  });
}

export interface VerifySessionResponse {
  valid: boolean;
  user?: User;
  expires_at?: string;
}

export async function verifySession(token: string): Promise<VerifySessionResponse> {
  return apiRequest('/api/auth/verify-session', {
    method: 'POST',
    body: JSON.stringify({ token }),
  });
}

export async function logout(): Promise<void> {
  const token = getSessionToken();
  if (token) {
    await apiRequest('/api/auth/logout', {
      method: 'POST',
      body: JSON.stringify({ token }),
    });
  }
  clearSession();
}

// ============================================
// Dashboard API
// ============================================

export interface User {
  id: string;
  email: string;
  name: string | null;
  avatar_url: string | null;
  created_at: string;
}

export interface License {
  id: string;
  license_key: string;
  tier: 'free' | 'pro' | 'team' | 'enterprise';
  status: 'active' | 'suspended' | 'cancelled' | 'expired';
  max_machines: number;
  expires_at: string | null;
  features: string[];
}

export interface Machine {
  id: string;
  machine_id: string;
  hostname: string | null;
  os: string | null;
  arch: string | null;
  omg_version: string | null;
  last_seen_at: string;
  first_seen_at: string;
  is_active: number;
}

export interface UsageStats {
  total_commands: number;
  total_packages_installed: number;
  total_packages_searched: number;
  total_runtimes_switched: number;
  total_sbom_generated: number;
  total_vulnerabilities_found: number;
  total_time_saved_ms: number;
  current_streak: number;
  longest_streak: number;
  daily: Array<{
    date: string;
    commands_run: number;
    time_saved_ms: number;
  }>;
}

export interface Achievement {
  id: string;
  emoji: string;
  name: string;
  description: string;
  unlocked: boolean;
  unlocked_at?: string;
}

export interface Subscription {
  status: string;
  current_period_end: string;
  cancel_at_period_end: number;
}

export interface Invoice {
  id: string;
  amount_cents: number;
  currency: string;
  status: string;
  invoice_url: string | null;
  invoice_pdf: string | null;
  period_start: string;
  period_end: string;
  created_at: string;
}

export interface Session {
  id: string;
  ip_address: string | null;
  user_agent: string | null;
  created_at: string;
  expires_at: string;
  is_current: boolean;
}

export interface AuditLogEntry {
  id: string;
  action: string;
  resource_type: string | null;
  resource_id: string | null;
  ip_address: string | null;
  created_at: string;
}

export interface DashboardData {
  user: User;
  license: License;
  machines: Machine[];
  usage: UsageStats;
  achievements: Achievement[];
  subscription: Subscription | null;
  invoices: Invoice[];
  is_admin?: boolean;
  global_stats?: {
    top_package: string;
    top_runtime: string;
    percentile: number;
  };
}

export async function getDashboard(): Promise<DashboardData> {
  return apiRequest('/api/dashboard');
}

export async function updateProfile(name: string): Promise<{ success: boolean }> {
  return apiRequest('/api/user/profile', {
    method: 'PUT',
    body: JSON.stringify({ name }),
  });
}

export async function regenerateLicense(): Promise<{
  success: boolean;
  license_key: string;
  message: string;
}> {
  return apiRequest('/api/license/regenerate', {
    method: 'POST',
  });
}

export async function revokeMachine(machineId: string): Promise<{ success: boolean }> {
  return apiRequest('/api/machines/revoke', {
    method: 'POST',
    body: JSON.stringify({ machine_id: machineId }),
  });
}

export async function getSessions(): Promise<{ sessions: Session[] }> {
  return apiRequest('/api/sessions');
}

export async function revokeSession(sessionId: string): Promise<{ success: boolean }> {
  return apiRequest('/api/sessions/revoke', {
    method: 'POST',
    body: JSON.stringify({ session_id: sessionId }),
  });
}

export async function getAuditLog(): Promise<{ logs: AuditLogEntry[] }> {
  return apiRequest('/api/audit-log');
}

// ============================================
// Team Management API (Team+ tiers only)
// ============================================

export interface TeamMember {
  id: string;
  machine_id: string;
  hostname: string | null;
  os: string | null;
  arch: string | null;
  omg_version: string | null;
  user_name: string | null;
  user_email: string | null;
  is_active: boolean;
  first_seen_at: string;
  last_seen_at: string;
  total_commands: number;
  total_packages: number;
  total_time_saved_ms: number;
  commands_last_7d: number;
  last_active: string | null;
}

export interface TeamData {
  license: {
    tier: string;
    max_seats: number;
    status: string;
  };
  members: TeamMember[];
  daily_usage: Array<{
    date: string;
    machine_id: string;
    commands_run: number;
    time_saved_ms: number;
  }>;
  totals: {
    total_machines: number;
    active_machines: number;
    total_commands: number;
    total_time_saved_ms: number;
    total_time_saved_hours: number;
    total_value_usd: number;
  };
  fleet_health: {
    compliance_rate: number;
    latest_version: string;
    version_drift: boolean;
  };
  productivity_score: number;
  insights: {
    engagement_rate: number;
    roi_multiplier: string;
  };
}

export async function getTeamMembers(): Promise<TeamData> {
  return apiRequest('/api/team/members');
}

export async function revokeTeamMember(machineId: string): Promise<{ success: boolean }> {
  return apiRequest('/api/team/revoke', {
    method: 'POST',
    body: JSON.stringify({ machine_id: machineId }),
  });
}

export async function createCheckout(email: string, priceId: string): Promise<{ url: string }> {
  return apiRequest('/api/billing/checkout', {
    method: 'POST',
    body: JSON.stringify({ email, priceId }),
  });
}

export async function openBillingPortal(email: string): Promise<{ success: boolean; url: string }> {
  return apiRequest('/api/billing/portal', {
    method: 'POST',
    body: JSON.stringify({ email }),
  });
}

// ============================================
// Team Controls API (Team/Enterprise tiers)
// ============================================

export interface Policy {
  id: string;
  scope: 'runtime' | 'package' | 'security' | 'network';
  rule: string;
  value: string;
  enforced: boolean;
  created_at: string;
}

export interface NotificationSetting {
  type: string;
  enabled: boolean;
  threshold?: number;
  channels: string[];
}

export interface TeamAuditLogEntry {
  id: string;
  action: string;
  resource_type: string | null;
  resource_id: string | null;
  ip_address: string | null;
  user_agent: string | null;
  metadata: Record<string, unknown> | null;
  created_at: string;
}

export async function getTeamPolicies(): Promise<{ policies: Policy[] }> {
  return apiRequest('/api/team/policies');
}

export async function createTeamPolicy(policy: {
  scope: string;
  rule: string;
  value: string;
  enforced?: boolean;
}): Promise<{ success: boolean; policy: Policy }> {
  return apiRequest('/api/team/policies', {
    method: 'POST',
    body: JSON.stringify(policy),
  });
}

export async function updateTeamPolicy(
  id: string,
  updates: { value?: string; enforced?: boolean }
): Promise<{ success: boolean }> {
  return apiRequest('/api/team/policies', {
    method: 'PUT',
    body: JSON.stringify({ id, ...updates }),
  });
}

export async function deleteTeamPolicy(id: string): Promise<{ success: boolean }> {
  return apiRequest('/api/team/policies', {
    method: 'DELETE',
    body: JSON.stringify({ id }),
  });
}

export async function getNotificationSettings(): Promise<{ settings: NotificationSetting[] }> {
  return apiRequest('/api/team/notifications');
}

export async function updateNotificationSettings(
  settings: NotificationSetting[]
): Promise<{ success: boolean }> {
  return apiRequest('/api/team/notifications', {
    method: 'POST',
    body: JSON.stringify({ settings }),
  });
}

export async function revokeTeamMemberAccess(machineId: string): Promise<{ success: boolean }> {
  return apiRequest('/api/team/members/revoke', {
    method: 'POST',
    body: JSON.stringify({ machine_id: machineId }),
  });
}

export async function getTeamAuditLogs(params?: {
  limit?: number;
  offset?: number;
  action?: string;
  resource_type?: string;
}): Promise<{
  logs: TeamAuditLogEntry[];
  total: number;
  limit: number;
  offset: number;
}> {
  const searchParams = new URLSearchParams();
  if (params?.limit) searchParams.set('limit', params.limit.toString());
  if (params?.offset) searchParams.set('offset', params.offset.toString());
  if (params?.action) searchParams.set('action', params.action);
  if (params?.resource_type) searchParams.set('resource_type', params.resource_type);
  return apiRequest(`/api/team/audit-logs?${searchParams}`);
}

export async function updateAlertThreshold(
  thresholdType: string,
  value: number
): Promise<{ success: boolean }> {
  return apiRequest('/api/team/thresholds', {
    method: 'POST',
    body: JSON.stringify({ threshold_type: thresholdType, value }),
  });
}

// ============================================
// Admin API (only accessible to admin user)
// ============================================

export interface AdminOverview {
  overview: {
    total_users: number;
    active_licenses: number;
    active_machines: number;
    total_installs: number;
    total_commands: number;
    mrr: number;
    global_value_usd: number;
    command_health: {
      success: number;
      failure: number;
    };
  };
  fleet: {
    versions: Array<{ omg_version: string; count: number }>;
  };
  tiers: Array<{ tier: string; count: number }>;
  usage: {
    total_commands: number;
    total_packages_installed: number;
    total_searches: number;
    total_time_saved_ms: number;
  };
  daily_active_users: Array<{ date: string; active_users: number; commands: number }>;
  recent_signups: Array<{ date: string; count: number }>;
  installs_by_platform: Array<{ platform: string; count: number }>;
  installs_by_version: Array<{ version: string; count: number }>;
  subscriptions: Array<{ status: string; count: number }>;
  geo_distribution: Array<{ dimension: string; count: number }>;
}

export interface AdminUser {
  id: string;
  email: string;
  company: string | null;
  customer_tier: string;
  created_at: string;
  license_key: string;
  tier: string;
  status: string;
  max_seats: number;
  machine_count: number;
  total_commands: number;
  last_active: string | null;
}

export interface AdminUsersResponse {
  users: AdminUser[];
  pagination: {
    page: number;
    limit: number;
    total: number;
    pages: number;
  };
}

export interface AdminActivity {
  id: string;
  type: 'signup' | 'activation' | 'install' | 'upgrade' | 'command' | 'login';
  description: string;
  user_id: string;
  user_email?: string;
  email?: string;
  hostname?: string;
  platform?: string;
  version?: string;
  timestamp: string;
  created_at: string;
}

export interface AdminHealth {
  active_users_today: number;
  active_users_week: number;
  commands_today: number;
  new_users_today: number;
  installs_today: number;
  timestamp: string;
}

export async function getAdminDashboard(): Promise<AdminOverview> {
  return apiRequest('/api/admin/dashboard');
}

export interface AdminAnalytics {
  request_id: string;
  dau: number;
  wau: number;
  mau: number;
  events_today: number;
  retention_rate: number;
  commands_by_type: Array<{ command: string; count: number }>;
  errors_by_type: Array<{ error_type: string; count: number }>;
  growth: {
    new_users_7d: number;
    new_paid_7d: number;
    growth_rate: number;
  };
  time_saved: {
    total_hours: number;
  };
  funnel: {
    installs: number;
    activated: number;
    power_users: number;
  };
  churn_risk: {
    at_risk_users: number;
  };
}

export async function getAdminAnalytics(): Promise<AdminAnalytics> {
  return apiRequest('/api/admin/analytics');
}

export async function getAdminFirehose(limit = 50): Promise<{ events: any[] }> {
  return apiRequest(`/api/admin/firehose?limit=${limit}`);
}

export async function getAdminUsers(
  page = 1,
  limit = 50,
  search = ''
): Promise<AdminUsersResponse> {
  const params = new URLSearchParams({ page: page.toString(), limit: limit.toString() });
  if (search) params.set('search', search);
  return apiRequest(`/api/admin/users?${params}`);
}

// Rich user detail response with Stripe integration
export interface AdminUserDetail {
  request_id: string;
  user: {
    id: string;
    email: string;
    company: string | null;
    stripe_customer_id: string | null;
    tier: string;
    created_at: string;
    created_at_relative: string;
  };
  license: {
    id: string;
    license_key: string;
    tier: string;
    status: string;
    max_seats: number;
    expires_at: string | null;
  } | null;
  machines: Array<{
    id: string;
    machine_id: string;
    hostname: string;
    os: string;
    arch: string;
    omg_version: string;
    is_active: boolean;
    first_seen_at: string;
    last_seen_at: string;
  }>;
  usage: {
    daily: Array<{
      date: string;
      commands_run: number;
      packages_installed: number;
      time_saved_ms: number;
    }>;
    summary: {
      total_commands: number;
      total_packages: number;
      total_searches: number;
      total_time_saved_ms: number;
      active_days: number;
      first_active: string | null;
      last_active: string | null;
    } | null;
  };
  engagement: {
    commands_last_7d: number;
    commands_last_30d: number;
    active_days_last_30d: number;
    avg_daily_commands: number;
    is_power_user: boolean;
    is_at_risk: boolean;
  };
  ltv: {
    total_paid: number;
    invoice_count: number;
    months_subscribed: number;
  };
}

export async function getAdminUserDetail(userId: string): Promise<AdminUserDetail> {
  return apiRequest(`/api/admin/user?id=${userId}`);
}

export async function updateAdminUser(
  userId: string,
  updates: { tier?: string; max_seats?: number; status?: string }
): Promise<{ success: boolean }> {
  return apiRequest('/api/admin/user', {
    method: 'PUT',
    body: JSON.stringify({ user_id: userId, ...updates }),
  });
}

export async function getAdminActivity(): Promise<{ activity: AdminActivity[] }> {
  return apiRequest('/api/admin/activity');
}

export async function getAdminHealth(): Promise<AdminHealth> {
  return apiRequest('/api/admin/health');
}

// Advanced Analytics
export interface AdminCohorts {
  request_id: string;
  cohorts: Array<{ 
    cohort_week: string; 
    weeks_since_signup: number; 
    active_users: number;
  }>;
}

export interface AdminRevenue {
  request_id: string;
  mrr: number;
  arr: number;
  monthly_revenue: Array<{ month: string; revenue: number; transactions: number }>;
}

export interface AdminAuditLogResponse {
  request_id: string;
  logs: Array<{
    id: string;
    user_id: string;
    user_email: string;
    action: string;
    resource_type: string | null;
    resource_id: string | null;
    ip_address: string | null;
    metadata: string | null;
    created_at: string;
  }>;
  pagination: { page: number; limit: number; total: number; pages: number };
}

export async function getAdminCohorts(): Promise<AdminCohorts> {
  return apiRequest('/api/admin/cohorts');
}

export async function getAdminRevenue(): Promise<AdminRevenue> {
  return apiRequest('/api/admin/revenue');
}

export async function getAdminAuditLog(
  page = 1,
  limit = 50,
  action = ''
): Promise<AdminAuditLogResponse> {
  const params = new URLSearchParams({ page: page.toString(), limit: limit.toString() });
  if (action) params.set('action', action);
  return apiRequest(`/api/admin/audit-log?${params}`);
}

// Data Export (returns download URLs)
export function getAdminExportUsersUrl(): string {
  return `${API_BASE}/api/admin/export/users`;
}

export function getAdminExportUsageUrl(days = 30): string {
  return `${API_BASE}/api/admin/export/usage?days=${days}`;
}

export function getAdminExportAuditUrl(days = 30): string {
  return `${API_BASE}/api/admin/export/audit?days=${days}`;
}

// ============================================
// Docs Analytics API
// ============================================

export interface DocsAnalyticsSummary {
  total_pageviews: number;
  total_sessions: number;
  avg_pages_per_session: string;
  period_days: number;
}

export interface DocsPageview {
  date: string;
  views: number;
  sessions: number;
}

export interface DocsTopPage {
  path: string;
  views: number;
  sessions: number;
  avg_time: number;
}

export interface DocsReferrer {
  referrer: string;
  sessions: number;
  pageviews: number;
}

export interface DocsUTMCampaign {
  utm_source: string | null;
  utm_medium: string | null;
  utm_campaign: string | null;
  sessions: number;
  pageviews: number;
}

export interface DocsGeo {
  country_code: string;
  sessions: number;
  pageviews: number;
}

export interface DocsInteraction {
  interaction_type: string;
  target: string;
  count: number;
}

export interface DocsPerformance {
  path: string;
  avg_load: number;
  p95_load: number;
  samples: number;
}

export interface DocsAnalyticsDashboard {
  summary: DocsAnalyticsSummary;
  pageviews_over_time: DocsPageview[];
  top_pages: DocsTopPage[];
  top_referrers: DocsReferrer[];
  utm_campaigns: DocsUTMCampaign[];
  geographic: DocsGeo[];
  top_interactions: DocsInteraction[];
  performance: DocsPerformance[];
}

export async function getDocsAnalytics(days = 30): Promise<DocsAnalyticsDashboard> {
  return get<DocsAnalyticsDashboard>(`/api/docs/analytics/dashboard?days=${days}`);
}

// ============================================
// AI Insights API
// ============================================

export interface SmartInsight {
  insight: string;
  timestamp: string;
  generated_by: string;
}

export async function getSmartInsights(target: 'user' | 'team' | 'admin' = 'user'): Promise<SmartInsight | null> {
  const token = getSessionToken();
  if (!token) return null;

  try {
    const res = await fetch(`${API_BASE}/api/insights?target=${target}`, {
      headers: { 'Authorization': `Bearer ${token}` }
    });
    if (!res.ok) return null;
    return await res.json();
  } catch (e) {
    console.error('Failed to fetch insights:', e);
    return null;
  }
}

// ============================================
// Helpers
// ============================================

export function formatTimeSaved(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 3600000) return `${(ms / 60000).toFixed(1)}min`;
  if (ms < 86400000) return `${(ms / 3600000).toFixed(1)}hr`;
  return `${(ms / 86400000).toFixed(1)} days`;
}

export function formatDate(dateStr: string | null | undefined): string {
  if (!dateStr) return 'N/A';
  return new Date(dateStr).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return formatDate(dateStr);
}

export function getTierColor(tier: string): string {
  switch (tier) {
    case 'pro':
      return 'from-indigo-500 to-blue-500';
    case 'team':
      return 'from-purple-500 to-pink-500';
    case 'enterprise':
      return 'from-amber-500 to-orange-500';
    default:
      return 'from-emerald-500 to-teal-500';
  }
}

export function getTierBadgeColor(tier: string): string {
  switch (tier) {
    case 'pro':
      return 'bg-indigo-500/20 text-indigo-300 border-indigo-500/30';
    case 'team':
      return 'bg-purple-500/20 text-purple-300 border-purple-500/30';
    case 'enterprise':
      return 'bg-amber-500/20 text-amber-300 border-amber-500/30';
    default:
      return 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30';
  }
}
