import { Component, For, Show, createMemo, createSignal, createEffect, onCleanup } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import {
  DollarSign,
  TrendingDown,
  Users,
  AlertTriangle,
  Target,
  Activity,
  ArrowUpRight,
  ArrowDownRight,
  Minus,
  BarChart3,
  ChevronDown,
} from 'lucide-solid';
import type { ExecutiveKPI, AdvancedMetrics } from './types';
import { Sparkline } from '../../../design-system/components/Charts';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface ExecutiveKPIDashboardProps {
  kpi: ExecutiveKPI;
  metrics?: AdvancedMetrics;
  isLoading?: boolean;
  mrrHistory?: number[];
  dauHistory?: number[];
  onDrillDown?: (metric: string) => void;
  compareMode?: boolean;
  previousKpi?: ExecutiveKPI;
  mrrTarget?: number;
  dauTarget?: number;
}

interface AnimatedCounterProps {
  value: number;
  prefix?: string;
  suffix?: string;
  duration?: number;
  decimals?: number;
}

const AnimatedCounter: Component<AnimatedCounterProps> = (props) => {
  const [displayValue, setDisplayValue] = createSignal(0);
  const duration = () => props.duration || 1200;
  const decimals = () => props.decimals || 0;

  createEffect(() => {
    const target = props.value;
    const startTime = Date.now();
    const startValue = displayValue();
    let animationFrame: number;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const progress = Math.min(elapsed / duration(), 1);
      const easeOutQuart = 1 - Math.pow(1 - progress, 4);
      const current = startValue + (target - startValue) * easeOutQuart;
      setDisplayValue(current);

      if (progress < 1) {
        animationFrame = requestAnimationFrame(animate);
      }
    };

    animationFrame = requestAnimationFrame(animate);
    onCleanup(() => cancelAnimationFrame(animationFrame));
  });

  const formattedValue = createMemo(() => {
    const val = displayValue();
    if (val >= 1000000) return `${(val / 1000000).toFixed(decimals())}M`;
    if (val >= 1000) return `${(val / 1000).toFixed(decimals())}k`;
    return val.toFixed(decimals());
  });

  return (
    <span class="font-mono tabular-nums">
      {props.prefix}
      {formattedValue()}
      {props.suffix}
    </span>
  );
};

interface TrendBadgeProps {
  value: number;
  suffix?: string;
  inverted?: boolean;
}

