import { Component, createMemo, createSignal, createEffect, Show, splitProps, JSX } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { TrendingUp, TrendingDown, Minus, DollarSign, Users, AlertTriangle, Target } from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

type KPIVariant = 'revenue' | 'users' | 'churn' | 'expansion';
type KPISize = 'sm' | 'md' | 'lg';
type TrendDirection = 'up' | 'down' | 'neutral';

interface KPICardProps {
  value: number;
  label: string;
  change?: number;
  trend?: TrendDirection;
  icon?: JSX.Element;
  variant?: KPIVariant;
  size?: KPISize;
  prefix?: string;
  suffix?: string;
  subtitle?: string;
  loading?: boolean;
  invertTrend?: boolean;
  animated?: boolean;
  sparklineData?: number[];
  class?: string;
}

const variantConfig: Record<KPIVariant, {
  iconBg: string;
  iconColor: string;
  sparklineColor: string;
  glowColor: string;
  defaultIcon: typeof DollarSign;
}> = {
  revenue: {
    iconBg: 'bg-aurora-500/10',
    iconColor: 'text-aurora-400',
    sparklineColor: 'var(--metric-revenue)',
    glowColor: 'rgba(16, 185, 129, 0.15)',
    defaultIcon: DollarSign,
  },
  users: {
    iconBg: 'bg-electric-500/10',
    iconColor: 'text-electric-400',
    sparklineColor: 'var(--metric-users)',
    glowColor: 'rgba(34, 211, 211, 0.15)',
    defaultIcon: Users,
  },
  churn: {
    iconBg: 'bg-flare-500/10',
    iconColor: 'text-flare-400',
    sparklineColor: 'var(--metric-churn)',
    glowColor: 'rgba(239, 68, 68, 0.15)',
    defaultIcon: AlertTriangle,
  },
  expansion: {
    iconBg: 'bg-photon-500/10',
    iconColor: 'text-photon-400',
    sparklineColor: 'var(--metric-expansion)',
    glowColor: 'rgba(157, 78, 221, 0.15)',
    defaultIcon: Target,
  },
};

const sizeConfig: Record<KPISize, {
  padding: string;
  titleSize: string;
  valueSize: string;
  subtitleSize: string;
  iconSize: number;
  iconPadding: string;
  sparklineHeight: number;
}> = {
  sm: {
    padding: 'p-4',
    titleSize: 'text-2xs',
    valueSize: 'text-2xl',
    subtitleSize: 'text-2xs',
    iconSize: 16,
    iconPadding: 'p-2 rounded-lg',
    sparklineHeight: 24,
  },
  md: {
    padding: 'p-6',
    titleSize: 'text-xs',
    valueSize: 'text-4xl',
    subtitleSize: 'text-xs',
    iconSize: 20,
    iconPadding: 'p-3 rounded-xl',
    sparklineHeight: 28,
  },
  lg: {
    padding: 'p-8',
    titleSize: 'text-sm',
    valueSize: 'text-5xl',
    subtitleSize: 'text-sm',
    iconSize: 24,
    iconPadding: 'p-4 rounded-2xl',
    sparklineHeight: 32,
  },
};

const AnimatedCounter: Component<{
  value: number;
  prefix?: string;
  suffix?: string;
  decimals?: number;
  animated?: boolean;
}> = (props) => {
  const [displayValue, setDisplayValue] = createSignal(props.animated ? 0 : props.value);
  const decimals = () => props.decimals ?? (props.value < 100 ? 1 : 0);

  createEffect(() => {
    if (!props.animated) {
      setDisplayValue(props.value);
      return;
    }

    const target = props.value;
    const startTime = Date.now();
    const startValue = displayValue();
    const duration = 1500;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const progress = Math.min(elapsed / duration, 1);
      const easeOutQuart = 1 - Math.pow(1 - progress, 4);
      const current = startValue + (target - startValue) * easeOutQuart;

      setDisplayValue(current);

      if (progress < 1) {
        requestAnimationFrame(animate);
      }
    };

    requestAnimationFrame(animate);
  });

  const formattedValue = createMemo(() => {
    const val = displayValue();
    if (val >= 1000000) return `${(val / 1000000).toFixed(decimals())}M`;
    if (val >= 1000) return `${(val / 1000).toFixed(decimals())}k`;
    return val.toFixed(decimals());
  });

  return (
    <span class="tabular-nums">
      {props.prefix}{formattedValue()}{props.suffix}
    </span>
  );
};

const TrendIndicator: Component<{
  value: number;
  inverted?: boolean;
  size?: KPISize;
}> = (props) => {
  const isPositive = createMemo(() => 
    props.inverted ? props.value < 0 : props.value > 0
  );

  const sizeClasses = {
    sm: 'px-1.5 py-0.5 text-2xs gap-0.5',
    md: 'px-2 py-0.5 text-xs gap-1',
    lg: 'px-2.5 py-1 text-sm gap-1',
  };

  const iconSize = () => props.size === 'lg' ? 14 : 12;

  return (
    <div
      class={cn(
        'inline-flex items-center rounded-full font-bold transition-colors',
        sizeClasses[props.size || 'md'],
        isPositive() 
          ? 'bg-aurora-500/10 text-aurora-400' 
          : 'bg-flare-500/10 text-flare-400'
      )}
    >
      <Show when={props.value > 0}>
        <TrendingUp size={iconSize()} />
      </Show>
      <Show when={props.value < 0}>
        <TrendingDown size={iconSize()} />
      </Show>
      <Show when={props.value === 0}>
        <Minus size={iconSize()} />
      </Show>
      <span class="tabular-nums">
        {props.value > 0 ? '+' : ''}{Math.abs(props.value).toFixed(1)}%
      </span>
    </div>
  );
};

