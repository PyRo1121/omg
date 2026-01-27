import { Component, For, Show, createMemo, splitProps } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import {
  Check,
  Clock,
  AlertCircle,
  SkipForward,
  Loader2,
  Zap,
  TrendingUp,
  TrendingDown,
  Target,
  Calendar,
  Timer,
} from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

type MilestoneStatus = 'pending' | 'active' | 'completed' | 'skipped' | 'blocked';
type TTVSpeed = 'slow' | 'average' | 'fast' | 'excellent';
type ScheduleStatus = 'ahead' | 'on-schedule' | 'behind' | 'at-risk';

interface Milestone {
  id: string;
  label: string;
  description?: string;
  status: MilestoneStatus;
  completedAt?: string;
  estimatedDays?: number;
  actualDays?: number;
}

interface TimelineMetricProps {
  milestones: Milestone[];
  title?: string;
  currentDays?: number;
  benchmarkDays?: number;
  variant?: 'horizontal' | 'vertical';
  showProgress?: boolean;
  showBenchmark?: boolean;
  animated?: boolean;
  compact?: boolean;
  class?: string;
}

interface TTVProgressProps {
  currentDays: number;
  benchmarkDays: number;
  targetDays?: number;
  showLabels?: boolean;
  animated?: boolean;
  class?: string;
}

const statusConfig: Record<MilestoneStatus, {
  icon: typeof Check;
  color: string;
  bgClass: string;
  borderClass: string;
  label: string;
}> = {
  pending: {
    icon: Clock,
    color: 'var(--ttv-stage-pending, #52525a)',
    bgClass: 'bg-nebula-600/10',
    borderClass: 'border-nebula-600/20',
    label: 'Pending',
  },
  active: {
    icon: Loader2,
    color: 'var(--ttv-stage-active, #6366f1)',
    bgClass: 'bg-indigo-500/10',
    borderClass: 'border-indigo-500/30',
    label: 'In Progress',
  },
  completed: {
    icon: Check,
    color: 'var(--ttv-stage-completed, #10b981)',
    bgClass: 'bg-aurora-500/10',
    borderClass: 'border-aurora-500/30',
    label: 'Completed',
  },
  skipped: {
    icon: SkipForward,
    color: 'var(--ttv-stage-skipped, #71717a)',
    bgClass: 'bg-nebula-500/5',
    borderClass: 'border-nebula-500/15',
    label: 'Skipped',
  },
  blocked: {
    icon: AlertCircle,
    color: 'var(--ttv-stage-blocked, #ef4444)',
    bgClass: 'bg-flare-500/10',
    borderClass: 'border-flare-500/25',
    label: 'Blocked',
  },
};

const speedConfig: Record<TTVSpeed, {
  label: string;
  color: string;
  bgClass: string;
  icon: typeof Zap;
}> = {
  slow: {
    label: 'Slow',
    color: 'var(--ttv-speed-slow, #f87171)',
    bgClass: 'bg-flare-400/10',
    icon: TrendingDown,
  },
  average: {
    label: 'Average',
    color: 'var(--ttv-speed-average, #fbbf24)',
    bgClass: 'bg-solar-400/10',
    icon: Timer,
  },
  fast: {
    label: 'Fast',
    color: 'var(--ttv-speed-fast, #34d399)',
    bgClass: 'bg-aurora-400/10',
    icon: TrendingUp,
  },
  excellent: {
    label: 'Excellent',
    color: 'var(--ttv-speed-excellent, #2ee8e8)',
    bgClass: 'bg-electric-400/10',
    icon: Zap,
  },
};

const scheduleConfig: Record<ScheduleStatus, {
  label: string;
  color: string;
}> = {
  ahead: { label: 'Ahead of Schedule', color: 'var(--ttv-ahead-of-schedule, #10b981)' },
  'on-schedule': { label: 'On Schedule', color: 'var(--ttv-on-schedule, #22d3d3)' },
  behind: { label: 'Behind Schedule', color: 'var(--ttv-behind-schedule, #f59e0b)' },
  'at-risk': { label: 'At Risk', color: 'var(--ttv-at-risk, #ef4444)' },
};

