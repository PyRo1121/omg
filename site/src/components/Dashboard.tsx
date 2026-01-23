import { Component, createSignal, Show, For } from 'solid-js';

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
}

const ACHIEVEMENTS: Record<string, Achievement> = {
  FirstStep: {
    id: 'FirstStep',
    emoji: '🚀',
    name: 'First Step',
    description: 'Executed your first command',
  },
  Centurion: {
    id: 'Centurion',
    emoji: '💯',
    name: 'Centurion',
    description: 'Executed 100 commands',
  },
  PowerUser: {
    id: 'PowerUser',
    emoji: '⚡',
    name: 'Power User',
    description: 'Executed 1,000 commands',
  },
  Legend: { id: 'Legend', emoji: '🏆', name: 'Legend', description: 'Executed 10,000 commands' },
  MinuteSaver: {
    id: 'MinuteSaver',
    emoji: '⏱️',
    name: 'Minute Saver',
    description: 'Saved 1 minute of time',
  },
  HourSaver: {
    id: 'HourSaver',
    emoji: '⏰',
    name: 'Hour Saver',
    description: 'Saved 1 hour of time',
  },
  DaySaver: {
    id: 'DaySaver',
    emoji: '📅',
    name: 'Day Saver',
    description: 'Saved 24 hours of time',
  },
  WeekStreak: {
    id: 'WeekStreak',
    emoji: '🔥',
    name: 'Week Streak',
    description: 'Used OMG for 7 days straight',
  },
  MonthStreak: {
    id: 'MonthStreak',
    emoji: '💎',
    name: 'Month Streak',
    description: 'Used OMG for 30 days straight',
  },
  Polyglot: {
    id: 'Polyglot',
    emoji: '🌐',
    name: 'Polyglot',
    description: 'Used all 7 built-in runtimes',
  },
  SecurityFirst: {
    id: 'SecurityFirst',
    emoji: '🛡️',
    name: 'Security First',
    description: 'Generated your first SBOM',
  },
  BugHunter: {
    id: 'BugHunter',
    emoji: '🐛',
    name: 'Bug Hunter',
    description: 'Found and addressed vulnerabilities',
  },
};

const formatTimeSaved = (ms: number): string => {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 3600000) return `${(ms / 60000).toFixed(1)}min`;
  return `${(ms / 3600000).toFixed(1)}hr`;
};

