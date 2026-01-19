import { Component } from 'solid-js';

type BadgeVariant = 'success' | 'warning' | 'error' | 'info' | 'neutral' | 'pro' | 'team' | 'enterprise';

interface BadgeProps {
  variant?: BadgeVariant;
  children: string;
  size?: 'sm' | 'md' | 'lg';
  pulse?: boolean;
}

const variantClasses: Record<BadgeVariant, string> = {
  success: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30',
  warning: 'bg-amber-500/20 text-amber-400 border-amber-500/30',
  error: 'bg-red-500/20 text-red-400 border-red-500/30',
  info: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
  neutral: 'bg-slate-500/20 text-slate-400 border-slate-500/30',
  pro: 'bg-gradient-to-r from-indigo-500/20 to-purple-500/20 text-indigo-300 border-indigo-500/30',
  team: 'bg-gradient-to-r from-cyan-500/20 to-blue-500/20 text-cyan-300 border-cyan-500/30',
  enterprise: 'bg-gradient-to-r from-amber-500/20 to-orange-500/20 text-amber-300 border-amber-500/30',
};

const sizeClasses = {
  sm: 'px-2 py-0.5 text-xs',
  md: 'px-2.5 py-1 text-xs',
  lg: 'px-3 py-1.5 text-sm',
};

export const Badge: Component<BadgeProps> = props => {
  return (
    <span
      class={`inline-flex items-center gap-1 rounded-full border font-medium ${variantClasses[props.variant || 'neutral']} ${sizeClasses[props.size || 'md']}`}
    >
      {props.pulse && (
        <span class="relative flex h-2 w-2">
          <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-current opacity-75" />
          <span class="relative inline-flex h-2 w-2 rounded-full bg-current" />
        </span>
      )}
      {props.children}
    </span>
  );
};

export const TierBadge: Component<{ tier: string }> = props => {
  const variant = (): BadgeVariant => {
    switch (props.tier.toLowerCase()) {
      case 'pro':
        return 'pro';
      case 'team':
        return 'team';
      case 'enterprise':
        return 'enterprise';
      default:
        return 'neutral';
    }
  };

  return <Badge variant={variant()}>{props.tier.toUpperCase()}</Badge>;
};

export const StatusBadge: Component<{ status: string; pulse?: boolean }> = props => {
  const variant = (): BadgeVariant => {
    switch (props.status.toLowerCase()) {
      case 'active':
      case 'online':
      case 'paid':
        return 'success';
      case 'pending':
      case 'trial':
        return 'warning';
      case 'inactive':
      case 'suspended':
      case 'failed':
        return 'error';
      default:
        return 'neutral';
    }
  };

  return (
    <Badge variant={variant()} pulse={props.pulse}>
      {props.status}
    </Badge>
  );
};

export default Badge;
