import { Component, For, Show, createMemo, createSignal, onMount } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface HeatmapCell {
  x: number;
  y: number;
  value: number;
}

interface HeatmapProps {
  data: HeatmapCell[];
  xLabels?: string[];
  yLabels?: string[];
  colorScale?: 'indigo' | 'aurora' | 'flare' | 'solar';
  showLegend?: boolean;
  cellSize?: 'sm' | 'md' | 'lg';
  class?: string;
}

const colorScales = {
  indigo: [
    'bg-void-800',
    'bg-indigo-900/40',
    'bg-indigo-800/50',
    'bg-indigo-700/60',
    'bg-indigo-600/70',
    'bg-indigo-500/80',
    'bg-indigo-400',
  ],
  aurora: [
    'bg-void-800',
    'bg-aurora-900/40',
    'bg-aurora-800/50',
    'bg-aurora-700/60',
    'bg-aurora-600/70',
    'bg-aurora-500/80',
    'bg-aurora-400',
  ],
  flare: [
    'bg-void-800',
    'bg-flare-900/40',
    'bg-flare-800/50',
    'bg-flare-700/60',
    'bg-flare-600/70',
    'bg-flare-500/80',
    'bg-flare-400',
  ],
  solar: [
    'bg-void-800',
    'bg-solar-900/40',
    'bg-solar-800/50',
    'bg-solar-700/60',
    'bg-solar-600/70',
    'bg-solar-500/80',
    'bg-solar-400',
  ],
};

const cellSizes = {
  sm: 'h-3 w-3',
  md: 'h-4 w-4',
  lg: 'h-5 w-5',
};

export const Heatmap: Component<HeatmapProps> = (props) => {
  const scale = () => colorScales[props.colorScale || 'indigo'];
  const size = () => cellSizes[props.cellSize || 'md'];
  
  const maxValue = createMemo(() => Math.max(...props.data.map(d => d.value), 1));
  const xCount = createMemo(() => props.xLabels?.length || Math.max(...props.data.map(d => d.x)) + 1);
  const yCount = createMemo(() => props.yLabels?.length || Math.max(...props.data.map(d => d.y)) + 1);

  const getCellColor = (value: number) => {
    if (value === 0) return scale()[0];
    const intensity = value / maxValue();
    const index = Math.min(Math.floor(intensity * (scale().length - 1)) + 1, scale().length - 1);
    return scale()[index];
  };

  const getValue = (x: number, y: number) => {
    const cell = props.data.find(d => d.x === x && d.y === y);
    return cell?.value || 0;
  };

  return (
    <div class={cn('space-y-2', props.class)}>
      <Show when={props.xLabels}>
        <div class="flex pl-10 gap-1">
          <For each={props.xLabels}>
            {(label, i) => (
              <Show when={i() % 3 === 0}>
                <div class={cn('text-center text-2xs text-nebula-600', size())} style={{ width: size() }}>
                  {label}
                </div>
              </Show>
            )}
          </For>
        </div>
      </Show>
      
      <div class="flex flex-col gap-1">
        <For each={Array(yCount()).fill(0)}>
          {(_, y) => (
            <div class="flex items-center gap-1">
              <Show when={props.yLabels}>
                <div class="w-8 text-right text-2xs text-nebula-600 pr-1">
                  {props.yLabels![y()]}
                </div>
              </Show>
              <For each={Array(xCount()).fill(0)}>
                {(_, x) => {
                  const value = getValue(x(), y());
                  return (
                    <div
                      class={cn(
                        'rounded-sm transition-all cursor-pointer',
                        'hover:scale-125 hover:z-10 hover:shadow-lg',
                        size(),
                        getCellColor(value)
                      )}
                      title={`${props.yLabels?.[y()] || y()} ${props.xLabels?.[x()] || x()}:00 - ${value} commands`}
                    />
                  );
                }}
              </For>
            </div>
          )}
        </For>
      </div>

      <Show when={props.showLegend}>
        <div class="flex items-center justify-end gap-2 text-2xs text-nebula-500 pt-2">
          <span>Less</span>
          <div class="flex gap-0.5">
            <For each={scale()}>
              {(color) => (
                <div class={cn('rounded-sm', size(), color)} style={{ width: '12px', height: '12px' }} />
              )}
            </For>
          </div>
          <span>More</span>
        </div>
      </Show>
    </div>
  );
};

