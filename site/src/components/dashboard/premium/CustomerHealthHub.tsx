import { Component, Show, For, createMemo, createSignal } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import type { CustomerHealth, LifecycleStage } from './types';
import {
  TrendingUp,
  TrendingDown,
  AlertTriangle,
  Zap,
  Target,
  Shield,
  Activity,
  ArrowRight,
  Sparkles,
  Rocket,
  Crown,
  XCircle,
  RefreshCw,
} from 'lucide-solid';
import { HealthScore } from '../../../design-system';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface CustomerHealthHubProps {
  health: CustomerHealth;
  customerName?: string;
  showDetails?: boolean;
}

const _ALL_LIFECYCLE_STAGES: LifecycleStage[] = [
  'new',
  'onboarding',
  'activated',
  'engaged',
  'power_user',
  'at_risk',
  'churning',
  'churned',
  'reactivated',
];

const POSITIVE_JOURNEY: LifecycleStage[] = [
  'new',
  'onboarding',
  'activated',
  'engaged',
  'power_user',
];

const LIFECYCLE_CONFIG: Record<LifecycleStage, { 
  icon: typeof Sparkles; 
  label: string; 
  color: string;
  bg: string;
}> = {
  new: { icon: Sparkles, label: 'New', color: 'text-plasma-400', bg: 'bg-plasma-500' },
  onboarding: { icon: Rocket, label: 'Onboarding', color: 'text-photon-400', bg: 'bg-photon-500' },
  activated: { icon: Zap, label: 'Activated', color: 'text-electric-400', bg: 'bg-electric-500' },
  engaged: { icon: Activity, label: 'Engaged', color: 'text-indigo-400', bg: 'bg-indigo-500' },
  power_user: { icon: Crown, label: 'Power User', color: 'text-solar-400', bg: 'bg-solar-500' },
  at_risk: { icon: AlertTriangle, label: 'At Risk', color: 'text-flare-400', bg: 'bg-flare-500' },
  churning: { icon: TrendingDown, label: 'Churning', color: 'text-flare-500', bg: 'bg-flare-600' },
  churned: { icon: XCircle, label: 'Churned', color: 'text-nebula-500', bg: 'bg-nebula-600' },
  reactivated: { icon: RefreshCw, label: 'Reactivated', color: 'text-aurora-400', bg: 'bg-aurora-500' },
};

const getScoreColor = (score: number): string => {
  if (score >= 80) return '#10b981';
  if (score >= 60) return '#22d3d3';
  if (score >= 40) return '#f59e0b';
  return '#ef4444';
};

interface RadarChartProps {
  data: { label: string; value: number; maxValue: number }[];
  size?: number;
}

const RadarChart: Component<RadarChartProps> = (props) => {
  const size = () => props.size || 200;
  const center = () => size() / 2;
  const radius = () => (size() - 40) / 2;

  const points = createMemo(() => {
    const data = props.data;
    const angleStep = (2 * Math.PI) / data.length;
    
    return data.map((item, i) => {
      const angle = i * angleStep - Math.PI / 2;
      const normalizedValue = item.value / item.maxValue;
      const r = radius() * normalizedValue;
      return {
        x: center() + r * Math.cos(angle),
        y: center() + r * Math.sin(angle),
        labelX: center() + (radius() + 20) * Math.cos(angle),
        labelY: center() + (radius() + 20) * Math.sin(angle),
        label: item.label,
        value: item.value,
      };
    });
  });

  const pathData = createMemo(() => {
    const pts = points();
    if (pts.length === 0) return '';
    return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ') + ' Z';
  });

  const gridLevels = [0.25, 0.5, 0.75, 1];

  return (
    <svg width={size()} height={size()} class="overflow-visible">
      <defs>
        <linearGradient id="radar-gradient" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stop-color="#6366f1" stop-opacity="0.4" />
          <stop offset="100%" stop-color="#8b5cf6" stop-opacity="0.2" />
        </linearGradient>
        <filter id="radar-glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="3" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      <For each={gridLevels}>
        {(level) => {
          const r = radius() * level;
          const pts = props.data.map((_, i) => {
            const angle = (i * 2 * Math.PI) / props.data.length - Math.PI / 2;
            return {
              x: center() + r * Math.cos(angle),
              y: center() + r * Math.sin(angle),
            };
          });
          const d = pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ') + ' Z';
          return (
            <path
              d={d}
              fill="none"
              stroke="rgba(255,255,255,0.08)"
              stroke-width="1"
            />
          );
        }}
      </For>

      <For each={props.data}>
        {(_, i) => {
          const angle = (i() * 2 * Math.PI) / props.data.length - Math.PI / 2;
          return (
            <line
              x1={center()}
              y1={center()}
              x2={center() + radius() * Math.cos(angle)}
              y2={center() + radius() * Math.sin(angle)}
              stroke="rgba(255,255,255,0.05)"
              stroke-width="1"
            />
          );
        }}
      </For>

      <path
        d={pathData()}
        fill="url(#radar-gradient)"
        stroke="#6366f1"
        stroke-width="2"
        filter="url(#radar-glow)"
        class="transition-all duration-700"
      />

      <For each={points()}>
        {(point) => (
          <>
            <circle
              cx={point.x}
              cy={point.y}
              r="4"
              fill="#6366f1"
              stroke="#fff"
              stroke-width="2"
              class="transition-all duration-500"
            />
            <text
              x={point.labelX}
              y={point.labelY}
              text-anchor="middle"
              dominant-baseline="middle"
              class="fill-nebula-400 text-2xs font-bold uppercase tracking-wider"
            >
              {point.label}
            </text>
          </>
        )}
      </For>
    </svg>
  );
};

