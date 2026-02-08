import { Component, createResource, For, Show } from 'solid-js';
import GlassCard from '../../ui/GlassCard';
import { StatCard } from '../analytics/StatCard';
import { Sparkline } from '../../ui/Sparkline';
import {
  Terminal,
  TrendingUp,
  TrendingDown,
  Monitor,
  CheckCircle,
  Package,
  Search,
  Repeat,
  Shield,
  Zap,
  Loader2,
  AlertCircle,
} from '../../ui/Icons';

interface CommandDistribution {
  command: string;
  count: number;
  percentage: number;
}

interface FeatureAdoption {
  feature: string;
  adopted: boolean;
  usageCount: number;
  lastUsed: string | null;
}

interface DailyUsage {
  date: string;
  commands: number;
  timeSavedMs: number;
}

interface UsageData {
  totalCommandsThisMonth: number;
  commandsThisWeek: number;
  commandsTrend: number;
  activeMachinesCount: number;
  totalMachinesCount: number;
  commandDistribution: CommandDistribution[];
  featureAdoption: FeatureAdoption[];
  dailyUsage: DailyUsage[];
}

async function fetchUsageData(): Promise<UsageData> {
  const response = await fetch('/api/dashboard/usage', {
    credentials: 'include',
  });
  if (!response.ok) {
    throw new Error('Failed to fetch usage data');
  }
  return response.json();
}

const getCommandIcon = (command: string) => {
  switch (command) {
    case 'install':
      return Package;
    case 'search':
      return Search;
    case 'update':
      return Repeat;
    case 'remove':
      return Package;
    case 'use':
      return Zap;
    case 'sbom':
    case 'audit':
      return Shield;
    default:
      return Terminal;
  }
};

const getCommandColor = (command: string) => {
  const colors: Record<string, { bg: string; text: string; bar: string }> = {
    install: { bg: 'bg-indigo-500/20', text: 'text-indigo-400', bar: 'bg-indigo-500' },
    search: { bg: 'bg-cyan-500/20', text: 'text-cyan-400', bar: 'bg-cyan-500' },
    update: { bg: 'bg-emerald-500/20', text: 'text-emerald-400', bar: 'bg-emerald-500' },
    remove: { bg: 'bg-rose-500/20', text: 'text-rose-400', bar: 'bg-rose-500' },
    use: { bg: 'bg-purple-500/20', text: 'text-purple-400', bar: 'bg-purple-500' },
    info: { bg: 'bg-amber-500/20', text: 'text-amber-400', bar: 'bg-amber-500' },
    list: { bg: 'bg-blue-500/20', text: 'text-blue-400', bar: 'bg-blue-500' },
    sbom: { bg: 'bg-green-500/20', text: 'text-green-400', bar: 'bg-green-500' },
    audit: { bg: 'bg-orange-500/20', text: 'text-orange-400', bar: 'bg-orange-500' },
  };
  return colors[command] || { bg: 'bg-slate-500/20', text: 'text-slate-400', bar: 'bg-slate-500' };
};

const getFeatureDisplayName = (feature: string) => {
  const names: Record<string, string> = {
    aur: 'AUR Support',
    daemon: 'Daemon Mode',
    sbom: 'SBOM Generation',
    fleet: 'Fleet Management',
    runtimes: 'Runtime Switching',
    audit: 'Security Audit',
  };
  return names[feature] || feature;
};

const getFeatureIcon = (feature: string) => {
  const icons: Record<string, typeof Package> = {
    aur: Package,
    daemon: Zap,
    sbom: Shield,
    fleet: Monitor,
    runtimes: Repeat,
    audit: Shield,
  };
  return icons[feature] || Terminal;
};

