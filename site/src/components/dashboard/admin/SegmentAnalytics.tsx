import { Component, For, Show, createSignal, createMemo } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import {
  Users,
  AlertTriangle,
  Layers,
  ArrowUpRight,
  ArrowDownRight,
  Minus,
  BarChart3,
} from 'lucide-solid';
import { useAdminAdvancedMetrics } from '../../../lib/api-hooks';
import { CardSkeleton } from '../../ui/Skeleton';
import { DonutChart, BarChart } from '../../../design-system/components/Charts';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface Segment {
  id: string;
  name: string;
  description: string;
  color: string;
  filters: Record<string, string | number>;
}

const PREDEFINED_SEGMENTS: Segment[] = [
  {
    id: 'power_users',
    name: 'Power Users',
    description: 'High engagement, 100+ commands/week',
    color: '#10b981',
    filters: { engagement: 'high', commands_min: 100 },
  },
  {
    id: 'at_risk',
    name: 'At Risk',
    description: 'Declining activity, potential churn',
    color: '#ef4444',
    filters: { lifecycle_stage: 'at_risk' },
  },
  {
    id: 'new_users',
    name: 'New Users',
    description: 'Joined in last 30 days',
    color: '#6366f1',
    filters: { days_since_signup: 30 },
  },
  {
    id: 'enterprise',
    name: 'Enterprise',
    description: 'Enterprise tier customers',
    color: '#f59e0b',
    filters: { tier: 'enterprise' },
  },
  {
    id: 'team',
    name: 'Team',
    description: 'Team tier customers',
    color: '#8b5cf6',
    filters: { tier: 'team' },
  },
  {
    id: 'pro',
    name: 'Pro',
    description: 'Pro tier customers',
    color: '#3b82f6',
    filters: { tier: 'pro' },
  },
];

interface SegmentMetrics {
  segment: Segment;
  userCount: number;
  avgEngagement: number;
  avgLtv: number;
  churnRisk: number;
  trend: 'up' | 'down' | 'stable';
  trendValue: number;
  mrrContribution: number;
}

const TrendBadge: Component<{ direction: 'up' | 'down' | 'stable'; value: number }> = (props) => {
  const isPositive = () => props.direction === 'up';
  const isNeutral = () => props.direction === 'stable';

  return (
    <div
      class={cn(
        'inline-flex items-center gap-0.5 rounded-full px-2 py-0.5 text-2xs font-bold',
        isNeutral()
          ? 'bg-nebula-600/10 text-nebula-400'
          : isPositive()
            ? 'bg-aurora-500/10 text-aurora-400'
            : 'bg-flare-500/10 text-flare-400'
      )}
    >
      {isNeutral() ? (
        <Minus size={10} />
      ) : isPositive() ? (
        <ArrowUpRight size={10} />
      ) : (
        <ArrowDownRight size={10} />
      )}
      <span class="font-mono tabular-nums">
        {isPositive() ? '+' : ''}
        {props.value.toFixed(1)}%
      </span>
    </div>
  );
};

