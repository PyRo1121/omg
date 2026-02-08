import { Component, createResource, For, Show } from 'solid-js';
import GlassCard from '../../ui/GlassCard';
import { StatCard } from '../analytics/StatCard';
import { Sparkline, TrendIndicator } from '../../ui/Sparkline';
import {
  Zap,
  Search,
  Package,
  Repeat,
  Timer,
  TrendingUp,
  TrendingDown,
  CheckCircle,
  Loader2,
  AlertCircle,
  Activity,
  Clock,
} from '../../ui/Icons';

interface PerformanceMetric {
  metricType: string;
  avgValue: number;
  minValue: number;
  maxValue: number;
  p50Value: number;
  p95Value: number;
  sampleCount: number;
}

interface PerformanceTrend {
  date: string;
  avgStartupMs: number;
  avgSearchMs: number;
  avgInstallMs: number;
  avgUpdateMs: number;
}

interface PerformanceData {
  summary: {
    avgStartupMs: number;
    avgSearchMs: number;
    avgInstallMs: number;
    avgUpdateMs: number;
    totalTimeSavedMs: number;
    commandSuccessRate: number;
  };
  metrics: PerformanceMetric[];
  trends: PerformanceTrend[];
  sparklineData: {
    startup: number[];
    search: number[];
    install: number[];
  };
}

async function fetchPerformanceData(): Promise<PerformanceData> {
  const response = await fetch('/api/dashboard/performance?days=30', {
    credentials: 'include',
  });
  if (!response.ok) {
    throw new Error('Failed to fetch performance data');
  }
  return response.json();
}

const formatDuration = (ms: number): string => {
  if (ms === 0) return '-';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
};

