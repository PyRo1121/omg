import { Component, For, Show, createMemo, splitProps, JSX } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import {
  TrendingUp,
  Package,
  Users,
  RefreshCw,
  DollarSign,
  Zap,
  Target,
  ArrowRight,
  Clock,
  Building,
  Star,
} from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

type OpportunityType = 'upsell' | 'cross-sell' | 'expansion' | 'renewal';
type ConfidenceLevel = 'low' | 'medium' | 'high' | 'very-high';
type RevenueImpact = 'low' | 'medium' | 'high' | 'critical';
type OpportunitySize = 'sm' | 'md' | 'lg';

interface OpportunityData {
  id: string;
  type: OpportunityType;
  confidence: ConfidenceLevel;
  revenueImpact: RevenueImpact;
  estimatedValue: number;
  customer: {
    name: string;
    currentTier?: string;
    currentMrr?: number;
  };
  title: string;
  description?: string;
  signals?: string[];
  daysToClose?: number;
  probability?: number;
}

interface OpportunityCardProps {
  opportunity: OpportunityData;
  size?: OpportunitySize;
  showSignals?: boolean;
  showActions?: boolean;
  compact?: boolean;
  animated?: boolean;
  onClick?: (opportunity: OpportunityData) => void;
  onAction?: (opportunity: OpportunityData, action: 'view' | 'dismiss' | 'snooze') => void;
  class?: string;
}

const typeConfig: Record<OpportunityType, {
  icon: typeof TrendingUp;
  label: string;
  color: string;
  bgClass: string;
  borderClass: string;
}> = {
  upsell: {
    icon: TrendingUp,
    label: 'Upsell',
    color: 'var(--opportunity-upsell, #34d399)',
    bgClass: 'bg-aurora-500/10',
    borderClass: 'border-aurora-500/30',
  },
  'cross-sell': {
    icon: Package,
    label: 'Cross-sell',
    color: 'var(--opportunity-cross-sell, #2ee8e8)',
    bgClass: 'bg-electric-500/10',
    borderClass: 'border-electric-500/30',
  },
  expansion: {
    icon: Users,
    label: 'Expansion',
    color: 'var(--opportunity-expansion, #b06de8)',
    bgClass: 'bg-photon-500/10',
    borderClass: 'border-photon-500/30',
  },
  renewal: {
    icon: RefreshCw,
    label: 'Renewal',
    color: 'var(--opportunity-renewal, #5a9ae8)',
    bgClass: 'bg-plasma-500/10',
    borderClass: 'border-plasma-500/30',
  },
};

const confidenceConfig: Record<ConfidenceLevel, {
  label: string;
  color: string;
  bgClass: string;
  borderClass: string;
  glowClass: string;
}> = {
  low: {
    label: 'Low',
    color: 'var(--opportunity-low, #71717a)',
    bgClass: 'bg-nebula-500/10',
    borderClass: 'border-nebula-500/20',
    glowClass: '',
  },
  medium: {
    label: 'Medium',
    color: 'var(--opportunity-medium, #3b7dd1)',
    bgClass: 'bg-plasma-500/10',
    borderClass: 'border-plasma-500/25',
    glowClass: 'shadow-[0_0_15px_rgba(59,125,209,0.2)]',
  },
  high: {
    label: 'High',
    color: 'var(--opportunity-high, #22d3d3)',
    bgClass: 'bg-electric-500/10',
    borderClass: 'border-electric-500/30',
    glowClass: 'shadow-[0_0_20px_rgba(34,211,211,0.25)]',
  },
  'very-high': {
    label: 'Very High',
    color: 'var(--opportunity-very-high, #10b981)',
    bgClass: 'bg-aurora-500/15',
    borderClass: 'border-aurora-500/35',
    glowClass: 'shadow-[0_0_25px_rgba(16,185,129,0.3)]',
  },
};

const revenueConfig: Record<RevenueImpact, {
  label: string;
  color: string;
}> = {
  low: { label: 'Low', color: 'var(--opportunity-revenue-low, #a1a1aa)' },
  medium: { label: 'Medium', color: 'var(--opportunity-revenue-medium, #2ee8e8)' },
  high: { label: 'High', color: 'var(--opportunity-revenue-high, #34d399)' },
  critical: { label: 'Critical', color: 'var(--opportunity-revenue-critical, #fbbf24)' },
};

const sizeConfig: Record<OpportunitySize, {
  padding: string;
  titleSize: string;
  valueSize: string;
  iconSize: number;
  iconPadding: string;
}> = {
  sm: { padding: 'p-4', titleSize: 'text-sm', valueSize: 'text-xl', iconSize: 16, iconPadding: 'p-2' },
  md: { padding: 'p-5', titleSize: 'text-base', valueSize: 'text-2xl', iconSize: 20, iconPadding: 'p-2.5' },
  lg: { padding: 'p-6', titleSize: 'text-lg', valueSize: 'text-3xl', iconSize: 24, iconPadding: 'p-3' },
};

