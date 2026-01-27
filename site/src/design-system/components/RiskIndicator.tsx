import { Component, Show, For, createMemo, Switch, Match } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { 
  AlertTriangle, 
  AlertCircle, 
  Info, 
  CheckCircle,
  TrendingDown,
  Shield
} from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type RiskLevel = 'critical' | 'high' | 'medium' | 'low' | 'healthy';

interface RiskIndicatorProps {
  level: RiskLevel;
  probability?: number;
  variant?: 'badge' | 'bar' | 'gauge' | 'card' | 'compact';
  size?: 'sm' | 'md' | 'lg';
  showLabel?: boolean;
  animated?: boolean;
  class?: string;
}

const riskConfig: Record<RiskLevel, {
  label: string;
  icon: typeof AlertTriangle;
  color: string;
  bg: string;
  border: string;
  gradient: string;
  glow: string;
  barColor: string;
}> = {
  critical: {
    label: 'Critical Risk',
    icon: AlertTriangle,
    color: 'text-flare-400',
    bg: 'bg-flare-500/10',
    border: 'border-flare-500/25',
    gradient: 'from-flare-600 to-flare-400',
    glow: 'shadow-[0_0_25px_rgba(239,68,68,0.35)]',
    barColor: '#ef4444',
  },
  high: {
    label: 'High Risk',
    icon: AlertCircle,
    color: 'text-orange-400',
    bg: 'bg-orange-500/10',
    border: 'border-orange-500/25',
    gradient: 'from-orange-600 to-orange-400',
    glow: 'shadow-[0_0_20px_rgba(249,115,22,0.3)]',
    barColor: '#f97316',
  },
  medium: {
    label: 'Medium Risk',
    icon: Info,
    color: 'text-solar-400',
    bg: 'bg-solar-500/10',
    border: 'border-solar-500/25',
    gradient: 'from-solar-600 to-solar-400',
    glow: 'shadow-[0_0_15px_rgba(245,158,11,0.25)]',
    barColor: '#f59e0b',
  },
  low: {
    label: 'Low Risk',
    icon: Shield,
    color: 'text-lime-400',
    bg: 'bg-lime-500/10',
    border: 'border-lime-500/25',
    gradient: 'from-lime-600 to-lime-400',
    glow: 'shadow-[0_0_12px_rgba(132,204,22,0.2)]',
    barColor: '#84cc16',
  },
  healthy: {
    label: 'Healthy',
    icon: CheckCircle,
    color: 'text-aurora-400',
    bg: 'bg-aurora-500/10',
    border: 'border-aurora-500/25',
    gradient: 'from-aurora-600 to-aurora-400',
    glow: 'shadow-[0_0_15px_rgba(16,185,129,0.25)]',
    barColor: '#10b981',
  },
};

const sizeConfig = {
  sm: {
    badge: 'px-2 py-0.5 text-2xs gap-1',
    icon: 12,
    bar: 'h-1.5',
    gauge: 48,
  },
  md: {
    badge: 'px-2.5 py-1 text-xs gap-1.5',
    icon: 14,
    bar: 'h-2',
    gauge: 64,
  },
  lg: {
    badge: 'px-3 py-1.5 text-sm gap-2',
    icon: 16,
    bar: 'h-3',
    gauge: 80,
  },
};

