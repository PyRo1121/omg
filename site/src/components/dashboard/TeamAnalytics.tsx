import { Component, For, Show, createSignal } from 'solid-js';
import * as api from '../../lib/api';
import { MetricCard } from '../ui/Card';
import { StatusBadge } from '../ui/Badge';
import { BarChart, DonutChart, LiveIndicator } from '../ui/Chart';
import {
  Users,
  BarChart3,
  TrendingUp,
  Settings,
  AlertTriangle,
  FileText,
  Lightbulb,
} from '../ui/Icons';

interface TeamAnalyticsProps {
  teamData: api.TeamData | null;
  licenseKey: string;
  onRevoke: (machineId: string) => void;
  onRefresh: () => void;
}

export const TeamAnalytics: Component<TeamAnalyticsProps> = props => {
  const [view, setView] = createSignal<'overview' | 'members' | 'activity' | 'insights' | 'settings'>('overview');
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
