import { Component, For, Show, createSignal } from 'solid-js';
import * as api from '../../lib/api';
import { MetricCard } from '../ui/Card';
import { TierBadge, StatusBadge } from '../ui/Badge';
import { BarChart, DonutChart, LiveIndicator } from '../ui/Chart';
import { Table } from '../ui/Table';
import {
  Users,
  Calendar,
  CalendarDays,
  RefreshCw,
  Zap,
  BarChart3,
  TrendingUp,
  AlertTriangle,
  Target,
  Globe,
  Activity,
  PieChart,
  DollarSign,
  FileText,
  Monitor,
  Key,
  Crown,
} from '../ui/Icons';

interface AdminDashboardProps {
  adminData: api.AdminOverview | null;
  adminUsers: api.AdminUser[];
  adminHealth: api.AdminHealth | null;
  adminRevenue: api.AdminRevenue | null;
  adminCohorts: api.AdminCohorts | null;
  adminActivity: api.AdminActivity[];
  adminAuditLog: api.AdminAuditLogResponse | null;
  adminAnalytics: api.AdminAnalytics | null;
  onRefresh: () => void;
  onUserClick: (userId: string) => void;
  onExport: (type: 'users' | 'usage' | 'audit') => void;
  onSearch: (query: string) => void;
  onPageChange: (page: number) => void;
  currentPage: number;
  totalPages: number;
  searchQuery: string;
}