const VelocityIndicator: Component<{
  velocity: number;
  trend: 'growing' | 'stable' | 'declining';
}> = (props) => {
  const trendConfig = {
    growing: { icon: TrendingUp, color: 'text-aurora-400', bg: 'bg-aurora-500/10', label: 'Growing' },
    stable: { icon: Activity, color: 'text-electric-400', bg: 'bg-electric-500/10', label: 'Stable' },
    declining: { icon: TrendingDown, color: 'text-flare-400', bg: 'bg-flare-500/10', label: 'Declining' },
  };

  const config = () => trendConfig[props.trend];

  return (
    <div class={cn('flex items-center gap-3 rounded-2xl border border-white/5 px-4 py-3', config().bg)}>
      <div class={cn('rounded-xl p-2', config().bg)}>
        {(() => {
          const Icon = config().icon;
          return <Icon size={20} class={config().color} />;
        })()}
      </div>
      <div>
        <div class="flex items-baseline gap-2">
          <span class="font-display text-2xl font-black text-white">{props.velocity.toLocaleString()}</span>
          <span class="text-xs font-bold uppercase text-nebula-500">cmds/7d</span>
        </div>
        <span class={cn('text-xs font-bold', config().color)}>{config().label}</span>
      </div>
    </div>
  );
};

interface LifecycleJourneyProps {
  currentStage: LifecycleStage;
}

const LifecycleJourney: Component<LifecycleJourneyProps> = (props) => {
  const currentIndex = createMemo(() => {
    const idx = POSITIVE_JOURNEY.indexOf(props.currentStage);
    return idx >= 0 ? idx : -1;
  });

  const isNegativeStage = createMemo(() =>
    ['at_risk', 'churning', 'churned'].includes(props.currentStage)
  );

  const isReactivated = createMemo(() => props.currentStage === 'reactivated');

  return (
    <div class="relative">
      <Show when={isNegativeStage()}>
        <div class="mb-4 flex items-center gap-2 rounded-xl border border-flare-500/20 bg-flare-500/10 px-4 py-2">
          <AlertTriangle size={16} class="text-flare-400" />
          <span class="text-sm font-bold text-flare-400">
            {LIFECYCLE_CONFIG[props.currentStage].label}
          </span>
        </div>
      </Show>

      <Show when={isReactivated()}>
        <div class="mb-4 flex items-center gap-2 rounded-xl border border-aurora-500/20 bg-aurora-500/10 px-4 py-2">
          <RefreshCw size={16} class="text-aurora-400" />
          <span class="text-sm font-bold text-aurora-400">Reactivated</span>
        </div>
      </Show>

      <div class="relative flex items-center justify-between">
        <div class="absolute left-0 right-0 top-1/2 h-1 -translate-y-1/2 bg-void-700" />

        <Show when={currentIndex() >= 0}>
          <div
            class="absolute left-0 top-1/2 h-1 -translate-y-1/2 rounded-full transition-all duration-700"
            style={{
              width: `${Math.max(0, (currentIndex() / (POSITIVE_JOURNEY.length - 1)) * 100)}%`,
              background: `linear-gradient(90deg, var(--stage-new, #5a9ae8), ${getScoreColor(80)})`,
            }}
          />
        </Show>

        <For each={POSITIVE_JOURNEY}>
          {(stage, i) => {
            const config = LIFECYCLE_CONFIG[stage];
            const isCompleted = () => i() < currentIndex();
            const isCurrent = () => i() === currentIndex();
            const IconComponent = config.icon;

            return (
              <div class="relative z-10 flex flex-col items-center">
                <div
                  class={cn(
                    'flex h-10 w-10 items-center justify-center rounded-full border-2 transition-all duration-500',
                    isCurrent()
                      ? 'scale-125 border-white shadow-lg'
                      : isCompleted()
                        ? 'border-transparent'
                        : 'border-void-600 bg-void-850'
                  )}
                  style={{
                    'background-color': isCompleted() || isCurrent() ? `var(--stage-${stage.replace('_', '-')}, ${config.bg.replace('bg-', '')})` : undefined,
                    'box-shadow': isCurrent() ? `0 0 20px var(--stage-${stage.replace('_', '-')}, currentColor)` : undefined,
                  }}
                >
                  <Show when={isCompleted()}>
                    <svg class="h-4 w-4 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                    </svg>
                  </Show>
                  <Show when={isCurrent()}>
                    <div class="h-2 w-2 animate-pulse rounded-full bg-white" />
                  </Show>
                  <Show when={!isCompleted() && !isCurrent()}>
                    <IconComponent size={14} class="text-nebula-600" />
                  </Show>
                </div>
                <span
                  class={cn(
                    'mt-3 text-2xs font-bold uppercase tracking-wider transition-colors',
                    isCurrent() ? 'text-white' : isCompleted() ? config.color : 'text-nebula-600'
                  )}
                >
                  {config.label}
                </span>
              </div>
            );
          }}
        </For>
      </div>
    </div>
  );
};