const formatCurrency = (value: number): string => {
  if (value >= 1000000) return `$${(value / 1000000).toFixed(1)}M`;
  if (value >= 1000) return `$${(value / 1000).toFixed(0)}k`;
  return `$${value.toLocaleString()}`;
};

export const OpportunityCard: Component<OpportunityCardProps> = (props) => {
  const [local, others] = splitProps(props, [
    'opportunity', 'size', 'showSignals', 'showActions', 'compact',
    'animated', 'onClick', 'onAction', 'class'
  ]);

  const opp = () => local.opportunity;
  const type = createMemo(() => typeConfig[opp().type]);
  const confidence = createMemo(() => confidenceConfig[opp().confidence]);
  const revenue = createMemo(() => revenueConfig[opp().revenueImpact]);
  const size = () => sizeConfig[local.size || 'md'];

  const TypeIcon = type().icon;

  return (
    <div
      class={cn(
        'group relative overflow-hidden rounded-2xl border transition-all duration-300',
        'bg-void-850 hover:bg-void-800',
        'border-white/5 hover:border-white/10',
        'hover:translate-y-[-2px]',
        confidence().glowClass && `hover:${confidence().glowClass}`,
        size().padding,
        local.onClick && 'cursor-pointer',
        local.animated && 'animate-fade-up',
        local.class
      )}
      onClick={() => local.onClick?.(opp())}
      role={local.onClick ? 'button' : undefined}
      tabIndex={local.onClick ? 0 : undefined}
      {...others}
    >
      <div
        class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-100"
        style={{ 'background-color': type().color }}
      />

      <div class="relative flex items-start justify-between">
        <div class="flex items-start gap-3">
          <div class={cn(type().bgClass, size().iconPadding, 'rounded-xl transition-transform duration-500 group-hover:scale-110')}>
            <TypeIcon size={size().iconSize} style={{ color: type().color }} />
          </div>

          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2 flex-wrap">
              <span
                class={cn('rounded-full px-2 py-0.5 text-2xs font-bold uppercase tracking-wider')}
                style={{ color: type().color, 'background-color': `${type().color}20` }}
              >
                {type().label}
              </span>
              <span
                class={cn('rounded-full px-2 py-0.5 text-2xs font-bold uppercase tracking-wider')}
                style={{ color: confidence().color, 'background-color': `${confidence().color}20` }}
              >
                {confidence().label} Confidence
              </span>
            </div>

            <h4 class={cn('mt-2 font-bold text-white', size().titleSize)}>
              {opp().title}
            </h4>

            <Show when={!local.compact && opp().description}>
              <p class="mt-1 text-sm text-nebula-400 line-clamp-2">
                {opp().description}
              </p>
            </Show>

            <div class="mt-3 flex items-center gap-4 text-sm">
              <div class="flex items-center gap-1.5">
                <Building size={14} class="text-nebula-500" />
                <span class="font-medium text-nebula-300">{opp().customer.name}</span>
                <Show when={opp().customer.currentTier}>
                  <span class="rounded bg-void-700 px-1.5 py-0.5 text-2xs font-bold uppercase text-nebula-500">
                    {opp().customer.currentTier}
                  </span>
                </Show>
              </div>
              <Show when={opp().daysToClose}>
                <div class="flex items-center gap-1 text-nebula-500">
                  <Clock size={14} />
                  <span class="font-medium">{opp().daysToClose}d</span>
                </div>
              </Show>
            </div>
          </div>
        </div>

        <div class="flex flex-col items-end gap-2">
          <div class="text-right">
            <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              Est. Value
            </div>
            <div class={cn('font-display font-black tabular-nums', size().valueSize)} style={{ color: revenue().color }}>
              {formatCurrency(opp().estimatedValue)}
            </div>
          </div>
          <Show when={opp().probability !== undefined}>
            <div class="flex items-center gap-1.5 rounded-full bg-void-700 px-2 py-1">
              <Target size={12} class="text-nebula-500" />
              <span class="text-xs font-bold tabular-nums text-nebula-300">
                {opp().probability}%
              </span>
            </div>
          </Show>
        </div>
      </div>

      <Show when={local.showSignals && opp().signals && (opp().signals?.length ?? 0) > 0}>
        <div class="mt-4 border-t border-white/5 pt-4">
          <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500 mb-2">
            Signals
          </div>
          <div class="flex flex-wrap gap-2">
            <For each={opp().signals}>
              {(signal) => (
                <div class="flex items-center gap-1.5 rounded-lg bg-void-700 px-2.5 py-1 text-xs text-nebula-300">
                  <Zap size={12} class="text-solar-400" />
                  {signal}
                </div>
              )}
            </For>
          </div>
        </div>
      </Show>

      <Show when={local.showActions}>
        <div class="mt-4 flex items-center justify-end gap-2 border-t border-white/5 pt-4">
          <button
            class="rounded-lg px-3 py-1.5 text-xs font-bold text-nebula-400 hover:bg-white/5 hover:text-nebula-300 transition-colors"
            onClick={(e) => { e.stopPropagation(); local.onAction?.(opp(), 'snooze'); }}
          >
            Snooze
          </button>
          <button
            class="rounded-lg px-3 py-1.5 text-xs font-bold text-nebula-400 hover:bg-white/5 hover:text-nebula-300 transition-colors"
            onClick={(e) => { e.stopPropagation(); local.onAction?.(opp(), 'dismiss'); }}
          >
            Dismiss
          </button>
          <button
            class="flex items-center gap-1.5 rounded-lg bg-indigo-500/10 px-3 py-1.5 text-xs font-bold text-indigo-400 hover:bg-indigo-500/20 transition-colors"
            onClick={(e) => { e.stopPropagation(); local.onAction?.(opp(), 'view'); }}
          >
            View Details
            <ArrowRight size={12} />
          </button>
        </div>
      </Show>
    </div>
  );
};

