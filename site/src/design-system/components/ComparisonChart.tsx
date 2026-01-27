import { Component, For, Show, createMemo } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { 
  TrendingUp, 
  TrendingDown, 
  Minus,
  Calendar,
  ArrowRight
} from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type ComparisonPeriod = 'week' | 'month' | 'quarter';

export interface DataPoint {
  timestamp: string;
  value: number;
  label?: string;
}

export interface ComparisonDataSet {
  current: DataPoint[];
  previous: DataPoint[];
}

export interface ComparisonChartProps {
  title: string;
  data: ComparisonDataSet;
  period: ComparisonPeriod;
  onPeriodChange?: (period: ComparisonPeriod) => void;
  unit?: string;
  prefix?: string;
  showSparkline?: boolean;
  dualAxis?: boolean;
  secondaryData?: ComparisonDataSet;
  secondaryUnit?: string;
  secondaryPrefix?: string;
  height?: number;
  class?: string;
}

const PERIOD_LABELS: Record<ComparisonPeriod, { current: string; previous: string }> = {
  week: { current: 'This Week', previous: 'Last Week' },
  month: { current: 'This Month', previous: 'Last Month' },
  quarter: { current: 'This Quarter', previous: 'Last Quarter' },
};

const formatValue = (value: number, prefix?: string, unit?: string): string => {
  let formatted: string;
  if (value >= 1000000) {
    formatted = `${(value / 1000000).toFixed(1)}M`;
  } else if (value >= 1000) {
    formatted = `${(value / 1000).toFixed(1)}k`;
  } else {
    formatted = value.toFixed(value % 1 === 0 ? 0 : 1);
  }
  return `${prefix || ''}${formatted}${unit || ''}`;
};

const Sparkline: Component<{
  data: number[];
  color: string;
  dashed?: boolean;
  width?: number;
  height?: number;
}> = (props) => {
  const width = () => props.width || 60;
  const height = () => props.height || 24;

  const path = createMemo(() => {
    const data = props.data;
    if (data.length < 2) return '';
    
    const max = Math.max(...data);
    const min = Math.min(...data);
    const range = max - min || 1;
    const padding = 2;

    return data.map((value, i) => {
      const x = padding + (i / (data.length - 1)) * (width() - padding * 2);
      const y = padding + (1 - (value - min) / range) * (height() - padding * 2);
      return `${i === 0 ? 'M' : 'L'} ${x},${y}`;
    }).join(' ');
  });

  return (
    <svg width={width()} height={height()} class="overflow-visible">
      <path
        d={path()}
        fill="none"
        stroke={props.color}
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-dasharray={props.dashed ? '4 4' : undefined}
      />
    </svg>
  );
};

const PeriodSelector: Component<{
  value: ComparisonPeriod;
  onChange: (period: ComparisonPeriod) => void;
}> = (props) => {
  const periods: ComparisonPeriod[] = ['week', 'month', 'quarter'];

  return (
    <div class="flex items-center gap-1 p-1 rounded-xl bg-void-800 border border-white/5">
      <For each={periods}>
        {(period) => (
          <button
            type="button"
            class={cn(
              'px-3 py-1.5 rounded-lg text-xs font-bold uppercase tracking-widest',
              'transition-all duration-300',
              props.value === period
                ? 'bg-indigo-500/20 text-indigo-300'
                : 'text-nebula-500 hover:text-nebula-300 hover:bg-white/5'
            )}
            onClick={() => props.onChange(period)}
          >
            {period}
          </button>
        )}
      </For>
    </div>
  );
};

const ComparisonBars: Component<{
  data: ComparisonDataSet;
  height: number;
  prefix?: string;
  unit?: string;
}> = (props) => {
  const maxValue = createMemo(() => {
    const allValues = [...props.data.current, ...props.data.previous].map(d => d.value);
    return Math.max(...allValues, 1);
  });

  const chartHeight = () => props.height - 40;

  return (
    <div class="flex items-end gap-1" style={{ height: `${props.height}px` }}>
      <For each={props.data.current}>
        {(point, index) => {
          const previousPoint = () => props.data.previous[index()];
          const currentHeight = () => (point.value / maxValue()) * chartHeight();
          const previousHeight = () => (previousPoint()?.value / maxValue()) * chartHeight();

          return (
            <div class="flex-1 flex flex-col items-center gap-1">
              <div class="flex items-end gap-0.5 flex-1">
                <div
                  class={cn(
                    'flex-1 rounded-t-sm transition-all duration-500',
                    'bg-comparison-previous opacity-50'
                  )}
                  style={{ height: `${previousHeight()}px`, 'min-width': '8px' }}
                />
                <div
                  class={cn(
                    'flex-1 rounded-t-sm transition-all duration-500',
                    'bg-comparison-current'
                  )}
                  style={{ height: `${currentHeight()}px`, 'min-width': '8px' }}
                />
              </div>
              <Show when={point.label}>
                <span class="text-2xs font-mono text-nebula-600 truncate max-w-full">
                  {point.label}
                </span>
              </Show>
            </div>
          );
        }}
      </For>
    </div>
  );
};

