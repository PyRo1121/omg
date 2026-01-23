import { Component, JSX, Show } from 'solid-js';
import { Sparkline, TrendIndicator } from './Sparkline';

interface CardProps {
  children: JSX.Element;
  class?: string;
  gradient?: 'emerald' | 'cyan' | 'indigo' | 'orange' | 'purple' | 'pink' | 'red' | 'none';
  hover?: boolean;
  glow?: boolean;
  onClick?: () => void;
}

const gradientClasses = {
  emerald: 'border-emerald-500/20 bg-gradient-to-br from-emerald-500/10 to-teal-500/10',
  cyan: 'border-cyan-500/20 bg-gradient-to-br from-cyan-500/10 to-blue-500/10',
  indigo: 'border-indigo-500/20 bg-gradient-to-br from-indigo-500/10 to-purple-500/10',
  orange: 'border-orange-500/20 bg-gradient-to-br from-orange-500/10 to-amber-500/10',
  purple: 'border-purple-500/20 bg-gradient-to-br from-purple-500/10 to-pink-500/10',
  pink: 'border-pink-500/20 bg-gradient-to-br from-pink-500/10 to-rose-500/10',
  red: 'border-red-500/20 bg-gradient-to-br from-red-500/10 to-orange-500/10',
  none: 'border-slate-800/60 bg-slate-900/50 backdrop-blur-sm',
};

const glowClasses = {
  emerald: 'hover:shadow-emerald-500/10',
  cyan: 'hover:shadow-cyan-500/10',
  indigo: 'hover:shadow-indigo-500/10',
  orange: 'hover:shadow-orange-500/10',
  purple: 'hover:shadow-purple-500/10',
  pink: 'hover:shadow-pink-500/10',
  red: 'hover:shadow-red-500/10',
  none: 'hover:shadow-slate-500/5',
};

export const Card: Component<CardProps> = props => {
  return (
    <div
      class={`rounded-2xl border p-6 transition-all duration-300 ${gradientClasses[props.gradient || 'none']} ${
        props.hover ? 'cursor-pointer hover:scale-[1.02] hover:border-opacity-50 hover:shadow-xl ' + glowClasses[props.gradient || 'none'] : ''
      } ${props.onClick ? 'cursor-pointer' : ''} ${props.class || ''}`}
      onClick={props.onClick}
    >
      {props.children}
    </div>
  );
};

interface StatCardProps {
  icon: string;
  label: string;
  value: string | number;
  subValue?: string;
  trend?: { value: number; positive: boolean };
  sparklineData?: number[];
  gradient?: CardProps['gradient'];
  loading?: boolean;
}

export const StatCard: Component<StatCardProps> = props => {
  const sparklineColor = () => {
    const colors: Record<string, string> = {
      emerald: '#10b981',
      cyan: '#06b6d4',
      indigo: '#6366f1',
      orange: '#f97316',
      purple: '#a855f7',
      pink: '#ec4899',
      red: '#ef4444',
      none: '#6366f1',
    };
    return colors[props.gradient || 'none'];
  };

  return (
    <Card gradient={props.gradient} hover>
      <div class="flex items-start justify-between">
        <div class="flex-1">
          <div class="mb-1 flex items-center gap-2">
            <span class="text-sm font-medium text-slate-400">{props.label}</span>
            <Show when={props.trend}>
              <TrendIndicator 
                value={props.trend!.positive ? 100 + props.trend!.value : 100 - props.trend!.value} 
                previousValue={100} 
                size="sm"
              />
            </Show>
          </div>
          <div class={`text-3xl font-bold text-white transition-all ${props.loading ? 'animate-pulse' : ''}`}>
            {props.loading ? (
              <div class="h-9 w-24 rounded-lg bg-slate-700/50" />
            ) : (
              props.value
            )}
          </div>
          {props.subValue && <div class="mt-1 text-sm text-slate-500">{props.subValue}</div>}
        </div>
        <div class="flex flex-col items-end gap-2">
          <div class="rounded-xl bg-white/5 p-3 text-2xl backdrop-blur-sm transition-transform hover:scale-110">
            {props.icon}
          </div>
          <Show when={props.sparklineData && props.sparklineData.length > 0}>
            <Sparkline 
              data={props.sparklineData!} 
              width={80} 
              height={24} 
              color={sparklineColor()}
              showDots
            />
          </Show>
        </div>
      </div>
    </Card>
  );
};

