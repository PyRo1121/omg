import { Component, For, Show, createSignal, createMemo } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { 
  TrendingUp, 
  TrendingDown, 
  Minus,
  LineChart,
  Info,
  ChevronDown
} from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type ForecastScenario = 'conservative' | 'moderate' | 'aggressive';

export interface ForecastDataPoint {
  timestamp: string;
  value: number;
  label?: string;
}

export interface ForecastConfidenceInterval {
  upper: number;
  lower: number;
}

export interface ForecastProjection {
  point: ForecastDataPoint;
  confidence: ForecastConfidenceInterval;
}

export interface ForecastAssumption {
  label: string;
  value: string;
  impact: 'positive' | 'negative' | 'neutral';
}

export interface ForecastData {
  historical: ForecastDataPoint[];
  projections: Record<ForecastScenario, ForecastProjection[]>;
  assumptions: Record<ForecastScenario, ForecastAssumption[]>;
}

export interface ForecastWidgetProps {
  title: string;
  data: ForecastData;
  defaultScenario?: ForecastScenario;
  unit?: string;
  prefix?: string;
  height?: number;
  showConfidenceBands?: boolean;
  showAssumptions?: boolean;
  onScenarioChange?: (scenario: ForecastScenario) => void;
  class?: string;
}

const SCENARIO_CONFIG: Record<ForecastScenario, {
  label: string;
  color: string;
  bgColor: string;
  description: string;
}> = {
  conservative: {
    label: 'Conservative',
    color: 'var(--color-plasma-500)',
    bgColor: 'rgba(59, 125, 209, 0.1)',
    description: 'Lower growth assumptions, higher churn risk',
  },
  moderate: {
    label: 'Moderate',
    color: 'var(--color-indigo-500)',
    bgColor: 'rgba(99, 102, 241, 0.1)',
    description: 'Baseline forecast based on current trends',
  },
  aggressive: {
    label: 'Aggressive',
    color: 'var(--color-aurora-500)',
    bgColor: 'rgba(16, 185, 129, 0.1)',
    description: 'Higher growth assumptions, optimistic outlook',
  },
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

const ScenarioSelector: Component<{
  value: ForecastScenario;
  onChange: (scenario: ForecastScenario) => void;
}> = (props) => {
  const scenarios: ForecastScenario[] = ['conservative', 'moderate', 'aggressive'];

  return (
    <div class="flex items-center gap-1 p-1 rounded-xl bg-void-800 border border-white/5">
      <For each={scenarios}>
        {(scenario) => (
          <button
            type="button"
            class={cn(
              'px-3 py-1.5 rounded-lg text-xs font-bold uppercase tracking-widest',
              'transition-all duration-300',
              props.value === scenario
                ? 'text-white'
                : 'text-nebula-500 hover:text-nebula-300 hover:bg-white/5'
            )}
            style={props.value === scenario ? {
              'background-color': SCENARIO_CONFIG[scenario].bgColor,
              'color': SCENARIO_CONFIG[scenario].color,
            } : {}}
            onClick={() => props.onChange(scenario)}
          >
            {SCENARIO_CONFIG[scenario].label}
          </button>
        )}
      </For>
    </div>
  );
};

const ForecastChart: Component<{
  historical: ForecastDataPoint[];
  projections: ForecastProjection[];
  scenario: ForecastScenario;
  height: number;
  showConfidenceBands: boolean;
}> = (props) => {
  const chartWidth = 600;
  const chartHeight = () => props.height - 40;
  const padding = { top: 20, right: 20, bottom: 30, left: 20 };

  const allValues = createMemo(() => {
    const histValues = props.historical.map(d => d.value);
    const projValues = props.projections.map(p => p.point.value);
    const upperBounds = props.projections.map(p => p.confidence.upper);
    const lowerBounds = props.projections.map(p => p.confidence.lower);
    return [...histValues, ...projValues, ...upperBounds, ...lowerBounds];
  });

  const maxValue = createMemo(() => Math.max(...allValues(), 1) * 1.1);
  const minValue = createMemo(() => Math.max(Math.min(...allValues()) * 0.9, 0));

  const totalPoints = createMemo(() => props.historical.length + props.projections.length);

  const getX = (index: number) => {
    const usableWidth = chartWidth - padding.left - padding.right;
    return padding.left + (index / (totalPoints() - 1)) * usableWidth;
  };

  const getY = (value: number) => {
    const usableHeight = chartHeight() - padding.top - padding.bottom;
    return padding.top + (1 - (value - minValue()) / (maxValue() - minValue())) * usableHeight;
  };

  const historicalPath = createMemo(() => {
    return props.historical.map((point, i) => {
      const x = getX(i);
      const y = getY(point.value);
      return `${i === 0 ? 'M' : 'L'} ${x},${y}`;
    }).join(' ');
  });

  const projectedPath = createMemo(() => {
    const startIndex = props.historical.length - 1;
    const lastHistorical = props.historical[startIndex];
    let path = `M ${getX(startIndex)},${getY(lastHistorical.value)}`;
    
    props.projections.forEach((proj, i) => {
      const x = getX(startIndex + i + 1);
      const y = getY(proj.point.value);
      path += ` L ${x},${y}`;
    });
    
    return path;
  });

  const confidenceAreaPath = createMemo(() => {
    if (!props.showConfidenceBands) return '';
    
    const startIndex = props.historical.length - 1;
    const lastHistorical = props.historical[startIndex];
    
    let upperPath = `M ${getX(startIndex)},${getY(lastHistorical.value)}`;
    const lowerPoints: string[] = [];
    
    props.projections.forEach((proj, i) => {
      const x = getX(startIndex + i + 1);
      upperPath += ` L ${x},${getY(proj.confidence.upper)}`;
      lowerPoints.unshift(`L ${x},${getY(proj.confidence.lower)}`);
    });
    
    const lastX = getX(startIndex + props.projections.length);
    upperPath += ` L ${lastX},${getY(props.projections[props.projections.length - 1].confidence.lower)}`;
    
    return upperPath + ' ' + lowerPoints.join(' ') + ' Z';
  });

  const scenarioConfig = () => SCENARIO_CONFIG[props.scenario];

  return (
    <div style={{ height: `${props.height}px` }}>
      <svg width="100%" height={chartHeight()} viewBox={`0 0 ${chartWidth} ${chartHeight()}`} preserveAspectRatio="xMidYMid meet">
        <defs>
          <linearGradient id="forecast-historical-gradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="var(--forecast-historical)" stop-opacity="0.2" />
            <stop offset="100%" stop-color="var(--forecast-historical)" stop-opacity="0" />
          </linearGradient>
          <linearGradient id={`forecast-projected-gradient-${props.scenario}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color={scenarioConfig().color} stop-opacity="0.15" />
            <stop offset="100%" stop-color={scenarioConfig().color} stop-opacity="0" />
          </linearGradient>
        </defs>

        <line
          x1={getX(props.historical.length - 1)}
          y1={padding.top}
          x2={getX(props.historical.length - 1)}
          y2={chartHeight() - padding.bottom}
          stroke="var(--border-subtle)"
          stroke-dasharray="4 4"
        />
        <text
          x={getX(props.historical.length - 1)}
          y={chartHeight() - 8}
          text-anchor="middle"
          class="fill-nebula-500 text-2xs font-mono"
        >
          Today
        </text>

        <Show when={props.showConfidenceBands}>
          <path
            d={confidenceAreaPath()}
            fill={`url(#forecast-projected-gradient-${props.scenario})`}
            class="transition-all duration-500"
          />
        </Show>

        <path
          d={historicalPath()}
          fill="none"
          stroke="var(--forecast-historical)"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />

        <path
          d={projectedPath()}
          fill="none"
          stroke={scenarioConfig().color}
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-dasharray="6 4"
          class="transition-all duration-500"
        />

        <For each={props.historical}>
          {(point, index) => (
            <circle
              cx={getX(index())}
              cy={getY(point.value)}
              r="3"
              fill="var(--color-void-850)"
              stroke="var(--forecast-historical)"
              stroke-width="2"
              class="opacity-0 hover:opacity-100 transition-opacity cursor-pointer"
            />
          )}
        </For>

        <For each={props.projections}>
          {(proj, index) => (
            <>
              <Show when={props.showConfidenceBands}>
                <line
                  x1={getX(props.historical.length + index())}
                  y1={getY(proj.confidence.upper)}
                  x2={getX(props.historical.length + index())}
                  y2={getY(proj.confidence.lower)}
                  stroke={scenarioConfig().color}
                  stroke-width="1"
                  stroke-opacity="0.3"
                />
              </Show>
              <circle
                cx={getX(props.historical.length + index())}
                cy={getY(proj.point.value)}
                r="4"
                fill="var(--color-void-850)"
                stroke={scenarioConfig().color}
                stroke-width="2"
                class="transition-all duration-500"
              />
            </>
          )}
        </For>

        <circle
          cx={getX(props.historical.length + props.projections.length - 1)}
          cy={getY(props.projections[props.projections.length - 1].point.value)}
          r="6"
          fill={scenarioConfig().color}
          class="animate-pulse transition-all duration-500"
        />
      </svg>

      <div class="flex items-center justify-center gap-6 mt-2 text-2xs">
        <div class="flex items-center gap-2">
          <div class="w-4 h-0.5 rounded bg-forecast-historical" />
          <span class="text-nebula-400">Historical</span>
        </div>
        <div class="flex items-center gap-2">
          <div 
            class="w-4 h-0.5 rounded" 
            style={{ 
              'background-color': scenarioConfig().color,
              'background': `repeating-linear-gradient(90deg, ${scenarioConfig().color} 0px, ${scenarioConfig().color} 4px, transparent 4px, transparent 7px)`
            }} 
          />
          <span class="text-nebula-400">Projected</span>
        </div>
        <Show when={props.showConfidenceBands}>
          <div class="flex items-center gap-2">
            <div 
              class="w-4 h-3 rounded opacity-30" 
              style={{ 'background-color': scenarioConfig().color }} 
            />
            <span class="text-nebula-500">Confidence Band</span>
          </div>
        </Show>
      </div>
    </div>
  );
};

const AssumptionsList: Component<{
  assumptions: ForecastAssumption[];
  scenario: ForecastScenario;
}> = (props) => {
  const [isExpanded, setIsExpanded] = createSignal(false);

  return (
    <div class="border-t border-white/5">
      <button
        type="button"
        class={cn(
          'w-full px-6 py-4 flex items-center justify-between',
          'text-sm text-nebula-400',
          'transition-all duration-300',
          'hover:bg-white/[0.02] hover:text-nebula-300'
        )}
        onClick={() => setIsExpanded(!isExpanded())}
      >
        <span class="flex items-center gap-2">
          <Info size={14} />
          <span>Key Assumptions ({props.assumptions.length})</span>
        </span>
        <ChevronDown 
          size={14} 
          class={cn('transition-transform duration-300', isExpanded() && 'rotate-180')} 
        />
      </button>

      <Show when={isExpanded()}>
        <div class="px-6 pb-4 space-y-3">
          <For each={props.assumptions}>
            {(assumption) => (
              <div class="flex items-center justify-between p-3 rounded-xl bg-void-900/50">
                <div class="flex items-center gap-3">
                  <div
                    class={cn(
                      'w-1.5 h-1.5 rounded-full',
                      assumption.impact === 'positive' && 'bg-aurora-500',
                      assumption.impact === 'negative' && 'bg-flare-500',
                      assumption.impact === 'neutral' && 'bg-nebula-500'
                    )}
                  />
                  <span class="text-sm text-nebula-300">{assumption.label}</span>
                </div>
                <span 
                  class={cn(
                    'text-sm font-mono font-bold tabular-nums',
                    assumption.impact === 'positive' && 'text-aurora-400',
                    assumption.impact === 'negative' && 'text-flare-400',
                    assumption.impact === 'neutral' && 'text-nebula-400'
                  )}
                >
                  {assumption.value}
                </span>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

export const ForecastWidget: Component<ForecastWidgetProps> = (props) => {
  const [scenario, setScenario] = createSignal<ForecastScenario>(props.defaultScenario || 'moderate');
  const height = () => props.height || 200;
  const showConfidenceBands = () => props.showConfidenceBands !== false;
  const showAssumptions = () => props.showAssumptions !== false;

  const handleScenarioChange = (newScenario: ForecastScenario) => {
    setScenario(newScenario);
    props.onScenarioChange?.(newScenario);
  };

  const currentProjections = () => props.data.projections[scenario()];
  const currentAssumptions = () => props.data.assumptions[scenario()];

  const lastHistorical = () => props.data.historical[props.data.historical.length - 1];
  const lastProjection = () => currentProjections()[currentProjections().length - 1];

  const projectedChange = createMemo(() => {
    const start = lastHistorical().value;
    const end = lastProjection().point.value;
    if (start === 0) return end > 0 ? 100 : 0;
    return ((end - start) / start) * 100;
  });

  const isPositive = () => projectedChange() > 0;
  const isNeutral = () => projectedChange() === 0;

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
          <div class="flex items-start gap-3">
            <div class="p-2.5 rounded-xl bg-forecast-projected/10">
              <LineChart size={20} class="text-forecast-projected" />
            </div>
            <div>
              <h3 class="font-display font-bold text-white text-lg">
                {props.title}
              </h3>
              <p class="text-sm text-nebula-500 mt-1">
                {SCENARIO_CONFIG[scenario()].description}
              </p>
            </div>
          </div>
          <ScenarioSelector
            value={scenario()}
            onChange={handleScenarioChange}
          />
        </div>

        <div class="grid grid-cols-3 gap-6 mb-6">
          <div>
            <span class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              Current
            </span>
            <div class="mt-1">
              <span class="font-mono text-2xl font-black text-white tabular-nums">
                {formatValue(lastHistorical().value, props.prefix, props.unit)}
              </span>
            </div>
          </div>

          <div>
            <span class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              Projected
            </span>
            <div class="mt-1">
              <span 
                class="font-mono text-2xl font-black tabular-nums transition-colors duration-500"
                style={{ color: SCENARIO_CONFIG[scenario()].color }}
              >
                {formatValue(lastProjection().point.value, props.prefix, props.unit)}
              </span>
            </div>
            <Show when={showConfidenceBands()}>
              <div class="text-2xs text-nebula-500 font-mono mt-1 tabular-nums">
                {formatValue(lastProjection().confidence.lower, props.prefix, props.unit)} – {formatValue(lastProjection().confidence.upper, props.prefix, props.unit)}
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
                  isPositive() && 'text-aurora-400',
                  !isPositive() && !isNeutral() && 'text-flare-400',
                  isNeutral() && 'text-nebula-400'
                )}
              >
                {isPositive() ? '+' : ''}{projectedChange().toFixed(1)}%
              </span>
              <Show when={isPositive()}>
                <TrendingUp size={20} class="text-aurora-400" />
              </Show>
              <Show when={!isPositive() && !isNeutral()}>
                <TrendingDown size={20} class="text-flare-400" />
              </Show>
              <Show when={isNeutral()}>
                <Minus size={20} class="text-nebula-500" />
              </Show>
            </div>
          </div>
        </div>

        <ForecastChart
          historical={props.data.historical}
          projections={currentProjections()}
          scenario={scenario()}
          height={height()}
          showConfidenceBands={showConfidenceBands()}
        />
      </div>

      <Show when={showAssumptions() && currentAssumptions().length > 0}>
        <AssumptionsList
          assumptions={currentAssumptions()}
          scenario={scenario()}
        />
      </Show>
    </div>
  );
};

export default ForecastWidget;
