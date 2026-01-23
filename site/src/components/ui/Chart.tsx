import { Component, For, Show, createSignal } from 'solid-js';

interface BarChartProps {
  data: Array<{ label: string; value: number; secondaryValue?: number; color?: string }>;
  height?: number;
  showLabels?: boolean;
  tooltipFormatter?: (value: number, label: string) => string;
  secondaryTooltipFormatter?: (value: number, label: string) => string;
  gradient?: 'indigo' | 'emerald' | 'cyan' | 'purple' | 'orange';
  animated?: boolean;
}

const gradientColors = {
  indigo: { from: 'from-indigo-600', to: 'to-indigo-400', hover: 'hover:from-indigo-500 hover:to-indigo-300' },
  emerald: { from: 'from-emerald-600', to: 'to-emerald-400', hover: 'hover:from-emerald-500 hover:to-emerald-300' },
  cyan: { from: 'from-cyan-600', to: 'to-cyan-400', hover: 'hover:from-cyan-500 hover:to-cyan-300' },
  purple: { from: 'from-purple-600', to: 'to-purple-400', hover: 'hover:from-purple-500 hover:to-purple-300' },
  orange: { from: 'from-orange-600', to: 'to-orange-400', hover: 'hover:from-orange-500 hover:to-orange-300' },
};

export const BarChart: Component<BarChartProps> = props => {
  const maxValue = () => Math.max(...props.data.map(d => d.value), 1);
  const height = props.height || 160;
  const colors = gradientColors[props.gradient || 'indigo'];

  return (
    <div class="flex items-end gap-2" style={{ height: `${height}px` }}>
      <For each={props.data}>
        {(item, index) => {
          const barHeight = Math.max((item.value / maxValue()) * 100, 3);
          const delay = props.animated ? `${index() * 40}ms` : '0ms';
          return (
            <div class="group relative flex flex-1 flex-col items-center gap-2">
              <div
                class={`w-full rounded-t-lg bg-gradient-to-t ${colors.from} ${colors.to} transition-all duration-700 ease-[cubic-bezier(0.23,1,0.32,1)] group-hover:brightness-125`}
                style={{ 
                  height: `${barHeight}%`, 
                  'min-height': '4px',
                  'animation-delay': delay,
                  'box-shadow': item.value > 0 ? `0 0 20px -5px ${props.gradient === 'emerald' ? 'rgba(16,185,129,0.3)' : 'rgba(99,102,241,0.3)'}` : 'none'
                }}
              />
              <Show when={props.showLabels}>
                <span class="w-full truncate text-center text-[9px] font-bold uppercase tracking-widest text-slate-600 group-hover:text-slate-400 transition-colors">
                  {item.label}
                </span>
              </Show>
              <Show when={item.value > 0}>
                <div class="pointer-events-none absolute bottom-full left-1/2 z-30 mb-3 -translate-x-1/2 scale-95 rounded-[1.25rem] border border-white/10 bg-[#0a0a0b]/95 p-3 text-xs whitespace-nowrap text-white opacity-0 shadow-[0_20px_50px_rgba(0,0,0,0.5)] backdrop-blur-md ring-1 ring-white/5 transition-all duration-300 group-hover:scale-100 group-hover:opacity-100">
                  <div class="flex flex-col gap-1">
                    <div class="text-[10px] font-bold uppercase tracking-widest text-slate-500">{item.label}</div>
                    <div class="flex items-center gap-2">
                      <div class={`h-2 w-2 rounded-full ${props.gradient === 'emerald' ? 'bg-emerald-500' : 'bg-indigo-500'}`} />
                      <span class="text-sm font-black tabular-nums">
                        {props.tooltipFormatter
                          ? props.tooltipFormatter(item.value, item.label)
                          : item.value.toLocaleString()}
                      </span>
                    </div>
                  </div>
                  <Show when={item.secondaryValue !== undefined}>
                    <div class="mt-2 flex items-center gap-2 border-t border-white/5 pt-2 text-[10px] font-bold uppercase tracking-wider text-slate-500">
                      <span>Volume:</span>
                      <span class="text-slate-300 tabular-nums">
                        {props.secondaryTooltipFormatter
                          ? props.secondaryTooltipFormatter(item.secondaryValue!, item.label)
                          : item.secondaryValue}
                      </span>
                    </div>
                  </Show>
                </div>
              </Show>
            </div>
          );
        }}
      </For>
    </div>
  );
};

interface HeatmapProps {
  data: Array<{ day: number; hour: number; value: number }>;
  maxValue?: number;
}

