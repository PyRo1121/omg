import { Component, createSignal, createEffect, Show, For, onMount } from 'solid-js';
import { A } from '@solidjs/router';

interface LicenseInfo {
  license_key: string;
  tier: string;
  expires_at: string;
  status: string;
  used_seats?: number;
  max_seats?: number;
  machines?: Machine[];
  usage?: UsageStats;
}

interface Machine {
  id: string;
  hostname: string;
  last_seen: string;
  os: string;
}

interface UsageStats {
  queries_today: number;
  queries_this_month: number;
  sbom_generated: number;
  vulnerabilities_found: number;
  time_saved_ms: number;
  total_commands: number;
  current_streak: number;
  longest_streak: number;
  achievements: string[];
}

interface Achievement {
  id: string;
  emoji: string;
  name: string;
  description: string;
  unlocked?: boolean;
}

const ALL_ACHIEVEMENTS: Achievement[] = [
  { id: 'FirstStep', emoji: '🚀', name: 'First Step', description: 'Executed your first command' },
  { id: 'Centurion', emoji: '💯', name: 'Centurion', description: 'Executed 100 commands' },
  { id: 'PowerUser', emoji: '⚡', name: 'Power User', description: 'Executed 1,000 commands' },
  { id: 'Legend', emoji: '🏆', name: 'Legend', description: 'Executed 10,000 commands' },
  { id: 'MinuteSaver', emoji: '⏱️', name: 'Minute Saver', description: 'Saved 1 minute of time' },
  { id: 'HourSaver', emoji: '⏰', name: 'Hour Saver', description: 'Saved 1 hour of time' },
  { id: 'DaySaver', emoji: '📅', name: 'Day Saver', description: 'Saved 24 hours of time' },
  { id: 'WeekStreak', emoji: '🔥', name: 'Week Streak', description: 'Used OMG for 7 days straight' },
  { id: 'MonthStreak', emoji: '💎', name: 'Month Streak', description: 'Used OMG for 30 days straight' },
  { id: 'Polyglot', emoji: '🌐', name: 'Polyglot', description: 'Used all 7 built-in runtimes' },
  { id: 'SecurityFirst', emoji: '🛡️', name: 'Security First', description: 'Generated your first SBOM' },
  { id: 'BugHunter', emoji: '🐛', name: 'Bug Hunter', description: 'Found and addressed vulnerabilities' },
];

const formatTimeSaved = (ms: number): string => {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 3600000) return `${(ms / 60000).toFixed(1)}min`;
  if (ms < 86400000) return `${(ms / 3600000).toFixed(1)}hr`;
  return `${(ms / 86400000).toFixed(1)} days`;
};

const API_BASE = 'https://api.pyro1121.com';

