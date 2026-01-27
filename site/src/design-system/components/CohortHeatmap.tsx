import { Component, For, Show, createMemo, createSignal, splitProps } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { Calendar, TrendingUp, TrendingDown, Users } from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface CohortData {
  cohortId: string;
  cohortLabel: string;
  cohortSize: number;
  periods: {
    periodIndex: number;
    retained: number;
    retentionRate: number;
  }[];
}

interface CohortHeatmapProps {
  data: CohortData[];
  periodLabels?: string[];
  periodType?: 'day' | 'week' | 'month';
  showCohortSize?: boolean;
  showPercentages?: boolean;
  showTooltips?: boolean;
  highlightDiagonal?: boolean;
  colorScale?: 'retention' | 'churn';
  cellSize?: 'sm' | 'md' | 'lg';
  animated?: boolean;
  onCellClick?: (cohortId: string, periodIndex: number, data: { retained: number; retentionRate: number }) => void;
  class?: string;
}

type RetentionLevel = 0 | 10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90 | 100;

const retentionColors: Record<RetentionLevel, string> = {
  0: 'bg-[rgba(239,68,68,0.15)]',
  10: 'bg-[rgba(239,68,68,0.25)]',
  20: 'bg-[rgba(249,115,22,0.3)]',
  30: 'bg-[rgba(245,158,11,0.35)]',
  40: 'bg-[rgba(245,158,11,0.45)]',
  50: 'bg-[rgba(132,204,22,0.4)]',
  60: 'bg-[rgba(34,211,211,0.45)]',
  70: 'bg-[rgba(34,211,211,0.55)]',
  80: 'bg-[rgba(16,185,129,0.6)]',
  90: 'bg-[rgba(16,185,129,0.75)]',
  100: 'bg-[rgba(5,150,105,0.85)]',
};

const churnColors: Record<RetentionLevel, string> = {
  0: 'bg-[rgba(5,150,105,0.85)]',
  10: 'bg-[rgba(16,185,129,0.75)]',
  20: 'bg-[rgba(16,185,129,0.6)]',
  30: 'bg-[rgba(34,211,211,0.55)]',
  40: 'bg-[rgba(34,211,211,0.45)]',
  50: 'bg-[rgba(132,204,22,0.4)]',
  60: 'bg-[rgba(245,158,11,0.45)]',
  70: 'bg-[rgba(245,158,11,0.35)]',
  80: 'bg-[rgba(249,115,22,0.3)]',
  90: 'bg-[rgba(239,68,68,0.25)]',
  100: 'bg-[rgba(239,68,68,0.15)]',
};

const cellSizes = {
  sm: { cell: 'h-6 w-10 text-2xs', header: 'text-2xs', label: 'w-20 text-2xs' },
  md: { cell: 'h-8 w-14 text-xs', header: 'text-xs', label: 'w-28 text-xs' },
  lg: { cell: 'h-10 w-18 text-sm', header: 'text-sm', label: 'w-36 text-sm' },
};

const getRetentionLevel = (rate: number): RetentionLevel => {
  if (rate >= 100) return 100;
  if (rate >= 90) return 90;
  if (rate >= 80) return 80;
  if (rate >= 70) return 70;
  if (rate >= 60) return 60;
  if (rate >= 50) return 50;
  if (rate >= 40) return 40;
  if (rate >= 30) return 30;
  if (rate >= 20) return 20;
  if (rate >= 10) return 10;
  return 0;
};

interface TooltipData {
  cohortLabel: string;
  periodLabel: string;
  retained: number;
  cohortSize: number;
  retentionRate: number;
  x: number;
  y: number;
}

const CohortTooltip: Component<{ data: TooltipData | null }> = (props) => {
  return (
    <Show when={props.data}>
      {(data) => (
        <div
          class="pointer-events-none fixed z-50 rounded-xl border border-white/10 bg-void-900/95 p-3 shadow-xl backdrop-blur-md transition-all duration-150"
          style={{
            left: `${data().x + 10}px`,
            top: `${data().y - 60}px`,
          }}
        >
          <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
            {data().cohortLabel} → {data().periodLabel}
          </div>
          <div class="mt-2 flex items-baseline gap-2">
            <span class="font-display text-xl font-black tabular-nums text-white">
              {data().retentionRate.toFixed(1)}%
            </span>
            <span class="text-xs text-nebula-400">
              ({data().retained.toLocaleString()} / {data().cohortSize.toLocaleString()})
            </span>
          </div>
        </div>
      )}
    </Show>
  );
};

