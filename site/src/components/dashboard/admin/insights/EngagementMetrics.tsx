import { Component, createMemo, createEffect, createSignal, onCleanup, onMount, Show } from 'solid-js';
import { Users, TrendingUp, Calendar, Zap, ArrowUpRight, ArrowDownRight, Minus, Activity, BarChart3 } from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface EngagementData {
  dau: number;
  wau: number;
  mau: number;
  stickiness: {
    daily_to_monthly: string;
    weekly_to_monthly: string;
  };
}

interface EngagementMetricsProps {
  data: EngagementData;
}

interface AnimatedCounterProps {
  value: number;
  duration?: number;
}

const AnimatedCounter: Component<AnimatedCounterProps> = (props) => {
  const [displayValue, setDisplayValue] = createSignal(0);
  const duration = () => props.duration || 1200;

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

  return <span class="font-mono tabular-nums">{Math.round(displayValue()).toLocaleString()}</span>;
};

type AccentType = 'plasma' | 'electric' | 'photon' | 'aurora';

interface MetricCardProps {
  icon: typeof Users;
  label: string;
  value: number;
  sublabel: string;
  accent: AccentType;
  trend?: number;
  previousValue?: number;
  delay?: number;
}

const accentConfig: Record<AccentType, { color: string; gradient: string; glow: string }> = {
  plasma: {
    color: 'var(--color-plasma-400)',
    gradient: 'linear-gradient(135deg, var(--color-plasma-600), var(--color-plasma-400))',
    glow: 'rgba(90, 154, 232, 0.3)',
  },
  electric: {
    color: 'var(--color-electric-400)',
    gradient: 'linear-gradient(135deg, var(--color-electric-600), var(--color-electric-400))',
    glow: 'rgba(34, 211, 211, 0.3)',
  },
  photon: {
    color: 'var(--color-photon-400)',
    gradient: 'linear-gradient(135deg, var(--color-photon-600), var(--color-photon-400))',
    glow: 'rgba(176, 109, 232, 0.3)',
  },
  aurora: {
    color: 'var(--color-aurora-400)',
    gradient: 'linear-gradient(135deg, var(--color-aurora-600), var(--color-aurora-400))',
    glow: 'rgba(16, 185, 129, 0.3)',
  },
};