const DualAxisChart: Component<{
  primaryData: ComparisonDataSet;
  secondaryData: ComparisonDataSet;
  height: number;
  primaryPrefix?: string;
  primaryUnit?: string;
  secondaryPrefix?: string;
  secondaryUnit?: string;
}> = (props) => {
  const chartHeight = () => props.height - 40;
  const chartWidth = 400;

  const primaryMax = createMemo(() => 
    Math.max(...props.primaryData.current.map(d => d.value), ...props.primaryData.previous.map(d => d.value), 1)
  );
  
  const secondaryMax = createMemo(() =>
    Math.max(...props.secondaryData.current.map(d => d.value), ...props.secondaryData.previous.map(d => d.value), 1)
  );

  const createPath = (data: DataPoint[], max: number): string => {
    if (data.length < 2) return '';
    const padding = 20;
    return data.map((point, i) => {
      const x = padding + (i / (data.length - 1)) * (chartWidth - padding * 2);
      const y = padding + (1 - point.value / max) * (chartHeight() - padding * 2);
      return `${i === 0 ? 'M' : 'L'} ${x},${y}`;
    }).join(' ');
  };

  return (
    <div style={{ height: `${props.height}px` }}>
      <svg width="100%" height={chartHeight()} viewBox={`0 0 ${chartWidth} ${chartHeight()}`} preserveAspectRatio="none">
        <defs>
          <linearGradient id="comparison-primary-gradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="var(--comparison-current)" stop-opacity="0.3" />
            <stop offset="100%" stop-color="var(--comparison-current)" stop-opacity="0" />
          </linearGradient>
          <linearGradient id="comparison-secondary-gradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="var(--color-photon-500)" stop-opacity="0.3" />
            <stop offset="100%" stop-color="var(--color-photon-500)" stop-opacity="0" />
          </linearGradient>
        </defs>

        <path
          d={createPath(props.primaryData.previous, primaryMax())}
          fill="none"
          stroke="var(--comparison-baseline)"
          stroke-width="2"
          stroke-dasharray="4 4"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="opacity-50"
        />
        <path
          d={createPath(props.primaryData.current, primaryMax())}
          fill="none"
          stroke="var(--comparison-current)"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />

        <path
          d={createPath(props.secondaryData.previous, secondaryMax())}
          fill="none"
          stroke="var(--color-photon-400)"
          stroke-width="2"
          stroke-dasharray="4 4"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="opacity-50"
        />
        <path
          d={createPath(props.secondaryData.current, secondaryMax())}
          fill="none"
          stroke="var(--color-photon-500)"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>

      <div class="flex items-center justify-between mt-2 px-2">
        <div class="flex items-center gap-4 text-2xs">
          <div class="flex items-center gap-2">
            <div class="w-4 h-0.5 rounded bg-comparison-current" />
            <span class="text-nebula-400">Primary (Current)</span>
          </div>
          <div class="flex items-center gap-2">
            <div class="w-4 h-0.5 rounded bg-comparison-baseline opacity-50" style={{ 'border-style': 'dashed', 'border-width': '1px 0 0 0', 'border-color': 'var(--comparison-baseline)' }} />
            <span class="text-nebula-500">Primary (Previous)</span>
          </div>
        </div>
        <div class="flex items-center gap-4 text-2xs">
          <div class="flex items-center gap-2">
            <div class="w-4 h-0.5 rounded bg-photon-500" />
            <span class="text-nebula-400">Secondary (Current)</span>
          </div>
          <div class="flex items-center gap-2">
            <div class="w-4 h-0.5 rounded bg-photon-400 opacity-50" />
            <span class="text-nebula-500">Secondary (Previous)</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export const ComparisonChart: Component<ComparisonChartProps> = (props) => {
  const height = () => props.height || 200;

  const currentTotal = createMemo(() => 
    props.data.current.reduce((sum, d) => sum + d.value, 0)
  );
  
  const previousTotal = createMemo(() =>
    props.data.previous.reduce((sum, d) => sum + d.value, 0)
  );

  const percentageChange = createMemo(() => {
    if (previousTotal() === 0) return currentTotal() > 0 ? 100 : 0;
    return ((currentTotal() - previousTotal()) / previousTotal()) * 100;
  });

  const isPositive = () => percentageChange() > 0;
  const isNeutral = () => percentageChange() === 0;

  return (
    <div
      class={cn(
        'rounded-3xl border border-white/5 bg-void-850',
        'transition-all duration-300',
        'hover:border-white/10',
        props.class
      )}
    >
      <div class="p-6">
        <div class="flex items-start justify-between mb-6">
          <div>
            <h3 class="font-display font-bold text-white text-lg">
              {props.title}
            </h3>
            <div class="flex items-center gap-2 mt-2">
              <Calendar size={14} class="text-nebula-500" />
              <span class="text-sm text-nebula-400">
                {PERIOD_LABELS[props.period].previous}
              </span>
              <ArrowRight size={12} class="text-nebula-600" />
              <span class="text-sm text-nebula-300">
                {PERIOD_LABELS[props.period].current}
              </span>
            </div>
          </div>
          <Show when={props.onPeriodChange}>
            <PeriodSelector
              value={props.period}
              onChange={props.onPeriodChange!}
            />
          </Show>
        </div>

        <div class="grid grid-cols-3 gap-6 mb-6">
          <div>
            <span class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              {PERIOD_LABELS[props.period].current}
            </span>
            <div class="mt-1">
              <span class="font-mono text-2xl font-black text-white tabular-nums">
                {formatValue(currentTotal(), props.prefix, props.unit)}
              </span>
            </div>
            <Show when={props.showSparkline}>
              <div class="mt-2">
                <Sparkline
                  data={props.data.current.map(d => d.value)}
                  color="var(--comparison-current)"
                  width={80}
                  height={20}
                />
              </div>
            </Show>
          </div>

          <div>
            <span class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              {PERIOD_LABELS[props.period].previous}
            </span>
            <div class="mt-1">
              <span class="font-mono text-2xl font-black text-nebula-400 tabular-nums">
                {formatValue(previousTotal(), props.prefix, props.unit)}
              </span>
            </div>
            <Show when={props.showSparkline}>
              <div class="mt-2">
                <Sparkline
                  data={props.data.previous.map(d => d.value)}
                  color="var(--comparison-baseline)"
                  dashed
                  width={80}
                  height={20}
                />
              </div>
            </Show>
          </div>

          <div>
            <span class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              Change
            </span>
            <div class="mt-1 flex items-center gap-2">
              <span
                class={cn(
                  'font-mono text-2xl font-black tabular-nums',
                  isPositive() && 'text-comparison-delta-positive',
                  !isPositive() && !isNeutral() && 'text-comparison-delta-negative',
                  isNeutral() && 'text-nebula-400'
                )}
              >
                {isPositive() ? '+' : ''}{percentageChange().toFixed(1)}%
              </span>
              <Show when={isPositive()}>
                <TrendingUp size={20} class="text-comparison-delta-positive" />
              </Show>
              <Show when={!isPositive() && !isNeutral()}>
                <TrendingDown size={20} class="text-comparison-delta-negative" />
              </Show>
              <Show when={isNeutral()}>
                <Minus size={20} class="text-nebula-500" />
              </Show>
            </div>
            <div class="mt-1 text-sm text-nebula-500">
              {isPositive() ? '+' : ''}{formatValue(currentTotal() - previousTotal(), props.prefix, props.unit)}
            </div>
          </div>
        </div>

        <Show 
          when={props.dualAxis && props.secondaryData}
          fallback={
            <ComparisonBars
              data={props.data}
              height={height()}
              prefix={props.prefix}
              unit={props.unit}
            />
          }
        >
          <DualAxisChart
            primaryData={props.data}
            secondaryData={props.secondaryData!}
            height={height()}
            primaryPrefix={props.prefix}
            primaryUnit={props.unit}
            secondaryPrefix={props.secondaryPrefix}
            secondaryUnit={props.secondaryUnit}
          />
        </Show>
      </div>

      <div class="px-6 py-4 border-t border-white/5 bg-void-900/50 rounded-b-3xl">
        <div class="flex items-center justify-between text-sm">
          <div class="flex items-center gap-4">
            <div class="flex items-center gap-2">
              <div class="w-3 h-3 rounded-sm bg-comparison-current" />
              <span class="text-nebula-400">{PERIOD_LABELS[props.period].current}</span>
            </div>
            <div class="flex items-center gap-2">
              <div class="w-3 h-3 rounded-sm bg-comparison-previous opacity-50" />
              <span class="text-nebula-500">{PERIOD_LABELS[props.period].previous}</span>
            </div>
          </div>
          <span class="text-nebula-600 font-mono text-xs tabular-nums">
            {props.data.current.length} data points
          </span>
        </div>
      </div>
    </div>
  );
};

export default ComparisonChart;