const Dashboard: Component<{ isOpen: boolean; onClose: () => void }> = props => {
  const [email, setEmail] = createSignal('');
  const [licenseKey, setLicenseKey] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal('');
  const [license, setLicense] = createSignal<LicenseInfo | null>(null);
  const [view, setView] = createSignal<'login' | 'register' | 'dashboard'>('login');
  const [actionLoading, setActionLoading] = createSignal(false);
  const [actionMessage, setActionMessage] = createSignal('');
  const [copied, setCopied] = createSignal(false);
  const [_registerSuccess, setRegisterSuccess] = createSignal(false);

  const API_BASE = 'https://api.pyro1121.com';

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
      } else {
        setError('No license found for this email. Check your email or purchase a license.');
      }
    } catch (_e) {
      setError('Failed to connect to license server. Please try again.');
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
        setLicense(prev => (prev ? { ...prev, ...data.license } : null));
        setActionMessage('License refreshed successfully!');
      } else {
        setActionMessage(data.error || 'Failed to refresh license');
      }
    } catch (_e) {
      setActionMessage('Failed to connect to server');
    }

    setActionLoading(false);
    setTimeout(() => setActionMessage(''), 3000);
  };

  const regenerateLicense = async () => {
    if (
      !confirm(
        'This will invalidate your current license key. All machines will need to re-activate. Continue?'
      )
    ) {
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
        setLicense(prev => (prev ? { ...prev, license_key: data.new_license_key } : null));
        setLicenseKey(data.new_license_key);
        setActionMessage(
          'New license key generated! Update your machines with: omg license activate ' +
            data.new_license_key
        );
      } else {
        setActionMessage(data.error || 'Failed to regenerate license');
      }
    } catch (_e) {
      setActionMessage('Failed to connect to server');
    }

    setActionLoading(false);
  };

  const _revokeMachine = async (machineId: string) => {
    if (!confirm('Revoke access for this machine?')) return;

    setActionLoading(true);
    try {
      const res = await fetch(`${API_BASE}/api/revoke-machine`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ license_key: licenseKey(), machine_id: machineId }),
      });
      const data = await res.json();

      if (data.success) {
        setActionMessage('Machine access revoked');
        // Refresh license info
        await fetchLicense();
      } else {
        setActionMessage(data.error || 'Failed to revoke machine');
      }
    } catch (_e) {
      setActionMessage('Failed to connect to server');
    }
    setActionLoading(false);
  };

  const openBillingPortal = async () => {
    setActionLoading(true);
    setActionMessage('');

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
    } catch (_e) {
      setActionMessage('Failed to connect to server');
    }

    setActionLoading(false);
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
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
        setRegisterSuccess(true);
        setLicense({
          license_key: data.license_key,
          tier: 'free',
          expires_at: 'Never',
          status: 'active',
          usage: data.usage,
        });
        setLicenseKey(data.license_key);
        setView('dashboard');
      } else {
        setError(data.error || 'Registration failed. Please try again.');
      }
    } catch (_e) {
      setError('Failed to connect to server. Please try again.');
    }

    setLoading(false);
  };

  const logout = () => {
    setView('login');
    setLicense(null);
    setEmail('');
    setLicenseKey('');
    setError('');
    setRegisterSuccess(false);
  };

  const getTierColor = (tier: string) => {
    switch (tier.toLowerCase()) {
      case 'pro':
        return 'from-indigo-500/20 to-indigo-500/5 border-indigo-500/30 text-indigo-400';
      case 'team':
        return 'from-cyan-500/20 to-cyan-500/5 border-cyan-500/30 text-cyan-400';
      case 'enterprise':
        return 'from-amber-500/20 to-amber-500/5 border-amber-500/30 text-amber-400';
      default:
        return 'from-slate-500/20 to-slate-500/5 border-slate-500/30 text-slate-400';
    }
  };

  const formatDate = (dateStr: string) => {
    if (!dateStr || dateStr === 'Never') return 'Never';
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' });
  };

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 z-[100] flex items-center justify-center p-4 sm:p-6"
        onClick={e => e.target === e.currentTarget && props.onClose()}
      >
        <div class="animate-fade-in absolute inset-0 bg-black/90 backdrop-blur-xl" />

          <div class="animate-scale-in relative flex max-h-[90vh] w-full max-w-4xl flex-col overflow-hidden rounded-[2rem] border border-white/10 bg-[#0a0a0b] shadow-2xl shadow-black">
            <div class="pointer-events-none absolute -top-[30%] -left-[10%] h-[70%] w-[70%] rounded-full bg-indigo-500/10 blur-[120px]" />
            <div class="pointer-events-none absolute -bottom-[30%] -right-[10%] h-[70%] w-[70%] rounded-full bg-purple-500/10 blur-[120px]" />

            <div class="flex items-center justify-between border-b border-white/[0.05] bg-white/[0.02] px-8 py-6 backdrop-blur-md">

            <div class="flex items-center gap-4">
              <div class="flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 shadow-lg shadow-indigo-500/20">
                <svg class="h-6 w-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
              </div>
              <div>
                <h2 class="text-xl font-bold tracking-tight text-white">OMG Dashboard</h2>
                <div class="flex items-center gap-2">
                  <span class="flex h-2 w-2 rounded-full bg-emerald-500" />
                  <p class="text-xs font-medium text-slate-400">System Status: Operational</p>
                </div>
              </div>
            </div>
            <button
              onClick={props.onClose}
              class="flex h-10 w-10 items-center justify-center rounded-full bg-white/[0.05] text-slate-400 transition-all hover:bg-white/10 hover:text-white"
            >
              <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          <div class="flex-1 overflow-y-auto px-8 py-8">
            <Show when={view() === 'login'}>
              <div class="mx-auto max-w-md space-y-8 py-12">
                <div class="text-center">
                  <h3 class="text-3xl font-bold text-white">Welcome Back</h3>
                  <p class="mt-2 text-slate-400">Enter your email to manage your license and view stats</p>
                </div>

                <div class="space-y-4">
                  <div class="relative group">
                    <input
                      type="email"
                      value={email()}
                      onInput={e => setEmail(e.currentTarget.value)}
                      onKeyPress={e => e.key === 'Enter' && fetchLicense()}
                      placeholder="name@company.com"
                      class="w-full rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-4 text-white placeholder-slate-500 transition-all focus:border-indigo-500 focus:bg-white/[0.05] focus:outline-none focus:ring-4 focus:ring-indigo-500/10"
                    />
                  </div>

                  <Show when={error()}>
                    <div class="flex items-center gap-3 rounded-2xl border border-red-500/20 bg-red-500/10 p-4 text-sm text-red-400">
                      <svg class="h-5 w-5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                      </svg>
                      {error()}
                    </div>
                  </Show>

                  <button
                    onClick={fetchLicense}
                    disabled={loading() || !email()}
                    class="relative flex w-full items-center justify-center gap-3 overflow-hidden rounded-2xl bg-white px-8 py-4 font-bold text-black transition-all hover:scale-[1.02] active:scale-[0.98] disabled:opacity-50"
                  >
                    <Show when={loading()}>
                      <svg class="h-5 w-5 animate-spin" fill="none" viewBox="0 0 24 24">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                      </svg>
                    </Show>
                    {loading() ? 'Verifying...' : 'Access Dashboard'}
                  </button>
                </div>

                <div class="flex flex-col gap-4 text-center">
                  <p class="text-sm text-slate-500">
                    Don't have an account?{' '}
                    <button onClick={() => setView('register')} class="font-semibold text-indigo-400 hover:text-indigo-300">
                      Create free account
                    </button>
                  </p>
                </div>
              </div>
            </Show>

            <Show when={view() === 'register'}>
              <div class="mx-auto max-w-md space-y-8 py-12">
                <div class="text-center">
                  <h3 class="text-3xl font-bold text-white">Join the Elite</h3>
                  <p class="mt-2 text-slate-400">Start tracking your productivity and managing packages faster</p>
                </div>

                <div class="space-y-4">
                  <input
                    type="email"
                    value={email()}
                    onInput={e => setEmail(e.currentTarget.value)}
                    onKeyPress={e => e.key === 'Enter' && registerFreeAccount()}
                    placeholder="name@email.com"
                    class="w-full rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-4 text-white placeholder-slate-500 transition-all focus:border-emerald-500 focus:bg-white/[0.05] focus:outline-none focus:ring-4 focus:ring-emerald-500/10"
                  />

                  <Show when={error()}>
                    <div class="flex items-center gap-3 rounded-2xl border border-red-500/20 bg-red-500/10 p-4 text-sm text-red-400">
                      {error()}
                    </div>
                  </Show>

                  <button
                    onClick={registerFreeAccount}
                    disabled={loading() || !email()}
                    class="flex w-full items-center justify-center gap-3 rounded-2xl bg-gradient-to-r from-emerald-500 to-teal-500 px-8 py-4 font-bold text-white transition-all hover:scale-[1.02] active:scale-[0.98] disabled:opacity-50"
                  >
                    {loading() ? 'Creating...' : 'Create Free Account'}
                  </button>
                </div>

                <div class="rounded-2xl border border-white/[0.05] bg-white/[0.02] p-6">
                  <p class="mb-3 text-xs font-bold uppercase tracking-wider text-slate-500">Free Tier Includes</p>
                  <ul class="grid grid-cols-2 gap-3 text-sm text-slate-400">
                    <li class="flex items-center gap-2">
                      <span class="text-emerald-500">✓</span> Usage Tracking
                    </li>
                    <li class="flex items-center gap-2">
                      <span class="text-emerald-500">✓</span> Analytics
                    </li>
                    <li class="flex items-center gap-2">
                      <span class="text-emerald-500">✓</span> Time Saved
                    </li>
                    <li class="flex items-center gap-2">
                      <span class="text-emerald-500">✓</span> Achievements
                    </li>
                  </ul>
                </div>

                <p class="text-center text-sm text-slate-500">
                  Already have an account?{' '}
                  <button onClick={() => setView('login')} class="font-semibold text-indigo-400 hover:text-indigo-300">
                    Sign in
                  </button>
                </p>
              </div>
            </Show>

            <Show when={view() === 'dashboard' && license()}>
              <div class="grid grid-cols-1 gap-8 lg:grid-cols-3">
                <div class="space-y-6 lg:col-span-2">
                  <div class={`relative overflow-hidden rounded-3xl border p-6 bg-gradient-to-br ${getTierColor(license()!.tier)}`}>
                    <div class="relative z-10 flex items-center justify-between">
                      <div>
                        <div class="flex items-center gap-3">
                          <h3 class="text-2xl font-bold text-white">{license()!.tier.toUpperCase()}</h3>
                          <span class={`rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider ${
                            license()!.status === 'active' ? 'bg-emerald-500/20 text-emerald-400' : 'bg-red-500/20 text-red-400'
                          }`}>
                            {license()!.status}
                          </span>
                        </div>
                        <p class="mt-1 text-sm opacity-70">License active for {email()}</p>
                      </div>
                      <div class="text-right">
                        <p class="text-[10px] font-bold uppercase tracking-wider opacity-50">Expires</p>
                        <p class="text-sm font-medium">{formatDate(license()!.expires_at)}</p>
                      </div>
                    </div>
                  </div>

                  <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
                    <div class="rounded-3xl border border-white/[0.05] bg-white/[0.02] p-6 transition-all hover:bg-white/[0.04]">
                      <div class="flex items-center justify-between">
                        <div>
                          <p class="text-sm font-medium text-slate-500">Productivity Gained</p>
                          <h4 class="mt-1 text-3xl font-bold text-emerald-400">
                            {license()!.usage?.time_saved_ms ? formatTimeSaved(license()!.usage!.time_saved_ms) : '0ms'}
                          </h4>
                        </div>
                        <div class="rounded-2xl bg-emerald-500/10 p-3 text-emerald-400">
                          <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                          </svg>
                        </div>
                      </div>
                      <div class="mt-4 flex items-center gap-2">
                        <div class="h-1.5 flex-1 overflow-hidden rounded-full bg-slate-800">
                          <div class="h-full w-[65%] rounded-full bg-emerald-500 shadow-[0_0_12px_rgba(16,185,129,0.4)]" />
                        </div>
                        <span class="text-[10px] font-bold text-emerald-500">+12%</span>
                      </div>
                    </div>

                    <div class="rounded-3xl border border-white/[0.05] bg-white/[0.02] p-6 transition-all hover:bg-white/[0.04]">
                      <div class="flex items-center justify-between">
                        <div>
                          <p class="text-sm font-medium text-slate-500">Daily Streak</p>
                          <h4 class="mt-1 text-3xl font-bold text-orange-400">
                            {license()!.usage?.current_streak || 0} Days
                          </h4>
                        </div>
                        <div class="rounded-2xl bg-orange-500/10 p-3 text-orange-400">
                          <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17.657 18.657A8 8 0 016.343 7.343S7 9 9 10c0-2 .5-5 2.5-7 3 3 3 6 3 6s.5 1 2 2c2 2 2.5 7 .5 10.657z" />
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 13a3 3 0 11-6 0 3 3 0 016 0z" />
                          </svg>
                        </div>
                      </div>
                      <p class="mt-4 text-xs text-slate-500">Personal Best: {license()!.usage?.longest_streak || 0} days</p>
                    </div>
                  </div>

                  <div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
                    {[
                      { label: 'Total Cmds', value: license()!.usage?.total_commands || 0, color: 'text-indigo-400' },
                      { label: 'Today', value: license()!.usage?.queries_today || 0, color: 'text-cyan-400' },
                      { label: 'SBOMs', value: license()!.usage?.sbom_generated || 0, color: 'text-purple-400' },
                      { label: 'CVEs Found', value: license()!.usage?.vulnerabilities_found || 0, color: 'text-rose-400' },
                    ].map(stat => (
                      <div class="rounded-2xl border border-white/[0.05] bg-white/[0.01] p-4 text-center">
                        <p class="text-[10px] font-bold uppercase tracking-wider text-slate-500">{stat.label}</p>
                        <p class={`mt-1 text-xl font-bold ${stat.color}`}>{stat.value.toLocaleString()}</p>
                      </div>
                    ))}
                  </div>

                  <Show when={license()!.usage?.achievements?.length}>
                    <div class="rounded-3xl border border-white/[0.05] bg-white/[0.01] p-6">
                      <div class="mb-4 flex items-center justify-between">
                        <h4 class="text-sm font-bold text-white uppercase tracking-tight">Unlocked Trophies</h4>
                        <span class="text-xs text-slate-500">{license()!.usage!.achievements.length} / 12</span>
                      </div>
                      <div class="flex flex-wrap gap-3">
                        <For each={license()!.usage!.achievements}>{(id) => {
                          const ach = ACHIEVEMENTS[id];
                          return ach ? (
                            <div class="group relative">
                              <div class="flex h-12 w-12 cursor-help items-center justify-center rounded-2xl bg-white/[0.05] text-xl transition-all hover:scale-110 hover:bg-white/10">
                                {ach.emoji}
                              </div>
                              <div class="pointer-events-none absolute bottom-full left-1/2 z-20 mb-2 w-48 -translate-x-1/2 scale-95 rounded-xl border border-white/10 bg-[#151516] p-3 text-xs opacity-0 shadow-2xl backdrop-blur-xl transition-all group-hover:scale-100 group-hover:opacity-100">
                                <p class="font-bold text-white">{ach.name}</p>
                                <p class="mt-1 leading-relaxed text-slate-400">{ach.description}</p>
                              </div>
                            </div>
                          ) : null;
                        }}</For>
                      </div>
                    </div>
                  </Show>
                </div>

                <div class="space-y-6">
                  <div class="rounded-3xl border border-white/[0.05] bg-white/[0.02] p-6">
                    <div class="mb-4 flex items-center justify-between">
                      <h4 class="text-xs font-bold uppercase tracking-wider text-slate-500">License Key</h4>
                      <button
                        onClick={() => copyToClipboard(licenseKey())}
                        class="text-indigo-400 transition-colors hover:text-indigo-300"
                      >
                        {copied() ? 'Copied!' : 'Copy'}
                      </button>
                    </div>
                    <div class="group relative rounded-2xl border border-white/[0.05] bg-black/40 p-4">
                      <code class="block truncate font-mono text-sm text-indigo-300">
                        {licenseKey()}
                      </code>
                    </div>
                    <p class="mt-3 text-[10px] leading-relaxed text-slate-500">
                      Run <code class="text-indigo-400">omg license activate</code> with this key to link your CLI.
                    </p>
                  </div>

                  {/* Actions */}
                  <div class="space-y-3">
                    <button
                      onClick={openBillingPortal}
                      disabled={actionLoading()}
                      class="flex w-full items-center justify-center gap-3 rounded-2xl bg-white/[0.05] px-6 py-4 text-sm font-bold text-white transition-all hover:bg-white/10 disabled:opacity-50"
                    >
                      <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z" />
                      </svg>
                      Billing Portal
                    </button>

                    <div class="grid grid-cols-2 gap-3">
                      <button
                        onClick={refreshLicense}
                        disabled={actionLoading()}
                        class="flex items-center justify-center gap-2 rounded-2xl border border-white/[0.05] bg-white/[0.02] py-4 text-xs font-bold text-slate-300 transition-all hover:bg-white/[0.05]"
                      >
                        <RefreshCw size={14} class={actionLoading() ? 'animate-spin' : ''} />
                        Refresh
                      </button>
                      <button
                        onClick={regenerateLicense}
                        disabled={actionLoading()}
                        class="flex items-center justify-center gap-2 rounded-2xl border border-rose-500/20 bg-rose-500/5 py-4 text-xs font-bold text-rose-400 transition-all hover:bg-rose-500/10"
                      >
                        <Zap size={14} />
                        Regen Key
                      </button>
                    </div>

                    <button
                      onClick={logout}
                      class="flex w-full items-center justify-center gap-2 py-4 text-xs font-bold text-slate-500 transition-colors hover:text-slate-300"
                    >
                      Logout Session
                    </button>
                  </div>

                  <div class="rounded-3xl bg-indigo-500/5 p-6 border border-indigo-500/10">
                    <h5 class="flex items-center gap-2 text-xs font-bold text-indigo-400 uppercase">
                      <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                      </svg>
                      Pro Tip
                    </h5>
                    <p class="mt-2 text-xs leading-relaxed text-indigo-300/60">
                      Use <code class="text-indigo-300">omg dash</code> in your terminal for a real-time TUI dashboard synced with this account.
                    </p>
                  </div>
                </div>
              </div>
            </Show>
          </div>
          
          <Show when={actionMessage()}>
            <div class="animate-slide-up border-t border-white/[0.05] bg-white/[0.02] px-8 py-3 text-center">
              <p class="text-sm font-medium text-indigo-400">{actionMessage()}</p>
            </div>
          </Show>
        </div>
      </div>
    </Show>
  );
};


export default Dashboard;
