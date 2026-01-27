import { Component, For, Show, createSignal, createMemo } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import type { CRMCustomer, CustomerTag } from './types';
import {
  ChevronRight,
  Mail,
  Phone,
  MessageSquare,
  MoreHorizontal,
  Eye,
  Edit,
  TrendingUp,
  TrendingDown,
  AlertTriangle,
  Star,
} from 'lucide-solid';
import { HealthScore, TierBadge } from '../../../design-system';
import type { Tier } from '../../../design-system/components/TierBadge';
import type { LifecycleStage } from '../../../design-system/components/LifecycleBadge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface CRMProfileCardProps {
  customer: CRMCustomer;
  onViewDetail?: (id: string) => void;
  onQuickAction?: (action: string, customerId: string) => void;
  compact?: boolean;
}

const STATUS_CONFIG: Record<string, { color: string; bg: string; label: string }> = {
  active: { color: 'text-aurora-400', bg: 'bg-aurora-500/10', label: 'Active' },
  suspended: { color: 'text-solar-400', bg: 'bg-solar-500/10', label: 'Suspended' },
  cancelled: { color: 'text-flare-400', bg: 'bg-flare-500/10', label: 'Cancelled' },
};

const formatRelativeTime = (date: string): string => {
  const now = new Date();
  const then = new Date(date);
  const diffMs = now.getTime() - then.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return 'just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)}w ago`;
  return `${Math.floor(diffDays / 30)}mo ago`;
};

const TagBadge: Component<{ tag: CustomerTag }> = (props) => (
  <span
    class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-2xs font-bold"
    style={{
      'background-color': `${props.tag.color}15`,
      color: props.tag.color,
      border: `1px solid ${props.tag.color}30`,
    }}
  >
    <span
      class="h-1.5 w-1.5 rounded-full"
      style={{ 'background-color': props.tag.color }}
    />
    {props.tag.name}
  </span>
);

interface QuickActionButtonProps {
  icon: typeof Mail;
  label: string;
  onClick: () => void;
  variant?: 'default' | 'primary' | 'danger';
}

const QuickActionButton: Component<QuickActionButtonProps> = (props) => {
  const variants = {
    default: 'text-nebula-400 hover:text-white hover:bg-white/10',
    primary: 'text-indigo-400 hover:text-indigo-300 hover:bg-indigo-500/10',
    danger: 'text-flare-400 hover:text-flare-300 hover:bg-flare-500/10',
  };

  const IconComponent = props.icon;

  return (
    <button
      onClick={props.onClick}
      class={cn(
        'flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-bold transition-all duration-200',
        variants[props.variant || 'default']
      )}
      title={props.label}
    >
      <IconComponent size={14} />
      <span class="hidden sm:inline">{props.label}</span>
    </button>
  );
};

const handleEmail = (email: string) => {
  window.location.href = `mailto:${email}`;
};

const handleCall = (customerId: string, onQuickAction?: (action: string, id: string) => void) => {
  onQuickAction?.('call', customerId);
};

const handleNote = (customerId: string, onQuickAction?: (action: string, id: string) => void) => {
  onQuickAction?.('note', customerId);
};

