import { Component, createSignal, createEffect, Show, For, onMount } from 'solid-js';
import { A } from '@solidjs/router';
import * as api from '../lib/api';
import { TeamAnalytics } from '../components/dashboard/TeamAnalytics';
import { AdminDashboard } from '../components/dashboard/AdminDashboard';
import { SmartInsights } from '../components/dashboard/SmartInsights';
import {
  BarChart3,
  Monitor,
  Users,
  Lock,
  CreditCard,
  Crown,
  Clock,
  Terminal,
  Flame,
  CheckCircle,
  Globe,
} from '../components/ui/Icons';

type View = 'login' | 'verify' | 'dashboard';
type Tab = 'overview' | 'machines' | 'team' | 'security' | 'billing' | 'admin';

const DashboardPage: Component = () => {
  // Auth state
  const [view, setView] = createSignal<View>('login');
  const [email, setEmail] = createSignal('');
  const [otpCode, setOtpCode] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');

  // Dashboard state
  const [dashboard, setDashboard] = createSignal<api.DashboardData | null>(null);
  const [activeTab, setActiveTab] = createSignal<Tab>('overview');
  const [sessions, setSessions] = createSignal<api.Session[]>([]);
  const [auditLog, setAuditLog] = createSignal<api.AuditLogEntry[]>([]);
  const [copied, setCopied] = createSignal(false);
  const [actionMessage, setActionMessage] = createSignal('');

  // Admin state
  const [adminData, setAdminData] = createSignal<api.AdminOverview | null>(null);
  const [adminUsers, setAdminUsers] = createSignal<api.AdminUser[]>([]);
  const [adminActivity, setAdminActivity] = createSignal<api.AdminActivity[]>([]);
  const [adminHealth, setAdminHealth] = createSignal<api.AdminHealth | null>(null);
  const [adminSearch, setAdminSearch] = createSignal('');
  const [adminPage, setAdminPage] = createSignal(1);
  const [adminTotalPages, setAdminTotalPages] = createSignal(1);
  const [selectedUser, setSelectedUser] = createSignal<string | null>(null);
  const [userDetail, setUserDetail] = createSignal<api.AdminUserDetail | null>(null);
  const [userDetailTab, setUserDetailTab] = createSignal<
    'overview' | 'usage' | 'billing' | 'activity'
  >('overview');
  const [adminRevenue, setAdminRevenue] = createSignal<api.AdminRevenue | null>(null);
  const [adminCohorts, setAdminCohorts] = createSignal<api.AdminCohorts | null>(null);
  const [adminAuditLog, setAdminAuditLog] = createSignal<api.AdminAuditLogResponse | null>(null);
  const [adminAnalytics, setAdminAnalytics] = createSignal<api.AdminAnalytics | null>(null);
  const [_adminTab, _setAdminTab] = createSignal<'overview' | 'users' | 'revenue' | 'audit'>(
    'overview'
  );

  // Team state
  const [teamData, setTeamData] = createSignal<api.TeamData | null>(null);
  const [teamLoading, setTeamLoading] = createSignal(false);

  // Check for existing session on mount
  onMount(async () => {
    const token = api.getSessionToken();
    if (token) {
      try {
        const result = await api.verifySession(token);
        if (result.valid && result.user) {
          await loadDashboard();
          setView('dashboard');
        } else {
          api.clearSession();
        }
      } catch {
        api.clearSession();
      }
    }
  });

  // Load dashboard data
  const loadDashboard = async () => {
    try {
      const data = await api.getDashboard();
      setDashboard(data);
    } catch (e) {
      if (e instanceof api.ApiError && e.status === 401) {
        api.clearSession();
        setView('login');
      }
      console.error('Failed to load dashboard:', e);
    }
  };

  // Send OTP code
  const handleSendCode = async () => {
    const userEmail = email().trim().toLowerCase();
    if (!userEmail || !userEmail.includes('@')) {
      setError('Please enter a valid email address');
      return;
    }

    setLoading(true);
    setError('');

    try {
      const result = await api.sendCode(userEmail);
      if (result.success) {
        setView('verify');
      } else {
        setError(result.error || 'Failed to send code');
      }
    } catch (e) {
      setError(e instanceof api.ApiError ? e.message : 'Failed to connect');
    }

    setLoading(false);
  };

  // Verify OTP code
  const handleVerifyCode = async () => {
    const code = otpCode().trim();
    if (code.length !== 6) {
      setError('Please enter the 6-digit code');
      return;
    }

    setLoading(true);
    setError('');

    try {
      const result = await api.verifyCode(email(), code);
      if (result.success && result.token) {
        api.setSessionToken(result.token);
        await loadDashboard();
        setView('dashboard');
      } else {
        setError(result.error || 'Invalid code');
      }
    } catch (e) {
      setError(e instanceof api.ApiError ? e.message : 'Verification failed');
    }

    setLoading(false);
  };

  // Logout
  const handleLogout = async () => {
    await api.logout();
    setDashboard(null);
    setEmail('');
    setOtpCode('');
    setView('login');
  };

  // Copy to clipboard
  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Regenerate license
  const handleRegenerateLicense = async () => {
    if (
      !confirm(
        'This will invalidate your current license key. All machines will need to re-activate. Continue?'
      )
    ) {
      return;
    }

    try {
      const result = await api.regenerateLicense();
      if (result.success) {
        setActionMessage(`New key: ${result.license_key}`);
        await loadDashboard();
      }
    } catch (_e) {
      setActionMessage('Failed to regenerate license');
    }
    setTimeout(() => setActionMessage(''), 5000);
  };

  // Revoke machine
  const handleRevokeMachine = async (machineId: string) => {
    if (!confirm('Revoke access for this machine?')) return;

    try {
      await api.revokeMachine(machineId);
      await loadDashboard();
      setActionMessage('Machine revoked');
    } catch (_e) {
      setActionMessage('Failed to revoke machine');
    }
    setTimeout(() => setActionMessage(''), 3000);
  };

  // Load sessions
  const loadSessions = async () => {
    try {
      const result = await api.getSessions();
      setSessions(result.sessions);
    } catch (_e) {
      // Session loading failed silently
    }
  };

  // Revoke session
  const handleRevokeSession = async (sessionId: string) => {
    try {
      await api.revokeSession(sessionId);
      await loadSessions();
      setActionMessage('Session revoked');
    } catch (_e) {
      setActionMessage('Failed to revoke session');
    }
    setTimeout(() => setActionMessage(''), 3000);
  };

  // Load audit log
  const loadAuditLog = async () => {
    try {
      const result = await api.getAuditLog();
      setAuditLog(result.logs);
    } catch (_e) {
      setAuditLog([]);
    }
  };

  // Open billing portal
  const handleBillingPortal = async () => {
    const d = dashboard();
    if (!d) return;

    try {
      const result = await api.openBillingPortal(d.user.email);
      if (result.url) {
        window.location.href = result.url;
      }
    } catch (_e) {
      setActionMessage('Failed to open billing portal');
      setTimeout(() => setActionMessage(''), 3000);
    }
  };

  // Load admin data
  const loadAdminData = async () => {
    try {
      const [dashboardData, usersData, activityData, healthData, revenueData, cohortsData, analyticsData] =
        await Promise.all([
          api.getAdminDashboard(),
          api.getAdminUsers(adminPage(), 50, adminSearch()),
          api.getAdminActivity(),
          api.getAdminHealth(),
          api.getAdminRevenue(),
          api.getAdminCohorts(),
          api.getAdminAnalytics().catch(() => null),
        ]);
      setAdminData(dashboardData);
      setAdminUsers(usersData.users);
      setAdminTotalPages(usersData.pagination.pages);
      setAdminActivity(activityData.activity);
      setAdminHealth(healthData);
      setAdminRevenue(revenueData);
      setAdminCohorts(cohortsData);
      setAdminAnalytics(analyticsData);
    } catch (_e) {
      // Admin data loading failed
    }
  };

  // Load admin audit log
  const _loadAdminAuditLog = async (page = 1) => {
    try {
      const data = await api.getAdminAuditLog(page, 50);
      setAdminAuditLog(data);
    } catch (_e) {
      // Audit log loading failed
    }
  };

  // Export data with auth header
  const handleExport = async (type: 'users' | 'usage' | 'audit') => {
    const token = api.getSessionToken();
    if (!token) return;

    let url: string;
    switch (type) {
      case 'users':
        url = api.getAdminExportUsersUrl();
        break;
      case 'usage':
        url = api.getAdminExportUsageUrl(30);
        break;
      case 'audit':
        url = api.getAdminExportAuditUrl(30);
        break;
    }

    try {
      const response = await fetch(url, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (!response.ok) throw new Error('Export failed');

      const blob = await response.blob();
      const filename =
        response.headers.get('Content-Disposition')?.match(/filename="(.+)"/)?.[1] ||
        `omg-${type}-export.${type === 'users' ? 'csv' : 'json'}`;

      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = filename;
      a.click();
      URL.revokeObjectURL(a.href);

      setActionMessage(`${type.charAt(0).toUpperCase() + type.slice(1)} exported successfully`);
    } catch (_e) {
      setActionMessage('Export failed');
    }
    setTimeout(() => setActionMessage(''), 3000);
  };

  // Load admin user detail
  const loadUserDetail = async (userId: string) => {
    try {
      const detail = await api.getAdminUserDetail(userId);
      setUserDetail(detail);
      setSelectedUser(userId);
    } catch (e) {
      console.error('Failed to load user detail:', e);
    }
  };

  // Update user from admin panel
  const handleAdminUpdateUser = async (
    userId: string,
    updates: { tier?: string; max_seats?: number; status?: string }
  ) => {
    try {
      await api.updateAdminUser(userId, updates);
      setActionMessage('User updated successfully');
      await loadAdminData();
      if (selectedUser() === userId) {
        await loadUserDetail(userId);
      }
    } catch (e) {
      setActionMessage(e instanceof api.ApiError ? e.message : 'Failed to update user');
    }
    setTimeout(() => setActionMessage(''), 3000);
  };

  // Load team data
  const loadTeamData = async () => {
    setTeamLoading(true);
    try {
      const data = await api.getTeamMembers();
      setTeamData(data);
    } catch (e) {
      console.error('Failed to load team data:', e);
      setTeamData(null);
    }
    setTeamLoading(false);
  };

  // Revoke team member
  const handleRevokeTeamMember = async (machineId: string) => {
    if (!confirm('Revoke access for this team member?')) return;
    try {
      await api.revokeTeamMember(machineId);
      setActionMessage('Team member access revoked');
      await loadTeamData();
    } catch (e) {
      setActionMessage(e instanceof api.ApiError ? e.message : 'Failed to revoke');
    }
    setTimeout(() => setActionMessage(''), 3000);
  };

  // Load tab data when switching
  createEffect(() => {
    const tab = activeTab();
    if (tab === 'security') {
      loadSessions();
      loadAuditLog();
    } else if (tab === 'team') {
      loadTeamData();
    } else if (tab === 'admin' && dashboard()?.is_admin) {
      loadAdminData();
    }
  });

  return (
    <div class="min-h-screen bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950">
      {/* Background effects */}
      <div class="pointer-events-none fixed inset-0 overflow-hidden">
        <div class="absolute top-0 left-1/4 h-96 w-96 rounded-full bg-indigo-500/10 blur-3xl" />
        <div class="absolute right-1/4 bottom-0 h-96 w-96 rounded-full bg-purple-500/10 blur-3xl" />
      </div>

      {/* Header */}
      <header class="relative z-10 border-b border-slate-800/50 backdrop-blur-sm">
        <div class="mx-auto flex max-w-7xl items-center justify-between px-6 py-4">
          <A href="/" class="group flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center overflow-hidden rounded-xl shadow-lg shadow-indigo-500/25 transition-shadow group-hover:shadow-indigo-500/40">
              <img src="/logo-globe.png" alt="OMG Logo" class="h-10 w-10 object-cover" />
            </div>
            <span class="text-xl font-bold text-white">OMG</span>
          </A>

          <Show when={view() === 'dashboard' && dashboard()}>
            <div class="flex items-center gap-4">
              <div class="hidden text-right sm:block">
                <div class="text-sm font-medium text-white">
                  {dashboard()!.user.name || dashboard()!.user.email}
                </div>
                <div class="text-xs text-slate-400">
                  {dashboard()!.license.tier.toUpperCase()} Plan
                </div>
              </div>
              <button
                onClick={handleLogout}
                class="flex items-center gap-2 rounded-lg px-4 py-2 text-sm text-slate-400 transition-all hover:bg-slate-800 hover:text-white"
              >
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"
                  />
                </svg>
                Sign Out
              </button>
            </div>
          </Show>
        </div>
      </header>

      <main class="relative z-10 mx-auto max-w-7xl px-6 py-8">
        {/* Login View */}
        <Show when={view() === 'login'}>
          <div class="mx-auto mt-16 max-w-md">
            <div class="mb-8 text-center">
              <h1 class="mb-2 text-3xl font-bold text-white">Welcome to OMG</h1>
              <p class="text-slate-400">Sign in to access your dashboard</p>
            </div>

            <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-8 backdrop-blur-sm">
              <div class="space-y-6">
                <div>
                  <label class="mb-2 block text-sm font-medium text-slate-300">Email Address</label>
                  <input
                    type="email"
                    value={email()}
                    onInput={e => setEmail(e.currentTarget.value)}
                    onKeyPress={e => e.key === 'Enter' && handleSendCode()}
                    placeholder="you@example.com"
                    class="w-full rounded-xl border border-slate-700 bg-slate-800/50 px-4 py-3 text-white placeholder-slate-500 transition-all focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
                  />
                </div>

                <Show when={error()}>
                  <div class="rounded-xl border border-red-500/30 bg-red-500/10 p-4">
                    <p class="text-sm text-red-400">{error()}</p>
                  </div>
                </Show>

                <button
                  onClick={handleSendCode}
                  disabled={loading()}
                  class="flex w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-indigo-500 to-purple-500 py-3 font-semibold text-white transition-all hover:from-indigo-400 hover:to-purple-400 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {loading() ? (
                    <svg class="h-5 w-5 animate-spin" fill="none" viewBox="0 0 24 24">
                      <circle
                        class="opacity-25"
                        cx="12"
                        cy="12"
                        r="10"
                        stroke="currentColor"
                        stroke-width="4"
                      />
                      <path
                        class="opacity-75"
                        fill="currentColor"
                        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                      />
                    </svg>
                  ) : (
                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
                      />
                    </svg>
                  )}
                  Send Verification Code
                </button>

                <p class="text-center text-xs text-slate-500">
                  We'll send a one-time code to verify your email
                </p>
              </div>
            </div>
          </div>
        </Show>

        {/* Verify View */}
        <Show when={view() === 'verify'}>
          <div class="mx-auto mt-16 max-w-md">
            <div class="mb-8 text-center">
              <div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-purple-500">
                <svg
                  class="h-8 w-8 text-white"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
                  />
                </svg>
              </div>
              <h1 class="mb-2 text-3xl font-bold text-white">Check Your Email</h1>
              <p class="text-slate-400">
                We sent a code to <span class="font-medium text-white">{email()}</span>
              </p>
            </div>

            <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-8 backdrop-blur-sm">
              <div class="space-y-6">
                <div>
                  <label class="mb-2 block text-sm font-medium text-slate-300">
                    Verification Code
                  </label>
                  <input
                    type="text"
                    value={otpCode()}
                    onInput={e => setOtpCode(e.currentTarget.value.replace(/\D/g, '').slice(0, 6))}
                    onKeyPress={e => e.key === 'Enter' && handleVerifyCode()}
                    placeholder="000000"
                    maxLength={6}
                    class="w-full rounded-xl border border-slate-700 bg-slate-800/50 px-4 py-4 text-center font-mono text-2xl tracking-[0.5em] text-white placeholder-slate-600 transition-all focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
                  />
                </div>

                <Show when={error()}>
                  <div class="rounded-xl border border-red-500/30 bg-red-500/10 p-4">
                    <p class="text-sm text-red-400">{error()}</p>
                  </div>
                </Show>

                <button
                  onClick={handleVerifyCode}
                  disabled={loading() || otpCode().length !== 6}
                  class="flex w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-indigo-500 to-purple-500 py-3 font-semibold text-white transition-all hover:from-indigo-400 hover:to-purple-400 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {loading() ? (
                    <svg class="h-5 w-5 animate-spin" fill="none" viewBox="0 0 24 24">
                      <circle
                        class="opacity-25"
                        cx="12"
                        cy="12"
                        r="10"
                        stroke="currentColor"
                        stroke-width="4"
                      />
                      <path
                        class="opacity-75"
                        fill="currentColor"
                        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                      />
                    </svg>
                  ) : (
                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"
                      />
                    </svg>
                  )}
                  Verify Code
                </button>

                <div class="flex items-center justify-between text-sm">
                  <button
                    onClick={() => {
                      setView('login');
                      setOtpCode('');
                      setError('');
                    }}
                    class="text-slate-400 transition-colors hover:text-white"
                  >
                    ← Change email
                  </button>
                  <button
                    onClick={handleSendCode}
                    disabled={loading()}
                    class="text-indigo-400 transition-colors hover:text-indigo-300"
                  >
                    Resend code
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Show>

        {/* Dashboard View */}
        <Show when={view() === 'dashboard' && dashboard()}>
          {(() => {
            const d = dashboard()!;
            return (
              <div class="space-y-8">
                {/* Action Message */}
                <Show when={actionMessage()}>
                  <div class="animate-in slide-in-from-right fixed top-20 right-6 z-50 rounded-xl border border-slate-700 bg-slate-800 p-4 shadow-xl">
                    <p class="text-sm text-white">{actionMessage()}</p>
                  </div>
                </Show>

                {/* Header */}
                <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
                  <div>
                    <h1 class="text-2xl font-bold text-white">Dashboard</h1>
                    <p class="text-slate-400">
                      Welcome back, {d.user.name || d.user.email.split('@')[0]}
                    </p>
                  </div>
                  <div class="flex items-center gap-3">
                    <span
                      class={`rounded-full border px-4 py-2 text-sm font-semibold uppercase ${api.getTierBadgeColor(d.license.tier)}`}
                    >
                      {d.license.tier}
                    </span>
                    <Show when={d.license.tier === 'free'}>
                      <A
                        href="/#pricing"
                        class="rounded-full bg-gradient-to-r from-indigo-500 to-purple-500 px-4 py-2 text-sm font-medium text-white transition-all hover:from-indigo-400 hover:to-purple-400"
                      >
                        Upgrade
                      </A>
                    </Show>
                  </div>
                </div>

                {/* Tabs */}
                <div role="tablist" class="flex w-fit flex-wrap gap-1 rounded-xl bg-slate-800/50 p-1">
                  <For each={[
                    { id: 'overview' as const, label: 'Overview', Icon: BarChart3 },
                    { id: 'machines' as const, label: 'Machines', Icon: Monitor },
                    { id: 'team' as const, label: 'Team', Icon: Users },
                    { id: 'security' as const, label: 'Security', Icon: Lock },
                    { id: 'billing' as const, label: 'Billing', Icon: CreditCard },
                  ]}>{tab => (
                    <button
                      role="tab"
                      aria-selected={activeTab() === tab.id}
                      aria-controls={`panel-${tab.id}`}
                      id={`tab-${tab.id}`}
                      onClick={() => setActiveTab(tab.id as Tab)}
                      class={`flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-all focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-slate-900 ${
                        activeTab() === tab.id
                          ? 'bg-slate-700 text-white shadow-sm'
                          : 'text-slate-400 hover:text-white'
                      }`}
                    >
                      <tab.Icon size={16} />
                      {tab.label}
                    </button>
                  )}</For>
                  {/* Admin tab - only visible to admin */}
                  <Show when={d.is_admin}>
                    <button
                      role="tab"
                      aria-selected={activeTab() === 'admin'}
                      onClick={() => setActiveTab('admin')}
                      class={`flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-all focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 focus:ring-offset-slate-900 ${
                        activeTab() === 'admin'
                          ? 'bg-gradient-to-r from-red-600 to-orange-600 text-white shadow-sm'
                          : 'border border-red-500/30 text-red-400 hover:text-red-300'
                      }`}
                    >
                      <Crown size={16} />
                      Admin
                    </button>
                  </Show>
                </div>

                {/* Overview Tab */}
                <Show when={activeTab() === 'overview'}>
                  <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
                    {/* Stats Cards */}
                    <div class="rounded-2xl border border-emerald-500/20 bg-gradient-to-br from-emerald-500/10 to-teal-500/10 p-6">
                      <div class="mb-3 flex items-center gap-3">
                        <div class="rounded-lg bg-emerald-500/20 p-2">
                          <Clock size={20} class="text-emerald-400" />
                        </div>
                        <span class="text-sm text-slate-400">Time Saved</span>
                      </div>
                      <div class="text-3xl font-bold text-white tabular-nums">
                        {api.formatTimeSaved(d.usage.total_time_saved_ms)}
                      </div>
                    </div>

                    <div class="rounded-2xl border border-cyan-500/20 bg-gradient-to-br from-cyan-500/10 to-blue-500/10 p-6">
                      <div class="mb-3 flex items-center gap-3">
                        <div class="rounded-lg bg-cyan-500/20 p-2">
                          <Terminal size={20} class="text-cyan-400" />
                        </div>
                        <span class="text-sm text-slate-400">Commands Run</span>
                      </div>
                      <div class="text-3xl font-bold text-white tabular-nums">
                        {d.usage.total_commands.toLocaleString()}
                      </div>
                    </div>

                    <div class="rounded-2xl border border-orange-500/20 bg-gradient-to-br from-orange-500/10 to-amber-500/10 p-6">
                      <div class="mb-3 flex items-center gap-3">
                        <div class="rounded-lg bg-orange-500/20 p-2">
                          <Flame size={20} class="text-orange-400" />
                        </div>
                        <span class="text-sm text-slate-400">Current Streak</span>
                      </div>
                      <div class="text-3xl font-bold text-white tabular-nums">{d.usage.current_streak} days</div>
                    </div>

                    <div class="rounded-2xl border border-purple-500/20 bg-gradient-to-br from-purple-500/10 to-pink-500/10 p-6">
                      <div class="mb-3 flex items-center gap-3">
                        <div class="rounded-lg bg-purple-500/20 p-2">
                          <Monitor size={20} class="text-purple-400" />
                        </div>
                        <span class="text-sm text-slate-400">Active Machines</span>
                      </div>
                      <div class="text-3xl font-bold text-white tabular-nums">
                        {d.machines.length} / {d.license.max_machines}
                      </div>
                    </div>
                  </div>

                  <SmartInsights target="user" />

                  {/* Telemetry & Global Insights */}
                  <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <div class="mb-6 flex items-center justify-between">
                      <div class="flex items-center gap-3">
                        <div class="rounded-lg bg-indigo-500/20 p-2">
                          <Globe size={20} class="text-indigo-400" />
                        </div>
                        <div>
                          <h3 class="text-lg font-semibold text-white">Global Telemetry</h3>
                          <p class="text-xs text-slate-400">Insights from the OMG community</p>
                        </div>
                      </div>
                      <Show when={d.usage.total_commands > 100}>
                        <span class="rounded-full bg-indigo-500/10 px-3 py-1 text-[10px] font-bold uppercase tracking-widest text-indigo-400 ring-1 ring-indigo-500/20">
                          Data Rich
                        </span>
                      </Show>
                    </div>
                    
                    <div class="grid grid-cols-1 gap-6 md:grid-cols-3">
                      <div class="rounded-xl bg-slate-800/50 p-4">
                        <div class="mb-2 text-xs font-bold text-slate-500 uppercase tracking-wider">Top Package</div>
                        <div class="flex items-center justify-between">
                          <span class="font-mono text-white">ripgrep</span>
                          <span class="text-xs text-emerald-400">Trending</span>
                        </div>
                        <div class="mt-2 h-1 w-full rounded-full bg-slate-700">
                          <div class="h-full w-[85%] rounded-full bg-emerald-500" />
                        </div>
                      </div>
                      <div class="rounded-xl bg-slate-800/50 p-4">
                        <div class="mb-2 text-xs font-bold text-slate-500 uppercase tracking-wider">Fastest Runtime</div>
                        <div class="flex items-center justify-between">
                          <span class="font-mono text-white">Bun</span>
                          <span class="text-xs text-indigo-400">4ms startup</span>
                        </div>
                        <div class="mt-2 h-1 w-full rounded-full bg-slate-700">
                          <div class="h-full w-[92%] rounded-full bg-indigo-500" />
                        </div>
                      </div>
                      <div class="rounded-xl bg-slate-800/50 p-4">
                        <div class="mb-2 text-xs font-bold text-slate-500 uppercase tracking-wider">Your Contribution</div>
                        <div class="flex items-center justify-between">
                          <span class="font-bold text-white">Top 5%</span>
                          <span class="text-xs text-purple-400">Power User</span>
                        </div>
                        <div class="mt-2 text-[10px] text-slate-500">
                          Your telemetry helps improve OMG for everyone.
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Usage Activity Chart */}
                  <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <div class="mb-6 flex items-center justify-between">
                      <h3 class="text-lg font-semibold text-white">Activity (Last 14 Days)</h3>
                      <div class="flex items-center gap-4 text-xs">
                        <div class="flex items-center gap-2">
                          <div class="h-3 w-3 rounded-full bg-indigo-500" />
                          <span class="text-slate-400">Commands</span>
                        </div>
                        <div class="flex items-center gap-2">
                          <div class="h-3 w-3 rounded-full bg-emerald-500" />
                          <span class="text-slate-400">Time Saved</span>
                        </div>
                      </div>
                    </div>
                    <div class="flex h-40 items-end gap-1">
                      <For
                        each={
                          d.usage.daily.length > 0
                            ? d.usage.daily
                            : Array(14).fill({ date: '', commands_run: 0, time_saved_ms: 0 })
                        }
                      >
                        {(day, _i) => {
                          const maxCommands = Math.max(
                            ...d.usage.daily.map(d => d.commands_run || 0),
                            1
                          );
                          const height = day.commands_run
                            ? Math.max((day.commands_run / maxCommands) * 100, 5)
                            : 5;
                          return (
                            <div class="group relative flex flex-1 flex-col items-center gap-1">
                              <div
                                class="w-full rounded-t-sm bg-gradient-to-t from-indigo-600 to-indigo-400 transition-all hover:from-indigo-500 hover:to-indigo-300"
                                style={{ height: `${height}%`, 'min-height': '4px' }}
                              />
                              <span class="w-full truncate text-center text-[10px] text-slate-500">
                                {day.date
                                  ? new Date(day.date)
                                      .toLocaleDateString('en-US', { weekday: 'short' })
                                      .slice(0, 2)
                                  : ''}
                              </span>
                              <Show when={day.commands_run > 0}>
                                <div class="pointer-events-none absolute bottom-full left-1/2 z-10 mb-2 -translate-x-1/2 rounded bg-slate-800 px-2 py-1 text-xs whitespace-nowrap text-white opacity-0 transition-opacity group-hover:opacity-100">
                                  {day.commands_run} commands
                                  <br />
                                  {api.formatTimeSaved(day.time_saved_ms)} saved
                                </div>
                              </Show>
                            </div>
                          );
                        }}
                      </For>
                    </div>
                  </div>

                  {/* Detailed Stats Breakdown */}
                  <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6 lg:col-span-2">
                      <h3 class="mb-6 text-lg font-semibold text-white">Usage Breakdown (30d)</h3>
                      <div class="grid grid-cols-2 gap-6 sm:grid-cols-4">
                        <div class="space-y-2">
                          <div class="text-xs text-slate-500 uppercase tracking-wider">Searches</div>
                          <div class="text-2xl font-bold text-white">{(d.usage as any).breakdown?.searched || 0}</div>
                          <div class="h-1.5 w-full rounded-full bg-slate-800">
                            <div class="h-full rounded-full bg-indigo-500" style={{ width: `${Math.min(((d.usage as any).breakdown?.searched || 0) / (d.usage.total_commands || 1) * 100, 100)}%` }} />
                          </div>
                        </div>
                        <div class="space-y-2">
                          <div class="text-xs text-slate-500 uppercase tracking-wider">Installs</div>
                          <div class="text-2xl font-bold text-white">{(d.usage as any).breakdown?.installed || 0}</div>
                          <div class="h-1.5 w-full rounded-full bg-slate-800">
                            <div class="h-full rounded-full bg-emerald-500" style={{ width: `${Math.min(((d.usage as any).breakdown?.installed || 0) / (d.usage.total_commands || 1) * 100, 100)}%` }} />
                          </div>
                        </div>
                        <div class="space-y-2">
                          <div class="text-xs text-slate-500 uppercase tracking-wider">Runtimes</div>
                          <div class="text-2xl font-bold text-white">{(d.usage as any).breakdown?.switched || 0}</div>
                          <div class="h-1.5 w-full rounded-full bg-slate-800">
                            <div class="h-full rounded-full bg-cyan-500" style={{ width: `${Math.min(((d.usage as any).breakdown?.switched || 0) / (d.usage.total_commands || 1) * 100, 100)}%` }} />
                          </div>
                        </div>
                        <div class="space-y-2">
                          <div class="text-xs text-slate-500 uppercase tracking-wider">Security</div>
                          <div class="text-2xl font-bold text-white">{(d.usage as any).breakdown?.sbom || 0}</div>
                          <div class="h-1.5 w-full rounded-full bg-slate-800">
                            <div class="h-full rounded-full bg-purple-500" style={{ width: `${Math.min(((d.usage as any).breakdown?.sbom || 0) / (d.usage.total_commands || 1) * 100, 100)}%` }} />
                          </div>
                        </div>
                      </div>
                    </div>

                    <div class="flex flex-col justify-center rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                      <div class="mb-4 text-center">
                        <div class="text-sm text-slate-400">Efficiency Score</div>
                        <div class="text-5xl font-black text-emerald-400">
                          {Math.min(99, Math.round((d.usage.total_time_saved_ms / 3600000) * 10 + 50))}%
                        </div>
                      </div>
                      <p class="text-center text-xs text-slate-500 leading-relaxed">
                        Your efficiency is calculated based on time saved vs traditional tools. 
                        Keep using OMG to increase your score!
                      </p>
                    </div>
                  </div>

                  {/* Detailed Stats */}
                  <div class="grid grid-cols-2 gap-4 md:grid-cols-4">
                    <div class="rounded-xl border border-slate-800 bg-slate-900/50 p-4">
                      <div class="mb-1 text-xs text-slate-400">Total Packages</div>
                      <div class="text-xl font-bold text-white">
                        {d.usage.total_packages_installed.toLocaleString()}
                      </div>
                    </div>
                    <div class="rounded-xl border border-slate-800 bg-slate-900/50 p-4">
                      <div class="mb-1 text-xs text-slate-400">SBOMs Generated</div>
                      <div class="text-xl font-bold text-white">
                        {d.usage.total_sbom_generated.toLocaleString()}
                      </div>
                    </div>
                    <div class="rounded-xl border border-slate-800 bg-slate-900/50 p-4">
                      <div class="mb-1 text-xs text-slate-400">Vulnerabilities Found</div>
                      <div class="text-xl font-bold text-amber-400">
                        {d.usage.total_vulnerabilities_found.toLocaleString()}
                      </div>
                    </div>
                    <div class="rounded-xl border border-slate-800 bg-slate-900/50 p-4">
                      <div class="mb-1 text-xs text-slate-400">Longest Streak</div>
                      <div class="text-xl font-bold text-white">{d.usage.longest_streak} days</div>
                    </div>
                  </div>

                  {/* License Key */}
                  <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <h3 class="mb-4 text-lg font-semibold text-white">License Key</h3>
                    <div class="flex items-center gap-4">
                      <code class="flex-1 overflow-x-auto rounded-xl bg-slate-800 px-4 py-3 font-mono text-sm text-slate-300">
                        {d.license.license_key}
                      </code>
                      <button
                        onClick={() => copyToClipboard(d.license.license_key)}
                        class="rounded-xl bg-slate-800 px-4 py-3 transition-colors hover:bg-slate-700"
                      >
                        {copied() ? '✓' : '📋'}
                      </button>
                    </div>
                    <p class="mt-3 text-sm text-slate-500">
                      Activate with:{' '}
                      <code class="text-indigo-400">
                        omg license activate {d.license.license_key}
                      </code>
                    </p>
                  </div>

                  {/* Achievements & Leaderboard */}
                  <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
                    <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6 lg:col-span-2">
                      <h3 class="mb-4 text-lg font-semibold text-white">Achievements</h3>
                      <div class="grid grid-cols-2 gap-4 sm:grid-cols-4 md:grid-cols-6">
                        <For each={d.achievements}>
                          {a => (
                            <div 
                              class={`group relative flex flex-col items-center justify-center rounded-xl p-3 transition-all duration-300 ${a.unlocked ? 'bg-indigo-500/10 grayscale-0' : 'bg-slate-800/20 grayscale'}`}
                              title={a.description}
                            >
                              <div class={`text-3xl mb-2 transition-transform duration-300 group-hover:scale-110 ${a.unlocked ? 'drop-shadow-[0_0_8px_rgba(99,102,241,0.5)]' : ''}`}>
                                {a.emoji}
                              </div>
                              <div class="text-[10px] font-bold text-slate-400 text-center uppercase tracking-tighter leading-tight">
                                {a.name}
                              </div>
                              <Show when={a.unlocked}>
                                <div class="absolute -top-1 -right-1 flex h-4 w-4 items-center justify-center rounded-full bg-emerald-500 text-[8px] text-white">
                                  <CheckCircle size={10} />
                                </div>
                              </Show>
                            </div>
                          )}
                        </For>
                      </div>
                    </div>

                    <div class="rounded-2xl border border-amber-500/20 bg-amber-500/5 p-6">
                      <div class="mb-4 flex items-center justify-between">
                        <h3 class="text-lg font-semibold text-white">Top Savers</h3>
                        <Crown size={18} class="text-amber-400" />
                      </div>
                      <div class="space-y-4">
                        <For each={(d as any).leaderboard || []}>
                          {(row: any, i) => (
                            <div class="flex items-center justify-between">
                              <div class="flex items-center gap-3">
                                <div class={`flex h-6 w-6 items-center justify-center rounded-full text-[10px] font-bold ${i() === 0 ? 'bg-amber-400 text-amber-950' : i() === 1 ? 'bg-slate-300 text-slate-900' : 'bg-amber-700 text-amber-100'}`}>
                                  {i() + 1}
                                </div>
                                <span class="text-sm font-medium text-slate-300">{row.user}</span>
                              </div>
                              <span class="text-xs font-bold text-white">{Math.round(row.time_saved / 3600000)}h saved</span>
                            </div>
                          )}
                        </For>
                        <div class="pt-4 border-t border-slate-800">
                          <div class="flex items-center justify-between text-xs">
                            <span class="text-slate-500">Your Rank</span>
                            <span class="font-bold text-indigo-400">#42 (Global)</span>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </Show>

                {/* Machines Tab */}
                <Show when={activeTab() === 'machines'}>
                  <div class="overflow-hidden rounded-2xl border border-slate-800 bg-slate-900/50">
                    <div class="flex items-center justify-between border-b border-slate-800 p-6">
                      <h3 class="text-lg font-semibold text-white">Active Machines</h3>
                      <span class="text-sm text-slate-400">
                        {d.machines.length} / {d.license.max_machines} slots used
                      </span>
                    </div>
                    <Show
                      when={d.machines.length > 0}
                      fallback={
                        <div class="p-12 text-center">
                          <div class="mb-4 text-4xl">💻</div>
                          <p class="text-slate-400">No machines activated yet</p>
                          <p class="mt-2 text-sm text-slate-500">
                            Run <code class="text-indigo-400">omg license activate</code> to get
                            started
                          </p>
                        </div>
                      }
                    >
                      <div class="divide-y divide-slate-800">
                        <For each={d.machines}>
                          {machine => (
                            <div class="flex items-center justify-between p-4 transition-colors hover:bg-slate-800/30">
                              <div class="flex items-center gap-4">
                                <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-slate-800">
                                  <span class="text-xl">💻</span>
                                </div>
                                <div>
                                  <div class="font-medium text-white">
                                    {machine.hostname || machine.machine_id.slice(0, 8)}
                                  </div>
                                  <div class="text-sm text-slate-500">
                                    {machine.os} • Last seen{' '}
                                    {api.formatRelativeTime(machine.last_seen_at)}
                                  </div>
                                </div>
                              </div>
                              <button
                                onClick={() => handleRevokeMachine(machine.id)}
                                class="rounded-lg px-3 py-1.5 text-sm text-red-400 transition-colors hover:bg-red-500/10 hover:text-red-300"
                              >
                                Revoke
                              </button>
                            </div>
                          )}
                        </For>
                      </div>
                    </Show>
                  </div>

                  {/* Regenerate License */}
                  <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <h3 class="mb-2 text-lg font-semibold text-white">Regenerate License Key</h3>
                    <p class="mb-4 text-sm text-slate-400">
                      Generate a new license key. This will invalidate your current key and require
                      all machines to re-activate.
                    </p>
                    <button
                      onClick={handleRegenerateLicense}
                      class="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-2 text-sm text-red-400 transition-colors hover:bg-red-500/20"
                    >
                      Regenerate Key
                    </button>
                  </div>
                </Show>

                {/* Team Tab - Enhanced Analytics */}
                <Show when={activeTab() === 'team'}>
                  <TeamAnalytics
                    teamData={teamData()}
                    licenseKey={d.license.license_key}
                    onRevoke={handleRevokeTeamMember}
                    onRefresh={loadTeamData}
                    loading={teamLoading()}
                  />
                </Show>

                {/* Security Tab */}
                <Show when={activeTab() === 'security'}>
                  {/* Active Sessions */}
                  <div class="overflow-hidden rounded-2xl border border-slate-800 bg-slate-900/50">
                    <div class="border-b border-slate-800 p-6">
                      <h3 class="text-lg font-semibold text-white">Active Sessions</h3>
                    </div>
                    <div class="divide-y divide-slate-800">
                      <For each={sessions()}>
                        {session => (
                          <div class="flex items-center justify-between p-4 transition-colors hover:bg-slate-800/30">
                            <div class="flex items-center gap-4">
                              <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-slate-800">
                                <span class="text-xl">🌐</span>
                              </div>
                              <div>
                                <div class="flex items-center gap-2 font-medium text-white">
                                  {session.ip_address || 'Unknown IP'}
                                  {session.is_current && (
                                    <span class="rounded-full bg-emerald-500/20 px-2 py-0.5 text-xs text-emerald-400">
                                      Current
                                    </span>
                                  )}
                                </div>
                                <div class="max-w-md truncate text-sm text-slate-500">
                                  {session.user_agent?.slice(0, 60) || 'Unknown device'}
                                </div>
                              </div>
                            </div>
                            <Show when={!session.is_current}>
                              <button
                                onClick={() => handleRevokeSession(session.id)}
                                class="rounded-lg px-3 py-1.5 text-sm text-red-400 transition-colors hover:bg-red-500/10 hover:text-red-300"
                              >
                                Revoke
                              </button>
                            </Show>
                          </div>
                        )}
                      </For>
                    </div>
                  </div>

                  {/* Audit Log (Team+ only) */}
                  <Show when={['team', 'enterprise'].includes(d.license.tier)}>
                    <div class="overflow-hidden rounded-2xl border border-slate-800 bg-slate-900/50">
                      <div class="border-b border-slate-800 p-6">
                        <h3 class="text-lg font-semibold text-white">Audit Log</h3>
                      </div>
                      <Show
                        when={auditLog().length > 0}
                        fallback={
                          <div class="p-8 text-center text-slate-400">No audit events yet</div>
                        }
                      >
                        <div class="max-h-96 divide-y divide-slate-800 overflow-y-auto">
                          <For each={auditLog()}>
                            {entry => (
                              <div class="flex items-center justify-between p-4 text-sm">
                                <div>
                                  <span class="text-white">{entry.action}</span>
                                  <Show when={entry.resource_type}>
                                    <span class="text-slate-500"> on {entry.resource_type}</span>
                                  </Show>
                                </div>
                                <div class="text-slate-500">
                                  {api.formatRelativeTime(entry.created_at)}
                                </div>
                              </div>
                            )}
                          </For>
                        </div>
                      </Show>
                    </div>
                  </Show>
                </Show>

                {/* Billing Tab */}
                <Show when={activeTab() === 'billing'}>
                  {/* Current Plan */}
                  <div class="rounded-2xl border border-slate-800 bg-slate-900/50 p-6">
                    <div class="mb-6 flex items-center justify-between">
                      <div>
                        <h3 class="text-lg font-semibold text-white">Current Plan</h3>
                        <p class="text-sm text-slate-400">
                          {d.license.tier === 'free'
                            ? 'Free forever'
                            : d.subscription
                              ? `Renews ${api.formatDate(d.subscription.current_period_end)}`
                              : 'Active'}
                        </p>
                      </div>
                      <span
                        class={`rounded-full border px-4 py-2 text-lg font-bold uppercase ${api.getTierBadgeColor(d.license.tier)}`}
                      >
                        {d.license.tier}
                      </span>
                    </div>

                    <div class="flex gap-3">
                      <Show when={d.license.tier === 'free'}>
                        <A
                          href="/#pricing"
                          class="rounded-lg bg-gradient-to-r from-indigo-500 to-purple-500 px-4 py-2 font-medium text-white transition-all hover:from-indigo-400 hover:to-purple-400"
                        >
                          Upgrade Plan
                        </A>
                      </Show>
                      <Show when={d.license.tier !== 'free'}>
                        <button
                          onClick={handleBillingPortal}
                          class="rounded-lg bg-slate-800 px-4 py-2 text-white transition-colors hover:bg-slate-700"
                        >
                          Manage Subscription
                        </button>
                      </Show>
                    </div>
                  </div>

                  {/* Invoices */}
                  <Show when={d.invoices.length > 0}>
                    <div class="overflow-hidden rounded-2xl border border-slate-800 bg-slate-900/50">
                      <div class="border-b border-slate-800 p-6">
                        <h3 class="text-lg font-semibold text-white">Billing History</h3>
                      </div>
                      <div class="divide-y divide-slate-800">
                        <For each={d.invoices}>
                          {invoice => (
                            <div class="flex items-center justify-between p-4">
                              <div>
                                <div class="font-medium text-white">
                                  ${(invoice.amount_cents / 100).toFixed(2)}{' '}
                                  {invoice.currency.toUpperCase()}
                                </div>
                                <div class="text-sm text-slate-500">
                                  {api.formatDate(invoice.created_at)}
                                </div>
                              </div>
                              <div class="flex items-center gap-3">
                                <span
                                  class={`rounded px-2 py-1 text-xs ${
                                    invoice.status === 'paid'
                                      ? 'bg-emerald-500/20 text-emerald-400'
                                      : 'bg-slate-700 text-slate-400'
                                  }`}
                                >
                                  {invoice.status}
                                </span>
                                <Show when={invoice.invoice_pdf}>
                                  <a
                                    href={invoice.invoice_pdf!}
                                    target="_blank"
                                    class="text-sm text-indigo-400 hover:text-indigo-300"
                                  >
                                    PDF
                                  </a>
                                </Show>
                              </div>
                            </div>
                          )}
                        </For>
                      </div>
                    </div>
                  </Show>
                </Show>

                {/* Admin Tab - Enhanced Dashboard */}
                <Show when={activeTab() === 'admin' && d.is_admin}>
                  <div class="space-y-6">
                    <AdminDashboard
                      adminData={adminData()}
                      adminUsers={adminUsers()}
                      adminHealth={adminHealth()}
                      adminRevenue={adminRevenue()}
                      adminCohorts={adminCohorts()}
                      adminActivity={adminActivity()}
                      adminAuditLog={adminAuditLog()}
                      adminAnalytics={adminAnalytics()}
                      onRefresh={loadAdminData}
                      onUserClick={loadUserDetail}
                      onExport={handleExport}
                      onSearch={(query) => {
                        setAdminSearch(query);
                        setAdminPage(1);
                        loadAdminData();
                      }}
                      onPageChange={(page) => {
                        setAdminPage(page);
                        loadAdminData();
                      }}
                      currentPage={adminPage()}
                      totalPages={adminTotalPages()}
                      searchQuery={adminSearch()}
                    />

                    {/* Enhanced User Detail Modal */}
                    <Show when={selectedUser() && userDetail()}>
                      {(() => {
                        const detail = userDetail()!;
                        return (
                          <div
                            class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
                            onClick={() => {
                              setSelectedUser(null);
                              setUserDetailTab('overview');
                            }}
                          >
                            <div
                              class="flex max-h-[90vh] w-full max-w-5xl flex-col overflow-hidden rounded-2xl border border-slate-700 bg-slate-900 shadow-2xl"
                              onClick={e => e.stopPropagation()}
                            >
                              {/* Header */}
                              <div class="flex items-center justify-between border-b border-slate-800 bg-slate-900/80 p-6">
                                <div class="flex items-center gap-4">
                                  <div class="flex h-12 w-12 items-center justify-center rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 text-xl font-bold text-white">
                                    {detail.user.email.charAt(0).toUpperCase()}
                                  </div>
                                  <div>
                                    <h3 class="text-lg font-semibold text-white">
                                      {detail.user.email}
                                    </h3>
                                    <div class="flex items-center gap-2 text-sm text-slate-400">
                                      <span>Joined {detail.user.created_at_relative}</span>
                                      {detail.engagement.is_power_user && (
                                        <span class="rounded-full bg-amber-500/20 px-2 py-0.5 text-xs text-amber-400">
                                          ⚡ Power User
                                        </span>
                                      )}
                                      {detail.engagement.is_at_risk && (
                                        <span class="rounded-full bg-red-500/20 px-2 py-0.5 text-xs text-red-400">
                                          ⚠️ At Risk
                                        </span>
                                      )}
                                    </div>
                                  </div>
                                </div>
                                <button
                                  onClick={() => {
                                    setSelectedUser(null);
                                    setUserDetailTab('overview');
                                  }}
                                  class="rounded-lg p-2 text-slate-400 transition-colors hover:bg-slate-800 hover:text-white"
                                >
                                  ✕
                                </button>
                              </div>

                              {/* Tabs */}
                              <div class="flex gap-1 border-b border-slate-800 bg-slate-900/50 px-6">
                                <For
                                  each={
                                    [
                                      { id: 'overview', label: 'Overview', icon: '📊' },
                                      { id: 'usage', label: 'Usage', icon: '📈' },
                                      { id: 'billing', label: 'Billing', icon: '💳' },
                                      { id: 'activity', label: 'Activity', icon: '📋' },
                                    ] as const
                                  }
                                >
                                  {tab => (
                                    <button
                                      onClick={() => setUserDetailTab(tab.id)}
                                      class={`flex items-center gap-2 border-b-2 px-4 py-3 text-sm font-medium transition-colors ${
                                        userDetailTab() === tab.id
                                          ? 'border-indigo-500 text-white'
                                          : 'border-transparent text-slate-400 hover:text-white'
                                      }`}
                                    >
                                      <span>{tab.icon}</span>
                                      {tab.label}
                                    </button>
                                  )}
                                </For>
                              </div>

                              {/* Content */}
                              <div class="flex-1 overflow-y-auto p-6">
                                {/* Overview Tab */}
                                <Show when={userDetailTab() === 'overview'}>
                                  <div class="space-y-6">
                                    {/* Key Metrics */}
                                    <div class="grid grid-cols-4 gap-4">
                                      <div class="rounded-xl bg-slate-800/50 p-4">
                                        <div class="text-2xl font-bold text-white">
                                          {(
                                            detail.usage.summary?.total_commands || 0
                                          ).toLocaleString()}
                                        </div>
                                        <div class="text-sm text-slate-400">Total Commands</div>
                                      </div>
                                      <div class="rounded-xl bg-slate-800/50 p-4">
                                        <div class="text-2xl font-bold text-emerald-400">
                                          {Math.round(
                                            (detail.usage.summary?.total_time_saved_ms || 0) / 60000
                                          )}
                                          m
                                        </div>
                                        <div class="text-sm text-slate-400">Time Saved</div>
                                      </div>
                                      <div class="rounded-xl bg-slate-800/50 p-4">
                                        <div class="text-2xl font-bold text-indigo-400">
                                          {detail.engagement.active_days_last_30d}
                                        </div>
                                        <div class="text-sm text-slate-400">Active Days (30d)</div>
                                      </div>
                                      <div class="rounded-xl bg-slate-800/50 p-4">
                                        <div class="text-2xl font-bold text-amber-400">
                                          ${detail.ltv.total_paid.toFixed(2)}
                                        </div>
                                        <div class="text-sm text-slate-400">Lifetime Value</div>
                                      </div>
                                    </div>

                                    {/* License & Machines */}
                                    <div class="grid grid-cols-2 gap-6">
                                      <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                        <h4 class="mb-3 font-medium text-white">License</h4>
                                        <div class="space-y-2 text-sm">
                                          <div class="flex justify-between">
                                            <span class="text-slate-400">Key</span>
                                            <span class="font-mono text-white">
                                              {detail.license?.license_key.slice(0, 8)}...
                                            </span>
                                          </div>
                                          <div class="flex justify-between">
                                            <span class="text-slate-400">Tier</span>
                                            <span
                                              class={`rounded-full px-2 py-0.5 text-xs font-medium ${api.getTierBadgeColor(detail.license?.tier || 'free')}`}
                                            >
                                              {(detail.license?.tier || 'free').toUpperCase()}
                                            </span>
                                          </div>
                                          <div class="flex justify-between">
                                            <span class="text-slate-400">Status</span>
                                            <span
                                              class={`text-xs ${detail.license?.status === 'active' ? 'text-emerald-400' : 'text-red-400'}`}
                                            >
                                              {detail.license?.status || 'active'}
                                            </span>
                                          </div>
                                          <div class="flex justify-between">
                                            <span class="text-slate-400">Max Seats</span>
                                            <span class="text-white">
                                              {detail.license?.max_seats || 1}
                                            </span>
                                          </div>
                                        </div>
                                      </div>

                                      <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                        <h4 class="mb-3 font-medium text-white">
                                          Machines ({detail.machines.length})
                                        </h4>
                                        <div class="max-h-40 space-y-2 overflow-y-auto">
                                          <For each={detail.machines}>
                                            {machine => (
                                              <div class="flex items-center justify-between rounded-lg bg-slate-800 p-2 text-sm">
                                                <div>
                                                  <div class="text-white">
                                                    {machine.hostname || machine.machine_id}
                                                  </div>
                                                  <div class="text-xs text-slate-500">
                                                    {machine.os} • v{machine.omg_version}
                                                  </div>
                                                </div>
                                                <div
                                                  class={`h-2 w-2 rounded-full ${machine.is_active ? 'bg-emerald-400' : 'bg-slate-600'}`}
                                                />
                                              </div>
                                            )}
                                          </For>
                                          <Show when={detail.machines.length === 0}>
                                            <div class="text-sm text-slate-500">
                                              No machines activated
                                            </div>
                                          </Show>
                                        </div>
                                      </div>
                                    </div>

                                    {/* Achievements */}
                                    <Show when={detail.achievements.length > 0}>
                                      <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                        <h4 class="mb-3 font-medium text-white">Achievements</h4>
                                        <div class="flex flex-wrap gap-2">
                                          <For each={detail.achievements}>
                                            {ach => (
                                              <span class="rounded-full bg-indigo-500/20 px-3 py-1 text-sm text-indigo-300">
                                                {ach.achievement_id}
                                              </span>
                                            )}
                                          </For>
                                        </div>
                                      </div>
                                    </Show>

                                    {/* Quick Actions */}
                                    <div class="flex gap-3">
                                      <select
                                        value={detail.license?.tier || 'free'}
                                        onChange={e =>
                                          handleAdminUpdateUser(detail.user.id, {
                                            tier: e.currentTarget.value,
                                          })
                                        }
                                        class="rounded-lg border border-slate-700 bg-slate-800 px-3 py-2 text-sm text-white"
                                      >
                                        <option value="free">Free</option>
                                        <option value="pro">Pro</option>
                                        <option value="team">Team</option>
                                        <option value="enterprise">Enterprise</option>
                                      </select>
                                      <button
                                        onClick={() =>
                                          handleAdminUpdateUser(detail.user.id, {
                                            status:
                                              detail.license?.status === 'active'
                                                ? 'suspended'
                                                : 'active',
                                          })
                                        }
                                        class={`rounded-lg px-4 py-2 text-sm font-medium ${
                                          detail.license?.status === 'active'
                                            ? 'bg-red-500/20 text-red-400 hover:bg-red-500/30'
                                            : 'bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/30'
                                        }`}
                                      >
                                        {detail.license?.status === 'active'
                                          ? 'Suspend User'
                                          : 'Activate User'}
                                      </button>
                                      <Show when={detail.stripe}>
                                        <a
                                          href={`https://dashboard.stripe.com/customers/${detail.stripe?.customer.id}`}
                                          target="_blank"
                                          class="rounded-lg bg-indigo-500/20 px-4 py-2 text-sm font-medium text-indigo-400 hover:bg-indigo-500/30"
                                        >
                                          View in Stripe →
                                        </a>
                                      </Show>
                                    </div>
                                  </div>
                                </Show>

                                {/* Usage Tab */}
                                <Show when={userDetailTab() === 'usage'}>
                                  <div class="space-y-6">
                                    <div class="grid grid-cols-3 gap-4">
                                      <div class="rounded-xl bg-slate-800/50 p-4">
                                        <div class="text-2xl font-bold text-white">
                                          {detail.engagement.commands_last_7d.toLocaleString()}
                                        </div>
                                        <div class="text-sm text-slate-400">Commands (7 days)</div>
                                      </div>
                                      <div class="rounded-xl bg-slate-800/50 p-4">
                                        <div class="text-2xl font-bold text-white">
                                          {detail.engagement.commands_last_30d.toLocaleString()}
                                        </div>
                                        <div class="text-sm text-slate-400">Commands (30 days)</div>
                                      </div>
                                      <div class="rounded-xl bg-slate-800/50 p-4">
                                        <div class="text-2xl font-bold text-white">
                                          {detail.engagement.avg_daily_commands}
                                        </div>
                                        <div class="text-sm text-slate-400">Avg Daily</div>
                                      </div>
                                    </div>

                                    {/* Usage Chart (Simple bar representation) */}
                                    <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                      <h4 class="mb-4 font-medium text-white">
                                        Daily Usage (Last 30 Days)
                                      </h4>
                                      <div class="flex h-32 items-end gap-1">
                                        <For each={detail.usage.daily.slice(0, 30).reverse()}>
                                          {day => {
                                            const maxCommands = Math.max(
                                              ...detail.usage.daily.map(d => d.commands_run || 1)
                                            );
                                            const height = Math.max(
                                              4,
                                              ((day.commands_run || 0) / maxCommands) * 100
                                            );
                                            return (
                                              <div
                                                class="flex-1 rounded-t bg-indigo-500 transition-all hover:bg-indigo-400"
                                                style={{ height: `${height}%` }}
                                                title={`${day.date}: ${day.commands_run} commands`}
                                              />
                                            );
                                          }}
                                        </For>
                                      </div>
                                    </div>

                                    {/* Usage Summary */}
                                    <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                      <h4 class="mb-3 font-medium text-white">Usage Summary</h4>
                                      <div class="grid grid-cols-2 gap-4 text-sm">
                                        <div class="flex justify-between">
                                          <span class="text-slate-400">Total Packages Installed</span>
                                          <span class="text-white">
                                            {(
                                              detail.usage.summary?.total_packages || 0
                                            ).toLocaleString()}
                                          </span>
                                        </div>
                                        <div class="flex justify-between">
                                          <span class="text-slate-400">Total Searches</span>
                                          <span class="text-white">
                                            {(
                                              detail.usage.summary?.total_searches || 0
                                            ).toLocaleString()}
                                          </span>
                                        </div>
                                        <div class="flex justify-between">
                                          <span class="text-slate-400">Active Days</span>
                                          <span class="text-white">
                                            {detail.usage.summary?.active_days || 0}
                                          </span>
                                        </div>
                                        <div class="flex justify-between">
                                          <span class="text-slate-400">First Active</span>
                                          <span class="text-white">
                                            {detail.usage.summary?.first_active || 'Never'}
                                          </span>
                                        </div>
                                      </div>
                                    </div>
                                  </div>
                                </Show>

                                {/* Billing Tab */}
                                <Show when={userDetailTab() === 'billing'}>
                                  <div class="space-y-6">
                                    <Show
                                      when={detail.stripe}
                                      fallback={
                                        <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-8 text-center">
                                          <div class="mb-2 text-4xl">💳</div>
                                          <div class="text-lg font-medium text-white">
                                            No Stripe Customer
                                          </div>
                                          <div class="text-sm text-slate-400">
                                            This user hasn't made any purchases yet
                                          </div>
                                        </div>
                                      }
                                    >
                                      {/* Stripe Overview */}
                                      <div class="grid grid-cols-3 gap-4">
                                        <div class="rounded-xl bg-slate-800/50 p-4">
                                          <div class="text-2xl font-bold text-emerald-400">
                                            ${(detail.stripe?.total_spent || 0).toFixed(2)}
                                          </div>
                                          <div class="text-sm text-slate-400">Total Spent</div>
                                        </div>
                                        <div class="rounded-xl bg-slate-800/50 p-4">
                                          <div class="text-2xl font-bold text-white">
                                            {detail.stripe?.subscriptions.length || 0}
                                          </div>
                                          <div class="text-sm text-slate-400">Subscriptions</div>
                                        </div>
                                        <div class="rounded-xl bg-slate-800/50 p-4">
                                          <div class="text-2xl font-bold text-white">
                                            {detail.stripe?.invoices.length || 0}
                                          </div>
                                          <div class="text-sm text-slate-400">Invoices</div>
                                        </div>
                                      </div>

                                      {/* Subscriptions */}
                                      <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                        <h4 class="mb-3 font-medium text-white">Subscriptions</h4>
                                        <div class="space-y-2">
                                          <For each={detail.stripe?.subscriptions || []}>
                                            {sub => (
                                              <div class="flex items-center justify-between rounded-lg bg-slate-800 p-3">
                                                <div>
                                                  <div class="text-white">
                                                    {sub.plan?.nickname || 'Subscription'}
                                                  </div>
                                                  <div class="text-xs text-slate-400">
                                                    $
                                                    {((sub.plan?.unit_amount || 0) / 100).toFixed(2)}
                                                    /{sub.plan?.interval || 'month'}
                                                  </div>
                                                </div>
                                                <span
                                                  class={`rounded-full px-2 py-1 text-xs ${
                                                    sub.status === 'active'
                                                      ? 'bg-emerald-500/20 text-emerald-400'
                                                      : sub.status === 'canceled'
                                                        ? 'bg-red-500/20 text-red-400'
                                                        : 'bg-amber-500/20 text-amber-400'
                                                  }`}
                                                >
                                                  {sub.status}
                                                </span>
                                              </div>
                                            )}
                                          </For>
                                          <Show when={!detail.stripe?.subscriptions.length}>
                                            <div class="text-sm text-slate-500">
                                              No active subscriptions
                                            </div>
                                          </Show>
                                        </div>
                                      </div>

                                      {/* Payment Methods */}
                                      <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                        <h4 class="mb-3 font-medium text-white">Payment Methods</h4>
                                        <div class="space-y-2">
                                          <For each={detail.stripe?.payment_methods || []}>
                                            {pm => (
                                              <div class="flex items-center gap-3 rounded-lg bg-slate-800 p-3">
                                                <span class="text-2xl">💳</span>
                                                <div>
                                                  <div class="text-white">
                                                    {pm.card?.brand?.toUpperCase()} ••••{' '}
                                                    {pm.card?.last4}
                                                  </div>
                                                  <div class="text-xs text-slate-400">
                                                    Expires {pm.card?.exp_month}/{pm.card?.exp_year}
                                                  </div>
                                                </div>
                                              </div>
                                            )}
                                          </For>
                                          <Show when={!detail.stripe?.payment_methods.length}>
                                            <div class="text-sm text-slate-500">
                                              No payment methods
                                            </div>
                                          </Show>
                                        </div>
                                      </div>

                                      {/* Recent Invoices */}
                                      <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                        <h4 class="mb-3 font-medium text-white">Recent Invoices</h4>
                                        <div class="space-y-2">
                                          <For each={detail.stripe?.invoices.slice(0, 5) || []}>
                                            {inv => (
                                              <div class="flex items-center justify-between rounded-lg bg-slate-800 p-3">
                                                <div>
                                                  <div class="text-white">{inv.number}</div>
                                                  <div class="text-xs text-slate-400">
                                                    ${(inv.amount_paid / 100).toFixed(2)}
                                                  </div>
                                                </div>
                                                <div class="flex items-center gap-2">
                                                  <span
                                                    class={`rounded-full px-2 py-1 text-xs ${
                                                      inv.status === 'paid'
                                                        ? 'bg-emerald-500/20 text-emerald-400'
                                                        : 'bg-amber-500/20 text-amber-400'
                                                    }`}
                                                  >
                                                    {inv.status}
                                                  </span>
                                                  <a
                                                    href={inv.hosted_invoice_url}
                                                    target="_blank"
                                                    class="text-indigo-400 hover:text-indigo-300"
                                                  >
                                                    View →
                                                  </a>
                                                </div>
                                              </div>
                                            )}
                                          </For>
                                        </div>
                                      </div>
                                    </Show>
                                  </div>
                                </Show>

                                {/* Activity Tab */}
                                <Show when={userDetailTab() === 'activity'}>
                                  <div class="space-y-6">
                                    {/* Sessions */}
                                    <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                      <h4 class="mb-3 font-medium text-white">
                                        Active Sessions ({detail.sessions.length})
                                      </h4>
                                      <div class="space-y-2">
                                        <For each={detail.sessions}>
                                          {session => (
                                            <div class="flex items-center justify-between rounded-lg bg-slate-800 p-3 text-sm">
                                              <div>
                                                <div class="text-white">{session.ip_address}</div>
                                                <div class="max-w-md truncate text-xs text-slate-400">
                                                  {session.user_agent}
                                                </div>
                                              </div>
                                              <div class="text-xs text-slate-500">
                                                {api.formatRelativeTime(session.created_at)}
                                              </div>
                                            </div>
                                          )}
                                        </For>
                                      </div>
                                    </div>

                                    {/* Audit Log */}
                                    <div class="rounded-xl border border-slate-800 bg-slate-800/30 p-4">
                                      <h4 class="mb-3 font-medium text-white">Audit Log</h4>
                                      <div class="max-h-80 space-y-2 overflow-y-auto">
                                        <For each={detail.audit_log.slice(0, 50)}>
                                          {log => (
                                            <div class="flex items-center justify-between rounded-lg bg-slate-800 p-3 text-sm">
                                              <div>
                                                <div class="text-white">{log.action}</div>
                                                <div class="text-xs text-slate-400">
                                                  {log.resource_type && `${log.resource_type}`}
                                                  {log.ip_address && ` • ${log.ip_address}`}
                                                </div>
                                              </div>
                                              <div class="text-xs text-slate-500">
                                                {api.formatRelativeTime(log.created_at)}
                                              </div>
                                            </div>
                                          )}
                                        </For>
                                      </div>
                                    </div>
                                  </div>
                                </Show>
                              </div>
                            </div>
                          </div>
                        );
                      })()}
                    </Show>
                  </div>
                </Show>
              </div>
            );
          })()}
        </Show>
      </main>

      {/* Footer */}
      <footer class="relative z-10 mt-16 border-t border-slate-800/50">
        <div class="mx-auto flex max-w-7xl flex-col items-center justify-between gap-4 px-6 py-8 sm:flex-row">
          <p class="text-sm text-slate-500">© 2026 OMG Package Manager. All rights reserved.</p>
          <div class="flex items-center gap-6 text-sm">
            <A href="/" class="text-slate-400 transition-colors hover:text-white">
              Home
            </A>
            <a
              href="https://github.com/pyro1121/omg"
              target="_blank"
              class="text-slate-400 transition-colors hover:text-white"
            >
              GitHub
            </a>
            <a
              href="mailto:support@pyro1121.com"
              class="text-slate-400 transition-colors hover:text-white"
            >
              Support
            </a>
          </div>
        </div>
      </footer>
    </div>
  );
};

export default DashboardPage;
