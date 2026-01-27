import { Component, createMemo, Show, For } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import {
  Sparkles,
  Rocket,
  Zap,
  Activity,
  Crown,
  AlertTriangle,
  TrendingDown,
  XCircle,
  RefreshCw,
  LucideIcon,
} from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type LifecycleStage =
  | 'new'
  | 'onboarding'
  | 'activated'
  | 'engaged'
  | 'power_user'
  | 'at_risk'
  | 'churning'
  | 'churned'
  | 'reactivated';

interface LifecycleBadgeProps {
  stage: LifecycleStage;
  size?: 'sm' | 'md' | 'lg';
  showIcon?: boolean;
  showLabel?: boolean;
  variant?: 'badge' | 'pill' | 'dot' | 'tag';
  animated?: boolean;
  class?: string;
}

interface StageConfig {
  icon: LucideIcon;
  label: string;
  color: string;
  bg: string;
  border: string;
  glow: string;
  order: number;
}

const stageConfig: Record<LifecycleStage, StageConfig> = {
  new: {
    icon: Sparkles,
    label: 'New',
    color: 'text-plasma-400',
    bg: 'bg-plasma-500/10',
    border: 'border-plasma-500/25',
    glow: 'shadow-[0_0_12px_rgba(90,154,232,0.3)]',
    order: 1,
  },
  onboarding: {
    icon: Rocket,
    label: 'Onboarding',
    color: 'text-photon-400',
    bg: 'bg-photon-500/10',
    border: 'border-photon-500/25',
    glow: 'shadow-[0_0_12px_rgba(176,109,232,0.3)]',
    order: 2,
  },
  activated: {
    icon: Zap,
    label: 'Activated',
    color: 'text-electric-400',
    bg: 'bg-electric-500/10',
    border: 'border-electric-500/25',
    glow: 'shadow-[0_0_12px_rgba(34,211,211,0.3)]',
    order: 3,
  },
  engaged: {
    icon: Activity,
    label: 'Engaged',
    color: 'text-indigo-400',
    bg: 'bg-indigo-500/10',
    border: 'border-indigo-500/25',
    glow: 'shadow-[0_0_12px_rgba(129,140,248,0.3)]',
    order: 4,
  },
  power_user: {
    icon: Crown,
    label: 'Power User',
    color: 'text-solar-400',
    bg: 'bg-solar-500/10',
    border: 'border-solar-500/25',
    glow: 'shadow-[0_0_12px_rgba(251,191,36,0.3)]',
    order: 5,
  },
  at_risk: {
    icon: AlertTriangle,
    label: 'At Risk',
    color: 'text-flare-400',
    bg: 'bg-flare-500/10',
    border: 'border-flare-500/25',
    glow: 'shadow-[0_0_12px_rgba(248,113,113,0.3)]',
    order: 6,
  },
  churning: {
    icon: TrendingDown,
    label: 'Churning',
    color: 'text-flare-500',
    bg: 'bg-flare-600/10',
    border: 'border-flare-600/25',
    glow: 'shadow-[0_0_12px_rgba(220,38,38,0.3)]',
    order: 7,
  },
  churned: {
    icon: XCircle,
    label: 'Churned',
    color: 'text-nebula-500',
    bg: 'bg-nebula-600/10',
    border: 'border-nebula-600/25',
    glow: '',
    order: 8,
  },
  reactivated: {
    icon: RefreshCw,
    label: 'Reactivated',
    color: 'text-aurora-400',
    bg: 'bg-aurora-500/10',
    border: 'border-aurora-500/25',
    glow: 'shadow-[0_0_12px_rgba(52,211,153,0.3)]',
    order: 9,
  },
};

const sizeClasses = {
  sm: {
    badge: 'px-2 py-0.5 text-2xs gap-1',
    icon: 12,
    dot: 'h-1.5 w-1.5',
  },
  md: {
    badge: 'px-2.5 py-1 text-xs gap-1.5',
    icon: 14,
    dot: 'h-2 w-2',
  },
  lg: {
    badge: 'px-3 py-1.5 text-sm gap-2',
    icon: 16,
    dot: 'h-2.5 w-2.5',
  },
};