export const RiskIndicator: Component<RiskIndicatorProps> = (props) => {
  const config = createMemo(() => riskConfig[props.level]);
  const size = createMemo(() => sizeConfig[props.size || 'md']);
  const showLabel = () => props.showLabel !== false;
  const variant = () => props.variant || 'badge';

  const IconComponent = config().icon;

  if (variant() === 'bar') {
    const probabilityWidth = () => props.probability !== undefined ? props.probability : 
      props.level === 'critical' ? 90 :
      props.level === 'high' ? 70 :
      props.level === 'medium' ? 50 :
      props.level === 'low' ? 30 : 10;

    return (
      <div class={cn('w-full', props.class)}>
        <Show when={showLabel()}>
          <div class="flex items-center justify-between mb-2">
            <div class="flex items-center gap-2">
              <IconComponent size={size().icon} class={config().color} />
              <span class={cn('font-bold uppercase tracking-wider', config().color)}>
                {config().label}
              </span>
            </div>
            <Show when={props.probability !== undefined}>
              <span class={cn('font-mono text-sm font-bold tabular-nums', config().color)}>
                {props.probability}%
              </span>
            </Show>
          </div>
        </Show>
        <div class={cn('w-full rounded-full bg-void-700 overflow-hidden', size().bar)}>
          <div
            class={cn(
              'h-full rounded-full bg-gradient-to-r transition-all duration-700 ease-smooth',
              config().gradient,
              props.animated && config().glow
            )}
            style={{ width: `${probabilityWidth()}%` }}
          />
        </div>
      </div>
    );
  }

  if (variant() === 'gauge') {
    const probability = () => props.probability ?? 50;
    const gaugeSize = size().gauge;
    const strokeWidth = 6;
    const radius = (gaugeSize - strokeWidth) / 2;
    const circumference = 2 * Math.PI * radius;
    const offset = circumference - (probability() / 100) * circumference;

    return (
      <div class={cn('relative inline-flex items-center justify-center', props.class)}>
        <svg width={gaugeSize} height={gaugeSize} class="rotate-[-90deg]">
          <circle
            cx={gaugeSize / 2}
            cy={gaugeSize / 2}
            r={radius}
            fill="none"
            stroke="currentColor"
            stroke-width={strokeWidth}
            class="text-void-700"
          />
          <circle
            cx={gaugeSize / 2}
            cy={gaugeSize / 2}
            r={radius}
            fill="none"
            stroke={config().barColor}
            stroke-width={strokeWidth}
            stroke-linecap="round"
            stroke-dasharray={`${circumference}`}
            stroke-dashoffset={props.animated ? circumference : offset}
            class={cn('transition-all duration-1000 ease-smooth', props.animated && 'animate-gauge-fill')}
            style={{
              '--gauge-circumference': `${circumference}`,
              '--gauge-offset': `${offset}`,
              filter: `drop-shadow(0 0 6px ${config().barColor})`,
            }}
          />
        </svg>
        <div class="absolute inset-0 flex flex-col items-center justify-center">
          <IconComponent size={size().icon} class={config().color} />
          <Show when={showLabel()}>
            <span class="text-2xs font-bold text-nebula-500 mt-1">{probability()}%</span>
          </Show>
        </div>
      </div>
    );
  }

  if (variant() === 'card') {
    return (
      <div
        class={cn(
          'rounded-2xl border p-4 transition-all',
          config().bg,
          config().border,
          props.animated && config().glow,
          props.class
        )}
      >
        <div class="flex items-center gap-3">
          <div class={cn('p-2 rounded-xl', config().bg)}>
            <IconComponent size={size().icon + 4} class={config().color} />
          </div>
          <div class="flex-1 min-w-0">
            <p class={cn('font-bold', config().color)}>{config().label}</p>
            <Show when={props.probability !== undefined}>
              <p class="text-xs text-nebula-500">
                {props.probability}% churn probability
              </p>
            </Show>
          </div>
        </div>
      </div>
    );
  }

  if (variant() === 'compact') {
    return (
      <div
        class={cn(
          'inline-flex items-center gap-1.5 rounded-md px-2 py-0.5',
          config().bg,
          props.class
        )}
      >
        <IconComponent size={12} class={config().color} />
        <span class={cn('text-xs font-bold', config().color)}>
          {props.probability !== undefined ? `${props.probability}%` : config().label.split(' ')[0]}
        </span>
      </div>
    );
  }

  return (
    <div
      class={cn(
        'inline-flex items-center rounded-full border font-bold uppercase tracking-wider transition-all',
        config().bg,
        config().border,
        config().color,
        props.animated && config().glow,
        size().badge,
        props.class
      )}
    >
      <IconComponent size={size().icon} />
      <Show when={showLabel()}>
        <span>{config().label}</span>
      </Show>
      <Show when={props.probability !== undefined}>
        <span class="font-mono tabular-nums">{props.probability}%</span>
      </Show>
    </div>
  );
};

interface RiskSegmentsProps {
  segments: Array<{
    level: RiskLevel;
    count: number;
    tier?: string;
    avgCommands?: number;
  }>;
  onSegmentClick?: (level: RiskLevel) => void;
  class?: string;
}

export const RiskSegments: Component<RiskSegmentsProps> = (props) => {
  const totalAtRisk = createMemo(() =>
    props.segments
      .filter(s => s.level !== 'healthy')
      .reduce((sum, s) => sum + s.count, 0)
  );

  return (
    <div class={cn('space-y-3', props.class)}>
      <div class="flex items-center justify-between mb-4">
        <h4 class="text-lg font-bold text-white">Risk Analysis</h4>
        <span class="text-sm text-nebula-500">
          {totalAtRisk()} customers at risk
        </span>
      </div>
      
      <For each={props.segments}>
        {(segment) => {
          const config = riskConfig[segment.level];
          const IconComponent = config.icon;

          return (
            <button
              onClick={() => props.onSegmentClick?.(segment.level)}
              class={cn(
                'w-full rounded-xl border p-4 text-left transition-all',
                'hover:translate-x-1 hover:shadow-lg',
                config.bg,
                config.border
              )}
            >
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <IconComponent size={18} class={config.color} />
                  <div>
                    <p class={cn('text-sm font-bold', config.color)}>
                      {config.label.replace(' Risk', '')}
                    </p>
                    <Show when={segment.tier}>
                      <p class="text-xs text-nebula-500 capitalize">{segment.tier} tier</p>
                    </Show>
                  </div>
                </div>
                <div class="text-right">
                  <p class="text-2xl font-black text-white tabular-nums">{segment.count}</p>
                  <Show when={segment.avgCommands !== undefined}>
                    <p class="text-xs text-nebula-500">
                      {Math.round(segment.avgCommands!)} avg cmds/mo
                    </p>
                  </Show>
                </div>
              </div>
            </button>
          );
        }}
      </For>

      <Show when={totalAtRisk() > 0}>
        <div class="mt-4 rounded-xl border border-flare-500/30 bg-flare-500/5 p-4">
          <p class="text-sm font-medium text-flare-400 flex items-center gap-2">
            <TrendingDown size={16} />
            Action Required: Reach out to high-risk customers before they churn
          </p>
        </div>
      </Show>
    </div>
  );
};

export default RiskIndicator;
