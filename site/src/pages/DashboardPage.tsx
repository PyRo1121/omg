import {
  Component,
  createSignal,
  createEffect,
  Show,
  For,
  onMount,
  onCleanup,
  Switch,
  Match,
} from 'solid-js';
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
  Activity,
} from 'lucide-solid';

type View = 'login' | 'verify' | 'dashboard';
type Tab = 'overview' | 'machines' | 'team' | 'security' | 'billing' | 'admin';

// Turnstile site key (public - safe to expose)
const TURNSTILE_SITE_KEY = '0x4AAAAAACREzmYYXC6MxMqi';

// Declare Turnstile global types
declare global {
  interface Window {
    turnstile?: {
      render: (
        container: string | HTMLElement,
        options: {
          sitekey: string;
          callback: (token: string) => void;
          'error-callback'?: () => void;
          'expired-callback'?: () => void;
          theme?: 'light' | 'dark' | 'auto';
          size?: 'normal' | 'compact';
        }
      ) => string;
      reset: (widgetId: string) => void;
      remove: (widgetId: string) => void;
    };
  }
}

const DashboardPage: Component = () => {
  // Auth state
  const [view, setView] = createSignal<View>('login');
  const [email, setEmail] = createSignal('');
  const [otpCode, setOtpCode] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');
  const [turnstileToken, setTurnstileToken] = createSignal<string | null>(null);
  let turnstileWidgetId: string | null = null;
  let turnstileContainer: HTMLDivElement | undefined;

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

  createEffect(() => {
    if (view() === 'dashboard' && dashboard()) {
      animate(
        'nav button',
        { opacity: [0, 1], x: [-20, 0] } as any,
        { delay: stagger(0.05), duration: 0.5, ease: [0.16, 1, 0.3, 1] }
      );

      animate(
        'main > div',
        { opacity: [0, 1], y: [10, 0] } as any,
        { duration: 0.6, ease: [0.16, 1, 0.3, 1] }
      );
    }
  });

  const initTurnstile = () => {
    if (!turnstileContainer || !window.turnstile) return;
    
    if (turnstileWidgetId) {
      try { window.turnstile.remove(turnstileWidgetId); } catch {}
      turnstileWidgetId = null;
    }
    
    turnstileWidgetId = window.turnstile.render(turnstileContainer, {
      sitekey: TURNSTILE_SITE_KEY,
      callback: (token: string) => {
        setTurnstileToken(token);
        setError('');
      },
      'error-callback': () => {
        setError('Security verification failed. Please refresh and try again.');
        setTurnstileToken(null);
      },
      'expired-callback': () => {
        setTurnstileToken(null);
      },
      theme: 'dark',
      size: 'normal',
    });
  };

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

    // Load Turnstile script
    if (!document.querySelector('script[src*="turnstile"]')) {
      const script = document.createElement('script');
      script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?onload=onTurnstileLoad';
      script.async = true;
      (window as any).onTurnstileLoad = initTurnstile;
      document.head.appendChild(script);
    } else if (window.turnstile) {
      initTurnstile();
    }
  });

  // Cleanup Turnstile widget
  onCleanup(() => {
    if (turnstileWidgetId && window.turnstile) {
      window.turnstile.remove(turnstileWidgetId);
      turnstileWidgetId = null;
    }
  });

  createEffect(() => {
    if (view() === 'login' && window.turnstile && turnstileContainer) {
      queueMicrotask(initTurnstile);
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

    // Require Turnstile verification
    if (!turnstileToken()) {
      setError('Please complete the security verification');
      return;
    }

    setLoading(true);
    setError('');
    try {
      const res = await api.sendCode(email(), turnstileToken()!);
      if (res.success || res.status === 'ok') {
        setView('verify');
        // Reset Turnstile for next login attempt
        setTurnstileToken(null);
        if (turnstileWidgetId && window.turnstile) {
          window.turnstile.reset(turnstileWidgetId);
        }
      } else {
        setError(res.error || 'Failed to send code');
        // Reset Turnstile on error
        if (turnstileWidgetId && window.turnstile) {
          window.turnstile.reset(turnstileWidgetId);
        }
        setTurnstileToken(null);
      }
    } catch (e: any) {
      console.error('Send code error:', e);
      setError(e.message || 'Network error');
      // Reset Turnstile on error
      if (turnstileWidgetId && window.turnstile) {
        window.turnstile.reset(turnstileWidgetId);
      }
      setTurnstileToken(null);
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
  const pageBg =
    'min-h-screen bg-[#0a0a0a] text-slate-200 font-sans selection:bg-blue-500/30 selection:text-blue-200 overflow-x-hidden relative';
  const bgEffects = (
    <>
      <div class="pointer-events-none fixed top-[-20%] left-[-10%] h-[50%] w-[50%] rounded-full bg-blue-600/10 blur-[120px]" />
      <div class="pointer-events-none fixed right-[-10%] bottom-[-20%] h-[50%] w-[50%] rounded-full bg-purple-600/10 blur-[120px]" />
      <div class="pointer-events-none fixed top-[20%] right-[10%] h-[30%] w-[30%] rounded-full bg-cyan-600/5 blur-[100px]" />
    </>
  );

  const glassPanel = 'bg-white/5 backdrop-blur-xl border border-white/10 rounded-2xl shadow-2xl';
  const glassInput =
    'w-full bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-white placeholder-white/30 focus:outline-none focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/50 transition-all';
  const glassButton =
    'w-full bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-500 hover:to-indigo-500 text-white font-medium py-3 rounded-xl shadow-lg shadow-blue-500/20 transition-all active:scale-[0.98]';

  const NavItem = (props: { id: Tab; icon: any; label: string }) => {
    const isActive = () => activeTab() === props.id;
    return (
      <button
        onClick={() => setActiveTab(props.id)}
        class={`group relative flex w-full items-center gap-3 rounded-xl px-4 py-3 transition-all duration-300 ${
          isActive()
            ? 'border border-blue-500/20 bg-blue-600/10 text-white'
            : 'text-slate-400 hover:bg-white/5 hover:text-white'
        }`}
      >
        <Show when={isActive()}>
          <div class="absolute inset-0 rounded-xl bg-blue-500/5 blur-lg" />
          <div class="absolute top-1/2 left-0 h-8 w-1 -translate-y-1/2 rounded-r-full bg-blue-500" />
        </Show>
        <props.icon
          class={`h-5 w-5 transition-colors ${isActive() ? 'text-blue-400' : 'text-slate-500 group-hover:text-slate-300'}`}
        />
        <span class="relative font-medium">{props.label}</span>
      </button>
    );
  };

  const InsightCard = (props: {
    title: string;
    value: string;
    icon: any;
    color: string;
    sub?: string;
  }) => (
    <div
      class={`${glassPanel} group relative overflow-hidden p-6 transition-colors hover:border-white/20`}
    >
      <div
        class={`absolute top-0 right-0 p-4 opacity-10 transition-opacity group-hover:opacity-20`}
      >
        <props.icon class={`h-24 w-24 text-${props.color}-500`} />
      </div>
      <div class="relative z-10">
        <div class={`inline-flex rounded-lg p-2 bg-${props.color}-500/10 mb-4`}>
          <props.icon class={`h-6 w-6 text-${props.color}-400`} />
        </div>
        <h3 class="mb-1 text-sm font-medium text-slate-400">{props.title}</h3>
        <div class="mb-2 text-3xl font-bold text-white">{props.value}</div>
        <Show when={props.sub}>
          <div class="font-mono text-xs text-slate-500">{props.sub}</div>
        </Show>
      </div>
    </div>
  );

  return (
    <Switch>
      <Match when={view() === 'login'}>
        <div class={pageBg}>
          {bgEffects}
          <div class="relative z-10 flex min-h-screen flex-col items-center justify-center p-4">
            <div class={`${glassPanel} animate-fade-in w-full max-w-md p-8 md:p-12`}>
              <div class="mb-8 text-center">
                <div class="mb-6 inline-flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-tr from-blue-500 to-purple-500 shadow-lg shadow-blue-500/20">
                  <Terminal class="h-8 w-8 text-white" />
                </div>
                <h1 class="mb-2 text-3xl font-bold tracking-tight text-white">Welcome back</h1>
                <p class="text-slate-400">Enter your email to access your dashboard</p>
              </div>

              <form onSubmit={handleSendCode} class="space-y-6">
                <div>
                  <label class="mb-2 ml-1 block text-sm font-medium text-slate-300">
                    Email Address
                  </label>
                  <input
                    type="email"
                    value={email()}
                    onInput={e => setEmail(e.currentTarget.value)}
                    placeholder="dev@example.com"
                    required
                    class={glassInput}
                  />
                </div>

                {/* Cloudflare Turnstile Widget */}
                <div class="flex justify-center">
                  <div ref={turnstileContainer} class="cf-turnstile" />
                </div>

                <Show when={error()}>
                  <div class="flex items-center gap-2 rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-400">
                    <ShieldAlert class="h-4 w-4" />
                    {error()}
                  </div>
                </Show>

                <button
                  type="submit"
                  disabled={loading() || !turnstileToken()}
                  class={`${glassButton} ${!turnstileToken() ? 'cursor-not-allowed opacity-50' : ''}`}
                >
                  {loading() ? (
                    <span class="flex items-center justify-center gap-2">
                      <span class="h-4 w-4 animate-spin rounded-full border-2 border-white/20 border-t-white" />
                      Sending Code...
                    </span>
                  ) : (
                    'Continue with Email'
                  )}
                </button>
              </form>
            </div>
          </div>
        </div>
      </Match>

      <Match when={view() === 'verify'}>
        <div class={pageBg}>
          {bgEffects}
          <div class="relative z-10 flex min-h-screen flex-col items-center justify-center p-4">
            <div class={`${glassPanel} animate-fade-in w-full max-w-md p-8 md:p-12`}>
              <div class="mb-8 text-center">
                <div class="mb-6 inline-flex h-16 w-16 items-center justify-center rounded-2xl bg-white/10">
                  <Shield class="h-8 w-8 text-blue-400" />
                </div>
                <h1 class="mb-2 text-3xl font-bold tracking-tight text-white">Check your email</h1>
                <p class="text-slate-400">
                  We sent a verification code to{' '}
                  <span class="font-medium text-white">{email()}</span>
                </p>
              </div>

              <form onSubmit={handleVerifyCode} class="space-y-6">
                <div>
                  <label class="mb-2 ml-1 block text-sm font-medium text-slate-300">
                    Verification Code
                  </label>
                  <input
                    type="text"
                    value={otpCode()}
                    onInput={e => setOtpCode(e.currentTarget.value)}
                    placeholder="123456"
                    required
                    class={`${glassInput} text-center font-mono text-2xl tracking-[0.5em]`}
                    maxLength={6}
                  />
                </div>

                <Show when={error()}>
                  <div class="flex items-center gap-2 rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-400">
                    <ShieldAlert class="h-4 w-4" />
                    {error()}
                  </div>
                </Show>

                <button type="submit" disabled={loading()} class={glassButton}>
                  {loading() ? (
                    <span class="flex items-center justify-center gap-2">
                      <span class="h-4 w-4 animate-spin rounded-full border-2 border-white/20 border-t-white" />
                      Verifying...
                    </span>
                  ) : (
                    'Verify & Login'
                  )}
                </button>

                <div class="text-center">
                  <button
                    type="button"
                    onClick={() => setView('login')}
                    class="text-sm text-slate-400 transition-colors hover:text-white"
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
            <aside class="fixed z-50 hidden h-screen w-72 flex-col border-r border-white/5 bg-black/20 backdrop-blur-xl lg:flex">
              <div class="border-b border-white/5 p-6">
                <div class="flex items-center gap-3">
                  <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-blue-500 to-purple-600 shadow-lg shadow-blue-500/20">
                    <Terminal class="h-6 w-6 text-white" />
                  </div>
                  <div>
                    <h1 class="text-xl leading-none font-bold text-white">OMG</h1>
                    <span class="text-xs font-medium tracking-wider text-slate-500">DASHBOARD</span>
                  </div>
                </div>
              </div>

              <nav class="flex-1 space-y-1 overflow-y-auto p-4">
                <div class="mb-2 px-4 pt-2 text-xs font-bold tracking-wider text-slate-600 uppercase">
                  Platform
                </div>
                <NavItem id="overview" icon={BarChart3} label="Overview" />
                <NavItem id="machines" icon={Monitor} label="Machines" />

                <div class="mb-2 px-4 pt-6 text-xs font-bold tracking-wider text-slate-600 uppercase">
                  Organization
                </div>
                <NavItem id="team" icon={Users} label="Team" />
                <NavItem id="security" icon={Lock} label="Security" />
                <NavItem id="billing" icon={CreditCard} label="Billing" />

                <Show
                  when={
                    dashboard()?.user.email === 'admin@omg.lol' ||
                    dashboard()?.is_admin ||
                    activeTab() === 'admin'
                  }
                >
                  {' '}
                  {/* Basic admin check */}
                  <div class="mb-2 px-4 pt-6 text-xs font-bold tracking-wider text-slate-600 uppercase">
                    System
                  </div>
                  <NavItem id="admin" icon={Crown} label="Admin Console" />
                </Show>
              </nav>

              <div class="border-t border-white/5 p-4">
                <button
                  onClick={handleLogout}
                  class="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-slate-400 transition-all hover:bg-white/5 hover:text-white"
                >
                  <LogOut class="h-5 w-5" />
                  <span class="font-medium">Sign Out</span>
                </button>
              </div>
            </aside>

            {/* Main Content */}
            <main class="flex-1 overflow-x-hidden p-4 md:p-8 lg:ml-72">
              <Show
                when={dashboard()}
                fallback={
                  <div class="flex h-[50vh] items-center justify-center">
                    <div class="h-8 w-8 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" />
                  </div>
                }
              >
                {/* Top Bar (Mobile Toggle + User Profile) */}
                <div class="mb-8 flex items-center justify-between">
                  <div class="lg:hidden">
                    {/* Mobile menu trigger placeholder */}
                    <Terminal class="h-8 w-8 text-blue-500" />
                  </div>

                  <div class="ml-auto flex items-center gap-4">
                    <div class="hidden flex-col items-end md:flex">
                      <span class="text-sm font-medium text-white">{dashboard()?.user.email}</span>
                      <span class="text-xs tracking-wider text-slate-500 uppercase">
                        {dashboard()?.license.tier} Plan
                      </span>
                    </div>
                    <div class="flex h-10 w-10 items-center justify-center rounded-full border border-white/10 bg-gradient-to-br from-slate-700 to-slate-800">
                      <span class="text-lg font-bold text-white">
                        {(dashboard()?.user?.email?.[0] || 'U').toUpperCase()}
                      </span>
                    </div>
                  </div>
                </div>

                <Show when={actionMessage()}>
                  <div class="animate-fade-in mb-6 flex items-center gap-3 rounded-xl border border-green-500/20 bg-green-500/10 p-4 text-green-400">
                    <CheckCircle class="h-5 w-5" />
                    {actionMessage()}
                  </div>
                </Show>

                <Show when={activeTab() === 'overview'}>
                  <div class="animate-fade-in space-y-8">
                    {/* License Card */}
                    <div class="relative overflow-hidden rounded-3xl bg-gradient-to-r from-blue-600 to-purple-600 p-1 shadow-2xl shadow-blue-500/20">
                      <div
                        class="absolute inset-0 opacity-20 mix-blend-overlay"
                        style="background-image: url('data:image/svg+xml,%3Csvg viewBox=%220 0 200 200%22 xmlns=%22http://www.w3.org/2000/svg%22%3E%3Cfilter id=%22n%22%3E%3CfeTurbulence type=%22fractalNoise%22 baseFrequency=%220.65%22 numOctaves=%223%22 stitchTiles=%22stitch%22/%3E%3C/filter%3E%3Crect width=%22100%25%22 height=%22100%25%22 filter=%22url(%23n)%22/%3E%3C/svg%3E');"
                      />
                      <div class="relative rounded-[20px] bg-black/40 p-6 backdrop-blur-xl md:p-8">
                        <div class="flex flex-col items-start justify-between gap-6 md:flex-row md:items-center">
                          <div>
                            <h2 class="mb-2 text-2xl font-bold text-white">
                              Welcome back, Developer
                            </h2>
                            <p class="text-blue-100/80">
                              You're running on the{' '}
                              <span class="font-bold text-white">
                                {dashboard()?.license?.tier || 'Free'}
                              </span>{' '}
                              tier.
                            </p>
                          </div>
                          <div
                            class="group flex cursor-pointer items-center gap-3 rounded-xl border border-white/10 bg-white/10 p-1 pr-4 transition-colors hover:bg-white/15"
                            onClick={copyLicense}
                          >
                            <div class="rounded-lg bg-black/50 p-2 font-mono text-xs text-slate-300">
                              LICENSE_KEY
                            </div>
                            <span class="font-mono tracking-wide text-white">
                              {dashboard()?.license?.license_key?.slice(0, 12) || '••••••••••••'}...
                            </span>
                            <Show
                              when={copied()}
                              fallback={
                                <span class="text-xs text-blue-200 opacity-0 transition-opacity group-hover:opacity-100">
                                  Copy
                                </span>
                              }
                            >
                              <CheckCircle class="h-4 w-4 text-green-400" />
                            </Show>
                          </div>
                        </div>
                      </div>
                    </div>

                    {/* Personal Insights Grid */}
                    <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
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
                        sub={
                          dashboard()?.usage?.total_vulnerabilities_found === 0
                            ? 'No critical vulnerabilities'
                            : `${dashboard()?.usage?.total_vulnerabilities_found} issues found`
                        }
                      />
                    </div>

                    {/* AI Insights Section */}
                    <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
                      <div class={`${glassPanel} p-6 md:p-8 lg:col-span-2`}>
                        <div class="mb-6 flex items-center gap-3">
                          <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-pink-500 to-rose-500">
                            <Flame class="h-5 w-5 text-white" />
                          </div>
                          <h2 class="text-xl font-bold text-white">Smart Insights</h2>
                        </div>
                        <SmartInsights target="user" />
                      </div>

                      <div class={`${glassPanel} p-6 md:p-8`}>
                        <div class="mb-6 flex items-center gap-3">
                          <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-amber-500 to-orange-500">
                            <Crown class="h-5 w-5 text-white" />
                          </div>
                          <h2 class="text-xl font-bold text-white">Leaderboard</h2>
                        </div>
                        <div class="space-y-4">
                          <For each={dashboard()?.leaderboard || []}>
                            {(entry, i) => (
                              <div class="flex items-center justify-between rounded-xl border border-white/5 bg-white/5 p-3">
                                <div class="flex items-center gap-3">
                                  <span
                                    class={`flex h-6 w-6 items-center justify-center rounded-full text-xs font-bold ${i() === 0 ? 'bg-amber-500 text-black' : 'bg-slate-800 text-slate-400'}`}
                                  >
                                    {i() + 1}
                                  </span>
                                  <span class="text-sm font-medium text-white">{entry.user}</span>
                                </div>
                                <span class="font-mono text-xs text-emerald-400">
                                  {api.formatTimeSaved(entry.time_saved)}
                                </span>
                              </div>
                            )}
                          </For>
                          <Show when={!dashboard()?.leaderboard?.length}>
                            <div class="py-4 text-center text-sm text-slate-500 italic">
                              No data available
                            </div>
                          </Show>
                        </div>
                      </div>
                    </div>

                    {/* Recent Activity */}
                    <div class={`${glassPanel} p-6 md:p-8`}>
                      <div class="mb-6 flex items-center justify-between">
                        <h2 class="text-xl font-bold text-white">Recent Activity</h2>
                        <button class="text-sm font-medium text-blue-400 hover:text-blue-300">
                          View All
                        </button>
                      </div>
                      <div class="space-y-4">
                        <Show
                          when={auditLog().length > 0}
                          fallback={
                            <div class="py-12 text-center text-slate-500 italic">
                              No recent activity recorded
                            </div>
                          }
                        >
                          <For each={auditLog().slice(0, 5)}>
                            {log => (
                              <div class="flex items-center gap-4 rounded-xl border border-white/5 bg-white/5 p-4 transition-colors hover:border-white/10">
                                <div class="flex h-10 w-10 items-center justify-center rounded-full bg-slate-800">
                                  <Terminal class="h-5 w-5 text-slate-400" />
                                </div>
                                <div class="flex-1">
                                  <div class="text-sm font-medium text-white">{log.action}</div>
                                  <div class="mt-1 font-mono text-xs text-slate-500">
                                    {log.created_at
                                      ? new Date(log.created_at).toLocaleString()
                                      : 'N/A'}{' '}
                                    • {log.ip_address}
                                  </div>
                                </div>
                                <ChevronRight class="h-5 w-5 text-slate-600" />
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
                    onRevoke={id => api.revokeMachine(id).then(loadTeamData)}
                    onRefresh={loadTeamData}
                  />
                </Show>

                <Show when={activeTab() === 'security'}>
                  <TeamAnalytics
                    teamData={teamData()}
                    licenseKey={dashboard()?.license?.license_key || ''}
                    onRevoke={id => api.revokeMachine(id).then(loadTeamData)}
                    onRefresh={loadTeamData}
                    initialView="security"
                  />
                </Show>

                <Show when={activeTab() === 'billing'}>
                  <div class="animate-fade-in space-y-8">
                    <div class={`${glassPanel} p-10 shadow-2xl`}>
                      <h3 class="mb-6 text-2xl font-black tracking-tight tracking-widest text-white uppercase">
                        Billing & Subscription
                      </h3>
                      <p class="mb-8 text-slate-400">
                        Manage your subscription, payment methods, and view invoices.
                      </p>
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
                              const res = await api.openBillingPortal(
                                dashboard()?.user?.email || ''
                              );
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

                    <div class="grid grid-cols-1 gap-6 md:grid-cols-3">
                      <div class={`${glassPanel} p-8`}>
                        <div class="mb-2 text-[10px] font-bold tracking-widest text-slate-500 uppercase">
                          Current Tier
                        </div>
                        <div class="text-2xl font-black text-indigo-400 uppercase">
                          {dashboard()?.license?.tier || 'Free'}
                        </div>
                      </div>
                      <div class={`${glassPanel} p-8`}>
                        <div class="mb-2 text-[10px] font-bold tracking-widest text-slate-500 uppercase">
                          Status
                        </div>
                        <div class="text-2xl font-black text-emerald-400 uppercase">
                          {dashboard()?.license?.status || 'Active'}
                        </div>
                      </div>
                      <div class={`${glassPanel} p-8`}>
                        <div class="mb-2 text-[10px] font-bold tracking-widest text-slate-500 uppercase">
                          Next Renewal
                        </div>
                        <div class="text-2xl font-black text-white">
                          {dashboard()?.license?.expires_at
                            ? new Date(dashboard()!.license.expires_at!).toLocaleDateString()
                            : 'N/A'}
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
