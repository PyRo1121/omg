import { Component, Show, createMemo } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { Crown, Sparkles, Users, Building2 } from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type Tier = 'free' | 'pro' | 'team' | 'enterprise';

interface TierBadgeProps {
  tier: Tier;
  size?: 'sm' | 'md' | 'lg';
  variant?: 'badge' | 'pill' | 'card' | 'chip';
  showIcon?: boolean;
  showLabel?: boolean;
  glowing?: boolean;
  class?: string;
}

const tierConfig: Record<Tier, {
  label: string;
  icon: typeof Crown;
  color: string;
  bg: string;
  border: string;
  gradient: string;
  glow: string;
  ringColor: string;
}> = {
  free: {
    label: 'Free',
    icon: Sparkles,
    color: 'text-nebula-400',
    bg: 'bg-nebula-600/10',
    border: 'border-nebula-600/20',
    gradient: 'from-nebula-600/20 to-nebula-700/10',
    glow: '',
    ringColor: 'ring-nebula-600/30',
  },
  pro: {
    label: 'Pro',
    icon: Crown,
    color: 'text-indigo-400',
    bg: 'bg-indigo-500/10',
    border: 'border-indigo-500/25',
    gradient: 'from-indigo-500/20 via-indigo-600/15 to-purple-600/10',
    glow: 'shadow-[0_0_20px_rgba(99,102,241,0.25)]',
    ringColor: 'ring-indigo-500/40',
  },
  team: {
    label: 'Team',
    icon: Users,
    color: 'text-electric-400',
    bg: 'bg-electric-500/10',
    border: 'border-electric-500/25',
    gradient: 'from-electric-500/20 via-electric-600/15 to-plasma-600/10',
    glow: 'shadow-[0_0_20px_rgba(34,211,211,0.25)]',
    ringColor: 'ring-electric-500/40',
  },
  enterprise: {
    label: 'Enterprise',
    icon: Building2,
    color: 'text-solar-400',
    bg: 'bg-solar-500/10',
    border: 'border-solar-500/25',
    gradient: 'from-solar-400/20 via-solar-500/15 to-orange-600/10',
    glow: 'shadow-[0_0_25px_rgba(245,158,11,0.3)]',
    ringColor: 'ring-solar-500/40',
  },
};

const sizeConfig = {
  sm: {
    badge: 'px-2 py-0.5 text-2xs gap-1',
    icon: 10,
    card: 'p-3 gap-2',
  },
  md: {
    badge: 'px-2.5 py-1 text-xs gap-1.5',
    icon: 12,
    card: 'p-4 gap-3',
  },
  lg: {
    badge: 'px-3 py-1.5 text-sm gap-2',
    icon: 14,
    card: 'p-5 gap-4',
  },
};

export const TierBadge: Component<TierBadgeProps> = (props) => {
  const config = createMemo(() => tierConfig[props.tier]);
  const size = createMemo(() => sizeConfig[props.size || 'md']);
  const showIcon = () => props.showIcon !== false;
  const showLabel = () => props.showLabel !== false;
  const variant = () => props.variant || 'badge';

  const IconComponent = config().icon;

  if (variant() === 'card') {
    return (
      <div
        class={cn(
          'rounded-2xl border bg-gradient-to-br transition-all',
          config().gradient,
          config().border,
          props.glowing && config().glow,
          size().card,
          props.class
        )}
      >
        <div class="flex items-center gap-3">
          <div class={cn('p-2 rounded-xl', config().bg)}>
            <IconComponent size={size().icon + 4} class={config().color} />
          </div>
          <div>
            <p class={cn('font-black uppercase tracking-wider', config().color)}>
              {config().label}
            </p>
            <p class="text-xs text-nebula-500 font-medium">Subscription Tier</p>
          </div>
        </div>
      </div>
    );
  }

  if (variant() === 'chip') {
    return (
      <div
        class={cn(
          'inline-flex items-center rounded-lg border font-bold uppercase tracking-wider transition-all',
          config().bg,
          config().border,
          config().color,
          props.glowing && 'ring-2',
          props.glowing && config().ringColor,
          size().badge,
          props.class
        )}
      >
        <Show when={showIcon()}>
          <IconComponent size={size().icon} />
        </Show>
        <Show when={showLabel()}>
          <span>{config().label}</span>
        </Show>
      </div>
    );
  }

  return (
    <div
      class={cn(
        'inline-flex items-center rounded-full border font-black uppercase tracking-wider transition-all',
        config().bg,
        config().border,
        config().color,
        props.glowing && config().glow,
        variant() === 'pill' && 'rounded-md',
        size().badge,
        props.class
      )}
    >
      <Show when={showIcon()}>
        <IconComponent size={size().icon} />
      </Show>
      <Show when={showLabel()}>
        <span>{config().label}</span>
      </Show>
    </div>
  );
};

