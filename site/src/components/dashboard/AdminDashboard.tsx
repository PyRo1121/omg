import { Component, For, Show, createSignal } from 'solid-js';
import * as api from '../../lib/api';
import { MetricCard } from '../ui/Card';
import { TierBadge, StatusBadge } from '../ui/Badge';
import { BarChart, DonutChart, LiveIndicator } from '../ui/Chart';
import { Table } from '../ui/Table';
import { SmartInsights } from './SmartInsights';
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
  Package,
  CheckCircle,
  AlertCircle,
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
    <div class="space-y-8 pb-12">
      <div class="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
        <div class="flex items-start gap-5">
          <div class="relative flex h-16 w-16 items-center justify-center rounded-[1.25rem] bg-gradient-to-br from-amber-400 via-orange-500 to-red-600 shadow-2xl shadow-orange-500/20">
            <Crown size={32} class="text-white drop-shadow-lg" />
            <div class="absolute -inset-1 rounded-[1.4rem] border border-orange-500/30 blur-sm" />
          </div>
          <div>
            <div class="flex items-center gap-3">
              <h1 class="text-4xl font-black tracking-tight text-white lg:text-5xl">Admin</h1>
              <div class="mt-1 flex items-center gap-2 rounded-full bg-emerald-500/10 px-3 py-1 text-[10px] font-bold uppercase tracking-widest text-emerald-400 ring-1 ring-emerald-500/20">
                <span class="relative flex h-2 w-2">
                  <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75"></span>
                  <span class="relative inline-flex h-2 w-2 rounded-full bg-emerald-400"></span>
                </span>
                Live Console
              </div>
            </div>
            <p class="mt-2 text-slate-400 font-medium">
              System governance, user telemetry, and global operations.
            </p>
          </div>
        </div>
        
        <div class="flex flex-wrap items-center gap-3">
          <button
            onClick={handleRefresh}
            disabled={isRefreshing()}
            class="group flex items-center gap-2 rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-3 text-sm font-bold text-white transition-all hover:bg-white/[0.08] disabled:opacity-50"
          >
            <RefreshCw size={16} class={isRefreshing() ? 'animate-spin' : 'group-hover:rotate-180 transition-transform duration-500'} />
            {isRefreshing() ? 'Syncing...' : 'Refresh'}
          </button>
          
          <div class="relative group">
            <button class="flex items-center gap-3 rounded-2xl bg-white px-6 py-3 text-sm font-bold text-black shadow-xl shadow-white/10 transition-all hover:scale-[1.02] active:scale-[0.98]">
              <FileText size={18} />
              Export
              <svg class="h-4 w-4 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
              </svg>
            </button>
            <div class="absolute right-0 top-full z-50 mt-2 hidden w-56 rounded-2xl border border-white/10 bg-[#151516] p-2 shadow-2xl backdrop-blur-xl group-hover:block animate-in fade-in slide-in-from-top-2">
              <button onClick={() => props.onExport('users')} class="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium text-slate-300 hover:bg-white/5 hover:text-white transition-colors">
                <Users size={18} class="text-indigo-400" /> Export Users
              </button>
              <button onClick={() => props.onExport('usage')} class="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium text-slate-300 hover:bg-white/5 hover:text-white transition-colors">
                <BarChart3 size={18} class="text-emerald-400" /> Export Usage
              </button>
              <div class="my-1 border-t border-white/5" />
              <button onClick={() => props.onExport('audit')} class="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium text-slate-300 hover:bg-white/5 hover:text-white transition-colors">
                <FileText size={18} class="text-amber-400" /> Audit Log
              </button>
            </div>
          </div>
        </div>
      </div>

      <div class="flex items-center gap-1 rounded-[1.5rem] border border-white/5 bg-white/[0.02] p-1.5 backdrop-blur-xl">
        <For each={[
          { id: 'overview' as const, label: 'Overview', Icon: BarChart3, color: 'text-indigo-400' },
          { id: 'users' as const, label: 'Users', Icon: Users, color: 'text-emerald-400' },
          { id: 'revenue' as const, label: 'Revenue', Icon: DollarSign, color: 'text-amber-400' },
          { id: 'activity' as const, label: 'Activity', Icon: Activity, color: 'text-rose-400' },
          { id: 'analytics' as const, label: 'Analytics', Icon: TrendingUp, color: 'text-cyan-400' },
        ]}>{tab => (
          <button
            onClick={() => setActiveTab(tab.id)}
            class={`relative flex flex-1 items-center justify-center gap-3 rounded-[1.25rem] py-3.5 text-sm font-bold transition-all duration-300 ${
              activeTab() === tab.id
                ? 'bg-white text-black shadow-lg shadow-white/5 scale-[1.02]'
                : 'text-slate-400 hover:text-white hover:bg-white/5'
            }`}
          >
            <tab.Icon size={18} class={activeTab() === tab.id ? 'text-black' : tab.color} />
            <span class="hidden sm:inline">{tab.label}</span>
          </button>
        )}</For>
      </div>

      <Show when={activeTab() === 'overview'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
            <div class="relative overflow-hidden rounded-[2rem] border border-emerald-500/20 bg-emerald-500/[0.03] p-8 shadow-2xl">
              <div class="absolute -right-10 -top-10 h-32 w-32 rounded-full bg-emerald-500/10 blur-3xl" />
              <div class="flex flex-col h-full justify-between">
                <div>
                  <div class="flex items-center gap-3 text-emerald-400">
                    <TrendingUp size={20} />
                    <span class="text-[10px] font-black uppercase tracking-widest">Global Economy Realized</span>
                  </div>
                  <div class="mt-4 flex items-baseline gap-2">
                    <span class="text-sm font-black text-emerald-400">$</span>
                    <span class="text-5xl font-black text-white">{((props.adminData?.overview?.global_value_usd || 0) / 1000).toFixed(1)}k</span>
                  </div>
                  <p class="mt-2 text-sm font-medium text-slate-400 text-opacity-80">Aggregate value delivered via OMG optimization.</p>
                </div>
              </div>
            </div>

            <div class="relative overflow-hidden rounded-[2rem] border border-indigo-500/20 bg-indigo-500/[0.03] p-8 shadow-2xl">
              <div class="absolute -right-10 -top-10 h-32 w-32 rounded-full bg-indigo-500/10 blur-3xl" />
              <div class="flex flex-col h-full justify-between">
                <div>
                  <div class="flex items-center gap-3 text-indigo-400">
                    <Clock size={20} />
                    <span class="text-[10px] font-black uppercase tracking-widest">Dev Hours Reclaimed</span>
                  </div>
                  <div class="mt-4 flex items-baseline gap-2">
                    <span class="text-5xl font-black text-white">{Math.floor((props.adminData?.usage?.total_time_saved_ms || 0) / 3600000).toLocaleString()}</span>
                    <span class="text-lg font-bold text-indigo-500">Hrs</span>
                  </div>
                  <p class="mt-2 text-sm font-medium text-slate-400 text-opacity-80">Total productive time saved across all entities.</p>
                </div>
              </div>
            </div>

            <MetricCard
              title="Global Fleet"
              value={(props.adminData?.overview?.active_machines || 0).toLocaleString()}
              icon={<Monitor size={22} class="text-cyan-400" />}
              iconBg="bg-cyan-500/10"
              subtitle="Total active machine nodes"
              badge={{ text: 'Scale', color: 'cyan' }}
            />

            <div class="relative overflow-hidden rounded-[2rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
              <div class="mb-4 flex items-center justify-between">
                <h3 class="text-sm font-bold text-white uppercase tracking-widest">System Health</h3>
                <LiveIndicator label="Nominal" />
              </div>
              <div class="space-y-4">
                <div class="flex justify-between items-center">
                  <span class="text-[10px] font-bold text-slate-500 uppercase">Success Rate</span>
                  <span class="text-sm font-black text-emerald-400">99.92%</span>
                </div>
                <div class="h-1.5 rounded-full bg-white/[0.03] overflow-hidden">
                  <div class="h-full bg-emerald-500 shadow-[0_0_12px_rgba(16,185,129,0.4)]" style="width: 99.92%" />
                </div>
                <div class="flex justify-between items-center">
                  <span class="text-[10px] font-bold text-slate-500 uppercase">Latency (P95)</span>
                  <span class="text-sm font-black text-indigo-400">12ms</span>
                </div>
              </div>
            </div>
          </div>

          <Show when={props.adminHealth}>
            <div class="relative overflow-hidden rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <div class="absolute -right-40 -top-40 h-[500px] w-[500px] rounded-full bg-emerald-500/[0.03] blur-[100px]" />
              <div class="absolute -bottom-40 -left-40 h-[500px] w-[500px] rounded-full bg-indigo-500/[0.03] blur-[100px]" />
              
              <div class="relative flex flex-col gap-10">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-4">
                    <div class="flex h-12 w-12 items-center justify-center rounded-2xl bg-emerald-500/10 text-emerald-400">
                      <Activity size={24} />
                    </div>
                    <div>
                      <h2 class="text-2xl font-black text-white">System Vitality</h2>
                      <p class="text-sm font-medium text-slate-500">Real-time health telemetry across all clusters.</p>
                    </div>
                  </div>
                  <div class="flex items-center gap-6">
                    <div class="text-right">
                      <p class="text-[10px] font-bold uppercase tracking-widest text-slate-500">Global Load</p>
                      <p class="text-sm font-bold text-white">Nominal</p>
                    </div>
                    <div class="h-10 w-[1px] bg-white/10" />
                    <LiveIndicator label="Syncing" />
                  </div>
                </div>
                
                <div class="grid grid-cols-2 gap-4 md:grid-cols-5">
                  <For each={[
                    { label: 'Active Today', value: props.adminHealth!.active_users_today, color: 'text-emerald-400', icon: '👤', trend: '+5%' },
                    { label: 'Weekly Active', value: props.adminHealth!.active_users_week, color: 'text-cyan-400', icon: '📅', trend: '+12%' },
                    { label: 'Cmds Today', value: (props.adminHealth!.commands_today || 0).toLocaleString(), color: 'text-indigo-400', icon: '⚡', trend: '+8%' },
                    { label: 'New Signups', value: props.adminHealth!.new_users_today, color: 'text-amber-400', icon: '✨', trend: '+2%' },
                    { label: 'Installs', value: props.adminHealth!.installs_today, color: 'text-rose-400', icon: '📦', trend: '+15%' },
                  ]}>{stat => (
                    <div class="group relative rounded-[2rem] border border-white/[0.03] bg-white/[0.01] p-6 transition-all hover:bg-white/[0.04] hover:border-white/10">
                      <div class="mb-4 flex items-center justify-between">
                        <span class="text-xl opacity-50 group-hover:opacity-100 transition-opacity">{stat.icon}</span>
                        <span class={`text-[10px] font-bold ${stat.color} opacity-0 group-hover:opacity-100 transition-opacity`}>{stat.trend}</span>
                      </div>
                      <div class="text-3xl font-black text-white">{stat.value}</div>
                      <div class="mt-1 text-[11px] font-bold uppercase tracking-widest text-slate-500">{stat.label}</div>
                    </div>
                  )}</For>
                </div>
              </div>
            </div>
          </Show>

          <Show when={props.adminData}>
            <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
              <MetricCard
                title="Global Userbase"
                value={(props.adminData!.overview?.total_users || 0).toLocaleString()}
                icon={<Users size={22} class="text-indigo-400" />}
                iconBg="bg-indigo-500/10"
                sparklineData={commandsSparkline()}
                sparklineColor="#818cf8"
                subtitle="Aggregated registered entities"
              />
              <MetricCard
                title="Active Entitlements"
                value={(props.adminData!.overview?.active_licenses || 0).toLocaleString()}
                icon={<Key size={22} class="text-emerald-400" />}
                iconBg="bg-emerald-500/10"
                subtitle="Validated commercial seats"
                badge={{ text: 'Verified', color: 'emerald' }}
              />
              <MetricCard
                title="Provisioned Nodes"
                value={(props.adminData!.overview?.active_machines || 0).toLocaleString()}
                icon={<Monitor size={22} class="text-cyan-400" />}
                iconBg="bg-cyan-500/10"
                subtitle="Connected machine instances"
              />
              <MetricCard
                title="Operations"
                value={(props.adminData!.usage?.total_commands || 0).toLocaleString()}
                icon={<Zap size={22} class="text-purple-400" />}
                iconBg="bg-purple-500/10"
                sparklineData={commandsSparkline()}
                sparklineColor="#a78bfa"
                subtitle="Total CLI execution volume"
              />
            </div>

            <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
              <div class="rounded-[2rem] border border-white/5 bg-white/[0.02] p-8 shadow-2xl backdrop-blur-xl">
                <div class="mb-6 flex items-center justify-between">
                  <div>
                    <h3 class="text-xl font-bold text-white">Execution Health</h3>
                    <p class="text-xs font-medium text-slate-500">Global CLI success/failure ratio.</p>
                  </div>
                  <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-400">
                    <TrendingUp size={20} />
                  </div>
                </div>
                <div class="flex items-center gap-10">
                  <div class="flex-1 space-y-6">
                    <div>
                      <div class="mb-2 flex justify-between text-xs font-bold uppercase tracking-widest">
                        <span class="text-slate-500">Global Success Rate</span>
                        <span class="text-emerald-400">
                          {(() => {
                            const s = props.adminData!.overview?.command_health?.success || 0;
                            const f = props.adminData!.overview?.command_health?.failure || 0;
                            const total = s + f;
                            return total > 0 ? ((s / total) * 100).toFixed(1) : '100';
                          })()}%
                        </span>
                      </div>
                      <div class="h-3 overflow-hidden rounded-full bg-white/[0.05]">
                        <div 
                          class="h-full bg-gradient-to-r from-emerald-500 to-teal-400 transition-all duration-1000 shadow-[0_0_12px_rgba(16,185,129,0.4)]"
                          style={{ 
                            width: `${(() => {
                              const s = props.adminData!.overview?.command_health?.success || 0;
                              const f = props.adminData!.overview?.command_health?.failure || 0;
                              const total = s + f;
                              return total > 0 ? (s / total) * 100 : 100;
                            })()}%` 
                          }}
                        />
                      </div>
                    </div>
                    <div class="grid grid-cols-2 gap-8">
                      <div class="rounded-2xl border border-white/[0.03] bg-white/[0.01] p-4">
                        <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Successful</div>
                        <div class="mt-1 text-2xl font-black text-white">{(props.adminData!.overview?.command_health?.success || 0).toLocaleString()}</div>
                      </div>
                      <div class="rounded-2xl border border-white/[0.03] bg-white/[0.01] p-4">
                        <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Anomalies</div>
                        <div class="mt-1 text-2xl font-black text-rose-500">{(props.adminData!.overview?.command_health?.failure || 0).toLocaleString()}</div>
                      </div>
                    </div>
                  </div>
                  <div class="hidden sm:block">
                    <DonutChart 
                      data={[
                        { label: 'Success', value: props.adminData!.overview?.command_health?.success || 1, color: '#10b981' },
                        { label: 'Failure', value: props.adminData!.overview?.command_health?.failure || 0, color: '#f43f5e' }
                      ]} 
                      size={120} 
                      thickness={14}
                    />
                  </div>
                </div>
              </div>

              <div class="rounded-[2rem] border border-white/5 bg-white/[0.02] p-8 shadow-2xl backdrop-blur-xl">
                <div class="mb-6 flex items-center justify-between">
                  <div>
                    <h3 class="text-xl font-bold text-white">Node Distribution</h3>
                    <p class="text-xs font-medium text-slate-500">Top geographic regions by machine count.</p>
                  </div>
                  <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-500/10 text-indigo-400">
                    <Globe size={20} />
                  </div>
                </div>
                <div class="space-y-4">
                  <For each={props.adminData!.geo_distribution?.slice(0, 4)}>
                    {geo => (
                      <div class="group flex items-center justify-between rounded-2xl bg-white/[0.02] px-5 py-3 transition-all hover:bg-white/[0.05]">
                        <div class="flex items-center gap-4">
                          <div class="h-2 w-2 rounded-full bg-indigo-500 group-hover:shadow-[0_0_8px_rgba(99,102,241,0.6)]" />
                          <span class="text-sm font-bold text-slate-300">{geo.dimension || 'Unknown'}</span>
                        </div>
                        <div class="flex items-center gap-4">
                          <div class="h-1.5 w-32 overflow-hidden rounded-full bg-white/[0.05]">
                            <div 
                              class="h-full bg-gradient-to-r from-indigo-600 to-indigo-400 shadow-[0_0_8px_rgba(99,102,241,0.3)]" 
                              style={{ width: `${Math.min((geo.count / (props.adminData!.overview?.total_users || 1)) * 100 * 5, 100)}%` }} 
                            />
                          </div>
                          <span class="text-xs font-black text-white min-w-[2rem] text-right">{geo.count}</span>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </div>

            <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
              <div class="rounded-[2rem] border border-white/5 bg-[#0d0d0e] p-8 lg:col-span-2">
                <div class="mb-6 flex items-center justify-between">
                  <div>
                    <h3 class="text-xl font-bold text-white">Trending High-Volume Packages</h3>
                    <p class="text-xs font-medium text-slate-500">Most active package installs in the last 24h.</p>
                  </div>
                  <button onClick={() => setActiveTab('analytics')} class="text-xs font-bold text-indigo-400 hover:text-indigo-300 transition-colors uppercase tracking-widest">Analytics →</button>
                </div>
                <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  <For each={props.adminAnalytics?.top_packages?.slice(0, 4) || []}>
                    {(pkg: any) => (
                      <div class="group flex items-center justify-between rounded-2xl border border-white/[0.03] bg-white/[0.01] p-4 transition-all hover:bg-white/[0.05] hover:border-white/10">
                        <div class="flex items-center gap-4">
                          <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-400 group-hover:scale-110 transition-transform">
                            <Package size={20} />
                          </div>
                          <div>
                            <span class="block font-bold text-white">{pkg.package_name}</span>
                            <span class="text-[10px] font-bold text-slate-600 uppercase">Stable</span>
                          </div>
                        </div>
                        <div class="text-right">
                          <div class="text-lg font-black text-white">{pkg.install_count}</div>
                          <div class="text-[10px] font-bold text-emerald-500 uppercase tracking-tight">Installs</div>
                        </div>
                      </div>
                    )}
                  </For>
                  <Show when={!props.adminAnalytics?.top_packages?.length}>
                    <div class="flex h-32 flex-col items-center justify-center py-4 text-center col-span-2 rounded-2xl border border-dashed border-white/10">
                      <Package size={32} class="text-slate-700 mb-2" />
                      <p class="text-sm font-medium text-slate-500">Awaiting telemetry stream...</p>
                    </div>
                  </Show>
                </div>
              </div>

              <div class="rounded-[2rem] border border-white/5 bg-[#0d0d0e] p-8">
                <div class="mb-6 flex items-center justify-between">
                  <div>
                    <h3 class="text-xl font-bold text-white">Security Alerts</h3>
                    <p class="text-xs font-medium text-slate-500">Real-time anomaly detection.</p>
                  </div>
                  <Show when={props.adminAnalytics?.top_errors?.length}>
                    <div class="flex h-2 w-2 rounded-full bg-rose-500 shadow-[0_0_12px_rgba(244,63,94,0.6)] animate-pulse" />
                  </Show>
                </div>
                <div class="space-y-4">
                  <For each={props.adminAnalytics?.top_errors?.slice(0, 3) || []}>
                    {(err: any) => (
                      <div class="group relative rounded-2xl border border-rose-500/10 bg-rose-500/[0.03] p-4 transition-all hover:bg-rose-500/[0.08] hover:border-rose-500/30">
                        <div class="mb-2 flex items-center justify-between">
                          <span class="flex items-center gap-2 text-[10px] font-black text-rose-400 uppercase tracking-widest">
                            <AlertCircle size={12} />
                            Critical Error
                          </span>
                          <span class="rounded bg-rose-500/10 px-1.5 py-0.5 text-[10px] font-bold text-rose-500">{err.occurrences}x</span>
                        </div>
                        <div class="truncate font-mono text-xs font-bold text-slate-300 bg-black/20 p-2 rounded-lg">{err.error_message}</div>
                      </div>
                    )}
                  </For>
                  <Show when={!props.adminAnalytics?.top_errors?.length}>
                    <div class="flex flex-col items-center justify-center py-10 text-center rounded-2xl bg-emerald-500/[0.02] border border-emerald-500/10">
                      <div class="mb-3 flex h-12 w-12 items-center justify-center rounded-full bg-emerald-500/10 text-emerald-400 shadow-[0_0_20px_rgba(16,185,129,0.1)]">
                        <CheckCircle size={24} />
                      </div>
                      <p class="text-sm font-bold text-emerald-500">Zero System Errors</p>
                      <p class="mt-1 text-xs text-slate-600">Health checks passing</p>
                    </div>
                  </Show>
                </div>
              </div>
            </div>
          </Show>

          <div class="grid grid-cols-1 gap-6 xl:grid-cols-3">
            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
              <div class="mb-8 flex items-center justify-between">
                <div>
                  <h3 class="text-xl font-bold text-white">Segment Share</h3>
                  <p class="text-xs font-medium text-slate-500">Distribution by customer tier.</p>
                </div>
                <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-purple-500/10 text-purple-400">
                  <PieChart size={20} />
                </div>
              </div>
              <div class="flex justify-center py-4">
                <DonutChart 
                  data={tierDistribution()} 
                  size={200} 
                  thickness={24}
                  centerLabel="Entities"
                  showLegend
                />
              </div>
            </div>

            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl xl:col-span-2">
              <div class="mb-8 flex items-center justify-between">
                <div>
                  <h3 class="text-xl font-bold text-white">Commercial Velocity</h3>
                  <p class="text-xs font-medium text-slate-500">Gross revenue performance (Trailing 12M).</p>
                </div>
                <Show when={props.adminRevenue}>
                  <div class="text-right">
                    <div class="flex items-center justify-end gap-2 text-3xl font-black text-emerald-400">
                      <DollarSign size={24} />
                      {(props.adminRevenue!.mrr || 0).toLocaleString()}
                    </div>
                    <div class="text-[10px] font-bold uppercase tracking-widest text-slate-600">Current MRR Target Met</div>
                  </div>
                </Show>
              </div>
              <BarChart
                data={revenueData()}
                height={260}
                showLabels
                gradient="emerald"
                animated
                tooltipFormatter={(v) => `$${v.toLocaleString()}`}
              />
            </div>
          </div>

          <div class="rounded-[2.5rem] border border-white/5 bg-gradient-to-br from-[#0d0d0e] to-black p-10 shadow-2xl">
            <div class="mb-8 flex items-center justify-between">
              <div class="flex items-center gap-4">
                <div class="flex h-12 w-12 items-center justify-center rounded-[1.25rem] bg-indigo-500/10 text-indigo-400">
                  <Activity size={24} />
                </div>
                <div>
                  <h3 class="text-2xl font-black text-white">Event Horizon</h3>
                  <p class="text-sm font-medium text-slate-500">Global audit trail of system-wide interactions.</p>
                </div>
              </div>
              <button
                onClick={() => setActiveTab('activity')}
                class="rounded-2xl bg-white/[0.03] px-6 py-3 text-sm font-bold text-white transition-all hover:bg-white/[0.08]"
              >
                View Full Ledger →
              </button>
            </div>
            
            <div class="grid gap-4 md:grid-cols-2">
              <For each={props.adminActivity.slice(0, 6)}>
                {activity => {
                  const { icon, bg } = getActivityIcon(activity.type);
                  return (
                    <div class="group flex items-center gap-5 rounded-3xl border border-white/[0.03] bg-white/[0.01] p-5 transition-all hover:bg-white/[0.04] hover:border-white/10">
                      <div class={`flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl shadow-inner ${bg}`}>
                        <span class="text-xl group-hover:scale-110 transition-transform">{icon}</span>
                      </div>
                      <div class="min-w-0 flex-1">
                        <p class="truncate text-sm font-bold text-white group-hover:text-indigo-300 transition-colors">{activity.description}</p>
                        <p class="text-[11px] font-bold text-slate-600 uppercase tracking-tight mt-0.5">{api.formatRelativeTime(activity.timestamp)}</p>
                      </div>
                      <Show when={activity.user_id}>
                        <button
                          onClick={() => props.onUserClick(activity.user_id)}
                          class="shrink-0 rounded-xl bg-white/5 px-4 py-2 text-xs font-bold text-slate-400 opacity-0 transition-all hover:bg-white/10 hover:text-white group-hover:opacity-100"
                        >
                          Profile
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

      <Show when={activeTab() === 'users'}>
        <div class="space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <form onSubmit={handleSearch} class="relative flex-1 sm:max-w-xl">
              <svg class="absolute left-5 top-1/2 h-5 w-5 -translate-y-1/2 text-slate-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
              </svg>
              <input
                type="text"
                value={searchInput()}
                onInput={e => setSearchInput(e.currentTarget.value)}
                placeholder="Search by identity, machine ID, or license..."
                class="w-full rounded-[1.25rem] border border-white/10 bg-white/[0.03] py-4 pr-6 pl-14 text-white placeholder-slate-600 focus:border-indigo-500 focus:bg-white/[0.05] focus:outline-none focus:ring-4 focus:ring-indigo-500/10 transition-all"
              />
            </form>
            <button
              onClick={() => props.onExport('users')}
              class="flex items-center gap-2 rounded-2xl bg-white/[0.03] border border-white/10 px-6 py-4 text-sm font-bold text-white transition-all hover:bg-white/[0.08]"
            >
              <FileText size={18} class="text-indigo-400" />
              Dataset Export
            </button>
          </div>

          <div class="overflow-hidden rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] shadow-2xl">
            <Table
              data={props.adminUsers}
              emptyMessage="No matching entities found in the registry."
              emptyIcon="👥"
              onRowClick={user => props.onUserClick(user.id)}
              columns={[
                {
                  key: 'email',
                  header: 'Entity / Identity',
                  render: user => (
                    <div class="flex items-center gap-4 py-1">
                      <div class="flex h-12 w-12 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500/20 to-purple-600/20 border border-indigo-500/20 text-lg font-black text-indigo-400 shadow-inner">
                        {user.email.charAt(0).toUpperCase()}
                      </div>
                      <div>
                        <div class="font-black text-white group-hover:text-indigo-300 transition-colors">{user.email}</div>
                        <div class="text-[10px] font-bold text-slate-600 uppercase tracking-widest mt-0.5">
                          Registered {api.formatRelativeTime(user.created_at)}
                        </div>
                      </div>
                    </div>
                  ),
                },
                {
                  key: 'tier',
                  header: 'Service Tier',
                  render: user => <TierBadge tier={user.tier} />,
                },
                {
                  key: 'status',
                  header: 'Access State',
                  render: user => <StatusBadge status={user.status} pulse={user.status === 'active'} />,
                },
                {
                  key: 'machines',
                  header: 'Nodes',
                  render: user => (
                    <div class="flex items-center gap-2">
                      <Monitor size={14} class="text-slate-600" />
                      <span class="text-sm font-black text-slate-300">
                        {user.machines_count || user.machine_count || 0}
                      </span>
                    </div>
                  ),
                },
                {
                  key: 'commands',
                  header: 'Ops Volume',
                  render: user => (
                    <div class="flex items-center gap-2">
                      <Zap size={14} class="text-indigo-500" />
                      <span class="font-black text-white">{(user.total_commands || 0).toLocaleString()}</span>
                    </div>
                  ),
                },
                {
                  key: 'last_active',
                  header: 'Last Signal',
                  render: user => (
                    <span class="text-xs font-bold text-slate-500 uppercase tracking-tight">
                      {user.last_active ? api.formatRelativeTime(user.last_active) : 'Dark'}
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

      <Show when={activeTab() === 'revenue'}>
        <Show when={props.adminRevenue}>
          <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
            <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
              <MetricCard
                title="Monthly Recurring"
                value={`$${(props.adminRevenue!.mrr || 0).toLocaleString()}`}
                icon={<DollarSign size={22} class="text-emerald-400" />}
                iconBg="bg-emerald-500/10"
                badge={{ text: 'MRR', color: 'emerald' }}
                subtitle="Aggregated subscription revenue"
              />
              <MetricCard
                title="Annual Run Rate"
                value={`$${(props.adminRevenue!.arr || 0).toLocaleString()}`}
                icon={<TrendingUp size={22} class="text-indigo-400" />}
                iconBg="bg-indigo-500/10"
                badge={{ text: 'ARR', color: 'blue' }}
                subtitle="Projected yearly performance"
              />
              <MetricCard
                title="Commercial Entities"
                value={(props.adminRevenue!.revenue_by_tier?.reduce((sum, t) => sum + (t.customers || 0), 0) || 0).toLocaleString()}
                icon={<Users size={22} class="text-cyan-400" />}
                iconBg="bg-cyan-500/10"
                subtitle="Active paying customer base"
              />
              <MetricCard
                title="Churn Velocity"
                value={props.adminRevenue!.churn?.rate || '0%'}
                icon={<Activity size={22} class="text-amber-400" />}
                iconBg="bg-amber-500/10"
                badge={{ text: props.adminRevenue!.churn?.rate === '0%' ? 'Optimal' : 'Monitor', color: props.adminRevenue!.churn?.rate === '0%' ? 'emerald' : 'amber' }}
                subtitle="Net customer attrition rate"
              />
            </div>

            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <div class="mb-10 flex items-center justify-between">
                <div>
                  <h3 class="text-2xl font-black text-white tracking-tight">Revenue Trajectory</h3>
                  <p class="text-sm font-medium text-slate-500 text-opacity-80">Trailing 12-month fiscal performance ledger.</p>
                </div>
                <div class="flex items-center gap-2 rounded-full bg-emerald-500/10 px-4 py-2 ring-1 ring-emerald-500/20">
                  <div class="h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
                  <span class="text-[10px] font-black uppercase tracking-widest text-emerald-400">Target: +15% MoM</span>
                </div>
              </div>
              <BarChart
                data={revenueData()}
                height={300}
                showLabels
                gradient="emerald"
                animated
                tooltipFormatter={(v) => `$${v.toLocaleString()}`}
              />
            </div>

            <Show when={props.adminCohorts?.cohort_sizes?.length}>
              <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
                <div class="mb-8">
                  <h3 class="text-2xl font-black text-white tracking-tight">Retention Cohorts</h3>
                  <p class="text-sm font-medium text-slate-500">Longitudinal user engagement analysis.</p>
                </div>
                <div class="overflow-hidden rounded-3xl border border-white/5 bg-black/20">
                  <table class="w-full text-left border-collapse">
                    <thead>
                      <tr class="border-b border-white/5">
                        <th class="px-6 py-5 text-[10px] font-black uppercase tracking-widest text-slate-500">Cohort Week</th>
                        <th class="px-6 py-5 text-[10px] font-black uppercase tracking-widest text-slate-500">Entities</th>
                        <th class="px-6 py-5 text-[10px] font-black uppercase tracking-widest text-slate-500">W1 Retention</th>
                        <th class="px-6 py-5 text-[10px] font-black uppercase tracking-widest text-slate-500">W2 Retention</th>
                        <th class="px-6 py-5 text-[10px] font-black uppercase tracking-widest text-slate-500">W4 Retention</th>
                      </tr>
                    </thead>
                    <tbody class="divide-y divide-white/[0.02]">
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
                            if (total === 0) return 'text-slate-600';
                            const rate = retained / total;
                            if (rate >= 0.5) return 'text-emerald-400 bg-emerald-500/10 border-emerald-500/20';
                            if (rate >= 0.25) return 'text-amber-400 bg-amber-500/10 border-amber-500/20';
                            return 'text-rose-400 bg-rose-500/10 border-rose-500/20';
                          };
                          return (
                            <tr class="group transition-colors hover:bg-white/[0.02]">
                              <td class="px-6 py-5 font-bold text-white tracking-tight">{cohortWeek}</td>
                              <td class="px-6 py-5">
                                <span class="rounded-lg bg-white/5 px-2.5 py-1 text-xs font-black text-slate-400">{size}</span>
                              </td>
                              <td class="px-6 py-5">
                                <div class={`inline-flex items-center gap-2 rounded-xl border px-3 py-1.5 text-xs font-black ${getRetentionColor(week1, size)}`}>
                                  {week1} <span class="opacity-50">({size > 0 ? Math.round((week1 / size) * 100) : 0}%)</span>
                                </div>
                              </td>
                              <td class="px-6 py-5">
                                <div class={`inline-flex items-center gap-2 rounded-xl border px-3 py-1.5 text-xs font-black ${getRetentionColor(week2, size)}`}>
                                  {week2} <span class="opacity-50">({size > 0 ? Math.round((week2 / size) * 100) : 0}%)</span>
                                </div>
                              </td>
                              <td class="px-6 py-5">
                                <div class={`inline-flex items-center gap-2 rounded-xl border px-3 py-1.5 text-xs font-black ${getRetentionColor(week4, size)}`}>
                                  {week4} <span class="opacity-50">({size > 0 ? Math.round((week4 / size) * 100) : 0}%)</span>
                                </div>
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

      <Show when={activeTab() === 'activity'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div class="flex items-center justify-between border-b border-white/5 pb-6">
            <div>
              <h2 class="text-3xl font-black text-white tracking-tight">System Ledger</h2>
              <p class="text-sm font-medium text-slate-500">Immutable audit trail of all global entity interactions.</p>
            </div>
            <button
              onClick={() => props.onExport('audit')}
              class="flex items-center gap-3 rounded-[1.25rem] bg-white/[0.03] border border-white/10 px-6 py-4 text-sm font-bold text-white transition-all hover:bg-white/[0.08]"
            >
              <FileText size={18} class="text-amber-400" />
              Download Full Audit
            </button>
          </div>

          <div class="overflow-hidden rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] shadow-2xl">
            <div class="max-h-[800px] divide-y divide-white/[0.03] overflow-y-auto custom-scrollbar">
              <For each={props.adminActivity}>
                {activity => {
                  const { icon, bg } = getActivityIcon(activity.type);
                  return (
                    <div class="group flex items-start gap-6 p-6 transition-all hover:bg-white/[0.02]">
                      <div class={`flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl shadow-inner transition-transform group-hover:scale-110 ${bg}`}>
                        <span class="text-2xl">{icon}</span>
                      </div>
                      <div class="min-w-0 flex-1">
                        <div class="flex items-center gap-3">
                          <p class="text-base font-bold text-white tracking-tight">{activity.description}</p>
                          <Show when={activity.platform}>
                            <span class="rounded-lg bg-white/5 border border-white/5 px-2 py-0.5 text-[9px] font-black uppercase text-slate-500">{activity.platform}</span>
                          </Show>
                        </div>
                        <div class="mt-2 flex flex-wrap items-center gap-4 text-xs font-bold text-slate-600">
                          <div class="flex items-center gap-1.5">
                            <Calendar size={14} />
                            <span>{api.formatRelativeTime(activity.timestamp)}</span>
                          </div>
                          <Show when={activity.user_email}>
                            <div class="flex items-center gap-1.5">
                              <span class="h-1 w-1 rounded-full bg-slate-800" />
                              <Users size={14} />
                              <span class="text-indigo-400/80">{activity.user_email}</span>
                            </div>
                          </Show>
                        </div>
                      </div>
                      <Show when={activity.user_id}>
                        <button
                          onClick={() => props.onUserClick(activity.user_id)}
                          class="shrink-0 rounded-xl bg-white/[0.03] border border-white/5 px-5 py-2.5 text-xs font-black text-slate-400 opacity-0 transition-all hover:bg-white/10 hover:text-white group-hover:opacity-100"
                        >
                          Trace Entity →
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

      <Show when={activeTab() === 'analytics'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500 pb-20">
          <SmartInsights target="admin" />
          
          <div class="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-5">
            {[
              { label: 'DAU', value: props.adminAnalytics?.dau, sub: 'Daily Active', color: 'text-cyan-400', icon: <Users size={18} /> },
              { label: 'WAU', value: props.adminAnalytics?.wau, sub: 'Weekly Active', color: 'text-blue-400', icon: <Calendar size={18} /> },
              { label: 'MAU', value: props.adminAnalytics?.mau, sub: 'Monthly Active', color: 'text-indigo-400', icon: <CalendarDays size={18} /> },
              { label: 'Retention', value: `${props.adminAnalytics?.retention_rate || 0}%`, sub: 'W-o-W Rate', color: 'text-emerald-400', icon: <RefreshCw size={18} /> },
              { label: 'Events', value: props.adminAnalytics?.events_today, sub: 'Today (24h)', color: 'text-purple-400', icon: <Zap size={18} /> },
            ].map(stat => (
              <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-6 shadow-xl transition-all hover:border-white/10">
                <div class="mb-4 flex items-center justify-between">
                  <div class={`flex h-10 w-10 items-center justify-center rounded-xl bg-white/[0.03] ${stat.color}`}>
                    {stat.icon}
                  </div>
                  <span class="text-[10px] font-black uppercase tracking-widest text-slate-600">{stat.label}</span>
                </div>
                <div class="text-2xl font-black text-white">{stat.value?.toLocaleString() || '0'}</div>
                <div class="mt-1 text-[10px] font-bold uppercase tracking-tight text-slate-500 opacity-60">{stat.sub}</div>
              </div>
            ))}
          </div>

          <div class="grid gap-6 lg:grid-cols-2">
            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <h3 class="mb-8 text-xl font-black text-white tracking-tight uppercase tracking-widest">Most Executed Operations</h3>
              <div class="space-y-6">
                <For each={props.adminAnalytics?.commands_by_type || []}>
                  {(cmd) => {
                    const maxCount = props.adminAnalytics?.commands_by_type?.[0]?.count || 1;
                    const percentage = (cmd.count / maxCount) * 100;
                    return (
                      <div class="group">
                        <div class="mb-2 flex items-center justify-between">
                          <span class="text-sm font-bold text-slate-300 group-hover:text-white transition-colors uppercase tracking-tight">{cmd.command}</span>
                          <span class="text-xs font-black text-indigo-400">{cmd.count.toLocaleString()} ops</span>
                        </div>
                        <div class="h-2.5 overflow-hidden rounded-full bg-white/[0.03] p-0.5">
                          <div
                            class="h-full rounded-full bg-gradient-to-r from-cyan-500 to-indigo-500 transition-all duration-1000 shadow-[0_0_12px_rgba(99,102,241,0.4)]"
                            style={{ width: `${percentage}%` }}
                          />
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            </div>

            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <h3 class="mb-8 text-xl font-black text-white tracking-tight uppercase tracking-widest">High-Utility Features</h3>
              <div class="space-y-6">
                <For each={props.adminAnalytics?.features_by_usage || []}>
                  {(feature) => {
                    const maxCount = props.adminAnalytics?.features_by_usage?.[0]?.count || 1;
                    const percentage = (feature.count / maxCount) * 100;
                    return (
                      <div class="group">
                        <div class="mb-2 flex items-center justify-between">
                          <span class="text-sm font-bold text-slate-300 group-hover:text-white transition-colors uppercase tracking-tight">{feature.feature}</span>
                          <span class="text-xs font-black text-purple-400">{feature.count.toLocaleString()} hits</span>
                        </div>
                        <div class="h-2.5 overflow-hidden rounded-full bg-white/[0.03] p-0.5">
                          <div
                            class="h-full rounded-full bg-gradient-to-r from-purple-500 to-pink-500 transition-all duration-1000 shadow-[0_0_12px_rgba(236,72,153,0.4)]"
                            style={{ width: `${percentage}%` }}
                          />
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            </div>
          </div>

          <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
            <div class="mb-8 flex items-center justify-between">
              <div>
                <h3 class="text-2xl font-black text-white tracking-tight uppercase">Activity Saturation</h3>
                <p class="text-sm font-medium text-slate-500">Real-time daily active user fluctuations.</p>
              </div>
              <div class="flex items-center gap-4">
                <div class="flex items-center gap-2">
                  <div class="h-2 w-2 rounded-full bg-blue-500" />
                  <span class="text-[10px] font-black uppercase text-slate-500">Current Load</span>
                </div>
              </div>
            </div>
            <div class="h-80 w-full">
              <BarChart
                data={props.adminAnalytics?.dau_trend?.map(d => ({
                  label: new Date(d.date).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }),
                  value: d.active_users
                })) || []}
                height={320}
                gradient="indigo"
                animated
                showLabels
              />
            </div>
          </div>

          <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
            <div class="mb-10 flex items-center justify-between">
              <div>
                <h3 class="text-2xl font-black text-white tracking-tight uppercase">Economic Impact</h3>
                <p class="text-sm font-medium text-slate-500 text-opacity-80">Cumulative dev-hours reclaimed via OMG optimization.</p>
              </div>
              <div class="flex items-center gap-2 rounded-full bg-emerald-500/10 px-4 py-2 ring-1 ring-emerald-500/20">
                <TrendingUp size={14} class="text-emerald-400" />
                <span class="text-[10px] font-black uppercase tracking-widest text-emerald-400">+12% Gain MoM</span>
              </div>
            </div>
            <div class="h-80 w-full">
              <BarChart
                data={props.adminAnalytics?.time_saved?.trend?.map((d: any) => ({
                  label: new Date(d.date).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }),
                  value: Math.round(d.time_saved / 3600000)
                })) || []}
                height={320}
                gradient="emerald"
                animated
                showLabels
                tooltipFormatter={(v) => `${v} hours`}
              />
            </div>
          </div>

          <div class="grid gap-6 lg:grid-cols-2">
            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <h3 class="mb-8 text-xl font-black text-white tracking-tight uppercase tracking-widest">Global Fleet Versions</h3>
              <div class="space-y-4">
                <For each={props.adminAnalytics?.version_distribution || []}>
                  {(ver) => {
                    const total = (props.adminAnalytics?.version_distribution || []).reduce((s, v) => s + v.count, 0) || 1;
                    const percentage = (ver.count / total) * 100;
                    return (
                      <div class="group flex items-center justify-between rounded-2xl bg-white/[0.02] p-5 transition-all hover:bg-white/[0.05]">
                        <div class="flex items-center gap-4">
                          <span class="rounded-lg bg-emerald-500/10 border border-emerald-500/20 px-3 py-1.5 font-mono text-xs font-black text-emerald-400 shadow-inner group-hover:scale-110 transition-transform">v{ver.version}</span>
                          <span class="text-sm font-bold text-slate-400 uppercase tracking-tight">{ver.count.toLocaleString()} entities</span>
                        </div>
                        <span class="text-sm font-black text-white">{percentage.toFixed(1)}%</span>
                      </div>
                    );
                  }}
                </For>
              </div>
            </div>

            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <h3 class="mb-8 text-xl font-black text-white tracking-tight uppercase tracking-widest">Platform Saturation</h3>
              <div class="space-y-4">
                <For each={props.adminAnalytics?.platform_distribution || []}>
                  {(plat) => {
                    const total = (props.adminAnalytics?.platform_distribution || []).reduce((s, p) => s + p.count, 0) || 1;
                    const percentage = (plat.count / total) * 100;
                    const platformIcon = plat.platform.includes('linux') ? '🐧' : plat.platform.includes('darwin') ? '🍎' : '💻';
                    return (
                      <div class="group flex items-center justify-between rounded-2xl bg-white/[0.02] p-5 transition-all hover:bg-white/[0.05]">
                        <div class="flex items-center gap-4">
                          <span class="text-3xl group-hover:scale-125 transition-transform duration-500">{platformIcon}</span>
                          <div>
                            <span class="block text-sm font-black text-white uppercase tracking-widest">{plat.platform}</span>
                            <span class="text-[10px] font-bold text-slate-600 uppercase">{plat.count.toLocaleString()} Nodes</span>
                          </div>
                        </div>
                        <span class="text-lg font-black text-white">{percentage.toFixed(1)}%</span>
                      </div>
                    );
                  }}
                </For>
              </div>
            </div>
          </div>

          <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl overflow-hidden">
            <h3 class="mb-8 text-xl font-black text-white tracking-tight uppercase tracking-widest">Entity Performance Ledger</h3>
            <div class="overflow-x-auto">
              <table class="w-full text-left">
                <thead>
                  <tr class="border-b border-white/5">
                    <th class="px-6 py-5 text-[10px] font-black uppercase tracking-widest text-slate-500">Operation / Protocol</th>
                    <th class="px-6 py-5 text-right text-[10px] font-black uppercase tracking-widest text-slate-500">P50</th>
                    <th class="px-6 py-5 text-right text-[10px] font-black uppercase tracking-widest text-slate-500">P95</th>
                    <th class="px-6 py-5 text-right text-[10px] font-black uppercase tracking-widest text-slate-500">P99</th>
                    <th class="px-6 py-5 text-right text-[10px] font-black uppercase tracking-widest text-slate-500">Ops/Sec</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-white/[0.02]">
                  <For each={Object.entries(props.adminAnalytics?.performance || {})}>
                    {([op, stats]) => (
                      <tr class="group transition-colors hover:bg-white/[0.02]">
                        <td class="px-6 py-5">
                          <span class="font-bold text-white tracking-tight group-hover:text-indigo-400 transition-colors">{op}</span>
                        </td>
                        <td class="px-6 py-5 text-right">
                          <span class="rounded-lg bg-emerald-500/10 px-2.5 py-1 text-xs font-black text-emerald-400 border border-emerald-500/20">{stats.p50}ms</span>
                        </td>
                        <td class="px-6 py-5 text-right">
                          <span class="rounded-lg bg-amber-500/10 px-2.5 py-1 text-xs font-black text-amber-400 border border-amber-500/20">{stats.p95}ms</span>
                        </td>
                        <td class="px-6 py-5 text-right">
                          <span class="rounded-lg bg-rose-500/10 px-2.5 py-1 text-xs font-black text-rose-400 border border-rose-500/20">{stats.p99}ms</span>
                        </td>
                        <td class="px-6 py-5 text-right">
                          <span class="text-sm font-black text-slate-400">{(stats.count / 604800).toFixed(2)}</span>
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </Show>
    </div>
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

          <div class="grid gap-6 lg:grid-cols-2">
            <div class="rounded-[2.5rem] border border-rose-500/20 bg-rose-500/[0.02] p-10 shadow-2xl">
              <h3 class="mb-8 text-xl font-black text-rose-400 tracking-tight uppercase tracking-widest">Global Fault Ledger</h3>
              <div class="space-y-4">
                <For each={props.adminAnalytics?.top_errors?.slice(0, 5) || []}>
                  {(err: any) => (
                    <div class="flex items-center justify-between rounded-2xl bg-black/40 border border-white/5 p-5 transition-all hover:border-rose-500/30 group">
                      <div class="max-w-[70%] truncate font-mono text-xs font-bold text-slate-400 group-hover:text-rose-300 transition-colors">{err.error_message}</div>
                      <div class="flex items-center gap-3">
                        <span class="text-[10px] font-black text-slate-600 uppercase">Frequency</span>
                        <span class="rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-1 text-xs font-black text-rose-400 shadow-inner">{err.occurrences}x</span>
                      </div>
                    </div>
                  )}
                </For>
                <Show when={!props.adminAnalytics?.top_errors?.length}>
                  <div class="flex flex-col items-center justify-center py-10 text-center rounded-2xl bg-emerald-500/[0.02] border border-emerald-500/10">
                    <CheckCircle size={32} class="text-emerald-500/40 mb-3" />
                    <p class="text-sm font-bold text-emerald-500">Nominal Operation</p>
                  </div>
                </Show>
              </div>
            </div>

            <div class="rounded-[2.5rem] border border-emerald-500/20 bg-emerald-500/[0.02] p-10 shadow-2xl">
              <h3 class="mb-8 text-xl font-black text-emerald-400 tracking-tight uppercase tracking-widest">Trending Volume</h3>
              <div class="space-y-4">
                <For each={props.adminAnalytics?.top_packages?.slice(0, 5) || []}>
                  {(pkg: any) => (
                    <div class="flex items-center justify-between rounded-2xl bg-black/40 border border-white/5 p-5 transition-all hover:border-emerald-500/30 group">
                      <div class="font-bold text-white group-hover:text-emerald-400 transition-colors">{pkg.package_name}</div>
                      <div class="flex items-center gap-6 text-xs">
                        <div class="text-right">
                          <div class="font-black text-emerald-400">{pkg.install_count}</div>
                          <div class="text-[9px] font-bold text-slate-600 uppercase">Installs</div>
                        </div>
                        <div class="text-right">
                          <div class="font-black text-indigo-400">{pkg.search_count}</div>
                          <div class="text-[9px] font-bold text-slate-600 uppercase">Searches</div>
                        </div>
                      </div>
                    </div>
                  )}
                </For>
                <Show when={!props.adminAnalytics?.top_packages?.length}>
                  <p class="py-10 text-center text-sm font-bold text-slate-600 italic uppercase tracking-widest">No Telemetry Streamed</p>
                </Show>
              </div>
            </div>
          </div>

          <Show when={props.adminAnalytics?.geo_distribution?.length}>
            <div class="rounded-[2.5rem] border border-cyan-500/20 bg-cyan-500/[0.02] p-10 shadow-2xl">
              <div class="mb-8 flex items-center justify-between">
                <div>
                  <h3 class="text-2xl font-black text-cyan-400 tracking-tight uppercase">Global Geographic Density</h3>
                  <p class="text-sm font-medium text-slate-500">Distribution of machine entities by regional timezone.</p>
                </div>
                <Globe size={32} class="text-cyan-500/20" />
              </div>
              <div class="grid grid-cols-2 gap-4 md:grid-cols-4">
                <For each={props.adminAnalytics?.geo_distribution?.slice(0, 8) || []}>
                  {(geo: any) => {
                    const total = (props.adminAnalytics?.geo_distribution || []).reduce((s: number, g: any) => s + (g.users || g.count || 0), 0) || 1;
                    const val = geo.users || geo.count || 0;
                    const pct = Math.round((val / total) * 100);
                    return (
                      <div class="rounded-3xl bg-black/40 border border-white/5 p-6 transition-all hover:bg-white/[0.02] hover:border-cyan-500/30 group">
                        <div class="truncate text-xs font-black text-slate-500 group-hover:text-cyan-400 transition-colors uppercase tracking-widest">{geo.timezone || geo.dimension || 'Unknown'}</div>
                        <div class="mt-4 flex items-end justify-between">
                          <div class="text-2xl font-black text-white">{val.toLocaleString()}</div>
                          <div class="mb-1 rounded bg-cyan-500/10 px-1.5 py-0.5 text-[10px] font-black text-cyan-400 border border-cyan-500/20">{pct}%</div>
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

