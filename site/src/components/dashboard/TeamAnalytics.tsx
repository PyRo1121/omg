import { Component, For, Show, createSignal, createMemo, createEffect } from 'solid-js';
import * as api from '../../lib/api';
import { 
  useTeamData, 
  useTeamPolicies, 
  useNotificationSettings, 
  useTeamAuditLogs 
} from '../../lib/api-hooks';
import { StatCard } from './analytics/StatCard';
import { RoiChart } from './analytics/RoiChart';
import { SecurityScore } from './analytics/SecurityScore';
import { MetricCard } from '../ui/Card';
import { StatusBadge, TierBadge } from '../ui/Badge';
import { BarChart, DonutChart, LiveIndicator, ActivityHeatmap } from '../ui/Chart';
import { Dialog } from '../ui/Dialog';
import { CardSkeleton } from '../ui/Skeleton';
import { SmartInsights } from './SmartInsights';
import {
  Users,
  BarChart3,
  TrendingUp,
  Settings,
  AlertTriangle,
  FileText,
  Lightbulb,
  DollarSign,
  Shield,
  Zap,
  Clock,
  CheckCircle,
  Activity,
  Cpu,
  Globe,
  Lock,
  Target,
  Package,
  RefreshCw,
  Crown
} from '../ui/Icons';

interface TeamAnalyticsProps {
  teamData: api.TeamData | null;
  licenseKey: string;
  onRevoke: (machineId: string) => void;
  onRefresh: () => void;
  loading?: boolean;
  initialView?: 'overview' | 'members' | 'security' | 'activity' | 'insights' | 'settings';
}

