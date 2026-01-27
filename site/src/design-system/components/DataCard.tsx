import { Component, JSX, Show, splitProps, ParentComponent } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { TrendingUp, TrendingDown, Minus } from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface DataCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  icon?: JSX.Element;
  trend?: {
    value: number;
    direction: 'up' | 'down' | 'neutral';
    period?: string;
  };
  variant?: 'default' | 'glass' | 'gradient' | 'outline' | 'elevated';
  size?: 'sm' | 'md' | 'lg';
  accent?: 'indigo' | 'electric' | 'aurora' | 'solar' | 'flare' | 'photon';
  loading?: boolean;
  class?: string;
}

const accentConfig = {
  indigo: {
    gradient: 'from-indigo-500/10 to-transparent',
    border: 'border-indigo-500/20',
    glow: 'shadow-[0_0_30px_-5px_rgba(99,102,241,0.3)]',
    icon: 'text-indigo-400 bg-indigo-500/10',
    trend: 'text-indigo-400',
  },
  electric: {
    gradient: 'from-electric-500/10 to-transparent',
    border: 'border-electric-500/20',
    glow: 'shadow-[0_0_30px_-5px_rgba(34,211,211,0.3)]',
    icon: 'text-electric-400 bg-electric-500/10',
    trend: 'text-electric-400',
  },
  aurora: {
    gradient: 'from-aurora-500/10 to-transparent',
    border: 'border-aurora-500/20',
    glow: 'shadow-[0_0_30px_-5px_rgba(16,185,129,0.3)]',
    icon: 'text-aurora-400 bg-aurora-500/10',
    trend: 'text-aurora-400',
  },
  solar: {
    gradient: 'from-solar-500/10 to-transparent',
    border: 'border-solar-500/20',
    glow: 'shadow-[0_0_30px_-5px_rgba(245,158,11,0.3)]',
    icon: 'text-solar-400 bg-solar-500/10',
    trend: 'text-solar-400',
  },
  flare: {
    gradient: 'from-flare-500/10 to-transparent',
    border: 'border-flare-500/20',
    glow: 'shadow-[0_0_30px_-5px_rgba(239,68,68,0.3)]',
    icon: 'text-flare-400 bg-flare-500/10',
    trend: 'text-flare-400',
  },
  photon: {
    gradient: 'from-photon-500/10 to-transparent',
    border: 'border-photon-500/20',
    glow: 'shadow-[0_0_30px_-5px_rgba(157,78,221,0.3)]',
    icon: 'text-photon-400 bg-photon-500/10',
    trend: 'text-photon-400',
  },
};

const sizeConfig = {
  sm: {
    padding: 'p-4',
    title: 'text-2xs',
    value: 'text-xl',
    subtitle: 'text-2xs',
    icon: 'p-2 rounded-lg',
    iconSize: 16,
  },
  md: {
    padding: 'p-6',
    title: 'text-xs',
    value: 'text-3xl',
    subtitle: 'text-xs',
    icon: 'p-3 rounded-xl',
    iconSize: 20,
  },
  lg: {
    padding: 'p-8',
    title: 'text-sm',
    value: 'text-4xl',
    subtitle: 'text-sm',
    icon: 'p-4 rounded-2xl',
    iconSize: 24,
  },
};

const variantClasses = {
  default: 'bg-void-850 border border-white/5',
  glass: 'backdrop-blur-xl bg-white/[0.03] border border-white/10',
  gradient: 'bg-gradient-to-br border',
  outline: 'bg-transparent border-2',
  elevated: 'bg-void-800 border border-white/5 shadow-xl',
};

