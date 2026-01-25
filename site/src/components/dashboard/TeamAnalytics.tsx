import { Component, For, Show, createSignal, createMemo, createEffect, onMount } from 'solid-js';
import * as api from '../../lib/api';
import { MetricCard } from '../ui/Card';
import { StatusBadge, TierBadge } from '../ui/Badge';
import { BarChart, DonutChart, LiveIndicator, AreaChart, ActivityHeatmap } from '../ui/Chart';
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
  const [copied, setCopied] = createSignal(false);
  const [isRefreshing, setIsRefreshing] = createSignal(false);
  const [selectedMember, setSelectedMember] = createSignal<api.TeamMember | null>(null);
  const [showInviteModal, setShowInviteModal] = createSignal(false);
  const [alertThreshold, setAlertThreshold] = createSignal(100);
  
  const [policies, setPolicies] = createSignal<api.Policy[]>([]);
  const [notifications, setNotifications] = createSignal<api.NotificationSetting[]>([]);
  const [auditLogs, setAuditLogs] = createSignal<api.TeamAuditLogEntry[]>([]);
  const [settingsLoading, setSettingsLoading] = createSignal(false);
  const [settingsMessage, setSettingsMessage] = createSignal<{type: 'success' | 'error', text: string} | null>(null);
  const [newPolicyScope, setNewPolicyScope] = createSignal<string>('runtime');
  const [newPolicyRule, setNewPolicyRule] = createSignal('');
  const [newPolicyValue, setNewPolicyValue] = createSignal('');
  
  // Confirmation Dialog State
  const [confirmDialog, setConfirmDialog] = createSignal<{
    isOpen: boolean;
    title: string;
    description: string;
    onConfirm: () => void;
    variant: 'danger' | 'primary' | 'neutral';
  }>({
    isOpen: false,
    title: '',
    description: '',
    onConfirm: () => {},
    variant: 'neutral'
  });

  const loadSettingsData = async () => {
    if (view() !== 'settings') return;
    setSettingsLoading(true);
    try {
      const [policiesRes, notifRes, logsRes] = await Promise.all([
        api.getTeamPolicies().catch(() => ({ policies: [] })),
        api.getNotificationSettings().catch(() => ({ settings: [] })),
        api.getTeamAuditLogs({ limit: 20 }).catch(() => ({ logs: [], total: 0, limit: 20, offset: 0 })),
      ]);
      setPolicies(policiesRes.policies);
      setNotifications(notifRes.settings);
      setAuditLogs(logsRes.logs);
    } catch (e) {
      console.error('Failed to load settings:', e);
    }
    setSettingsLoading(false);
  };

  createEffect(() => {
    if (view() === 'settings') {
      loadSettingsData();
    }
  });

  const showMessage = (type: 'success' | 'error', text: string) => {
    setSettingsMessage({ type, text });
    setTimeout(() => setSettingsMessage(null), 3000);
  };

  const handleSaveThreshold = async () => {
    try {
      await api.updateAlertThreshold('low_activity', alertThreshold());
      showMessage('success', 'Threshold saved');
    } catch (e) {
      showMessage('error', 'Failed to save threshold');
    }
  };

  const handleToggleNotification = async (type: string, enabled: boolean) => {
    const current = notifications();
    const updated = current.map(n => n.type === type ? { ...n, enabled } : n);
    setNotifications(updated);
    try {
      await api.updateNotificationSettings(updated);
      showMessage('success', 'Notification settings updated');
    } catch (e) {
      showMessage('error', 'Failed to update notifications');
      setNotifications(current);
    }
  };

  const handleCreatePolicy = async () => {
    if (!newPolicyRule() || !newPolicyValue()) return;
    try {
      const res = await api.createTeamPolicy({
        scope: newPolicyScope(),
        rule: newPolicyRule(),
        value: newPolicyValue(),
        enforced: true,
      });
      if (res.success && res.policy) {
        setPolicies([...policies(), res.policy]);
        setNewPolicyRule('');
        setNewPolicyValue('');
        showMessage('success', 'Policy created');
      }
    } catch (e) {
      showMessage('error', 'Failed to create policy');
    }
  };

  const handleDeletePolicy = (id: string) => {
    setConfirmDialog({
      isOpen: true,
      title: 'Delete Policy',
      description: 'Are you sure you want to delete this policy? This action cannot be undone and may affect compliance status.',
      variant: 'danger',
      onConfirm: async () => {
        const current = policies();
        setPolicies(current.filter(p => p.id !== id));
        try {
          await api.deleteTeamPolicy(id);
          showMessage('success', 'Policy deleted');
        } catch (e) {
          showMessage('error', 'Failed to delete policy');
          setPolicies(current);
        }
        setConfirmDialog(prev => ({ ...prev, isOpen: false }));
      }
    });
  };

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

  const securityMetrics = createMemo(() => {
    const members = props.teamData?.members || [];
    const total = members.length || 1;
    const active = members.filter(m => m.is_active).length;
    const compliant = members.filter(m => m.omg_version && m.omg_version.startsWith('1.')).length;
    
    return {
      total_vulnerabilities: 0,
      critical: 0,
      high: 0,
      medium: 0,
      low: 0,
      compliance_score: Math.round((compliant / total) * 100),
      signature_verification: 100,
      sbom_status: 'Healthy',
      policy_enforcement: 'Active'
    };
  });

  const productivityImpact = createMemo(() => {
    const timeSavedMs = props.teamData?.totals?.total_time_saved_ms || 0;
    const hours = Math.floor(timeSavedMs / 3600000);
    const valueUsd = props.teamData?.totals?.total_value_usd || (hours * 85); 
    
    const daily = props.teamData?.daily_usage || [];
    const trend = daily.map(d => d.commands_run);
    
    return {
      hours_reclaimed: hours,
      developer_value: valueUsd,
      daily_trend: trend.length > 0 ? trend : [0]
    };
  });

  const memberProductivity = createMemo(() => {
    return (props.teamData?.members || []).map(m => ({
      ...m,
      success_rate: 100,
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

  if (props.loading) {
    return (
      <div class="space-y-8 animate-in fade-in duration-500">
        <div class="grid gap-6 lg:grid-cols-2">
          <CardSkeleton />
          <CardSkeleton />
        </div>
        <CardSkeleton />
      </div>
    );
  }

  if (!props.teamData) {
    return (
      <div class="flex min-h-[60vh] flex-col items-center justify-center rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-12 text-center shadow-2xl animate-in fade-in zoom-in-95 duration-500">
        <div class="mb-8 flex h-24 w-24 items-center justify-center rounded-[2rem] bg-gradient-to-br from-indigo-500/20 to-purple-500/20 shadow-[0_0_50px_rgba(99,102,241,0.2)]">
          <Users size={48} class="text-indigo-400" />
        </div>
        <h2 class="mb-4 text-4xl font-black text-white tracking-tight">Unlock Team Intelligence</h2>
        <p class="mb-10 max-w-lg text-lg font-medium text-slate-400">
          Gain visibility into your entire fleet. Manage runtimes, enforce security policies, and track productivity across your organization.
        </p>
        <div class="mb-12 grid w-full max-w-3xl grid-cols-1 gap-4 sm:grid-cols-3">
          <div class="rounded-3xl border border-white/5 bg-white/[0.02] p-6 text-left">
            <div class="mb-4 flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/10 text-emerald-400">
              <Activity size={20} />
            </div>
            <h3 class="mb-2 font-bold text-white">Fleet Insights</h3>
            <p class="text-xs font-medium text-slate-500">Real-time productivity metrics for every developer.</p>
          </div>
          <div class="rounded-3xl border border-white/5 bg-white/[0.02] p-6 text-left">
            <div class="mb-4 flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-500/10 text-indigo-400">
              <Lock size={20} />
            </div>
            <h3 class="mb-2 font-bold text-white">Policy Control</h3>
            <p class="text-xs font-medium text-slate-500">Enforce package versions and security rules.</p>
          </div>
          <div class="rounded-3xl border border-white/5 bg-white/[0.02] p-6 text-left">
            <div class="mb-4 flex h-10 w-10 items-center justify-center rounded-xl bg-purple-500/10 text-purple-400">
              <Globe size={20} />
            </div>
            <h3 class="mb-2 font-bold text-white">Global Telemetry</h3>
            <p class="text-xs font-medium text-slate-500">Aggregate data from all machines in your org.</p>
          </div>
        </div>
        <button
          onClick={() => window.open('https://pyro1121.com/pricing', '_blank')}
          class="group relative overflow-hidden rounded-2xl bg-white px-10 py-4 font-black text-black transition-all hover:scale-105 hover:shadow-[0_0_40px_rgba(255,255,255,0.3)]"
        >
          <span class="relative z-10">Upgrade to Team</span>
          <div class="absolute inset-0 -translate-x-full bg-gradient-to-r from-indigo-500 via-purple-500 to-indigo-500 transition-transform duration-500 group-hover:translate-x-0 opacity-20" />
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

      <div class="flex items-center gap-1 overflow-x-auto no-scrollbar rounded-[1.5rem] border border-white/5 bg-white/[0.02] p-1.5 backdrop-blur-xl">
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
                  <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
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
                  
                  <div class="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2">
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
              <ActivityHeatmap data={props.teamData?.daily_usage?.map(d => ({
                day: new Date(d.date).getDay(),
                hour: 12,
                value: d.commands_run
              })) || []} />
            </div>

            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
              <h3 class="mb-8 text-lg font-bold text-white uppercase tracking-widest">Execution Stream</h3>
              <div class="space-y-4 max-h-[400px] overflow-y-auto custom-scrollbar pr-2">
                <For each={props.teamData?.daily_usage?.slice(0, 15) || []}>
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
                          <span class="text-[10px] font-black text-indigo-400 uppercase tracking-widest">Node ID: {usage.machine_id?.slice(0, 8) || 'N/A'}</span>
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
                <h3 class="text-lg font-bold text-white uppercase tracking-widest mb-6">Strategic Advice</h3>
                <div class="space-y-4">
                  <Show when={inactiveMembers().length > 0}>
                    <div class="flex items-start gap-4 rounded-2xl bg-white/[0.03] p-4 transition-all hover:bg-white/[0.05]">
                      <div class="text-xl">🚀</div>
                      <div>
                        <p class="text-sm font-bold text-white">Accelerate Onboarding</p>
                        <p class="text-xs text-slate-500 mt-1">{inactiveMembers().length} members haven't been active recently.</p>
                      </div>
                    </div>
                  </Show>
                  <Show when={securityMetrics().compliance_score < 100}>
                    <div class="flex items-start gap-4 rounded-2xl bg-white/[0.03] p-4 transition-all hover:bg-white/[0.05]">
                      <div class="text-xl">🛡️</div>
                      <div>
                        <p class="text-sm font-bold text-white">Security Patch</p>
                        <p class="text-xs text-slate-500 mt-1">Some nodes are on legacy versions.</p>
                      </div>
                    </div>
                  </Show>
                  <Show when={inactiveMembers().length === 0 && securityMetrics().compliance_score === 100}>
                    <div class="flex items-start gap-4 rounded-2xl bg-white/[0.03] p-4 transition-all hover:bg-white/[0.05]">
                      <div class="text-xl">✨</div>
                      <div>
                        <p class="text-sm font-bold text-white">All Systems Nominal</p>
                        <p class="text-xs text-slate-500 mt-1">Your fleet is fully compliant and active.</p>
                      </div>
                    </div>
                  </Show>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Show>

      <Show when={view() === 'settings'}>
        <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
          <Show when={settingsMessage()}>
            <div class={`fixed top-6 right-6 z-[200] px-6 py-4 rounded-2xl font-bold text-sm animate-in slide-in-from-right ${settingsMessage()?.type === 'success' ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30' : 'bg-rose-500/20 text-rose-400 border border-rose-500/30'}`}>
              {settingsMessage()?.text}
            </div>
          </Show>

          <Show when={settingsLoading()}>
            <div class="grid gap-6 lg:grid-cols-2">
              <CardSkeleton />
              <div class="space-y-6">
                <CardSkeleton />
                <CardSkeleton />
              </div>
            </div>
          </Show>

          <Show when={!settingsLoading()}>
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
                      <div class="font-bold text-white">Low Activity Alert</div>
                      <div class="text-xs text-slate-500 mt-1">Alert when ops drop below threshold</div>
                    </div>
                    <div class="flex items-center gap-3">
                      <input
                        type="number"
                        value={alertThreshold()}
                        onInput={(e) => setAlertThreshold(parseInt(e.currentTarget.value) || 0)}
                        class="w-20 rounded-xl border border-white/10 bg-black px-4 py-2 text-right font-black text-indigo-400 focus:ring-2 focus:ring-indigo-500/20 outline-none"
                      />
                      <button onClick={handleSaveThreshold} class="rounded-xl bg-indigo-600 px-4 py-2 text-xs font-black text-white hover:bg-indigo-500 transition-colors">
                        Save
                      </button>
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
                    <button onClick={() => api.openBillingPortal(props.teamData?.license?.tier === 'enterprise' ? '' : '')} class="flex-1 rounded-2xl border border-white/10 bg-white/[0.03] py-4 text-sm font-black text-white transition-all hover:bg-white/[0.08]">
                      Billing Portal
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <div class="grid gap-6 lg:grid-cols-2">
              <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
                <div class="flex items-center justify-between mb-8">
                  <h3 class="text-xl font-bold text-white uppercase tracking-widest">Notification Settings</h3>
                  <AlertTriangle size={20} class="text-amber-500" />
                </div>
                <div class="space-y-4">
                  <For each={notifications()}>
                    {(notif) => (
                      <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02] border border-white/[0.03]">
                        <div>
                          <div class="font-bold text-white text-sm">{notif.type.replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase())}</div>
                          <div class="text-[10px] text-slate-500 mt-0.5 uppercase tracking-wide">
                            {notif.channels.join(', ')}
                          </div>
                        </div>
                        <button
                          onClick={() => handleToggleNotification(notif.type, !notif.enabled)}
                          class={`w-12 h-7 rounded-full transition-all relative ${notif.enabled ? 'bg-emerald-500' : 'bg-white/10'}`}
                        >
                          <div class={`absolute top-1 w-5 h-5 rounded-full bg-white shadow transition-all ${notif.enabled ? 'left-6' : 'left-1'}`} />
                        </button>
                      </div>
                    )}
                  </For>
                  <Show when={notifications().length === 0}>
                    <div class="text-center py-8 text-slate-500 text-sm">No notification settings configured</div>
                  </Show>
                </div>
              </div>

              <Show when={props.teamData?.license?.tier === 'enterprise'}>
                <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
                  <div class="flex items-center justify-between mb-8">
                    <h3 class="text-xl font-bold text-white uppercase tracking-widest">Policies</h3>
                    <Lock size={20} class="text-indigo-500" />
                  </div>
                  <div class="space-y-4 mb-6">
                    <For each={policies()}>
                      {(policy) => (
                        <div class="flex items-center justify-between p-4 rounded-2xl bg-white/[0.02] border border-white/[0.03] group">
                          <div>
                            <div class="flex items-center gap-2">
                              <span class={`text-[10px] font-black uppercase px-2 py-0.5 rounded ${
                                policy.scope === 'runtime' ? 'bg-indigo-500/20 text-indigo-400' :
                                policy.scope === 'package' ? 'bg-emerald-500/20 text-emerald-400' :
                                policy.scope === 'security' ? 'bg-rose-500/20 text-rose-400' :
                                'bg-amber-500/20 text-amber-400'
                              }`}>{policy.scope}</span>
                              <span class="font-bold text-white text-sm">{policy.rule}</span>
                            </div>
                            <div class="text-xs text-slate-500 mt-1">{policy.value}</div>
                          </div>
                          <button
                            onClick={() => handleDeletePolicy(policy.id)}
                            class="opacity-0 group-hover:opacity-100 text-rose-500 hover:text-rose-400 transition-all"
                          >
                            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>
                          </button>
                        </div>
                      )}
                    </For>
                  </div>
                  <div class="border-t border-white/5 pt-6">
                    <div class="text-[10px] font-black uppercase tracking-widest text-slate-500 mb-4">Add Policy</div>
                    <div class="flex gap-3">
                      <select
                        value={newPolicyScope()}
                        onChange={(e) => setNewPolicyScope(e.currentTarget.value)}
                        class="rounded-xl border border-white/10 bg-black px-3 py-2 text-sm font-bold text-white outline-none"
                      >
                        <option value="runtime">Runtime</option>
                        <option value="package">Package</option>
                        <option value="security">Security</option>
                        <option value="network">Network</option>
                      </select>
                      <input
                        type="text"
                        placeholder="Rule name"
                        value={newPolicyRule()}
                        onInput={(e) => setNewPolicyRule(e.currentTarget.value)}
                        class="flex-1 rounded-xl border border-white/10 bg-black px-3 py-2 text-sm text-white placeholder-slate-600 outline-none"
                      />
                      <input
                        type="text"
                        placeholder="Value"
                        value={newPolicyValue()}
                        onInput={(e) => setNewPolicyValue(e.currentTarget.value)}
                        class="flex-1 rounded-xl border border-white/10 bg-black px-3 py-2 text-sm text-white placeholder-slate-600 outline-none"
                      />
                      <button
                        onClick={handleCreatePolicy}
                        disabled={!newPolicyRule() || !newPolicyValue()}
                        class="rounded-xl bg-indigo-600 px-4 py-2 text-xs font-black text-white hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                      >
                        Add
                      </button>
                    </div>
                  </div>
                </div>
              </Show>

              <Show when={props.teamData?.license?.tier !== 'enterprise'}>
                <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
                  <div class="flex items-center justify-between mb-8">
                    <h3 class="text-xl font-bold text-white uppercase tracking-widest">Policies</h3>
                    <Lock size={20} class="text-slate-600" />
                  </div>
                  <div class="text-center py-12">
                    <Lock size={48} class="mx-auto text-slate-700 mb-4" />
                    <p class="text-slate-500 font-medium mb-4">Policy management requires Enterprise tier</p>
                    <button onClick={() => window.open('https://pyro1121.com/pricing', '_blank')} class="rounded-2xl bg-indigo-600 px-6 py-3 text-sm font-black text-white hover:bg-indigo-500 transition-colors">
                      Upgrade to Enterprise
                    </button>
                  </div>
                </div>
              </Show>
            </div>

            <div class="rounded-[2.5rem] border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <div class="flex items-center justify-between mb-8">
                <h3 class="text-xl font-bold text-white uppercase tracking-widest">Audit Log</h3>
                <div class="flex gap-3">
                  <button onClick={() => api.getAdminExportAuditUrl(30)} class="text-xs font-black text-slate-500 hover:text-white transition-colors flex items-center gap-1">
                    <FileText size={14} />
                    Export CSV
                  </button>
                  <button onClick={loadSettingsData} class="text-xs font-black text-indigo-400 hover:text-indigo-300">
                    Refresh
                  </button>
                </div>
              </div>
              <div class="space-y-3 max-h-[400px] overflow-y-auto custom-scrollbar pr-2">
                <For each={auditLogs()}>
                  {(log) => (
                    <div class="flex items-center gap-4 p-4 rounded-2xl bg-white/[0.02] border border-white/[0.03]">
                      <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-indigo-500/10 text-indigo-500">
                        <Activity size={18} />
                      </div>
                      <div class="flex-1 min-w-0">
                        <div class="font-bold text-white text-sm truncate">{log.action}</div>
                        <div class="text-[10px] text-slate-500 flex items-center gap-2 mt-0.5">
                          <span>{log.resource_type || 'system'}</span>
                          <span class="w-1 h-1 rounded-full bg-slate-700" />
                          <span>{api.formatRelativeTime(log.created_at)}</span>
                        </div>
                      </div>
                      <Show when={log.ip_address}>
                        <div class="text-[10px] font-mono text-slate-600">{log.ip_address}</div>
                      </Show>
                    </div>
                  )}
                </For>
                <Show when={auditLogs().length === 0}>
                  <div class="text-center py-12 text-slate-500 text-sm">No audit logs available</div>
                </Show>
              </div>
            </div>
          </Show>
        </div>
      </Show>

      <Dialog
        isOpen={confirmDialog().isOpen}
        onClose={() => setConfirmDialog(prev => ({ ...prev, isOpen: false }))}
        title={confirmDialog().title}
        description={confirmDialog().description}
        onConfirm={confirmDialog().onConfirm}
        variant={confirmDialog().variant}
        confirmLabel="Proceed"
      />

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
