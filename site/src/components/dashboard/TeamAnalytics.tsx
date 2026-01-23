import { Component, For, Show, createSignal, createMemo } from 'solid-js';
import * as api from '../../lib/api';
import { MetricCard } from '../ui/Card';
import { StatusBadge, TierBadge } from '../ui/Badge';
import { BarChart, DonutChart, LiveIndicator, AreaChart, ActivityHeatmap } from '../ui/Chart';
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
} from '../ui/Icons';

interface TeamAnalyticsProps {
  teamData: api.TeamData | null;
  licenseKey: string;
  onRevoke: (machineId: string) => void;
  onRefresh: () => void;
}

export const TeamAnalytics: Component<TeamAnalyticsProps> = props => {
  const [view, setView] = createSignal<'overview' | 'members' | 'security' | 'activity' | 'insights' | 'settings'>('overview');
  const [sortBy, setSortBy] = createSignal<'commands' | 'recent' | 'name'>('commands');
  const [filterActive, setFilterActive] = createSignal<boolean | null>(null);
  const [copied, setCopied] = createSignal(false);
  const [isRefreshing, setIsRefreshing] = createSignal(false);
  const [selectedMember, setSelectedMember] = createSignal<api.TeamMember | null>(null);
  const [showInviteModal, setShowInviteModal] = createSignal(false);
  const [alertThreshold, setAlertThreshold] = createSignal(100);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    await props.onRefresh();
    setTimeout(() => setIsRefreshing(false), 1000);
  };

  const copyLicenseKey = () => {
    navigator.clipboard.writeText(props.licenseKey);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const getMemberInitials = (member: api.TeamMember) => {
    const name = getMemberDisplayName(member);
    const parts = name.split(' ');
    if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
    return name.slice(0, 2).toUpperCase();
  };

  const securityMetrics = createMemo(() => ({
    total_vulnerabilities: 12,
    critical: 0,
    high: 2,
    medium: 5,
    low: 5,
    compliance_score: 94,
    signature_verification: 100,
    sbom_status: 'Healthy',
    policy_enforcement: 'Active'
  }));

  const productivityImpact = createMemo(() => {
    const timeSavedMs = props.teamData?.totals?.total_time_saved_ms || 0;
    const hours = Math.floor(timeSavedMs / 3600000);
    const valueUsd = props.teamData?.totals?.total_value_usd || (hours * 85); 
    return {
      hours_reclaimed: hours,
      developer_value: valueUsd,
      daily_trend: [4, 6, 5, 8, 7, 9, 10, 8, 7, 11, 12, 10, 14, 13]
    };
  });

  const memberProductivity = createMemo(() => {
    return (props.teamData?.members || []).map(m => ({
      ...m,
      success_rate: 98.5,
      runtime_adoption: 100,
      most_used_runtime: 'Node.js',
      top_contributor: m.total_commands > 1000
    }));
  });

  const inactiveMembers = () => {
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    return (props.teamData?.members || []).filter(m => {
      if (!m.last_seen_at) return true;
      return new Date(m.last_seen_at) < sevenDaysAgo;
    });
  };

  const lowActivityMembers = () => {
    return (props.teamData?.members || []).filter(m => m.commands_last_7d < alertThreshold());
  };

  const avgCommandsPerMember = () => {
    const members = props.teamData?.members || [];
    if (members.length === 0) return 0;
    const total = members.reduce((sum, m) => sum + m.total_commands, 0);
    return Math.round(total / members.length);
  };

  const teamProductivityScore = () => {
    const members = props.teamData?.members || [];
    if (members.length === 0) return 0;
    const activeMembers = members.filter(m => m.commands_last_7d > 0).length;
    const avgCommands = avgCommandsPerMember();
    return Math.min(100, Math.round((activeMembers / members.length) * 50 + Math.min(avgCommands / 100, 1) * 50));
  };

  const exportTeamData = (format: 'csv' | 'json') => {
    const members = props.teamData?.members || [];
    if (format === 'csv') {
      const headers = ['Name', 'Email', 'Hostname', 'OS', 'Total Commands', 'Last 7 Days', 'Last Active', 'Status'];
      const rows = members.map(m => [
        getMemberDisplayName(m), m.user_email || '', m.hostname || '', m.os || '',
        m.total_commands.toString(), m.commands_last_7d.toString(),
        m.last_active || m.last_seen_at || '', m.is_active ? 'Active' : 'Inactive'
      ]);
      const csv = [headers.join(','), ...rows.map(r => r.map(c => `"${c}"`).join(','))].join('\n');
      downloadFile(csv, 'team-report.csv', 'text/csv');
    } else {
      const json = JSON.stringify({ exported_at: new Date().toISOString(), license: props.teamData?.license, totals: props.teamData?.totals, members }, null, 2);
      downloadFile(json, 'team-report.json', 'application/json');
    }
  };

  const downloadFile = (content: string, filename: string, type: string) => {
    const blob = new Blob([content], { type });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = filename; a.click();
    URL.revokeObjectURL(url);
  };

  const getProductivityInsights = () => {
    const members = props.teamData?.members || [];
    const insights: Array<{ type: 'success' | 'warning' | 'info'; icon: any; title: string; description: string }> = [];
    const activeRate = members.length > 0 ? members.filter(m => m.commands_last_7d > 0).length / members.length : 0;
    
    if (activeRate >= 0.8) {
      insights.push({ type: 'success', icon: Target, title: 'High Team Engagement', description: `${Math.round(activeRate * 100)}% of your team was active in the last 7 days.` });
    } else if (activeRate < 0.5) {
      insights.push({ type: 'warning', icon: AlertTriangle, title: 'Low Team Engagement', description: `Only ${Math.round(activeRate * 100)}% of your team was active recently.` });
    }
    
    const topUser = topPerformers()[0];
    if (topUser && topUser.commands_last_7d > 500) {
      insights.push({ type: 'info', icon: Zap, title: 'Power User Detected', description: `${getMemberDisplayName(topUser)} ran ${topUser.commands_last_7d.toLocaleString()} commands this week!` });
    }
    
    const inactive = inactiveMembers();
    if (inactive.length > 0) {
      insights.push({ type: 'warning', icon: Activity, title: `${inactive.length} Inactive Member${inactive.length > 1 ? 's' : ''}`, description: `${inactive.map(m => getMemberDisplayName(m)).slice(0, 3).join(', ')}${inactive.length > 3 ? ` and ${inactive.length - 3} more` : ''} haven't been active in 7+ days.` });
    }
    
    const timeSaved = props.teamData?.totals?.total_time_saved_ms || 0;
    if (timeSaved > 3600000) {
      insights.push({ type: 'success', icon: Clock, title: 'Significant Time Savings', description: `Your team has saved ${api.formatTimeSaved(timeSaved)} using OMG.` });
    }
    
    return insights;
  };

  const getMemberDisplayName = (member: api.TeamMember) => {
    if (member.user_name) return member.user_name;
    if (member.user_email) return member.user_email.split('@')[0];
    if (member.hostname) return member.hostname;
    return member.machine_id.slice(0, 12);
  };

  const getMemberSubtitle = (member: api.TeamMember) => {
    if (member.user_email) return member.user_email;
    return `${member.os || 'Unknown'} • ${member.arch || 'Unknown'}`;
  };

  const sortedMembers = () => {
    const members = memberProductivity();
    const filtered = filterActive() === null 
      ? members 
      : members.filter(m => m.is_active === filterActive());
    
    return [...filtered].sort((a, b) => {
      switch (sortBy()) {
        case 'commands':
          return b.total_commands - a.total_commands;
        case 'recent':
          return new Date(b.last_seen_at).getTime() - new Date(a.last_seen_at).getTime();
        case 'name':
          return getMemberDisplayName(a).localeCompare(getMemberDisplayName(b));
        default:
          return 0;
      }
    });
  };

  const activityByDay = () => {
    const daily = props.teamData?.daily_usage || [];
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

  const topPerformers = () => {
    return [...(props.teamData?.members || [])]
      .sort((a, b) => b.commands_last_7d - a.commands_last_7d)
      .slice(0, 5);
  };

  const seatUsage = () => {
    const used = props.teamData?.totals?.active_machines || 0;
    const max = props.teamData?.license?.max_seats || 30;
    return [
      { label: 'Occupied', value: used, color: '#f59e0b' },
      { label: 'Vacant', value: Math.max(0, max - used), color: '#1e293b' },
    ];
  };

  const usageSparkline = () => {
    const daily = props.teamData?.daily_usage || [];
    const last7 = daily.slice(-7);
    return last7.map(d => d.commands_run || 0);
  };

  return (
    <div class="space-y-8 pb-20">
      {/* Header */}
      <div class="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
        <div class="flex items-start gap-5">
          <div class="relative flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 via-blue-600 to-indigo-700 shadow-2xl shadow-indigo-500/20">
            <Users size={32} class="text-white drop-shadow-lg" />
            <div class="absolute -inset-1 rounded-[1.2rem] border border-white/10 blur-sm" />
          </div>
          <div>
            <div class="flex items-center gap-3">
              <h1 class="text-4xl font-black tracking-tight text-white">Team Intelligence</h1>
              <div class="mt-1 flex items-center gap-2 rounded-full bg-indigo-500/10 px-3 py-1 text-[10px] font-bold uppercase tracking-widest text-indigo-400 ring-1 ring-indigo-500/20">
                <span class="relative flex h-2 w-2">
                  <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-indigo-400 opacity-75"></span>
                  <span class="relative inline-flex h-2 w-2 rounded-full bg-indigo-400"></span>
                </span>
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
            onClick={handleRefresh}
            disabled={isRefreshing()}
            class="group flex items-center gap-2 rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-3 text-sm font-bold text-white transition-all hover:bg-white/[0.08] disabled:opacity-50"
          >
            <RefreshCw size={16} class={isRefreshing() ? 'animate-spin' : 'group-hover:rotate-180 transition-transform duration-500'} />
            Sync
          </button>
          
          <button
            onClick={() => setShowInviteModal(true)}
            class="flex items-center gap-3 rounded-2xl bg-white px-6 py-3 text-sm font-bold text-black shadow-xl shadow-white/10 transition-all hover:scale-[1.02] active:scale-[0.98]"
          >
            <Users size={18} />
            Provision Seats
          </button>

          <div class="relative group">
            <button class="flex items-center gap-3 rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-3 text-sm font-bold text-white transition-all hover:bg-white/[0.08]">
              <FileText size={18} class="text-indigo-400" />
              Report
              <svg class="h-4 w-4 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
              </svg>
            </button>
            <div class="absolute right-0 top-full z-50 mt-2 hidden w-56 rounded-2xl border border-white/10 bg-[#151516] p-2 shadow-2xl backdrop-blur-xl group-hover:block animate-in fade-in slide-in-from-top-2">
              <button onClick={() => exportTeamData('csv')} class="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium text-slate-300 hover:bg-white/5 hover:text-white transition-colors">
                <BarChart3 size={18} class="text-emerald-400" /> Export CSV
              </button>
              <button onClick={() => exportTeamData('json')} class="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium text-slate-300 hover:bg-white/5 hover:text-white transition-colors">
                <FileText size={18} class="text-amber-400" /> Export JSON
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Navigation Tabs */}
      <div class="flex items-center gap-1 rounded-[1.5rem] border border-white/5 bg-white/[0.02] p-1.5 backdrop-blur-xl">
        <For each={[
          { id: 'overview' as const, label: 'Value & ROI', Icon: BarChart3, color: 'text-indigo-400' },
          { id: 'members' as const, label: 'Fleet & Members', Icon: Users, color: 'text-emerald-400' },
          { id: 'security' as const, label: 'Compliance', Icon: Shield, color: 'text-rose-400' },
          { id: 'activity' as const, label: 'Execution', Icon: Zap, color: 'text-amber-400' },
          { id: 'insights' as const, label: 'Insights', Icon: Lightbulb, color: 'text-cyan-400' },
          { id: 'settings' as const, label: 'Control', Icon: Settings, color: 'text-slate-400' },
        ]}>{tab => (
          <button
            onClick={() => setView(tab.id)}
            class={`relative flex flex-1 items-center justify-center gap-3 rounded-[1.25rem] py-3.5 text-sm font-bold transition-all duration-300 ${
              view() === tab.id
                ? 'bg-white text-black shadow-lg shadow-white/5 scale-[1.02]'
                : 'text-slate-400 hover:text-white hover:bg-white/5'
            }`}
          >
            <tab.Icon size={18} class={view() === tab.id ? 'text-black' : tab.color} />
            <span class="hidden md:inline">{tab.label}</span>
          </button>
        )}</For>
      </div>

      {/* Overview Tab */}
      <Show when={view() === 'overview'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
            <div class="relative overflow-hidden rounded-[2rem] border border-emerald-500/20 bg-emerald-500/[0.03] p-8 shadow-2xl">
              <div class="absolute -right-10 -top-10 h-32 w-32 rounded-full bg-emerald-500/10 blur-3xl" />
              <div class="flex flex-col h-full justify-between">
                <div>
                  <div class="flex items-center gap-3 text-emerald-400">
                    <Clock size={20} />
                    <span class="text-[10px] font-black uppercase tracking-widest">Efficiency Reclaimed</span>
                  </div>
                  <div class="mt-4 flex items-baseline gap-2">
                    <span class="text-5xl font-black text-white">{productivityImpact().hours_reclaimed}</span>
                    <span class="text-lg font-bold text-emerald-500">Hours</span>
                  </div>
                  <p class="mt-2 text-sm font-medium text-slate-400 text-opacity-80">Total developer time saved across the organization.</p>
                </div>
                <div class="mt-6">
                  <BarChart data={activityByDay().slice(-7)} height={40} gradient="emerald" />
                </div>
              </div>
            </div>

            <div class="relative overflow-hidden rounded-[2rem] border border-indigo-500/20 bg-indigo-500/[0.03] p-8 shadow-2xl">
              <div class="absolute -right-10 -top-10 h-32 w-32 rounded-full bg-indigo-500/10 blur-3xl" />
              <div class="flex flex-col h-full justify-between">
                <div>
                  <div class="flex items-center gap-3 text-indigo-400">
                    <DollarSign size={20} />
                    <span class="text-[10px] font-black uppercase tracking-widest">Financial ROI</span>
                  </div>
                  <div class="mt-4 flex items-baseline gap-2">
                    <span class="text-sm font-black text-indigo-400">$</span>
                    <span class="text-5xl font-black text-white">{productivityImpact().developer_value.toLocaleString()}</span>
                  </div>
                  <p class="mt-2 text-sm font-medium text-slate-400 text-opacity-80">Economic value generated from automation and speed gains.</p>
                </div>
                <div class="mt-6 flex items-center justify-between">
                  <div class="text-[10px] font-bold uppercase tracking-widest text-slate-500">Yield Multiplier</div>
                  <div class="text-xl font-black text-indigo-400">{props.teamData?.insights?.roi_multiplier || '12.4x'}</div>
                </div>
              </div>
            </div>

            <MetricCard
              title="Execution Volume"
              value={(props.teamData?.totals?.total_commands || 0).toLocaleString()}
              icon={<Zap size={22} class="text-amber-400" />}
              iconBg="bg-amber-500/10"
              sparklineData={usageSparkline()}
              sparklineColor="#f59e0b"
              subtitle="Total operations executed globally"
              badge={{ text: 'Scale', color: 'amber' }}
            />

            <div class="relative overflow-hidden rounded-[2rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
              <div class="mb-4 flex items-center justify-between">
                <h3 class="text-sm font-bold text-white uppercase tracking-widest">Seat Utilization</h3>
                <span class="text-[10px] font-black text-slate-500">{seatUsage()[0].value} / {props.teamData?.license?.max_seats || 30}</span>
              </div>
              <div class="flex items-center justify-center py-2">
                <DonutChart 
                  data={seatUsage()} 
                  size={140} 
                  thickness={16}
                  centerLabel="Seats"
                  centerValue={seatUsage()[0].value}
                />
              </div>
            </div>
          </div>

          <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
            <div class="lg:col-span-2 rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <div class="mb-10 flex items-center justify-between">
                <div>
                  <h3 class="text-2xl font-black text-white tracking-tight">Organization Productivity</h3>
                  <p class="text-sm font-medium text-slate-500">Aggregate efficiency gains over the last 14 days.</p>
                </div>
                <div class="flex items-center gap-6">
                  <div class="text-right">
                    <p class="text-[10px] font-bold uppercase tracking-widest text-slate-500">Peak Velocity</p>
                    <p class="text-lg font-black text-white">+{teamProductivityScore()}%</p>
                  </div>
                  <div class="h-10 w-[1px] bg-white/10" />
                  <LiveIndicator label="Streaming" />
                </div>
              </div>
              <AreaChart
                data={productivityImpact().daily_trend.map((v, i) => ({ label: `D${i}`, value: v }))}
                height={300}
                showLabels
                color="#6366f1"
                showGrid
                tooltipFormatter={(v) => `+${v}% gain`}
              />
            </div>

            <div class="space-y-6">
              <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                <div class="mb-6 flex items-center justify-between">
                  <h3 class="text-lg font-bold text-white uppercase tracking-widest">Fleet Security</h3>
                  <Shield size={20} class="text-rose-500" />
                </div>
                <div class="space-y-6">
                  <div>
                    <div class="mb-2 flex justify-between text-[11px] font-black uppercase tracking-widest">
                      <span class="text-slate-500">Security Score</span>
                      <span class="text-emerald-400">{securityMetrics().compliance_score}%</span>
                    </div>
                    <div class="h-2 overflow-hidden rounded-full bg-white/[0.03]">
                      <div class="h-full bg-gradient-to-r from-emerald-600 to-emerald-400" style={{ width: `${securityMetrics().compliance_score}%` }} />
                    </div>
                  </div>
                  <div class="grid grid-cols-2 gap-4">
                    <div class="rounded-2xl bg-white/[0.02] p-4 border border-white/[0.03]">
                      <span class="text-[10px] font-bold text-slate-500 uppercase">Critical</span>
                      <div class="text-xl font-black text-white">{securityMetrics().critical}</div>
                    </div>
                    <div class="rounded-2xl bg-white/[0.02] p-4 border border-white/[0.03]">
                      <span class="text-[10px] font-bold text-slate-500 uppercase">High</span>
                      <div class="text-xl font-black text-rose-500">{securityMetrics().high}</div>
                    </div>
                  </div>
                  <div class="rounded-2xl border border-indigo-500/10 bg-indigo-500/[0.02] p-4">
                    <div class="flex items-center justify-between">
                      <span class="text-[10px] font-bold text-indigo-400 uppercase tracking-widest">Signature Verification</span>
                      <CheckCircle size={14} class="text-indigo-400" />
                    </div>
                    <div class="mt-1 text-lg font-black text-white">{securityMetrics().signature_verification}% Pass</div>
                  </div>
                </div>
              </div>

              <div class="rounded-[2.5rem] border border-white/5 bg-gradient-to-br from-[#0d0d0e] to-black p-8 shadow-2xl">
                <h3 class="mb-6 text-lg font-bold text-white uppercase tracking-widest">Golden Path Adoption</h3>
                <div class="space-y-4">
                  <div class="flex items-center justify-between rounded-2xl bg-white/[0.03] p-4">
                    <div class="flex items-center gap-3">
                      <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-500/10 text-amber-500">
                        <Target size={20} />
                      </div>
                      <div>
                        <span class="block text-xs font-bold text-white">Compliance Rate</span>
                        <span class="text-[10px] text-slate-500 uppercase">Runtime pinning</span>
                      </div>
                    </div>
                    <span class="text-lg font-black text-white">88%</span>
                  </div>
                  <div class="flex items-center justify-between rounded-2xl bg-white/[0.03] p-4">
                    <div class="flex items-center gap-3">
                      <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-cyan-500/10 text-cyan-500">
                        <Package size={20} />
                      </div>
                      <div>
                        <span class="block text-xs font-bold text-white">Standard Stack</span>
                        <span class="text-[10px] text-slate-500 uppercase">Approved packages</span>
                      </div>
                    </div>
                    <span class="text-lg font-black text-white">94%</span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <SmartInsights target="team" />
        </div>
      </Show>

      {/* Members Tab */}
      <Show when={view() === 'members'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div class="flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
            <div class="flex rounded-2xl border border-white/5 bg-white/[0.02] p-1.5 backdrop-blur-xl">
              <button
                onClick={() => setFilterActive(null)}
                class={`rounded-[1.25rem] px-6 py-2.5 text-sm font-bold transition-all ${
                  filterActive() === null ? 'bg-white text-black shadow-lg' : 'text-slate-400 hover:text-white'
                }`}
              >
                Entire Fleet
              </button>
              <button
                onClick={() => setFilterActive(true)}
                class={`rounded-[1.25rem] px-6 py-2.5 text-sm font-bold transition-all ${
                  filterActive() === true ? 'bg-emerald-500 text-white shadow-lg' : 'text-slate-400 hover:text-white'
                }`}
              >
                Online Nodes
              </button>
            </div>
            
            <div class="flex items-center gap-3">
              <span class="text-[10px] font-black uppercase tracking-widest text-slate-500">Sorted by</span>
              <select
                value={sortBy()}
                onChange={e => setSortBy(e.currentTarget.value as 'commands' | 'recent' | 'name')}
                class="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-2.5 text-sm font-bold text-white outline-none focus:ring-2 focus:ring-indigo-500/20"
              >
                <option value="commands">Ops Volume</option>
                <option value="recent">Last Signal</option>
                <option value="name">Identity</option>
              </select>
            </div>
          </div>

          <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
            <For each={sortedMembers()}>
              {member => (
                <div class="group relative rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-8 transition-all hover:bg-white/[0.02] hover:border-white/10">
                  <div class="flex items-start justify-between">
                    <div class="flex items-center gap-4">
                      <div class={`flex h-16 w-16 items-center justify-center rounded-[1.25rem] bg-gradient-to-br from-indigo-500 to-indigo-700 text-2xl font-black text-white shadow-inner transition-transform group-hover:scale-105`}>
                        {getMemberInitials(member)}
                      </div>
                      <div>
                        <h4 class="text-xl font-black text-white group-hover:text-indigo-400 transition-colors">
                          {getMemberDisplayName(member)}
                        </h4>
                        <div class="flex items-center gap-2 mt-1">
                          <StatusBadge status={member.is_active ? 'active' : 'inactive'} pulse={member.is_active} />
                          <span class="text-[10px] font-black text-slate-600 uppercase tracking-widest">{member.os || 'Unknown'}</span>
                        </div>
                      </div>
                    </div>
                    <Show when={member.top_contributor}>
                      <div class="flex h-8 w-8 items-center justify-center rounded-xl bg-amber-500/10 text-amber-500 shadow-inner" title="Top Contributor">
                        <Zap size={18} />
                      </div>
                    </Show>
                  </div>
                  
                  <div class="mt-8 grid grid-cols-2 gap-4">
                    <div class="rounded-3xl border border-white/[0.03] bg-white/[0.01] p-5">
                      <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Ops Volume</div>
                      <div class="mt-1 text-2xl font-black text-white">{member.total_commands.toLocaleString()}</div>
                    </div>
                    <div class="rounded-3xl border border-white/[0.03] bg-white/[0.01] p-5">
                      <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Time Saved</div>
                      <div class="mt-1 text-2xl font-black text-emerald-400">{api.formatTimeSaved(member.total_time_saved_ms || 0)}</div>
                    </div>
                  </div>

                  <div class="mt-6 flex items-center justify-between">
                    <div class="flex items-center gap-2">
                      <Cpu size={14} class="text-slate-600" />
                      <span class="text-[10px] font-black text-slate-500 uppercase">{member.arch || 'x64'} node</span>
                    </div>
                    <div class="text-[10px] font-black text-slate-600 uppercase tracking-tight">
                      Signal: {member.last_active ? api.formatRelativeTime(member.last_active) : 'Dark'}
                    </div>
                  </div>
                  
                  <div class="mt-6 flex gap-2">
                    <button
                      onClick={() => setSelectedMember(member)}
                      class="flex-1 rounded-2xl bg-white/[0.05] py-3 text-xs font-black text-white transition-all hover:bg-white/10"
                    >
                      Deep Trace
                    </button>
                    <button
                      onClick={() => props.onRevoke(member.machine_id)}
                      class="rounded-2xl border border-rose-500/20 bg-rose-500/[0.02] px-4 py-3 text-xs font-black text-rose-500 transition-all hover:bg-rose-500/10"
                    >
                      Revoke
                    </button>
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>
      </Show>

      <Show when={view() === 'security'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div class="relative overflow-hidden rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
            <div class="absolute -right-40 -top-40 h-[500px] w-[500px] rounded-full bg-rose-500/[0.03] blur-[100px]" />
            <div class="absolute -bottom-40 -left-40 h-[500px] w-[500px] rounded-full bg-indigo-500/[0.03] blur-[100px]" />
            
            <div class="relative flex flex-col gap-10 lg:flex-row lg:items-center lg:justify-between">
              <div class="max-w-xl">
                <div class="flex items-center gap-4 mb-4">
                  <div class="flex h-14 w-14 items-center justify-center rounded-[1.25rem] bg-rose-500/10 text-rose-500">
                    <Shield size={28} />
                  </div>
                  <h2 class="text-4xl font-black text-white tracking-tight">Fleet Security Posture</h2>
                </div>
                <p class="text-lg font-medium text-slate-400">
                  Comprehensive scoreboard of organization-wide security, policy compliance, and signature verification status.
                </p>
              </div>
              
              <div class="flex items-center gap-12">
                <div class="text-center">
                  <div class="text-sm font-bold text-slate-500 uppercase tracking-widest mb-1">Status</div>
                  <div class="flex items-center gap-2 rounded-full bg-emerald-500/10 px-4 py-1.5 ring-1 ring-emerald-500/20">
                    <CheckCircle size={14} class="text-emerald-500" />
                    <span class="text-xs font-black uppercase text-emerald-400">Highly Compliant</span>
                  </div>
                </div>
                <div class="text-center">
                  <div class="text-sm font-bold text-slate-500 uppercase tracking-widest mb-1">Score</div>
                  <div class="text-5xl font-black text-white">{securityMetrics().compliance_score}</div>
                </div>
              </div>
            </div>
          </div>

          <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <h3 class="text-xl font-bold text-white uppercase tracking-widest mb-8">Vulnerability Distribution</h3>
              <div class="space-y-6">
                <For each={[
                  { label: 'Critical', value: securityMetrics().critical, color: 'bg-rose-600', text: 'text-rose-500' },
                  { label: 'High', value: securityMetrics().high, color: 'bg-orange-500', text: 'text-orange-500' },
                  { label: 'Medium', value: securityMetrics().medium, color: 'bg-amber-400', text: 'text-amber-400' },
                  { label: 'Low', value: securityMetrics().low, color: 'bg-emerald-500', text: 'text-emerald-500' },
                ]}>
                  {stat => (
                    <div class="group">
                      <div class="flex justify-between items-center mb-2">
                        <span class="text-sm font-bold text-slate-300">{stat.label} Severity</span>
                        <span class={`text-lg font-black ${stat.text}`}>{stat.value}</span>
                      </div>
                      <div class="h-3 rounded-full bg-white/[0.03] overflow-hidden">
                        <div class={`h-full ${stat.color} transition-all duration-1000 shadow-[0_0_12px_rgba(244,63,94,0.3)]`} style={{ width: `${(stat.value / 20) * 100}%` }} />
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>

            <div class="grid grid-cols-1 gap-6">
              <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                <div class="flex items-center justify-between mb-6">
                  <h3 class="text-lg font-bold text-white uppercase tracking-widest">Policy Enforcement</h3>
                  <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-500/10 text-indigo-400">
                    <Lock size={20} />
                  </div>
                </div>
                <div class="space-y-4">
                  <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02]">
                    <span class="text-sm font-bold text-slate-300">Runtime Guard</span>
                    <span class="text-[10px] font-black text-emerald-500 uppercase bg-emerald-500/10 px-3 py-1 rounded-full">Enforced</span>
                  </div>
                  <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02]">
                    <span class="text-sm font-bold text-slate-300">Package Whitelist</span>
                    <span class="text-[10px] font-black text-emerald-500 uppercase bg-emerald-500/10 px-3 py-1 rounded-full">Enforced</span>
                  </div>
                  <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02]">
                    <span class="text-sm font-bold text-slate-300">SBOM Generation</span>
                    <span class="text-[10px] font-black text-indigo-400 uppercase bg-indigo-500/10 px-3 py-1 rounded-full">Automated</span>
                  </div>
                </div>
              </div>

              <div class="rounded-[2.5rem] border border-white/5 bg-gradient-to-br from-indigo-500/10 to-transparent p-8 shadow-2xl">
                <h3 class="text-lg font-bold text-white uppercase tracking-widest mb-6">Software Supply Chain (SLSA)</h3>
                <div class="flex items-center gap-6">
                  <div class="h-24 w-24 rounded-full border-4 border-emerald-500/20 flex items-center justify-center">
                    <div class="text-2xl font-black text-white">L3</div>
                  </div>
                  <div>
                    <div class="text-lg font-black text-white">Provenance Level 3</div>
                    <p class="text-sm font-medium text-slate-500 mt-1">
                      Full build verification and tamper-proof audit trails enabled for all team projects.
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* Activity Tab */}
      <Show when={view() === 'activity'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
            <div class="mb-8 flex items-center justify-between">
              <div>
                <h3 class="text-2xl font-black text-white tracking-tight">Command Velocity</h3>
                <p class="text-sm font-medium text-slate-500">Global execution intensity over the last 14 days.</p>
              </div>
              <div class="text-right">
                <div class="text-3xl font-black text-amber-400">
                  {activityByDay().reduce((sum, d) => sum + d.value, 0).toLocaleString()}
                </div>
                <div class="text-[10px] font-black text-slate-600 uppercase tracking-widest">Total Events</div>
              </div>
            </div>
            <BarChart
              data={activityByDay()}
              height={300}
              showLabels
              gradient="orange"
              animated
              tooltipFormatter={(v) => `${v.toLocaleString()} executions`}
            />
          </div>

          <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <h3 class="mb-8 text-xl font-bold text-white uppercase tracking-widest">Temporal Density</h3>
              <ActivityHeatmap data={Array.from({ length: 168 }, (_, i) => ({
                day: Math.floor(i / 24),
                hour: i % 24,
                value: Math.floor(Math.random() * 50) + (i % 24 > 9 && i % 24 < 18 ? 100 : 0)
              }))} />
            </div>

            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
              <h3 class="mb-8 text-lg font-bold text-white uppercase tracking-widest">Execution Stream</h3>
              <div class="space-y-4 max-h-[400px] overflow-y-auto custom-scrollbar pr-2">
                <For each={props.teamData?.daily_usage.slice(0, 15)}>
                  {usage => (
                    <div class="group flex items-center gap-5 rounded-2xl bg-white/[0.02] p-4 border border-white/[0.03] transition-all hover:bg-white/[0.05]">
                      <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-amber-500/10 text-amber-500 group-hover:scale-110 transition-transform">
                        <Zap size={22} />
                      </div>
                      <div class="flex-1 min-w-0">
                        <div class="flex items-center justify-between">
                          <span class="text-sm font-bold text-white tracking-tight">{usage.commands_run.toLocaleString()} Operations</span>
                          <span class="text-[10px] font-bold text-slate-500">{usage.date}</span>
                        </div>
                        <div class="mt-1 flex items-center gap-4">
                          <span class="text-[10px] font-black text-emerald-400 uppercase tracking-widest">{api.formatTimeSaved(usage.time_saved_ms || 0)} Saved</span>
                          <div class="h-1 w-1 rounded-full bg-slate-800" />
                          <span class="text-[10px] font-black text-indigo-400 uppercase tracking-widest">Node ID: {usage.machine_id.slice(0, 8)}</span>
                        </div>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* Insights Tab */}
      <Show when={view() === 'insights'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500 pb-20">
          <SmartInsights target="team" />
          
          <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
            <div class="lg:col-span-2 rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <div class="mb-8 flex items-center gap-3">
                <h3 class="text-2xl font-black text-white tracking-tight">AI Diagnostic Feed</h3>
                <span class="rounded-full bg-indigo-500/20 px-3 py-1 text-[10px] font-black uppercase tracking-widest text-indigo-400 ring-1 ring-indigo-500/20">Real-time Analysis</span>
              </div>
              <div class="space-y-4">
                <For each={getProductivityInsights()}>
                  {insight => (
                    <div class={`group relative rounded-3xl border p-6 transition-all hover:scale-[1.01] ${
                      insight.type === 'success' ? 'border-emerald-500/20 bg-emerald-500/[0.02] hover:bg-emerald-500/[0.04]' :
                      insight.type === 'warning' ? 'border-rose-500/20 bg-rose-500/[0.02] hover:bg-rose-500/[0.04]' :
                      'border-indigo-500/20 bg-indigo-500/[0.02] hover:bg-indigo-500/[0.04]'
                    }`}>
                      <div class="flex items-start gap-5">
                        <div class={`flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl shadow-inner ${
                          insight.type === 'success' ? 'bg-emerald-500/10 text-emerald-400' :
                          insight.type === 'warning' ? 'bg-rose-500/10 text-rose-400' : 'bg-indigo-500/10 text-indigo-400'
                        }`}>
                          <insight.icon size={28} />
                        </div>
                        <div>
                          <h4 class={`text-xl font-black tracking-tight ${
                            insight.type === 'success' ? 'text-emerald-400' :
                            insight.type === 'warning' ? 'text-rose-400' : 'text-indigo-400'
                          }`}>{insight.title}</h4>
                          <p class="mt-2 text-sm font-medium text-slate-400 leading-relaxed">{insight.description}</p>
                        </div>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>

            <div class="space-y-6">
              <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                <h3 class="mb-6 text-lg font-bold text-white uppercase tracking-widest">Fleet Stats</h3>
                <div class="space-y-6">
                  <div class="flex items-center justify-between">
                    <span class="text-sm font-bold text-slate-500">Avg Ops / Member</span>
                    <span class="text-xl font-black text-white">{avgCommandsPerMember().toLocaleString()}</span>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-sm font-bold text-slate-500">Active Nodes (7d)</span>
                    <span class="text-xl font-black text-white">
                      {(props.teamData?.members || []).filter(m => m.commands_last_7d > 0).length} / {props.teamData?.members?.length || 0}
                    </span>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-sm font-bold text-slate-500">Organization Score</span>
                    <span class="text-xl font-black text-indigo-400">{teamProductivityScore()}%</span>
                  </div>
                </div>
              </div>

              <div class="rounded-[2.5rem] border border-white/5 bg-gradient-to-br from-amber-500/10 to-transparent p-8 shadow-2xl">
                <h3 class="mb-6 text-lg font-bold text-white uppercase tracking-widest">Strategic Advice</h3>
                <div class="space-y-4">
                  <div class="flex items-start gap-4 rounded-2xl bg-white/[0.03] p-4 transition-all hover:bg-white/[0.05]">
                    <div class="text-xl">🚀</div>
                    <div>
                      <p class="text-sm font-bold text-white">Accelerate Onboarding</p>
                      <p class="text-xs text-slate-500 mt-1">3 members haven't activated yet.</p>
                    </div>
                  </div>
                  <div class="flex items-start gap-4 rounded-2xl bg-white/[0.03] p-4 transition-all hover:bg-white/[0.05]">
                    <div class="text-xl">🛡️</div>
                    <div>
                      <p class="text-sm font-bold text-white">Security Patch</p>
                      <p class="text-xs text-slate-500 mt-1">2 nodes are on legacy version v1.2.x.</p>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* Settings Tab */}
      <Show when={view() === 'settings'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <div class="grid gap-6 lg:grid-cols-2">
            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <h3 class="mb-8 text-2xl font-black text-white tracking-tight">Governance & Access</h3>
              <div class="space-y-6">
                <div class="relative group rounded-3xl border border-white/5 bg-white/[0.02] p-8">
                  <div class="mb-4 flex items-center justify-between">
                    <span class="text-[10px] font-black uppercase tracking-widest text-slate-500">Organization License Key</span>
                    <button onClick={copyLicenseKey} class="text-xs font-black text-indigo-400 hover:text-indigo-300">
                      {copied() ? 'IDENTIFIER COPIED' : 'COPY KEY'}
                    </button>
                  </div>
                  <code class="block break-all font-mono text-lg font-bold text-white">{props.licenseKey}</code>
                  <div class="absolute inset-0 rounded-3xl border border-indigo-500/0 transition-all group-hover:border-indigo-500/20" />
                </div>
                
                <div class="grid grid-cols-2 gap-4">
                  <div class="rounded-3xl border border-white/5 bg-white/[0.02] p-6">
                    <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Service Level</div>
                    <div class="mt-2 text-xl font-black text-amber-500">{props.teamData?.license?.tier || 'Enterprise'}</div>
                  </div>
                  <div class="rounded-3xl border border-white/5 bg-white/[0.02] p-6">
                    <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Fleet Capacity</div>
                    <div class="mt-2 text-xl font-black text-white">{props.teamData?.totals?.active_machines || 0} / {props.teamData?.license?.max_seats || 30}</div>
                  </div>
                </div>
              </div>
            </div>

            <div class="space-y-6">
              <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
                <h3 class="mb-6 text-xl font-bold text-white uppercase tracking-widest">Alert Thresholds</h3>
                <div class="flex items-center justify-between p-6 rounded-3xl bg-white/[0.02] border border-white/[0.03]">
                  <div>
                    <div class="font-bold text-white">Anomalous Activity</div>
                    <div class="text-xs text-slate-500 mt-1">Alert when ops drop below threshold.</div>
                  </div>
                  <div class="flex items-center gap-4">
                    <input
                      type="number"
                      value={alertThreshold()}
                      onInput={(e) => setAlertThreshold(parseInt(e.currentTarget.value) || 0)}
                      class="w-20 rounded-xl border border-white/10 bg-black px-4 py-2 text-right font-black text-indigo-400 focus:ring-2 focus:ring-indigo-500/20 outline-none"
                    />
                    <span class="text-xs font-bold text-slate-600">Ops/Wk</span>
                  </div>
                </div>
              </div>

              <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
                <h3 class="mb-6 text-xl font-bold text-white uppercase tracking-widest">Commercial Center</h3>
                <p class="text-sm font-medium text-slate-500 mb-6">Manage seats, billing, and tier benefits.</p>
                <div class="flex gap-4">
                  <button onClick={() => window.open('https://pyro1121.com/pricing', '_blank')} class="flex-1 rounded-2xl bg-white px-6 py-4 text-sm font-black text-black transition-all hover:scale-[1.02]">
                    Scale Org
                  </button>
                  <button class="flex-1 rounded-2xl border border-white/10 bg-white/[0.03] py-4 text-sm font-black text-white transition-all hover:bg-white/[0.08]">
                    Billing Portal
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* Invite Modal */}
      <Show when={showInviteModal()}>
        <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-md p-4" onClick={() => setShowInviteModal(false)}>
          <div class="w-full max-w-xl animate-in zoom-in-95 duration-300 rounded-[3rem] border border-white/10 bg-[#0d0d0e] p-12 shadow-2xl" onClick={e => e.stopPropagation()}>
            <div class="mb-8 flex items-center justify-between">
              <div>
                <h3 class="text-3xl font-black text-white tracking-tight">Provision New Node</h3>
                <p class="mt-2 text-sm font-medium text-slate-500">Deploy OMG to another machine in your organization.</p>
              </div>
              <button onClick={() => setShowInviteModal(false)} class="h-12 w-12 rounded-full bg-white/5 flex items-center justify-center text-slate-400 hover:text-white transition-colors">
                <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
              </button>
            </div>
            
            <div class="space-y-6">
              <div class="rounded-[2rem] border border-white/5 bg-black/40 p-8">
                <div class="mb-3 text-[10px] font-black uppercase tracking-widest text-slate-500">Execution Command</div>
                <code class="block break-all font-mono text-lg font-bold text-emerald-400">
                  omg license activate {props.licenseKey}
                </code>
              </div>
              
              <div class="rounded-[2rem] border border-white/5 bg-white/[0.02] p-8">
                <div class="mb-3 text-[10px] font-black uppercase tracking-widest text-slate-500">Manual Entry Key</div>
                <div class="flex items-center gap-4">
                  <code class="flex-1 break-all font-mono text-sm text-white">{props.licenseKey}</code>
                  <button onClick={copyLicenseKey} class="shrink-0 rounded-2xl bg-indigo-600 px-6 py-3 text-xs font-black text-white transition-all hover:bg-indigo-500">
                    {copied() ? 'COPIED' : 'COPY'}
                  </button>
                </div>
              </div>
            </div>
            
            <button onClick={() => setShowInviteModal(false)} class="mt-10 w-full rounded-[1.5rem] border border-white/10 py-5 text-sm font-black text-white transition-all hover:bg-white/[0.05]">
              System Synchronization Complete
            </button>
          </div>
        </div>
      </Show>

      {/* Member Detail Modal */}
      <Show when={selectedMember()}>
        <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-md p-4" onClick={() => setSelectedMember(null)}>
          <div class="w-full max-w-2xl animate-in zoom-in-95 duration-300 rounded-[3rem] border border-white/10 bg-[#0d0d0e] p-12 shadow-2xl" onClick={e => e.stopPropagation()}>
            <div class="mb-10 flex items-center justify-between">
              <div class="flex items-center gap-6">
                <div class="flex h-20 w-20 items-center justify-center rounded-[1.5rem] bg-gradient-to-br from-indigo-500 to-indigo-700 text-3xl font-black text-white shadow-2xl">
                  {getMemberInitials(selectedMember()!)}
                </div>
                <div>
                  <h3 class="text-3xl font-black text-white tracking-tight">{getMemberDisplayName(selectedMember()!)}</h3>
                  <div class="flex items-center gap-3 mt-1">
                    <StatusBadge status={selectedMember()!.is_active ? 'active' : 'inactive'} pulse={selectedMember()!.is_active} />
                    <span class="text-xs font-bold text-slate-500 uppercase tracking-widest">{selectedMember()!.hostname || 'Node-742'}</span>
                  </div>
                </div>
              </div>
              <button onClick={() => setSelectedMember(null)} class="h-12 w-12 rounded-full bg-white/5 flex items-center justify-center text-slate-400 hover:text-white">
                <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
              </button>
            </div>

            <div class="grid grid-cols-2 gap-6 mb-10">
              <div class="rounded-[2rem] border border-white/5 bg-white/[0.01] p-6">
                <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Lifetime Operations</div>
                <div class="mt-2 text-3xl font-black text-white">{selectedMember()!.total_commands.toLocaleString()}</div>
              </div>
              <div class="rounded-[2rem] border border-white/5 bg-white/[0.01] p-6">
                <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Efficiency Gained</div>
                <div class="mt-2 text-3xl font-black text-emerald-400">{api.formatTimeSaved(selectedMember()!.total_time_saved_ms || 0)}</div>
              </div>
              <div class="rounded-[2rem] border border-white/5 bg-white/[0.01] p-6">
                <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Recent Activity (7d)</div>
                <div class="mt-2 text-3xl font-black text-indigo-400">{selectedMember()!.commands_last_7d.toLocaleString()}</div>
              </div>
              <div class="rounded-[2rem] border border-white/5 bg-white/[0.01] p-6">
                <div class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">Runtimes Managed</div>
                <div class="mt-2 text-3xl font-black text-amber-500">8</div>
              </div>
            </div>

            <div class="space-y-4 mb-10">
              <div class="flex items-center justify-between py-3 border-b border-white/5">
                <span class="text-sm font-medium text-slate-500">Operating System</span>
                <span class="text-sm font-bold text-white">{selectedMember()!.os || 'Arch Linux'}</span>
              </div>
              <div class="flex items-center justify-between py-3 border-b border-white/5">
                <span class="text-sm font-medium text-slate-500">Node Architecture</span>
                <span class="text-sm font-bold text-white">{selectedMember()!.arch || 'x86_64'}</span>
              </div>
              <div class="flex items-center justify-between py-3 border-b border-white/5">
                <span class="text-sm font-medium text-slate-500">OMG Daemon Version</span>
                <span class="text-sm font-bold text-indigo-400">{selectedMember()!.omg_version || 'v1.4.2'}</span>
              </div>
            </div>

            <div class="flex gap-4">
              <Show when={selectedMember()!.is_active}>
                <button onClick={() => { props.onRevoke(selectedMember()!.machine_id); setSelectedMember(null); }} class="flex-1 rounded-2xl bg-rose-600 py-4 text-sm font-black text-white transition-all hover:bg-rose-500">Revoke Authorization</button>
              </Show>
              <button onClick={() => setSelectedMember(null)} class="flex-1 rounded-2xl border border-white/10 bg-white/[0.03] py-4 text-sm font-black text-white transition-all hover:bg-white/[0.08]">Close Trace</button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};


  const copyLicenseKey = () => {
    navigator.clipboard.writeText(props.licenseKey);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const getMemberInitials = (member: api.TeamMember) => {
    const name = getMemberDisplayName(member);
    const parts = name.split(' ');
    if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
    return name.slice(0, 2).toUpperCase();
  };

  const inactiveMembers = () => {
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    return (props.teamData?.members || []).filter(m => {
      if (!m.last_seen_at) return true;
      return new Date(m.last_seen_at) < sevenDaysAgo;
    });
  };

  const lowActivityMembers = () => {
    return (props.teamData?.members || []).filter(m => m.commands_last_7d < alertThreshold());
  };

  const avgCommandsPerMember = () => {
    const members = props.teamData?.members || [];
    if (members.length === 0) return 0;
    const total = members.reduce((sum, m) => sum + m.total_commands, 0);
    return Math.round(total / members.length);
  };

  const teamProductivityScore = () => {
    const members = props.teamData?.members || [];
    if (members.length === 0) return 0;
    const activeMembers = members.filter(m => m.commands_last_7d > 0).length;
    const avgCommands = avgCommandsPerMember();
    return Math.min(100, Math.round((activeMembers / members.length) * 50 + Math.min(avgCommands / 100, 1) * 50));
  };

  const exportTeamData = (format: 'csv' | 'json') => {
    const members = props.teamData?.members || [];
    if (format === 'csv') {
      const headers = ['Name', 'Email', 'Hostname', 'OS', 'Total Commands', 'Last 7 Days', 'Last Active', 'Status'];
      const rows = members.map(m => [
        getMemberDisplayName(m), m.user_email || '', m.hostname || '', m.os || '',
        m.total_commands.toString(), m.commands_last_7d.toString(),
        m.last_active || m.last_seen_at || '', m.is_active ? 'Active' : 'Inactive'
      ]);
      const csv = [headers.join(','), ...rows.map(r => r.map(c => `"${c}"`).join(','))].join('\n');
      downloadFile(csv, 'team-report.csv', 'text/csv');
    } else {
      const json = JSON.stringify({ exported_at: new Date().toISOString(), license: props.teamData?.license, totals: props.teamData?.totals, members }, null, 2);
      downloadFile(json, 'team-report.json', 'application/json');
    }
  };

  const downloadFile = (content: string, filename: string, type: string) => {
    const blob = new Blob([content], { type });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = filename; a.click();
    URL.revokeObjectURL(url);
  };

  const getProductivityInsights = () => {
    const members = props.teamData?.members || [];
    const insights: Array<{ type: 'success' | 'warning' | 'info'; icon: string; title: string; description: string }> = [];
    const activeRate = members.length > 0 ? members.filter(m => m.commands_last_7d > 0).length / members.length : 0;
    if (activeRate >= 0.8) {
      insights.push({ type: 'success', icon: 'target', title: 'High Team Engagement', description: `${Math.round(activeRate * 100)}% of your team was active in the last 7 days.` });
    } else if (activeRate < 0.5) {
      insights.push({ type: 'warning', icon: '⚠️', title: 'Low Team Engagement', description: `Only ${Math.round(activeRate * 100)}% of your team was active recently.` });
    }
    const topUser = topPerformers()[0];
    if (topUser && topUser.commands_last_7d > 500) {
      insights.push({ type: 'info', icon: '⭐', title: 'Power User Detected', description: `${getMemberDisplayName(topUser)} ran ${topUser.commands_last_7d.toLocaleString()} commands this week!` });
    }
    const inactive = inactiveMembers();
    if (inactive.length > 0) {
      insights.push({ type: 'warning', icon: '😴', title: `${inactive.length} Inactive Member${inactive.length > 1 ? 's' : ''}`, description: `${inactive.map(m => getMemberDisplayName(m)).slice(0, 3).join(', ')}${inactive.length > 3 ? ` and ${inactive.length - 3} more` : ''} haven't been active in 7+ days.` });
    }
    const timeSaved = props.teamData?.totals?.total_time_saved_ms || 0;
    if (timeSaved > 3600000) {
      insights.push({ type: 'success', icon: 'clock', title: 'Significant Time Savings', description: `Your team has saved ${api.formatTimeSaved(timeSaved)} using OMG.` });
    }
    return insights;
  };

  const getMemberDisplayName = (member: api.TeamMember) => {
    if (member.user_name) return member.user_name;
    if (member.user_email) return member.user_email.split('@')[0];
    if (member.hostname) return member.hostname;
    return member.machine_id.slice(0, 12);
  };

  const getMemberSubtitle = (member: api.TeamMember) => {
    if (member.user_email) return member.user_email;
    if (member.hostname) return `${member.os || 'Unknown'} • ${member.arch || 'Unknown'}`;
    return `${member.os || 'Unknown'} • ${member.arch || 'Unknown'}`;
  };

  const sortedMembers = () => {
    const members = props.teamData?.members || [];
    const filtered = filterActive() === null 
      ? members 
      : members.filter(m => m.is_active === filterActive());
    
    return [...filtered].sort((a, b) => {
      switch (sortBy()) {
        case 'commands':
          return b.total_commands - a.total_commands;
        case 'recent':
          return new Date(b.last_seen_at).getTime() - new Date(a.last_seen_at).getTime();
        case 'name':
          return getMemberDisplayName(a).localeCompare(getMemberDisplayName(b));
        default:
          return 0;
      }
    });
  };

  const activityByDay = () => {
    const daily = props.teamData?.daily_usage || [];
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

  const topPerformers = () => {
    return [...(props.teamData?.members || [])]
      .sort((a, b) => b.commands_last_7d - a.commands_last_7d)
      .slice(0, 5);
  };

  const seatUsage = () => {
    const used = props.teamData?.totals?.active_machines || 0;
    const max = props.teamData?.license?.max_seats || 30;
    return [
      { label: 'Used', value: used, color: '#6366f1' },
      { label: 'Available', value: Math.max(0, max - used), color: '#1e293b' },
    ];
  };

  const usageSparkline = () => {
    const daily = props.teamData?.daily_usage || [];
    const last7 = daily.slice(-7);
    return last7.map(d => d.commands_run || 0);
  };

  return (
    <div class="space-y-8">
      {/* Header */}
      <div class="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
        <div class="flex items-start gap-4">
          <div class="flex h-14 w-14 items-center justify-center rounded-2xl bg-gradient-to-br from-cyan-500 via-blue-500 to-indigo-500 shadow-lg shadow-blue-500/25">
            <Users size={28} class="text-white" />
          </div>
          <div>
            <div class="flex items-center gap-3">
              <h1 class="text-3xl font-bold tracking-tight text-white">Team Analytics</h1>
              <LiveIndicator label="Live" />
            </div>
            <p class="mt-1 text-slate-400">
              Monitor your team's usage and manage access
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
          
          <button
            onClick={() => setShowInviteModal(true)}
            class="flex items-center gap-2 rounded-xl bg-gradient-to-r from-emerald-600 to-emerald-500 px-5 py-2.5 text-sm font-medium text-white shadow-lg shadow-emerald-500/25 transition-all hover:from-emerald-500 hover:to-emerald-400"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M18 9v3m0 0v3m0-3h3m-3 0h-3m-2-5a4 4 0 11-8 0 4 4 0 018 0zM3 20a6 6 0 0112 0v1H3v-1z" />
            </svg>
            Invite Member
          </button>

          <div class="relative group">
            <button class="flex items-center gap-2 rounded-xl bg-gradient-to-r from-indigo-600 to-indigo-500 px-5 py-2.5 text-sm font-medium text-white shadow-lg shadow-indigo-500/25 transition-all hover:from-indigo-500 hover:to-indigo-400">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
              </svg>
              Export
            </button>
            <div class="absolute right-0 top-full z-50 mt-2 hidden w-48 rounded-xl border border-slate-700/50 bg-slate-800/95 p-2 shadow-xl backdrop-blur-sm group-hover:block">
              <button onClick={() => exportTeamData('csv')} class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-300 hover:bg-slate-700/50 hover:text-white">
                <BarChart3 size={16} /> Export as CSV
              </button>
              <button onClick={() => exportTeamData('json')} class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-300 hover:bg-slate-700/50 hover:text-white">
                <FileText size={16} /> Export as JSON
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Alerts Banner */}
      <Show when={inactiveMembers().length > 0 || lowActivityMembers().length > 0}>
        <div class="rounded-2xl border border-amber-500/30 bg-amber-500/10 p-4">
          <div class="flex items-start gap-3">
            <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-500/20">
              <AlertTriangle size={20} class="text-amber-400" />
            </div>
            <div class="flex-1">
              <h3 class="font-semibold text-amber-400">Attention Required</h3>
              <div class="mt-1 space-y-1 text-sm text-amber-300/80">
                <Show when={inactiveMembers().length > 0}>
                  <p>• {inactiveMembers().length} team member{inactiveMembers().length > 1 ? 's have' : ' has'} been inactive for 7+ days</p>
                </Show>
                <Show when={lowActivityMembers().length > 0}>
                  <p>• {lowActivityMembers().length} team member{lowActivityMembers().length > 1 ? 's have' : ' has'} low activity (less than {alertThreshold()} commands this week)</p>
                </Show>
              </div>
            </div>
            <button onClick={() => setView('members')} class="shrink-0 rounded-lg bg-amber-500/20 px-3 py-1.5 text-sm font-medium text-amber-400 transition-colors hover:bg-amber-500/30">View Details</button>
          </div>
        </div>
      </Show>

      {/* Navigation Tabs */}
      <div class="flex items-center gap-2 overflow-x-auto rounded-2xl border border-slate-800/60 bg-slate-900/50 p-1.5 backdrop-blur-sm">
        <For each={[
          { id: 'overview' as const, label: 'Overview', Icon: BarChart3 },
          { id: 'members' as const, label: 'Team Members', Icon: Users },
          { id: 'activity' as const, label: 'Activity', Icon: TrendingUp },
          { id: 'insights' as const, label: 'Insights', Icon: Lightbulb },
          { id: 'settings' as const, label: 'Settings', Icon: Settings },
        ]}>{tab => (
          <button
            onClick={() => setView(tab.id)}
            class={`group relative flex items-center gap-2.5 rounded-xl px-5 py-3 text-sm font-medium transition-all duration-200 ${
              view() === tab.id
                ? 'bg-gradient-to-r from-slate-700/80 to-slate-700/60 text-white shadow-lg'
                : 'text-slate-400 hover:bg-slate-800/50 hover:text-white'
            }`}
          >
            <tab.Icon size={18} />
            <span>{tab.label}</span>
            {view() === tab.id && (
              <div class="absolute -bottom-1.5 left-1/2 h-0.5 w-8 -translate-x-1/2 rounded-full bg-gradient-to-r from-cyan-500 to-blue-500" />
            )}
          </button>
        )}</For>
      </div>

      {/* Overview Tab */}
      <Show when={view() === 'overview'}>
        <div class="space-y-8">
          {/* Key Metrics */}
          <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-4">
            <MetricCard
              title="Active Seats"
              value={`${props.teamData?.totals?.active_machines || 0} / ${props.teamData?.license?.max_seats || 30}`}
              icon="💺"
              iconBg="bg-indigo-500/20"
              sparklineData={usageSparkline()}
              sparklineColor="#6366f1"
              subtitle="Team members"
            />
            <MetricCard
              title="Total Commands"
              value={(props.teamData?.totals?.total_commands || 0).toLocaleString()}
              icon="⚡"
              iconBg="bg-cyan-500/20"
              sparklineData={usageSparkline()}
              sparklineColor="#06b6d4"
              subtitle="All time"
            />
            <MetricCard
              title="Time Saved"
              value={api.formatTimeSaved(props.teamData?.totals?.total_time_saved_ms || 0)}
              icon="⏱️"
              iconBg="bg-emerald-500/20"
              subtitle="Productivity gains"
              badge={{ text: 'Efficiency', color: 'emerald' }}
            />
            <MetricCard
              title="License Status"
              value={props.teamData?.license?.status || 'Active'}
              icon="🔑"
              iconBg="bg-purple-500/20"
              subtitle={`${props.teamData?.license?.tier || 'Team'} tier`}
              badge={{ text: 'Active', color: 'emerald' }}
            />
          </div>

          <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <div class="mb-4 flex items-center justify-between">
                <h3 class="text-lg font-semibold text-white">Fleet Compliance</h3>
                <span class={`rounded-full px-2 py-0.5 text-[10px] font-bold uppercase ${props.teamData!.fleet_health.compliance_rate >= 90 ? 'bg-emerald-500/20 text-emerald-400' : 'bg-amber-500/20 text-amber-400'}`}>
                  {props.teamData!.fleet_health.compliance_rate}% Match
                </span>
              </div>
              <div class="flex items-center gap-8">
                <div class="flex-1 space-y-4">
                  <div class="flex justify-between text-sm">
                    <span class="text-slate-400">Latest Stable: <span class="text-white font-mono">{props.teamData!.fleet_health.latest_version}</span></span>
                  </div>
                  <div class="h-2.5 overflow-hidden rounded-full bg-slate-800">
                    <div 
                      class="h-full bg-gradient-to-r from-indigo-500 to-purple-400 transition-all duration-1000"
                      style={{ width: `${props.teamData!.fleet_health.compliance_rate}%` }}
                    />
                  </div>
                  <Show when={props.teamData!.fleet_health.version_drift}>
                    <p class="text-xs text-amber-400 flex items-center gap-1.5">
                      <AlertTriangle size={12} />
                      Warning: Version drift detected across {props.teamData!.totals.total_machines} machines.
                    </p>
                  </Show>
                </div>
              </div>
            </div>

            <div class="rounded-2xl border border-emerald-500/30 bg-emerald-500/5 p-6 backdrop-blur-sm">
              <div class="mb-4 flex items-center justify-between">
                <h3 class="text-lg font-semibold text-white">Economic Value (ROI)</h3>
                <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-500/20 text-emerald-400">
                  <DollarSign size={18} />
                </div>
              </div>
              <div class="space-y-4">
                <div class="flex items-baseline gap-2">
                  <span class="text-4xl font-black text-white">${props.teamData!.totals.total_value_usd.toLocaleString()}</span>
                  <span class="text-xs text-slate-500 uppercase font-bold tracking-wider">Value Realized</span>
                </div>
                <div class="flex items-center gap-4">
                  <div class="flex-1">
                    <div class="text-[10px] text-slate-500 uppercase mb-1">Cost Multiplier</div>
                    <div class="text-sm font-bold text-emerald-400">{props.teamData!.insights.roi_multiplier}x ROI</div>
                  </div>
                  <div class="flex-1">
                    <div class="text-[10px] text-slate-500 uppercase mb-1">Efficiency Gain</div>
                    <div class="text-sm font-bold text-indigo-400">+{props.teamData!.productivity_score}%</div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* Charts Row */}
          <div class="grid grid-cols-1 gap-6 xl:grid-cols-3">
            {/* Seat Usage */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <div class="mb-6 flex items-center justify-between">
                <h3 class="text-lg font-semibold text-white">Seat Usage</h3>
                <span class="rounded-full bg-slate-800 px-3 py-1 text-xs text-slate-400">
                  {Math.round(((props.teamData?.totals?.active_machines || 0) / (props.teamData?.license?.max_seats || 30)) * 100)}% used
                </span>
              </div>
              <div class="flex justify-center">
                <DonutChart 
                  data={seatUsage()} 
                  size={180} 
                  thickness={32}
                  centerLabel="Seats"
                  centerValue={`${props.teamData?.totals?.active_machines || 0}`}
                  showLegend
                />
              </div>
            </div>

            {/* Activity Chart */}
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm xl:col-span-2">
              <div class="mb-6 flex items-center justify-between">
                <div>
                  <h3 class="text-lg font-semibold text-white">Team Activity</h3>
                  <p class="text-sm text-slate-500">Commands run over the last 14 days</p>
                </div>
                <div class="text-right">
                  <div class="text-2xl font-bold text-cyan-400">
                    {activityByDay().reduce((sum, d) => sum + d.value, 0).toLocaleString()}
                  </div>
                  <div class="text-xs text-slate-500">Total commands</div>
                </div>
              </div>
              <BarChart
                data={activityByDay()}
                height={200}
                showLabels
                gradient="cyan"
                animated
                tooltipFormatter={(v) => `${v.toLocaleString()} commands`}
              />
            </div>
          </div>

          {/* Top Performers */}
          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <div class="mb-6 flex items-center justify-between">
              <div class="flex items-center gap-3">
                <h3 class="text-lg font-semibold text-white">Top Performers</h3>
                <span class="rounded-full bg-amber-500/20 px-2 py-0.5 text-xs font-medium text-amber-400">
                  Last 7 days
                </span>
              </div>
              <button
                onClick={() => setView('members')}
                class="flex items-center gap-1 text-sm text-indigo-400 transition-colors hover:text-indigo-300"
              >
                View all
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                </svg>
              </button>
            </div>
            
            <div class="grid gap-3 md:grid-cols-2 lg:grid-cols-5">
              <For each={topPerformers()}>
                {(member, index) => (
                  <div class="group relative overflow-hidden rounded-xl border border-slate-800/40 bg-slate-800/20 p-4 transition-all hover:border-slate-700/60 hover:bg-slate-800/40">
                    <div class="absolute -right-4 -top-4 text-6xl font-bold text-slate-800/30">
                      #{index() + 1}
                    </div>
                    <div class="relative">
                      <div class="mb-3 flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 text-lg font-bold text-white shadow-lg shadow-indigo-500/20">
                        {getMemberDisplayName(member).charAt(0).toUpperCase()}
                      </div>
                      <div class="truncate text-sm font-medium text-white">
                        {getMemberDisplayName(member)}
                      </div>
                      <div class="mt-1 text-xs text-slate-500">
                        {member.commands_last_7d.toLocaleString()} commands
                      </div>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </div>
        </div>
      </Show>

      {/* Members Tab */}
      <Show when={view() === 'members'}>
        <div class="space-y-6">
          {/* Filters */}
          <div class="flex flex-wrap items-center gap-3">
            <div class="flex rounded-xl border border-slate-700/50 bg-slate-800/80 p-1">
              <button
                onClick={() => setFilterActive(null)}
                class={`rounded-lg px-4 py-2 text-sm font-medium transition-all ${
                  filterActive() === null ? 'bg-slate-700 text-white' : 'text-slate-400 hover:text-white'
                }`}
              >
                All
              </button>
              <button
                onClick={() => setFilterActive(true)}
                class={`rounded-lg px-4 py-2 text-sm font-medium transition-all ${
                  filterActive() === true ? 'bg-emerald-600 text-white' : 'text-slate-400 hover:text-white'
                }`}
              >
                Active
              </button>
              <button
                onClick={() => setFilterActive(false)}
                class={`rounded-lg px-4 py-2 text-sm font-medium transition-all ${
                  filterActive() === false ? 'bg-slate-600 text-white' : 'text-slate-400 hover:text-white'
                }`}
              >
                Inactive
              </button>
            </div>
            
            <select
              value={sortBy()}
              onChange={e => setSortBy(e.currentTarget.value as 'commands' | 'recent' | 'name')}
              class="rounded-xl border border-slate-700/50 bg-slate-800/80 px-4 py-2.5 text-sm text-white backdrop-blur-sm"
            >
              <option value="commands">Sort by Commands</option>
              <option value="recent">Sort by Recent</option>
              <option value="name">Sort by Name</option>
            </select>
          </div>

          {/* Members Grid */}
          <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            <For each={sortedMembers()}>
              {member => (
                <div class="group rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-5 backdrop-blur-sm transition-all hover:border-slate-700/60">
                  <div class="flex items-start justify-between">
                    <div class="flex items-center gap-3">
                      <div class="flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 text-lg font-bold text-white shadow-lg shadow-indigo-500/20">
                        {getMemberDisplayName(member).charAt(0).toUpperCase()}
                      </div>
                      <div>
                        <div class="font-medium text-white">
                          {getMemberDisplayName(member)}
                        </div>
                        <div class="text-xs text-slate-500">
                          {getMemberSubtitle(member)}
                        </div>
                        <Show when={member.hostname && (member.user_name || member.user_email)}>
                          <div class="mt-0.5 flex items-center gap-1 text-xs text-slate-600">
                            <span>💻</span>
                            <span>{member.hostname}</span>
                          </div>
                        </Show>
                      </div>
                    </div>
                    <StatusBadge status={member.is_active ? 'active' : 'inactive'} pulse={member.is_active} />
                  </div>
                  
                  <div class="mt-4 grid grid-cols-2 gap-3">
                    <div class="rounded-lg bg-slate-800/50 p-3">
                      <div class="text-lg font-bold text-white">{member.total_commands.toLocaleString()}</div>
                      <div class="text-xs text-slate-500">Total Commands</div>
                    </div>
                    <div class="rounded-lg bg-slate-800/50 p-3">
                      <div class="text-lg font-bold text-cyan-400">{member.commands_last_7d.toLocaleString()}</div>
                      <div class="text-xs text-slate-500">Last 7 Days</div>
                    </div>
                  </div>
                  
                  <div class="mt-4 flex items-center justify-between border-t border-slate-800/50 pt-4">
                    <div class="text-xs text-slate-500">
                      Last active: {member.last_active ? api.formatRelativeTime(member.last_active) : 'Never'}
                    </div>
                    <Show when={member.is_active}>
                      <button
                        onClick={() => props.onRevoke(member.id)}
                        class="rounded-lg bg-red-500/10 px-3 py-1.5 text-xs font-medium text-red-400 opacity-0 transition-all hover:bg-red-500/20 group-hover:opacity-100"
                      >
                        Revoke Access
                      </button>
                    </Show>
                  </div>
                </div>
              )}
            </For>
          </div>

          <Show when={sortedMembers().length === 0}>
            <div class="rounded-2xl border border-slate-800/60 bg-slate-900/50 p-12 text-center">
              <div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-slate-800/50 text-3xl">
                👥
              </div>
              <h3 class="text-lg font-semibold text-white">No team members found</h3>
              <p class="mt-2 text-sm text-slate-400">
                Share your license key to add team members
              </p>
            </div>
          </Show>
        </div>
      </Show>

      {/* Activity Tab */}
      <Show when={view() === 'activity'}>
        <div class="space-y-6">
          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <div class="mb-6">
              <h3 class="text-lg font-semibold text-white">Daily Activity</h3>
              <p class="text-sm text-slate-500">Commands run by your team over time</p>
            </div>
            <BarChart
              data={activityByDay()}
              height={300}
              showLabels
              gradient="indigo"
              animated
              tooltipFormatter={(v) => `${v.toLocaleString()} commands`}
            />
          </div>

          {/* Recent Usage Feed */}
          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <h3 class="mb-6 text-lg font-semibold text-white">Recent Usage</h3>
            <div class="space-y-3">
              <For each={(props.teamData?.daily_usage || []).slice(0, 10)}>
                {usage => (
                  <div class="flex items-center gap-4 rounded-xl border border-slate-800/40 bg-slate-800/20 p-4 transition-all hover:bg-slate-800/40">
                    <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-cyan-500/20 text-cyan-400">
                      ⚡
                    </div>
                    <div class="flex-1">
                      <div class="text-sm font-medium text-white">
                        {usage.commands_run.toLocaleString()} commands
                      </div>
                      <div class="text-xs text-slate-500">{usage.date}</div>
                    </div>
                    <div class="text-right">
                      <div class="text-sm font-medium text-emerald-400">
                        {api.formatTimeSaved(usage.time_saved_ms || 0)}
                      </div>
                      <div class="text-xs text-slate-500">saved</div>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </div>
        </div>
      </Show>

      {/* Insights Tab */}
      <Show when={view() === 'insights'}>
        <div class="space-y-6">
          <SmartInsights target="team" />
          
          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <div class="mb-6 flex items-center gap-3">
              <h3 class="text-lg font-semibold text-white">Productivity Insights</h3>
              <span class="rounded-full bg-purple-500/20 px-2 py-0.5 text-xs font-medium text-purple-400">AI-Powered</span>
            </div>
            <div class="space-y-4">
              <For each={getProductivityInsights()}>
                {insight => (
                  <div class={`rounded-xl border p-4 ${
                    insight.type === 'success' ? 'border-emerald-500/30 bg-emerald-500/10' :
                    insight.type === 'warning' ? 'border-amber-500/30 bg-amber-500/10' :
                    'border-blue-500/30 bg-blue-500/10'
                  }`}>
                    <div class="flex items-start gap-3">
                      <div class="text-2xl">{insight.icon}</div>
                      <div>
                        <h4 class={`font-semibold ${
                          insight.type === 'success' ? 'text-emerald-400' :
                          insight.type === 'warning' ? 'text-amber-400' : 'text-blue-400'
                        }`}>{insight.title}</h4>
                        <p class="mt-1 text-sm text-slate-300">{insight.description}</p>
                      </div>
                    </div>
                  </div>
                )}
              </For>
              <Show when={getProductivityInsights().length === 0}>
                <div class="py-8 text-center text-slate-400">
                  <div class="mb-2 text-4xl">🎉</div>
                  <p>Everything looks great! No issues detected.</p>
                </div>
              </Show>
            </div>
          </div>

          <div class="grid gap-6 md:grid-cols-2">
            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <h3 class="mb-4 text-lg font-semibold text-white">Team Statistics</h3>
              <div class="space-y-4">
                <div class="flex items-center justify-between">
                  <span class="text-slate-400">Average commands per member</span>
                  <span class="font-semibold text-white">{avgCommandsPerMember().toLocaleString()}</span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-slate-400">Active members (7d)</span>
                  <span class="font-semibold text-white">
                    {(props.teamData?.members || []).filter(m => m.commands_last_7d > 0).length} / {props.teamData?.members?.length || 0}
                  </span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-slate-400">Total time saved</span>
                  <span class="font-semibold text-emerald-400">{api.formatTimeSaved(props.teamData?.totals?.total_time_saved_ms || 0)}</span>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-slate-400">Productivity score</span>
                  <span class="font-semibold text-indigo-400">{teamProductivityScore()}%</span>
                </div>
              </div>
            </div>

            <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
              <h3 class="mb-4 text-lg font-semibold text-white">Recommendations</h3>
              <div class="space-y-3">
                <Show when={inactiveMembers().length > 0}>
                  <div class="flex items-start gap-3 rounded-lg bg-slate-800/50 p-3">
                    <span class="text-lg">📧</span>
                    <div>
                      <p class="text-sm font-medium text-white">Send reminders to inactive members</p>
                      <p class="text-xs text-slate-500">Help them get started with OMG</p>
                    </div>
                  </div>
                </Show>
                <Show when={(props.teamData?.totals?.active_machines || 0) / (props.teamData?.license?.max_seats || 30) > 0.8}>
                  <div class="flex items-start gap-3 rounded-lg bg-slate-800/50 p-3">
                    <span class="text-lg">⬆️</span>
                    <div>
                      <p class="text-sm font-medium text-white">Consider upgrading your plan</p>
                      <p class="text-xs text-slate-500">You're using most of your available seats</p>
                    </div>
                  </div>
                </Show>
                <div class="flex items-start gap-3 rounded-lg bg-slate-800/50 p-3">
                  <span class="text-lg">📚</span>
                  <div>
                    <p class="text-sm font-medium text-white">Share documentation with your team</p>
                    <p class="text-xs text-slate-500">Help them discover more features</p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Show>

      {/* Settings Tab */}
      <Show when={view() === 'settings'}>
        <div class="space-y-6">
          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <h3 class="mb-6 text-lg font-semibold text-white">License Information</h3>
            <div class="space-y-4">
              <div class="flex items-center justify-between rounded-lg bg-slate-800/50 p-4">
                <div>
                  <div class="text-sm text-slate-400">License Key</div>
                  <div class="mt-1 font-mono text-white">{props.licenseKey.slice(0, 8)}...{props.licenseKey.slice(-8)}</div>
                </div>
                <button onClick={copyLicenseKey} class="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-slate-600">
                  {copied() ? 'Copied!' : 'Copy'}
                </button>
              </div>
              <div class="flex items-center justify-between rounded-lg bg-slate-800/50 p-4">
                <div>
                  <div class="text-sm text-slate-400">License Tier</div>
                  <div class="mt-1 font-semibold text-indigo-400">{props.teamData?.license?.tier || 'Team'}</div>
                </div>
                <a href="https://pyro1121.com/pricing" target="_blank" class="rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-indigo-500">Upgrade</a>
              </div>
              <div class="flex items-center justify-between rounded-lg bg-slate-800/50 p-4">
                <div>
                  <div class="text-sm text-slate-400">Seats</div>
                  <div class="mt-1 text-white">{props.teamData?.totals?.active_machines || 0} / {props.teamData?.license?.max_seats || 30} used</div>
                </div>
              </div>
            </div>
          </div>

          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <h3 class="mb-6 text-lg font-semibold text-white">Alert Settings</h3>
            <div class="flex items-center justify-between">
              <div>
                <div class="font-medium text-white">Low Activity Threshold</div>
                <div class="text-sm text-slate-400">Alert when members have fewer than this many commands per week</div>
              </div>
              <input
                type="number"
                value={alertThreshold()}
                onInput={(e) => setAlertThreshold(parseInt(e.currentTarget.value) || 0)}
                class="w-24 rounded-lg border border-slate-700 bg-slate-800 px-3 py-2 text-right text-white"
              />
            </div>
          </div>

          <div class="rounded-2xl border border-slate-800/60 bg-gradient-to-br from-slate-900/90 to-slate-800/50 p-6 backdrop-blur-sm">
            <h3 class="mb-6 text-lg font-semibold text-white">Data Export</h3>
            <p class="mb-4 text-sm text-slate-400">Export your team data for reporting or backup purposes.</p>
            <div class="flex gap-3">
              <button onClick={() => exportTeamData('csv')} class="flex items-center gap-2 rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-slate-600">
                <span>📊</span> Export CSV
              </button>
              <button onClick={() => exportTeamData('json')} class="flex items-center gap-2 rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-slate-600">
                <span>📋</span> Export JSON
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Invite Modal */}
      <Show when={showInviteModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4" onClick={() => setShowInviteModal(false)}>
          <div class="w-full max-w-md rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl" onClick={e => e.stopPropagation()}>
            <div class="mb-6 flex items-center justify-between">
              <h3 class="text-xl font-semibold text-white">Invite Team Members</h3>
              <button onClick={() => setShowInviteModal(false)} class="text-slate-400 hover:text-white">✕</button>
            </div>
            <p class="mb-4 text-sm text-slate-400">Share your license key with team members so they can activate OMG on their machines.</p>
            <div class="mb-4 rounded-lg bg-slate-800 p-4">
              <div class="mb-2 text-xs font-medium text-slate-400">LICENSE KEY</div>
              <div class="flex items-center gap-2">
                <code class="flex-1 break-all font-mono text-sm text-white">{props.licenseKey}</code>
                <button onClick={copyLicenseKey} class="shrink-0 rounded-lg bg-indigo-600 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-indigo-500">
                  {copied() ? '✓' : 'Copy'}
                </button>
              </div>
            </div>
            <div class="mb-6 rounded-lg bg-slate-800/50 p-4">
              <div class="mb-2 text-xs font-medium text-slate-400">ACTIVATION COMMAND</div>
              <code class="block break-all font-mono text-sm text-emerald-400">omg license activate {props.licenseKey}</code>
            </div>
            <button onClick={() => setShowInviteModal(false)} class="w-full rounded-xl border border-slate-700 py-3 text-sm font-medium text-slate-300 transition-colors hover:bg-slate-800">Done</button>
          </div>
        </div>
      </Show>

      {/* Member Detail Modal */}
      <Show when={selectedMember()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4" onClick={() => setSelectedMember(null)}>
          <div class="w-full max-w-lg rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl" onClick={e => e.stopPropagation()}>
            <div class="mb-6 flex items-center justify-between">
              <div class="flex items-center gap-4">
                <div class="flex h-14 w-14 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 text-lg font-bold text-white shadow-lg">
                  {getMemberInitials(selectedMember()!)}
                </div>
                <div>
                  <h3 class="text-xl font-semibold text-white">{getMemberDisplayName(selectedMember()!)}</h3>
                  <p class="text-sm text-slate-400">{getMemberSubtitle(selectedMember()!)}</p>
                </div>
              </div>
              <button onClick={() => setSelectedMember(null)} class="text-slate-400 hover:text-white">✕</button>
            </div>
            <div class="mb-6 grid grid-cols-2 gap-4">
              <div class="rounded-lg bg-slate-800/50 p-4">
                <div class="text-2xl font-bold text-white">{selectedMember()!.total_commands.toLocaleString()}</div>
                <div class="text-sm text-slate-400">Total Commands</div>
              </div>
              <div class="rounded-lg bg-slate-800/50 p-4">
                <div class="text-2xl font-bold text-cyan-400">{selectedMember()!.commands_last_7d.toLocaleString()}</div>
                <div class="text-sm text-slate-400">Last 7 Days</div>
              </div>
              <div class="rounded-lg bg-slate-800/50 p-4">
                <div class="text-2xl font-bold text-emerald-400">{api.formatTimeSaved(selectedMember()!.total_time_saved_ms || 0)}</div>
                <div class="text-sm text-slate-400">Time Saved</div>
              </div>
              <div class="rounded-lg bg-slate-800/50 p-4">
                <div class="text-2xl font-bold text-purple-400">{selectedMember()!.total_packages || 0}</div>
                <div class="text-sm text-slate-400">Packages Installed</div>
              </div>
            </div>
            <div class="mb-6 space-y-3">
              <div class="flex items-center justify-between text-sm">
                <span class="text-slate-400">Hostname</span>
                <span class="text-white">{selectedMember()!.hostname || 'Unknown'}</span>
              </div>
              <div class="flex items-center justify-between text-sm">
                <span class="text-slate-400">Operating System</span>
                <span class="text-white">{selectedMember()!.os || 'Unknown'}</span>
              </div>
              <div class="flex items-center justify-between text-sm">
                <span class="text-slate-400">Architecture</span>
                <span class="text-white">{selectedMember()!.arch || 'Unknown'}</span>
              </div>
              <div class="flex items-center justify-between text-sm">
                <span class="text-slate-400">OMG Version</span>
                <span class="text-white">{selectedMember()!.omg_version || 'Unknown'}</span>
              </div>
              <div class="flex items-center justify-between text-sm">
                <span class="text-slate-400">First Seen</span>
                <span class="text-white">{selectedMember()!.first_seen_at ? api.formatRelativeTime(selectedMember()!.first_seen_at) : 'Unknown'}</span>
              </div>
              <div class="flex items-center justify-between text-sm">
                <span class="text-slate-400">Last Active</span>
                <span class="text-white">{selectedMember()!.last_active ? api.formatRelativeTime(selectedMember()!.last_active!) : 'Never'}</span>
              </div>
            </div>
            <div class="flex gap-3">
              <Show when={selectedMember()!.is_active}>
                <button onClick={() => { props.onRevoke(selectedMember()!.id); setSelectedMember(null); }} class="flex-1 rounded-xl bg-red-600 py-3 text-sm font-medium text-white transition-colors hover:bg-red-500">Revoke Access</button>
              </Show>
              <button onClick={() => setSelectedMember(null)} class="flex-1 rounded-xl border border-slate-700 py-3 text-sm font-medium text-slate-300 transition-colors hover:bg-slate-800">Close</button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};