const MiniSparkline: Component<{
  data: number[];
  color: string;
  height: number;
}> = (props) => {
  const width = 70;

  const pathData = createMemo(() => {
    const data = props.data;
    if (data.length < 2) return '';

    const max = Math.max(...data);
    const min = Math.min(...data);
    const range = max - min || 1;
    const padding = 2;

    const points = data.map((value, i) => {
      const x = padding + (i / (data.length - 1)) * (width - padding * 2);
      const y = padding + (1 - (value - min) / range) * (props.height - padding * 2);
      return `${x},${y}`;
    });

    return `M ${points.join(' L ')}`;
  });

  const areaPath = createMemo(() => {
    const data = props.data;
    if (data.length < 2) return '';

    const max = Math.max(...data);
    const min = Math.min(...data);
    const range = max - min || 1;
    const padding = 2;

    const points = data.map((value, i) => {
      const x = padding + (i / (data.length - 1)) * (width - padding * 2);
      const y = padding + (1 - (value - min) / range) * (props.height - padding * 2);
      return { x, y };
    });

    let d = `M ${padding},${props.height - padding}`;
    points.forEach((p) => {
      d += ` L ${p.x},${p.y}`;
    });
    d += ` L ${width - padding},${props.height - padding} Z`;

    return d;
  });

  const lastPointY = createMemo(() => {
    const data = props.data;
    if (data.length === 0) return props.height / 2;
    const max = Math.max(...data);
    const min = Math.min(...data);
    const range = max - min || 1;
    const lastValue = data[data.length - 1];
    return 2 + (1 - (lastValue - min) / range) * (props.height - 4);
  });

  const gradientId = `sparkline-gradient-${Math.random().toString(36).substr(2, 9)}`;

  return (
    <svg width={width} height={props.height} class="overflow-visible">
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color={props.color} stop-opacity="0.3" />
          <stop offset="100%" stop-color={props.color} stop-opacity="0" />
        </linearGradient>
      </defs>
      <path
        d={areaPath()}
        fill={`url(#${gradientId})`}
      />
      <path
        d={pathData()}
        fill="none"
        stroke={props.color}
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <circle
        cx={width - 2}
        cy={lastPointY()}
        r="3"
        fill={props.color}
        class="animate-pulse"
      />
    </svg>
  );
};

export const KPICard: Component<KPICardProps> = (props) => {
  const [local, others] = splitProps(props, [
    'value', 'label', 'change', 'trend', 'icon', 'variant', 'size',
    'prefix', 'suffix', 'subtitle', 'loading', 'invertTrend', 'animated',
    'sparklineData', 'class'
  ]);

  const variant = () => variantConfig[local.variant || 'revenue'];
  const size = () => sizeConfig[local.size || 'md'];
  const animated = () => local.animated !== false;

  const DefaultIcon = variant().defaultIcon;

  return (
    <div
      class={cn(
        'group relative overflow-hidden rounded-3xl border transition-all duration-300',
        'bg-void-850 border-white/5',
        'hover:border-white/10 hover:translate-y-[-2px]',
        'shadow-card hover:shadow-card-hover',
        size().padding,
        local.class
      )}
      role="article"
      aria-label={`${local.label}: ${local.prefix || ''}${local.value}${local.suffix || ''}`}
      {...others}
    >
      <div
        class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-100"
        style={{ 'background-color': variant().glowColor }}
      />

      <div class="relative flex items-start justify-between">
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2 flex-wrap">
            <span class={cn(
              'font-bold uppercase tracking-widest text-nebula-500',
              size().titleSize
            )}>
              {local.label}
            </span>
            <Show when={local.change !== undefined}>
              <TrendIndicator 
                value={local.change!} 
                inverted={local.invertTrend} 
                size={local.size}
              />
            </Show>
          </div>

          <div class="mt-3">
            <Show
              when={!local.loading}
              fallback={
                <div class={cn(
                  'animate-pulse rounded-lg bg-white/5',
                  local.size === 'lg' ? 'h-14 w-40' : local.size === 'sm' ? 'h-8 w-24' : 'h-10 w-32'
                )} />
              }
            >
              <span class={cn(
                'font-display font-black tracking-tight text-white',
                size().valueSize
              )}>
                <AnimatedCounter
                  value={local.value}
                  prefix={local.prefix}
                  suffix={local.suffix}
                  animated={animated()}
                />
              </span>
            </Show>
          </div>

          <Show when={local.subtitle}>
            <p class={cn('text-nebula-500 mt-2 font-medium', size().subtitleSize)}>
              {local.subtitle}
            </p>
          </Show>
        </div>

        <div class="flex flex-col items-end gap-3">
          <div class={cn(variant().iconBg, size().iconPadding, 'transition-transform duration-500 group-hover:scale-110')}>
            <Show when={local.icon} fallback={
              <DefaultIcon size={size().iconSize} class={variant().iconColor} />
            }>
              {local.icon}
            </Show>
          </div>

          <Show when={local.sparklineData && local.sparklineData.length > 1}>
            <div class="opacity-50 transition-opacity duration-500 group-hover:opacity-100">
              <MiniSparkline
                data={local.sparklineData!}
                color={variant().sparklineColor}
                height={size().sparklineHeight}
              />
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
};

export interface KPIGridProps {
  columns?: 2 | 3 | 4;
  class?: string;
  children: JSX.Element;
}

export const KPIGrid: Component<KPIGridProps> = (props) => {
  const colClasses = {
    2: 'grid-cols-1 md:grid-cols-2',
    3: 'grid-cols-1 md:grid-cols-2 lg:grid-cols-3',
    4: 'grid-cols-1 md:grid-cols-2 xl:grid-cols-4',
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

export default KPICard;
