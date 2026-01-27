import { Component, createMemo, Show, splitProps, For, Switch, Match } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

type HealthLevel = 'critical' | 'poor' | 'fair' | 'good' | 'excellent';

interface HealthScoreProps {
  score: number;
  size?: 'sm' | 'md' | 'lg' | 'xl';
  variant?: 'gauge' | 'ring' | 'bar' | 'badge' | 'compact';
  showLabel?: boolean;
  showTrend?: boolean;
  trend?: number;
  animated?: boolean;
  class?: string;
}

const getHealthLevel = (score: number): HealthLevel => {
  if (score <= 20) return 'critical';
  if (score <= 40) return 'poor';
  if (score <= 60) return 'fair';
  if (score <= 80) return 'good';
  return 'excellent';
};

const healthConfig: Record<HealthLevel, { 
  color: string; 
  bg: string; 
  glow: string;
  text: string;
  gradient: string;
  label: string;
}> = {
  critical: {
    color: 'var(--health-critical, #ef4444)',
    bg: 'bg-flare-500/10',
    glow: 'shadow-[0_0_20px_rgba(239,68,68,0.4)]',
    text: 'text-flare-400',
    gradient: 'from-flare-600 to-flare-400',
    label: 'Critical',
  },
  poor: {
    color: 'var(--health-poor, #f87171)',
    bg: 'bg-flare-400/10',
    glow: 'shadow-[0_0_15px_rgba(248,113,113,0.35)]',
    text: 'text-flare-300',
    gradient: 'from-flare-500 to-flare-300',
    label: 'Poor',
  },
  fair: {
    color: 'var(--health-fair, #f59e0b)',
    bg: 'bg-solar-500/10',
    glow: 'shadow-[0_0_15px_rgba(245,158,11,0.3)]',
    text: 'text-solar-400',
    gradient: 'from-solar-600 to-solar-400',
    label: 'Fair',
  },
  good: {
    color: 'var(--health-good, #22d3d3)',
    bg: 'bg-electric-500/10',
    glow: 'shadow-[0_0_15px_rgba(34,211,211,0.3)]',
    text: 'text-electric-400',
    gradient: 'from-electric-600 to-electric-400',
    label: 'Good',
  },
  excellent: {
    color: 'var(--health-excellent, #10b981)',
    bg: 'bg-aurora-500/10',
    glow: 'shadow-[0_0_20px_rgba(16,185,129,0.4)]',
    text: 'text-aurora-400',
    gradient: 'from-aurora-600 to-aurora-400',
    label: 'Excellent',
  },
};

const sizeConfig = {
  sm: { ring: 48, stroke: 4, fontSize: 'text-sm', labelSize: 'text-2xs' },
  md: { ring: 64, stroke: 5, fontSize: 'text-lg', labelSize: 'text-xs' },
  lg: { ring: 96, stroke: 6, fontSize: 'text-2xl', labelSize: 'text-sm' },
  xl: { ring: 128, stroke: 8, fontSize: 'text-4xl', labelSize: 'text-base' },
};

export const HealthScoreRing: Component<HealthScoreProps> = (props) => {
  const [local, others] = splitProps(props, ['score', 'size', 'showLabel', 'animated', 'class']);
  
  const level = createMemo(() => getHealthLevel(local.score));
  const config = createMemo(() => healthConfig[level()]);
  const sizeValues = createMemo(() => sizeConfig[local.size || 'md']);
  
  const radius = createMemo(() => (sizeValues().ring - sizeValues().stroke) / 2);
  const circumference = createMemo(() => 2 * Math.PI * radius());
  const offset = createMemo(() => circumference() - (local.score / 100) * circumference());

  return (
    <div 
      class={cn(
        'relative inline-flex flex-col items-center justify-center',
        local.class
      )}
      {...others}
    >
      <svg
        width={sizeValues().ring}
        height={sizeValues().ring}
        class="rotate-[-90deg]"
      >
        <circle
          cx={sizeValues().ring / 2}
          cy={sizeValues().ring / 2}
          r={radius()}
          fill="none"
          stroke="currentColor"
          stroke-width={sizeValues().stroke}
          class="text-void-700"
        />
        <circle
          cx={sizeValues().ring / 2}
          cy={sizeValues().ring / 2}
          r={radius()}
          fill="none"
          stroke={config().color}
          stroke-width={sizeValues().stroke}
          stroke-linecap="round"
          stroke-dasharray={`${circumference()}`}
          stroke-dashoffset={local.animated ? circumference() : offset()}
          class={cn(
            'transition-all duration-1000',
            local.animated && 'animate-gauge-fill'
          )}
          style={{
            '--gauge-circumference': `${circumference()}`,
            '--gauge-offset': `${offset()}`,
          }}
        />
      </svg>
      <div class="absolute inset-0 flex flex-col items-center justify-center">
        <span class={cn('font-display font-black tabular-nums', config().text, sizeValues().fontSize)}>
          {local.score}
        </span>
        <Show when={local.showLabel}>
          <span class={cn('font-medium text-nebula-500 uppercase tracking-wider', sizeValues().labelSize)}>
            {config().label}
          </span>
        </Show>
      </div>
    </div>
  );
};