interface MetricCardProps {
  title: string;
  value: string | number;
  previousValue?: number;
  currentValue?: number;
  icon?: string | JSX.Element;
  iconBg?: string;
  sparklineData?: number[];
  sparklineColor?: string;
  subtitle?: string;
  badge?: { text: string; color: 'emerald' | 'amber' | 'red' | 'blue' };
  onClick?: () => void;
}

export const MetricCard: Component<MetricCardProps> = props => {
  const badgeColors = {
    emerald: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
    amber: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
    red: 'bg-red-500/10 text-red-400 border-red-500/20',
    blue: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
  };

  return (
    <div 
      class={`group relative overflow-hidden rounded-[2rem] border border-white/5 bg-[#0d0d0e] p-6 shadow-2xl transition-all duration-500 hover:border-white/10 hover:scale-[1.02] ${
        props.onClick ? 'cursor-pointer' : ''
      }`}
      onClick={props.onClick}
    >
      <div class="absolute inset-0 bg-gradient-to-br from-white/[0.03] to-transparent opacity-0 transition-opacity group-hover:opacity-100" />
      
      <div class="relative flex items-start justify-between">
        <div class="flex-1">
          <div class="flex items-center gap-3">
            <span class="text-[10px] font-black uppercase tracking-widest text-slate-500 group-hover:text-slate-400 transition-colors">{props.title}</span>
            <Show when={props.badge}>
              <span class={`rounded-full border px-2 py-0.5 text-[9px] font-black uppercase tracking-tighter ${badgeColors[props.badge!.color]}`}>
                {props.badge!.text}
              </span>
            </Show>
          </div>
          
          <div class="mt-4 flex items-baseline gap-3">
            <span class="text-4xl font-black tracking-tight text-white">{props.value}</span>
            <Show when={props.previousValue !== undefined && props.currentValue !== undefined}>
              <TrendIndicator 
                value={props.currentValue!} 
                previousValue={props.previousValue!}
                size="sm"
              />
            </Show>
          </div>
          
          <Show when={props.subtitle}>
            <p class="mt-2 text-xs font-bold text-slate-600 group-hover:text-slate-500 transition-colors uppercase tracking-tight">{props.subtitle}</p>
          </Show>
        </div>

        <div class="flex flex-col items-end gap-4">
          <Show when={props.icon}>
            <div class={`flex h-12 w-12 items-center justify-center rounded-2xl shadow-inner transition-all duration-500 group-hover:scale-110 group-hover:rotate-3 ${props.iconBg || 'bg-white/[0.03]'}`}>
              {typeof props.icon === 'string' ? <span class="text-2xl">{props.icon}</span> : props.icon}
            </div>
          </Show>
          
          <Show when={props.sparklineData && props.sparklineData.length > 0}>
            <div class="opacity-50 group-hover:opacity-100 transition-opacity duration-500">
              <Sparkline 
                data={props.sparklineData!} 
                width={80} 
                height={32}
                color={props.sparklineColor || '#6366f1'}
                showDots
                strokeWidth={2.5}
              />
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
};

interface GlassCardProps {
  children: JSX.Element;
  class?: string;
  padding?: 'sm' | 'md' | 'lg';
}

export const GlassCard: Component<GlassCardProps> = props => {
  const paddingClasses = {
    sm: 'p-4',
    md: 'p-6',
    lg: 'p-8',
  };

  return (
    <div class={`rounded-2xl border border-white/10 bg-white/5 backdrop-blur-xl ${paddingClasses[props.padding || 'md']} ${props.class || ''}`}>
      {props.children}
    </div>
  );
};

export default Card;