export const CohortHeatmap: Component<CohortHeatmapProps> = (props) => {
  const [local, others] = splitProps(props, [
    'data', 'periodLabels', 'periodType', 'showCohortSize', 'showPercentages',
    'showTooltips', 'highlightDiagonal', 'colorScale', 'cellSize', 'animated',
    'onCellClick', 'class'
  ]);

  const [tooltip, setTooltip] = createSignal<TooltipData | null>(null);
  const [selectedCell, setSelectedCell] = createSignal<string | null>(null);

  const size = () => cellSizes[local.cellSize || 'md'];
  const colors = () => local.colorScale === 'churn' ? churnColors : retentionColors;
  const showTooltips = () => local.showTooltips !== false;

  const maxPeriods = createMemo(() => {
    return Math.max(...local.data.map(c => c.periods.length));
  });

  const periodLabels = createMemo(() => {
    if (local.periodLabels) return local.periodLabels;
    const prefix = local.periodType === 'day' ? 'D' : local.periodType === 'week' ? 'W' : 'M';
    return Array.from({ length: maxPeriods() }, (_, i) => `${prefix}${i}`);
  });

  const averageRetention = createMemo(() => {
    const totals: number[] = [];
    for (let i = 0; i < maxPeriods(); i++) {
      const rates = local.data
        .filter(c => c.periods[i])
        .map(c => c.periods[i].retentionRate);
      if (rates.length > 0) {
        totals.push(rates.reduce((a, b) => a + b, 0) / rates.length);
      }
    }
    return totals;
  });

  const handleMouseEnter = (
    e: MouseEvent,
    cohort: CohortData,
    period: { periodIndex: number; retained: number; retentionRate: number }
  ) => {
    if (!showTooltips()) return;
    setTooltip({
      cohortLabel: cohort.cohortLabel,
      periodLabel: periodLabels()[period.periodIndex] || `Period ${period.periodIndex}`,
      retained: period.retained,
      cohortSize: cohort.cohortSize,
      retentionRate: period.retentionRate,
      x: e.clientX,
      y: e.clientY,
    });
  };

  const handleMouseLeave = () => {
    setTooltip(null);
  };

  const handleCellClick = (
    cohort: CohortData,
    period: { periodIndex: number; retained: number; retentionRate: number }
  ) => {
    const cellKey = `${cohort.cohortId}-${period.periodIndex}`;
    setSelectedCell(prev => prev === cellKey ? null : cellKey);
    local.onCellClick?.(cohort.cohortId, period.periodIndex, {
      retained: period.retained,
      retentionRate: period.retentionRate,
    });
  };

  return (
    <div class={cn('relative', local.class)} {...others}>
      <CohortTooltip data={tooltip()} />

      <div class="overflow-x-auto">
        <table class="border-separate border-spacing-1">
          <thead>
            <tr>
              <th class={cn('text-left font-bold uppercase tracking-widest text-nebula-500', size().label)}>
                Cohort
              </th>
              <Show when={local.showCohortSize}>
                <th class={cn('text-center font-bold uppercase tracking-widest text-nebula-500 px-2', size().header)}>
                  <Users size={12} class="inline-block" />
                </th>
              </Show>
              <For each={periodLabels()}>
                {(label) => (
                  <th class={cn('text-center font-bold uppercase tracking-widest text-nebula-500', size().header)}>
                    {label}
                  </th>
                )}
              </For>
            </tr>
          </thead>
          <tbody>
            <For each={local.data}>
              {(cohort, cohortIndex) => (
                <tr>
                  <td class={cn('font-medium text-nebula-300 pr-3', size().label)}>
                    {cohort.cohortLabel}
                  </td>
                  <Show when={local.showCohortSize}>
                    <td class={cn('text-center font-mono text-nebula-500 tabular-nums px-2', size().header)}>
                      {cohort.cohortSize.toLocaleString()}
                    </td>
                  </Show>
                  <For each={Array.from({ length: maxPeriods() })}>
                    {(_, periodIndex) => {
                      const period = cohort.periods.find(p => p.periodIndex === periodIndex());
                      const isDiagonal = periodIndex() === cohortIndex();
                      const cellKey = `${cohort.cohortId}-${periodIndex()}`;
                      const isSelected = selectedCell() === cellKey;

                      if (!period) {
                        return (
                          <td>
                            <div class={cn('rounded', size().cell, 'bg-void-800/50')} />
                          </td>
                        );
                      }

                      const level = getRetentionLevel(period.retentionRate);
                      const colorClass = colors()[level];

                      return (
                        <td>
                          <div
                            class={cn(
                              'flex items-center justify-center rounded font-bold tabular-nums cursor-pointer',
                              'transition-all duration-200',
                              size().cell,
                              colorClass,
                              isDiagonal && local.highlightDiagonal && 'ring-2 ring-indigo-500/50',
                              isSelected && 'ring-2 ring-indigo-400 shadow-[0_0_15px_rgba(99,102,241,0.3)]',
                              'hover:scale-105 hover:shadow-lg hover:z-10',
                              local.animated && 'animate-fade-up',
                              period.retentionRate >= 70 ? 'text-white' : 'text-nebula-200'
                            )}
                            style={local.animated ? { 'animation-delay': `${(cohortIndex() * maxPeriods() + periodIndex()) * 20}ms` } : {}}
                            onMouseEnter={(e) => handleMouseEnter(e, cohort, period)}
                            onMouseLeave={handleMouseLeave}
                            onClick={() => handleCellClick(cohort, period)}
                            role="button"
                            tabIndex={0}
                            aria-label={`${cohort.cohortLabel} ${periodLabels()[periodIndex()]}: ${period.retentionRate.toFixed(1)}% retention`}
                          >
                            <Show when={local.showPercentages} fallback={period.retentionRate >= 1 ? Math.round(period.retentionRate) : ''}>
                              {period.retentionRate.toFixed(0)}%
                            </Show>
                          </div>
                        </td>
                      );
                    }}
                  </For>
                </tr>
              )}
            </For>
          </tbody>
          <tfoot>
            <tr class="border-t border-white/5">
              <td class={cn('font-bold uppercase tracking-widest text-nebula-500 pt-2', size().label)}>
                Average
              </td>
              <Show when={local.showCohortSize}>
                <td />
              </Show>
              <For each={averageRetention()}>
                {(avg, i) => {
                  const trend = i() > 0 ? avg - averageRetention()[i() - 1] : 0;
                  return (
                    <td class="pt-2">
                      <div class={cn('flex flex-col items-center gap-0.5', size().header)}>
                        <span class="font-bold text-nebula-300 tabular-nums">
                          {avg.toFixed(1)}%
                        </span>
                        <Show when={i() > 0}>
                          <span class={cn(
                            'flex items-center gap-0.5 text-2xs',
                            trend >= 0 ? 'text-aurora-400' : 'text-flare-400'
                          )}>
                            {trend >= 0 ? <TrendingUp size={10} /> : <TrendingDown size={10} />}
                            {Math.abs(trend).toFixed(1)}
                          </span>
                        </Show>
                      </div>
                    </td>
                  );
                }}
              </For>
            </tr>
          </tfoot>
        </table>
      </div>

      <div class="mt-4 flex items-center justify-between">
        <div class="flex items-center gap-2 text-2xs text-nebula-500">
          <Calendar size={12} />
          <span class="font-medium">
            {local.data.length} cohorts, {maxPeriods()} periods
          </span>
        </div>

        <div class="flex items-center gap-2 text-2xs text-nebula-500">
          <span>0%</span>
          <div class="flex gap-0.5">
            <For each={[0, 20, 40, 60, 80, 100] as RetentionLevel[]}>
              {(level) => (
                <div
                  class={cn('h-3 w-6 rounded-sm', colors()[level])}
                />
              )}
            </For>
          </div>
          <span>100%</span>
        </div>
      </div>
    </div>
  );
};

