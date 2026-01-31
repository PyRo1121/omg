import { Component, createSignal, onMount, Show, For } from "solid-js";
import { Title } from "@solidjs/meta";
import { useSession, signOut } from "~/lib/auth-client";
import { A } from "@solidjs/router";
import { Users, Activity, Database, TrendingUp, LogOut, LayoutDashboard, Package } from "lucide-solid";

type TabType = 'overview' | 'users' | 'analytics';

interface Analytics {
  overview: {
    totalUsers: number;
    totalLicenses: number;
    totalMachines: number;
    activeMachines: number;
    newUsersLast7Days: number;
    newUsersLast30Days: number;
  };
  licenses: {
    byTier: Array<{ tier: string; count: number }>;
    byStatus: Array<{ status: string; count: number }>;
  };
  usage: {
    last30Days: {
      totalCommands: number;
      totalPackagesInstalled: number;
      totalPackagesSearched: number;
      totalRuntimesSwitched: number;
      totalTimeSavedMs: number;
      totalTimeSavedHours: number;
    };
    dailyTrend: Array<{
      date: string;
      commands: number;
      packages: number;
    }>;
  };
}

interface User {
  id: string;
  name: string;
  email: string;
  role: string;
  emailVerified: boolean;
  createdAt: number;
  license: {
    id: string;
    key: string;
    tier: string;
    status: string;
    maxMachines: number;
    expiresAt: string | null;
  } | null;
  machines: number;
  totalCommands: number;
  totalPackages: number;
}

interface UsersResponse {
  users: User[];
  pagination: {
    page: number;
    limit: number;
    total: number;
    pages: number;
  };
}

