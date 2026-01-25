import { Component, createSignal, createEffect, Show, For, onMount, Switch, Match } from 'solid-js';
import { A } from '@solidjs/router';
import { animate, stagger } from 'motion';
import * as api from '../lib/api';
import { TeamAnalytics } from '../components/dashboard/TeamAnalytics';
import { AdminDashboard } from '../components/dashboard/AdminDashboard';
import { MachinesView } from '../components/dashboard/MachinesView';
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
  LogOut,
  ChevronRight,
  Shield,
  ShieldAlert,
  Activity
} from 'lucide-solid';

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
  const [teamData, setTeamData] = createSignal<api.TeamData | null>(null);
  const [activeTab, setActiveTab] = createSignal<Tab>('overview');
  const [sessions, setSessions] = createSignal<api.Session[]>([]);
  const [auditLog, setAuditLog] = createSignal<api.AuditLogEntry[]>([]);
  const [copied, setCopied] = createSignal(false);
  const [actionMessage, setActionMessage] = createSignal('');

  // Admin state
  const [adminData, setAdminData] = createSignal<api.AdminOverview | null>(null);
  const [adminUsers, setAdminUsers] = createSignal<api.AdminUser[]>([]);
  const [adminActivity, setAdminActivity] = createSignal<api.AdminActivity[]>([]);

  // Entrance animations
  createEffect(() => {
    if (view() === 'dashboard' && dashboard()) {
      // Stagger nav items
      animate(
        "nav button",
        { opacity: [0, 1], x: [-20, 0] },
        { delay: stagger(0.05), duration: 0.5, easing: [0.16, 1, 0.3, 1] }
      );

      // Animate main content area
      animate(
        "main > div",
        { opacity: [0, 1], y: [10, 0] },
        { duration: 0.6, easing: [0.16, 1, 0.3, 1] }
      );
    }
  });

  // Check for existing session
  onMount(async () => {
    const params = new URLSearchParams(window.location.search);
    const token = api.getSessionToken();

    if (params.get('success') === 'true') {
      setActionMessage('Subscription updated successfully!');
      setTimeout(() => setActionMessage(''), 5000);
    }

    if (token) {
      setLoading(true);
      try {
        const session = await api.verifySession(token);
        if (session.valid) {
          setView('dashboard');
          await loadDashboardData();
        } else {
          api.clearSession();
        }
      } catch (e) {
        api.clearSession();
      } finally {
        setLoading(false);
      }
    }
  });

  const loadDashboardData = async () => {
    try {
      const data = await api.getDashboard();
      setDashboard(data);
      
      if (data?.license?.tier && ['team', 'enterprise'].includes(data.license.tier)) {
        await loadTeamData();
      }
    } catch (e) {
      console.error('Failed to load dashboard:', e);
      if ((e as any).message === 'Unauthorized' || (e as any).status === 401) {
        handleLogout();
      }
    }
  };

  const loadTeamData = async () => {
    try {
      const data = await api.getTeamMembers();
      setTeamData(data);
    } catch (e) {
      console.error('Failed to load team data:', e);
    }
  };

  // Auth handlers
  const handleSendCode = async (e: Event) => {
    e.preventDefault();
    setLoading(true);
    setError('');
    try {
      const res = await api.sendCode(email());
      if (res.success || res.status === 'ok') {
        setView('verify');
      } else {
        setError(res.error || 'Failed to send code');
      }
    } catch (e: any) {
      console.error('Send code error:', e);
      setError(e.message || 'Network error');
    } finally {
      setLoading(false);
    }
  };

  const handleVerifyCode = async (e: Event) => {
    e.preventDefault();
    setLoading(true);
    setError('');
    try {
      const res = await api.verifyCode(email(), otpCode());
      if (res.success && res.token) {
        api.setSessionToken(res.token);
        setView('dashboard');
        await loadDashboardData();
      } else {
        setError(res.error || 'Invalid code');
      }
    } catch (e: any) {
      console.error('Verify code error:', e);
      setError(e.message || 'Verification failed');
    } finally {
      setLoading(false);
    }
  };

  const handleLogout = () => {
    api.clearSession();
    setView('login');
    setEmail('');
    setOtpCode('');
    setDashboard(null);
  };

  const copyLicense = () => {
    const key = dashboard()?.license?.license_key;
    if (key) {
      navigator.clipboard.writeText(key);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  // Glassmorphism Styles
  const pageBg = "min-h-screen bg-[#0a0a0a] text-slate-200 font-sans selection:bg-blue-500/30 selection:text-blue-200 overflow-x-hidden relative";
  const bgEffects = (
    <>
      <div class="fixed top-[-20%] left-[-10%] w-[50%] h-[50%] bg-blue-600/10 rounded-full blur-[120px] pointer-events-none" />
      <div class="fixed bottom-[-20%] right-[-10%] w-[50%] h-[50%] bg-purple-600/10 rounded-full blur-[120px] pointer-events-none" />
      <div class="fixed top-[20%] right-[10%] w-[30%] h-[30%] bg-cyan-600/5 rounded-full blur-[100px] pointer-events-none" />
    </>
  );

  const glassPanel = "bg-white/5 backdrop-blur-xl border border-white/10 rounded-2xl shadow-2xl";
  const glassInput = "w-full bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-white placeholder-white/30 focus:outline-none focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/50 transition-all";
  const glassButton = "w-full bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-500 hover:to-indigo-500 text-white font-medium py-3 rounded-xl shadow-lg shadow-blue-500/20 transition-all active:scale-[0.98]";

  const NavItem = (props: { id: Tab; icon: any; label: string }) => {
    const isActive = () => activeTab() === props.id;
    return (
      <button
        onClick={() => setActiveTab(props.id)}
        class={`w-full flex items-center gap-3 px-4 py-3 rounded-xl transition-all duration-300 group relative ${
          isActive()
            ? 'text-white bg-blue-600/10 border border-blue-500/20'
            : 'text-slate-400 hover:text-white hover:bg-white/5'
        }`}
      >
        <Show when={isActive()}>
          <div class="absolute inset-0 bg-blue-500/5 blur-lg rounded-xl" />
          <div class="absolute left-0 top-1/2 -translate-y-1/2 w-1 h-8 bg-blue-500 rounded-r-full" />
        </Show>
        <props.icon class={`w-5 h-5 transition-colors ${isActive() ? 'text-blue-400' : 'text-slate-500 group-hover:text-slate-300'}`} />
        <span class="font-medium relative">{props.label}</span>
      </button>
    );
  };

  const InsightCard = (props: { title: string; value: string; icon: any; color: string; sub?: string }) => (
    <div class={`${glassPanel} p-6 relative overflow-hidden group hover:border-white/20 transition-colors`}>
      <div class={`absolute top-0 right-0 p-4 opacity-10 group-hover:opacity-20 transition-opacity`}>
        <props.icon class={`w-24 h-24 text-${props.color}-500`} />
      </div>
      <div class="relative z-10">
        <div class={`inline-flex p-2 rounded-lg bg-${props.color}-500/10 mb-4`}>
          <props.icon class={`w-6 h-6 text-${props.color}-400`} />
        </div>
        <h3 class="text-slate-400 text-sm font-medium mb-1">{props.title}</h3>
        <div class="text-3xl font-bold text-white mb-2">{props.value}</div>
        <Show when={props.sub}>
          <div class="text-xs text-slate-500 font-mono">{props.sub}</div>
        </Show>
      </div>
    </div>
  );

  return (
    <Switch>
      <Match when={view() === 'login'}>
        <div class={pageBg}>
          {bgEffects}
          <div class="relative z-10 min-h-screen flex flex-col items-center justify-center p-4">
            <div class={`${glassPanel} w-full max-w-md p-8 md:p-12 animate-fade-in`}>
              <div class="text-center mb-8">
                <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-gradient-to-tr from-blue-500 to-purple-500 mb-6 shadow-lg shadow-blue-500/20">
                  <Terminal class="w-8 h-8 text-white" />
                </div>
                <h1 class="text-3xl font-bold text-white mb-2 tracking-tight">Welcome back</h1>
                <p class="text-slate-400">Enter your email to access your dashboard</p>
              </div>

              <form onSubmit={handleSendCode} class="space-y-6">
                <div>
                  <label class="block text-sm font-medium text-slate-300 mb-2 ml-1">Email Address</label>
                  <input
                    type="email"
                    value={email()}
                    onInput={(e) => setEmail(e.currentTarget.value)}
                    placeholder="dev@example.com"
                    required
                    class={glassInput}
                  />
                </div>

                <Show when={error()}>
                  <div class="p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-400 text-sm flex items-center gap-2">
                    <ShieldAlert class="w-4 h-4" />
                    {error()}
                  </div>
                </Show>

                <button type="submit" disabled={loading()} class={glassButton}>
                  {loading() ? (
                    <span class="flex items-center justify-center gap-2">
                      <span class="w-4 h-4 border-2 border-white/20 border-t-white rounded-full animate-spin" />
                      Sending Code...
                    </span>
                  ) : 'Continue with Email'}
                </button>
              </form>
            </div>
          </div>
        </div>
      </Match>

      <Match when={view() === 'verify'}>
        <div class={pageBg}>
          {bgEffects}
          <div class="relative z-10 min-h-screen flex flex-col items-center justify-center p-4">
            <div class={`${glassPanel} w-full max-w-md p-8 md:p-12 animate-fade-in`}>
              <div class="text-center mb-8">
                <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-white/10 mb-6">
                  <Shield class="w-8 h-8 text-blue-400" />
                </div>
                <h1 class="text-3xl font-bold text-white mb-2 tracking-tight">Check your email</h1>
                <p class="text-slate-400">We sent a verification code to <span class="text-white font-medium">{email()}</span></p>
              </div>

              <form onSubmit={handleVerifyCode} class="space-y-6">
                <div>
                  <label class="block text-sm font-medium text-slate-300 mb-2 ml-1">Verification Code</label>
                  <input
                    type="text"
                    value={otpCode()}
                    onInput={(e) => setOtpCode(e.currentTarget.value)}
                    placeholder="123456"
                    required
                    class={`${glassInput} text-center text-2xl tracking-[0.5em] font-mono`}
                    maxLength={6}
                  />
                </div>

                <Show when={error()}>
                  <div class="p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-400 text-sm flex items-center gap-2">
                    <ShieldAlert class="w-4 h-4" />
                    {error()}
                  </div>
                </Show>

                <button type="submit" disabled={loading()} class={glassButton}>
                  {loading() ? (
                    <span class="flex items-center justify-center gap-2">
                      <span class="w-4 h-4 border-2 border-white/20 border-t-white rounded-full animate-spin" />
                      Verifying...
                    </span>
                  ) : 'Verify & Login'}
                </button>

                <div class="text-center">
                  <button
                    type="button"
                    onClick={() => setView('login')}
                    class="text-sm text-slate-400 hover:text-white transition-colors"
                  >
                    Use a different email
                  </button>
                </div>
              </form>
            </div>
          </div>
        </div>
      </Match>

      <Match when={view() === 'dashboard'}>
        <div class={pageBg}>
          {bgEffects}

          <div class="relative z-10 flex min-h-screen">
            {/* Sidebar */}
            <aside class="w-72 hidden lg:flex flex-col border-r border-white/5 bg-black/20 backdrop-blur-xl fixed h-screen z-50">
              <div class="p-6 border-b border-white/5">
                <div class="flex items-center gap-3">
                  <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center shadow-lg shadow-blue-500/20">
                    <Terminal class="w-6 h-6 text-white" />
                  </div>
                  <div>
                    <h1 class="text-xl font-bold text-white leading-none">OMG</h1>
                    <span class="text-xs text-slate-500 font-medium tracking-wider">DASHBOARD</span>
                  </div>
                </div>
              </div>

              <nav class="flex-1 p-4 space-y-1 overflow-y-auto">
                <div class="text-xs font-bold text-slate-600 uppercase tracking-wider mb-2 px-4 pt-2">Platform</div>
                <NavItem id="overview" icon={BarChart3} label="Overview" />
                <NavItem id="machines" icon={Monitor} label="Machines" />

                <div class="text-xs font-bold text-slate-600 uppercase tracking-wider mb-2 px-4 pt-6">Organization</div>
                <NavItem id="team" icon={Users} label="Team" />
                <NavItem id="security" icon={Lock} label="Security" />
                <NavItem id="billing" icon={CreditCard} label="Billing" />

                <Show when={dashboard()?.user.email === 'admin@omg.lol' || dashboard()?.is_admin || activeTab() === 'admin'}> {/* Basic admin check */}
                  <div class="text-xs font-bold text-slate-600 uppercase tracking-wider mb-2 px-4 pt-6">System</div>
                  <NavItem id="admin" icon={Crown} label="Admin Console" />
                </Show>
              </nav>

              <div class="p-4 border-t border-white/5">
                <button
                  onClick={handleLogout}
                  class="w-full flex items-center gap-3 px-4 py-3 rounded-xl text-slate-400 hover:text-white hover:bg-white/5 transition-all"
                >
                  <LogOut class="w-5 h-5" />
                  <span class="font-medium">Sign Out</span>
                </button>
              </div>
            </aside>

            {/* Main Content */}
            <main class="flex-1 lg:ml-72 p-4 md:p-8 overflow-x-hidden">
              <Show when={dashboard()} fallback={
                <div class="flex items-center justify-center h-[50vh]">
                  <div class="w-8 h-8 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" />
                </div>
              }>
                {/* Top Bar (Mobile Toggle + User Profile) */}
                <div class="flex justify-between items-center mb-8">
                  <div class="lg:hidden">
                    {/* Mobile menu trigger placeholder */}
                    <Terminal class="w-8 h-8 text-blue-500" />
                  </div>

              <div class="flex items-center gap-4 ml-auto">
                <div class="hidden md:flex flex-col items-end">
                  <span class="text-sm font-medium text-white">{dashboard()?.user.email}</span>
                  <span class="text-xs text-slate-500 uppercase tracking-wider">{dashboard()?.license.tier} Plan</span>
                </div>
                <div class="w-10 h-10 rounded-full bg-gradient-to-br from-slate-700 to-slate-800 border border-white/10 flex items-center justify-center">
                  <span class="text-lg font-bold text-white">{(dashboard()?.user?.email?.[0] || 'U').toUpperCase()}</span>
                </div>
              </div>
                </div>

                <Show when={actionMessage()}>
                  <div class="mb-6 p-4 rounded-xl bg-green-500/10 border border-green-500/20 text-green-400 flex items-center gap-3 animate-fade-in">
                    <CheckCircle class="w-5 h-5" />
                    {actionMessage()}
                  </div>
                </Show>

                <Show when={activeTab() === 'overview'}>
                  <div class="space-y-8 animate-fade-in">
                    {/* License Card */}
                    <div class="relative overflow-hidden rounded-3xl bg-gradient-to-r from-blue-600 to-purple-600 p-1 shadow-2xl shadow-blue-500/20">
                      <div class="absolute inset-0 bg-[url('/noise.png')] opacity-20 mix-blend-overlay" />
                      <div class="relative rounded-[20px] bg-black/40 backdrop-blur-xl p-6 md:p-8">
                        <div class="flex flex-col md:flex-row justify-between items-start md:items-center gap-6">
                          <div>
                            <h2 class="text-2xl font-bold text-white mb-2">Welcome back, Developer</h2>
                            <p class="text-blue-100/80">You're running on the <span class="font-bold text-white">{dashboard()?.license?.tier || 'Free'}</span> tier.</p>
                          </div>
                          <div class="flex items-center gap-3 bg-white/10 rounded-xl p-1 pr-4 border border-white/10 hover:bg-white/15 transition-colors cursor-pointer group" onClick={copyLicense}>
                            <div class="bg-black/50 p-2 rounded-lg text-xs font-mono text-slate-300">
                              LICENSE_KEY
                            </div>
                            <span class="font-mono text-white tracking-wide">
                              {dashboard()?.license?.license_key?.slice(0, 12) || '••••••••••••'}...
                            </span>
                            <Show when={copied()} fallback={<span class="text-xs text-blue-200 opacity-0 group-hover:opacity-100 transition-opacity">Copy</span>}>
                              <CheckCircle class="w-4 h-4 text-green-400" />
                            </Show>
                          </div>
                        </div>
                      </div>
                    </div>

                    {/* Personal Insights Grid */}
                    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                      <InsightCard
                        title="Time Saved"
                        value={`${((dashboard()?.usage?.total_time_saved_ms || 0) / 3600000).toFixed(1)}h`}
                        icon={Clock}
                        color="emerald"
                        sub="Reclaimed productivity"
                      />
                      <InsightCard
                        title="Commands Run"
                        value={(dashboard()?.usage?.total_commands ?? 0).toLocaleString()}
                        icon={Terminal}
                        color="blue"
                        sub="Total executions"
                      />
                      <InsightCard
                        title="Top Runtime"
                        value={dashboard()?.global_stats?.top_runtime || 'Node.js'}
                        icon={Activity}
                        color="purple"
                        sub="Most active environment"
                      />
                      <InsightCard
                        title="Security Score"
                        value={dashboard()?.usage?.total_vulnerabilities_found === 0 ? 'A+' : 'B'}
                        icon={Shield}
                        color="indigo"
                        sub={dashboard()?.usage?.total_vulnerabilities_found === 0 ? "No critical vulnerabilities" : `${dashboard()?.usage?.total_vulnerabilities_found} issues found`}
                      />
                    </div>

                    {/* AI Insights Section */}
                    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                      <div class={`${glassPanel} p-6 md:p-8 lg:col-span-2`}>
                        <div class="flex items-center gap-3 mb-6">
                          <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-pink-500 to-rose-500 flex items-center justify-center">
                            <Flame class="w-5 h-5 text-white" />
                          </div>
                          <h2 class="text-xl font-bold text-white">Smart Insights</h2>
                        </div>
                        <SmartInsights target="user" />
                      </div>

                      <div class={`${glassPanel} p-6 md:p-8`}>
                        <div class="flex items-center gap-3 mb-6">
                          <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-amber-500 to-orange-500 flex items-center justify-center">
                            <Crown class="w-5 h-5 text-white" />
                          </div>
                          <h2 class="text-xl font-bold text-white">Leaderboard</h2>
                        </div>
                        <div class="space-y-4">
                          <For each={dashboard()?.leaderboard || []}>
                            {(entry, i) => (
                              <div class="flex items-center justify-between p-3 rounded-xl bg-white/5 border border-white/5">
                                <div class="flex items-center gap-3">
                                  <span class={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold ${i() === 0 ? 'bg-amber-500 text-black' : 'bg-slate-800 text-slate-400'}`}>
                                    {i() + 1}
                                  </span>
                                  <span class="text-sm font-medium text-white">{entry.user}</span>
                                </div>
                                <span class="text-xs font-mono text-emerald-400">{api.formatTimeSaved(entry.time_saved)}</span>
                              </div>
                            )}
                          </For>
                          <Show when={!dashboard()?.leaderboard?.length}>
                            <div class="text-center py-4 text-slate-500 italic text-sm">No data available</div>
                          </Show>
                        </div>
                      </div>
                    </div>

                    {/* Recent Activity */}
                    <div class={`${glassPanel} p-6 md:p-8`}>
                      <div class="flex items-center justify-between mb-6">
                        <h2 class="text-xl font-bold text-white">Recent Activity</h2>
                        <button class="text-sm text-blue-400 hover:text-blue-300 font-medium">View All</button>
                      </div>
                      <div class="space-y-4">
                        <Show when={auditLog().length > 0} fallback={
                          <div class="text-center py-12 text-slate-500 italic">No recent activity recorded</div>
                        }>
                          <For each={auditLog().slice(0, 5)}>
                            {(log) => (
                              <div class="flex items-center gap-4 p-4 rounded-xl bg-white/5 border border-white/5 hover:border-white/10 transition-colors">
                                <div class="w-10 h-10 rounded-full bg-slate-800 flex items-center justify-center">
                                  <Terminal class="w-5 h-5 text-slate-400" />
                                </div>
                                <div class="flex-1">
                                  <div class="text-sm font-medium text-white">{log.action}</div>
                                  <div class="text-xs text-slate-500 font-mono mt-1">
                                    {log.created_at ? new Date(log.created_at).toLocaleString() : 'N/A'} • {log.ip_address}
                                  </div>
                                </div>
                                <ChevronRight class="w-5 h-5 text-slate-600" />
                              </div>
                            )}
                          </For>
                        </Show>
                      </div>
                    </div>
                  </div>
                </Show>

                {/* Other Tabs Placeholders */}
                <Show when={activeTab() === 'machines'}>
                  <MachinesView
                    machines={dashboard()?.machines || []}
                    onRevoke={loadDashboardData}
                  />
                </Show>

                <Show when={activeTab() === 'team'}>
                  <TeamAnalytics
                    teamData={teamData()}
                    licenseKey={dashboard()?.license?.license_key || ''}
                    onRevoke={(id) => api.revokeMachine(id).then(loadTeamData)}
                    onRefresh={loadTeamData}
                  />
                </Show>

                <Show when={activeTab() === 'security'}>
                  <TeamAnalytics
                    teamData={teamData()}
                    licenseKey={dashboard()?.license?.license_key || ''}
                    onRevoke={(id) => api.revokeMachine(id).then(loadTeamData)}
                    onRefresh={loadTeamData}
                    initialView="security"
                  />
                </Show>

                <Show when={activeTab() === 'billing'}>
                  <div class="space-y-8 animate-fade-in">
                    <div class={`${glassPanel} p-10 shadow-2xl`}>
                      <h3 class="mb-6 text-2xl font-black text-white tracking-tight uppercase tracking-widest">Billing & Subscription</h3>
                      <p class="text-slate-400 mb-8">Manage your subscription, payment methods, and view invoices.</p>
                      <div class="flex gap-4">
                        <button
                          onClick={() => window.open('https://pyro1121.com/pricing', '_blank')}
                          class="rounded-2xl bg-white px-8 py-4 text-sm font-black text-black transition-all hover:scale-[1.02]"
                        >
                          View Plans
                        </button>
                        <button
                          onClick={async () => {
                            try {
                              const res = await api.openBillingPortal(dashboard()?.user?.email || '');
                              if (res.url) window.open(res.url, '_blank');
                            } catch (e) {
                              console.error('Failed to open billing portal:', e);
                            }
                          }}
                          class="rounded-2xl border border-white/10 bg-white/[0.03] px-8 py-4 text-sm font-black text-white transition-all hover:bg-white/[0.08]"
                        >
                          Customer Portal
                        </button>
                      </div>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                      <div class={`${glassPanel} p-8`}>
                        <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest mb-2">Current Tier</div>
                        <div class="text-2xl font-black text-indigo-400 uppercase">{dashboard()?.license?.tier || 'Free'}</div>
                      </div>
                      <div class={`${glassPanel} p-8`}>
                        <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest mb-2">Status</div>
                        <div class="text-2xl font-black text-emerald-400 uppercase">{dashboard()?.license?.status || 'Active'}</div>
                      </div>
                      <div class={`${glassPanel} p-8`}>
                        <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest mb-2">Next Renewal</div>
                        <div class="text-2xl font-black text-white">
                          {dashboard()?.license?.expires_at ? new Date(dashboard()!.license.expires_at).toLocaleDateString() : 'N/A'}
                        </div>
                      </div>
                    </div>
                  </div>
                </Show>

                <Show when={activeTab() === 'admin'}>
                  <AdminDashboard />
                </Show>

              </Show>
            </main>
          </div>
        </div>
      </Match>
    </Switch>
  );
};

export default DashboardPage;