export interface CohortSummaryProps {
  data: CohortData[];
  class?: string;
}

export const CohortSummary: Component<CohortSummaryProps> = (props) => {
  const stats = createMemo(() => {
    const allRates = props.data.flatMap(c => c.periods.map(p => p.retentionRate));
    const totalUsers = props.data.reduce((sum, c) => sum + c.cohortSize, 0);
    const avgRetention = allRates.length > 0
      ? allRates.reduce((a, b) => a + b, 0) / allRates.length
      : 0;

    const latestCohort = props.data[props.data.length - 1];
    const latestFirstMonth = latestCohort?.periods[0]?.retentionRate ?? 0;

    return { totalUsers, avgRetention, latestFirstMonth, cohortCount: props.data.length };
  });

  return (
    <div class={cn('grid grid-cols-4 gap-4', props.class)}>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Total Cohorts
        </div>
        <div class="mt-2 font-display text-2xl font-black text-white tabular-nums">
          {stats().cohortCount}
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Total Users
        </div>
        <div class="mt-2 font-display text-2xl font-black text-white tabular-nums">
          {stats().totalUsers.toLocaleString()}
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Avg Retention
        </div>
        <div class="mt-2 font-display text-2xl font-black text-aurora-400 tabular-nums">
          {stats().avgRetention.toFixed(1)}%
        </div>
      </div>
      <div class="rounded-xl border border-white/5 bg-void-850 p-4">
        <div class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
          Latest M0
        </div>
        <div class="mt-2 font-display text-2xl font-black text-electric-400 tabular-nums">
          {stats().latestFirstMonth.toFixed(1)}%
        </div>
      </div>
    </div>
  );
};

export default CohortHeatmap;