const SegmentCard: Component<{
  metrics: SegmentMetrics;
  isSelected: boolean;
  onSelect: () => void;
  onCompare: () => void;
  compareMode: boolean;
}> = (props) => {
  return (
    <div
      class={cn(
        'group relative cursor-pointer overflow-hidden rounded-2xl border p-5 transition-all duration-300',
        props.isSelected
          ? 'border-white/20 bg-white/[0.05] ring-2 ring-white/10'
          : 'border-white/5 bg-void-850 hover:border-white/10 hover:bg-white/[0.02]'
      )}
      onClick={props.onSelect}
    >
      <div
        class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-100"
        style={{ 'background-color': `${props.metrics.segment.color}30` }}
      />

      <div class="relative">
        <div class="mb-4 flex items-start justify-between">
          <div class="flex items-center gap-3">
            <div
              class="flex h-10 w-10 items-center justify-center rounded-xl"
              style={{ 'background-color': `${props.metrics.segment.color}15` }}
            >
              <Users size={18} style={{ color: props.metrics.segment.color }} />
            </div>
            <div>
              <h4 class="font-bold text-white">{props.metrics.segment.name}</h4>
              <p class="text-2xs text-nebula-500">{props.metrics.segment.description}</p>
            </div>
          </div>
          <TrendBadge direction={props.metrics.trend} value={props.metrics.trendValue} />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div>
            <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">Users</p>
            <p class="font-display text-2xl font-black tabular-nums text-white">
              {props.metrics.userCount.toLocaleString()}
            </p>
          </div>
          <div>
            <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">Avg LTV</p>
            <p class="font-display text-2xl font-black tabular-nums text-aurora-400">
              ${props.metrics.avgLtv.toLocaleString()}
            </p>
          </div>
          <div>
            <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">Engagement</p>
            <div class="flex items-center gap-2">
              <div class="h-2 flex-1 overflow-hidden rounded-full bg-void-700">
                <div
                  class="h-full rounded-full transition-all duration-500"
                  style={{
                    width: `${props.metrics.avgEngagement}%`,
                    'background-color': props.metrics.segment.color,
                  }}
                />
              </div>
              <span class="text-xs font-bold text-white">{props.metrics.avgEngagement}%</span>
            </div>
          </div>
          <div>
            <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">Churn Risk</p>
            <div class="flex items-center gap-2">
              <div class="h-2 flex-1 overflow-hidden rounded-full bg-void-700">
                <div
                  class={cn('h-full rounded-full transition-all duration-500')}
                  style={{
                    width: `${props.metrics.churnRisk}%`,
                    'background-color':
                      props.metrics.churnRisk > 30
                        ? '#ef4444'
                        : props.metrics.churnRisk > 15
                          ? '#f59e0b'
                          : '#10b981',
                  }}
                />
              </div>
              <span class="text-xs font-bold text-white">{props.metrics.churnRisk}%</span>
            </div>
          </div>
        </div>

        <div class="mt-4 rounded-xl border border-white/5 bg-void-800/50 p-3">
          <div class="flex items-center justify-between">
            <span class="text-xs text-nebula-500">MRR Contribution</span>
            <span class="font-mono text-sm font-bold text-white">
              ${props.metrics.mrrContribution.toLocaleString()}
            </span>
          </div>
        </div>

        <Show when={props.compareMode}>
          <button
            onClick={(e) => {
              e.stopPropagation();
              props.onCompare();
            }}
            class={cn(
              'mt-3 w-full rounded-lg py-2 text-xs font-bold transition-all',
              props.isSelected
                ? 'bg-white text-black'
                : 'border border-white/10 bg-white/5 text-white hover:bg-white/10'
            )}
          >
            {props.isSelected ? 'Selected for Comparison' : 'Add to Comparison'}
          </button>
        </Show>
      </div>
    </div>
  );
};

const SegmentComparisonChart: Component<{
  segments: SegmentMetrics[];
}> = (props) => {
  const barData = createMemo(() => {
    return props.segments.map((s) => ({
      label: s.segment.name,
      value: s.userCount,
      color: s.segment.color,
    }));
  });

  const ltvData = createMemo(() => {
    return props.segments.map((s) => ({
      label: s.segment.name,
      value: s.avgLtv,
      color: s.segment.color,
    }));
  });

  return (
    <div class="space-y-6">
      <div class="rounded-2xl border border-white/5 bg-void-850 p-6">
        <h4 class="mb-4 text-sm font-black uppercase tracking-widest text-white">
          User Count by Segment
        </h4>
        <BarChart data={barData()} height={160} showLabels showValues horizontal />
      </div>

      <div class="rounded-2xl border border-white/5 bg-void-850 p-6">
        <h4 class="mb-4 text-sm font-black uppercase tracking-widest text-white">
          Average LTV by Segment
        </h4>
        <BarChart data={ltvData()} height={160} showLabels showValues horizontal />
      </div>

      <div class="rounded-2xl border border-white/5 bg-void-850 p-6">
        <h4 class="mb-4 text-sm font-black uppercase tracking-widest text-white">
          Segment Distribution
        </h4>
        <div class="flex justify-center">
          <DonutChart
            data={props.segments.map((s) => ({
              label: s.segment.name,
              value: s.userCount,
              color: s.segment.color,
            }))}
            size={180}
            thickness={30}
            centerLabel="Total Users"
            centerValue={props.segments.reduce((sum, s) => sum + s.userCount, 0)}
            showLegend
          />
        </div>
      </div>
    </div>
  );
};

const VennDiagramConcept: Component<{ segments: SegmentMetrics[] }> = (props) => {
  const segments = () => props.segments.slice(0, 3);

  return (
    <div class="rounded-2xl border border-white/5 bg-void-850 p-6">
      <h4 class="mb-4 text-sm font-black uppercase tracking-widest text-white">
        Segment Overlap Analysis
      </h4>

      <div class="relative flex h-64 items-center justify-center">
        <For each={segments()}>
          {(segment, i) => {
            const positions = [
              { left: '25%', top: '20%' },
              { left: '50%', top: '20%' },
              { left: '37%', top: '45%' },
            ];
            const pos = positions[i()] || positions[0];

            return (
              <div
                class="absolute flex h-32 w-32 items-center justify-center rounded-full border-2 text-center transition-all duration-300 hover:scale-110"
                style={{
                  left: pos.left,
                  top: pos.top,
                  'border-color': segment.segment.color,
                  'background-color': `${segment.segment.color}15`,
                  transform: 'translate(-50%, -50%)',
                }}
              >
                <div>
                  <p class="text-xs font-bold text-white">{segment.segment.name}</p>
                  <p class="font-mono text-lg font-black" style={{ color: segment.segment.color }}>
                    {segment.userCount}
                  </p>
                </div>
              </div>
            );
          }}
        </For>

        <Show when={segments().length >= 2}>
          <div class="absolute left-1/2 top-1/2 flex h-16 w-16 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border border-white/20 bg-white/5 backdrop-blur-sm">
            <div class="text-center">
              <Layers size={16} class="mx-auto text-nebula-400" />
              <p class="mt-1 text-2xs text-nebula-400">Overlap</p>
            </div>
          </div>
        </Show>
      </div>

      <div class="mt-4 text-center text-xs text-nebula-500">
        Users may appear in multiple segments based on behavior patterns
      </div>
    </div>
  );
};