export const CRMProfileCard: Component<CRMProfileCardProps> = (props) => {
  const [isHovered, setIsHovered] = createSignal(false);
  const [showActions, setShowActions] = createSignal(false);

  const statusConfig = createMemo(() => STATUS_CONFIG[props.customer.status] || STATUS_CONFIG.active);

  const tierValue = createMemo(() => {
    const tier = props.customer.tier.toLowerCase();
    if (['free', 'pro', 'team', 'enterprise'].includes(tier)) {
      return tier as Tier;
    }
    return 'free' as Tier;
  });

  const _lifecycleStage = createMemo(() => props.customer.health.lifecycle_stage as LifecycleStage);

  const isAtRisk = createMemo(() =>
    ['at_risk', 'churning'].includes(props.customer.health.lifecycle_stage)
  );

  const isPowerUser = createMemo(() => props.customer.health.lifecycle_stage === 'power_user');

  if (props.compact) {
    return (
      <div
        class="group relative flex items-center gap-4 rounded-xl border border-white/5 bg-white/[0.02] p-4 transition-all duration-300 hover:border-white/10 hover:bg-white/[0.04]"
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500/20 to-photon-500/20 text-sm font-black text-white">
          {props.customer.email.charAt(0).toUpperCase()}
        </div>

        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <span class="truncate text-sm font-bold text-white">{props.customer.email}</span>
            <TierBadge tier={tierValue()} size="sm" showIcon={false} />
          </div>
          <div class="mt-0.5 flex items-center gap-2 text-2xs text-nebula-500">
            <span class="font-mono tabular-nums">{props.customer.total_commands.toLocaleString()} cmds</span>
            <span>•</span>
            <span>{formatRelativeTime(props.customer.last_activity_at)}</span>
          </div>
        </div>

        <HealthScore score={props.customer.health.overall_score} size="sm" variant="ring" />

        <Show when={isHovered()}>
          <button
            onClick={() => props.onViewDetail?.(props.customer.id)}
            class="rounded-lg bg-white/5 p-2 text-nebula-400 transition-all hover:bg-white/10 hover:text-white"
          >
            <Eye size={16} />
          </button>
        </Show>
      </div>
    );
  }

  return (
    <div
      class="group relative overflow-hidden rounded-3xl border border-white/5 bg-void-850 shadow-card transition-all duration-300 hover:border-white/10 hover:shadow-card-hover"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => {
        setIsHovered(false);
        setShowActions(false);
      }}
    >
      <div class="pointer-events-none absolute -right-16 -top-16 h-32 w-32 rounded-full bg-indigo-500/5 opacity-0 blur-[60px] transition-opacity duration-500 group-hover:opacity-100" />

      <Show when={isAtRisk()}>
        <div class="absolute left-0 top-0 h-1 w-full bg-gradient-to-r from-flare-500 to-flare-400" />
      </Show>

      <Show when={isPowerUser()}>
        <div class="absolute left-0 top-0 h-1 w-full bg-gradient-to-r from-solar-500 to-solar-400" />
      </Show>

      <div class="relative p-6">
        <div class="flex items-start gap-4">
          <div class="relative">
            <div
              class={cn(
                'flex h-14 w-14 items-center justify-center rounded-2xl bg-gradient-to-br text-lg font-black text-white',
                isAtRisk()
                  ? 'from-flare-500/30 to-flare-600/30'
                  : isPowerUser()
                    ? 'from-solar-500/30 to-solar-600/30'
                    : 'from-indigo-500/30 to-photon-500/30'
              )}
            >
              {props.customer.email.charAt(0).toUpperCase()}
            </div>

            <Show when={isPowerUser()}>
              <div class="absolute -right-1 -top-1 flex h-5 w-5 items-center justify-center rounded-full bg-solar-500 text-2xs">
                <Star size={12} class="text-white" />
              </div>
            </Show>

            <Show when={isAtRisk()}>
              <div class="absolute -right-1 -top-1 flex h-5 w-5 items-center justify-center rounded-full bg-flare-500">
                <AlertTriangle size={10} class="text-white" />
              </div>
            </Show>
          </div>

          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <h3 class="truncate text-base font-black text-white">{props.customer.email}</h3>
            </div>

            <Show when={props.customer.company}>
              <p class="mt-0.5 truncate text-xs text-nebula-400">{props.customer.company}</p>
            </Show>

            <div class="mt-2 flex flex-wrap items-center gap-2">
              <TierBadge tier={tierValue()} size="sm" />

              <span
                class={cn(
                  'rounded-full px-2 py-0.5 text-2xs font-black uppercase',
                  statusConfig().bg,
                  statusConfig().color
                )}
              >
                {statusConfig().label}
              </span>

              <Show when={props.customer.mrr > 0}>
                <span class="rounded-full bg-aurora-500/10 px-2 py-0.5 font-mono text-2xs font-black text-aurora-400">
                  ${props.customer.mrr}/mo
                </span>
              </Show>
            </div>
          </div>

          <div class="flex flex-col items-end gap-2">
            <HealthScore 
              score={props.customer.health.overall_score} 
              size="sm" 
              variant="ring" 
              showLabel 
            />

            <button
              onClick={() => setShowActions(!showActions())}
              class="rounded-lg p-1.5 text-nebula-500 transition-all hover:bg-white/5 hover:text-white"
            >
              <MoreHorizontal size={16} />
            </button>
          </div>
        </div>

        <Show when={props.customer.tags.length > 0}>
          <div class="mt-4 flex flex-wrap gap-1.5">
            <For each={props.customer.tags.slice(0, 4)}>
              {(tag) => <TagBadge tag={tag} />}
            </For>
            <Show when={props.customer.tags.length > 4}>
              <span class="rounded-full bg-void-700 px-2 py-0.5 text-2xs font-bold text-nebula-500">
                +{props.customer.tags.length - 4}
              </span>
            </Show>
          </div>
        </Show>

        <div class="mt-4 grid grid-cols-4 gap-3">
          <div class="rounded-xl bg-white/[0.03] p-3 text-center">
            <p class="font-display text-lg font-black tabular-nums text-white">
              {props.customer.total_commands >= 1000
                ? `${(props.customer.total_commands / 1000).toFixed(1)}k`
                : props.customer.total_commands}
            </p>
            <p class="text-2xs font-bold uppercase tracking-wider text-nebula-500">Commands</p>
          </div>

          <div class="rounded-xl bg-white/[0.03] p-3 text-center">
            <p class="font-display text-lg font-black tabular-nums text-white">{props.customer.machine_count}</p>
            <p class="text-2xs font-bold uppercase tracking-wider text-nebula-500">Machines</p>
          </div>

          <div class="rounded-xl bg-white/[0.03] p-3 text-center">
            <HealthScore score={props.customer.health.overall_score} size="sm" variant="compact" />
            <p class="mt-1 text-2xs font-bold uppercase tracking-wider text-nebula-500">Health</p>
          </div>

          <div class="rounded-xl bg-white/[0.03] p-3 text-center">
            <div class="flex items-center justify-center gap-1">
              {props.customer.health.command_velocity_trend === 'growing' ? (
                <TrendingUp size={14} class="text-aurora-400" />
              ) : props.customer.health.command_velocity_trend === 'declining' ? (
                <TrendingDown size={14} class="text-flare-400" />
              ) : (
                <span class="font-display text-lg font-black text-electric-400">—</span>
              )}
            </div>
            <p class="text-2xs font-bold uppercase tracking-wider text-nebula-500">Trend</p>
          </div>
        </div>

        <div class="mt-4 flex items-center justify-between border-t border-white/5 pt-4">
          <div class="text-2xs text-nebula-500">
            <span>Last active </span>
            <span class="font-bold text-nebula-400">
              {formatRelativeTime(props.customer.last_activity_at)}
            </span>
          </div>

          <div class="flex items-center gap-1">
            <Show when={isHovered() || showActions()}>
              <QuickActionButton
                icon={Mail}
                label="Email"
                onClick={() => handleEmail(props.customer.email)}
              />
              <QuickActionButton
                icon={Phone}
                label="Call"
                onClick={() => handleCall(props.customer.id, props.onQuickAction)}
              />
              <QuickActionButton
                icon={MessageSquare}
                label="Note"
                onClick={() => handleNote(props.customer.id, props.onQuickAction)}
              />
            </Show>

            <button
              onClick={() => props.onViewDetail?.(props.customer.id)}
              class="flex items-center gap-1.5 rounded-lg bg-indigo-500/10 px-3 py-2 text-xs font-bold text-indigo-400 transition-all hover:bg-indigo-500/20"
            >
              <Eye size={14} />
              <span>View</span>
              <ChevronRight size={12} />
            </button>
          </div>
        </div>
      </div>

      <Show when={showActions()}>
        <div class="border-t border-white/5 bg-white/[0.02] p-4">
          <div class="grid grid-cols-3 gap-2">
            <button
              onClick={() => props.onQuickAction?.('edit', props.customer.id)}
              class="flex items-center justify-center gap-2 rounded-xl bg-white/5 py-2 text-xs font-bold text-nebula-400 transition-all hover:bg-white/10 hover:text-white"
            >
              <Edit size={14} />
              Edit
            </button>
            <button
              onClick={() => props.onQuickAction?.('upgrade', props.customer.id)}
              class="flex items-center justify-center gap-2 rounded-xl bg-indigo-500/10 py-2 text-xs font-bold text-indigo-400 transition-all hover:bg-indigo-500/20"
            >
              <TrendingUp size={14} />
              Upgrade
            </button>
            <button
              onClick={() => props.onQuickAction?.('suspend', props.customer.id)}
              class="flex items-center justify-center gap-2 rounded-xl bg-flare-500/10 py-2 text-xs font-bold text-flare-400 transition-all hover:bg-flare-500/20"
            >
              <AlertTriangle size={14} />
              Suspend
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
};