export const ActivityHeatmap: Component<HeatmapProps> = props => {
  const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const hours = Array.from({ length: 24 }, (_, i) => i);
  const max = () => props.maxValue || Math.max(...props.data.map(d => d.value), 1);

  const getValue = (day: number, hour: number) => {
    const item = props.data.find(d => d.day === day && d.hour === hour);
    return item?.value || 0;
  };

  const getColor = (value: number) => {
    if (value === 0) return 'bg-slate-800/50';
    const intensity = value / max();
    if (intensity < 0.25) return 'bg-indigo-900/50';
    if (intensity < 0.5) return 'bg-indigo-700/60';
    if (intensity < 0.75) return 'bg-indigo-500/70';
    return 'bg-indigo-400';
  };

  return (
    <div class="overflow-x-auto">
      <div class="inline-flex flex-col gap-1">
        <div class="flex gap-1 pl-10">
          <For each={hours.filter((_, i) => i % 3 === 0)}>
            {hour => (
              <div class="w-8 text-center text-[10px] text-slate-500">
                {hour.toString().padStart(2, '0')}
              </div>
            )}
          </For>
        </div>
        <For each={days}>
          {(day, dayIndex) => (
            <div class="flex items-center gap-1">
              <div class="w-8 text-right text-[10px] text-slate-500">{day}</div>
              <For each={hours}>
                {hour => (
                  <div
                    class={`h-3 w-3 rounded-sm transition-colors ${getColor(getValue(dayIndex(), hour))}`}
                    title={`${day} ${hour}:00 - ${getValue(dayIndex(), hour)} commands`}
                  />
                )}
              </For>
            </div>
          )}
        </For>
      </div>
      <div class="mt-3 flex items-center justify-end gap-2 text-[10px] text-slate-500">
        <span>Less</span>
        <div class="flex gap-0.5">
          <div class="h-3 w-3 rounded-sm bg-slate-800/50" />
          <div class="h-3 w-3 rounded-sm bg-indigo-900/50" />
          <div class="h-3 w-3 rounded-sm bg-indigo-700/60" />
          <div class="h-3 w-3 rounded-sm bg-indigo-500/70" />
          <div class="h-3 w-3 rounded-sm bg-indigo-400" />
        </div>
        <span>More</span>
      </div>
    </div>
  );
};

interface DonutChartProps {
  data: Array<{ label: string; value: number; color: string }>;
  size?: number;
  thickness?: number;
  centerLabel?: string;
  centerValue?: string | number;
  showLegend?: boolean;
}