export const HealthScoreBadge: Component<HealthScoreProps> = (props) => {
  const level = createMemo(() => getHealthLevel(props.score));
  const config = createMemo(() => healthConfig[level()]);

  const sizeClasses = {
    sm: 'px-2 py-0.5 text-xs gap-1',
    md: 'px-2.5 py-1 text-sm gap-1.5',
    lg: 'px-3 py-1.5 text-base gap-2',
    xl: 'px-4 py-2 text-lg gap-2',
  };

  return (
    <div
      class={cn(
        'inline-flex items-center rounded-full border font-bold tabular-nums transition-all',
        config().bg,
        config().text,
        props.animated && config().glow,
        sizeClasses[props.size || 'md'],
        props.class
      )}
      style={{ 'border-color': `color-mix(in srgb, ${config().color} 30%, transparent)` }}
    >
      <span class="font-display font-black">{props.score}</span>
      <Show when={props.showLabel}>
        <span class="text-nebula-400 font-medium">/100</span>
      </Show>
      <Show when={props.showTrend && props.trend !== undefined}>
        <span class={cn(
          'flex items-center text-xs',
          props.trend! > 0 ? 'text-aurora-400' : props.trend! < 0 ? 'text-flare-400' : 'text-nebula-500'
        )}>
          {props.trend! > 0 ? '+' : ''}{props.trend}
        </span>
      </Show>
    </div>
  );
};

export const HealthScoreBar: Component<HealthScoreProps & { showSegments?: boolean }> = (props) => {
  const level = createMemo(() => getHealthLevel(props.score));
  const config = createMemo(() => healthConfig[level()]);

  const heightClasses = {
    sm: 'h-1.5',
    md: 'h-2',
    lg: 'h-3',
    xl: 'h-4',
  };

  return (
    <div class={cn('w-full', props.class)}>
      <Show when={props.showLabel}>
        <div class="flex items-center justify-between mb-1.5">
          <span class="text-xs font-bold text-nebula-400 uppercase tracking-wider">Health Score</span>
          <span class={cn('text-sm font-black tabular-nums', config().text)}>{props.score}/100</span>
        </div>
      </Show>
      <div class={cn('w-full rounded-full bg-void-700 overflow-hidden', heightClasses[props.size || 'md'])}>
        <div
          class={cn(
            'h-full rounded-full bg-gradient-to-r transition-all duration-700 ease-smooth',
            config().gradient,
            props.animated && config().glow
          )}
          style={{ width: `${props.score}%` }}
        />
      </div>
      <Show when={props.showSegments}>
        <div class="flex justify-between mt-1.5 text-2xs text-nebula-600 font-medium">
          <span>Critical</span>
          <span>Poor</span>
          <span>Fair</span>
          <span>Good</span>
          <span>Excellent</span>
        </div>
      </Show>
    </div>
  );
};

