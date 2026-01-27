import { Component, createSignal, onCleanup, Show, For, splitProps } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface SimpleLiveIndicatorProps {
  isLive: boolean;
  label?: string;
  class?: string;
}

export const SimpleLiveIndicator: Component<SimpleLiveIndicatorProps> = (props) => {
  const [local, others] = splitProps(props, ['isLive', 'label', 'class']);

  return (
    <Show when={local.isLive}>
      <div
        class={cn(
          'inline-flex items-center gap-2 rounded-full font-bold uppercase tracking-wider',
          'bg-aurora-500/10 px-2.5 py-1 text-xs text-aurora-400',
          'animate-pulse-glow',
          local.class
        )}
        role="status"
        aria-live="polite"
        aria-label={local.label || 'Live'}
        {...others}
      >
        <div class="relative">
          <div class="h-2 w-2 rounded-full bg-aurora-500" />
          <div class="absolute inset-0 h-2 w-2 rounded-full bg-aurora-500 animate-ping opacity-75" />
        </div>
        <span>{local.label || 'Live'}</span>
      </div>
    </Show>
  );
};

interface LiveIndicatorProps {
  label?: string;
  variant?: 'pulse' | 'ring' | 'dot' | 'bar';
  color?: 'success' | 'warning' | 'error' | 'info';
  size?: 'sm' | 'md' | 'lg';
  showLabel?: boolean;
  count?: number;
  class?: string;
}

const colorConfig = {
  success: {
    dot: 'bg-aurora-500',
    ring: 'border-aurora-500',
    glow: 'shadow-[0_0_8px_rgba(16,185,129,0.5)]',
    text: 'text-aurora-400',
    bg: 'bg-aurora-500/10',
  },
  warning: {
    dot: 'bg-solar-500',
    ring: 'border-solar-500',
    glow: 'shadow-[0_0_8px_rgba(245,158,11,0.5)]',
    text: 'text-solar-400',
    bg: 'bg-solar-500/10',
  },
  error: {
    dot: 'bg-flare-500',
    ring: 'border-flare-500',
    glow: 'shadow-[0_0_8px_rgba(239,68,68,0.5)]',
    text: 'text-flare-400',
    bg: 'bg-flare-500/10',
  },
  info: {
    dot: 'bg-plasma-500',
    ring: 'border-plasma-500',
    glow: 'shadow-[0_0_8px_rgba(59,125,209,0.5)]',
    text: 'text-plasma-400',
    bg: 'bg-plasma-500/10',
  },
};

const sizeConfig = {
  sm: { dot: 'h-1.5 w-1.5', ring: 'h-3 w-3', text: 'text-2xs', padding: 'px-2 py-0.5' },
  md: { dot: 'h-2 w-2', ring: 'h-4 w-4', text: 'text-xs', padding: 'px-2.5 py-1' },
  lg: { dot: 'h-2.5 w-2.5', ring: 'h-5 w-5', text: 'text-sm', padding: 'px-3 py-1.5' },
};

export const LiveIndicator: Component<LiveIndicatorProps> = (props) => {
  const color = () => colorConfig[props.color || 'success'];
  const size = () => sizeConfig[props.size || 'md'];
  const variant = () => props.variant || 'pulse';
  const showLabel = () => props.showLabel !== false;

  return (
    <div
      class={cn(
        'inline-flex items-center gap-2 rounded-full font-bold uppercase tracking-wider',
        color().bg,
        size().padding,
        size().text,
        color().text,
        props.class
      )}
    >
      <div class="relative">
        {variant() === 'pulse' && (
          <>
            <div class={cn('rounded-full', size().dot, color().dot)} />
            <div
              class={cn(
                'absolute inset-0 rounded-full animate-ping opacity-75',
                size().dot,
                color().dot
              )}
            />
          </>
        )}
        
        {variant() === 'ring' && (
          <div class="relative">
            <div class={cn('rounded-full', size().dot, color().dot, color().glow)} />
            <div
              class={cn(
                'absolute inset-[-4px] rounded-full border-2 animate-[ring-expand_1.5s_ease-out_infinite]',
                color().ring,
                'opacity-50'
              )}
            />
          </div>
        )}
        
        {variant() === 'dot' && (
          <div class={cn('rounded-full', size().dot, color().dot, color().glow)} />
        )}
        
        {variant() === 'bar' && (
          <div class="flex gap-0.5">
            {[0, 1, 2].map((i) => (
              <div
                class={cn('w-0.5 rounded-full', color().dot)}
                style={{
                  height: '8px',
                  animation: `data-pulse 1s ease-in-out ${i * 0.15}s infinite`,
                }}
              />
            ))}
          </div>
        )}
      </div>
      
      <Show when={showLabel()}>
        <span>{props.label || 'Live'}</span>
      </Show>
      
      <Show when={props.count !== undefined}>
        <span class="font-mono tabular-nums">{props.count}</span>
      </Show>
    </div>
  );
};

