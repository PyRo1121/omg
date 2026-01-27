import { Component, For, Show, createMemo, createSignal, splitProps } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { Activity, Users, Calendar, TrendingUp, TrendingDown, Zap } from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

type EngagementMetricType = 'dau' | 'wau' | 'mau' | 'stickiness';
type GaugeSize = 'sm' | 'md' | 'lg' | 'xl';
type EngagementLevel = 'dormant' | 'casual' | 'regular' | 'active' | 'power';

interface EngagementGaugeProps {
  dau: number;
  wau: number;
  mau: number;
  previousDau?: number;
  previousWau?: number;
  previousMau?: number;
  size?: GaugeSize;
  variant?: 'rings' | 'bars' | 'cards';
  showStickiness?: boolean;
  showTrend?: boolean;
  animated?: boolean;
  class?: string;
}

interface SingleGaugeProps {
  value: number;
  max: number;
  label: string;
  type: EngagementMetricType;
  size?: GaugeSize;
  change?: number;
  animated?: boolean;
  class?: string;
}

const metricConfig: Record<EngagementMetricType, {
  color: string;
  bgClass: string;
  glowClass: string;
  icon: typeof Activity;
  label: string;
}> = {
  dau: {
    color: 'var(--metric-dau, #10b981)',
    bgClass: 'bg-aurora-500/10',
    glowClass: 'shadow-[0_0_20px_rgba(16,185,129,0.3)]',
    icon: Zap,
    label: 'Daily Active',
  },
  wau: {
    color: 'var(--metric-wau, #22d3d3)',
    bgClass: 'bg-electric-500/10',
    glowClass: 'shadow-[0_0_20px_rgba(34,211,211,0.3)]',
    icon: Activity,
    label: 'Weekly Active',
  },
  mau: {
    color: 'var(--metric-mau, #3b7dd1)',
    bgClass: 'bg-plasma-500/10',
    glowClass: 'shadow-[0_0_20px_rgba(59,125,209,0.3)]',
    icon: Users,
    label: 'Monthly Active',
  },
  stickiness: {
    color: 'var(--metric-stickiness, #9d4edd)',
    bgClass: 'bg-photon-500/10',
    glowClass: 'shadow-[0_0_20px_rgba(157,78,221,0.3)]',
    icon: Calendar,
    label: 'Stickiness',
  },
};

const sizeConfig: Record<GaugeSize, {
  diameter: number;
  strokeWidth: number;
  fontSize: string;
  labelSize: string;
  iconSize: number;
}> = {
  sm: { diameter: 80, strokeWidth: 6, fontSize: 'text-lg', labelSize: 'text-2xs', iconSize: 14 },
  md: { diameter: 120, strokeWidth: 8, fontSize: 'text-2xl', labelSize: 'text-xs', iconSize: 16 },
  lg: { diameter: 160, strokeWidth: 10, fontSize: 'text-3xl', labelSize: 'text-sm', iconSize: 20 },
  xl: { diameter: 200, strokeWidth: 12, fontSize: 'text-4xl', labelSize: 'text-base', iconSize: 24 },
};

const getEngagementLevel = (stickiness: number): EngagementLevel => {
  if (stickiness >= 75) return 'power';
  if (stickiness >= 50) return 'active';
  if (stickiness >= 25) return 'regular';
  if (stickiness >= 10) return 'casual';
  return 'dormant';
};

const engagementLevelLabels: Record<EngagementLevel, { label: string; color: string }> = {
  dormant: { label: 'Dormant', color: 'text-nebula-500' },
  casual: { label: 'Casual', color: 'text-plasma-400' },
  regular: { label: 'Regular', color: 'text-electric-400' },
  active: { label: 'Active', color: 'text-aurora-400' },
  power: { label: 'Power Users', color: 'text-solar-400' },
};