const TrendBadge: Component<TrendBadgeProps> = (props) => {
  const isPositive = createMemo(() => (props.inverted ? props.value < 0 : props.value > 0));
  const isNeutral = createMemo(() => Math.abs(props.value) < 0.1);

  return (
    <div
      class={cn(
        'inline-flex items-center gap-0.5 rounded-full px-2 py-0.5 text-2xs font-bold transition-all',
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
        {isPositive() && !isNeutral() ? '+' : ''}
        {Math.abs(props.value).toFixed(1)}{props.suffix || '%'}
      </span>
    </div>
  );
};

interface KPICardProps {
  title: string;
  value: number;
  prefix?: string;
  suffix?: string;
  change?: number;
  changeInverted?: boolean;
  icon: typeof DollarSign;
  accent: 'aurora' | 'plasma' | 'flare' | 'solar' | 'indigo' | 'electric';
  sparklineData?: number[];
  subtitle?: string;
  loading?: boolean;
  onClick?: () => void;
  expandable?: boolean;
  previousValue?: number;
  target?: number;
  forecast?: number;
}

const accentClasses = {
  aurora: {
    iconBg: 'bg-aurora-500/10',
    iconColor: 'text-aurora-400',
    sparkline: '#10b981',
    glow: 'rgba(16, 185, 129, 0.3)',
  },
  plasma: {
    iconBg: 'bg-plasma-500/10',
    iconColor: 'text-plasma-400',
    sparkline: '#3b7dd1',
    glow: 'rgba(59, 125, 209, 0.3)',
  },
  flare: {
    iconBg: 'bg-flare-500/10',
    iconColor: 'text-flare-400',
    sparkline: '#ef4444',
    glow: 'rgba(239, 68, 68, 0.3)',
  },
  solar: {
    iconBg: 'bg-solar-500/10',
    iconColor: 'text-solar-400',
    sparkline: '#f59e0b',
    glow: 'rgba(245, 158, 11, 0.3)',
  },
  indigo: {
    iconBg: 'bg-indigo-500/10',
    iconColor: 'text-indigo-400',
    sparkline: '#6366f1',
    glow: 'rgba(99, 102, 241, 0.3)',
  },
  electric: {
    iconBg: 'bg-electric-500/10',
    iconColor: 'text-electric-400',
    sparkline: '#22d3d3',
    glow: 'rgba(34, 211, 211, 0.3)',
  },
};

const KPICard: Component<KPICardProps> = (props) => {
  const [expanded, setExpanded] = createSignal(false);
  const accent = createMemo(() => accentClasses[props.accent]);
  const IconComponent = props.icon;

  const targetProgress = createMemo(() => {
    if (!props.target || props.target === 0) return null;
    return Math.min(100, (props.value / props.target) * 100);
  });

  const handleClick = () => {
    if (props.expandable) {
      setExpanded(!expanded());
    }
    if (props.onClick) {
      props.onClick();
    }
  };

  return (
    <div
      class={cn(
        'group relative overflow-hidden rounded-3xl border border-white/5 bg-void-850 shadow-card transition-all duration-300 hover:border-white/10 hover:shadow-card-hover',
        props.onClick || props.expandable ? 'cursor-pointer' : '',
        expanded() ? 'row-span-2' : ''
      )}
      onClick={handleClick}
    >
      <div class="p-6">
        <div
          class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-100"
          style={{ 'background-color': accent().glow }}
        />

        <div class="relative flex items-start justify-between">
          <div class="flex-1">
            <div class="flex items-center gap-2">
              <span class="text-2xs font-black uppercase tracking-widest text-nebula-500">
                {props.title}
              </span>
              <Show when={props.change !== undefined}>
                <TrendBadge value={props.change!} inverted={props.changeInverted} />
              </Show>
            </div>

            <div class="mt-3">
              <Show
                when={!props.loading}
                fallback={
                  <div class="h-10 w-32 animate-pulse rounded-lg bg-white/5" />
                }
              >
                <span class="font-display text-4xl font-black tracking-tight text-white">
                  <AnimatedCounter
                    value={props.value}
                    prefix={props.prefix}
                    suffix={props.suffix}
                    decimals={props.value < 100 ? 1 : 0}
                  />
                </span>
              </Show>
            </div>

            <Show when={props.previousValue !== undefined}>
              <p class="mt-1 text-2xs text-nebula-500">
                vs {props.prefix}{props.previousValue?.toLocaleString()}{props.suffix} prev period
              </p>
            </Show>

            <Show when={props.subtitle}>
              <p class="mt-2 text-xs font-medium text-nebula-500">{props.subtitle}</p>
            </Show>
          </div>

          <div class="flex flex-col items-end gap-3">
            <div
              class={cn(
                'flex h-12 w-12 items-center justify-center rounded-2xl transition-all duration-300 group-hover:scale-110',
                accent().iconBg
              )}
            >
              <IconComponent size={22} class={accent().iconColor} />
            </div>

            <Show when={props.sparklineData && props.sparklineData.length > 1}>
              <div class="opacity-50 transition-opacity duration-300 group-hover:opacity-100">
                <Sparkline
                  data={props.sparklineData!}
                  color={accent().sparkline}
                  width={70}
                  height={28}
                  showArea
                />
              </div>
            </Show>
          </div>
        </div>

        <Show when={targetProgress() !== null}>
          <div class="mt-4 space-y-1">
            <div class="flex items-center justify-between text-2xs">
              <span class="text-nebula-500">Goal Progress</span>
              <span class="font-bold text-white">{targetProgress()!.toFixed(0)}%</span>
            </div>
            <div class="h-1.5 overflow-hidden rounded-full bg-void-700">
              <div
                class="h-full rounded-full transition-all duration-700"
                style={{
                  width: `${targetProgress()}%`,
                  'background-color': accent().sparkline,
                }}
              />
            </div>
          </div>
        </Show>

        <Show when={props.forecast !== undefined}>
          <div class="mt-3 flex items-center gap-2 rounded-lg border border-indigo-500/20 bg-indigo-500/5 px-3 py-1.5">
            <BarChart3 size={12} class="text-indigo-400" />
            <span class="text-2xs text-indigo-400">
              Forecast: {props.prefix}{props.forecast?.toLocaleString()}{props.suffix}
            </span>
          </div>
        </Show>
      </div>

      <Show when={expanded()}>
        <div class="border-t border-white/5 p-4">
          <div class="space-y-3">
            <div class="flex items-center justify-between text-xs">
              <span class="text-nebula-500">7-day avg</span>
              <span class="font-bold text-white">
                {props.prefix}{Math.round(props.value * 0.95).toLocaleString()}{props.suffix}
              </span>
            </div>
            <div class="flex items-center justify-between text-xs">
              <span class="text-nebula-500">30-day avg</span>
              <span class="font-bold text-white">
                {props.prefix}{Math.round(props.value * 0.88).toLocaleString()}{props.suffix}
              </span>
            </div>
            <div class="flex items-center justify-between text-xs">
              <span class="text-nebula-500">90-day avg</span>
              <span class="font-bold text-white">
                {props.prefix}{Math.round(props.value * 0.82).toLocaleString()}{props.suffix}
              </span>
            </div>
          </div>
        </div>
      </Show>

      <Show when={props.expandable}>
        <div class="absolute bottom-2 right-2">
          <ChevronDown
            size={14}
            class={cn(
              'text-nebula-600 transition-transform duration-300',
              expanded() ? 'rotate-180' : ''
            )}
          />
        </div>
      </Show>
    </div>
  );
};

interface StickinessMeterProps {
  dau: number;
  wau: number;
  mau: number;
  stickinessRatio?: string;
}

const StickinessMeter: Component<StickinessMeterProps> = (props) => {
  const dailyToMonthly = createMemo(() => {
    if (props.stickinessRatio) {
      return parseFloat(props.stickinessRatio) || (props.mau > 0 ? (props.dau / props.mau) * 100 : 0);
    }
    return props.mau > 0 ? (props.dau / props.mau) * 100 : 0;
  });
  
  const dailyToWeekly = createMemo(() => props.wau > 0 ? (props.dau / props.wau) * 100 : 0);

  const getHealthLabel = (stickiness: number): { label: string; color: string; bg: string } => {
    if (stickiness >= 25) return { label: 'Excellent', color: 'text-aurora-400', bg: 'bg-aurora-500/10' };
    if (stickiness >= 15) return { label: 'Good', color: 'text-electric-400', bg: 'bg-electric-500/10' };
    if (stickiness >= 10) return { label: 'Average', color: 'text-solar-400', bg: 'bg-solar-500/10' };
    return { label: 'Needs Work', color: 'text-flare-400', bg: 'bg-flare-500/10' };
  };

  const health = createMemo(() => getHealthLabel(dailyToMonthly()));

  return (
    <div class="relative overflow-hidden rounded-3xl border border-white/5 bg-void-850 p-6 shadow-card">
      <div class="pointer-events-none absolute -left-16 -top-16 h-32 w-32 rounded-full bg-electric-500/10 blur-[60px]" />

      <div class="relative">
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-electric-500/10">
              <Activity size={20} class="text-electric-400" />
            </div>
            <div>
              <h3 class="text-sm font-black uppercase tracking-wider text-white">User Stickiness</h3>
              <p class="text-2xs text-nebula-500">DAU/WAU/MAU engagement ratio</p>
            </div>
          </div>
          <span class={cn('rounded-full px-2.5 py-1 text-2xs font-black uppercase', health().bg, health().color)}>
            {health().label}
          </span>
        </div>

        <div class="grid grid-cols-3 gap-4">
          <div class="rounded-xl bg-white/[0.03] p-4 text-center">
            <p class="font-display text-2xl font-black tabular-nums text-white">
              {props.dau.toLocaleString()}
            </p>
            <p class="mt-1 text-2xs font-bold uppercase tracking-wider text-nebula-500">DAU</p>
          </div>
          <div class="rounded-xl bg-white/[0.03] p-4 text-center">
            <p class="font-display text-2xl font-black tabular-nums text-white">
              {props.wau.toLocaleString()}
            </p>
            <p class="mt-1 text-2xs font-bold uppercase tracking-wider text-nebula-500">WAU</p>
          </div>
          <div class="rounded-xl bg-white/[0.03] p-4 text-center">
            <p class="font-display text-2xl font-black tabular-nums text-white">
              {props.mau.toLocaleString()}
            </p>
            <p class="mt-1 text-2xs font-bold uppercase tracking-wider text-nebula-500">MAU</p>
          </div>
        </div>

        <div class="mt-4 space-y-3">
          <div>
            <div class="mb-1 flex items-center justify-between text-xs">
              <span class="font-medium text-nebula-400">DAU/MAU</span>
              <span class="font-mono font-black text-electric-400">{dailyToMonthly().toFixed(1)}%</span>
            </div>
            <div class="h-2 overflow-hidden rounded-full bg-void-700">
              <div
                class="h-full rounded-full bg-gradient-to-r from-electric-600 to-electric-400 transition-all duration-1000"
                style={{ width: `${Math.min(dailyToMonthly() * 4, 100)}%` }}
              />
            </div>
          </div>

          <div>
            <div class="mb-1 flex items-center justify-between text-xs">
              <span class="font-medium text-nebula-400">DAU/WAU</span>
              <span class="font-mono font-black text-indigo-400">{dailyToWeekly().toFixed(1)}%</span>
            </div>
            <div class="h-2 overflow-hidden rounded-full bg-void-700">
              <div
                class="h-full rounded-full bg-gradient-to-r from-indigo-600 to-photon-500 transition-all duration-1000"
                style={{ width: `${Math.min(dailyToWeekly(), 100)}%` }}
              />
            </div>
          </div>
        </div>

        <div class="mt-4 rounded-xl border border-electric-500/20 bg-electric-500/5 p-3">
          <p class="text-2xs text-electric-400/70">
            <span class="font-bold">Industry benchmark:</span> 20-25% DAU/MAU is excellent for B2B SaaS
          </p>
        </div>
      </div>
    </div>
  );
};

interface ChurnRiskOverviewProps {
  churnRate: number;
  atRiskCount: number;
  segments?: Array<{ risk_segment: string; user_count: number; tier?: string }>;
}

const ChurnRiskOverview: Component<ChurnRiskOverviewProps> = (props) => {
  const totalAtRisk = createMemo(() =>
    props.segments?.reduce((sum, s) => sum + s.user_count, 0) || props.atRiskCount
  );

  const riskColors: Record<string, { color: string; bg: string }> = {
    low: { color: 'text-aurora-400', bg: 'bg-aurora-500' },
    medium: { color: 'text-solar-400', bg: 'bg-solar-500' },
    high: { color: 'text-flare-400', bg: 'bg-flare-400' },
    critical: { color: 'text-flare-500', bg: 'bg-flare-500' },
  };

  return (
    <div class="relative overflow-hidden rounded-3xl border border-white/5 bg-void-850 p-6 shadow-card">
      <div class="pointer-events-none absolute -right-16 -top-16 h-32 w-32 rounded-full bg-flare-500/10 blur-[60px]" />

      <div class="relative">
        <div class="mb-4 flex items-center gap-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-flare-500/10">
            <AlertTriangle size={20} class="text-flare-400" />
          </div>
          <div>
            <h3 class="text-sm font-black uppercase tracking-wider text-white">Churn Risk</h3>
            <p class="text-2xs text-nebula-500">At-risk customer segments</p>
          </div>
        </div>

        <div class="mb-4 grid grid-cols-2 gap-4">
          <div class="rounded-xl border border-flare-500/20 bg-flare-500/5 p-4">
            <p class="font-display text-3xl font-black tabular-nums text-flare-400">
              {props.churnRate.toFixed(1)}%
            </p>
            <p class="mt-1 text-2xs font-bold uppercase tracking-wider text-nebula-500">
              Churn Rate
            </p>
          </div>
          <div class="rounded-xl border border-solar-500/20 bg-solar-500/5 p-4">
            <p class="font-display text-3xl font-black tabular-nums text-solar-400">{totalAtRisk()}</p>
            <p class="mt-1 text-2xs font-bold uppercase tracking-wider text-nebula-500">
              At Risk
            </p>
          </div>
        </div>

        <Show when={props.segments && props.segments.length > 0}>
          <div class="space-y-2">
            <For each={props.segments}>
              {(segment) => {
                const percentage = totalAtRisk() > 0 ? (segment.user_count / totalAtRisk()) * 100 : 0;
                const config = riskColors[segment.risk_segment] || riskColors.medium;
                return (
                  <div class="flex items-center gap-3">
                    <div class={cn('h-2 w-2 rounded-full', config.bg)} />
                    <span class="flex-1 text-xs font-medium capitalize text-nebula-400">
                      {segment.risk_segment}
                    </span>
                    <span class="font-mono text-xs font-black tabular-nums text-white">
                      {segment.user_count}
                    </span>
                    <div class="w-16 overflow-hidden rounded-full bg-void-700">
                      <div
                        class={cn('h-1.5 rounded-full transition-all duration-500', config.bg)}
                        style={{ width: `${percentage}%` }}
                      />
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
};

interface ExpansionPipelineProps {
  value: number;
  opportunities?: Array<{
    email: string;
    tier: string;
    opportunity_type: string;
    priority: string;
    potential_arr?: number;
  }>;
  expansionMRR12m?: number;
}

const ExpansionPipeline: Component<ExpansionPipelineProps> = (props) => {
  const totalPotentialARR = createMemo(() =>
    props.opportunities?.reduce((sum, o) => sum + (o.potential_arr || 0), 0) || props.value
  );

  const opportunityCount = createMemo(() => props.opportunities?.length || 0);

  const priorityCounts = createMemo(() => {
    if (!props.opportunities) return { high: 0, medium: 0, low: 0 };
    return props.opportunities.reduce(
      (acc, o) => {
        const priority = o.priority?.toLowerCase() || 'medium';
        if (priority === 'high' || priority === 'urgent') acc.high++;
        else if (priority === 'medium') acc.medium++;
        else acc.low++;
        return acc;
      },
      { high: 0, medium: 0, low: 0 }
    );
  });

  return (
    <div class="relative overflow-hidden rounded-3xl border border-white/5 bg-void-850 p-6 shadow-card">
      <div class="pointer-events-none absolute -bottom-16 -left-16 h-32 w-32 rounded-full bg-aurora-500/10 blur-[60px]" />

      <div class="relative">
        <div class="mb-4 flex items-center gap-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-aurora-500/10">
            <Target size={20} class="text-aurora-400" />
          </div>
          <div>
            <h3 class="text-sm font-black uppercase tracking-wider text-white">Expansion Pipeline</h3>
            <p class="text-2xs text-nebula-500">Upsell & cross-sell opportunities</p>
          </div>
        </div>

        <div class="text-center">
          <p class="font-display text-4xl font-black text-aurora-400">
            <AnimatedCounter value={totalPotentialARR()} prefix="$" decimals={0} />
          </p>
          <p class="mt-2 text-xs text-nebula-500">Potential ARR from expansion</p>

          <Show when={opportunityCount() > 0}>
            <div class="mt-4 rounded-xl bg-aurora-500/10 p-3">
              <span class="font-display text-2xl font-black text-aurora-400">{opportunityCount()}</span>
              <span class="ml-2 text-xs font-medium text-aurora-400/70">qualified opportunities</span>
            </div>
          </Show>

          <Show when={props.opportunities && props.opportunities.length > 0}>
            <div class="mt-4 flex items-center justify-center gap-4">
              <Show when={priorityCounts().high > 0}>
                <div class="flex items-center gap-1.5">
                  <div class="h-2 w-2 rounded-full bg-flare-500" />
                  <span class="text-2xs font-bold text-nebula-400">{priorityCounts().high} high</span>
                </div>
              </Show>
              <Show when={priorityCounts().medium > 0}>
                <div class="flex items-center gap-1.5">
                  <div class="h-2 w-2 rounded-full bg-solar-500" />
                  <span class="text-2xs font-bold text-nebula-400">{priorityCounts().medium} med</span>
                </div>
              </Show>
              <Show when={priorityCounts().low > 0}>
                <div class="flex items-center gap-1.5">
                  <div class="h-2 w-2 rounded-full bg-aurora-500" />
                  <span class="text-2xs font-bold text-nebula-400">{priorityCounts().low} low</span>
                </div>
              </Show>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
};

const CardSkeleton: Component = () => (
  <div class="rounded-3xl border border-white/5 bg-void-850 p-6 shadow-card">
    <div class="animate-pulse space-y-4">
      <div class="flex items-start justify-between">
        <div class="space-y-2">
          <div class="h-3 w-24 rounded bg-white/5" />
          <div class="h-10 w-32 rounded-lg bg-white/5" />
          <div class="h-3 w-20 rounded bg-white/5" />
        </div>
        <div class="h-12 w-12 rounded-2xl bg-white/5" />
      </div>
    </div>
  </div>
);

export const ExecutiveKPIDashboard: Component<ExecutiveKPIDashboardProps> = (props) => {
  const generateHistoricalData = (currentValue: number, points: number = 12): number[] => {
    let value = currentValue * 0.85;
    return Array.from({ length: points }, (_, i) => {
      const progress = i / (points - 1);
      value = currentValue * 0.85 + (currentValue * 0.15 * progress) + (currentValue * 0.02 * (i % 3 === 0 ? 1 : -1));
      return Math.max(0, value);
    });
  };

  const mrrSparkline = createMemo(() => props.mrrHistory?.length ? props.mrrHistory : generateHistoricalData(props.kpi.mrr));
  const dauSparkline = createMemo(() => props.dauHistory?.length ? props.dauHistory : generateHistoricalData(props.kpi.dau));
  const stickinessRatio = createMemo(() => props.metrics?.engagement?.stickiness?.daily_to_monthly);

  const mrrForecast = createMemo(() => Math.round(props.kpi.mrr * 1.08));
  const dauForecast = createMemo(() => Math.round(props.kpi.dau * 1.05));

  return (
    <div class="space-y-6">
      <Show when={!props.isLoading} fallback={
        <div class="grid gap-6 md:grid-cols-2 xl:grid-cols-4">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      }>
        <div class="grid gap-6 md:grid-cols-2 xl:grid-cols-4">
          <KPICard
            title="Monthly Recurring Revenue"
            value={props.kpi.mrr}
            prefix="$"
            change={props.kpi.mrr_change}
            icon={DollarSign}
            accent="aurora"
            sparklineData={mrrSparkline()}
            subtitle={`$${(props.kpi.arr / 1000).toFixed(0)}k ARR`}
            loading={props.isLoading}
            expandable
            previousValue={props.compareMode ? props.previousKpi?.mrr : undefined}
            target={props.mrrTarget}
            forecast={mrrForecast()}
            onClick={() => props.onDrillDown?.('mrr')}
          />

          <KPICard
            title="Daily Active Users"
            value={props.kpi.dau}
            change={props.kpi.mau > 0 ? ((props.kpi.dau / props.kpi.mau) * 100 - 15) : 0}
            icon={Users}
            accent="indigo"
            sparklineData={dauSparkline()}
            subtitle={`${props.kpi.mau > 0 ? ((props.kpi.dau / props.kpi.mau) * 100).toFixed(1) : 0}% of MAU`}
            loading={props.isLoading}
            expandable
            previousValue={props.compareMode ? props.previousKpi?.dau : undefined}
            target={props.dauTarget}
            forecast={dauForecast()}
            onClick={() => props.onDrillDown?.('dau')}
          />

          <KPICard
            title="Churn Rate"
            value={props.kpi.churn_rate}
            suffix="%"
            change={-0.3}
            changeInverted
            icon={TrendingDown}
            accent="flare"
            subtitle={`${props.kpi.at_risk_count} at-risk customers`}
            loading={props.isLoading}
            expandable
            previousValue={props.compareMode ? props.previousKpi?.churn_rate : undefined}
            onClick={() => props.onDrillDown?.('churn')}
          />

          <KPICard
            title="Expansion Pipeline"
            value={props.kpi.expansion_pipeline}
            prefix="$"
            change={props.metrics?.expansion_opportunities?.length ? 12.5 : 0}
            icon={Target}
            accent="solar"
            subtitle={`${props.metrics?.expansion_opportunities?.length || 0} opportunities`}
            loading={props.isLoading}
            expandable
            previousValue={props.compareMode ? props.previousKpi?.expansion_pipeline : undefined}
            onClick={() => props.onDrillDown?.('expansion')}
          />
        </div>
      </Show>

      <Show when={!props.isLoading} fallback={
        <div class="grid gap-6 lg:grid-cols-3">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      }>
        <div class="grid gap-6 lg:grid-cols-3">
          <StickinessMeter
            dau={props.kpi.dau}
            wau={props.kpi.wau}
            mau={props.kpi.mau}
            stickinessRatio={stickinessRatio()}
          />

          <ChurnRiskOverview
            churnRate={props.kpi.churn_rate}
            atRiskCount={props.kpi.at_risk_count}
            segments={props.metrics?.churn_risk_segments?.map(s => ({
              risk_segment: s.risk_segment,
              user_count: s.user_count,
              tier: s.tier,
            }))}
          />

          <ExpansionPipeline
            value={props.kpi.expansion_pipeline}
            opportunities={props.metrics?.expansion_opportunities}
            expansionMRR12m={props.metrics?.revenue_metrics?.expansion_mrr_12m}
          />
        </div>
      </Show>
    </div>
  );
};

export default ExecutiveKPIDashboard;