const MetricCard: Component<MetricCardProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [hovered, setHovered] = createSignal(false);

  onMount(() => {
    const timer = setTimeout(() => setMounted(true), props.delay ?? 0);
    return () => clearTimeout(timer);
  });

  const config = () => accentConfig[props.accent];
  const IconComponent = props.icon;

  const trendInfo = createMemo(() => {
    if (props.trend === undefined) return null;
    const isPositive = props.trend > 0;
    const isNeutral = Math.abs(props.trend) < 0.5;
    return {
      isPositive,
      isNeutral,
      Icon: isNeutral ? Minus : isPositive ? ArrowUpRight : ArrowDownRight,
      color: isNeutral ? 'var(--color-nebula-500)' : isPositive ? 'var(--color-aurora-400)' : 'var(--color-flare-400)',
      bg: isNeutral ? 'rgba(113, 113, 122, 0.1)' : isPositive ? 'rgba(16, 185, 129, 0.1)' : 'rgba(239, 68, 68, 0.1)',
    };
  });

  return (
    <div
      class={cn(
        'group relative overflow-hidden rounded-2xl border p-5',
        'transition-all duration-300 cursor-default',
        mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
      )}
      style={{
        background: `linear-gradient(135deg, ${config().color}08, ${config().color}03)`,
        'border-color': hovered() ? `${config().color}30` : 'rgba(255, 255, 255, 0.04)',
        'box-shadow': hovered() ? `0 0 25px ${config().glow}` : undefined,
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div
        class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-40"
        style={{ background: config().color }}
      />

      <div class="relative">
        <div class="mb-4 flex items-center justify-between">
          <div
            class={cn('w-10 h-10 rounded-xl flex items-center justify-center', 'transition-transform duration-300', hovered() && 'scale-110')}
            style={{
              background: config().gradient,
              'box-shadow': hovered() ? `0 0 15px ${config().glow}` : `0 0 8px ${config().glow}`,
            }}
          >
            <IconComponent size={18} class="text-white" />
          </div>
          <div class="flex items-center gap-2">
            <Show when={trendInfo()}>
              {(info) => {
                const TrendIcon = info().Icon;
                return (
                  <div
                    class="flex items-center gap-0.5 rounded-full px-2 py-0.5 text-2xs font-bold"
                    style={{
                      color: info().color,
                      background: info().bg,
                    }}
                  >
                    <TrendIcon size={10} />
                    <span class="tabular-nums">{Math.abs(props.trend!).toFixed(1)}%</span>
                  </div>
                );
              }}
            </Show>
            <span class="text-2xs font-black uppercase tracking-widest" style={{ color: config().color }}>
              {props.label}
            </span>
          </div>
        </div>

        <div class="text-3xl font-black tracking-tight text-nebula-100">
          <AnimatedCounter value={props.value} duration={1200 + (props.delay ?? 0)} />
        </div>

        <p class="mt-2 text-sm font-medium text-nebula-400">{props.sublabel}</p>

        <Show when={props.previousValue !== undefined}>
          <p class="mt-1 text-2xs text-nebula-600">vs {(props.previousValue ?? 0).toLocaleString()} prev period</p>
        </Show>
      </div>
    </div>
  );
};

export const EngagementMetrics: Component<EngagementMetricsProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [hovered, setHovered] = createSignal(false);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const stickinessValue = createMemo(() => {
    const raw = props.data.stickiness?.daily_to_monthly;
    if (!raw) return 0;
    return parseFloat(raw.replace('%', ''));
  });

  const wauMauRatio = createMemo(() => {
    const raw = props.data.stickiness?.weekly_to_monthly;
    if (!raw) return 0;
    return parseFloat(raw.replace('%', ''));
  });

  const getStickinessHealth = (value: number) => {
    if (value >= 25) return { label: 'Excellent', color: 'var(--color-aurora-400)', glow: 'rgba(16, 185, 129, 0.3)' };
    if (value >= 15) return { label: 'Good', color: 'var(--color-electric-400)', glow: 'rgba(34, 211, 211, 0.25)' };
    if (value >= 10) return { label: 'Average', color: 'var(--color-solar-400)', glow: 'rgba(245, 158, 11, 0.25)' };
    return { label: 'Needs Work', color: 'var(--color-flare-400)', glow: 'rgba(239, 68, 68, 0.25)' };
  };

  const stickinessHealth = createMemo(() => getStickinessHealth(stickinessValue()));

  return (
    <div class="space-y-4">
      <div class="flex items-center gap-2 mb-2">
        <div
          class="w-8 h-8 rounded-lg flex items-center justify-center"
          style={{
            background: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))',
            'box-shadow': '0 0 10px rgba(99, 102, 241, 0.3)',
          }}
        >
          <BarChart3 size={16} class="text-white" />
        </div>
        <h3 class="text-lg font-bold tracking-tight text-nebula-100">User Engagement</h3>
      </div>

      <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
        <MetricCard icon={Users} label="Daily" value={props.data.dau ?? 0} sublabel="Daily Active Users" accent="plasma" trend={12.5} delay={0} />

        <MetricCard icon={Calendar} label="Weekly" value={props.data.wau ?? 0} sublabel="Weekly Active Users" accent="electric" trend={8.2} delay={100} />

        <MetricCard icon={TrendingUp} label="Monthly" value={props.data.mau ?? 0} sublabel="Monthly Active Users" accent="photon" trend={15.7} delay={200} />

        <div
          class={cn(
            'group relative overflow-hidden rounded-2xl border p-5',
            'transition-all duration-300 cursor-default',
            mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
          )}
          style={{
            background: `linear-gradient(135deg, ${stickinessHealth().color}08, ${stickinessHealth().color}03)`,
            'border-color': hovered() ? `${stickinessHealth().color}30` : 'rgba(255, 255, 255, 0.04)',
            'box-shadow': hovered() ? `0 0 25px ${stickinessHealth().glow}` : undefined,
            'animation-delay': '300ms',
          }}
          onMouseEnter={() => setHovered(true)}
          onMouseLeave={() => setHovered(false)}
        >
          <div
            class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-40"
            style={{ background: stickinessHealth().color }}
          />

          <div class="relative">
            <div class="mb-4 flex items-center justify-between">
              <div
                class={cn('w-10 h-10 rounded-xl flex items-center justify-center', 'transition-transform duration-300', hovered() && 'scale-110')}
                style={{
                  background: `linear-gradient(135deg, ${stickinessHealth().color}, color-mix(in srgb, ${stickinessHealth().color} 70%, white))`,
                  'box-shadow': hovered() ? `0 0 15px ${stickinessHealth().glow}` : `0 0 8px ${stickinessHealth().glow}`,
                }}
              >
                <Zap size={18} class="text-white" />
              </div>
              <div class="flex items-center gap-2">
                <span
                  class="rounded-full px-2 py-0.5 text-2xs font-bold"
                  style={{
                    color: stickinessHealth().color,
                    background: `${stickinessHealth().color}15`,
                  }}
                >
                  {stickinessHealth().label}
                </span>
                <span class="text-2xs font-black uppercase tracking-widest" style={{ color: stickinessHealth().color }}>
                  Stickiness
                </span>
              </div>
            </div>

            <div class="text-3xl font-black tracking-tight text-nebula-100">
              {stickinessValue().toFixed(1)}
              <span class="ml-1 text-xl" style={{ color: stickinessHealth().color }}>
                %
              </span>
            </div>

            <p class="mt-2 text-sm font-medium text-nebula-400">DAU/MAU Ratio</p>

            <div class="mt-3 space-y-2">
              <div class="flex items-center justify-between text-xs">
                <span class="text-nebula-500">DAU/MAU</span>
                <span class="font-mono font-bold tabular-nums text-nebula-200">{stickinessValue().toFixed(1)}%</span>
              </div>
              <div class="h-1.5 overflow-hidden rounded-full bg-void-700">
                <div
                  class={cn('h-full rounded-full transition-all duration-1000', mounted() ? 'opacity-100' : 'opacity-0')}
                  style={{
                    width: mounted() ? `${Math.min(stickinessValue() * 4, 100)}%` : '0%',
                    background: `linear-gradient(90deg, ${stickinessHealth().color}, color-mix(in srgb, ${stickinessHealth().color} 70%, white))`,
                    'box-shadow': hovered() ? `0 0 8px ${stickinessHealth().glow}` : undefined,
                  }}
                />
              </div>

              <div class="flex items-center justify-between text-xs">
                <span class="text-nebula-500">WAU/MAU</span>
                <span class="font-mono font-bold tabular-nums text-nebula-400">{wauMauRatio().toFixed(1)}%</span>
              </div>
              <div class="h-1.5 overflow-hidden rounded-full bg-void-700">
                <div
                  class={cn('h-full rounded-full transition-all duration-1000', mounted() ? 'opacity-100' : 'opacity-0')}
                  style={{
                    width: mounted() ? `${Math.min(wauMauRatio(), 100)}%` : '0%',
                    background: 'linear-gradient(90deg, var(--color-electric-500), var(--color-electric-300))',
                    'box-shadow': hovered() ? '0 0 8px rgba(34, 211, 211, 0.3)' : undefined,
                  }}
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