export const DataCard: Component<DataCardProps> = (props) => {
  const [local, others] = splitProps(props, [
    'title', 'value', 'subtitle', 'icon', 'trend', 'variant', 'size', 'accent', 'loading', 'class'
  ]);

  const accent = () => accentConfig[local.accent || 'indigo'];
  const size = () => sizeConfig[local.size || 'md'];
  const variant = () => local.variant || 'default';

  const TrendIcon = local.trend?.direction === 'up' 
    ? TrendingUp 
    : local.trend?.direction === 'down' 
    ? TrendingDown 
    : Minus;

  const trendColor = local.trend?.direction === 'up'
    ? 'text-aurora-400'
    : local.trend?.direction === 'down'
    ? 'text-flare-400'
    : 'text-nebula-500';

  return (
    <div
      class={cn(
        'rounded-3xl transition-all duration-300 hover:translate-y-[-2px]',
        variantClasses[variant()],
        variant() === 'gradient' && cn(accent().gradient, accent().border),
        variant() === 'outline' && accent().border,
        size().padding,
        'hover:shadow-card-hover',
        local.class
      )}
      {...others}
    >
      <div class="flex items-start justify-between">
        <div class="flex-1 min-w-0">
          <p class={cn(
            'font-bold uppercase tracking-widest text-nebula-500',
            size().title
          )}>
            {local.title}
          </p>
          
          <Show when={!local.loading} fallback={
            <div class={cn('h-9 w-24 rounded-lg bg-white/5 animate-pulse mt-2', size().value)} />
          }>
            <h3 class={cn(
              'font-display font-black tracking-tight text-white mt-2 tabular-nums',
              size().value
            )}>
              {local.value}
            </h3>
          </Show>

          <Show when={local.trend}>
            <div class={cn('flex items-center gap-1 mt-2', trendColor)}>
              <TrendIcon size={14} />
              <span class="text-xs font-bold">
                {local.trend!.direction === 'up' ? '+' : local.trend!.direction === 'down' ? '' : ''}
                {local.trend!.value}%
              </span>
              <Show when={local.trend!.period}>
                <span class="text-nebula-500 font-medium">{local.trend!.period}</span>
              </Show>
            </div>
          </Show>

          <Show when={local.subtitle}>
            <p class={cn('text-nebula-500 mt-2 font-medium', size().subtitle)}>
              {local.subtitle}
            </p>
          </Show>
        </div>

        <Show when={local.icon}>
          <div class={cn(accent().icon, size().icon)}>
            {local.icon}
          </div>
        </Show>
      </div>
    </div>
  );
};

interface MetricGridProps {
  columns?: 2 | 3 | 4;
  class?: string;
}

export const MetricGrid: ParentComponent<MetricGridProps> = (props) => {
  const colClasses = {
    2: 'grid-cols-1 md:grid-cols-2',
    3: 'grid-cols-1 md:grid-cols-2 lg:grid-cols-3',
    4: 'grid-cols-1 md:grid-cols-2 lg:grid-cols-4',
  };

  return (
    <div class={cn(
      'grid gap-6',
      colClasses[props.columns || 4],
      props.class
    )}>
      {props.children}
    </div>
  );
};

interface SparklineCardProps extends DataCardProps {
  data: number[];
  sparklineColor?: string;
}

export const SparklineCard: Component<SparklineCardProps> = (props) => {
  const maxValue = () => Math.max(...props.data, 1);
  const minValue = () => Math.min(...props.data);
  const range = () => maxValue() - minValue() || 1;

  const points = () => {
    const width = 100;
    const height = 30;
    return props.data.map((value, index) => {
      const x = (index / (props.data.length - 1)) * width;
      const y = height - ((value - minValue()) / range()) * height;
      return `${x},${y}`;
    }).join(' ');
  };

  const areaPath = () => {
    const width = 100;
    const height = 30;
    let path = `M 0,${height}`;
    props.data.forEach((value, index) => {
      const x = (index / (props.data.length - 1)) * width;
      const y = height - ((value - minValue()) / range()) * height;
      path += ` L ${x},${y}`;
    });
    path += ` L ${width},${height} Z`;
    return path;
  };

  return (
    <DataCard {...props}>
      <div class="mt-4 h-8">
        <svg viewBox="0 0 100 30" class="w-full h-full" preserveAspectRatio="none">
          <defs>
            <linearGradient id={`sparkline-gradient-${props.title}`} x1="0%" y1="0%" x2="0%" y2="100%">
              <stop offset="0%" stop-color={props.sparklineColor || '#6366f1'} stop-opacity="0.3" />
              <stop offset="100%" stop-color={props.sparklineColor || '#6366f1'} stop-opacity="0" />
            </linearGradient>
          </defs>
          <path
            d={areaPath()}
            fill={`url(#sparkline-gradient-${props.title})`}
          />
          <polyline
            points={points()}
            fill="none"
            stroke={props.sparklineColor || '#6366f1'}
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </div>
    </DataCard>
  );
};

interface ComparisonCardProps extends DataCardProps {
  previousValue: string | number;
  previousLabel?: string;
}

export const ComparisonCard: Component<ComparisonCardProps> = (props) => {
  return (
    <DataCard {...props}>
      <div class="mt-4 pt-4 border-t border-white/5">
        <div class="flex items-center justify-between">
          <span class="text-xs text-nebula-500 font-medium">
            {props.previousLabel || 'Previous period'}
          </span>
          <span class="text-sm font-bold text-nebula-400 tabular-nums">
            {props.previousValue}
          </span>
        </div>
      </div>
    </DataCard>
  );
};

export default DataCard;
