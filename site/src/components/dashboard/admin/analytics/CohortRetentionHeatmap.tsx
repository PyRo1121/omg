import { Component, For, Show, createMemo, createSignal, onMount } from 'solid-js';
import { Calendar, Users, TrendingUp, Info, Download, Maximize2, Minimize2 } from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface CohortData {
  cohort_month: string;
  month_index: number;
  active_users: number;
  retention_rate?: number;
}

interface CohortRetentionHeatmapProps {
  data: CohortData[];
  maxMonths?: number;
}

const RETENTION_COLORS = [
  { min: 0, bg: 'rgba(239, 68, 68, 0.7)', glow: 'rgba(239, 68, 68, 0.3)', label: 'Critical' },
  { min: 10, bg: 'rgba(245, 158, 11, 0.6)', glow: 'rgba(245, 158, 11, 0.3)', label: 'Poor' },
  { min: 25, bg: 'rgba(251, 191, 36, 0.6)', glow: 'rgba(251, 191, 36, 0.3)', label: 'Fair' },
  { min: 40, bg: 'rgba(34, 197, 94, 0.5)', glow: 'rgba(34, 197, 94, 0.25)', label: 'Good' },
  { min: 60, bg: 'rgba(34, 197, 94, 0.65)', glow: 'rgba(34, 197, 94, 0.3)', label: 'Great' },
  { min: 80, bg: 'rgba(16, 185, 129, 0.8)', glow: 'rgba(16, 185, 129, 0.4)', label: 'Excellent' },
];

function getRetentionColor(rate: number) {
  for (let i = RETENTION_COLORS.length - 1; i >= 0; i--) {
    if (rate >= RETENTION_COLORS[i].min) {
      return RETENTION_COLORS[i];
    }
  }
  return RETENTION_COLORS[0];
}

function formatMonth(monthStr: string): string {
  const date = new Date(monthStr + '-01');
  return date.toLocaleDateString('en-US', { month: 'short', year: '2-digit' });
}

