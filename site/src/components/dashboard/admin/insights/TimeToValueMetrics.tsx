import { Component, createSignal, createMemo, onMount, Show } from 'solid-js';
import { Rocket, Clock, Award, TrendingUp, Target, Zap, CheckCircle } from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface TimeToValueData {
  avg_days_to_activation: number;
  avg_days_to_power_user: number;
  pct_activated_day1: number;
  pct_activated_week1: number;
  pct_became_power_users: number;
}

interface TimeToValueMetricsProps {
  data: TimeToValueData;
}

interface MetricCardProps {
  icon: Component<{ size?: number; class?: string }>;
  label: string;
  value: string | number;
  suffix?: string;
  color: string;
  glow: string;
  delay?: number;
  benchmark?: { value: number; label: string };
  isGood?: boolean;
}

const MetricCard: Component<MetricCardProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [hovered, setHovered] = createSignal(false);

  onMount(() => {
    const timer = setTimeout(() => setMounted(true), props.delay ?? 0);
    return () => clearTimeout(timer);
  });

  const Icon = props.icon;

  return (
    <div
      class={cn(
        'relative rounded-xl border bg-void-800/40 p-4',
        'transition-all duration-300 cursor-default',
        'hover:bg-void-750/60',
        hovered() && 'border-white/15'
      )}
      style={{
        'border-color': hovered() ? `color-mix(in srgb, ${props.color} 30%, transparent)` : 'rgba(255,255,255,0.04)',
        'box-shadow': hovered() ? `0 0 20px ${props.glow}, inset 0 1px 0 rgba(255,255,255,0.03)` : undefined,
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div class="flex items-center gap-2 mb-3">
        <div
          class={cn(
            'w-8 h-8 rounded-lg flex items-center justify-center',
            'transition-transform duration-300',
            hovered() && 'scale-110'
          )}
          style={{
            background: `linear-gradient(135deg, ${props.color}, color-mix(in srgb, ${props.color} 70%, white))`,
            'box-shadow': hovered() ? `0 0 12px ${props.glow}` : `0 0 6px ${props.glow}`,
          }}
        >
          <Icon size={14} class="text-white" />
        </div>
        <span class="text-2xs font-bold uppercase tracking-wider text-nebula-500">
          {props.label}
        </span>
        <Show when={props.isGood !== undefined}>
          <CheckCircle
            size={12}
            class={cn(
              'transition-colors duration-300',
              props.isGood ? 'text-aurora-400' : 'text-nebula-600'
            )}
          />
        </Show>
      </div>

      <div
        class={cn(
          'text-2xl font-black tabular-nums transition-all duration-500',
          mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2'
        )}
        style={{ color: props.color }}
      >
        {props.value}
        <Show when={props.suffix}>
          <span class="text-sm font-medium text-nebula-500 ml-1">{props.suffix}</span>
        </Show>
      </div>

      <Show when={props.benchmark}>
        <div class="mt-2 flex items-center gap-1.5 text-2xs text-nebula-500">
          <div class="w-12 h-1 rounded-full bg-void-700 overflow-hidden">
            <div
              class="h-full rounded-full transition-all duration-700"
              style={{
                width: `${Math.min((Number(props.value) / props.benchmark!.value) * 100, 100)}%`,
                background: props.color,
              }}
            />
          </div>
          <span>{props.benchmark!.label}</span>
        </div>
      </Show>
    </div>
  );
};

export const TimeToValueMetrics: Component<TimeToValueMetricsProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const activationDays = createMemo(() => props.data.avg_days_to_activation ?? 0);
  const powerUserDays = createMemo(() => props.data.avg_days_to_power_user ?? 0);
  const day1Activated = createMemo(() => props.data.pct_activated_day1 ?? 0);
  const week1Activated = createMemo(() => props.data.pct_activated_week1 ?? 0);
  const powerUserConversion = createMemo(() => props.data.pct_became_power_users ?? 0);

  const activationHealth = createMemo(() => {
    const days = activationDays();
    if (days <= 1) return { status: 'excellent', color: 'var(--color-aurora-400)', label: 'Excellent' };
    if (days <= 3) return { status: 'good', color: 'var(--color-electric-400)', label: 'Good' };
    if (days <= 7) return { status: 'fair', color: 'var(--color-solar-400)', label: 'Fair' };
    return { status: 'slow', color: 'var(--color-flare-400)', label: 'Needs Work' };
  });

  const onboardingInsight = createMemo(() => {
    const d1 = day1Activated();
    const w1 = week1Activated();
    const pu = powerUserConversion();

    if (d1 >= 60 && pu >= 40) {
      return {
        message: 'Outstanding onboarding! Users activate quickly and convert to power users at high rates.',
        type: 'excellent' as const,
        icon: Rocket,
      };
    }
    if (d1 >= 40 && w1 >= 70) {
      return {
        message: 'Strong week-1 activation. Consider improving day-1 experience to accelerate time-to-value.',
        type: 'good' as const,
        icon: TrendingUp,
      };
    }
    if (w1 >= 50) {
      return {
        message: 'Moderate activation rate. Focus on improving first-run experience and reducing friction.',
        type: 'fair' as const,
        icon: Target,
      };
    }
    return {
      message: 'Activation needs improvement. Consider adding guided onboarding and quick-start tutorials.',
      type: 'attention' as const,
      icon: Zap,
    };
  });

  const insightColors = {
    excellent: { bg: 'rgba(16, 185, 129, 0.08)', border: 'rgba(16, 185, 129, 0.2)', text: 'var(--color-aurora-400)' },
    good: { bg: 'rgba(34, 211, 211, 0.08)', border: 'rgba(34, 211, 211, 0.2)', text: 'var(--color-electric-400)' },
    fair: { bg: 'rgba(245, 158, 11, 0.08)', border: 'rgba(245, 158, 11, 0.2)', text: 'var(--color-solar-400)' },
    attention: { bg: 'rgba(99, 102, 241, 0.08)', border: 'rgba(99, 102, 241, 0.2)', text: 'var(--color-indigo-400)' },
  };

  return (
    <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl relative overflow-hidden">
      <div
        class="absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl opacity-15"
        style={{ background: activationHealth().color }}
      />

      <div class="mb-6 flex items-start justify-between">
        <div>
          <div class="flex items-center gap-2 mb-1">
            <Rocket size={20} class="text-indigo-400" />
            <h3 class="text-lg font-bold tracking-tight text-nebula-100">Time to Value</h3>
          </div>
          <p class="text-xs text-nebula-500">Onboarding success and activation metrics</p>
        </div>

        <div
          class="px-3 py-1.5 rounded-full text-xs font-bold border"
          style={{
            color: activationHealth().color,
            'background-color': `color-mix(in srgb, ${activationHealth().color} 10%, transparent)`,
            'border-color': `color-mix(in srgb, ${activationHealth().color} 20%, transparent)`,
            'box-shadow': `0 0 10px color-mix(in srgb, ${activationHealth().color} 30%, transparent)`,
          }}
        >
          {activationHealth().label}
        </div>
      </div>

      <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-3">
        <MetricCard
          icon={Clock}
          label="Activation"
          value={activationDays().toFixed(1)}
          suffix="days avg"
          color="var(--color-indigo-400)"
          glow="rgba(129, 140, 248, 0.3)"
          delay={0}
          benchmark={{ value: 7, label: 'vs 7d target' }}
          isGood={activationDays() <= 3}
        />

        <MetricCard
          icon={Award}
          label="Power User"
          value={powerUserDays().toFixed(1)}
          suffix="days avg"
          color="var(--color-photon-400)"
          glow="rgba(176, 109, 232, 0.3)"
          delay={50}
          benchmark={{ value: 30, label: 'vs 30d target' }}
          isGood={powerUserDays() <= 14}
        />

        <MetricCard
          icon={Target}
          label="Day 1"
          value={day1Activated().toFixed(0)}
          suffix="%"
          color="var(--color-aurora-400)"
          glow="rgba(52, 211, 153, 0.3)"
          delay={100}
          isGood={day1Activated() >= 50}
        />

        <MetricCard
          icon={TrendingUp}
          label="Week 1"
          value={week1Activated().toFixed(0)}
          suffix="%"
          color="var(--color-electric-400)"
          glow="rgba(34, 211, 211, 0.3)"
          delay={150}
          isGood={week1Activated() >= 70}
        />

        <MetricCard
          icon={Award}
          label="Conversion"
          value={powerUserConversion().toFixed(0)}
          suffix="%"
          color="var(--color-solar-400)"
          glow="rgba(251, 191, 36, 0.3)"
          delay={200}
          isGood={powerUserConversion() >= 30}
        />
      </div>

      <div
        class={cn(
          'mt-6 rounded-xl border p-4 transition-all duration-500',
          mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
        )}
        style={{
          background: insightColors[onboardingInsight().type].bg,
          'border-color': insightColors[onboardingInsight().type].border,
        }}
      >
        <div class="flex items-start gap-3">
          <div
            class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0"
            style={{
              background: `linear-gradient(135deg, ${insightColors[onboardingInsight().type].text}, color-mix(in srgb, ${insightColors[onboardingInsight().type].text} 60%, white))`,
            }}
          >
            {(() => {
              const InsightIcon = onboardingInsight().icon;
              return <InsightIcon size={14} class="text-white" />;
            })()}
          </div>
          <div>
            <p
              class="text-sm font-semibold mb-0.5"
              style={{ color: insightColors[onboardingInsight().type].text }}
            >
              Onboarding Insight
            </p>
            <p class="text-xs text-nebula-400 leading-relaxed">
              {onboardingInsight().message}
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