const formatTimeSaved = (ms: number): string => {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(0)}s`;
  if (ms < 3600000) return `${(ms / 60000).toFixed(0)}min`;
  if (ms < 86400000) return `${(ms / 3600000).toFixed(1)}hr`;
  return `${(ms / 86400000).toFixed(1)} days`;
};

const getMetricIcon = (metricType: string) => {
  switch (metricType) {
    case 'search':
      return Search;
    case 'install':
      return Package;
    case 'update':
      return Repeat;
    case 'remove':
      return Package;
    default:
      return Timer;
  }
};

const getMetricColor = (metricType: string) => {
  const colors: Record<string, { text: string; bg: string }> = {
    search: { text: 'text-cyan-400', bg: 'bg-cyan-500' },
    install: { text: 'text-indigo-400', bg: 'bg-indigo-500' },
    update: { text: 'text-emerald-400', bg: 'bg-emerald-500' },
    remove: { text: 'text-rose-400', bg: 'bg-rose-500' },
  };
  return colors[metricType] || { text: 'text-slate-400', bg: 'bg-slate-500' };
};

const getSparklineColor = (metricType: string) => {
  const colors: Record<string, string> = {
    startup: '#6366f1',
    search: '#22d3ee',
    install: '#a855f7',
  };
  return colors[metricType] || '#6366f1';
};

export const PerformanceMetrics: Component = () => {
  const [performance] = createResource(fetchPerformanceData);

  return (
    <div class="space-y-6">
      <Show when={performance.loading}>
        <div class="flex items-center justify-center py-20">
          <Loader2 size={32} class="animate-spin text-indigo-400" />
        </div>
      </Show>

      <Show when={performance.error}>
        <GlassCard class="p-6">
          <div class="flex items-center gap-3 text-rose-400">
            <AlertCircle size={24} />
            <span>Failed to load performance data. Please try again later.</span>
          </div>
        </GlassCard>
      </Show>

      <Show when={performance()}>
        {(data) => (
          <>
            {/* Summary Stats */}
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
              <StatCard
                title="Avg Search Time"
                value={formatDuration(data().summary.avgSearchMs)}
                icon={<Search size={24} />}
                description="Average search operation speed"
              />

              <StatCard
                title="Avg Install Time"
                value={formatDuration(data().summary.avgInstallMs)}
                icon={<Package size={24} />}
                description="Average package installation time"
              />

              <StatCard
                title="Avg Update Time"
                value={formatDuration(data().summary.avgUpdateMs)}
                icon={<Repeat size={24} />}
                description="Average update operation speed"
              />

              <StatCard
                title="Success Rate"
                value={`${data().summary.commandSuccessRate}%`}
                icon={<CheckCircle size={24} />}
                description="Command execution success rate"
              />
            </div>

            {/* Time Saved Highlight */}
            <GlassCard class="p-6">
              <div class="flex items-center justify-between">
                <div>
                  <p class="text-xs font-bold uppercase tracking-widest text-slate-500">
                    Total Time Saved
                  </p>
                  <p class="mt-2 text-4xl font-black text-transparent bg-clip-text bg-gradient-to-r from-indigo-400 to-purple-400">
                    {formatTimeSaved(data().summary.totalTimeSavedMs)}
                  </p>
                  <p class="mt-2 text-sm text-slate-400">
                    Compared to traditional package managers
                  </p>
                </div>
                <div class="rounded-2xl bg-gradient-to-br from-indigo-500/20 to-purple-500/20 p-4">
                  <Clock size={48} class="text-indigo-400" />
                </div>
              </div>
            </GlassCard>

            {/* Performance Trends */}
            <GlassCard class="p-6">
              <h3 class="text-lg font-bold text-white mb-6">Performance Trends</h3>
              <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                {/* Search Performance */}
                <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                  <div class="flex items-center justify-between mb-4">
                    <div class="flex items-center gap-2">
                      <Search size={18} class="text-cyan-400" />
                      <span class="text-sm font-medium text-white">Search</span>
                    </div>
                    <Show when={data().sparklineData.search.length >= 2}>
                      <TrendIndicator
                        value={data().sparklineData.search[data().sparklineData.search.length - 1] || 0}
                        previousValue={data().sparklineData.search[0] || 0}
                        size="sm"
                      />
                    </Show>
                  </div>
                  <Sparkline
                    data={data().sparklineData.search}
                    width={200}
                    height={48}
                    color="#22d3ee"
                    fillOpacity={0.15}
                    showDots
                  />
                  <p class="mt-2 text-xs text-slate-500">
                    Avg: {formatDuration(data().summary.avgSearchMs)}
                  </p>
                </div>

                {/* Install Performance */}
                <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                  <div class="flex items-center justify-between mb-4">
                    <div class="flex items-center gap-2">
                      <Package size={18} class="text-indigo-400" />
                      <span class="text-sm font-medium text-white">Install</span>
                    </div>
                    <Show when={data().sparklineData.install.length >= 2}>
                      <TrendIndicator
                        value={data().sparklineData.install[data().sparklineData.install.length - 1] || 0}
                        previousValue={data().sparklineData.install[0] || 0}
                        size="sm"
                      />
                    </Show>
                  </div>
                  <Sparkline
                    data={data().sparklineData.install}
                    width={200}
                    height={48}
                    color="#6366f1"
                    fillOpacity={0.15}
                    showDots
                  />
                  <p class="mt-2 text-xs text-slate-500">
                    Avg: {formatDuration(data().summary.avgInstallMs)}
                  </p>
                </div>

                {/* Startup Performance */}
                <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                  <div class="flex items-center justify-between mb-4">
                    <div class="flex items-center gap-2">
                      <Zap size={18} class="text-amber-400" />
                      <span class="text-sm font-medium text-white">Update</span>
                    </div>
                    <Show when={data().sparklineData.startup.length >= 2}>
                      <TrendIndicator
                        value={data().sparklineData.startup[data().sparklineData.startup.length - 1] || 0}
                        previousValue={data().sparklineData.startup[0] || 0}
                        size="sm"
                      />
                    </Show>
                  </div>
                  <Sparkline
                    data={data().sparklineData.startup}
                    width={200}
                    height={48}
                    color="#f59e0b"
                    fillOpacity={0.15}
                    showDots
                  />
                  <p class="mt-2 text-xs text-slate-500">
                    Avg: {formatDuration(data().summary.avgUpdateMs)}
                  </p>
                </div>
              </div>
            </GlassCard>

            {/* Detailed Metrics Table */}
            <GlassCard class="p-6">
              <h3 class="text-lg font-bold text-white mb-4">Detailed Performance Metrics</h3>
              <Show when={data().metrics.length === 0}>
                <p class="text-slate-400 text-sm py-8 text-center">
                  No performance data available yet. Start using OMG to see metrics here.
                </p>
              </Show>
              <Show when={data().metrics.length > 0}>
                <div class="overflow-x-auto">
                  <table class="w-full">
                    <thead>
                      <tr class="border-b border-white/10">
                        <th class="px-4 py-3 text-left text-xs font-bold uppercase tracking-wider text-slate-500">
                          Command
                        </th>
                        <th class="px-4 py-3 text-right text-xs font-bold uppercase tracking-wider text-slate-500">
                          Avg
                        </th>
                        <th class="px-4 py-3 text-right text-xs font-bold uppercase tracking-wider text-slate-500">
                          P50
                        </th>
                        <th class="px-4 py-3 text-right text-xs font-bold uppercase tracking-wider text-slate-500">
                          P95
                        </th>
                        <th class="px-4 py-3 text-right text-xs font-bold uppercase tracking-wider text-slate-500">
                          Min
                        </th>
                        <th class="px-4 py-3 text-right text-xs font-bold uppercase tracking-wider text-slate-500">
                          Max
                        </th>
                        <th class="px-4 py-3 text-right text-xs font-bold uppercase tracking-wider text-slate-500">
                          Samples
                        </th>
                      </tr>
                    </thead>
                    <tbody class="divide-y divide-white/5">
                      <For each={data().metrics}>
                        {(metric) => {
                          const Icon = getMetricIcon(metric.metricType);
                          const colors = getMetricColor(metric.metricType);
                          return (
                            <tr class="hover:bg-white/5 transition-colors">
                              <td class="px-4 py-3">
                                <div class="flex items-center gap-2">
                                  <Icon size={16} class={colors.text} />
                                  <span class={`font-medium capitalize ${colors.text}`}>
                                    {metric.metricType}
                                  </span>
                                </div>
                              </td>
                              <td class="px-4 py-3 text-right">
                                <span class="text-sm font-mono text-white">
                                  {formatDuration(metric.avgValue)}
                                </span>
                              </td>
                              <td class="px-4 py-3 text-right">
                                <span class="text-sm font-mono text-slate-300">
                                  {formatDuration(metric.p50Value)}
                                </span>
                              </td>
                              <td class="px-4 py-3 text-right">
                                <span class="text-sm font-mono text-slate-300">
                                  {formatDuration(metric.p95Value)}
                                </span>
                              </td>
                              <td class="px-4 py-3 text-right">
                                <span class="text-sm font-mono text-emerald-400">
                                  {formatDuration(metric.minValue)}
                                </span>
                              </td>
                              <td class="px-4 py-3 text-right">
                                <span class="text-sm font-mono text-amber-400">
                                  {formatDuration(metric.maxValue)}
                                </span>
                              </td>
                              <td class="px-4 py-3 text-right">
                                <span class="text-sm text-slate-500">
                                  {metric.sampleCount.toLocaleString()}
                                </span>
                              </td>
                            </tr>
                          );
                        }}
                      </For>
                    </tbody>
                  </table>
                </div>
              </Show>
            </GlassCard>

            {/* Performance Tips */}
            <GlassCard class="p-6">
              <div class="flex items-start gap-4">
                <div class="rounded-xl bg-indigo-500/20 p-3">
                  <Activity size={24} class="text-indigo-400" />
                </div>
                <div>
                  <h4 class="text-sm font-bold text-white mb-1">Performance Tips</h4>
                  <ul class="text-sm text-slate-400 space-y-1">
                    <li>
                      <span class="text-emerald-400">Enable daemon mode</span> for faster startup times
                    </li>
                    <li>
                      <span class="text-emerald-400">Use parallel sync</span> to speed up package updates
                    </li>
                    <li>
                      <span class="text-emerald-400">Enable caching</span> for frequently accessed packages
                    </li>
                  </ul>
                </div>
              </div>
            </GlassCard>
          </>
        )}
      </Show>
    </div>
  );
};

export default PerformanceMetrics;