const DashboardPage: Component = () => {
  const [email, setEmail] = createSignal('');
  const [licenseKey, setLicenseKey] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');
  const [license, setLicense] = createSignal<LicenseInfo | null>(null);
  const [view, setView] = createSignal<'login' | 'register' | 'dashboard'>('login');
  const [actionLoading, setActionLoading] = createSignal(false);
  const [actionMessage, setActionMessage] = createSignal('');
  const [copied, setCopied] = createSignal(false);
  const [activeTab, setActiveTab] = createSignal<'overview' | 'usage' | 'security' | 'settings'>('overview');

  // Check for stored session on mount
  onMount(() => {
    const storedEmail = localStorage.getItem('omg_email');
    const storedKey = localStorage.getItem('omg_license_key');
    if (storedEmail && storedKey) {
      setEmail(storedEmail);
      setLicenseKey(storedKey);
      fetchLicenseWithKey(storedEmail, storedKey);
    }
  });

  const fetchLicenseWithKey = async (userEmail: string, key: string) => {
    setLoading(true);
    setError('');
    try {
      const res = await fetch(`${API_BASE}/api/get-license?email=${encodeURIComponent(userEmail)}`);
      const data = await res.json();
      if (data.found) {
        setLicense({
          license_key: data.license_key,
          tier: data.tier,
          expires_at: data.expires_at,
          status: data.status,
          used_seats: data.used_seats,
          max_seats: data.max_seats,
          usage: data.usage,
        });
        setLicenseKey(data.license_key);
        setView('dashboard');
        localStorage.setItem('omg_email', userEmail);
        localStorage.setItem('omg_license_key', data.license_key);
      } else {
        localStorage.removeItem('omg_email');
        localStorage.removeItem('omg_license_key');
      }
    } catch (e) {
      console.error(e);
    }
    setLoading(false);
  };

  const fetchLicense = async () => {
    const userEmail = email();
    if (!userEmail) {
      setError('Please enter your email');
      return;
    }
    setLoading(true);
    setError('');
    try {
      const res = await fetch(`${API_BASE}/api/get-license?email=${encodeURIComponent(userEmail)}`);
      const data = await res.json();
      if (data.found) {
        setLicense({
          license_key: data.license_key,
          tier: data.tier,
          expires_at: data.expires_at,
          status: data.status,
          used_seats: data.used_seats,
          max_seats: data.max_seats,
          usage: data.usage,
        });
        setLicenseKey(data.license_key);
        setView('dashboard');
        localStorage.setItem('omg_email', userEmail);
        localStorage.setItem('omg_license_key', data.license_key);
      } else {
        setError('No license found for this email. Check your email or create a free account.');
      }
    } catch (e) {
      setError('Failed to connect to license server. Please try again.');
    }
    setLoading(false);
  };

  const registerFreeAccount = async () => {
    const userEmail = email();
    if (!userEmail) {
      setError('Please enter your email');
      return;
    }
    setLoading(true);
    setError('');
    try {
      const res = await fetch(`${API_BASE}/api/register-free`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: userEmail }),
      });
      const data = await res.json();
      if (data.success) {
        setLicense({
          license_key: data.license_key,
          tier: 'free',
          expires_at: 'Never',
          status: 'active',
          usage: data.usage,
        });
        setLicenseKey(data.license_key);
        setView('dashboard');
        localStorage.setItem('omg_email', userEmail);
        localStorage.setItem('omg_license_key', data.license_key);
      } else {
        setError(data.error || 'Registration failed. Please try again.');
      }
    } catch (e) {
      setError('Failed to connect to server. Please try again.');
    }
    setLoading(false);
  };

  const refreshLicense = async () => {
    setActionLoading(true);
    setActionMessage('');
    try {
      const res = await fetch(`${API_BASE}/api/refresh-license`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ license_key: licenseKey() }),
      });
      const data = await res.json();
      if (data.success) {
        setLicense(prev => prev ? { ...prev, ...data.license } : null);
        setActionMessage('License refreshed successfully!');
      } else {
        setActionMessage(data.error || 'Failed to refresh license');
      }
    } catch (e) {
      setActionMessage('Failed to connect to server');
    }
    setActionLoading(false);
    setTimeout(() => setActionMessage(''), 3000);
  };

  const regenerateLicense = async () => {
    if (!confirm('This will invalidate your current license key. All machines will need to re-activate. Continue?')) {
      return;
    }
    setActionLoading(true);
    setActionMessage('');
    try {
      const res = await fetch(`${API_BASE}/api/regenerate-license`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: email(), old_license_key: licenseKey() }),
      });
      const data = await res.json();
      if (data.success) {
        setLicense(prev => prev ? { ...prev, license_key: data.new_license_key } : null);
        setLicenseKey(data.new_license_key);
        localStorage.setItem('omg_license_key', data.new_license_key);
        setActionMessage('New license key generated!');
      } else {
        setActionMessage(data.error || 'Failed to regenerate license');
      }
    } catch (e) {
      setActionMessage('Failed to connect to server');
    }
    setActionLoading(false);
  };

  const openBillingPortal = async () => {
    setActionLoading(true);
    try {
      const res = await fetch(`${API_BASE}/api/billing-portal`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: email() }),
      });
      const data = await res.json();
      if (data.success && data.url) {
        window.location.href = data.url;
      } else {
        setActionMessage(data.error || 'Failed to open billing portal');
      }
    } catch (e) {
      setActionMessage('Failed to connect to server');
    }
    setActionLoading(false);
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const logout = () => {
    setView('login');
    setLicense(null);
    setEmail('');
    setLicenseKey('');
    setError('');
    localStorage.removeItem('omg_email');
    localStorage.removeItem('omg_license_key');
  };

  const getTierColor = (tier: string) => {
    switch (tier.toLowerCase()) {
      case 'pro': return 'from-indigo-500 to-blue-500';
      case 'team': return 'from-purple-500 to-pink-500';
      case 'enterprise': return 'from-amber-500 to-orange-500';
      default: return 'from-emerald-500 to-teal-500';
    }
  };

  const getTierBadgeColor = (tier: string) => {
    switch (tier.toLowerCase()) {
      case 'pro': return 'bg-indigo-500/20 text-indigo-400 border-indigo-500/30';
      case 'team': return 'bg-purple-500/20 text-purple-400 border-purple-500/30';
      case 'enterprise': return 'bg-amber-500/20 text-amber-400 border-amber-500/30';
      default: return 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30';
    }
  };

  const formatDate = (dateStr: string) => {
    if (!dateStr || dateStr === 'Never') return 'Never';
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' });
  };

  return (
    <div class="min-h-screen bg-[#0a0a1a]">
      {/* Background Effects */}
      <div class="fixed inset-0 overflow-hidden pointer-events-none">
        <div class="absolute top-0 left-1/4 w-96 h-96 bg-indigo-500/10 rounded-full blur-3xl" />
        <div class="absolute bottom-0 right-1/4 w-96 h-96 bg-purple-500/10 rounded-full blur-3xl" />
        <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-cyan-500/5 rounded-full blur-3xl" />
      </div>

      {/* Header */}
      <header class="relative z-10 border-b border-white/5 bg-[#0f0f23]/80 backdrop-blur-xl">
        <div class="max-w-7xl mx-auto px-6 py-4 flex items-center justify-between">
          <A href="/" class="flex items-center gap-3 hover:opacity-80 transition-opacity">
            <img src="/favicon.svg" alt="OMG" class="w-8 h-8 rounded-lg" />
            <span class="text-xl font-bold text-white">OMG</span>
          </A>
          <Show when={view() === 'dashboard'}>
            <div class="flex items-center gap-4">
              <span class="text-slate-400 text-sm hidden sm:block">{email()}</span>
              <button
                onClick={logout}
                class="flex items-center gap-2 px-4 py-2 text-slate-400 hover:text-white hover:bg-slate-800/50 rounded-lg transition-all text-sm"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                </svg>
                Sign Out
              </button>
            </div>
          </Show>
        </div>
      </header>

      <main class="relative z-10 max-w-7xl mx-auto px-6 py-12">
        {/* Login/Register View */}
        <Show when={view() !== 'dashboard'}>
          <div class="max-w-md mx-auto">
            <div class="text-center mb-10">
              <h1 class="text-4xl font-bold text-white mb-3">
                {view() === 'login' ? 'Welcome Back' : 'Get Started Free'}
              </h1>
              <p class="text-slate-400">
                {view() === 'login' 
                  ? 'Sign in to access your OMG dashboard' 
                  : 'Create a free account to track your productivity'}
              </p>
            </div>

            <div class="bg-slate-900/50 border border-slate-800 rounded-2xl p-8 backdrop-blur-sm">
              <div class="space-y-6">
                <div>
                  <label class="block text-sm font-medium text-slate-300 mb-2">Email Address</label>
                  <input
                    type="email"
                    value={email()}
                    onInput={(e) => setEmail(e.currentTarget.value)}
                    onKeyPress={(e) => e.key === 'Enter' && (view() === 'login' ? fetchLicense() : registerFreeAccount())}
                    placeholder="you@company.com"
                    class="w-full px-4 py-3 bg-slate-800/50 border border-slate-700 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 transition-all"
                  />
                </div>

                <Show when={error()}>
                  <div class="flex items-start gap-3 p-4 bg-red-500/10 border border-red-500/30 rounded-xl">
                    <svg class="w-5 h-5 text-red-400 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <p class="text-red-400 text-sm">{error()}</p>
                  </div>
                </Show>

                <button
                  onClick={view() === 'login' ? fetchLicense : registerFreeAccount}
                  disabled={loading() || !email()}
                  class={`w-full py-4 font-semibold rounded-xl transition-all flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed ${
                    view() === 'login'
                      ? 'bg-gradient-to-r from-indigo-500 to-blue-500 hover:from-indigo-400 hover:to-blue-400 text-white'
                      : 'bg-gradient-to-r from-emerald-500 to-teal-500 hover:from-emerald-400 hover:to-teal-400 text-white'
                  }`}
                >
                  {loading() ? (
                    <>
                      <svg class="w-5 h-5 animate-spin" fill="none" viewBox="0 0 24 24">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                      </svg>
                      {view() === 'login' ? 'Signing in...' : 'Creating account...'}
                    </>
                  ) : (
                    view() === 'login' ? 'Sign In' : 'Create Free Account'
                  )}
                </button>

                <div class="text-center">
                  <button
                    onClick={() => { setView(view() === 'login' ? 'register' : 'login'); setError(''); }}
                    class="text-indigo-400 hover:text-indigo-300 text-sm transition-colors"
                  >
                    {view() === 'login' ? "Don't have an account? Create one free" : 'Already have an account? Sign in'}
                  </button>
                </div>
              </div>
            </div>

            <Show when={view() === 'register'}>
              <div class="mt-8 grid grid-cols-2 gap-4">
                <div class="bg-slate-900/30 border border-slate-800 rounded-xl p-4 text-center">
                  <div class="text-2xl mb-1">📊</div>
                  <div class="text-sm text-white font-medium">Usage Analytics</div>
                  <div class="text-xs text-slate-500">Track your productivity</div>
                </div>
                <div class="bg-slate-900/30 border border-slate-800 rounded-xl p-4 text-center">
                  <div class="text-2xl mb-1">⏱️</div>
                  <div class="text-sm text-white font-medium">Time Saved</div>
                  <div class="text-xs text-slate-500">See your efficiency gains</div>
                </div>
                <div class="bg-slate-900/30 border border-slate-800 rounded-xl p-4 text-center">
                  <div class="text-2xl mb-1">🏆</div>
                  <div class="text-sm text-white font-medium">Achievements</div>
                  <div class="text-xs text-slate-500">Unlock badges & rewards</div>
                </div>
                <div class="bg-slate-900/30 border border-slate-800 rounded-xl p-4 text-center">
                  <div class="text-2xl mb-1">🔥</div>
                  <div class="text-sm text-white font-medium">Streaks</div>
                  <div class="text-xs text-slate-500">Build daily habits</div>
                </div>
              </div>
            </Show>
          </div>
        </Show>

        {/* Dashboard View */}
        <Show when={view() === 'dashboard' && license()}>
          <div class="space-y-8">
            {/* Welcome Header */}
            <div class="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
              <div>
                <h1 class="text-3xl font-bold text-white mb-1">Dashboard</h1>
                <p class="text-slate-400">Welcome back! Here's your OMG overview.</p>
              </div>
              <div class="flex items-center gap-3">
                <div class={`px-4 py-2 rounded-full border ${getTierBadgeColor(license()!.tier)}`}>
                  <span class="font-semibold uppercase text-sm">{license()!.tier}</span>
                </div>
                <Show when={license()!.tier.toLowerCase() === 'free'}>
                  <A href="/#pricing" class="px-4 py-2 bg-gradient-to-r from-indigo-500 to-purple-500 hover:from-indigo-400 hover:to-purple-400 text-white font-medium rounded-full text-sm transition-all">
                    Upgrade to Pro
                  </A>
                </Show>
              </div>
            </div>

            {/* Stats Grid */}
            <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
              <div class="bg-gradient-to-br from-emerald-500/10 to-teal-500/10 border border-emerald-500/20 rounded-2xl p-6">
                <div class="flex items-center gap-3 mb-3">
                  <div class="p-2 bg-emerald-500/20 rounded-lg">
                    <svg class="w-5 h-5 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                  </div>
                  <span class="text-slate-400 text-sm">Time Saved</span>
                </div>
                <div class="text-3xl font-bold text-white">
                  {license()!.usage?.time_saved_ms ? formatTimeSaved(license()!.usage!.time_saved_ms) : '0s'}
                </div>
                <div class="text-emerald-400 text-sm mt-1">vs manual operations</div>
              </div>

              <div class="bg-gradient-to-br from-cyan-500/10 to-blue-500/10 border border-cyan-500/20 rounded-2xl p-6">
                <div class="flex items-center gap-3 mb-3">
                  <div class="p-2 bg-cyan-500/20 rounded-lg">
                    <svg class="w-5 h-5 text-cyan-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
                    </svg>
                  </div>
                  <span class="text-slate-400 text-sm">Total Commands</span>
                </div>
                <div class="text-3xl font-bold text-white">
                  {(license()!.usage?.total_commands || 0).toLocaleString()}
                </div>
                <div class="text-cyan-400 text-sm mt-1">lifetime executions</div>
              </div>

              <div class="bg-gradient-to-br from-orange-500/10 to-amber-500/10 border border-orange-500/20 rounded-2xl p-6">
                <div class="flex items-center gap-3 mb-3">
                  <div class="p-2 bg-orange-500/20 rounded-lg">
                    <span class="text-lg">🔥</span>
                  </div>
                  <span class="text-slate-400 text-sm">Current Streak</span>
                </div>
                <div class="text-3xl font-bold text-white">
                  {license()!.usage?.current_streak || 0} days
                </div>
                <div class="text-orange-400 text-sm mt-1">
                  Best: {license()!.usage?.longest_streak || 0} days
                </div>
              </div>

              <div class="bg-gradient-to-br from-indigo-500/10 to-purple-500/10 border border-indigo-500/20 rounded-2xl p-6">
                <div class="flex items-center gap-3 mb-3">
                  <div class="p-2 bg-indigo-500/20 rounded-lg">
                    <svg class="w-5 h-5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                    </svg>
                  </div>
                  <span class="text-slate-400 text-sm">Today</span>
                </div>
                <div class="text-3xl font-bold text-white">
                  {license()!.usage?.queries_today || 0}
                </div>
                <div class="text-indigo-400 text-sm mt-1">commands run</div>
              </div>
            </div>

            {/* Main Content Grid */}
            <div class="grid lg:grid-cols-3 gap-6">
              {/* License Card */}
              <div class="lg:col-span-2 space-y-6">
                {/* License Key Section */}
                <div class="bg-slate-900/50 border border-slate-800 rounded-2xl p-6 backdrop-blur-sm">
                  <div class="flex items-center justify-between mb-4">
                    <h2 class="text-lg font-semibold text-white flex items-center gap-2">
                      <svg class="w-5 h-5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                      </svg>
                      License Key
                    </h2>
                    <button
                      onClick={() => copyToClipboard(licenseKey())}
                      class="flex items-center gap-2 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 rounded-lg text-sm text-slate-300 hover:text-white transition-all"
                    >
                      {copied() ? (
                        <>
                          <svg class="w-4 h-4 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                          </svg>
                          Copied!
                        </>
                      ) : (
                        <>
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                          </svg>
                          Copy
                        </>
                      )}
                    </button>
                  </div>
                  <code class="block text-green-400 font-mono text-sm break-all bg-slate-950 p-4 rounded-xl border border-slate-800">
                    {licenseKey()}
                  </code>
                  <div class="mt-4 p-4 bg-slate-800/50 rounded-xl">
                    <p class="text-slate-400 text-sm mb-2">Activate on a new machine:</p>
                    <code class="text-cyan-400 font-mono text-sm">omg license activate {licenseKey()}</code>
                  </div>
                </div>

                {/* Achievements Section */}
                <div class="bg-slate-900/50 border border-slate-800 rounded-2xl p-6 backdrop-blur-sm">
                  <h2 class="text-lg font-semibold text-white flex items-center gap-2 mb-4">
                    <span class="text-xl">🏆</span>
                    Achievements
                    <span class="text-sm font-normal text-slate-500">
                      ({license()!.usage?.achievements?.length || 0}/{ALL_ACHIEVEMENTS.length})
                    </span>
                  </h2>
                  <div class="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-6 gap-3">
                    <For each={ALL_ACHIEVEMENTS}>
                      {(achievement) => {
                        const unlocked = license()!.usage?.achievements?.includes(achievement.id);
                        return (
                          <div
                            class={`relative group p-3 rounded-xl text-center transition-all cursor-help ${
                              unlocked
                                ? 'bg-gradient-to-br from-indigo-500/20 to-purple-500/20 border border-indigo-500/30'
                                : 'bg-slate-800/30 border border-slate-700/30 opacity-40'
                            }`}
                            title={`${achievement.name}: ${achievement.description}`}
                          >
                            <div class={`text-2xl ${unlocked ? '' : 'grayscale'}`}>{achievement.emoji}</div>
                            <div class={`text-xs mt-1 truncate ${unlocked ? 'text-slate-300' : 'text-slate-500'}`}>
                              {achievement.name}
                            </div>
                            {/* Tooltip */}
                            <div class="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-xs text-left w-48 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-10 pointer-events-none">
                              <div class="font-medium text-white mb-1">{achievement.name}</div>
                              <div class="text-slate-400">{achievement.description}</div>
                              <div class={`mt-1 ${unlocked ? 'text-green-400' : 'text-slate-500'}`}>
                                {unlocked ? '✓ Unlocked' : '🔒 Locked'}
                              </div>
                            </div>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                </div>

                {/* Security Stats (Pro+ only) */}
                <Show when={license()!.tier.toLowerCase() !== 'free'}>
                  <div class="bg-slate-900/50 border border-slate-800 rounded-2xl p-6 backdrop-blur-sm">
                    <h2 class="text-lg font-semibold text-white flex items-center gap-2 mb-4">
                      <span class="text-xl">🛡️</span>
                      Security Overview
                    </h2>
                    <div class="grid grid-cols-2 gap-4">
                      <div class="bg-slate-800/50 rounded-xl p-4">
                        <div class="text-3xl font-bold text-green-400">{license()!.usage?.sbom_generated || 0}</div>
                        <div class="text-sm text-slate-400 mt-1">SBOMs Generated</div>
                      </div>
                      <div class="bg-slate-800/50 rounded-xl p-4">
                        <div class="text-3xl font-bold text-amber-400">{license()!.usage?.vulnerabilities_found || 0}</div>
                        <div class="text-sm text-slate-400 mt-1">Vulnerabilities Found</div>
                      </div>
                    </div>
                  </div>
                </Show>
              </div>

              {/* Sidebar */}
              <div class="space-y-6">
                {/* Account Status */}
                <div class={`bg-gradient-to-br ${getTierColor(license()!.tier)} p-[1px] rounded-2xl`}>
                  <div class="bg-slate-900 rounded-2xl p-6">
                    <div class="flex items-center gap-3 mb-4">
                      <div class="w-12 h-12 bg-gradient-to-br from-slate-700 to-slate-800 rounded-full flex items-center justify-center text-xl">
                        {email().charAt(0).toUpperCase()}
                      </div>
                      <div>
                        <div class="text-white font-medium truncate max-w-[150px]">{email()}</div>
                        <div class={`text-sm ${license()!.status === 'active' ? 'text-green-400' : 'text-red-400'}`}>
                          {license()!.status === 'active' ? '● Active' : '○ Inactive'}
                        </div>
                      </div>
                    </div>
                    <div class="space-y-3 text-sm">
                      <div class="flex justify-between">
                        <span class="text-slate-400">Plan</span>
                        <span class="text-white font-medium capitalize">{license()!.tier}</span>
                      </div>
                      <div class="flex justify-between">
                        <span class="text-slate-400">Expires</span>
                        <span class="text-white">{formatDate(license()!.expires_at)}</span>
                      </div>
                      <Show when={license()!.max_seats}>
                        <div class="flex justify-between">
                          <span class="text-slate-400">Seats</span>
                          <span class="text-white">{license()!.used_seats || 0} / {license()!.max_seats}</span>
                        </div>
                      </Show>
                    </div>
                  </div>
                </div>

                {/* Quick Actions */}
                <div class="bg-slate-900/50 border border-slate-800 rounded-2xl p-6 backdrop-blur-sm">
                  <h3 class="text-white font-semibold mb-4">Quick Actions</h3>
                  <div class="space-y-3">
                    <button
                      onClick={refreshLicense}
                      disabled={actionLoading()}
                      class="w-full flex items-center gap-3 px-4 py-3 bg-slate-800/50 hover:bg-slate-700/50 border border-slate-700 rounded-xl text-white transition-all disabled:opacity-50"
                    >
                      <svg class={`w-5 h-5 text-cyan-400 ${actionLoading() ? 'animate-spin' : ''}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                      Refresh License
                    </button>
                    <button
                      onClick={openBillingPortal}
                      disabled={actionLoading()}
                      class="w-full flex items-center gap-3 px-4 py-3 bg-slate-800/50 hover:bg-slate-700/50 border border-slate-700 rounded-xl text-white transition-all disabled:opacity-50"
                    >
                      <svg class="w-5 h-5 text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z" />
                      </svg>
                      Manage Billing
                    </button>
                    <button
                      onClick={regenerateLicense}
                      disabled={actionLoading()}
                      class="w-full flex items-center gap-3 px-4 py-3 bg-amber-500/10 hover:bg-amber-500/20 border border-amber-500/30 rounded-xl text-amber-400 transition-all disabled:opacity-50"
                    >
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                      </svg>
                      Regenerate Key
                    </button>
                  </div>
                </div>

                {/* Action Message */}
                <Show when={actionMessage()}>
                  <div class={`p-4 rounded-xl text-sm ${
                    actionMessage().includes('success') || actionMessage().includes('generated')
                      ? 'bg-green-500/10 border border-green-500/30 text-green-400'
                      : 'bg-amber-500/10 border border-amber-500/30 text-amber-400'
                  }`}>
                    {actionMessage()}
                  </div>
                </Show>

                {/* Help */}
                <div class="bg-slate-900/30 border border-slate-800 rounded-2xl p-6">
                  <h3 class="text-white font-semibold mb-3 flex items-center gap-2">
                    <svg class="w-5 h-5 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    Need Help?
                  </h3>
                  <ul class="text-sm text-slate-400 space-y-2">
                    <li>
                      <a href="https://github.com/PyRo1121/omg/wiki" class="hover:text-white transition-colors">
                        📚 Documentation
                      </a>
                    </li>
                    <li>
                      <a href="https://github.com/PyRo1121/omg/issues" class="hover:text-white transition-colors">
                        🐛 Report an Issue
                      </a>
                    </li>
                    <li>
                      <a href="mailto:support@pyro1121.com" class="hover:text-white transition-colors">
                        ✉️ Contact Support
                      </a>
                    </li>
                  </ul>
                </div>
              </div>
            </div>
          </div>
        </Show>
      </main>

      {/* Footer */}
      <footer class="relative z-10 border-t border-white/5 mt-20">
        <div class="max-w-7xl mx-auto px-6 py-8 flex flex-col sm:flex-row items-center justify-between gap-4">
          <div class="text-slate-500 text-sm">
            © {new Date().getFullYear()} OMG Package Manager. All rights reserved.
          </div>
          <div class="flex items-center gap-6 text-sm">
            <A href="/" class="text-slate-400 hover:text-white transition-colors">Home</A>
            <a href="https://github.com/PyRo1121/omg" class="text-slate-400 hover:text-white transition-colors">GitHub</a>
            <a href="mailto:support@pyro1121.com" class="text-slate-400 hover:text-white transition-colors">Support</a>
          </div>
        </div>
      </footer>
    </div>
  );
};

export default DashboardPage;