export const HealthScoreGauge: Component<HealthScoreProps> = (props) => {
  const level = createMemo(() => getHealthLevel(props.score));
  const config = createMemo(() => healthConfig[level()]);
  const sizeValues = createMemo(() => sizeConfig[props.size || 'lg']);

  const width = createMemo(() => sizeValues().ring * 1.5);
  const height = createMemo(() => sizeValues().ring);
  const strokeWidth = sizeValues().stroke + 2;
  const radius = createMemo(() => height() - strokeWidth);
  const circumference = createMemo(() => Math.PI * radius());
  const offset = createMemo(() => circumference() - (props.score / 100) * circumference());

  return (
    <div class={cn('relative inline-flex flex-col items-center', props.class)}>
      <svg
        width={width()}
        height={height() + 10}
        class="overflow-visible"
      >
        <path
          d={`M ${strokeWidth} ${height()} A ${radius()} ${radius()} 0 0 1 ${width() - strokeWidth} ${height()}`}
          fill="none"
          stroke="currentColor"
          stroke-width={strokeWidth}
          stroke-linecap="round"
          class="text-void-700"
        />
        <path
          d={`M ${strokeWidth} ${height()} A ${radius()} ${radius()} 0 0 1 ${width() - strokeWidth} ${height()}`}
          fill="none"
          stroke={config().color}
          stroke-width={strokeWidth}
          stroke-linecap="round"
          stroke-dasharray={`${circumference()}`}
          stroke-dashoffset={props.animated ? circumference() : offset()}
          class={cn(
            'transition-all duration-1000 ease-smooth',
            props.animated && 'animate-gauge-fill'
          )}
          style={{
            '--gauge-circumference': `${circumference()}`,
            '--gauge-offset': `${offset()}`,
            filter: `drop-shadow(0 0 8px ${config().color})`,
          }}
        />
        
        <For each={[0, 20, 40, 60, 80, 100]}>
          {(tick) => {
            const angle = Math.PI * (1 - tick / 100);
            const innerRadius = radius() - 12;
            const outerRadius = radius() - 6;
            const x1 = width() / 2 + innerRadius * Math.cos(angle);
            const y1 = height() - innerRadius * Math.sin(angle);
            const x2 = width() / 2 + outerRadius * Math.cos(angle);
            const y2 = height() - outerRadius * Math.sin(angle);
            return (
              <line
                x1={x1}
                y1={y1}
                x2={x2}
                y2={y2}
                stroke="currentColor"
                stroke-width="2"
                class="text-nebula-700"
              />
            );
          }}
        </For>
      </svg>
      
      <div class="absolute bottom-0 left-1/2 -translate-x-1/2 flex flex-col items-center">
        <span class={cn('font-display font-black tabular-nums', config().text, sizeValues().fontSize)}>
          {props.score}
        </span>
        <Show when={props.showLabel}>
          <span class={cn('font-medium text-nebula-500 uppercase tracking-wider', sizeValues().labelSize)}>
            {config().label}
          </span>
        </Show>
      </div>
    </div>
  );
};

export const HealthScoreCompact: Component<Omit<HealthScoreProps, 'size'>> = (props) => {
  const level = createMemo(() => getHealthLevel(props.score));
  const config = createMemo(() => healthConfig[level()]);

  return (
    <div
      class={cn(
        'inline-flex items-center gap-1.5 rounded-md px-2 py-0.5',
        config().bg,
        props.class
      )}
    >
      <div
        class="h-1.5 w-1.5 rounded-full"
        style={{ 'background-color': config().color }}
      />
      <span class={cn('text-xs font-bold tabular-nums', config().text)}>
        {props.score}
      </span>
    </div>
  );
};

export const HealthScore: Component<HealthScoreProps> = (props) => {
  const variant = () => props.variant || 'ring';

  return (
    <Switch fallback={<HealthScoreRing {...props} />}>
      <Match when={variant() === 'gauge'}>
        <HealthScoreGauge {...props} />
      </Match>
      <Match when={variant() === 'ring'}>
        <HealthScoreRing {...props} />
      </Match>
      <Match when={variant() === 'bar'}>
        <HealthScoreBar {...props} />
      </Match>
      <Match when={variant() === 'badge'}>
        <HealthScoreBadge {...props} />
      </Match>
      <Match when={variant() === 'compact'}>
        <HealthScoreCompact {...props} />
      </Match>
    </Switch>
  );
};

export default HealthScore;