interface SparklineProps {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  showArea?: boolean;
  showDots?: boolean;
  animated?: boolean;
  class?: string;
}

export const Sparkline: Component<SparklineProps> = (props) => {
  const width = () => props.width || 120;
  const height = () => props.height || 32;
  const color = () => props.color || '#6366f1';

  const maxValue = createMemo(() => Math.max(...props.data, 1));
  const minValue = createMemo(() => Math.min(...props.data, 0));
  const range = createMemo(() => maxValue() - minValue() || 1);

  const points = createMemo(() => {
    return props.data.map((value, i) => ({
      x: (i / (props.data.length - 1)) * width(),
      y: height() - ((value - minValue()) / range()) * height(),
      value,
    }));
  });

  const linePath = createMemo(() => {
    const pts = points();
    if (pts.length === 0) return '';
    return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x},${p.y}`).join(' ');
  });

  const areaPath = createMemo(() => {
    const pts = points();
    if (pts.length === 0) return '';
    let path = `M 0,${height()}`;
    pts.forEach(p => { path += ` L ${p.x},${p.y}`; });
    path += ` L ${width()},${height()} Z`;
    return path;
  });

  return (
    <svg
      width={width()}
      height={height()}
      class={cn('overflow-visible', props.class)}
      viewBox={`0 0 ${width()} ${height()}`}
    >
      <defs>
        <linearGradient id={`sparkline-grad-${color().replace('#', '')}`} x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stop-color={color()} stop-opacity="0.4" />
          <stop offset="100%" stop-color={color()} stop-opacity="0" />
        </linearGradient>
      </defs>
      
      <Show when={props.showArea}>
        <path
          d={areaPath()}
          fill={`url(#sparkline-grad-${color().replace('#', '')})`}
          class={props.animated ? 'animate-fade-up' : ''}
        />
      </Show>
      
      <path
        d={linePath()}
        fill="none"
        stroke={color()}
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        class={props.animated ? 'animate-fade-up' : ''}
      />

      <Show when={props.showDots}>
        <For each={points()}>
          {(point, i) => (
            <circle
              cx={point.x}
              cy={point.y}
              r="3"
              fill={color()}
              class="opacity-0 hover:opacity-100 transition-opacity cursor-pointer"
            />
          )}
        </For>
      </Show>
    </svg>
  );
};

interface ProgressRingProps {
  value: number;
  max?: number;
  size?: number;
  strokeWidth?: number;
  color?: string;
  trackColor?: string;
  showValue?: boolean;
  label?: string;
  animated?: boolean;
  class?: string;
}

export const ProgressRing: Component<ProgressRingProps> = (props) => {
  const size = () => props.size || 80;
  const strokeWidth = () => props.strokeWidth || 6;
  const max = () => props.max || 100;
  const color = () => props.color || '#6366f1';
  const trackColor = () => props.trackColor || '#1e1e2e';

  const radius = createMemo(() => (size() - strokeWidth()) / 2);
  const circumference = createMemo(() => 2 * Math.PI * radius());
  const progress = createMemo(() => Math.min(props.value / max(), 1));
  const offset = createMemo(() => circumference() - progress() * circumference());

  return (
    <div class={cn('relative inline-flex items-center justify-center', props.class)}>
      <svg width={size()} height={size()} class="rotate-[-90deg]">
        <circle
          cx={size() / 2}
          cy={size() / 2}
          r={radius()}
          fill="none"
          stroke={trackColor()}
          stroke-width={strokeWidth()}
        />
        <circle
          cx={size() / 2}
          cy={size() / 2}
          r={radius()}
          fill="none"
          stroke={color()}
          stroke-width={strokeWidth()}
          stroke-linecap="round"
          stroke-dasharray={`${circumference()}`}
          stroke-dashoffset={props.animated ? circumference() : offset()}
          class={cn(
            'transition-all duration-1000 ease-smooth',
            props.animated && 'animate-gauge-fill'
          )}
          style={{
            '--gauge-circumference': `${circumference()}`,
            '--gauge-offset': `${offset()}`,
            filter: `drop-shadow(0 0 6px ${color()})`,
          }}
        />
      </svg>
      <Show when={props.showValue || props.label}>
        <div class="absolute inset-0 flex flex-col items-center justify-center">
          <Show when={props.showValue}>
            <span class="font-display font-black text-white tabular-nums" style={{ 'font-size': `${size() / 4}px` }}>
              {Math.round(props.value)}
            </span>
          </Show>
          <Show when={props.label}>
            <span class="text-2xs text-nebula-500 font-medium">{props.label}</span>
          </Show>
        </div>
      </Show>
    </div>
  );
};

interface BarData {
  label: string;
  value: number;
  color?: string;
}

interface BarChartProps {
  data: BarData[];
  height?: number;
  horizontal?: boolean;
  showLabels?: boolean;
  showValues?: boolean;
  animated?: boolean;
  class?: string;
}

export const BarChart: Component<BarChartProps> = (props) => {
  const height = () => props.height || 160;
  const maxValue = createMemo(() => Math.max(...props.data.map(d => d.value), 1));

  if (props.horizontal) {
    return (
      <div class={cn('space-y-3', props.class)}>
        <For each={props.data}>
          {(item, i) => (
            <div class="space-y-1">
              <Show when={props.showLabels}>
                <div class="flex items-center justify-between text-sm">
                  <span class="text-nebula-400 font-medium">{item.label}</span>
                  <Show when={props.showValues}>
                    <span class="text-white font-bold tabular-nums">{item.value.toLocaleString()}</span>
                  </Show>
                </div>
              </Show>
              <div class="h-2 rounded-full bg-void-700 overflow-hidden">
                <div
                  class={cn(
                    'h-full rounded-full transition-all duration-700 ease-smooth',
                    props.animated && 'animate-[score-fill_1s_ease-out_forwards]'
                  )}
                  style={{
                    width: `${(item.value / maxValue()) * 100}%`,
                    'background-color': item.color || '#6366f1',
                    'animation-delay': props.animated ? `${i() * 100}ms` : '0ms',
                  }}
                />
              </div>
            </div>
          )}
        </For>
      </div>
    );
  }

  return (
    <div class={cn('flex items-end gap-2', props.class)} style={{ height: `${height()}px` }}>
      <For each={props.data}>
        {(item, i) => {
          const barHeight = (item.value / maxValue()) * 100;
          return (
            <div class="group relative flex flex-1 flex-col items-center gap-2">
              <div
                class={cn(
                  'w-full rounded-t-lg transition-all duration-700 ease-smooth',
                  'group-hover:brightness-125'
                )}
                style={{
                  height: `${Math.max(barHeight, 4)}%`,
                  'min-height': '4px',
                  'background-color': item.color || '#6366f1',
                  'box-shadow': item.value > 0 ? `0 0 20px -5px ${item.color || 'rgba(99,102,241,0.3)'}` : 'none',
                  'animation-delay': props.animated ? `${i() * 50}ms` : '0ms',
                }}
              />
              <Show when={props.showLabels}>
                <span class="w-full truncate text-center text-2xs font-bold uppercase tracking-widest text-nebula-600 group-hover:text-nebula-400 transition-colors">
                  {item.label}
                </span>
              </Show>
              <div class="pointer-events-none absolute bottom-full left-1/2 z-30 mb-3 -translate-x-1/2 scale-95 rounded-xl border border-white/10 bg-void-900/95 p-2 text-xs whitespace-nowrap text-white opacity-0 shadow-xl backdrop-blur-md transition-all duration-200 group-hover:scale-100 group-hover:opacity-100">
                <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">{item.label}</div>
                <div class="text-sm font-black tabular-nums">{item.value.toLocaleString()}</div>
              </div>
            </div>
          );
        }}
      </For>
    </div>
  );
};

interface DonutSegment {
  label: string;
  value: number;
  color: string;
}

interface DonutChartProps {
  data: DonutSegment[];
  size?: number;
  thickness?: number;
  centerLabel?: string;
  centerValue?: string | number;
  showLegend?: boolean;
  animated?: boolean;
  class?: string;
}

export const DonutChart: Component<DonutChartProps> = (props) => {
  const size = () => props.size || 120;
  const thickness = () => props.thickness || 20;
  const radius = () => (size() - thickness()) / 2;
  const circumference = () => 2 * Math.PI * radius();
  const total = createMemo(() => props.data.reduce((sum, d) => sum + d.value, 0));

  const [hoveredIndex, setHoveredIndex] = createSignal<number | null>(null);

  const segments = createMemo(() => {
    let offset = 0;
    return props.data.map((item, index) => {
      const percentage = total() > 0 ? item.value / total() : 0;
      const strokeDasharray = `${percentage * circumference()} ${circumference()}`;
      const strokeDashoffset = -offset;
      offset += percentage * circumference();
      return { ...item, percentage, strokeDasharray, strokeDashoffset, index };
    });
  });

  return (
    <div class={cn('flex items-center gap-6', props.class)}>
      <div class="relative" style={{ width: `${size()}px`, height: `${size()}px` }}>
        <svg width={size()} height={size()} class="-rotate-90">
          <circle
            cx={size() / 2}
            cy={size() / 2}
            r={radius()}
            fill="none"
            stroke="#1e293b"
            stroke-width={thickness()}
          />
          <For each={segments()}>
            {(segment) => (
              <circle
                cx={size() / 2}
                cy={size() / 2}
                r={radius()}
                fill="none"
                stroke={segment.color}
                stroke-width={hoveredIndex() === segment.index ? thickness() + 4 : thickness()}
                stroke-dasharray={segment.strokeDasharray}
                stroke-dashoffset={segment.strokeDashoffset}
                stroke-linecap="round"
                class="cursor-pointer transition-all duration-300"
                style={hoveredIndex() === segment.index ? { filter: `drop-shadow(0 0 8px ${segment.color})` } : {}}
                onMouseEnter={() => setHoveredIndex(segment.index)}
                onMouseLeave={() => setHoveredIndex(null)}
              />
            )}
          </For>
        </svg>
        <div class="absolute inset-0 flex flex-col items-center justify-center">
          <div class="text-2xl font-display font-black text-white">
            {props.centerValue !== undefined ? props.centerValue : total()}
          </div>
          <div class="text-xs text-nebula-500">{props.centerLabel || 'Total'}</div>
        </div>
      </div>
      
      <Show when={props.showLegend}>
        <div class="flex flex-col gap-2">
          <For each={props.data}>
            {(item, index) => (
              <div
                class={cn(
                  'flex items-center gap-2 rounded-lg px-2 py-1.5 transition-all cursor-pointer',
                  hoveredIndex() === index() && 'bg-void-800'
                )}
                onMouseEnter={() => setHoveredIndex(index())}
                onMouseLeave={() => setHoveredIndex(null)}
              >
                <div class="h-3 w-3 rounded-full" style={{ background: item.color }} />
                <span class="text-sm text-nebula-400">{item.label}</span>
                <span class="ml-auto text-sm font-bold text-white tabular-nums">{item.value}</span>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

export default {
  Heatmap,
  Sparkline,
  ProgressRing,
  BarChart,
  DonutChart,
};