export const DonutChart: Component<DonutChartProps> = props => {
  const size = props.size || 120;
  const thickness = props.thickness || 20;
  const radius = (size - thickness) / 2;
  const circumference = 2 * Math.PI * radius;
  const total = () => props.data.reduce((sum, d) => sum + d.value, 0);
  const [hoveredIndex, setHoveredIndex] = createSignal<number | null>(null);

  const segments = () => {
    let offset = 0;
    return props.data.map((item, index) => {
      const percentage = total() > 0 ? item.value / total() : 0;
      const strokeDasharray = `${percentage * circumference} ${circumference}`;
      const strokeDashoffset = -offset;
      offset += percentage * circumference;
      return { ...item, percentage, strokeDasharray, strokeDashoffset, index };
    });
  };

  return (
    <div class="flex items-center gap-4">
      <div class="relative" style={{ width: `${size}px`, height: `${size}px` }}>
        <svg width={size} height={size} class="-rotate-90">
          <circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            fill="none"
            stroke="#1e293b"
            stroke-width={thickness}
          />
          <For each={segments()}>
            {segment => (
              <circle
                cx={size / 2}
                cy={size / 2}
                r={radius}
                fill="none"
                stroke={segment.color}
                stroke-width={hoveredIndex() === segment.index ? thickness + 4 : thickness}
                stroke-dasharray={segment.strokeDasharray}
                stroke-dashoffset={segment.strokeDashoffset}
                stroke-linecap="round"
                class="cursor-pointer transition-all duration-300"
                onMouseEnter={() => setHoveredIndex(segment.index)}
                onMouseLeave={() => setHoveredIndex(null)}
              />
            )}
          </For>
        </svg>
        <div class="absolute inset-0 flex flex-col items-center justify-center">
          <div class="text-2xl font-bold text-white">
            {props.centerValue !== undefined ? props.centerValue : total()}
          </div>
          <div class="text-xs text-slate-400">{props.centerLabel || 'Total'}</div>
        </div>
      </div>
      <Show when={props.showLegend}>
        <div class="flex flex-col gap-2">
          <For each={props.data}>
            {(item, index) => (
              <div 
                class={`flex items-center gap-2 rounded-lg px-2 py-1 transition-all ${
                  hoveredIndex() === index() ? 'bg-slate-800/50' : ''
                }`}
                onMouseEnter={() => setHoveredIndex(index())}
                onMouseLeave={() => setHoveredIndex(null)}
              >
                <div class="h-3 w-3 rounded-full" style={{ background: item.color }} />
                <span class="text-sm text-slate-400">{item.label}</span>
                <span class="ml-auto text-sm font-medium text-white">{item.value}</span>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

interface AreaChartProps {
  data: Array<{ label: string; value: number }>;
  height?: number;
  color?: string;
  showLabels?: boolean;
  showGrid?: boolean;
  tooltipFormatter?: (value: number, label: string) => string;
}

export const AreaChart: Component<AreaChartProps> = props => {
  const height = props.height || 160;
  const width = 400;
  const padding = { top: 20, right: 20, bottom: 30, left: 40 };
  const color = props.color || '#6366f1';
  
  const chartWidth = width - padding.left - padding.right;
  const chartHeight = height - padding.top - padding.bottom;
  
  const maxValue = () => Math.max(...props.data.map(d => d.value), 1);
  const minValue = () => Math.min(...props.data.map(d => d.value), 0);
  const range = () => maxValue() - minValue() || 1;

  const points = () => {
    return props.data.map((d, i) => ({
      x: padding.left + (i / (props.data.length - 1 || 1)) * chartWidth,
      y: padding.top + chartHeight - ((d.value - minValue()) / range()) * chartHeight,
      ...d,
    }));
  };

  const linePath = () => {
    const pts = points();
    if (pts.length === 0) return '';
    return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x},${p.y}`).join(' ');
  };

  const areaPath = () => {
    const pts = points();
    if (pts.length === 0) return '';
    let path = `M ${padding.left},${padding.top + chartHeight}`;
    pts.forEach(p => { path += ` L ${p.x},${p.y}`; });
    path += ` L ${padding.left + chartWidth},${padding.top + chartHeight} Z`;
    return path;
  };

  const gridLines = () => {
    const lines = [];
    for (let i = 0; i <= 4; i++) {
      const y = padding.top + (i / 4) * chartHeight;
      const value = maxValue() - (i / 4) * range();
      lines.push({ y, value: Math.round(value) });
    }
    return lines;
  };

  return (
    <svg width="100%" height={height} viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="xMidYMid meet">
      <defs>
        <linearGradient id="area-gradient" x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stop-color={color} stop-opacity="0.3" />
          <stop offset="100%" stop-color={color} stop-opacity="0" />
        </linearGradient>
      </defs>
      
      <Show when={props.showGrid}>
        <For each={gridLines()}>
          {line => (
            <>
              <line
                x1={padding.left}
                y1={line.y}
                x2={padding.left + chartWidth}
                y2={line.y}
                stroke="#334155"
                stroke-width="1"
                stroke-dasharray="4,4"
              />
              <text
                x={padding.left - 8}
                y={line.y + 4}
                text-anchor="end"
                class="fill-slate-500 text-[10px]"
              >
                {line.value}
              </text>
            </>
          )}
        </For>
      </Show>

      <path d={areaPath()} fill="url(#area-gradient)" />
      <path d={linePath()} fill="none" stroke={color} stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
      
      <For each={points()}>
        {(point, index) => (
          <g class="group">
            <circle
              cx={point.x}
              cy={point.y}
              r="4"
              fill={color}
              class="opacity-0 transition-opacity group-hover:opacity-100"
            />
            <foreignObject
              x={point.x - 60}
              y={point.y - 60}
              width="120"
              height="50"
              class="pointer-events-none opacity-0 transition-opacity group-hover:opacity-100"
            >
              <div class="flex flex-col items-center justify-center">
                <div class="rounded-xl border border-white/10 bg-[#0a0a0b]/95 p-2 text-[10px] shadow-2xl backdrop-blur-md ring-1 ring-white/5">
                  <div class="font-bold uppercase tracking-widest text-slate-500">{point.label}</div>
                  <div class="font-black text-white tabular-nums">
                    {props.tooltipFormatter ? props.tooltipFormatter(point.value, point.label) : point.value.toLocaleString()}
                  </div>
                </div>
              </div>
            </foreignObject>
            <circle
              cx={point.x}
              cy={point.y}
              r="12"
              fill="transparent"
              class="cursor-pointer"
            />
            <Show when={props.showLabels && index() % Math.ceil(props.data.length / 7) === 0}>
              <text
                x={point.x}
                y={padding.top + chartHeight + 16}
                text-anchor="middle"
                class="fill-slate-500 text-[10px]"
              >
                {point.label}
              </text>
            </Show>
          </g>
        )}
      </For>
    </svg>
  );
};

interface LiveIndicatorProps {
  label?: string;
  pulse?: boolean;
}

export const LiveIndicator: Component<LiveIndicatorProps> = props => {
  return (
    <div class="inline-flex items-center gap-2 rounded-full bg-emerald-500/10 px-3 py-1 text-xs font-medium text-emerald-400">
      <div class="relative">
        <div class="h-2 w-2 rounded-full bg-emerald-400" />
        <Show when={props.pulse !== false}>
          <div class="absolute inset-0 h-2 w-2 animate-ping rounded-full bg-emerald-400 opacity-75" />
        </Show>
      </div>
      {props.label || 'Live'}
    </div>
  );
};