export const CohortRetentionHeatmap: Component<CohortRetentionHeatmapProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [isExpanded, setIsExpanded] = createSignal(false);
  const [hoveredCell, setHoveredCell] = createSignal<{ cohort: string; month: number } | null>(null);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const maxMonths = () => props.maxMonths || 12;

  const cohortMap = createMemo(() => {
    const groupedByMonth = new Map<string, Map<number, CohortData>>();
    
    for (const item of props.data) {
      if (!groupedByMonth.has(item.cohort_month)) {
        groupedByMonth.set(item.cohort_month, new Map());
      }
      groupedByMonth.get(item.cohort_month)!.set(item.month_index, item);
    }

    return Array.from(groupedByMonth.entries())
      .sort((a, b) => b[0].localeCompare(a[0]))
      .slice(0, 12);
  });

  const getRetentionRate = (cohortMonth: string, monthIndex: number) => {
    const cohort = cohortMap().find(([month]) => month === cohortMonth);
    if (!cohort) return null;
    
    const monthData = cohort[1].get(monthIndex);
    const baseData = cohort[1].get(0);
    
    if (!monthData || !baseData || baseData.active_users === 0) return null;
    
    if (monthData.retention_rate !== undefined) {
      return monthData.retention_rate;
    }
    
    return Math.round((monthData.active_users / baseData.active_users) * 100);
  };

  const getActiveUsers = (cohortMonth: string, monthIndex: number) => {
    const cohort = cohortMap().find(([month]) => month === cohortMonth);
    if (!cohort) return null;
    return cohort[1].get(monthIndex)?.active_users ?? null;
  };

  const getBaseUsers = (cohortMonth: string) => {
    const cohort = cohortMap().find(([month]) => month === cohortMonth);
    if (!cohort) return 0;
    return cohort[1].get(0)?.active_users ?? 0;
  };

  const avgRetentionByMonth = createMemo(() => {
    const totals: number[] = [];
    const counts: number[] = [];
    
    for (let i = 0; i <= maxMonths(); i++) {
      totals[i] = 0;
      counts[i] = 0;
    }
    
    for (const [cohortMonth] of cohortMap()) {
      for (let i = 0; i <= maxMonths(); i++) {
        const rate = getRetentionRate(cohortMonth, i);
        if (rate !== null) {
          totals[i] += rate;
          counts[i]++;
        }
      }
    }
    
    return totals.map((total, i) => counts[i] > 0 ? Math.round(total / counts[i]) : null);
  });

  const overallHealth = createMemo(() => {
    const month3Avg = avgRetentionByMonth()[3];
    if (month3Avg === null) return { label: 'N/A', color: 'var(--color-nebula-400)' };
    if (month3Avg >= 60) return { label: 'Excellent', color: 'var(--color-aurora-400)' };
    if (month3Avg >= 40) return { label: 'Good', color: 'var(--color-electric-400)' };
    if (month3Avg >= 25) return { label: 'Fair', color: 'var(--color-solar-400)' };
    return { label: 'Needs Work', color: 'var(--color-flare-400)' };
  });

  return (
    <div
      class={cn(
        'rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl relative overflow-hidden',
        'transition-all duration-300',
        isExpanded() && 'col-span-full'
      )}
    >
      <div
        class="absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl opacity-10 transition-opacity duration-500"
        style={{ background: overallHealth().color }}
      />

      <div class="mb-6 flex items-start justify-between relative">
        <div>
          <div class="flex items-center gap-3 mb-1">
            <div
              class="w-10 h-10 rounded-xl flex items-center justify-center"
              style={{
                background: 'linear-gradient(135deg, var(--color-photon-600), var(--color-photon-400))',
                'box-shadow': '0 0 15px rgba(176, 109, 232, 0.3)',
              }}
            >
              <Calendar size={20} class="text-white" />
            </div>
            <div>
              <h3 class="text-lg font-bold tracking-tight text-nebula-100">Cohort Retention</h3>
              <p class="text-xs text-nebula-500">
                <span class="font-bold text-nebula-300">{cohortMap().length}</span> cohorts • Monthly retention tracking
              </p>
            </div>
          </div>
        </div>

        <div class="flex items-center gap-2">
          <div
            class="px-3 py-1.5 rounded-full text-xs font-bold border"
            style={{
              color: overallHealth().color,
              'background-color': `color-mix(in srgb, ${overallHealth().color} 10%, transparent)`,
              'border-color': `color-mix(in srgb, ${overallHealth().color} 20%, transparent)`,
            }}
          >
            {overallHealth().label}
          </div>
          <button
            onClick={() => setIsExpanded(!isExpanded())}
            class={cn(
              'rounded-xl border border-white/[0.06] bg-void-800/50 p-2',
              'text-nebula-400 hover:text-nebula-200',
              'transition-all duration-200 hover:bg-void-750/50 hover:border-white/10'
            )}
          >
            {isExpanded() ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
          </button>
        </div>
      </div>

      <Show when={cohortMap().length === 0}>
        <div class="flex flex-col items-center justify-center py-12">
          <div class="w-16 h-16 rounded-full bg-void-800 flex items-center justify-center mb-4">
            <Calendar size={32} class="text-nebula-600" />
          </div>
          <p class="text-lg font-bold text-nebula-200">No Cohort Data</p>
          <p class="mt-1 text-sm text-nebula-500">Retention data will appear once users sign up</p>
        </div>
      </Show>

      <Show when={cohortMap().length > 0}>
        <div
          class={cn(
            'overflow-x-auto transition-all duration-500',
            mounted() ? 'opacity-100' : 'opacity-0'
          )}
        >
          <table class="w-full border-collapse">
            <thead>
              <tr>
                <th class="sticky left-0 z-10 bg-void-900 px-3 py-2 text-left text-xs font-bold uppercase tracking-wider text-nebula-500">
                  Cohort
                </th>
                <th class="sticky left-[100px] z-10 bg-void-900 px-2 py-2 text-center text-xs font-bold text-nebula-500">
                  Users
                </th>
                <For each={Array.from({ length: maxMonths() + 1 }, (_, i) => i)}>
                  {(month) => (
                    <th
                      class={cn(
                        'px-1 py-2 text-center text-xs font-bold transition-colors',
                        hoveredCell()?.month === month ? 'text-nebula-200' : 'text-nebula-600'
                      )}
                    >
                      M{month}
                    </th>
                  )}
                </For>
              </tr>
            </thead>
            <tbody>
              <For each={cohortMap()}>
                {([cohortMonth, _monthData], rowIndex) => (
                  <tr
                    class={cn(
                      'transition-colors duration-200',
                      hoveredCell()?.cohort === cohortMonth && 'bg-white/[0.02]'
                    )}
                  >
                    <td class="sticky left-0 z-10 bg-void-900 px-3 py-1.5">
                      <span class="text-sm font-bold text-nebula-200">
                        {formatMonth(cohortMonth)}
                      </span>
                    </td>
                    <td class="sticky left-[100px] z-10 bg-void-900 px-2 py-1.5 text-center">
                      <span class="text-xs font-mono tabular-nums text-nebula-400">
                        {getBaseUsers(cohortMonth).toLocaleString()}
                      </span>
                    </td>
                    <For each={Array.from({ length: maxMonths() + 1 }, (_, i) => i)}>
                      {(monthIndex) => {
                        const rate = getRetentionRate(cohortMonth, monthIndex);
                        const users = getActiveUsers(cohortMonth, monthIndex);
                        const colors = rate !== null ? getRetentionColor(rate) : null;
                        const isHovered = hoveredCell()?.cohort === cohortMonth && hoveredCell()?.month === monthIndex;

                        return (
                          <td class="px-1 py-1.5">
                            <Show
                              when={rate !== null}
                              fallback={
                                <div class="h-8 w-10 rounded bg-void-800/30 flex items-center justify-center">
                                  <span class="text-[10px] text-nebula-700">-</span>
                                </div>
                              }
                            >
                              <div
                                class={cn(
                                  'group relative h-8 w-10 rounded cursor-pointer',
                                  'transition-all duration-200',
                                  'hover:scale-110 hover:z-20',
                                  isHovered && 'scale-110 z-20'
                                )}
                                style={{
                                  background: colors?.bg,
                                  'box-shadow': isHovered ? `0 0 12px ${colors?.glow}` : undefined,
                                  'animation-delay': `${(rowIndex() * (maxMonths() + 1) + monthIndex) * 20}ms`,
                                }}
                                onMouseEnter={() => setHoveredCell({ cohort: cohortMonth, month: monthIndex })}
                                onMouseLeave={() => setHoveredCell(null)}
                              >
                                <div class="absolute inset-0 flex items-center justify-center">
                                  <span class="text-[11px] font-bold text-white/90 tabular-nums">
                                    {rate}%
                                  </span>
                                </div>

                                <div
                                  class={cn(
                                    'pointer-events-none absolute left-1/2 top-full z-30 mt-2 -translate-x-1/2',
                                    'whitespace-nowrap rounded-xl border border-white/10 px-3 py-2',
                                    'text-xs shadow-xl backdrop-blur-sm',
                                    'opacity-0 scale-95 transition-all duration-150',
                                    'group-hover:opacity-100 group-hover:scale-100'
                                  )}
                                  style={{ background: 'var(--bg-overlay, rgba(10, 10, 11, 0.95))' }}
                                >
                                  <div class="text-nebula-400 mb-1">
                                    {formatMonth(cohortMonth)} → Month {monthIndex}
                                  </div>
                                  <div class="flex items-center gap-3">
                                    <div>
                                      <div class="text-[10px] text-nebula-500">Retention</div>
                                      <div class="text-sm font-bold" style={{ color: colors?.bg }}>
                                        {rate}%
                                      </div>
                                    </div>
                                    <div class="h-6 w-px bg-white/10" />
                                    <div>
                                      <div class="text-[10px] text-nebula-500">Active</div>
                                      <div class="text-sm font-bold text-nebula-200">
                                        {users?.toLocaleString()}
                                      </div>
                                    </div>
                                  </div>
                                </div>
                              </div>
                            </Show>
                          </td>
                        );
                      }}
                    </For>
                  </tr>
                )}
              </For>

              <tr class="border-t border-white/[0.06]">
                <td class="sticky left-0 z-10 bg-void-900 px-3 py-2">
                  <span class="text-xs font-bold uppercase tracking-wider text-nebula-500">Avg</span>
                </td>
                <td class="sticky left-[100px] z-10 bg-void-900 px-2 py-2" />
                <For each={avgRetentionByMonth()}>
                  {(avgRate, monthIndex) => {
                    const colors = avgRate !== null ? getRetentionColor(avgRate) : null;
                    return (
                      <td class="px-1 py-2">
                        <Show
                          when={avgRate !== null}
                          fallback={<div class="h-6 w-10" />}
                        >
                          <div
                            class="h-6 w-10 rounded flex items-center justify-center"
                            style={{ background: `color-mix(in srgb, ${colors?.bg} 50%, transparent)` }}
                          >
                            <span class="text-[10px] font-bold tabular-nums" style={{ color: colors?.bg }}>
                              {avgRate}%
                            </span>
                          </div>
                        </Show>
                      </td>
                    );
                  }}
                </For>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="mt-6 flex flex-wrap items-center justify-between gap-4 rounded-xl border border-white/[0.06] bg-void-800/30 p-4">
          <div class="flex items-center gap-4">
            <div class="flex items-center gap-2 text-xs text-nebula-500">
              <span>Retention:</span>
            </div>
            <div class="flex items-center gap-1">
              <For each={RETENTION_COLORS}>
                {(color) => (
                  <div
                    class="group relative h-4 w-6 rounded cursor-help transition-transform hover:scale-110"
                    style={{ background: color.bg }}
                  >
                    <div class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 opacity-0 group-hover:opacity-100 transition-opacity whitespace-nowrap text-[10px] text-nebula-400 bg-void-900 px-2 py-1 rounded border border-white/10">
                      {color.min}%+ {color.label}
                    </div>
                  </div>
                )}
              </For>
            </div>
          </div>

          <div class="flex items-center gap-4 text-xs">
            <div class="flex items-center gap-2">
              <TrendingUp size={14} class="text-aurora-400" />
              <span class="text-nebula-400">
                Month 3 Avg: <span class="font-bold text-nebula-200">{avgRetentionByMonth()[3] ?? '-'}%</span>
              </span>
            </div>
            <div class="flex items-center gap-2">
              <Users size={14} class="text-indigo-400" />
              <span class="text-nebula-400">
                Month 6 Avg: <span class="font-bold text-nebula-200">{avgRetentionByMonth()[6] ?? '-'}%</span>
              </span>
            </div>
          </div>
        </div>

        <Show when={avgRetentionByMonth()[1] !== null}>
          <div
            class={cn(
              'mt-4 rounded-xl border p-4 transition-all duration-500',
              mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
            )}
            style={{
              background: `color-mix(in srgb, ${overallHealth().color} 5%, transparent)`,
              'border-color': `color-mix(in srgb, ${overallHealth().color} 20%, transparent)`,
            }}
          >
            <div class="flex items-start gap-3">
              <div
                class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0"
                style={{ background: `color-mix(in srgb, ${overallHealth().color} 15%, transparent)` }}
              >
                <Info size={14} style={{ color: overallHealth().color }} />
              </div>
              <div>
                <p class="text-sm font-semibold text-nebula-100">Retention Insight</p>
                <p class="mt-0.5 text-xs text-nebula-400 leading-relaxed">
                  {avgRetentionByMonth()[3]! >= 50
                    ? 'Strong retention! Your product has good stickiness. Focus on converting more trial users.'
                    : avgRetentionByMonth()[3]! >= 30
                    ? 'Moderate retention. Consider improving onboarding and feature discovery to boost engagement.'
                    : 'Retention needs attention. Prioritize understanding why users churn and improving first-week experience.'}
                </p>
              </div>
            </div>
          </div>
        </Show>
      </Show>
    </div>
  );
};

export default CohortRetentionHeatmap;