const AdminDashboard: Component = () => {
  const [activeTab, setActiveTab] = createSignal<TabType>('overview');
  const [analytics, setAnalytics] = createSignal<Analytics | null>(null);
  const [users, setUsers] = createSignal<UsersResponse | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal('');
  const [page, setPage] = createSignal(1);
  
  const session = useSession();

  onMount(async () => {
    await Promise.all([loadAnalytics(), loadUsers()]);
  });

  const loadAnalytics = async () => {
    try {
      setLoading(true);
      const response = await fetch('/api/admin/analytics');
      const result = await response.json();

      if (response.ok) {
        setAnalytics(result);
      } else {
        setError(result.error || 'Failed to load analytics');
      }
    } catch (e) {
      console.error('Analytics error:', e);
      setError('Failed to load analytics');
    } finally {
      setLoading(false);
    }
  };

  const loadUsers = async (newPage = 1) => {
    try {
      const response = await fetch(`/api/admin/users?page=${newPage}&limit=20`);
      const result = await response.json();

      if (response.ok) {
        setUsers(result);
        setPage(newPage);
      } else {
        setError(result.error || 'Failed to load users');
      }
    } catch (e) {
      console.error('Users error:', e);
      setError('Failed to load users');
    }
  };

  const handleSignOut = async () => {
    try {
      await signOut();
      window.location.href = '/';
    } catch (err) {
      console.error('Sign out error:', err);
    }
  };

  const formatNumber = (num: number) => {
    return new Intl.NumberFormat().format(num);
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp).toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  };

  return (
    <>
      <Title>Admin Dashboard - OMG</Title>
      <div class="min-h-screen bg-gradient-to-br from-slate-950 via-indigo-950 to-slate-950">
        <nav class="border-b border-white/10 backdrop-blur-xl bg-slate-900/50">
          <div class="mx-auto px-6 py-4">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-8">
                <A href="/" class="text-2xl font-bold gradient-text">OMG Admin</A>
                <A href="/dashboard" class="text-slate-400 hover:text-white transition-colors flex items-center gap-2">
                  <LayoutDashboard class="h-4 w-4" />
                  User Dashboard
                </A>
              </div>
              <div class="flex items-center gap-4">
                <Show when={session()?.user}>
                  <span class="text-slate-300">{session()?.user?.email}</span>
                  <button
                    onClick={handleSignOut}
                    class="flex items-center gap-2 px-4 py-2 rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 transition-colors"
                  >
                    <LogOut class="h-4 w-4" />
                    Sign Out
                  </button>
                </Show>
              </div>
            </div>
          </div>
        </nav>

        <div class="mx-auto px-6 py-8 max-w-7xl">
          <Show when={error()}>
            <div class="mb-6 p-4 rounded-lg bg-red-600/20 border border-red-500/30 text-red-400">
              {error()}
            </div>
          </Show>

          <div class="mb-8 flex gap-4 border-b border-white/10">
            <button
              onClick={() => setActiveTab('overview')}
              class={`px-6 py-3 font-medium transition-all ${
                activeTab() === 'overview'
                  ? 'text-indigo-400 border-b-2 border-indigo-400'
                  : 'text-slate-400 hover:text-white'
              }`}
            >
              <div class="flex items-center gap-2">
                <TrendingUp class="h-4 w-4" />
                Overview
              </div>
            </button>
            <button
              onClick={() => setActiveTab('users')}
              class={`px-6 py-3 font-medium transition-all ${
                activeTab() === 'users'
                  ? 'text-indigo-400 border-b-2 border-indigo-400'
                  : 'text-slate-400 hover:text-white'
              }`}
            >
              <div class="flex items-center gap-2">
                <Users class="h-4 w-4" />
                Users
              </div>
            </button>
            <button
              onClick={() => setActiveTab('analytics')}
              class={`px-6 py-3 font-medium transition-all ${
                activeTab() === 'analytics'
                  ? 'text-indigo-400 border-b-2 border-indigo-400'
                  : 'text-slate-400 hover:text-white'
              }`}
            >
              <div class="flex items-center gap-2">
                <Activity class="h-4 w-4" />
                Analytics
              </div>
            </button>
          </div>

          <Show when={activeTab() === 'overview' && analytics()}>
            <div class="space-y-6">
              <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                <div class="rounded-lg bg-gradient-to-br from-indigo-500/10 to-purple-500/10 border border-indigo-500/20 p-6">
                  <div class="flex items-center justify-between mb-2">
                    <Users class="h-8 w-8 text-indigo-400" />
                    <span class="text-xs text-slate-400">+{analytics()!.overview.newUsersLast7Days} this week</span>
                  </div>
                  <div class="text-3xl font-bold text-white mb-1">{formatNumber(analytics()!.overview.totalUsers)}</div>
                  <div class="text-sm text-slate-400">Total Users</div>
                </div>

                <div class="rounded-lg bg-gradient-to-br from-emerald-500/10 to-teal-500/10 border border-emerald-500/20 p-6">
                  <div class="flex items-center justify-between mb-2">
                    <Database class="h-8 w-8 text-emerald-400" />
                    <span class="text-xs text-slate-400">Active</span>
                  </div>
                  <div class="text-3xl font-bold text-white mb-1">{formatNumber(analytics()!.overview.totalLicenses)}</div>
                  <div class="text-sm text-slate-400">Licenses</div>
                </div>

                <div class="rounded-lg bg-gradient-to-br from-blue-500/10 to-cyan-500/10 border border-blue-500/20 p-6">
                  <div class="flex items-center justify-between mb-2">
                    <Package class="h-8 w-8 text-blue-400" />
                    <span class="text-xs text-slate-400">{analytics()!.overview.activeMachines} active</span>
                  </div>
                  <div class="text-3xl font-bold text-white mb-1">{formatNumber(analytics()!.overview.totalMachines)}</div>
                  <div class="text-sm text-slate-400">Machines</div>
                </div>

                <div class="rounded-lg bg-gradient-to-br from-amber-500/10 to-orange-500/10 border border-amber-500/20 p-6">
                  <div class="flex items-center justify-between mb-2">
                    <Activity class="h-8 w-8 text-amber-400" />
                    <span class="text-xs text-slate-400">Last 30d</span>
                  </div>
                  <div class="text-3xl font-bold text-white mb-1">{formatNumber(analytics()!.usage.last30Days.totalCommands)}</div>
                  <div class="text-sm text-slate-400">Commands Run</div>
                </div>
              </div>

              <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div class="rounded-lg bg-slate-900/50 border border-white/10 p-6">
                  <h3 class="text-lg font-semibold text-white mb-4">Licenses by Tier</h3>
                  <div class="space-y-3">
                    <For each={analytics()!.licenses.byTier}>
                      {(tier) => (
                        <div class="flex items-center justify-between">
                          <span class="text-slate-300 capitalize">{tier.tier}</span>
                          <span class="text-white font-semibold">{formatNumber(tier.count)}</span>
                        </div>
                      )}
                    </For>
                  </div>
                </div>

                <div class="rounded-lg bg-slate-900/50 border border-white/10 p-6">
                  <h3 class="text-lg font-semibold text-white mb-4">Usage (Last 30 Days)</h3>
                  <div class="space-y-3">
                    <div class="flex items-center justify-between">
                      <span class="text-slate-300">Packages Installed</span>
                      <span class="text-white font-semibold">{formatNumber(analytics()!.usage.last30Days.totalPackagesInstalled)}</span>
                    </div>
                    <div class="flex items-center justify-between">
                      <span class="text-slate-300">Packages Searched</span>
                      <span class="text-white font-semibold">{formatNumber(analytics()!.usage.last30Days.totalPackagesSearched)}</span>
                    </div>
                    <div class="flex items-center justify-between">
                      <span class="text-slate-300">Runtimes Switched</span>
                      <span class="text-white font-semibold">{formatNumber(analytics()!.usage.last30Days.totalRuntimesSwitched)}</span>
                    </div>
                    <div class="flex items-center justify-between">
                      <span class="text-slate-300">Time Saved</span>
                      <span class="text-white font-semibold">{analytics()!.usage.last30Days.totalTimeSavedHours}h</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </Show>

          <Show when={activeTab() === 'users' && users()}>
            <div class="rounded-lg bg-slate-900/50 border border-white/10 overflow-hidden">
              <div class="overflow-x-auto">
                <table class="w-full">
                  <thead class="bg-slate-800/50 border-b border-white/10">
                    <tr>
                      <th class="text-left p-4 text-sm font-semibold text-slate-300">User</th>
                      <th class="text-left p-4 text-sm font-semibold text-slate-300">License Tier</th>
                      <th class="text-left p-4 text-sm font-semibold text-slate-300">Status</th>
                      <th class="text-left p-4 text-sm font-semibold text-slate-300">Machines</th>
                      <th class="text-left p-4 text-sm font-semibold text-slate-300">Commands</th>
                      <th class="text-left p-4 text-sm font-semibold text-slate-300">Joined</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={users()!.users}>
                      {(user) => (
                        <tr class="border-b border-white/5 hover:bg-white/5 transition-colors">
                          <td class="p-4">
                            <div>
                              <div class="text-white font-medium">{user.name}</div>
                              <div class="text-sm text-slate-400">{user.email}</div>
                            </div>
                          </td>
                          <td class="p-4">
                            <Show when={user.license} fallback={<span class="text-slate-500">No license</span>}>
                              <span class="uppercase text-sm font-semibold text-indigo-400">
                                {user.license!.tier}
                              </span>
                            </Show>
                          </td>
                          <td class="p-4">
                            <Show when={user.license} fallback={<span class="text-slate-500">-</span>}>
                              <span class={`text-sm font-medium ${
                                user.license!.status === 'active' ? 'text-emerald-400' :
                                user.license!.status === 'suspended' ? 'text-amber-400' :
                                'text-red-400'
                              }`}>
                                {user.license!.status}
                              </span>
                            </Show>
                          </td>
                          <td class="p-4 text-white">{user.machines}</td>
                          <td class="p-4 text-white">{formatNumber(user.totalCommands)}</td>
                          <td class="p-4 text-slate-400 text-sm">{formatDate(user.createdAt)}</td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>

              <Show when={users()!.pagination.pages > 1}>
                <div class="p-4 border-t border-white/10 flex items-center justify-between">
                  <button
                    onClick={() => loadUsers(page() - 1)}
                    disabled={page() === 1}
                    class="px-4 py-2 rounded-lg bg-indigo-600/20 text-indigo-400 hover:bg-indigo-600/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    Previous
                  </button>
                  <span class="text-slate-400">
                    Page {page()} of {users()!.pagination.pages}
                  </span>
                  <button
                    onClick={() => loadUsers(page() + 1)}
                    disabled={page() === users()!.pagination.pages}
                    class="px-4 py-2 rounded-lg bg-indigo-600/20 text-indigo-400 hover:bg-indigo-600/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    Next
                  </button>
                </div>
              </Show>
            </div>
          </Show>

          <Show when={activeTab() === 'analytics' && analytics()}>
            <div class="space-y-6">
              <div class="rounded-lg bg-slate-900/50 border border-white/10 p-6">
                <h3 class="text-lg font-semibold text-white mb-4">Daily Command Trend (Last 30 Days)</h3>
                <div class="h-64 flex items-end gap-1">
                  <For each={analytics()!.usage.dailyTrend}>
                    {(day) => {
                      const maxCommands = Math.max(...analytics()!.usage.dailyTrend.map(d => d.commands));
                      const height = (day.commands / maxCommands) * 100;
                      return (
                        <div
                          class="flex-1 bg-gradient-to-t from-indigo-600 to-indigo-400 rounded-t hover:from-indigo-500 hover:to-indigo-300 transition-all cursor-pointer"
                          style={{ height: `${height}%` }}
                          title={`${day.date}: ${formatNumber(day.commands)} commands`}
                        />
                      );
                    }}
                  </For>
                </div>
              </div>
            </div>
          </Show>
        </div>
      </div>
    </>
  );
};

export default AdminDashboard;