interface TierComparisonProps {
  currentTier: Tier;
  recommendedTier?: Tier;
  class?: string;
}

const tierOrder: Tier[] = ['free', 'pro', 'team', 'enterprise'];

export const TierComparison: Component<TierComparisonProps> = (props) => {
  const currentIndex = createMemo(() => tierOrder.indexOf(props.currentTier));
  const recommendedIndex = createMemo(() => 
    props.recommendedTier ? tierOrder.indexOf(props.recommendedTier) : -1
  );

  return (
    <div class={cn('flex items-center gap-2', props.class)}>
      {tierOrder.map((tier, index) => {
        const config = tierConfig[tier];
        const isActive = index <= currentIndex();
        const isRecommended = tier === props.recommendedTier;
        const isCurrent = tier === props.currentTier;

        return (
          <div class="flex items-center gap-2">
            <div
              class={cn(
                'flex h-8 w-8 items-center justify-center rounded-lg border transition-all',
                isActive
                  ? cn(config.bg, config.border, config.color)
                  : 'bg-void-800 border-void-600 text-nebula-600',
                isCurrent && 'ring-2 ring-white/20',
                isRecommended && 'animate-pulse ring-2',
                isRecommended && config.ringColor
              )}
            >
              <config.icon size={14} />
            </div>
            <Show when={index < tierOrder.length - 1}>
              <div
                class={cn(
                  'h-0.5 w-4 rounded-full',
                  index < currentIndex() ? 'bg-indigo-500/50' : 'bg-void-700'
                )}
              />
            </Show>
          </div>
        );
      })}
    </div>
  );
};

interface TierSelectorProps {
  value: Tier;
  onChange: (tier: Tier) => void;
  disabledTiers?: Tier[];
  class?: string;
}

export const TierSelector: Component<TierSelectorProps> = (props) => {
  return (
    <div class={cn('grid grid-cols-4 gap-3', props.class)}>
      {tierOrder.map((tier) => {
        const config = tierConfig[tier];
        const isSelected = tier === props.value;
        const isDisabled = props.disabledTiers?.includes(tier);
        const IconComponent = config.icon;

        return (
          <button
            onClick={() => !isDisabled && props.onChange(tier)}
            disabled={isDisabled}
            class={cn(
              'rounded-2xl border p-4 transition-all text-left',
              isSelected
                ? cn('bg-gradient-to-br', config.gradient, config.border, config.glow)
                : 'bg-void-850 border-void-700 hover:border-void-600',
              isDisabled && 'opacity-50 cursor-not-allowed'
            )}
          >
            <div class={cn('p-2 rounded-xl w-fit mb-3', config.bg)}>
              <IconComponent size={18} class={isSelected ? config.color : 'text-nebula-500'} />
            </div>
            <p class={cn(
              'font-bold uppercase tracking-wider',
              isSelected ? config.color : 'text-nebula-400'
            )}>
              {config.label}
            </p>
          </button>
        );
      })}
    </div>
  );
};

export default TierBadge;