export interface OpportunityListProps {
  opportunities: OpportunityData[];
  sortBy?: 'value' | 'confidence' | 'daysToClose';
  filterType?: OpportunityType;
  showSignals?: boolean;
  showActions?: boolean;
  emptyMessage?: string;
  onItemClick?: (opportunity: OpportunityData) => void;
  onAction?: (opportunity: OpportunityData, action: 'view' | 'dismiss' | 'snooze') => void;
  class?: string;
}

export const OpportunityList: Component<OpportunityListProps> = (props) => {
  const sortedOpportunities = createMemo(() => {
    let items = [...props.opportunities];
    
    if (props.filterType) {
      items = items.filter(o => o.type === props.filterType);
    }

    const sortBy = props.sortBy ?? 'value';
    items.sort((a, b) => {
      if (sortBy === 'value') return b.estimatedValue - a.estimatedValue;
      if (sortBy === 'confidence') {
        const order: Record<ConfidenceLevel, number> = { 'very-high': 4, high: 3, medium: 2, low: 1 };
        return order[b.confidence] - order[a.confidence];
      }
      if (sortBy === 'daysToClose') {
        return (a.daysToClose ?? 999) - (b.daysToClose ?? 999);
      }
      return 0;
    });

    return items;
  });

  return (
    <div class={cn('space-y-4', props.class)}>
      <Show when={sortedOpportunities().length === 0}>
        <div class="flex flex-col items-center justify-center rounded-2xl border border-white/5 bg-void-850 py-12">
          <Star size={32} class="text-nebula-600 mb-3" />
          <p class="text-nebula-500 font-medium">
            {props.emptyMessage || 'No opportunities found'}
          </p>
        </div>
      </Show>
      <For each={sortedOpportunities()}>
        {(opportunity, index) => (
          <OpportunityCard
            opportunity={opportunity}
            showSignals={props.showSignals}
            showActions={props.showActions}
            onClick={props.onItemClick}
            onAction={props.onAction}
            animated
          />
        )}
      </For>
    </div>
  );
};

export interface OpportunitySummaryProps {
  opportunities: OpportunityData[];
  class?: string;
}

export const OpportunitySummary: Component<OpportunitySummaryProps> = (props) => {
  const stats = createMemo(() => {
    const total = props.opportunities.length;
    const totalValue = props.opportunities.reduce((sum, o) => sum + o.estimatedValue, 0);
    const highConfidence = props.opportunities.filter(o => o.confidence === 'high' || o.confidence === 'very-high').length;
    const avgProbability = props.opportunities.reduce((sum, o) => sum + (o.probability ?? 0), 0) / (total || 1);

    const byType = props.opportunities.reduce((acc, o) => {
      acc[o.type] = (acc[o.type] || 0) + 1;
      return acc;
    }, {} as Record<OpportunityType, number>);

    return { total, totalValue, highConfidence, avgProbability, byType };
  });

  return (
    <div class={cn('grid grid-cols-4 gap-4', props.class)}>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Total Pipeline
        </div>
        <div class="mt-2 font-display text-2xl font-black text-aurora-400 tabular-nums">
          {formatCurrency(stats().totalValue)}
        </div>
        <div class="mt-1 text-xs text-nebula-500">
          {stats().total} opportunities
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          High Confidence
        </div>
        <div class="mt-2 font-display text-2xl font-black text-electric-400 tabular-nums">
          {stats().highConfidence}
        </div>
        <div class="mt-1 text-xs text-nebula-500">
          {stats().total > 0 ? ((stats().highConfidence / stats().total) * 100).toFixed(0) : 0}% of total
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Avg Probability
        </div>
        <div class="mt-2 font-display text-2xl font-black text-plasma-400 tabular-nums">
          {stats().avgProbability.toFixed(0)}%
        </div>
        <div class="mt-1 text-xs text-nebula-500">
          Weighted close rate
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          By Type
        </div>
        <div class="mt-2 flex gap-2">
          <For each={Object.entries(stats().byType)}>
            {([type, count]) => (
                <div
                  class="rounded-lg px-2 py-1 text-xs font-bold tabular-nums"
                  style={{
                    color: typeConfig[type as OpportunityType].color,
                    'background-color': `${typeConfig[type as OpportunityType].color}20`,
                  }}
                >
                  {count}
                </div>
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

export default OpportunityCard;