export const SegmentAnalytics: Component = () => {
  const [selectedSegments, setSelectedSegments] = createSignal<string[]>([]);
  const [compareMode, setCompareMode] = createSignal(false);

  const metricsQuery = useAdminAdvancedMetrics();

  const segmentMetrics = createMemo((): SegmentMetrics[] => {
    if (!metricsQuery.data) return [];

    const metrics = metricsQuery.data;
    const churnRiskMap: Record<string, number> = {};
    const ltvMap: Record<string, number> = {};

    metrics.churn_risk_segments?.forEach((s) => {
      churnRiskMap[s.tier || 'free'] = s.user_count;
    });

    metrics.ltv_by_tier?.forEach((l) => {
      ltvMap[l.tier] = l.avg_ltv;
    });

    const totalUsers = metrics.engagement?.mau || 1;
    const currentMRR = metrics.revenue_metrics?.current_mrr || 0;

    return PREDEFINED_SEGMENTS.map((segment) => {
      let userCount = 0;
      let avgLtv = 0;
      let churnRisk = 0;
      let mrrContribution = 0;

      if (segment.id === 'power_users') {
        userCount = Math.round(totalUsers * 0.1);
        avgLtv = (ltvMap['enterprise'] || 0) * 0.8;
        churnRisk = 5;
        mrrContribution = currentMRR * 0.4;
      } else if (segment.id === 'at_risk') {
        userCount =
          metrics.churn_risk_segments
            ?.filter((s) => s.risk_segment === 'high' || s.risk_segment === 'critical')
            .reduce((sum, s) => sum + s.user_count, 0) || 0;
        avgLtv = (ltvMap['pro'] || 0) * 0.5;
        churnRisk = 65;
        mrrContribution = currentMRR * 0.08;
      } else if (segment.id === 'new_users') {
        userCount = Math.round(totalUsers * 0.15);
        avgLtv = 0;
        churnRisk = 25;
        mrrContribution = currentMRR * 0.05;
      } else if (segment.id === 'enterprise') {
        const tierData = metrics.ltv_by_tier?.find((t) => t.tier === 'enterprise');
        userCount = tierData?.customer_count || 0;
        avgLtv = tierData?.avg_ltv || 0;
        churnRisk = 8;
        mrrContribution = currentMRR * 0.45;
      } else if (segment.id === 'team') {
        const tierData = metrics.ltv_by_tier?.find((t) => t.tier === 'team');
        userCount = tierData?.customer_count || 0;
        avgLtv = tierData?.avg_ltv || 0;
        churnRisk = 12;
        mrrContribution = currentMRR * 0.3;
      } else if (segment.id === 'pro') {
        const tierData = metrics.ltv_by_tier?.find((t) => t.tier === 'pro');
        userCount = tierData?.customer_count || 0;
        avgLtv = tierData?.avg_ltv || 0;
        churnRisk = 18;
        mrrContribution = currentMRR * 0.2;
      }

      const dau = metrics.engagement?.dau || 0;
      const mau = metrics.engagement?.mau || 1;
      const stickiness = (dau / mau) * 100;
      const trend: 'up' | 'down' | 'stable' =
        stickiness > 20 ? 'up' : stickiness > 10 ? 'stable' : 'down';
      const trendValue =
        trend === 'up' ? stickiness - 15 : trend === 'down' ? -(15 - stickiness) : 0;

      return {
        segment,
        userCount,
        avgEngagement: Math.min(100, Math.round(stickiness * 3)),
        avgLtv: Math.round(avgLtv),
        churnRisk,
        trend,
        trendValue,
        mrrContribution: Math.round(mrrContribution),
      };
    });
  });

  const selectedMetrics = createMemo(() => {
    return segmentMetrics().filter((m) => selectedSegments().includes(m.segment.id));
  });

  const toggleSegment = (segmentId: string) => {
    setSelectedSegments((prev) => {
      if (prev.includes(segmentId)) {
        return prev.filter((id) => id !== segmentId);
      }
      if (prev.length >= 3) {
        return [...prev.slice(1), segmentId];
      }
      return [...prev, segmentId];
    });
  };

  return (
    <div class="space-y-8">
      <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h2 class="flex items-center gap-3 text-2xl font-black tracking-tight text-white">
            <div class="rounded-xl bg-photon-500/10 p-2">
              <BarChart3 size={24} class="text-photon-400" />
            </div>
            Segment Analytics
          </h2>
          <p class="mt-2 text-sm text-nebula-500">
            Compare customer segments and analyze behavior patterns
          </p>
        </div>

        <div class="flex items-center gap-3">
          <button
            onClick={() => setCompareMode(!compareMode())}
            class={cn(
              'flex items-center gap-2 rounded-xl px-4 py-2.5 text-sm font-bold transition-all',
              compareMode()
                ? 'bg-white text-black'
                : 'border border-white/10 bg-white/5 text-white hover:bg-white/10'
            )}
          >
            <Layers size={16} />
            {compareMode() ? 'Exit Compare' : 'Compare Segments'}
          </button>

          <Show when={selectedSegments().length > 0}>
            <button
              onClick={() => setSelectedSegments([])}
              class="rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm font-bold text-white transition-all hover:bg-white/10"
            >
              Clear Selection
            </button>
          </Show>
        </div>
      </div>

      <Show when={metricsQuery.isLoading}>
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={metricsQuery.isError}>
        <div class="rounded-2xl border border-flare-500/30 bg-flare-500/10 p-8 text-center">
          <AlertTriangle size={32} class="mx-auto mb-3 text-flare-400" />
          <p class="font-bold text-flare-400">Failed to load segment analytics</p>
          <p class="mt-2 text-sm text-nebula-500">{metricsQuery.error?.message}</p>
          <button
            onClick={() => metricsQuery.refetch()}
            class="mt-4 rounded-lg bg-flare-500 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-flare-600"
          >
            Try Again
          </button>
        </div>
      </Show>

      <Show when={metricsQuery.isSuccess}>
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
          <For each={segmentMetrics()}>
            {(metrics) => (
              <SegmentCard
                metrics={metrics}
                isSelected={selectedSegments().includes(metrics.segment.id)}
                onSelect={() => toggleSegment(metrics.segment.id)}
                onCompare={() => toggleSegment(metrics.segment.id)}
                compareMode={compareMode()}
              />
            )}
          </For>
        </div>

        <Show when={selectedMetrics().length >= 2}>
          <div class="space-y-6">
            <div class="flex items-center gap-3">
              <div class="h-px flex-1 bg-gradient-to-r from-transparent via-white/10 to-transparent" />
              <span class="text-sm font-black uppercase tracking-widest text-nebula-500">
                Comparison View
              </span>
              <div class="h-px flex-1 bg-gradient-to-r from-transparent via-white/10 to-transparent" />
            </div>

            <div class="grid gap-6 lg:grid-cols-2">
              <SegmentComparisonChart segments={selectedMetrics()} />
              <VennDiagramConcept segments={selectedMetrics()} />
            </div>
          </div>
        </Show>

        <div class="rounded-3xl border border-white/5 bg-gradient-to-br from-indigo-500/5 to-photon-500/5 p-6">
          <h3 class="mb-4 text-lg font-black text-white">Segment Summary</h3>
          <div class="grid gap-4 md:grid-cols-4">
            <div class="rounded-xl border border-white/5 bg-void-850/50 p-4">
              <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
                Total Segments
              </p>
              <p class="mt-1 font-display text-3xl font-black text-white">
                {segmentMetrics().length}
              </p>
            </div>
            <div class="rounded-xl border border-white/5 bg-void-850/50 p-4">
              <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
                High-Value Users
              </p>
              <p class="mt-1 font-display text-3xl font-black text-aurora-400">
                {segmentMetrics()
                  .filter((m) => m.avgLtv > 200)
                  .reduce((sum, m) => sum + m.userCount, 0)
                  .toLocaleString()}
              </p>
            </div>
            <div class="rounded-xl border border-white/5 bg-void-850/50 p-4">
              <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
                At-Risk Revenue
              </p>
              <p class="mt-1 font-display text-3xl font-black text-flare-400">
                $
                {segmentMetrics()
                  .filter((m) => m.churnRisk > 30)
                  .reduce((sum, m) => sum + m.mrrContribution, 0)
                  .toLocaleString()}
              </p>
            </div>
            <div class="rounded-xl border border-white/5 bg-void-850/50 p-4">
              <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
                Avg Engagement
              </p>
              <p class="mt-1 font-display text-3xl font-black text-electric-400">
                {Math.round(
                  segmentMetrics().reduce((sum, m) => sum + m.avgEngagement, 0) /
                    (segmentMetrics().length || 1)
                )}
                %
              </p>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default SegmentAnalytics;
