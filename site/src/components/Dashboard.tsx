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
        return 'from-indigo-500 to-blue-500';
      case 'team':
        return 'from-purple-500 to-pink-500';
      case 'enterprise':
        return 'from-amber-500 to-orange-500';
      default:
        return 'from-slate-500 to-slate-600';
    }
  };

  const formatDate = (dateStr: string) => {
    if (!dateStr) return 'Never';
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' });
  };

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 z-[100] flex items-center justify-center p-4"
        onClick={e => e.target === e.currentTarget && props.onClose()}
      >
        <div class="animate-fade-in absolute inset-0 bg-black/80 backdrop-blur-md" />

        <div class="animate-scale-in relative max-h-[90vh] w-full max-w-2xl overflow-hidden overflow-y-auto rounded-3xl border border-slate-700/50 bg-gradient-to-b from-slate-800 to-slate-900 shadow-2xl shadow-black/50">
          {/* Glow effects */}
          <div class="pointer-events-none absolute -top-20 -right-20 h-40 w-40 rounded-full bg-indigo-500/20 blur-3xl" />
          <div class="pointer-events-none absolute -bottom-20 -left-20 h-40 w-40 rounded-full bg-purple-500/20 blur-3xl" />

          <div class="relative p-8">
            {/* Header */}
            <div class="mb-8 flex items-center justify-between">
              <div class="flex items-center gap-3">
                <div class="flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-cyan-400">
                  <svg
                    class="h-6 w-6 text-white"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M5.121 17.804A13.937 13.937 0 0112 16c2.5 0 4.847.655 6.879 1.804M15 10a3 3 0 11-6 0 3 3 0 016 0zm6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                    />
                  </svg>
                </div>
                <div>
                  <h2 class="text-2xl font-bold text-white">License Dashboard</h2>
                  <p class="text-sm text-slate-400">Manage your OMG subscription</p>
                </div>
              </div>
              <button
                onClick={props.onClose}
                class="rounded-lg p-2 text-slate-400 transition-all hover:bg-slate-700/50 hover:text-white"
              >
                <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>

            <Show when={view() === 'login'}>
              <div class="space-y-6">
                <div class="rounded-2xl border border-slate-700/50 bg-slate-800/50 p-6">
                  <div class="mb-5 flex items-start gap-3">
                    <div class="rounded-lg bg-indigo-500/20 p-2">
                      <svg
                        class="h-5 w-5 text-indigo-400"
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
                    <div>
                      <p class="font-medium text-white">Access Your Dashboard</p>
                      <p class="text-sm text-slate-400">
                        Enter the email associated with your license
                      </p>
                    </div>
                  </div>

                  <input
                    type="email"
                    value={email()}
                    onInput={e => setEmail(e.currentTarget.value)}
                    onKeyPress={e => e.key === 'Enter' && fetchLicense()}
                    placeholder="you@company.com"
                    class="w-full rounded-xl border border-slate-600 bg-slate-900/50 px-4 py-3 text-white placeholder-slate-500 transition-all focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
                  />

                  <Show when={error()}>
                    <div class="mt-4 flex items-start gap-3 rounded-xl border border-red-500/30 bg-red-500/10 p-4">
                      <svg
                        class="mt-0.5 h-5 w-5 flex-shrink-0 text-red-400"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                        />
                      </svg>
                      <p class="text-sm text-red-400">{error()}</p>
                    </div>
                  </Show>

                  <button
                    onClick={fetchLicense}
                    disabled={loading() || !email()}
                    class="mt-5 flex w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-indigo-500 to-blue-500 py-4 font-semibold text-white transition-all hover:from-indigo-400 hover:to-blue-400 disabled:from-slate-600 disabled:to-slate-600"
                  >
                    {loading() ? (
                      <>
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
                            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                          />
                        </svg>
                        Loading...
                      </>
                    ) : (
                      <>
                        <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M11 16l-4-4m0 0l4-4m-4 4h14m-5 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h7a3 3 0 013 3v1"
                          />
                        </svg>
                        Access Dashboard
                      </>
                    )}
                  </button>
                </div>

                <div class="space-y-3 text-center">
                  <p class="text-sm text-slate-500">
                    Don't have an account?{' '}
                    <button
                      onClick={() => setView('register')}
                      class="text-indigo-400 hover:underline"
                    >
                      Create free account
                    </button>
                  </p>
                  <p class="text-xs text-slate-600">
                    Or{' '}
                    <a
                      href="#pricing"
                      onClick={props.onClose}
                      class="text-indigo-400 hover:underline"
                    >
                      upgrade to Pro
                    </a>{' '}
                    for security features
                  </p>
                </div>
              </div>
            </Show>

            <Show when={view() === 'register'}>
              <div class="space-y-6">
                <div class="rounded-2xl border border-green-500/30 bg-gradient-to-br from-green-500/10 to-emerald-500/10 p-6">
                  <div class="mb-5 flex items-start gap-3">
                    <div class="rounded-lg bg-green-500/20 p-2">
                      <svg
                        class="h-5 w-5 text-green-400"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M18 9v3m0 0v3m0-3h3m-3 0h-3m-2-5a4 4 0 11-8 0 4 4 0 018 0zM3 20a6 6 0 0112 0v1H3v-1z"
                        />
                      </svg>
                    </div>
                    <div>
                      <p class="font-medium text-white">Create Free Account</p>
                      <p class="text-sm text-slate-400">
                        Track your usage, time saved, and most-used commands
                      </p>
                    </div>
                  </div>

                  <input
                    type="email"
                    value={email()}
                    onInput={e => setEmail(e.currentTarget.value)}
                    onKeyPress={e => e.key === 'Enter' && registerFreeAccount()}
                    placeholder="you@email.com"
                    class="w-full rounded-xl border border-slate-600 bg-slate-900/50 px-4 py-3 text-white placeholder-slate-500 transition-all focus:border-green-500 focus:ring-2 focus:ring-green-500/20 focus:outline-none"
                  />

                  <Show when={error()}>
                    <div class="mt-4 flex items-start gap-3 rounded-xl border border-red-500/30 bg-red-500/10 p-4">
                      <svg
                        class="mt-0.5 h-5 w-5 flex-shrink-0 text-red-400"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                        />
                      </svg>
                      <p class="text-sm text-red-400">{error()}</p>
                    </div>
                  </Show>

                  <button
                    onClick={registerFreeAccount}
                    disabled={loading() || !email()}
                    class="mt-5 flex w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-green-500 to-emerald-500 py-4 font-semibold text-white transition-all hover:from-green-400 hover:to-emerald-400 disabled:from-slate-600 disabled:to-slate-600"
                  >
                    {loading() ? (
                      <>
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
                            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                          />
                        </svg>
                        Creating account...
                      </>
                    ) : (
                      <>
                        <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M5 13l4 4L19 7"
                          />
                        </svg>
                        Create Free Account
                      </>
                    )}
                  </button>

                  <div class="mt-4 rounded-xl bg-slate-800/50 p-4">
                    <p class="mb-2 text-sm font-medium text-slate-300">Free tier includes:</p>
                    <ul class="space-y-1 text-xs text-slate-400">
                      <li class="flex items-center gap-2">
                        <span class="text-green-400">✓</span> Usage tracking & analytics
                      </li>
                      <li class="flex items-center gap-2">
                        <span class="text-green-400">✓</span> Time saved calculations
                      </li>
                      <li class="flex items-center gap-2">
                        <span class="text-green-400">✓</span> Command history & stats
                      </li>
                      <li class="flex items-center gap-2">
                        <span class="text-green-400">✓</span> Package management
                      </li>
                      <li class="flex items-center gap-2">
                        <span class="text-green-400">✓</span> 100+ runtimes via mise
                      </li>
                    </ul>
                  </div>
                </div>

                <p class="text-center text-sm text-slate-500">
                  Already have an account?{' '}
                  <button onClick={() => setView('login')} class="text-indigo-400 hover:underline">
                    Sign in
                  </button>
                </p>
              </div>
            </Show>

            <Show when={view() === 'dashboard' && license()}>
              <div class="space-y-6">
                {/* License Status Card */}
                <div
                  class={`bg-gradient-to-br ${getTierColor(license()!.tier)} rounded-2xl p-[1px]`}
                >
                  <div class="rounded-2xl bg-slate-900 p-6">
                    <div class="mb-4 flex items-center justify-between">
                      <div class="flex items-center gap-3">
                        <div
                          class={`bg-gradient-to-r px-3 py-1 ${getTierColor(license()!.tier)} rounded-full`}
                        >
                          <span class="text-sm font-bold text-white uppercase">
                            {license()!.tier}
                          </span>
                        </div>
                        <span
                          class={`rounded-full px-2 py-1 text-xs font-medium ${
                            license()!.status === 'active'
                              ? 'bg-green-500/20 text-green-400'
                              : 'bg-red-500/20 text-red-400'
                          }`}
                        >
                          {license()!.status}
                        </span>
                      </div>
                      <button
                        onClick={logout}
                        class="flex items-center gap-1 text-sm text-slate-400 hover:text-white"
                      >
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"
                          />
                        </svg>
                        Logout
                      </button>
                    </div>

                    <div class="grid grid-cols-2 gap-4 text-sm">
                      <div>
                        <p class="text-slate-400">Email</p>
                        <p class="font-medium text-white">{email()}</p>
                      </div>
                      <div>
                        <p class="text-slate-400">Expires</p>
                        <p class="font-medium text-white">{formatDate(license()!.expires_at)}</p>
                      </div>
                      <Show when={license()!.max_seats}>
                        <div>
                          <p class="text-slate-400">Seats Used</p>
                          <p class="font-medium text-white">
                            {license()!.used_seats || 0} / {license()!.max_seats}
                          </p>
                        </div>
                      </Show>
                    </div>
                  </div>
                </div>

                {/* License Key */}
                <div class="rounded-2xl border border-slate-700/50 bg-slate-800/50 p-6">
                  <div class="mb-3 flex items-center justify-between">
                    <h3 class="flex items-center gap-2 font-semibold text-white">
                      <svg
                        class="h-5 w-5 text-indigo-400"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"
                        />
                      </svg>
                      License Key
                    </h3>
                    <button
                      onClick={() => copyToClipboard(licenseKey())}
                      class="flex items-center gap-1.5 rounded-lg bg-slate-700/50 px-3 py-1.5 text-sm text-slate-300 transition-all hover:bg-slate-600/50 hover:text-white"
                    >
                      {copied() ? (
                        <>
                          <svg
                            class="h-4 w-4 text-green-400"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              stroke-linecap="round"
                              stroke-linejoin="round"
                              stroke-width="2"
                              d="M5 13l4 4L19 7"
                            />
                          </svg>
                          Copied!
                        </>
                      ) : (
                        <>
                          <svg
                            class="h-4 w-4"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              stroke-linecap="round"
                              stroke-linejoin="round"
                              stroke-width="2"
                              d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                            />
                          </svg>
                          Copy
                        </>
                      )}
                    </button>
                  </div>
                  <code class="block rounded-lg bg-slate-900/50 p-3 font-mono text-sm break-all text-green-400">
                    {licenseKey()}
                  </code>
                  <p class="mt-2 text-xs text-slate-500">
                    Activate with:{' '}
                    <code class="text-slate-400">omg license activate {licenseKey()}</code>
                  </p>
                </div>

                {/* Usage Statistics - All tiers */}
                <div class="rounded-2xl border border-slate-700/50 bg-slate-800/50 p-6">
                  <h3 class="mb-4 flex items-center gap-2 font-semibold text-white">
                    <svg
                      class="h-5 w-5 text-cyan-400"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
                      />
                    </svg>
                    Usage Statistics
                  </h3>
                  <div class="mb-4 grid grid-cols-2 gap-3">
                    <div class="rounded-xl bg-slate-900/50 p-4 text-center">
                      <div class="text-2xl font-bold text-green-400">
                        {license()!.usage?.time_saved_ms
                          ? formatTimeSaved(license()!.usage!.time_saved_ms)
                          : '0ms'}
                      </div>
                      <div class="text-xs text-slate-500">Time Saved</div>
                    </div>
                    <div class="rounded-xl bg-slate-900/50 p-4 text-center">
                      <div class="text-2xl font-bold text-cyan-400">
                        {license()!.usage?.total_commands || 0}
                      </div>
                      <div class="text-xs text-slate-500">Commands</div>
                    </div>
                    <div class="rounded-xl bg-slate-900/50 p-4 text-center">
                      <div class="text-2xl font-bold text-indigo-400">
                        {license()!.usage?.queries_today || 0}
                      </div>
                      <div class="text-xs text-slate-500">Today</div>
                    </div>
                    <div class="flex flex-col items-center justify-center rounded-xl bg-slate-900/50 p-4 text-center">
                      <div class="flex items-center gap-1 text-2xl font-bold text-orange-400">
                        🔥 {license()!.usage?.current_streak || 0}
                      </div>
                      <div class="text-xs text-slate-500">Day Streak</div>
                    </div>
                  </div>

                  {/* Achievements */}
                  <Show
                    when={
                      license()!.usage?.achievements && license()!.usage!.achievements.length > 0
                    }
                  >
                    <div class="mt-4 border-t border-slate-700/50 pt-4">
                      <h4 class="mb-3 flex items-center gap-2 text-sm font-medium text-slate-300">
                        🏆 Achievements ({license()!.usage!.achievements.length}/12)
                      </h4>
                      <div class="flex flex-wrap gap-2">
                        <For each={license()!.usage!.achievements}>{(id: string) => {
                          const achievement = ACHIEVEMENTS[id];
                          return achievement ? (
                            <div
                              class="group relative cursor-help rounded-lg bg-slate-700/50 px-2 py-1 text-sm"
                              title={`${achievement.name}: ${achievement.description}`}
                            >
                              <span>{achievement.emoji}</span>
                            </div>
                          ) : null;
                        }}</For>
                      </div>
                    </div>
                  </Show>
                </div>

                {/* Security Stats - Pro/Team feature */}
                <Show
                  when={
                    license()!.tier.toLowerCase() !== 'free' &&
                    (license()!.usage?.sbom_generated || license()!.usage?.vulnerabilities_found)
                  }
                >
                  <div class="rounded-2xl border border-slate-700/50 bg-slate-800/50 p-6">
                    <h3 class="mb-4 flex items-center gap-2 font-semibold text-white">
                      🛡️ Security Stats
                    </h3>
                    <div class="grid grid-cols-2 gap-4">
                      <div class="rounded-xl bg-slate-900/50 p-4 text-center">
                        <div class="text-2xl font-bold text-green-400">
                          {license()!.usage?.sbom_generated || 0}
                        </div>
                        <div class="text-xs text-slate-500">SBOMs Generated</div>
                      </div>
                      <div class="rounded-xl bg-slate-900/50 p-4 text-center">
                        <div class="text-2xl font-bold text-amber-400">
                          {license()!.usage?.vulnerabilities_found || 0}
                        </div>
                        <div class="text-xs text-slate-500">CVEs Found</div>
                      </div>
                    </div>
                  </div>
                </Show>

                {/* Subscription Management */}
                <div class="rounded-2xl border border-slate-700/50 bg-slate-800/50 p-6">
                  <h3 class="mb-4 flex items-center gap-2 font-semibold text-white">
                    <svg
                      class="h-5 w-5 text-purple-400"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z"
                      />
                    </svg>
                    Subscription
                  </h3>
                  <p class="mb-4 text-sm text-slate-400">
                    Manage your subscription, update payment methods, upgrade, downgrade, or cancel.
                  </p>
                  <button
                    onClick={openBillingPortal}
                    disabled={actionLoading()}
                    class="flex w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-purple-500 to-pink-500 py-3 font-medium text-white transition-all hover:from-purple-400 hover:to-pink-400 disabled:opacity-50"
                  >
                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
                      />
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                      />
                    </svg>
                    Manage Subscription
                  </button>
                </div>

                {/* Actions */}
                <div class="grid grid-cols-2 gap-4">
                  <button
                    onClick={refreshLicense}
                    disabled={actionLoading()}
                    class="flex items-center justify-center gap-2 rounded-xl border border-slate-600 bg-slate-700/50 py-3 font-medium text-white transition-all hover:bg-slate-600/50 disabled:opacity-50"
                  >
                    <svg
                      class={`h-5 w-5 ${actionLoading() ? 'animate-spin' : ''}`}
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                      />
                    </svg>
                    Refresh Token
                  </button>

                  <button
                    onClick={regenerateLicense}
                    disabled={actionLoading()}
                    class="flex items-center justify-center gap-2 rounded-xl border border-amber-500/50 bg-amber-500/20 py-3 font-medium text-amber-400 transition-all hover:bg-amber-500/30 disabled:opacity-50"
                  >
                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                      />
                    </svg>
                    Regenerate Key
                  </button>
                </div>

                {/* Action Message */}
                <Show when={actionMessage()}>
                  <div
                    class={`rounded-xl p-4 text-sm ${
                      actionMessage().includes('success') || actionMessage().includes('generated')
                        ? 'border border-green-500/30 bg-green-500/10 text-green-400'
                        : 'border border-amber-500/30 bg-amber-500/10 text-amber-400'
                    }`}
                  >
                    {actionMessage()}
                  </div>
                </Show>

                {/* Help Section */}
                <div class="rounded-xl border border-slate-700/30 bg-slate-800/30 p-4">
                  <h4 class="mb-2 flex items-center gap-2 font-medium text-white">
                    <svg
                      class="h-4 w-4 text-slate-400"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                      />
                    </svg>
                    Need Help?
                  </h4>
                  <ul class="space-y-1 text-sm text-slate-400">
                    <li>
                      • <strong>Refresh Token</strong>: Get a new JWT without changing your license
                      key
                    </li>
                    <li>
                      • <strong>Regenerate Key</strong>: Create a new license key if yours was
                      leaked
                    </li>
                    <li>
                      • Contact{' '}
                      <a href="mailto:support@pyro1121.com" class="text-indigo-400 hover:underline">
                        support@pyro1121.com
                      </a>{' '}
                      for billing issues
                    </li>
                  </ul>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default Dashboard;