export const LifecycleBadge: Component<LifecycleBadgeProps> = (props) => {
  const config = createMemo(() => stageConfig[props.stage]);
  const sizes = createMemo(() => sizeClasses[props.size || 'md']);
  const showIcon = () => props.showIcon !== false;
  const showLabel = () => props.showLabel !== false;
  const variant = () => props.variant || 'badge';
  const IconComponent = createMemo(() => config().icon);

  return (
    <Show
      when={variant() !== 'dot'}
      fallback={
        <div class={cn('flex items-center gap-2', props.class)}>
          <div
            class={cn(
              'rounded-full',
              sizes().dot,
              props.animated && 'animate-pulse'
            )}
            style={{ 'background-color': `var(--stage-${props.stage}, currentColor)` }}
          />
          <Show when={showLabel()}>
            <span class={cn('font-medium', config().color)}>{config().label}</span>
          </Show>
        </div>
      }
    >
      <div
        class={cn(
          'inline-flex items-center rounded-full border font-bold uppercase tracking-wide transition-all',
          config().bg,
          config().border,
          config().color,
          sizes().badge,
          props.animated && config().glow,
          variant() === 'tag' && 'rounded-md',
          props.class
        )}
      >
        <Show when={showIcon()}>
          {(() => {
            const Icon = IconComponent();
            return <Icon size={sizes().icon} class={props.animated ? 'animate-pulse-slow' : ''} />;
          })()}
        </Show>
        <Show when={showLabel()}>
          <span>{config().label}</span>
        </Show>
      </div>
    </Show>
  );
};

interface LifecycleProgressProps {
  currentStage: LifecycleStage;
  showLabels?: boolean;
  size?: 'sm' | 'md' | 'lg';
  class?: string;
}

const progressStages: LifecycleStage[] = [
  'new',
  'onboarding',
  'activated',
  'engaged',
  'power_user',
];

export const LifecycleProgress: Component<LifecycleProgressProps> = (props) => {
  const currentOrder = createMemo(() => stageConfig[props.currentStage]?.order || 0);
  const size = () => props.size || 'md';

  const dotSizes = {
    sm: 'h-3 w-3',
    md: 'h-4 w-4',
    lg: 'h-5 w-5',
  };

  const lineSizes = {
    sm: 'h-0.5',
    md: 'h-1',
    lg: 'h-1.5',
  };

  return (
    <div class={cn('flex items-center gap-1', props.class)}>
      <For each={progressStages}>
        {(stage, index) => {
          const config = stageConfig[stage];
          const isActive = currentOrder() >= config.order;
          const isCurrent = props.currentStage === stage;
          const isLast = index() === progressStages.length - 1;

          return (
            <>
              <div class="flex flex-col items-center gap-1">
                <div
                  class={cn(
                    'rounded-full flex items-center justify-center transition-all',
                    dotSizes[size()],
                    isActive
                      ? cn(config.bg, 'ring-2', config.border)
                      : 'bg-void-700',
                    isCurrent && config.glow
                  )}
                  style={isActive ? { 'border-color': `var(--stage-${stage})` } : undefined}
                >
                  <Show when={isCurrent}>
                    <div class="h-1/2 w-1/2 rounded-full animate-pulse" 
                      style={{ 'background-color': `var(--stage-${stage})` }} 
                    />
                  </Show>
                </div>
                <Show when={props.showLabels}>
                  <span
                    class={cn(
                      'text-2xs font-medium uppercase tracking-wider',
                      isActive ? config.color : 'text-nebula-600'
                    )}
                  >
                    {config.label}
                  </span>
                </Show>
              </div>
              <Show when={!isLast}>
                <div
                  class={cn(
                    'flex-1 min-w-[20px] rounded-full transition-all',
                    lineSizes[size()],
                    isActive && currentOrder() > config.order
                      ? 'bg-gradient-to-r from-indigo-500/50 to-indigo-500/20'
                      : 'bg-void-700'
                  )}
                />
              </Show>
            </>
          );
        }}
      </For>
    </div>
  );
};

interface LifecycleTimelineProps {
  currentStage: LifecycleStage;
  stageHistory?: Array<{ stage: LifecycleStage; date: string }>;
  class?: string;
}

export const LifecycleTimeline: Component<LifecycleTimelineProps> = (props) => {
  return (
    <div class={cn('space-y-3', props.class)}>
      <For each={props.stageHistory || []}>
        {(item, index) => {
          const config = stageConfig[item.stage];
          const IconComponent = config.icon;
          const isCurrent = item.stage === props.currentStage;

          return (
            <div class="flex items-start gap-3">
              <div class="relative flex flex-col items-center">
                <div
                  class={cn(
                    'flex h-8 w-8 items-center justify-center rounded-full border transition-all',
                    config.bg,
                    config.border,
                    isCurrent && config.glow
                  )}
                >
                  <IconComponent size={14} class={config.color} />
                </div>
                <Show when={index() < (props.stageHistory?.length || 0) - 1}>
                  <div class="absolute top-8 h-8 w-px bg-void-600" />
                </Show>
              </div>
              <div class="flex-1 pt-1">
                <p class={cn('text-sm font-bold', config.color)}>{config.label}</p>
                <p class="text-xs text-nebula-500">{item.date}</p>
              </div>
            </div>
          );
        }}
      </For>
    </div>
  );
};

export default LifecycleBadge;