export const AdminDashboard: Component<AdminDashboardProps> = props => {
  const [activeTab, setActiveTab] = createSignal<'overview' | 'users' | 'revenue' | 'activity' | 'analytics'>('overview');
  const [searchInput, setSearchInput] = createSignal(props.searchQuery);
  const [isRefreshing, setIsRefreshing] = createSignal(false);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await props.onRefresh();
    setTimeout(() => setIsRefreshing(false), 1000);
  };

  const handleSearch = (e: Event) => {
    e.preventDefault();
    props.onSearch(searchInput());
  };

  const tierDistribution = () => {
    if (!props.adminData?.tiers) return [];
    const tierColors: Record<string, string> = {
      free: '#64748b',
      pro: '#8b5cf6',
      team: '#06b6d4',
      enterprise: '#f59e0b',
    };
    return props.adminData.tiers.map(t => ({
      label: t.tier.charAt(0).toUpperCase() + t.tier.slice(1),
      value: t.count || 0,
      color: tierColors[t.tier] || '#64748b',
    }));
  };

  const revenueData = () => {
    const monthly = props.adminRevenue?.monthly_revenue || props.adminRevenue?.monthly || [];
    if (!monthly.length) return [];
    return monthly.slice(-12).map(m => {
      const item = m as { month?: string; revenue?: number; transactions?: number; new_subscriptions?: number };
      return {
        label: (item.month || '').slice(5) || '?',
        value: (item.revenue || 0) / 100,
        secondaryValue: item.transactions || item.new_subscriptions || 0,
      };
    });
  };

  const _dailyActiveData = () => {
    const dau = props.adminData?.daily_active_users || [];
    return dau.slice(-14).map(d => ({
      label: d.date?.slice(5) || '',
      value: d.active_users || 0,
    }));
  };

  const commandsSparkline = () => {
    const dau = props.adminData?.daily_active_users || [];
    return dau.slice(-7).map(d => d.commands || 0);
  };

  const getActivityIcon = (type: string) => {
    const icons: Record<string, { icon: string; bg: string }> = {
      signup: { icon: '👤', bg: 'bg-emerald-500/20 text-emerald-400' },
      upgrade: { icon: '⬆️', bg: 'bg-indigo-500/20 text-indigo-400' },
      command: { icon: '⚡', bg: 'bg-cyan-500/20 text-cyan-400' },
      login: { icon: '🔐', bg: 'bg-blue-500/20 text-blue-400' },
      install: { icon: '📦', bg: 'bg-purple-500/20 text-purple-400' },
      activation: { icon: '🔑', bg: 'bg-amber-500/20 text-amber-400' },
    };
    return icons[type] || { icon: '📋', bg: 'bg-slate-700' };
  };

  return (
    <div class="space-y-8">
      {/* Header with Live Status */}
      <div class="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
        <div class="flex items-start gap-4">
          <div class="flex h-14 w-14 items-center justify-center rounded-2xl bg-gradient-to-br from-amber-500 via-orange-500 to-red-500 shadow-lg shadow-orange-500/25">
            <Crown size={28} class="text-white" />
          </div>
          <div>
            <div class="flex items-center gap-3">
              <h1 class="text-3xl font-bold tracking-tight text-white">Admin Console</h1>
              <LiveIndicator label="Real-time" />
            </div>
            <p class="mt-1 text-slate-400">
              Monitor system health, manage users, and track revenue
            </p>
          </div>
        </div>
        
        <div class="flex flex-wrap items-center gap-3">
          <button
            onClick={handleRefresh}
            disabled={isRefreshing()}
            class="group flex items-center gap-2 rounded-xl border border-slate-700/50 bg-slate-800/80 px-4 py-2.5 text-sm font-medium text-white backdrop-blur-sm transition-all hover:border-slate-600 hover:bg-slate-700/80 disabled:opacity-50"
          >
            <svg class={`h-4 w-4 transition-transform ${isRefreshing() ? 'animate-spin' : 'group-hover:rotate-180'}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            {isRefreshing() ? 'Refreshing...' : 'Refresh'}
          </button>
          
          <div class="relative group">
            <button class="flex items-center gap-2 rounded-xl bg-gradient-to-r from-indigo-600 to-indigo-500 px-5 py-2.5 text-sm font-medium text-white shadow-lg shadow-indigo-500/25 transition-all hover:from-indigo-500 hover:to-indigo-400 hover:shadow-indigo-500/40">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
              Export Data
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
              </svg>
            </button>
            <div class="absolute right-0 top-full z-50 mt-2 hidden w-48 rounded-xl border border-slate-700/50 bg-slate-800/95 p-2 shadow-xl backdrop-blur-sm group-hover:block">
              <button onClick={() => props.onExport('users')} class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-300 hover:bg-slate-700/50 hover:text-white">
                <Users size={16} /> Export Users
              </button>
              <button onClick={() => props.onExport('usage')} class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-300 hover:bg-slate-700/50 hover:text-white">
                <BarChart3 size={16} /> Export Usage
              </button>
              <button onClick={() => props.onExport('audit')} class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-300 hover:bg-slate-700/50 hover:text-white">
                <FileText size={16} /> Export Audit Log
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Navigation Tabs */}
      <div class="flex items-center gap-2 rounded-2xl border border-slate-800/60 bg-slate-900/50 p-1.5 backdrop-blur-sm">
        <For each={[
          { id: 'overview' as const, label: 'Overview', Icon: BarChart3 },
          { id: 'users' as const, label: 'Users', Icon: Users },
          { id: 'revenue' as const, label: 'Revenue', Icon: DollarSign },
          { id: 'activity' as const, label: 'Activity', Icon: Activity },
          { id: 'analytics' as const, label: 'Analytics', Icon: TrendingUp },
        ]}>{tab => (
          <button
            onClick={() => setActiveTab(tab.id)}
            class={`group relative flex items-center gap-2.5 rounded-xl px-5 py-3 text-sm font-medium transition-all duration-200 ${
              activeTab() === tab.id
                ? 'bg-gradient-to-r from-slate-700/80 to-slate-700/60 text-white shadow-lg'
                : 'text-slate-400 hover:bg-slate-800/50 hover:text-white'
            }`}
          >
            <tab.Icon size={18} />
            <span>{tab.label}</span>
            {activeTab() === tab.id && (
              <div class="absolute -bottom-1.5 left-1/2 h-0.5 w-8 -translate-x-1/2 rounded-full bg-gradient-to-r from-indigo-500 to-purple-500" />
            )}
          </button>
        )}</For>
      </div>

      {/* Overview Tab */}
      <Show when={activeTab() === 'overview'}>
        <div class="space-y-8">
          {/* Real-time Health Banner */}
          <Show when={props.adminHealth}>
            <div class="relative overflow-hidden rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900 via-slate-900 to-slate-800/50 p-6">
              <div class="absolute -right-20 -top-20 h-64 w-64 rounded-full bg-gradient-to-br from-emerald-500/10 to-transparent blur-3xl" />
              <div class="absolute -bottom-20 -left-20 h-64 w-64 rounded-full bg-gradient-to-br from-indigo-500/10 to-transparent blur-3xl" />
              
              <div class="relative">
                <div class="mb-6 flex items-center justify-between">
                  <div class="flex items-center gap-3">
                    <div class="relative">
                      <div class="h-3 w-3 rounded-full bg-emerald-400" />
                      <div class="absolute inset-0 h-3 w-3 animate-ping rounded-full bg-emerald-400 opacity-75" />
                    </div>
                    <h2 class="text-lg font-semibold text-white">Live System Status</h2>
                  </div>
                  <span class="text-xs text-slate-500">Updated just now</span>
                </div>
                
                <div class="grid grid-cols-2 gap-4 md:grid-cols-5">
                  <div class="group rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4 transition-all hover:border-emerald-500/40 hover:bg-emerald-500/10">
                    <div class="flex items-center gap-2">
                      <span class="text-2xl">🟢</span>
                      <div>
                        <div class="text-2xl font-bold text-white">{props.adminHealth!.active_users_today}</div>
                        <div class="text-xs text-emerald-400">Active Today</div>
                      </div>
                    </div>
                  </div>
                  
                  <div class="rounded-xl border border-cyan-500/20 bg-cyan-500/5 p-4 transition-all hover:border-cyan-500/40 hover:bg-cyan-500/10">
                    <div class="text-2xl font-bold text-white">{props.adminHealth!.active_users_week}</div>
                    <div class="text-xs text-cyan-400">Active This Week</div>
                  </div>
                  
                  <div class="rounded-xl border border-indigo-500/20 bg-indigo-500/5 p-4 transition-all hover:border-indigo-500/40 hover:bg-indigo-500/10">
                    <div class="text-2xl font-bold text-white">{(props.adminHealth!.commands_today || 0).toLocaleString()}</div>
                    <div class="text-xs text-indigo-400">Commands Today</div>
                  </div>
                  
                  <div class="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 transition-all hover:border-amber-500/40 hover:bg-amber-500/10">
                    <div class="text-2xl font-bold text-white">{props.adminHealth!.new_users_today}</div>
                    <div class="text-xs text-amber-400">New Signups</div>
                  </div>
                  
                  <div class="rounded-xl border border-purple-500/20 bg-purple-500/5 p-4 transition-all hover:border-purple-500/40 hover:bg-purple-500/10">
                    <div class="text-2xl font-bold text-white">{props.adminHealth!.installs_today}</div>
                    <div class="text-xs text-purple-400">Installs Today</div>
                  </div>
                </div>
              </div>
            </div>
          </Show>

          {/* Key Metrics Grid */}
          <Show when={props.adminData}>
            <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-4">
              <MetricCard
                title="Total Users"
                value={(props.adminData!.overview?.total_users || 0).toLocaleString()}
                icon={<Users size={20} class="text-indigo-400" />}
                iconBg="bg-indigo-500/20"
                sparklineData={commandsSparkline()}
                sparklineColor="#6366f1"
                subtitle="All registered accounts"
              />
              <MetricCard
                title="Active Licenses"
                value={(props.adminData!.overview?.active_licenses || 0).toLocaleString()}
                icon={<Key size={20} class="text-emerald-400" />}
                iconBg="bg-emerald-500/20"
                subtitle="Currently active"
                badge={{ text: 'Healthy', color: 'emerald' }}
              />
              <MetricCard
                title="Active Machines"
                value={(props.adminData!.overview?.active_machines || 0).toLocaleString()}
                icon={<Monitor size={20} class="text-cyan-400" />}
                iconBg="bg-cyan-500/20"
                subtitle="Connected devices"
              />
              <MetricCard
                title="Total Commands"
                value={(props.adminData!.usage?.total_commands || 0).toLocaleString()}
                icon={<Zap size={20} class="text-purple-400" />}
                iconBg="bg-purple-500/20"
                sparklineData={commandsSparkline()}
                sparklineColor="#a855f7"
                subtitle="Last 30 days"
              />
            </div>
          </Show>

          {/* Charts Section */}
          <div class="grid grid-cols-1 gap-6 xl:grid-cols-3">
            {/* User Distribution */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <div class="mb-6 flex items-center justify-between">
                <h3 class="text-lg font-semibold text-white">User Distribution</h3>
                <span class="rounded-full bg-slate-800 px-3 py-1 text-xs text-slate-400">By Tier</span>
              </div>
              <div class="flex justify-center">
                <DonutChart 
                  data={tierDistribution()} 
                  size={180} 
                  thickness={32}
                  centerLabel="Users"
                  showLegend
                />
              </div>
            </div>

            {/* Revenue Chart */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm xl:col-span-2">
              <div class="mb-6 flex items-center justify-between">
                <div>
                  <h3 class="text-lg font-semibold text-white">Revenue Trend</h3>
                  <p class="text-sm text-slate-500">Monthly recurring revenue</p>
                </div>
                <Show when={props.adminRevenue}>
                  <div class="text-right">
                    <div class="text-3xl font-bold text-emerald-400">
                      ${(props.adminRevenue!.mrr || 0).toLocaleString()}
                    </div>
                    <div class="text-xs text-slate-500">Current MRR</div>
                  </div>
                </Show>
              </div>
              <BarChart
                data={revenueData()}
                height={220}
                showLabels
                gradient="emerald"
                animated
                tooltipFormatter={(v) => `$${v.toLocaleString()}`}
              />
            </div>
          </div>

          {/* Activity Feed */}
          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <div class="mb-6 flex items-center justify-between">
              <div class="flex items-center gap-3">
                <h3 class="text-lg font-semibold text-white">Recent Activity</h3>
                <span class="rounded-full bg-indigo-500/20 px-2 py-0.5 text-xs font-medium text-indigo-400">
                  {props.adminActivity.length} events
                </span>
              </div>
              <button
                onClick={() => setActiveTab('activity')}
                class="flex items-center gap-1 text-sm text-indigo-400 transition-colors hover:text-indigo-300"
              >
                View all
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                </svg>
              </button>
            </div>
            
            <div class="grid gap-3 md:grid-cols-2">
              <For each={props.adminActivity.slice(0, 6)}>
                {activity => {
                  const { icon, bg } = getActivityIcon(activity.type);
                  return (
                    <div class="group flex items-center gap-4 rounded-xl border border-slate-800/40 bg-slate-800/20 p-4 transition-all hover:border-slate-700/60 hover:bg-slate-800/40">
                      <div class={`flex h-11 w-11 shrink-0 items-center justify-center rounded-xl ${bg}`}>
                        <span class="text-lg">{icon}</span>
                      </div>
                      <div class="min-w-0 flex-1">
                        <p class="truncate text-sm font-medium text-white">{activity.description}</p>
                        <p class="text-xs text-slate-500">{api.formatRelativeTime(activity.timestamp)}</p>
                      </div>
                      <Show when={activity.user_id}>
                        <button
                          onClick={() => props.onUserClick(activity.user_id)}
                          class="shrink-0 rounded-lg bg-slate-700/50 px-3 py-1.5 text-xs font-medium text-slate-300 opacity-0 transition-all hover:bg-slate-700 hover:text-white group-hover:opacity-100"
                        >
                          View
                        </button>
                      </Show>
                    </div>
                  );
                }}
              </For>
            </div>
          </div>
        </div>
      </Show>

      {/* Users Tab */}
      <Show when={activeTab() === 'users'}>
        <div class="space-y-6">
          {/* Search and Filters */}
          <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <form onSubmit={handleSearch} class="relative flex-1 sm:max-w-md">
              <svg class="absolute left-4 top-1/2 h-5 w-5 -translate-y-1/2 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
              </svg>
              <input
                type="text"
                value={searchInput()}
                onInput={e => setSearchInput(e.currentTarget.value)}
                placeholder="Search users by email or license key..."
                class="w-full rounded-xl border border-slate-700/50 bg-slate-800/80 py-3 pr-4 pl-12 text-white placeholder-slate-500 backdrop-blur-sm transition-all focus:border-indigo-500/50 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
              />
            </form>
            <button
              onClick={() => props.onExport('users')}
              class="flex items-center gap-2 rounded-xl border border-slate-700/50 bg-slate-800/80 px-4 py-3 text-sm font-medium text-white backdrop-blur-sm transition-all hover:border-slate-600 hover:bg-slate-700/80"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
              Export CSV
            </button>
          </div>

          {/* Users Table */}
          <div class="overflow-hidden rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 backdrop-blur-sm">
            <Table
              data={props.adminUsers}
              emptyMessage="No users found"
              emptyIcon="👥"
              onRowClick={user => props.onUserClick(user.id)}
              columns={[
                {
                  key: 'email',
                  header: 'User',
                  render: user => (
                    <div class="flex items-center gap-3">
                      <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 text-sm font-bold text-white shadow-lg shadow-indigo-500/20">
                        {user.email.charAt(0).toUpperCase()}
                      </div>
                      <div>
                        <div class="font-medium text-white">{user.email}</div>
                        <div class="text-xs text-slate-500">
                          Joined {api.formatRelativeTime(user.created_at)}
                        </div>
                      </div>
                    </div>
                  ),
                },
                {
                  key: 'tier',
                  header: 'Tier',
                  render: user => <TierBadge tier={user.tier} />,
                },
                {
                  key: 'status',
                  header: 'Status',
                  render: user => <StatusBadge status={user.status} pulse={user.status === 'active'} />,
                },
                {
                  key: 'machines',
                  header: 'Machines',
                  render: user => (
                    <span class="rounded-lg bg-slate-800/80 px-2.5 py-1 text-sm text-slate-300">
                      {user.machines_count || user.machine_count || 0}
                    </span>
                  ),
                },
                {
                  key: 'commands',
                  header: 'Commands',
                  render: user => (
                    <span class="font-medium text-indigo-400">{(user.total_commands || 0).toLocaleString()}</span>
                  ),
                },
                {
                  key: 'last_active',
                  header: 'Last Active',
                  render: user => (
                    <span class="text-sm text-slate-400">
                      {user.last_active ? api.formatRelativeTime(user.last_active) : 'Never'}
                    </span>
                  ),
                },
              ]}
            />
          </div>

          {/* Pagination */}
          <Show when={props.totalPages > 1}>
            <div class="flex items-center justify-between rounded-xl border border-slate-800/60 bg-slate-900/50 p-4">
              <span class="text-sm text-slate-400">
                Page <span class="font-medium text-white">{props.currentPage}</span> of{' '}
                <span class="font-medium text-white">{props.totalPages}</span>
              </span>
              <div class="flex gap-2">
                <button
                  onClick={() => props.onPageChange(props.currentPage - 1)}
                  disabled={props.currentPage <= 1}
                  class="flex items-center gap-1 rounded-lg bg-slate-800 px-4 py-2 text-sm text-white transition-all disabled:opacity-50 hover:bg-slate-700"
                >
                  <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                  </svg>
                  Previous
                </button>
                <button
                  onClick={() => props.onPageChange(props.currentPage + 1)}
                  disabled={props.currentPage >= props.totalPages}
                  class="flex items-center gap-1 rounded-lg bg-slate-800 px-4 py-2 text-sm text-white transition-all disabled:opacity-50 hover:bg-slate-700"
                >
                  Next
                  <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                  </svg>
                </button>
              </div>
            </div>
          </Show>
        </div>
      </Show>

      {/* Revenue Tab */}
      <Show when={activeTab() === 'revenue'}>
        <Show when={props.adminRevenue}>
          <div class="space-y-6">
            {/* Revenue Stats */}
            <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-4">
              <MetricCard
                title="Monthly Recurring Revenue"
                value={`$${(props.adminRevenue!.mrr || 0).toLocaleString()}`}
                icon={<DollarSign size={20} class="text-emerald-400" />}
                iconBg="bg-emerald-500/20"
                badge={{ text: 'MRR', color: 'emerald' }}
              />
              <MetricCard
                title="Annual Run Rate"
                value={`$${(props.adminRevenue!.arr || 0).toLocaleString()}`}
                icon={<TrendingUp size={20} class="text-indigo-400" />}
                iconBg="bg-indigo-500/20"
                badge={{ text: 'ARR', color: 'blue' }}
              />
              <MetricCard
                title="Paying Customers"
                value={(props.adminRevenue!.revenue_by_tier?.reduce((sum, t) => sum + (t.customers || 0), 0) || 0).toLocaleString()}
                icon={<Users size={20} class="text-cyan-400" />}
                iconBg="bg-cyan-500/20"
              />
              <MetricCard
                title="Churn Rate"
                value={props.adminRevenue!.churn?.rate || '0%'}
                icon={<TrendingUp size={20} class="text-amber-400 rotate-180" />}
                iconBg="bg-amber-500/20"
                badge={{ text: props.adminRevenue!.churn?.rate === '0%' ? 'Excellent' : 'Monitor', color: props.adminRevenue!.churn?.rate === '0%' ? 'emerald' : 'amber' }}
              />
            </div>

            {/* Revenue Chart */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <div class="mb-6">
                <h3 class="text-lg font-semibold text-white">Revenue Trend</h3>
                <p class="text-sm text-slate-500">Last 12 months performance</p>
              </div>
              <BarChart
                data={revenueData()}
                height={280}
                showLabels
                gradient="emerald"
                animated
                tooltipFormatter={(v) => `$${v.toLocaleString()}`}
              />
            </div>

            {/* Cohorts */}
            <Show when={props.adminCohorts?.cohort_sizes?.length}>
              <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
                <div class="mb-6">
                  <h3 class="text-lg font-semibold text-white">User Retention Cohorts</h3>
                  <p class="text-sm text-slate-500">Weekly cohort analysis</p>
                </div>
                <div class="overflow-x-auto">
                  <table class="w-full">
                    <thead>
                      <tr class="border-b border-slate-800">
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-slate-400">Cohort</th>
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-slate-400">Size</th>
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-slate-400">Week 1</th>
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-slate-400">Week 2</th>
                        <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-slate-400">Week 4</th>
                      </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-800/50">
                      <For each={props.adminCohorts!.cohort_sizes?.slice(0, 6) || []}>
                        {cohortSize => {
                          const cohortWeek = cohortSize.cohort_week;
                          const size = cohortSize.size || 0;
                          const getRetention = (week: number) => {
                            const data = props.adminCohorts!.cohorts?.find(
                              c => c.cohort_week === cohortWeek && c.weeks_since_signup === week
                            );
                            return data?.active_users || 0;
                          };
                          const week1 = getRetention(1);
                          const week2 = getRetention(2);
                          const week4 = getRetention(4);
                          const getRetentionColor = (retained: number, total: number) => {
                            if (total === 0) return 'text-slate-500';
                            const rate = retained / total;
                            if (rate >= 0.5) return 'text-emerald-400';
                            if (rate >= 0.25) return 'text-amber-400';
                            return 'text-red-400';
                          };
                          return (
                            <tr class="transition-colors hover:bg-slate-800/30">
                              <td class="px-4 py-3 font-medium text-white">{cohortWeek}</td>
                              <td class="px-4 py-3 text-slate-300">{size}</td>
                              <td class="px-4 py-3">
                                <span class={getRetentionColor(week1, size)}>
                                  {week1} <span class="text-slate-500">({size > 0 ? Math.round((week1 / size) * 100) : 0}%)</span>
                                </span>
                              </td>
                              <td class="px-4 py-3">
                                <span class={getRetentionColor(week2, size)}>
                                  {week2} <span class="text-slate-500">({size > 0 ? Math.round((week2 / size) * 100) : 0}%)</span>
                                </span>
                              </td>
                              <td class="px-4 py-3">
                                <span class={getRetentionColor(week4, size)}>
                                  {week4} <span class="text-slate-500">({size > 0 ? Math.round((week4 / size) * 100) : 0}%)</span>
                                </span>
                              </td>
                            </tr>
                          );
                        }}
                      </For>
                    </tbody>
                  </table>
                </div>
              </div>
            </Show>
          </div>
        </Show>
      </Show>

      {/* Activity Tab */}
      <Show when={activeTab() === 'activity'}>
        <div class="space-y-6">
          <div class="flex items-center justify-between">
            <div>
              <h2 class="text-xl font-semibold text-white">Activity Log</h2>
              <p class="text-sm text-slate-500">Complete event history</p>
            </div>
            <button
              onClick={() => props.onExport('audit')}
              class="flex items-center gap-2 rounded-xl border border-slate-700/50 bg-slate-800/80 px-4 py-2.5 text-sm font-medium text-white backdrop-blur-sm transition-all hover:border-slate-600 hover:bg-slate-700/80"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
              Export Log
            </button>
          </div>

          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 backdrop-blur-sm">
            <div class="max-h-[600px] divide-y divide-slate-800/50 overflow-y-auto">
              <For each={props.adminActivity}>
                {activity => {
                  const { icon, bg } = getActivityIcon(activity.type);
                  return (
                    <div class="group flex items-start gap-4 p-5 transition-colors hover:bg-slate-800/30">
                      <div class={`flex h-12 w-12 shrink-0 items-center justify-center rounded-xl ${bg}`}>
                        <span class="text-xl">{icon}</span>
                      </div>
                      <div class="min-w-0 flex-1">
                        <p class="text-sm font-medium text-white">{activity.description}</p>
                        <div class="mt-1 flex flex-wrap items-center gap-3 text-xs text-slate-500">
                          <span>{api.formatRelativeTime(activity.timestamp)}</span>
                          <Show when={activity.user_email}>
                            <span class="flex items-center gap-1">
                              <span>•</span>
                              <span class="text-slate-400">{activity.user_email}</span>
                            </span>
                          </Show>
                          <Show when={activity.platform}>
                            <span class="flex items-center gap-1">
                              <span>•</span>
                              <span class="rounded bg-slate-800 px-1.5 py-0.5 text-slate-400">{activity.platform}</span>
                            </span>
                          </Show>
                        </div>
                      </div>
                      <Show when={activity.user_id}>
                        <button
                          onClick={() => props.onUserClick(activity.user_id)}
                          class="shrink-0 rounded-lg bg-slate-800/80 px-4 py-2 text-xs font-medium text-indigo-400 opacity-0 transition-all hover:bg-slate-700 hover:text-indigo-300 group-hover:opacity-100"
                        >
                          View User →
                        </button>
                      </Show>
                    </div>
                  );
                }}
              </For>
            </div>
          </div>
        </div>
      </Show>

      {/* Analytics Tab */}
      <Show when={activeTab() === 'analytics'}>
        <div class="space-y-8">
          {/* Key Metrics */}
          <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-5">
            <MetricCard
              icon={<Users size={20} class="text-cyan-400" />}
              title="DAU"
              value={props.adminAnalytics?.dau?.toLocaleString() || '0'}
              subtitle="Daily Active Users"
              iconBg="bg-cyan-500/20"
            />
            <MetricCard
              icon={<Calendar size={20} class="text-blue-400" />}
              title="WAU"
              value={props.adminAnalytics?.wau?.toLocaleString() || '0'}
              subtitle="Weekly Active Users"
              iconBg="bg-blue-500/20"
            />
            <MetricCard
              icon={<CalendarDays size={20} class="text-indigo-400" />}
              title="MAU"
              value={props.adminAnalytics?.mau?.toLocaleString() || '0'}
              subtitle="Monthly Active Users"
              iconBg="bg-indigo-500/20"
            />
            <MetricCard
              icon={<RefreshCw size={20} class="text-emerald-400" />}
              title="Retention"
              value={`${props.adminAnalytics?.retention_rate || 0}%`}
              subtitle="Week over week"
              iconBg="bg-emerald-500/20"
            />
            <MetricCard
              icon={<Zap size={20} class="text-purple-400" />}
              title="Events Today"
              value={props.adminAnalytics?.events_today?.toLocaleString() || '0'}
              subtitle="Telemetry events"
              iconBg="bg-purple-500/20"
            />
          </div>

          <div class="grid gap-6 lg:grid-cols-2">
            {/* Commands by Type */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <h3 class="mb-4 text-lg font-semibold text-white">Most Used Commands</h3>
              <div class="space-y-3">
                <For each={props.adminAnalytics?.commands_by_type || []}>
                  {(cmd, _i) => {
                    const maxCount = props.adminAnalytics?.commands_by_type?.[0]?.count || 1;
                    const percentage = (cmd.count / maxCount) * 100;
                    return (
                      <div class="group">
                        <div class="mb-1 flex items-center justify-between text-sm">
                          <span class="font-medium text-white">{cmd.command}</span>
                          <span class="text-slate-400">{cmd.count.toLocaleString()}</span>
                        </div>
                        <div class="h-2 overflow-hidden rounded-full bg-slate-800">
                          <div
                            class="h-full rounded-full bg-gradient-to-r from-cyan-500 to-blue-500 transition-all"
                            style={{ width: `${percentage}%` }}
                          />
                        </div>
                      </div>
                    );
                  }}
                </For>
                <Show when={!props.adminAnalytics?.commands_by_type?.length}>
                  <p class="py-4 text-center text-sm text-slate-500">No command data yet</p>
                </Show>
              </div>
            </div>

            {/* Features by Usage */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <h3 class="mb-4 text-lg font-semibold text-white">Feature Usage</h3>
              <div class="space-y-3">
                <For each={props.adminAnalytics?.features_by_usage || []}>
                  {(feature) => {
                    const maxCount = props.adminAnalytics?.features_by_usage?.[0]?.count || 1;
                    const percentage = (feature.count / maxCount) * 100;
                    return (
                      <div class="group">
                        <div class="mb-1 flex items-center justify-between text-sm">
                          <span class="font-medium text-white">{feature.feature}</span>
                          <span class="text-slate-400">{feature.count.toLocaleString()}</span>
                        </div>
                        <div class="h-2 overflow-hidden rounded-full bg-slate-800">
                          <div
                            class="h-full rounded-full bg-gradient-to-r from-purple-500 to-pink-500 transition-all"
                            style={{ width: `${percentage}%` }}
                          />
                        </div>
                      </div>
                    );
                  }}
                </For>
                <Show when={!props.adminAnalytics?.features_by_usage?.length}>
                  <p class="py-4 text-center text-sm text-slate-500">No feature data yet</p>
                </Show>
              </div>
            </div>
          </div>

          {/* DAU Trend Chart */}
          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <h3 class="mb-4 text-lg font-semibold text-white">Daily Active Users Trend</h3>
            <Show when={props.adminAnalytics?.dau_trend?.length}>
              <BarChart
                data={(props.adminAnalytics?.dau_trend || []).map(d => ({
                  label: d.date.slice(5),
                  value: d.active_users,
                }))}
                height={250}
                showLabels
                gradient="cyan"
                animated
                tooltipFormatter={(v) => `${v.toLocaleString()} users`}
              />
            </Show>
            <Show when={!props.adminAnalytics?.dau_trend?.length}>
              <p class="py-8 text-center text-sm text-slate-500">No trend data available yet</p>
            </Show>
          </div>

          <div class="grid gap-6 lg:grid-cols-2">
            {/* Version Distribution */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <h3 class="mb-4 text-lg font-semibold text-white">Version Distribution</h3>
              <div class="space-y-3">
                <For each={props.adminAnalytics?.version_distribution || []}>
                  {(ver) => {
                    const total = (props.adminAnalytics?.version_distribution || []).reduce((s, v) => s + v.count, 0) || 1;
                    const percentage = (ver.count / total) * 100;
                    return (
                      <div class="flex items-center justify-between rounded-lg bg-slate-800/50 p-3">
                        <div class="flex items-center gap-3">
                          <span class="rounded bg-emerald-500/20 px-2 py-1 font-mono text-xs text-emerald-400">v{ver.version}</span>
                          <span class="text-sm text-slate-300">{ver.count.toLocaleString()} users</span>
                        </div>
                        <span class="text-sm font-medium text-white">{percentage.toFixed(1)}%</span>
                      </div>
                    );
                  }}
                </For>
                <Show when={!props.adminAnalytics?.version_distribution?.length}>
                  <p class="py-4 text-center text-sm text-slate-500">No version data yet</p>
                </Show>
              </div>
            </div>

            {/* Platform Distribution */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <h3 class="mb-4 text-lg font-semibold text-white">Platform Distribution</h3>
              <div class="space-y-3">
                <For each={props.adminAnalytics?.platform_distribution || []}>
                  {(plat) => {
                    const total = (props.adminAnalytics?.platform_distribution || []).reduce((s, p) => s + p.count, 0) || 1;
                    const percentage = (plat.count / total) * 100;
                    const platformIcon = plat.platform.includes('linux') ? '🐧' : plat.platform.includes('darwin') ? '🍎' : '💻';
                    return (
                      <div class="flex items-center justify-between rounded-lg bg-slate-800/50 p-3">
                        <div class="flex items-center gap-3">
                          <span class="text-xl">{platformIcon}</span>
                          <span class="text-sm text-slate-300">{plat.platform}</span>
                        </div>
                        <div class="flex items-center gap-3">
                          <span class="text-sm text-slate-400">{plat.count.toLocaleString()}</span>
                          <span class="text-sm font-medium text-white">{percentage.toFixed(1)}%</span>
                        </div>
                      </div>
                    );
                  }}
                </For>
                <Show when={!props.adminAnalytics?.platform_distribution?.length}>
                  <p class="py-4 text-center text-sm text-slate-500">No platform data yet</p>
                </Show>
              </div>
            </div>
          </div>

          {/* Performance Metrics */}
          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <h3 class="mb-4 text-lg font-semibold text-white">Performance Metrics (Last 7 Days)</h3>
            <div class="overflow-x-auto">
              <table class="w-full">
                <thead>
                  <tr class="border-b border-slate-800">
                    <th class="px-4 py-3 text-left text-sm font-medium text-slate-400">Operation</th>
                    <th class="px-4 py-3 text-right text-sm font-medium text-slate-400">P50</th>
                    <th class="px-4 py-3 text-right text-sm font-medium text-slate-400">P95</th>
                    <th class="px-4 py-3 text-right text-sm font-medium text-slate-400">P99</th>
                    <th class="px-4 py-3 text-right text-sm font-medium text-slate-400">Count</th>
                  </tr>
                </thead>
                <tbody>
                  <For each={Object.entries(props.adminAnalytics?.performance || {})}>
                    {([op, stats]) => (
                      <tr class="border-b border-slate-800/50">
                        <td class="px-4 py-3 font-medium text-white">{op}</td>
                        <td class="px-4 py-3 text-right text-emerald-400">{stats.p50}ms</td>
                        <td class="px-4 py-3 text-right text-amber-400">{stats.p95}ms</td>
                        <td class="px-4 py-3 text-right text-red-400">{stats.p99}ms</td>
                        <td class="px-4 py-3 text-right text-slate-400">{stats.count.toLocaleString()}</td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
              <Show when={!Object.keys(props.adminAnalytics?.performance || {}).length}>
                <p class="py-8 text-center text-sm text-slate-500">No performance data yet</p>
              </Show>
            </div>
          </div>

          {/* Errors */}
          <Show when={props.adminAnalytics?.errors_by_type?.length}>
            <div class="rounded-2xl border border-red-500/30 bg-red-500/5 p-6">
              <h3 class="mb-4 flex items-center gap-2 text-lg font-semibold text-red-400">
                <AlertTriangle size={20} /> Error Tracking
              </h3>
              <div class="space-y-2">
                <For each={props.adminAnalytics?.errors_by_type || []}>
                  {(err) => (
                    <div class="flex items-center justify-between rounded-lg bg-red-500/10 p-3">
                      <span class="font-mono text-sm text-red-300">{err.error_type}</span>
                      <span class="rounded bg-red-500/20 px-2 py-1 text-sm font-medium text-red-400">{err.count.toLocaleString()}</span>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>

          {/* Gold-Tier: User Funnel */}
          <div class="rounded-2xl border border-amber-500/30 bg-gradient-to-br from-amber-500/5 to-orange-500/5 p-6">
            <h3 class="mb-6 flex items-center gap-2 text-lg font-semibold text-amber-400">
              <Target size={20} /> User Funnel
            </h3>
            <div class="flex items-center justify-between gap-2">
              {(() => {
                const funnel = props.adminAnalytics?.funnel;
                const stages = [
                  { label: 'Installs', value: funnel?.installs || 0, color: 'bg-slate-500' },
                  { label: 'Activated', value: funnel?.activated || 0, color: 'bg-blue-500' },
                  { label: 'First Cmd', value: funnel?.first_command || 0, color: 'bg-cyan-500' },
                  { label: 'Engaged', value: funnel?.engaged_7d || 0, color: 'bg-emerald-500' },
                  { label: 'Power Users', value: funnel?.power_users || 0, color: 'bg-amber-500' },
                ];
                const maxVal = Math.max(...stages.map(s => s.value), 1);
                return (
                  <For each={stages}>
                    {(stage, i) => {
                      const height = Math.max((stage.value / maxVal) * 120, 20);
                      const convRate = i() > 0 && stages[i() - 1].value > 0 
                        ? Math.round((stage.value / stages[i() - 1].value) * 100) 
                        : 100;
                      return (
                        <div class="flex flex-1 flex-col items-center gap-2">
                          <span class="text-xs text-slate-400">{stage.value.toLocaleString()}</span>
                          <div 
                            class={`w-full rounded-t-lg ${stage.color} transition-all`}
                            style={{ height: `${height}px` }}
                          />
                          <span class="text-xs font-medium text-white">{stage.label}</span>
                          <Show when={i() > 0}>
                            <span class={`text-xs ${convRate >= 50 ? 'text-emerald-400' : convRate >= 25 ? 'text-amber-400' : 'text-red-400'}`}>
                              {convRate}%
                            </span>
                          </Show>
                        </div>
                      );
                    }}
                  </For>
                );
              })()}
            </div>
          </div>

          {/* Gold-Tier: Growth & Churn */}
          <div class="grid gap-6 lg:grid-cols-2">
            {/* Growth Metrics */}
            <div class="rounded-2xl border border-emerald-500/30 bg-emerald-500/5 p-6">
              <h3 class="mb-4 flex items-center gap-2 text-lg font-semibold text-emerald-400">
                <TrendingUp size={20} /> Growth
              </h3>
              <div class="grid grid-cols-2 gap-4">
                <div class="rounded-lg bg-slate-800/50 p-4">
                  <div class="text-2xl font-bold text-white">{props.adminAnalytics?.growth?.new_users_7d || 0}</div>
                  <div class="text-xs text-slate-400">New Users (7d)</div>
                </div>
                <div class="rounded-lg bg-slate-800/50 p-4">
                  <div class={`text-2xl font-bold ${(props.adminAnalytics?.growth?.growth_rate || 0) >= 0 ? 'text-emerald-400' : 'text-red-400'}`}>
                    {(props.adminAnalytics?.growth?.growth_rate || 0) >= 0 ? '+' : ''}{props.adminAnalytics?.growth?.growth_rate || 0}%
                  </div>
                  <div class="text-xs text-slate-400">Growth Rate</div>
                </div>
                <div class="rounded-lg bg-slate-800/50 p-4">
                  <div class="text-2xl font-bold text-amber-400">{props.adminAnalytics?.growth?.new_paid_7d || 0}</div>
                  <div class="text-xs text-slate-400">New Paid (7d)</div>
                </div>
                <div class="rounded-lg bg-slate-800/50 p-4">
                  <div class="text-2xl font-bold text-cyan-400">{props.adminAnalytics?.retention_rate || 0}%</div>
                  <div class="text-xs text-slate-400">Retention</div>
                </div>
              </div>
            </div>

            {/* Churn Risk */}
            <div class="rounded-2xl border border-red-500/30 bg-red-500/5 p-6">
              <h3 class="mb-4 flex items-center gap-2 text-lg font-semibold text-red-400">
                <AlertTriangle size={20} /> Churn Risk
              </h3>
              <div class="flex items-center gap-6">
                <div class="flex h-24 w-24 items-center justify-center rounded-full border-4 border-red-500/30 bg-red-500/10">
                  <span class="text-3xl font-bold text-red-400">{props.adminAnalytics?.churn_risk?.at_risk_users || 0}</span>
                </div>
                <div>
                  <div class="text-sm text-slate-300">Users at risk of churning</div>
                  <div class="mt-1 text-xs text-slate-500">Inactive 7-14 days after being active</div>
                  <Show when={(props.adminAnalytics?.churn_risk?.at_risk_users || 0) > 0}>
                    <div class="mt-3 text-xs text-amber-400">Consider re-engagement campaign</div>
                  </Show>
                </div>
              </div>
            </div>
          </div>

          {/* Gold-Tier: Cohort Analysis */}
          <Show when={props.adminAnalytics?.cohorts?.length}>
            <div class="rounded-2xl border border-purple-500/30 bg-purple-500/5 p-6">
              <h3 class="mb-4 flex items-center gap-2 text-lg font-semibold text-purple-400">
                <PieChart size={20} /> Cohort Retention
              </h3>
              <div class="overflow-x-auto">
                <table class="w-full">
                  <thead>
                    <tr class="border-b border-slate-800">
                      <th class="px-4 py-2 text-left text-sm font-medium text-slate-400">Cohort</th>
                      <th class="px-4 py-2 text-right text-sm font-medium text-slate-400">Users</th>
                      <th class="px-4 py-2 text-right text-sm font-medium text-slate-400">Active Now</th>
                      <th class="px-4 py-2 text-right text-sm font-medium text-slate-400">Retention</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For each={props.adminAnalytics?.cohorts || []}>
                      {(cohort) => {
                        const retention = cohort.users > 0 ? Math.round((cohort.active_this_week / cohort.users) * 100) : 0;
                        return (
                          <tr class="border-b border-slate-800/50">
                            <td class="px-4 py-2 font-mono text-sm text-white">{cohort.cohort_week}</td>
                            <td class="px-4 py-2 text-right text-slate-300">{cohort.users}</td>
                            <td class="px-4 py-2 text-right text-slate-300">{cohort.active_this_week}</td>
                            <td class={`px-4 py-2 text-right font-medium ${retention >= 50 ? 'text-emerald-400' : retention >= 25 ? 'text-amber-400' : 'text-red-400'}`}>
                              {retention}%
                            </td>
                          </tr>
                        );
                      }}
                    </For>
                  </tbody>
                </table>
              </div>
            </div>
          </Show>

          {/* Gold-Tier: Geographic Distribution */}
          <Show when={props.adminAnalytics?.geo_distribution?.length}>
            <div class="rounded-2xl border border-cyan-500/30 bg-cyan-500/5 p-6">
              <h3 class="mb-4 flex items-center gap-2 text-lg font-semibold text-cyan-400">
                <Globe size={20} /> Geographic Distribution
              </h3>
              <div class="grid grid-cols-2 gap-3 md:grid-cols-4">
                <For each={props.adminAnalytics?.geo_distribution?.slice(0, 8) || []}>
                  {(geo) => {
                    const total = (props.adminAnalytics?.geo_distribution || []).reduce((s, g) => s + g.users, 0) || 1;
                    const pct = Math.round((geo.users / total) * 100);
                    return (
                      <div class="rounded-lg bg-slate-800/50 p-3">
                        <div class="truncate text-sm font-medium text-white">{geo.timezone || 'Unknown'}</div>
                        <div class="mt-1 flex items-center justify-between">
                          <span class="text-xs text-slate-400">{geo.users} users</span>
                          <span class="text-xs text-cyan-400">{pct}%</span>
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
};