export const TeamAnalytics: Component<TeamAnalyticsProps> = props => {
  const [view, setView] = createSignal<'overview' | 'members' | 'security' | 'activity' | 'insights' | 'settings'>(props.initialView || 'overview');
  const [sortBy, setSortBy] = createSignal<'commands' | 'recent' | 'name'>('commands');
  const [filterActive, setFilterActive] = createSignal<boolean | null>(null);
  const [alertThreshold, setAlertThreshold] = createSignal(100);

  // TanStack Queries
  const teamQuery = useTeamData();
  const policiesQuery = useTeamPolicies();
  const notificationsQuery = useNotificationSettings();
  const auditLogsQuery = useTeamAuditLogs({ limit: 20 });

  const teamData = () => teamQuery.data || props.teamData;
  const isRefreshing = () => teamQuery.isFetching;

  // Memos for derived data
  const securityMetrics = createMemo(() => {
    const data = teamData();
    const members = data?.members || [];
    const total = members.length || 1;
    const compliant = members.filter(m => m.omg_version && m.omg_version.startsWith('1.')).length;
    
    return {
      compliance_score: Math.round((compliant / total) * 100),
      critical: 0,
      high: 0,
      medium: 0,
      low: 0,
    };
  });

  const productivityImpact = createMemo(() => {
    const data = teamData();
    const timeSavedMs = data?.totals?.total_time_saved_ms || 0;
    const hours = Math.floor(timeSavedMs / 3600000);
    const valueUsd = data?.totals?.total_value_usd || (hours * 85); 
    
    const daily = data?.daily_usage || [];
    const trend = daily.map(d => d.commands_run);
    
    return {
      hours_reclaimed: hours,
      developer_value: valueUsd,
      daily_trend: trend.length > 0 ? trend : [0]
    };
  });

  const teamProductivityScore = () => 84; // Placeholder or calculate from data

  const seatUsage = () => {
    const used = teamData()?.totals?.active_machines || 0;
    const max = teamData()?.license?.max_seats || 30;
    return [
      { label: 'Occupied', value: used, color: '#f59e0b' },
      { label: 'Vacant', value: Math.max(0, max - used), color: '#1e293b' },
    ];
  };

  const activityByDay = () => {
    const daily = teamData()?.daily_usage || [];
    const last14Days = Array.from({ length: 14 }, (_, i) => {
      const date = new Date();
      date.setDate(date.getDate() - (13 - i));
      return date.toISOString().split('T')[0];
    });

    return last14Days.map(date => {
      const dayData = daily.filter(d => d.date === date);
      const total = dayData.reduce((sum, d) => sum + d.commands_run, 0);
      return {
        label: new Date(date).toLocaleDateString('en-US', { weekday: 'short' }).slice(0, 2),
        value: total,
      };
    });
  };

  // Tier check
  const isFreeOrPro = () => {
    const tier = teamData()?.license?.tier;
    return tier === 'free' || tier === 'pro';
  };

  if (isFreeOrPro()) {
    return (
      <div class="flex flex-col items-center justify-center py-20 text-center animate-in fade-in duration-700">
        <div class="mb-8 flex h-24 w-24 items-center justify-center rounded-3xl bg-gradient-to-br from-indigo-500 to-purple-600 shadow-2xl shadow-indigo-500/20">
          <Crown size={48} class="text-white" />
        </div>
        <h2 class="mb-4 text-4xl font-black text-white tracking-tight">Unlock Team Intelligence</h2>
        <p class="mb-10 max-w-lg text-lg font-medium text-slate-400">
          Gain visibility into your entire fleet. Manage runtimes, enforce security policies, and track productivity across your organization.
        </p>
        <button
          onClick={() => window.open('https://pyro1121.com/pricing', '_blank')}
          class="rounded-2xl bg-white px-10 py-4 font-black text-black transition-all hover:scale-105"
        >
          Upgrade to Team
        </button>
      </div>
    );
  }

  return (
    <div class="space-y-8 pb-20">
      {/* Header */}
      <div class="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
        <div class="flex items-start gap-5">
          <div class="relative flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 via-blue-600 to-indigo-700 shadow-2xl shadow-indigo-500/20">
            <Users size={32} class="text-white drop-shadow-lg" />
          </div>
          <div>
            <div class="flex items-center gap-3">
              <h1 class="text-4xl font-black tracking-tight text-white">Team Intelligence</h1>
              <div class="mt-1 flex items-center gap-2 rounded-full bg-indigo-500/10 px-3 py-1 text-[10px] font-bold uppercase tracking-widest text-indigo-400 ring-1 ring-indigo-500/20">
                Enterprise Active
              </div>
            </div>
            <p class="mt-2 text-slate-400 font-medium">
              Aggregate value, fleet health, and developer productivity insights.
            </p>
          </div>
        </div>
        
        <div class="flex flex-wrap items-center gap-3">
          <button
            onClick={() => teamQuery.refetch()}
            disabled={isRefreshing()}
            class="group flex items-center gap-2 rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-3 text-sm font-bold text-white transition-all hover:bg-white/[0.08] disabled:opacity-50"
          >
            <RefreshCw size={16} class={isRefreshing() ? 'animate-spin' : ''} />
            Sync
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div class="flex items-center gap-1 overflow-x-auto no-scrollbar rounded-[1.5rem] border border-white/5 bg-white/[0.02] p-1.5 backdrop-blur-xl">
        <For each={[
          { id: 'overview' as const, label: 'Value & ROI', Icon: BarChart3, color: 'text-indigo-400' },
          { id: 'members' as const, label: 'Fleet & Members', Icon: Users, color: 'text-emerald-400' },
          { id: 'security' as const, label: 'Compliance', Icon: Shield, color: 'text-rose-400' },
          { id: 'activity' as const, label: 'Execution', Icon: Zap, color: 'text-amber-400' },
          { id: 'settings' as const, label: 'Control', Icon: Settings, color: 'text-slate-400' },
        ]}>{tab => (
          <button
            onClick={() => setView(tab.id)}
            class={`relative flex flex-1 items-center justify-center gap-3 rounded-[1.25rem] py-3.5 text-sm font-bold transition-all duration-300 ${
              view() === tab.id
                ? 'bg-white text-black shadow-lg scale-[1.02]'
                : 'text-slate-400 hover:text-white hover:bg-white/5'
            }`}
          >
            <tab.Icon size={18} class={view() === tab.id ? 'text-black' : tab.color} />
            <span class="hidden md:inline">{tab.label}</span>
          </button>
        )}</For>
      </div>

      <Show when={teamQuery.isLoading}>
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={teamQuery.isSuccess}>
        <Switch>
          <Match when={view() === 'overview'}>
            <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
              <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
                <StatCard 
                  title="Efficiency Reclaimed"
                  value={`${productivityImpact().hours_reclaimed} Hours`}
                  icon={<Clock size={20} />}
                  description="Total developer time saved across the organization."
                  class="border-emerald-500/20 bg-emerald-500/[0.03]"
                />
                
                <StatCard 
                  title="Financial ROI"
                  value={`$${productivityImpact().developer_value.toLocaleString()}`}
                  icon={<DollarSign size={20} />}
                  description="Economic value generated from automation gains."
                  class="border-indigo-500/20 bg-indigo-500/[0.03]"
                  trend={{ value: 12.4, isUp: true }}
                />

                <StatCard 
                  title="Execution Volume"
                  value={(teamData()?.totals?.total_commands || 0).toLocaleString()}
                  icon={<Zap size={22} />}
                  description="Total operations executed globally."
                  class="border-amber-500/20 bg-amber-500/[0.03]"
                />

                <div class="relative overflow-hidden rounded-[2rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                  <div class="mb-4 flex items-center justify-between">
                    <h3 class="text-sm font-bold text-white uppercase tracking-widest">Seat Utilization</h3>
                    <span class="text-[10px] font-black text-slate-500">{seatUsage()[0].value} / {teamData()?.license?.max_seats || 30}</span>
                  </div>
                  <div class="flex items-center justify-center py-2">
                    <DonutChart data={seatUsage()} size={140} thickness={16} centerLabel="Seats" centerValue={seatUsage()[0].value} />
                  </div>
                </div>
              </div>

              <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
                <div class="lg:col-span-2">
                  <RoiChart 
                    data={productivityImpact().daily_trend} 
                    peakVelocity={teamProductivityScore()}
                  />
                </div>
                <SecurityScore 
                  score={securityMetrics().compliance_score}
                  critical={securityMetrics().critical}
                  high={securityMetrics().high}
                  medium={securityMetrics().medium}
                  low={securityMetrics().low}
                />
              </div>

              <SmartInsights target="team" />
            </div>
          </Match>

          <Match when={view() === 'members'}>
             <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10">
                <h3 class="text-2xl font-black text-white mb-6">Team Fleet</h3>
                <p class="text-slate-400">View and manage all active nodes in your organization.</p>
                {/* Simplified member list for now to keep file size reasonable */}
                <div class="mt-8 grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                  <For each={teamData()?.members}>
                    {member => (
                      <div class="p-6 rounded-2xl bg-white/[0.02] border border-white/5">
                        <div class="flex justify-between items-start mb-4">
                          <div class="h-10 w-10 rounded-full bg-slate-800 flex items-center justify-center font-bold text-white">
                            {member.user_email?.[0].toUpperCase()}
                          </div>
                          <StatusBadge status={member.is_active ? 'active' : 'inactive'} />
                        </div>
                        <div class="text-sm font-bold text-white">{member.user_email}</div>
                        <div class="text-xs text-slate-500 mt-1">{member.hostname} • {member.os}</div>
                        <div class="mt-4 flex justify-between items-center text-[10px] font-black uppercase tracking-widest text-slate-500">
                          <span>Total Ops</span>
                          <span class="text-white">{member.total_commands.toLocaleString()}</span>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
             </div>
          </Match>

          <Match when={view() === 'settings'}>
            <div class="space-y-6">
              <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10">
                <h3 class="text-2xl font-black text-white mb-6">Organization Controls</h3>
                <div class="space-y-8">
                  <div>
                    <label class="block text-sm font-bold text-slate-400 uppercase tracking-widest mb-4">Activity Thresholds</label>
                    <div class="flex items-center gap-4">
                      <input 
                        type="range" min="10" max="1000" step="10" 
                        value={alertThreshold()} 
                        onInput={e => setAlertThreshold(parseInt(e.currentTarget.value))}
                        class="flex-1"
                      />
                      <span class="text-xl font-black text-white w-20 text-right">{alertThreshold()}</span>
                    </div>
                  </div>
                  
                  <div class="pt-6 border-t border-white/5">
                    <h4 class="text-lg font-bold text-white mb-4">Security Policies</h4>
                    <For each={policiesQuery.data?.policies || []}>
                      {policy => (
                        <div class="flex items-center justify-between py-3 border-b border-white/5">
                          <div class="flex items-center gap-3">
                            <Lock size={16} class="text-indigo-400" />
                            <span class="text-sm text-slate-300">{policy.rule}: {policy.value}</span>
                          </div>
                          <StatusBadge status={policy.enforced ? 'enforced' : 'audit'} />
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </div>
            </div>
          </Match>
        </Switch>
      </Show>
    </div>
  );
};

// Sub-components used in Match views
import { Switch, Match } from 'solid-js';

export default TeamAnalytics;