const getSpeed = (currentDays: number, benchmarkDays: number): TTVSpeed => {
  const ratio = currentDays / benchmarkDays;
  if (ratio <= 0.5) return 'excellent';
  if (ratio <= 0.75) return 'fast';
  if (ratio <= 1.0) return 'average';
  return 'slow';
};

const getScheduleStatus = (currentDays: number, benchmarkDays: number): ScheduleStatus => {
  const ratio = currentDays / benchmarkDays;
  if (ratio <= 0.75) return 'ahead';
  if (ratio <= 1.0) return 'on-schedule';
  if (ratio <= 1.25) return 'behind';
  return 'at-risk';
};

const MilestoneIcon: Component<{ status: MilestoneStatus; size?: number }> = (props) => {
  const config = () => statusConfig[props.status];
  const Icon = config().icon;
  const isActive = () => props.status === 'active';

  return (
    <div
      class={cn(
        'flex items-center justify-center rounded-full transition-all duration-300',
        config().bgClass,
        'border-2',
        config().borderClass,
        isActive() && 'animate-pulse shadow-[0_0_15px_rgba(99,102,241,0.25)]'
      )}
      style={{ width: `${(props.size || 32) + 8}px`, height: `${(props.size || 32) + 8}px` }}
    >
      <Icon
        size={props.size || 16}
        class={cn(isActive() && 'animate-spin')}
        style={{ color: config().color }}
      />
    </div>
  );
};