export const SingleGauge: Component<SingleGaugeProps> = (props) => {
  const [local, others] = splitProps(props, [
    'value', 'max', 'label', 'type', 'size', 'change', 'animated', 'class'
  ]);

  const config = () => sizeConfig[local.size || 'md'];
  const metric = () => metricConfig[local.type];
  const percentage = createMemo(() => Math.min((local.value / local.max) * 100, 100));

  const radius = createMemo(() => (config().diameter - config().strokeWidth) / 2);
  const circumference = createMemo(() => 2 * Math.PI * radius());
  const offset = createMemo(() => circumference() - (percentage() / 100) * circumference());

  const gradientId = `gauge-${local.type}-${Math.random().toString(36).substr(2, 9)}`;

  const Icon = metric().icon;

  return (
    <div
      class={cn('relative inline-flex flex-col items-center', local.class)}
      role="meter"
      aria-valuenow={local.value}
      aria-valuemin={0}
      aria-valuemax={local.max}
      aria-label={`${local.label}: ${local.value.toLocaleString()}`}
      {...others}
    >
      <svg
        width={config().diameter}
        height={config().diameter}
        class="rotate-[-90deg]"
      >
        <defs>
          <linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stop-color={metric().color} stop-opacity="1" />
            <stop offset="100%" stop-color={metric().color} stop-opacity="0.6" />
          </linearGradient>
          <filter id={`glow-${gradientId}`}>
            <feGaussianBlur stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        <circle
          cx={config().diameter / 2}
          cy={config().diameter / 2}
          r={radius()}
          fill="none"
          stroke="var(--gauge-track, var(--color-void-700))"
          stroke-width={config().strokeWidth}
          class="opacity-60"
        />

        <circle
          cx={config().diameter / 2}
          cy={config().diameter / 2}
          r={radius()}
          fill="none"
          stroke={`url(#${gradientId})`}
          stroke-width={config().strokeWidth}
          stroke-linecap="round"
          stroke-dasharray={`${circumference()}`}
          stroke-dashoffset={local.animated ? circumference() : offset()}
          filter={`url(#glow-${gradientId})`}
          class={cn(
            'transition-all duration-1000 ease-smooth',
            local.animated && 'animate-gauge-fill'
          )}
          style={{
            '--gauge-circumference': `${circumference()}`,
            '--gauge-offset': `${offset()}`,
          }}
        />
      </svg>

      <div class="absolute inset-0 flex flex-col items-center justify-center">
        <div class={cn(metric().bgClass, 'rounded-xl p-2 mb-1')}>
          <Icon size={config().iconSize} style={{ color: metric().color }} />
        </div>
        <span
          class={cn('font-display font-black tabular-nums text-white', config().fontSize)}
        >
          {local.value >= 1000 ? `${(local.value / 1000).toFixed(1)}k` : local.value.toLocaleString()}
        </span>
        <span class={cn('font-bold uppercase tracking-widest text-nebula-500', config().labelSize)}>
          {local.label}
        </span>
        <Show when={local.change !== undefined}>
          <div class={cn(
            'flex items-center gap-0.5 mt-1',
            local.change! >= 0 ? 'text-aurora-400' : 'text-flare-400',
            config().labelSize
          )}>
            {local.change! >= 0 ? <TrendingUp size={12} /> : <TrendingDown size={12} />}
            <span class="font-bold tabular-nums">
              {local.change! >= 0 ? '+' : ''}{local.change!.toFixed(1)}%
            </span>
          </div>
        </Show>
      </div>
    </div>
  );
};