interface StreamCounterProps {
  count: number;
  label?: string;
  rate?: number;
  class?: string;
}

export const StreamCounter: Component<StreamCounterProps> = (props) => {
  const [displayCount, setDisplayCount] = createSignal(props.count);
  const [isAnimating, setIsAnimating] = createSignal(false);

  let prevCount = props.count;

  const updateDisplay = () => {
    if (props.count !== prevCount) {
      setIsAnimating(true);
      const diff = props.count - prevCount;
      const steps = Math.min(Math.abs(diff), 20);
      const increment = diff / steps;
      let current = prevCount;
      let step = 0;

      const interval = setInterval(() => {
        step++;
        current += increment;
        setDisplayCount(Math.round(current));
        if (step >= steps) {
          clearInterval(interval);
          setDisplayCount(props.count);
          setIsAnimating(false);
        }
      }, 30);

      prevCount = props.count;
    }
  };

  onCleanup(() => {});

  return (
    <div class={cn('inline-flex items-center gap-3', props.class)}>
      <div class="relative">
        <span
          class={cn(
            'font-display text-2xl font-black tabular-nums text-white transition-transform',
            isAnimating() && 'scale-110'
          )}
        >
          {displayCount().toLocaleString()}
        </span>
        <Show when={isAnimating()}>
          <div class="absolute inset-0 bg-gradient-to-r from-transparent via-white/20 to-transparent animate-shimmer" />
        </Show>
      </div>
      <Show when={props.label}>
        <span class="text-xs font-bold uppercase tracking-widest text-nebula-500">
          {props.label}
        </span>
      </Show>
      <Show when={props.rate}>
        <span class="text-xs text-aurora-400 font-medium">
          +{props.rate}/s
        </span>
      </Show>
    </div>
  );
};

interface PresenceIndicatorProps {
  status: 'online' | 'idle' | 'offline' | 'busy';
  label?: string;
  lastSeen?: string;
  size?: 'sm' | 'md' | 'lg';
  class?: string;
}

const presenceConfig = {
  online: {
    color: 'bg-aurora-500',
    glow: 'shadow-[0_0_6px_rgba(16,185,129,0.5)]',
    label: 'Online',
  },
  idle: {
    color: 'bg-solar-500',
    glow: 'shadow-[0_0_6px_rgba(245,158,11,0.4)]',
    label: 'Idle',
  },
  offline: {
    color: 'bg-nebula-600',
    glow: '',
    label: 'Offline',
  },
  busy: {
    color: 'bg-flare-500',
    glow: 'shadow-[0_0_6px_rgba(239,68,68,0.4)]',
    label: 'Busy',
  },
};

export const PresenceIndicator: Component<PresenceIndicatorProps> = (props) => {
  const config = () => presenceConfig[props.status];
  const size = () => sizeConfig[props.size || 'md'];

  return (
    <div class={cn('inline-flex items-center gap-2', props.class)}>
      <div class="relative">
        <div class={cn('rounded-full', size().dot, config().color, config().glow)} />
        <Show when={props.status === 'online'}>
          <div
            class={cn(
              'absolute inset-0 rounded-full animate-ping opacity-50',
              size().dot,
              config().color
            )}
          />
        </Show>
      </div>
      <Show when={props.label !== undefined}>
        <span class="text-sm font-medium text-nebula-300">
          {props.label || config().label}
        </span>
      </Show>
      <Show when={props.lastSeen && props.status === 'offline'}>
        <span class="text-xs text-nebula-500">{props.lastSeen}</span>
      </Show>
    </div>
  );
};

interface DataStreamProps {
  items: Array<{ id: string; content: string; type?: 'success' | 'error' | 'info' }>;
  maxVisible?: number;
  class?: string;
}

export const DataStream: Component<DataStreamProps> = (props) => {
  const maxVisible = () => props.maxVisible || 5;
  const visibleItems = () => props.items.slice(0, maxVisible());

  const typeColors = {
    success: 'border-l-aurora-500',
    error: 'border-l-flare-500',
    info: 'border-l-plasma-500',
  };

  return (
    <div class={cn('space-y-2 overflow-hidden', props.class)}>
      <For each={visibleItems()}>
        {(item, index) => (
          <div
            class={cn(
              'rounded-lg border border-white/5 bg-void-800/50 px-3 py-2 border-l-2',
              'animate-stream-in opacity-0',
              typeColors[item.type || 'info']
            )}
            style={{
              'animation-delay': `${index() * 50}ms`,
              'animation-fill-mode': 'forwards',
            }}
          >
            <p class="text-sm text-nebula-300 font-mono truncate">{item.content}</p>
          </div>
        )}
      </For>
    </div>
  );
};

export default LiveIndicator;