export const TimelineMetric: Component<TimelineMetricProps> = (props) => {
  const [local, others] = splitProps(props, [
    'milestones', 'title', 'currentDays', 'benchmarkDays', 'variant',
    'showProgress', 'showBenchmark', 'animated', 'compact', 'class'
  ]);

  const variant = () => local.variant || 'horizontal';
  const completedCount = createMemo(() => local.milestones.filter(m => m.status === 'completed').length);
  const progress = createMemo(() => (completedCount() / local.milestones.length) * 100);
  
  const speed = createMemo(() => {
    if (local.currentDays === undefined || local.benchmarkDays === undefined) return null;
    return getSpeed(local.currentDays, local.benchmarkDays);
  });

  const schedule = createMemo(() => {
    if (local.currentDays === undefined || local.benchmarkDays === undefined) return null;
    return getScheduleStatus(local.currentDays, local.benchmarkDays);
  });

  return (
    <div class={cn('space-y-4', local.class)} {...others}>
      <Show when={local.title || local.showProgress}>
        <div class="flex items-center justify-between">
          <Show when={local.title}>
            <h4 class="text-sm font-bold text-white">{local.title}</h4>
          </Show>
          <Show when={local.showProgress}>
            <div class="flex items-center gap-3">
              <div class="flex items-center gap-1.5 text-xs text-nebula-400">
                <Target size={14} />
                <span class="font-medium tabular-nums">
                  {completedCount()}/{local.milestones.length} complete
                </span>
              </div>
              <Show when={speed()}>
                {(s) => {
                  const SpeedIcon = speedConfig[s()].icon;
                  return (
                    <div
                      class="flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-bold"
                      style={{
                        color: speedConfig[s()].color,
                        'background-color': `${speedConfig[s()].color}15`,
                      }}
                    >
                      <SpeedIcon size={12} />
                      {speedConfig[s()].label}
                    </div>
                  );
                }}
              </Show>
            </div>
          </Show>
        </div>
      </Show>

      <Show when={variant() === 'horizontal'}>
        <div class="relative">
          <div class="absolute inset-x-0 top-5 h-0.5 bg-void-700" />
          <div
            class={cn(
              'absolute left-0 top-5 h-0.5 transition-all duration-1000 ease-smooth',
              local.animated && 'animate-[score-fill_1s_ease-out_forwards]'
            )}
            style={{
              width: `${progress()}%`,
              background: 'linear-gradient(90deg, var(--color-indigo-600), var(--color-indigo-500), var(--color-electric-500))',
              'box-shadow': '0 0 10px rgba(99, 102, 241, 0.3)',
            }}
          />

          <div class="relative flex justify-between">
            <For each={local.milestones}>
              {(milestone, index) => {
                const statusColor = milestone.status === 'completed' ? 'text-aurora-400' :
                  milestone.status === 'active' ? 'text-indigo-400' :
                  milestone.status === 'blocked' ? 'text-flare-400' :
                  'text-nebula-400';
                
                const daysColor = milestone.estimatedDays && (milestone.actualDays ?? 0) <= milestone.estimatedDays
                  ? 'text-aurora-400'
                  : 'text-solar-400';

                return (
                  <div
                    class={cn(
                      'flex flex-col items-center',
                      local.animated && 'animate-fade-up'
                    )}
                    style={local.animated ? { 'animation-delay': `${index() * 100}ms` } : {}}
                  >
                    <MilestoneIcon status={milestone.status} size={local.compact ? 14 : 16} />
                    <div class="mt-3 text-center">
                      <div class={cn('font-medium', local.compact ? 'text-2xs' : 'text-xs', statusColor)}>
                        {milestone.label}
                      </div>
                      <Show when={!local.compact && milestone.completedAt}>
                        <div class="mt-0.5 text-2xs text-nebula-500">
                          {milestone.completedAt}
                        </div>
                      </Show>
                      <Show when={!local.compact && milestone.actualDays !== undefined}>
                        <div class={cn('mt-0.5 text-2xs font-medium tabular-nums', daysColor)}>
                          {milestone.actualDays}d
                          <Show when={milestone.estimatedDays}>
                            <span class="text-nebula-500"> / {milestone.estimatedDays}d</span>
                          </Show>
                        </div>
                      </Show>
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </div>
      </Show>

      <Show when={local.showBenchmark && local.currentDays !== undefined && local.benchmarkDays !== undefined}>
        <TTVProgress
          currentDays={local.currentDays!}
          benchmarkDays={local.benchmarkDays!}
          showLabels
          animated={local.animated}
        />
      </Show>
    </div>
  );
};

export const TTVProgress: Component<TTVProgressProps> = (props) => {
  const [local, others] = splitProps(props, [
    'currentDays', 'benchmarkDays', 'targetDays', 'showLabels', 'animated', 'class'
  ]);

  const maxDays = createMemo(() => Math.max(local.currentDays, local.benchmarkDays, local.targetDays ?? 0) * 1.2);
  const currentPercent = createMemo(() => (local.currentDays / maxDays()) * 100);
  const benchmarkPercent = createMemo(() => (local.benchmarkDays / maxDays()) * 100);
  const targetPercent = createMemo(() => local.targetDays ? (local.targetDays / maxDays()) * 100 : null);

  const schedule = createMemo(() => getScheduleStatus(local.currentDays, local.benchmarkDays));
  const scheduleConf = createMemo(() => scheduleConfig[schedule()]);

  return (
    <div class={cn('space-y-3', local.class)} {...others}>
      <Show when={local.showLabels}>
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Timer size={14} class="text-nebula-500" />
            <span class="text-xs font-medium text-nebula-400">Time to Value</span>
          </div>
          <div
            class="flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-bold"
            style={{ color: scheduleConf().color, 'background-color': `${scheduleConf().color}15` }}
          >
            {scheduleConf().label}
          </div>
        </div>
      </Show>

      <div class="relative h-6 rounded-full bg-void-700 overflow-hidden">
        <div
          class={cn(
            'absolute inset-y-0 left-0 rounded-full transition-all duration-1000 ease-smooth',
            local.animated && 'animate-[score-fill_1s_ease-out_forwards]'
          )}
          style={{
            width: `${currentPercent()}%`,
            background: `linear-gradient(90deg, ${scheduleConf().color}, ${scheduleConf().color}cc)`,
            'box-shadow': `0 0 15px ${scheduleConf().color}40`,
          }}
        />

        <div
          class="absolute inset-y-1 w-0.5 bg-nebula-400"
          style={{ left: `${benchmarkPercent()}%` }}
        />
        <div
          class="absolute -top-6 text-2xs font-medium text-nebula-400"
          style={{ left: `${benchmarkPercent()}%`, transform: 'translateX(-50%)' }}
        >
          Benchmark
        </div>

        <Show when={targetPercent()}>
          <div
            class="absolute inset-y-1 w-0.5 bg-indigo-400"
            style={{ left: `${targetPercent()}%` }}
          />
        </Show>

        <div class="absolute inset-0 flex items-center justify-center">
          <span class="font-display text-sm font-bold tabular-nums text-white">
            {local.currentDays}d
            <span class="text-nebula-500 font-normal"> / {local.benchmarkDays}d</span>
          </span>
        </div>
      </div>

      <Show when={local.showLabels}>
        <div class="flex items-center justify-between text-2xs">
          <span class="text-nebula-500">0 days</span>
          <div class="flex items-center gap-4">
            <div class="flex items-center gap-1.5">
              <div class="h-2 w-2 rounded-full bg-nebula-400" />
              <span class="text-nebula-400">Benchmark ({local.benchmarkDays}d)</span>
            </div>
            <Show when={local.targetDays}>
              <div class="flex items-center gap-1.5">
                <div class="h-2 w-2 rounded-full bg-indigo-400" />
                <span class="text-nebula-400">Target ({local.targetDays}d)</span>
              </div>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
};

export interface TTVSummaryProps {
  milestones: Milestone[];
  currentDays: number;
  benchmarkDays: number;
  class?: string;
}

export const TTVSummary: Component<TTVSummaryProps> = (props) => {
  const completed = createMemo(() => props.milestones.filter(m => m.status === 'completed').length);
  const blocked = createMemo(() => props.milestones.filter(m => m.status === 'blocked').length);
  const speed = createMemo(() => getSpeed(props.currentDays, props.benchmarkDays));
  const speedConf = createMemo(() => speedConfig[speed()]);
  const SpeedIcon = speedConf().icon;

  const avgDaysPerMilestone = createMemo(() => {
    const completedMilestones = props.milestones.filter(m => m.status === 'completed' && m.actualDays);
    if (completedMilestones.length === 0) return 0;
    return completedMilestones.reduce((sum, m) => sum + (m.actualDays || 0), 0) / completedMilestones.length;
  });

  return (
    <div class={cn('grid grid-cols-4 gap-4', props.class)}>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Progress
        </div>
        <div class="mt-2 font-display text-2xl font-black text-white tabular-nums">
          {completed()}/{props.milestones.length}
        </div>
        <div class="mt-1 text-xs text-nebula-500">
          milestones complete
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Time Elapsed
        </div>
        <div class="mt-2 font-display text-2xl font-black text-indigo-400 tabular-nums">
          {props.currentDays}d
        </div>
        <div class="mt-1 text-xs text-nebula-500">
          of {props.benchmarkDays}d benchmark
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Velocity
        </div>
        <div class="mt-2 flex items-center gap-2">
          <SpeedIcon size={20} style={{ color: speedConf().color }} />
          <span class="font-display text-2xl font-black tabular-nums" style={{ color: speedConf().color }}>
            {speedConf().label}
          </span>
        </div>
        <div class="mt-1 text-xs text-nebula-500">
          {avgDaysPerMilestone().toFixed(1)}d avg per milestone
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Blockers
        </div>
        <div class={cn(
          'mt-2 font-display text-2xl font-black tabular-nums',
          blocked() > 0 ? 'text-flare-400' : 'text-aurora-400'
        )}>
          {blocked()}
        </div>
        <div class="mt-1 text-xs text-nebula-500">
          {blocked() > 0 ? 'needs attention' : 'all clear'}
        </div>
      </div>
    </div>
  );
};

export default TimelineMetric;
