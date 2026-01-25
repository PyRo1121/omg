import { Component, For, Show, createSignal, createMemo, Switch, Match } from 'solid-js';
import * as api from '../../lib/api';
import { 
  useTeamData, 
  useTeamPolicies, 
  useNotificationSettings, 
  useTeamAuditLogs,
  useRevokeMachine,
  useCreatePolicy,
  useDeletePolicy,
  useUpdateThreshold,
  useUpdateNotifications
} from '../../lib/api-hooks';
import { StatCard } from './analytics/StatCard';
import { RoiChart } from './analytics/RoiChart';
import { SecurityScore } from './analytics/SecurityScore';
import { StatusBadge } from '../ui/Badge';
import { DonutChart, LiveIndicator } from '../ui/Chart';
import { CardSkeleton } from '../ui/Skeleton';
import { SmartInsights } from './SmartInsights';
import {
  Users,
  BarChart3,
  Settings,
  DollarSign,
  Shield,
  Zap,
  Clock,
  Activity,
  Lock,
  RefreshCw,
  Crown,
  Plus,
  Trash2,
  Bell
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
  const [view, setView] = createSignal<string>(props.initialView || 'overview');
  const [sortBy, setSortBy] = createSignal<'commands' | 'recent' | 'name'>('commands');
  const [filterActive, setFilterActive] = createSignal<boolean | null>(null);
  const [alertThreshold, setAlertThreshold] = createSignal(100);
  
  // Forms state
  const [newPolicyScope, setNewPolicyScope] = createSignal('runtime');
  const [newPolicyRule, setNewPolicyRule] = createSignal('');
  const [newPolicyValue, setNewPolicyValue] = createSignal('');

  // TanStack Queries
  const teamQuery = useTeamData();
  const policiesQuery = useTeamPolicies();
  const notificationsQuery = useNotificationSettings();
  const auditLogsQuery = useTeamAuditLogs({ limit: 20 });

  // Mutations
  const revokeMutation = useRevokeMachine();
  const createPolicyMutation = useCreatePolicy();
  const deletePolicyMutation = useDeletePolicy();
  const updateThresholdMutation = useUpdateThreshold();
  const updateNotifMutation = useUpdateNotifications();

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
      critical: members.filter(m => m.omg_version && !m.omg_version.startsWith('1.')).length,
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

  const sortedMembers = createMemo(() => {
    let members = [...(teamData()?.members || [])];
    
    if (filterActive() !== null) {
      members = members.filter(m => m.is_active === filterActive());
    }

    return members.sort((a, b) => {
      if (sortBy() === 'commands') return (b.total_commands || 0) - (a.total_commands || 0);
      const timeB = b.last_seen_at ? new Date(b.last_seen_at).getTime() : 0;
      const timeA = a.last_seen_at ? new Date(a.last_seen_at).getTime() : 0;
      if (sortBy() === 'recent') return timeB - timeA;
      return (a.user_email || '').localeCompare(b.user_email || '');
    });
  });

  const seatUsage = () => {
    const used = teamData()?.totals?.active_machines || 0;
    const max = teamData()?.license?.max_seats || 30;
    return [
      { label: 'Occupied', value: used, color: '#f59e0b' },
      { label: 'Vacant', value: Math.max(0, max - used), color: '#1e293b' },
    ];
  };

  const handleCreatePolicy = async (e: Event) => {
    e.preventDefault();
    if (!newPolicyRule() || !newPolicyValue()) return;
    await createPolicyMutation.mutateAsync({
      scope: newPolicyScope(),
      rule: newPolicyRule(),
      value: newPolicyValue(),
      enforced: true
    });
    setNewPolicyRule('');
    setNewPolicyValue('');
  };

  if (teamData()?.license?.tier === 'free' || teamData()?.license?.tier === 'pro') {
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

      <div class="flex items-center gap-1 overflow-x-auto no-scrollbar rounded-[1.5rem] border border-white/5 bg-white/[0.02] p-1.5 backdrop-blur-xl">
        <For each={[
          { id: 'overview', label: 'Value & ROI', Icon: BarChart3, color: 'text-indigo-400' },
          { id: 'members', label: 'Fleet & Members', Icon: Users, color: 'text-emerald-400' },
          { id: 'security', label: 'Compliance', Icon: Shield, color: 'text-rose-400' },
          { id: 'activity', label: 'Execution', Icon: Zap, color: 'text-amber-400' },
          { id: 'settings', label: 'Control', Icon: Settings, color: 'text-slate-400' },
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

      <Switch>
        <Match when={teamQuery.isLoading}>
          <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
            <CardSkeleton /> <CardSkeleton /> <CardSkeleton /> <CardSkeleton />
          </div>
        </Match>

        <Match when={teamQuery.isSuccess}>
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
                    value={`$${(productivityImpact().developer_value ?? 0).toLocaleString()}`}
                    icon={<DollarSign size={20} />}
                    description="Economic value generated from automation gains."
                    class="border-indigo-500/20 bg-indigo-500/[0.03]"
                    trend={{ value: 12.4, isUp: true }}
                  />
                  <StatCard
                    title="Execution Volume"
                    value={(teamData()?.totals?.total_commands ?? 0).toLocaleString()}
                    icon={<Zap size={22} />}
                    description="Total operations executed globally."
                    class="border-amber-500/20 bg-amber-500/[0.03]"
                  />
                  <div class="relative overflow-hidden rounded-[2rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                    <div class="mb-4 flex items-center justify-between">
                      <h3 class="text-sm font-bold text-white uppercase tracking-widest">Seat Utilization</h3>
                      <span class="text-[10px] font-black text-slate-500">{seatUsage()?.[0]?.value || 0} / {teamData()?.license?.max_seats || 30}</span>
                    </div>
                    <div class="flex items-center justify-center py-2">
                      <DonutChart data={seatUsage()} size={140} thickness={16} centerLabel="Seats" centerValue={seatUsage()?.[0]?.value || 0} />
                    </div>
                  </div>
                </div>

                <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
                  <div class="lg:col-span-2">
                    <RoiChart data={productivityImpact().daily_trend} peakVelocity={84} />
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
               <div class="space-y-6">
                 <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between mb-2">
                    <div class="flex rounded-2xl bg-white/[0.03] p-1.5 border border-white/5">
                      <button onClick={() => setFilterActive(null)} class={`px-4 py-2 rounded-xl text-xs font-bold transition-all ${filterActive() === null ? 'bg-white text-black' : 'text-slate-400'}`}>All</button>
                      <button onClick={() => setFilterActive(true)} class={`px-4 py-2 rounded-xl text-xs font-bold transition-all ${filterActive() === true ? 'bg-emerald-500 text-white' : 'text-slate-400'}`}>Online</button>
                    </div>
                    <select value={sortBy()} onChange={e => setSortBy(e.currentTarget.value as any)} class="bg-white/5 border border-white/10 rounded-xl px-4 py-2 text-sm font-bold text-white outline-none">
                      <option value="commands">Ops Volume</option>
                      <option value="recent">Last Seen</option>
                      <option value="name">Email</option>
                    </select>
                 </div>
                 
                 <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                    <For each={sortedMembers()}>
                      {member => (
                        <div class="p-6 rounded-3xl bg-[#0d0d0e] border border-white/5 group hover:border-white/10 transition-all">
                          <div class="flex justify-between items-start mb-4">
                            <div class="h-10 w-10 rounded-xl bg-gradient-to-br from-slate-700 to-slate-800 flex items-center justify-center font-bold text-white shadow-lg">
                              {(member.user_email?.[0] || 'U').toUpperCase()}
                            </div>
                            <div class="flex flex-col items-end">
                              <StatusBadge status={member.is_active ? 'active' : 'inactive'} />
                              <span class="text-[9px] font-mono text-slate-500 mt-1 uppercase tracking-widest">{api.formatRelativeTime(member.last_seen_at)}</span>
                            </div>
                          </div>
                          <div class="text-sm font-bold text-white truncate">{member.user_email || 'Unknown User'}</div>
                          <div class="text-xs text-slate-500 mt-1 truncate">{member.hostname || 'Unknown Host'} • {member.os || 'Unknown OS'}</div>
                          <div class="mt-6 pt-4 border-t border-white/5 flex justify-between items-center">
                            <div>
                               <div class="text-[9px] font-black uppercase tracking-widest text-slate-500">Throughput</div>
                               <div class="text-lg font-black text-indigo-400">{(member.total_commands ?? 0).toLocaleString()}</div>
                            </div>
                            <button 
                              onClick={() => { if(confirm('Revoke node access?')) revokeMutation.mutate(member.machine_id) }}
                              disabled={revokeMutation.isPending}
                              class="p-2 rounded-lg hover:bg-rose-500/10 text-slate-600 hover:text-rose-500 transition-all"
                            >
                              <Trash2 size={16} />
                            </button>
                          </div>
                        </div>
                      )}
                    </For>
                 </div>
               </div>
            </Match>

            <Match when={view() === 'settings'}>
              <div class="grid grid-cols-1 lg:grid-cols-3 gap-8">
                <div class="lg:col-span-2 space-y-8">
                  {/* Policies Section */}
                  <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                    <div class="flex items-center justify-between mb-8">
                      <h3 class="text-xl font-bold text-white uppercase tracking-widest">Fleet Policies</h3>
                      <Shield size={20} class="text-indigo-400" />
                    </div>
                    
                    <form onSubmit={handleCreatePolicy} class="grid grid-cols-1 sm:grid-cols-4 gap-3 mb-8">
                      <select value={newPolicyScope()} onChange={e => setNewPolicyScope(e.currentTarget.value)} class="sm:col-span-1 bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-sm font-bold text-white outline-none">
                        <option value="runtime">Runtime</option>
                        <option value="package">Package</option>
                        <option value="security">Security</option>
                      </select>
                      <input value={newPolicyRule()} onInput={e => setNewPolicyRule(e.currentTarget.value)} placeholder="Rule (e.g. min-version)" class="sm:col-span-1 bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-sm font-medium text-white outline-none" />
                      <input value={newPolicyValue()} onInput={e => setNewPolicyValue(e.currentTarget.value)} placeholder="Value (e.g. 20.x)" class="sm:col-span-1 bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-sm font-medium text-white outline-none" />
                      <button type="submit" disabled={createPolicyMutation.isPending} class="rounded-xl bg-white text-black font-bold text-sm hover:scale-[1.02] transition-all flex items-center justify-center gap-2">
                        <Plus size={16} /> Add
                      </button>
                    </form>

                    <div class="space-y-4">
                      <For each={policiesQuery.data?.policies || []}>
                        {policy => (
                          <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02] border border-white/5 group">
                            <div class="flex items-center gap-4">
                              <div class="h-8 w-8 rounded-lg bg-indigo-500/10 flex items-center justify-center text-indigo-400">
                                <Lock size={16} />
                              </div>
                              <div>
                                <div class="text-sm font-bold text-white uppercase tracking-wider">{policy.scope} Policy</div>
                                <div class="text-xs text-slate-500 font-medium">{policy.rule}: <span class="text-slate-300">{policy.value}</span></div>
                              </div>
                            </div>
                            <div class="flex items-center gap-4">
                               <StatusBadge status={policy.enforced ? 'enforced' : 'audit'} />
                               <button onClick={() => deletePolicyMutation.mutate(policy.id)} class="p-2 opacity-0 group-hover:opacity-100 transition-opacity text-slate-600 hover:text-rose-500">
                                 <Trash2 size={16} />
                               </button>
                            </div>
                          </div>
                        )}
                      </For>
                    </div>
                  </div>

                  {/* Audit Logs */}
                  <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                    <h3 class="text-xl font-bold text-white uppercase tracking-widest mb-6">Security Audit</h3>
                    <div class="space-y-4">
                      <For each={auditLogsQuery.data?.logs || []}>
                        {log => (
                          <div class="flex items-center gap-4 text-sm py-2 border-b border-white/5">
                            <div class="text-slate-500 font-mono text-[10px] w-32">{log.created_at ? new Date(log.created_at).toLocaleString() : 'N/A'}</div>
                            <div class="text-indigo-400 font-bold w-32 uppercase tracking-tighter">{log.action || 'ACTION'}</div>
                            <div class="text-slate-300 flex-1 truncate">{log.resource_type || 'System'} {log.resource_id || ''}</div>
                          </div>
                        )}
                      </For>
                    </div>
                  </div>
                </div>

                <div class="space-y-8">
                  {/* Thresholds */}
                  <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                    <h3 class="text-lg font-bold text-white uppercase tracking-widest mb-6">Activity Monitor</h3>
                    <div class="space-y-6">
                      <div>
                        <div class="flex justify-between mb-4">
                          <span class="text-xs font-bold text-slate-400">Node Inactivity (Ops)</span>
                          <span class="text-xs font-black text-indigo-400">{alertThreshold()}</span>
                        </div>
                        <input type="range" min="10" max="1000" step="10" value={alertThreshold()} onInput={e => setAlertThreshold(parseInt(e.currentTarget.value))} class="w-full" />
                        <button 
                          onClick={() => updateThresholdMutation.mutate({ type: 'low_activity', value: alertThreshold() })}
                          disabled={updateThresholdMutation.isPending}
                          class="mt-4 w-full py-2 rounded-xl bg-white/5 border border-white/10 text-xs font-bold text-white hover:bg-white/10 transition-all"
                        >
                          Save Threshold
                        </button>
                      </div>
                    </div>
                  </div>

                  {/* Notifications */}
                  <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                    <div class="flex items-center gap-3 mb-6">
                      <Bell size={18} class="text-amber-400" />
                      <h3 class="text-lg font-bold text-white uppercase tracking-widest">Alerts</h3>
                    </div>
                    <div class="space-y-4">
                      <For each={notificationsQuery.data?.settings || []}>
                        {notif => (
                          <div class="flex items-center justify-between">
                            <span class="text-xs font-bold text-slate-400 uppercase tracking-widest">{notif.type.replace('_', ' ')}</span>
                            <button 
                              onClick={() => {
                                const updated = (notificationsQuery.data?.settings || []).map(n => n.type === notif.type ? { ...n, enabled: !n.enabled } : n);
                                updateNotifMutation.mutate(updated);
                              }}
                              class={`w-10 h-5 rounded-full relative transition-all ${notif.enabled ? 'bg-indigo-500' : 'bg-slate-700'}`}
                            >
                              <div class={`absolute top-1 w-3 h-3 rounded-full bg-white transition-all ${notif.enabled ? 'left-6' : 'left-1'}`} />
                            </button>
                          </div>
                        )}
                      </For>
                    </div>
                  </div>
                </div>
              </div>
            </Match>
          </Switch>
        </Match>
      </Switch>
    </div>
  );
};

export default TeamAnalytics;