interface CRMProfileCardTableRowProps {
  customer: CRMCustomer;
  onViewDetail?: (id: string) => void;
  onQuickAction?: (action: string, customerId: string) => void;
}

export const CRMProfileCardTableRow: Component<CRMProfileCardTableRowProps> = (props) => {
  const [_isHovered, setIsHovered] = createSignal(false);
  
  const tierValue = createMemo(() => {
    const tier = props.customer.tier.toLowerCase();
    if (['free', 'pro', 'team', 'enterprise'].includes(tier)) {
      return tier as Tier;
    }
    return 'free' as Tier;
  });

  const statusConfig = createMemo(() => STATUS_CONFIG[props.customer.status] || STATUS_CONFIG.active);

  const isAtRisk = createMemo(() =>
    ['at_risk', 'churning'].includes(props.customer.health.lifecycle_stage)
  );

  return (
    <tr
      class="group transition-colors hover:bg-white/[0.02]"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <td class="px-4 py-3">
        <div class="flex items-center gap-3">
          <div
            class={cn(
              'flex h-9 w-9 items-center justify-center rounded-xl text-sm font-black text-white',
              isAtRisk() ? 'bg-flare-500/20' : 'bg-indigo-500/20'
            )}
          >
            {props.customer.email.charAt(0).toUpperCase()}
          </div>
          <div>
            <p class="text-sm font-bold text-white">{props.customer.email}</p>
            <p class="text-2xs text-nebula-500">{props.customer.company || props.customer.id.slice(0, 8)}</p>
          </div>
        </div>
      </td>

      <td class="px-4 py-3">
        <TierBadge tier={tierValue()} size="sm" showIcon={false} />
      </td>

      <td class="px-4 py-3">
        <span class={cn('rounded-full px-2 py-0.5 text-2xs font-black uppercase', statusConfig().bg, statusConfig().color)}>
          {statusConfig().label}
        </span>
      </td>

      <td class="px-4 py-3">
        <HealthScore score={props.customer.health.overall_score} size="sm" variant="badge" showLabel />
      </td>

      <td class="px-4 py-3 font-mono text-sm tabular-nums text-nebula-300">
        {props.customer.machine_count}
      </td>

      <td class="px-4 py-3 font-mono text-sm tabular-nums text-nebula-300">
        {props.customer.total_commands.toLocaleString()}
      </td>

      <td class="px-4 py-3 text-xs text-nebula-500">
        {formatRelativeTime(props.customer.last_activity_at)}
      </td>

      <td class="px-4 py-3">
        <div class="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
          <button
            onClick={() => handleEmail(props.customer.email)}
            class="rounded-lg p-1.5 text-nebula-500 transition-all hover:bg-white/5 hover:text-white"
            title="Send Email"
          >
            <Mail size={14} />
          </button>
          <button
            onClick={() => props.onViewDetail?.(props.customer.id)}
            class="rounded-lg p-1.5 text-nebula-500 transition-all hover:bg-white/5 hover:text-white"
            title="View Details"
          >
            <Eye size={14} />
          </button>
        </div>
      </td>
    </tr>
  );
};

export default CRMProfileCard;