export const UsageOverview: Component = () => {
  const [usage] = createResource(fetchUsageData);

  return (
    <div class="space-y-6">
      <Show when={usage.loading}>
        <div class="flex items-center justify-center py-20">
          <Loader2 size={32} class="animate-spin text-indigo-400" />
        </div>
      </Show>

      <Show when={usage.error}>
        <GlassCard class="p-6">
          <div class="flex items-center gap-3 text-rose-400">
            <AlertCircle size={24} />
            <span>Failed to load usage data. Please try again later.</span>
          </div>
        </GlassCard>
      </Show>

      <Show when={usage()}>
        {(data) => (
          <>
            {/* Summary Stats */}
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
              <StatCard
                title="Commands This Month"
                value={data().totalCommandsThisMonth.toLocaleString()}
                icon={<Terminal size={24} />}
                trend={{
                  value: Math.abs(data().commandsTrend),
                  isUp: data().commandsTrend >= 0,
                }}
                description="Total CLI commands executed"
              />

              <StatCard
                title="Commands This Week"
                value={data().commandsThisWeek.toLocaleString()}
                icon={<Zap size={24} />}
                description="Activity in the last 7 days"
              />

              <StatCard
                title="Active Machines"
                value={`${data().activeMachinesCount} / ${data().totalMachinesCount}`}
                icon={<Monitor size={24} />}
                description="Machines active in the last 24h"
              />

              <StatCard
                title="Features Adopted"
                value={`${data().featureAdoption.filter(f => f.adopted).length} / ${data().featureAdoption.length}`}
                icon={<CheckCircle size={24} />}
                description="Advanced features in use"
              />
            </div>

            {/* Usage Trend Sparkline */}
            <GlassCard class="p-6">
              <div class="flex items-center justify-between mb-4">
                <div>
                  <h3 class="text-lg font-bold text-white">Daily Activity</h3>
                  <p class="text-sm text-slate-400">Command execution over the last 30 days</p>
                </div>
                <div class="flex items-center gap-2">
                  <Show when={data().commandsTrend >= 0} fallback={
                    <TrendingDown size={18} class="text-rose-400" />
                  }>
                    <TrendingUp size={18} class="text-emerald-400" />
                  </Show>
                  <span class={`text-sm font-bold ${data().commandsTrend >= 0 ? 'text-emerald-400' : 'text-rose-400'}`}>
                    {data().commandsTrend >= 0 ? '+' : ''}{data().commandsTrend}% vs last week
                  </span>
                </div>
              </div>
              <Sparkline
                data={data().dailyUsage.map(d => d.commands)}
                width={600}
                height={80}
                color="#6366f1"
                fillOpacity={0.2}
                showDots
              />
            </GlassCard>

            <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
              {/* Command Distribution */}
              <GlassCard class="p-6">
                <h3 class="text-lg font-bold text-white mb-4">Most Used Commands</h3>
                <Show when={data().commandDistribution.length > 0} fallback={
                  <p class="text-slate-400 text-sm">No command data available yet.</p>
                }>
                  <div class="space-y-3">
                    <For each={data().commandDistribution.slice(0, 6)}>
                      {(cmd) => {
                        const colors = getCommandColor(cmd.command);
                        const Icon = getCommandIcon(cmd.command);
                        return (
                          <div class="rounded-lg border border-white/10 bg-white/5 p-3">
                            <div class="flex items-center justify-between mb-2">
                              <div class="flex items-center gap-2">
                                <div class={`rounded-lg ${colors.bg} p-1.5`}>
                                  <Icon size={14} class={colors.text} />
                                </div>
                                <span class="text-sm font-medium text-white capitalize">
                                  {cmd.command}
                                </span>
                              </div>
                              <div class="text-right">
                                <span class={`text-sm font-bold ${colors.text}`}>
                                  {cmd.count.toLocaleString()}
                                </span>
                                <span class="text-xs text-slate-500 ml-1">
                                  ({cmd.percentage}%)
                                </span>
                              </div>
                            </div>
                            <div class="h-1.5 rounded-full bg-white/5 overflow-hidden">
                              <div
                                class={`h-full rounded-full ${colors.bar} transition-all duration-500`}
                                style={{ width: `${cmd.percentage}%` }}
                              />
                            </div>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                </Show>
              </GlassCard>

              {/* Feature Adoption Badges */}
              <GlassCard class="p-6">
                <h3 class="text-lg font-bold text-white mb-4">Feature Adoption</h3>
                <p class="text-sm text-slate-400 mb-4">
                  Unlock the full power of OMG with these advanced features
                </p>
                <div class="grid grid-cols-2 gap-3">
                  <For each={data().featureAdoption}>
                    {(feature) => {
                      const Icon = getFeatureIcon(feature.feature);
                      return (
                        <div
                          class={`rounded-xl border p-4 transition-all ${
                            feature.adopted
                              ? 'border-emerald-500/30 bg-emerald-500/10'
                              : 'border-white/10 bg-white/5 opacity-60'
                          }`}
                        >
                          <div class="flex items-center gap-2 mb-2">
                            <Icon
                              size={18}
                              class={feature.adopted ? 'text-emerald-400' : 'text-slate-500'}
                            />
                            <span
                              class={`text-sm font-medium ${
                                feature.adopted ? 'text-white' : 'text-slate-400'
                              }`}
                            >
                              {getFeatureDisplayName(feature.feature)}
                            </span>
                          </div>
                          <Show when={feature.adopted}>
                            <div class="flex items-center gap-1">
                              <CheckCircle size={12} class="text-emerald-400" />
                              <span class="text-xs text-emerald-400">
                                {feature.usageCount.toLocaleString()} uses
                              </span>
                            </div>
                          </Show>
                          <Show when={!feature.adopted}>
                            <span class="text-xs text-slate-500">Not yet used</span>
                          </Show>
                        </div>
                      );
                    }}
                  </For>
                </div>
              </GlassCard>
            </div>
          </>
        )}
      </Show>
    </div>
  );
};

export default UsageOverview;