export const EngagementGauge: Component<EngagementGaugeProps> = (props) => {
  const [local, others] = splitProps(props, [
    'dau', 'wau', 'mau', 'previousDau', 'previousWau', 'previousMau',
    'size', 'variant', 'showStickiness', 'showTrend', 'animated', 'class'
  ]);

  const stickiness = createMemo(() => local.mau > 0 ? (local.dau / local.mau) * 100 : 0);
  const engagementLevel = createMemo(() => getEngagementLevel(stickiness()));
  const levelConfig = createMemo(() => engagementLevelLabels[engagementLevel()]);

  const dauChange = createMemo(() => {
    if (local.previousDau === undefined || local.previousDau === 0) return undefined;
    return ((local.dau - local.previousDau) / local.previousDau) * 100;
  });

  const wauChange = createMemo(() => {
    if (local.previousWau === undefined || local.previousWau === 0) return undefined;
    return ((local.wau - local.previousWau) / local.previousWau) * 100;
  });

  const mauChange = createMemo(() => {
    if (local.previousMau === undefined || local.previousMau === 0) return undefined;
    return ((local.mau - local.previousMau) / local.previousMau) * 100;
  });

  const variant = () => local.variant || 'rings';

  return (
    <div class={cn('space-y-6', local.class)} {...others}>
      <Show when={variant() === 'rings'}>
        <div class="flex items-center justify-center gap-8">
          <SingleGauge
            value={local.dau}
            max={local.mau}
            label="DAU"
            type="dau"
            size={local.size}
            change={local.showTrend ? dauChange() : undefined}
            animated={local.animated}
          />
          <SingleGauge
            value={local.wau}
            max={local.mau}
            label="WAU"
            type="wau"
            size={local.size}
            change={local.showTrend ? wauChange() : undefined}
            animated={local.animated}
          />
          <SingleGauge
            value={local.mau}
            max={local.mau}
            label="MAU"
            type="mau"
            size={local.size}
            change={local.showTrend ? mauChange() : undefined}
            animated={local.animated}
          />
        </div>
      </Show>

      <Show when={variant() === 'bars'}>
        <div class="space-y-4">
          <EngagementBar
            value={local.dau}
            max={local.mau}
            label="Daily Active Users"
            type="dau"
            change={local.showTrend ? dauChange() : undefined}
            animated={local.animated}
          />
          <EngagementBar
            value={local.wau}
            max={local.mau}
            label="Weekly Active Users"
            type="wau"
            change={local.showTrend ? wauChange() : undefined}
            animated={local.animated}
          />
          <EngagementBar
            value={local.mau}
            max={local.mau}
            label="Monthly Active Users"
            type="mau"
            change={local.showTrend ? mauChange() : undefined}
            animated={local.animated}
          />
        </div>
      </Show>

      <Show when={variant() === 'cards'}>
        <div class="grid grid-cols-3 gap-4">
          <EngagementCard
            value={local.dau}
            type="dau"
            change={local.showTrend ? dauChange() : undefined}
          />
          <EngagementCard
            value={local.wau}
            type="wau"
            change={local.showTrend ? wauChange() : undefined}
          />
          <EngagementCard
            value={local.mau}
            type="mau"
            change={local.showTrend ? mauChange() : undefined}
          />
        </div>
      </Show>

      <Show when={local.showStickiness}>
        <div class="flex items-center justify-center gap-6 rounded-2xl border border-white/5 bg-void-850 p-6">
          <div class="flex-1">
            <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              DAU/MAU Stickiness
            </div>
            <div class="mt-2 flex items-baseline gap-2">
              <span class="font-display text-4xl font-black tabular-nums" style={{ color: metricConfig.stickiness.color }}>
                {stickiness().toFixed(1)}%
              </span>
              <span class={cn('text-sm font-bold', levelConfig().color)}>
                {levelConfig().label}
              </span>
            </div>
            <div class="mt-3">
              <StickinessBar value={stickiness()} animated={local.animated} />
            </div>
          </div>
          <div class="h-20 w-px bg-white/5" />
          <div class="flex flex-col gap-2 text-sm">
            <div class="flex items-center gap-2">
              <div class="h-2 w-2 rounded-full bg-nebula-600" />
              <span class="text-nebula-500">Dormant</span>
              <span class="ml-auto text-nebula-400">&lt;10%</span>
            </div>
            <div class="flex items-center gap-2">
              <div class="h-2 w-2 rounded-full bg-plasma-500" />
              <span class="text-nebula-500">Casual</span>
              <span class="ml-auto text-nebula-400">10-25%</span>
            </div>
            <div class="flex items-center gap-2">
              <div class="h-2 w-2 rounded-full bg-electric-500" />
              <span class="text-nebula-500">Regular</span>
              <span class="ml-auto text-nebula-400">25-50%</span>
            </div>
            <div class="flex items-center gap-2">
              <div class="h-2 w-2 rounded-full bg-aurora-500" />
              <span class="text-nebula-500">Active</span>
              <span class="ml-auto text-nebula-400">50-75%</span>
            </div>
            <div class="flex items-center gap-2">
              <div class="h-2 w-2 rounded-full bg-solar-400" />
              <span class="text-nebula-500">Power</span>
              <span class="ml-auto text-nebula-400">75%+</span>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

const EngagementBar: Component<{
  value: number;
  max: number;
  label: string;
  type: EngagementMetricType;
  change?: number;
  animated?: boolean;
}> = (props) => {
  const metric = () => metricConfig[props.type];
  const percentage = createMemo(() => (props.value / props.max) * 100);
  const Icon = metric().icon;

  return (
    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <div class={cn(metric().bgClass, 'rounded-lg p-1.5')}>
            <Icon size={14} style={{ color: metric().color }} />
          </div>
          <span class="text-sm font-medium text-nebula-300">{props.label}</span>
        </div>
        <div class="flex items-center gap-2">
          <span class="font-display text-lg font-black tabular-nums text-white">
            {props.value.toLocaleString()}
          </span>
          <Show when={props.change !== undefined}>
            <span class={cn(
              'text-xs font-bold tabular-nums',
              props.change! >= 0 ? 'text-aurora-400' : 'text-flare-400'
            )}>
              {props.change! >= 0 ? '+' : ''}{props.change!.toFixed(1)}%
            </span>
          </Show>
        </div>
      </div>
      <div class="h-2 rounded-full bg-void-700 overflow-hidden">
        <div
          class={cn(
            'h-full rounded-full transition-all duration-1000 ease-smooth',
            props.animated && 'animate-[score-fill_1s_ease-out_forwards]'
          )}
          style={{
            width: `${percentage()}%`,
            background: metric().color,
            'box-shadow': `0 0 10px ${metric().color}`,
          }}
        />
      </div>
    </div>
  );
};

const EngagementCard: Component<{
  value: number;
  type: EngagementMetricType;
  change?: number;
}> = (props) => {
  const metric = () => metricConfig[props.type];
  const Icon = metric().icon;

  return (
    <div class={cn(
      'rounded-2xl border border-white/5 bg-void-850 p-5',
      'hover:border-white/10 hover:translate-y-[-2px] transition-all duration-300',
      'group'
    )}>
      <div class="flex items-start justify-between">
        <div class={cn(metric().bgClass, 'rounded-xl p-3 transition-transform duration-500 group-hover:scale-110')}>
          <Icon size={20} style={{ color: metric().color }} />
        </div>
        <Show when={props.change !== undefined}>
          <div class={cn(
            'flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-bold',
            props.change! >= 0 ? 'bg-aurora-500/10 text-aurora-400' : 'bg-flare-500/10 text-flare-400'
          )}>
            {props.change! >= 0 ? <TrendingUp size={12} /> : <TrendingDown size={12} />}
            {Math.abs(props.change!).toFixed(1)}%
          </div>
        </Show>
      </div>
      <div class="mt-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          {metric().label}
        </div>
        <div class="mt-1 font-display text-3xl font-black tabular-nums text-white">
          {props.value >= 1000000
            ? `${(props.value / 1000000).toFixed(1)}M`
            : props.value >= 1000
            ? `${(props.value / 1000).toFixed(1)}k`
            : props.value.toLocaleString()}
        </div>
      </div>
    </div>
  );
};

const StickinessBar: Component<{ value: number; animated?: boolean }> = (props) => {
  const getColor = (value: number) => {
    if (value >= 75) return 'bg-solar-400';
    if (value >= 50) return 'bg-aurora-500';
    if (value >= 25) return 'bg-electric-500';
    if (value >= 10) return 'bg-plasma-500';
    return 'bg-nebula-600';
  };

  return (
    <div class="relative h-3 rounded-full bg-void-700 overflow-hidden">
      <div class="absolute inset-0 flex">
        <div class="w-[10%] border-r border-void-600" />
        <div class="w-[15%] border-r border-void-600" />
        <div class="w-[25%] border-r border-void-600" />
        <div class="w-[25%] border-r border-void-600" />
        <div class="w-[25%]" />
      </div>
      <div
        class={cn(
          'absolute inset-y-0 left-0 rounded-full transition-all duration-1000 ease-smooth',
          getColor(props.value),
          props.animated && 'animate-[score-fill_1s_ease-out_forwards]'
        )}
        style={{
          width: `${Math.min(props.value, 100)}%`,
          'box-shadow': `0 0 10px currentColor`,
        }}
      />
    </div>
  );
};

export default EngagementGauge;
