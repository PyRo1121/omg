import { Component, Show, createMemo, createSignal, onMount, onCleanup, For } from 'solid-js';
import { 
  Users, 
  TrendingUp, 
  Activity, 
  Calendar, 
  Zap, 
  Target, 
  ArrowUpRight, 
  ArrowDownRight,
  Minus,
  RefreshCw
} from 'lucide-solid';
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
  dailyTrend?: Array<{ date: string; active_users: number; commands: number }>;
}

interface FunnelStage {
  label: string;
  count: number;
  percentage: number;
}

interface EngagementDashboardProps {
  engagement: EngagementData;
  funnel?: {
    installs: number;
    activated: number;
    engaged: number;
    power_users: number;
  };
  previousPeriod?: EngagementData;
  isLoading?: boolean;
  onRefresh?: () => void;
}

interface AnimatedCounterProps {
  value: number;
  duration?: number;
}

const AnimatedCounter: Component<AnimatedCounterProps> = (props) => {
  const [displayValue, setDisplayValue] = createSignal(0);
  const duration = () => props.duration || 1200;

  onMount(() => {
    const target = props.value;
    const startTime = Date.now();
    let animationFrame: number;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const progress = Math.min(elapsed / duration(), 1);
      const easeOutQuart = 1 - Math.pow(1 - progress, 4);
      setDisplayValue(target * easeOutQuart);

      if (progress < 1) {
        animationFrame = requestAnimationFrame(animate);
      }
    };

    animationFrame = requestAnimationFrame(animate);
    onCleanup(() => cancelAnimationFrame(animationFrame));
  });

  return <span class="font-mono tabular-nums">{Math.round(displayValue()).toLocaleString()}</span>;
};