export const CustomerHealthHub: Component<CustomerHealthHubProps> = (props) => {
  const [isExpanded, setIsExpanded] = createSignal(props.showDetails ?? true);

  const radarData = createMemo(() => [
    { label: 'Engage', value: props.health.engagement_score, maxValue: 100 },
    { label: 'Active', value: props.health.activation_score, maxValue: 100 },
    { label: 'Growth', value: props.health.growth_score, maxValue: 100 },
    { label: 'Health', value: 100 - props.health.risk_score, maxValue: 100 },
    { label: 'Expand', value: props.health.expansion_readiness_score, maxValue: 100 },
  ]);

  const riskLevel = createMemo(() => {
    const churn = props.health.predicted_churn_probability;
    if (churn >= 0.7) return { level: 'critical', color: 'text-flare-500', bg: 'bg-flare-500/10' };
    if (churn >= 0.4) return { level: 'high', color: 'text-flare-400', bg: 'bg-flare-500/10' };
    if (churn >= 0.2) return { level: 'medium', color: 'text-solar-400', bg: 'bg-solar-500/10' };
    return { level: 'low', color: 'text-aurora-400', bg: 'bg-aurora-500/10' };
  });

  return (
    <div class="relative overflow-hidden rounded-4xl border border-white/5 bg-void-850 shadow-2xl">
      <div class="pointer-events-none absolute -right-32 -top-32 h-64 w-64 rounded-full bg-indigo-500/10 blur-[100px]" />
      <div class="pointer-events-none absolute -bottom-32 -left-32 h-64 w-64 rounded-full bg-photon-500/10 blur-[100px]" />

      <div class="relative border-b border-white/5 bg-gradient-to-r from-white/[0.02] to-transparent p-8">
        <div class="flex items-start justify-between">
          <div>
            <div class="mb-2 flex items-center gap-3">
              <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-photon-600">
                <Activity size={20} class="text-white" />
              </div>
              <h2 class="font-display text-xl font-black tracking-tight text-white">Customer Health Hub</h2>
            </div>
            <Show when={props.customerName}>
              <p class="text-sm text-nebula-400">{props.customerName}</p>
            </Show>
          </div>

          <button
            onClick={() => setIsExpanded(!isExpanded())}
            class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-2 text-xs font-bold text-nebula-400 transition-all hover:bg-white/10 hover:text-white"
          >
            {isExpanded() ? 'Collapse' : 'Expand'}
            <ArrowRight
              size={14}
              class={cn('transition-transform duration-300', isExpanded() && 'rotate-90')}
            />
          </button>
        </div>
      </div>

      <div class="relative p-8">
        <div class="grid gap-8 lg:grid-cols-3">
          <div class="flex flex-col items-center justify-center lg:col-span-1">
            <HealthScore
              score={props.health.overall_score}
              size="xl"
              variant="ring"
              showLabel
              animated
            />

            <div class="mt-6 flex items-center gap-4">
              <div class={cn('rounded-full px-3 py-1.5', riskLevel().bg)}>
                <span class={cn('text-xs font-black uppercase', riskLevel().color)}>
                  {riskLevel().level} Risk
                </span>
              </div>
              <div class="rounded-full bg-void-700 px-3 py-1.5">
                <span class="text-xs font-bold text-nebula-400">
                  {(props.health.predicted_churn_probability * 100).toFixed(0)}% Churn
                </span>
              </div>
            </div>
          </div>

          <div class="flex items-center justify-center lg:col-span-1">
            <RadarChart data={radarData()} size={220} />
          </div>

          <div class="flex flex-col gap-4 lg:col-span-1">
            <div class="grid grid-cols-2 gap-3">
              <For each={[
                { label: 'Engagement', value: props.health.engagement_score, Icon: Zap },
                { label: 'Activation', value: props.health.activation_score, Icon: Target },
                { label: 'Growth', value: props.health.growth_score, Icon: TrendingUp },
                { label: 'Stability', value: 100 - props.health.risk_score, Icon: Shield },
              ]}>
                {(metric) => {
                  const IconComponent = metric.Icon;
                  return (
                    <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4 transition-all hover:bg-white/[0.04]">
                      <div class="mb-2 flex items-center justify-between">
                        <IconComponent size={14} class="text-nebula-500" />
                        <span
                          class="font-display text-lg font-black tabular-nums"
                          style={{ color: getScoreColor(metric.value) }}
                        >
                          {metric.value}
                        </span>
                      </div>
                      <span class="text-2xs font-bold uppercase tracking-wider text-nebula-500">
                        {metric.label}
                      </span>
                      <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-void-700">
                        <div
                          class="h-full rounded-full transition-all duration-700"
                          style={{
                            width: `${metric.value}%`,
                            'background-color': getScoreColor(metric.value),
                          }}
                        />
                      </div>
                    </div>
                  );
                }}
              </For>
            </div>

            <VelocityIndicator
              velocity={props.health.command_velocity_7d}
              trend={props.health.command_velocity_trend}
            />
          </div>
        </div>

        <Show when={isExpanded()}>
          <div class="mt-8 border-t border-white/5 pt-8">
            <h3 class="mb-6 text-sm font-black uppercase tracking-widest text-nebula-500">
              Lifecycle Journey
            </h3>
            <LifecycleJourney currentStage={props.health.lifecycle_stage} />

            <div class="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
              <div class="rounded-2xl border border-aurora-500/20 bg-aurora-500/5 p-4">
                <span class="text-2xs font-bold uppercase tracking-wider text-aurora-400/70">
                  Upgrade Probability
                </span>
                <div class="mt-2 flex items-baseline gap-1">
                  <span class="font-display text-3xl font-black text-aurora-400">
                    {(props.health.predicted_upgrade_probability * 100).toFixed(0)}
                  </span>
                  <span class="text-sm font-bold text-aurora-400/50">%</span>
                </div>
              </div>

              <div class="rounded-2xl border border-indigo-500/20 bg-indigo-500/5 p-4">
                <span class="text-2xs font-bold uppercase tracking-wider text-indigo-400/70">
                  Expansion Ready
                </span>
                <div class="mt-2 flex items-baseline gap-1">
                  <span class="font-display text-3xl font-black text-indigo-400">
                    {props.health.expansion_readiness_score}
                  </span>
                  <span class="text-sm font-bold text-indigo-400/50">/100</span>
                </div>
              </div>

              <div class="rounded-2xl border border-electric-500/20 bg-electric-500/5 p-4">
                <span class="text-2xs font-bold uppercase tracking-wider text-electric-400/70">
                  Velocity (7d)
                </span>
                <div class="mt-2 flex items-baseline gap-1">
                  <span class="font-display text-3xl font-black text-electric-400">
                    {props.health.command_velocity_7d.toLocaleString()}
                  </span>
                  <span class="text-sm font-bold text-electric-400/50">cmds</span>
                </div>
              </div>

              <div
                class={cn(
                  'rounded-2xl border p-4',
                  props.health.risk_score > 50
                    ? 'border-flare-500/20 bg-flare-500/5'
                    : 'border-solar-500/20 bg-solar-500/5'
                )}
              >
                <span
                  class={cn(
                    'text-2xs font-bold uppercase tracking-wider',
                    props.health.risk_score > 50 ? 'text-flare-400/70' : 'text-solar-400/70'
                  )}
                >
                  Risk Score
                </span>
                <div class="mt-2 flex items-baseline gap-1">
                  <span
                    class={cn(
                      'font-display text-3xl font-black',
                      props.health.risk_score > 50 ? 'text-flare-400' : 'text-solar-400'
                    )}
                  >
                    {props.health.risk_score}
                  </span>
                  <span
                    class={cn(
                      'text-sm font-bold',
                      props.health.risk_score > 50 ? 'text-flare-400/50' : 'text-solar-400/50'
                    )}
                  >
                    /100
                  </span>
                </div>
              </div>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
};

export default CustomerHealthHub;