export const EngagementDashboard: Component<EngagementDashboardProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const stickinessValue = createMemo(() => {
    const raw = props.engagement.stickiness?.daily_to_monthly;
    if (!raw) return 0;
    return parseFloat(raw.replace('%', ''));
  });

  const wauMauRatio = createMemo(() => {
    const raw = props.engagement.stickiness?.weekly_to_monthly;
    if (!raw) return 0;
    return parseFloat(raw.replace('%', ''));
  });

  const calculateTrend = (current: number, previous: number | undefined) => {
    if (!previous || previous === 0) return null;
    return ((current - previous) / previous) * 100;
  };

  const dauTrend = createMemo(() => 
    calculateTrend(props.engagement.dau, props.previousPeriod?.dau)
  );

  const wauTrend = createMemo(() => 
    calculateTrend(props.engagement.wau, props.previousPeriod?.wau)
  );

  const mauTrend = createMemo(() => 
    calculateTrend(props.engagement.mau, props.previousPeriod?.mau)
  );

  const getStickinessHealth = (value: number) => {
    if (value >= 25) return { label: 'Excellent', color: 'var(--color-aurora-400)', glow: 'rgba(16, 185, 129, 0.3)', status: 'excellent' };
    if (value >= 15) return { label: 'Good', color: 'var(--color-electric-400)', glow: 'rgba(34, 211, 211, 0.25)', status: 'good' };
    if (value >= 10) return { label: 'Average', color: 'var(--color-solar-400)', glow: 'rgba(245, 158, 11, 0.25)', status: 'average' };
    return { label: 'Needs Work', color: 'var(--color-flare-400)', glow: 'rgba(239, 68, 68, 0.25)', status: 'poor' };
  };

  const stickinessHealth = createMemo(() => getStickinessHealth(stickinessValue()));

  const funnelStages = createMemo<FunnelStage[]>(() => {
    if (!props.funnel) return [];
    const { installs, activated, engaged, power_users } = props.funnel;
    const base = installs || 1;
    
    return [
      { label: 'Installed', count: installs, percentage: 100 },
      { label: 'Activated', count: activated, percentage: (activated / base) * 100 },
      { label: 'Engaged', count: engaged, percentage: (engaged / base) * 100 },
      { label: 'Power Users', count: power_users, percentage: (power_users / base) * 100 },
    ];
  });

  const TrendIndicator: Component<{ trend: number | null }> = (trendProps) => {
    if (trendProps.trend === null) return null;
    
    const isPositive = trendProps.trend > 0;
    const isNeutral = Math.abs(trendProps.trend) < 0.5;
    const Icon = isNeutral ? Minus : isPositive ? ArrowUpRight : ArrowDownRight;
    const color = isNeutral ? 'var(--color-nebula-500)' : isPositive ? 'var(--color-aurora-400)' : 'var(--color-flare-400)';
    const bg = isNeutral ? 'rgba(113, 113, 122, 0.1)' : isPositive ? 'rgba(16, 185, 129, 0.1)' : 'rgba(239, 68, 68, 0.1)';

    return (
      <div
        class="flex items-center gap-0.5 rounded-full px-2 py-0.5 text-[10px] font-bold"
        style={{ color, background: bg }}
      >
        <Icon size={10} />
        <span class="tabular-nums">{Math.abs(trendProps.trend).toFixed(1)}%</span>
      </div>
    );
  };

  return (
    <div class="space-y-6">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div
            class="w-10 h-10 rounded-xl flex items-center justify-center"
            style={{
              background: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))',
              'box-shadow': '0 0 15px rgba(99, 102, 241, 0.3)',
            }}
          >
            <Activity size={20} class="text-white" />
          </div>
          <div>
            <h2 class="text-xl font-bold tracking-tight text-nebula-100">User Engagement</h2>
            <p class="text-xs text-nebula-500">Real-time activity and engagement metrics</p>
          </div>
        </div>

        <Show when={props.onRefresh}>
          <button
            onClick={props.onRefresh}
            disabled={props.isLoading}
            class={cn(
              'flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-2.5',
              'text-sm font-bold text-white transition-all hover:bg-white/10',
              'disabled:cursor-not-allowed disabled:opacity-50'
            )}
          >
            <RefreshCw size={16} class={props.isLoading ? 'animate-spin' : ''} />
            Refresh
          </button>
        </Show>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <MetricCard
          icon={Users}
          label="Daily Active"
          value={props.engagement.dau}
          sublabel="users today"
          color="var(--color-plasma-400)"
          gradient="linear-gradient(135deg, var(--color-plasma-600), var(--color-plasma-400))"
          glow="rgba(90, 154, 232, 0.3)"
          trend={dauTrend()}
          mounted={mounted()}
          delay={0}
        />

        <MetricCard
          icon={Calendar}
          label="Weekly Active"
          value={props.engagement.wau}
          sublabel="users this week"
          color="var(--color-electric-400)"
          gradient="linear-gradient(135deg, var(--color-electric-600), var(--color-electric-400))"
          glow="rgba(34, 211, 211, 0.3)"
          trend={wauTrend()}
          mounted={mounted()}
          delay={50}
        />

        <MetricCard
          icon={TrendingUp}
          label="Monthly Active"
          value={props.engagement.mau}
          sublabel="users this month"
          color="var(--color-photon-400)"
          gradient="linear-gradient(135deg, var(--color-photon-600), var(--color-photon-400))"
          glow="rgba(176, 109, 232, 0.3)"
          trend={mauTrend()}
          mounted={mounted()}
          delay={100}
        />

        <div
          class={cn(
            'group relative overflow-hidden rounded-2xl border p-5',
            'transition-all duration-500 cursor-default',
            mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
          )}
          style={{
            background: `linear-gradient(135deg, ${stickinessHealth().color}08, ${stickinessHealth().color}03)`,
            'border-color': 'rgba(255, 255, 255, 0.04)',
            'animation-delay': '150ms',
          }}
        >
          <div
            class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-40"
            style={{ background: stickinessHealth().color }}
          />

          <div class="relative">
            <div class="mb-4 flex items-center justify-between">
              <div
                class="w-10 h-10 rounded-xl flex items-center justify-center transition-transform duration-300 group-hover:scale-110"
                style={{
                  background: `linear-gradient(135deg, ${stickinessHealth().color}, color-mix(in srgb, ${stickinessHealth().color} 70%, white))`,
                  'box-shadow': `0 0 10px ${stickinessHealth().glow}`,
                }}
              >
                <Zap size={18} class="text-white" />
              </div>
              <div class="flex items-center gap-2">
                <span
                  class="rounded-full px-2 py-0.5 text-[10px] font-bold"
                  style={{
                    color: stickinessHealth().color,
                    background: `color-mix(in srgb, ${stickinessHealth().color} 15%, transparent)`,
                  }}
                >
                  {stickinessHealth().label}
                </span>
                <span 
                  class="text-[10px] font-black uppercase tracking-widest"
                  style={{ color: stickinessHealth().color }}
                >
                  Stickiness
                </span>
              </div>
            </div>

            <div class="text-3xl font-black tracking-tight text-nebula-100">
              {stickinessValue().toFixed(1)}
              <span class="ml-1 text-xl" style={{ color: stickinessHealth().color }}>%</span>
            </div>

            <p class="mt-2 text-sm font-medium text-nebula-400">DAU/MAU Ratio</p>

            <div class="mt-3 space-y-2">
              <div class="flex items-center justify-between text-xs">
                <span class="text-nebula-500">DAU/MAU</span>
                <span class="font-mono font-bold tabular-nums text-nebula-200">{stickinessValue().toFixed(1)}%</span>
              </div>
              <div class="h-1.5 overflow-hidden rounded-full bg-void-700">
                <div
                  class="h-full rounded-full transition-all duration-1000"
                  style={{
                    width: mounted() ? `${Math.min(stickinessValue() * 4, 100)}%` : '0%',
                    background: `linear-gradient(90deg, ${stickinessHealth().color}, color-mix(in srgb, ${stickinessHealth().color} 70%, white))`,
                  }}
                />
              </div>

              <div class="flex items-center justify-between text-xs">
                <span class="text-nebula-500">WAU/MAU</span>
                <span class="font-mono font-bold tabular-nums text-nebula-400">{wauMauRatio().toFixed(1)}%</span>
              </div>
              <div class="h-1.5 overflow-hidden rounded-full bg-void-700">
                <div
                  class="h-full rounded-full transition-all duration-1000"
                  style={{
                    width: mounted() ? `${Math.min(wauMauRatio(), 100)}%` : '0%',
                    background: 'linear-gradient(90deg, var(--color-electric-500), var(--color-electric-300))',
                  }}
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      <Show when={props.funnel && funnelStages().length > 0}>
        <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6">
          <div class="mb-4 flex items-center gap-2">
            <Target size={18} class="text-aurora-400" />
            <h3 class="text-lg font-bold text-nebula-100">Engagement Funnel</h3>
          </div>

          <div class="space-y-3">
            <For each={funnelStages()}>
              {(stage, index) => {
                const colors = [
                  { gradient: 'linear-gradient(90deg, var(--color-indigo-600), var(--color-indigo-400))', glow: 'rgba(99, 102, 241, 0.3)' },
                  { gradient: 'linear-gradient(90deg, var(--color-electric-600), var(--color-electric-400))', glow: 'rgba(34, 211, 211, 0.3)' },
                  { gradient: 'linear-gradient(90deg, var(--color-aurora-600), var(--color-aurora-400))', glow: 'rgba(16, 185, 129, 0.3)' },
                  { gradient: 'linear-gradient(90deg, var(--color-photon-600), var(--color-photon-400))', glow: 'rgba(176, 109, 232, 0.3)' },
                ];
                const stageColors = colors[index() % colors.length];
                const prevStage = funnelStages()[index() - 1];
                const dropoff = prevStage ? ((prevStage.count - stage.count) / prevStage.count * 100).toFixed(1) : null;

                return (
                  <div class="flex items-center gap-4">
                    <div class="w-24 text-sm font-medium text-nebula-200">{stage.label}</div>
                    <div class="flex-1">
                      <div class="h-8 rounded-lg bg-void-800 overflow-hidden relative">
                        <div
                          class="h-full rounded-lg transition-all duration-1000"
                          style={{
                            width: mounted() ? `${stage.percentage}%` : '0%',
                            background: stageColors.gradient,
                            'box-shadow': `inset 0 1px 0 rgba(255,255,255,0.1), 0 0 10px ${stageColors.glow}`,
                          }}
                        />
                        <div class="absolute inset-0 flex items-center px-3">
                          <span class="text-sm font-bold text-white drop-shadow-lg">
                            {stage.count.toLocaleString()}
                          </span>
                          <span class="ml-2 text-xs text-white/70">
                            ({stage.percentage.toFixed(1)}%)
                          </span>
                        </div>
                      </div>
                    </div>
                    <div class="w-16 text-right">
                      <Show when={dropoff !== null}>
                        <span class="text-xs text-flare-400/80">-{dropoff}%</span>
                      </Show>
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
};

interface MetricCardProps {
  icon: Component<{ size?: number; class?: string }>;
  label: string;
  value: number;
  sublabel: string;
  color: string;
  gradient: string;
  glow: string;
  trend?: number | null;
  mounted: boolean;
  delay: number;
}

const MetricCard: Component<MetricCardProps> = (props) => {
  const [hovered, setHovered] = createSignal(false);
  const IconComponent = props.icon;

  const TrendIndicator: Component<{ trend: number | null | undefined }> = (trendProps) => {
    if (trendProps.trend === null || trendProps.trend === undefined) return null;
    
    const isPositive = trendProps.trend > 0;
    const isNeutral = Math.abs(trendProps.trend) < 0.5;
    const Icon = isNeutral ? Minus : isPositive ? ArrowUpRight : ArrowDownRight;
    const color = isNeutral ? 'var(--color-nebula-500)' : isPositive ? 'var(--color-aurora-400)' : 'var(--color-flare-400)';
    const bg = isNeutral ? 'rgba(113, 113, 122, 0.1)' : isPositive ? 'rgba(16, 185, 129, 0.1)' : 'rgba(239, 68, 68, 0.1)';

    return (
      <div
        class="flex items-center gap-0.5 rounded-full px-2 py-0.5 text-[10px] font-bold"
        style={{ color, background: bg }}
      >
        <Icon size={10} />
        <span class="tabular-nums">{Math.abs(trendProps.trend).toFixed(1)}%</span>
      </div>
    );
  };

  return (
    <div
      class={cn(
        'group relative overflow-hidden rounded-2xl border p-5',
        'transition-all duration-500 cursor-default',
        props.mounted ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
      )}
      style={{
        background: `linear-gradient(135deg, ${props.color}08, ${props.color}03)`,
        'border-color': hovered() ? `color-mix(in srgb, ${props.color} 30%, transparent)` : 'rgba(255, 255, 255, 0.04)',
        'box-shadow': hovered() ? `0 0 25px ${props.glow}` : undefined,
        'animation-delay': `${props.delay}ms`,
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div
        class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-40"
        style={{ background: props.color }}
      />

      <div class="relative">
        <div class="mb-4 flex items-center justify-between">
          <div
            class={cn(
              'w-10 h-10 rounded-xl flex items-center justify-center',
              'transition-transform duration-300',
              hovered() && 'scale-110'
            )}
            style={{
              background: props.gradient,
              'box-shadow': hovered() ? `0 0 15px ${props.glow}` : `0 0 8px ${props.glow}`,
            }}
          >
            <IconComponent size={18} class="text-white" />
          </div>
          <div class="flex items-center gap-2">
            <TrendIndicator trend={props.trend} />
            <span 
              class="text-[10px] font-black uppercase tracking-widest"
              style={{ color: props.color }}
            >
              {props.label}
            </span>
          </div>
        </div>

        <div class="text-3xl font-black tracking-tight text-nebula-100">
          <AnimatedCounter value={props.value} duration={1200 + props.delay} />
        </div>

        <p class="mt-2 text-sm font-medium text-nebula-400">{props.sublabel}</p>
      </div>
    </div>
  );
};

export default EngagementDashboard;
