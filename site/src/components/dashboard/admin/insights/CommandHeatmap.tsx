import { Component, For, createMemo, createSignal, Show, onMount } from 'solid-js';
import { Activity, Maximize2, Minimize2, Flame, Clock, Calendar, TrendingUp } from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface HeatmapData {
  hour: string;
  day_of_week: string;
  event_count: number;
}

interface CommandHeatmapProps {
  data: HeatmapData[];
}

const DAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'] as const;
const HOURS = Array.from({ length: 24 }, (_, i) => i);

const heatmapColors = [
  { bg: 'var(--color-void-800)', glow: 'transparent' },
  { bg: 'var(--color-indigo-950)', glow: 'rgba(99, 102, 241, 0.1)' },
  { bg: 'var(--color-indigo-900)', glow: 'rgba(99, 102, 241, 0.15)' },
  { bg: 'var(--color-indigo-800)', glow: 'rgba(99, 102, 241, 0.2)' },
  { bg: 'var(--color-indigo-700)', glow: 'rgba(99, 102, 241, 0.25)' },
  { bg: 'var(--color-indigo-600)', glow: 'rgba(99, 102, 241, 0.3)' },
  { bg: 'var(--color-indigo-500)', glow: 'rgba(99, 102, 241, 0.4)' },
  { bg: 'var(--color-electric-500)', glow: 'rgba(34, 211, 211, 0.4)' },
  { bg: 'var(--color-electric-400)', glow: 'rgba(34, 211, 211, 0.5)' },
];

export const CommandHeatmap: Component<CommandHeatmapProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [isExpanded, setIsExpanded] = createSignal(false);
  const [hoveredCell, setHoveredCell] = createSignal<{ day: number; hour: number } | null>(null);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const maxCount = createMemo(() => {
    if (props.data.length === 0) return 1;
    return Math.max(...props.data.map((d) => d.event_count));
  });

  const totalEvents = createMemo(() => props.data.reduce((sum, d) => sum + d.event_count, 0));

  const getCountForCell = (day: number, hour: number) => {
    const cell = props.data.find((d) => parseInt(d.day_of_week) === day && parseInt(d.hour) === hour);
    return cell?.event_count || 0;
  };

  const getHeatmapLevel = (count: number): number => {
    if (count === 0) return 0;
    const intensity = count / maxCount();
    if (intensity > 0.875) return 8;
    if (intensity > 0.75) return 7;
    if (intensity > 0.625) return 6;
    if (intensity > 0.5) return 5;
    if (intensity > 0.375) return 4;
    if (intensity > 0.25) return 3;
    if (intensity > 0.125) return 2;
    return 1;
  };

  const peakActivity = createMemo(() => {
    if (props.data.length === 0) return { day: 0, hour: 0, count: 0 };
    const peak = props.data.reduce((max, d) => (d.event_count > max.event_count ? d : max), props.data[0]);
    return {
      day: parseInt(peak.day_of_week),
      hour: parseInt(peak.hour),
      count: peak.event_count,
    };
  });

  const busyDays = createMemo(() => {
    const dayTotals = DAYS.map((_, dayIndex) => ({
      day: dayIndex,
      total: HOURS.reduce((sum, hour) => sum + getCountForCell(dayIndex, hour), 0),
    }));
    return dayTotals.sort((a, b) => b.total - a.total).slice(0, 3);
  });

  const busyHours = createMemo(() => {
    const hourTotals = HOURS.map((hour) => ({
      hour,
      total: DAYS.reduce((sum, _, dayIndex) => sum + getCountForCell(dayIndex, hour), 0),
    }));
    return hourTotals.sort((a, b) => b.total - a.total).slice(0, 3);
  });

  const avgEventsPerDay = createMemo(() => {
    if (props.data.length === 0) return 0;
    return Math.round(totalEvents() / 7);
  });

  return (
    <div
      class={cn(
        'rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl relative overflow-hidden',
        'transition-all duration-300',
        isExpanded() && 'col-span-2'
      )}
    >
      <div class="absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl opacity-10 transition-opacity duration-500" style={{ background: 'var(--color-indigo-500)' }} />

      <div class="mb-6 flex items-start justify-between relative">
        <div>
          <div class="flex items-center gap-3 mb-1">
            <div
              class="w-10 h-10 rounded-xl flex items-center justify-center"
              style={{
                background: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))',
                'box-shadow': '0 0 15px rgba(99, 102, 241, 0.3)',
              }}
            >
              <Activity size={20} class="text-white" />
            </div>
            <div>
              <h3 class="text-lg font-bold tracking-tight text-nebula-100">Command Heatmap</h3>
              <p class="text-xs text-nebula-500">
                <span class="font-bold text-nebula-300 tabular-nums">{totalEvents().toLocaleString()}</span> events
                {props.data.length > 0 && (
                  <>
                    {' '}
                    • <span class="tabular-nums">{avgEventsPerDay().toLocaleString()}</span> avg/day
                  </>
                )}
              </p>
            </div>
          </div>
        </div>

        <button
          onClick={() => setIsExpanded(!isExpanded())}
          class={cn(
            'rounded-xl border border-white/[0.06] bg-void-800/50 p-2',
            'text-nebula-400 hover:text-nebula-200',
            'transition-all duration-200 hover:bg-void-750/50 hover:border-white/10'
          )}
          title={isExpanded() ? 'Collapse' : 'Expand'}
        >
          {isExpanded() ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
        </button>
      </div>

      <Show when={props.data.length === 0}>
        <div class="flex flex-col items-center justify-center py-12">
          <div class="w-16 h-16 rounded-full bg-void-800 flex items-center justify-center mb-4">
            <Activity size={32} class="text-nebula-600" />
          </div>
          <p class="text-lg font-bold text-nebula-200">No Activity Data</p>
          <p class="mt-1 text-sm text-nebula-500">Command activity will appear here once users start using OMG</p>
        </div>
      </Show>

      <Show when={props.data.length > 0}>
        <div class={cn('overflow-x-auto transition-all duration-500', mounted() ? 'opacity-100' : 'opacity-0')}>
          <div class="inline-flex flex-col gap-1">
            <div class="flex gap-1 pl-12">
              <For each={HOURS}>
                {(hour) => (
                  <div
                    class={cn('flex h-6 w-6 items-center justify-center text-[10px] font-bold transition-colors', hoveredCell()?.hour === hour ? 'text-nebula-200' : 'text-nebula-600')}
                  >
                    {hour % 6 === 0 ? hour : ''}
                  </div>
                )}
              </For>
            </div>

            <For each={DAYS.map((_, i) => i)}>
              {(dayIndex) => (
                <div class="flex items-center gap-1">
                  <div class={cn('w-10 text-right text-xs font-bold transition-colors', hoveredCell()?.day === dayIndex ? 'text-nebula-200' : 'text-nebula-500')}>{DAYS[dayIndex]}</div>
                  <div class="flex gap-1">
                    <For each={HOURS}>
                      {(hour) => {
                        const count = getCountForCell(dayIndex, hour);
                        const level = getHeatmapLevel(count);
                        const colors = heatmapColors[level];
                        const isPeak = dayIndex === peakActivity().day && hour === peakActivity().hour;
                        const isHovered = hoveredCell()?.day === dayIndex && hoveredCell()?.hour === hour;

                        return (
                          <div
                            class={cn(
                              'group relative h-6 w-6 rounded transition-all duration-200',
                              'hover:scale-125 hover:z-10 cursor-pointer',
                              isPeak && 'ring-2 ring-electric-400 ring-offset-1 ring-offset-void-900'
                            )}
                            style={{
                              background: colors.bg,
                              'box-shadow': isHovered && level > 0 ? `0 0 12px ${colors.glow}` : undefined,
                            }}
                            onMouseEnter={() => setHoveredCell({ day: dayIndex, hour })}
                            onMouseLeave={() => setHoveredCell(null)}
                          >
                            <div
                              class={cn(
                                'pointer-events-none absolute left-1/2 top-full z-20 mt-2 -translate-x-1/2',
                                'whitespace-nowrap rounded-lg border border-white/10 px-3 py-2',
                                'text-[10px] font-bold shadow-xl backdrop-blur-sm',
                                'opacity-0 scale-95 transition-all duration-150',
                                'group-hover:opacity-100 group-hover:scale-100'
                              )}
                              style={{ background: 'var(--bg-overlay)' }}
                            >
                              <div class="text-nebula-400">
                                {DAYS[dayIndex]} {hour}:00
                              </div>
                              <div class="mt-0.5 text-sm" style={{ color: level >= 6 ? 'var(--color-electric-400)' : 'var(--color-indigo-400)' }}>
                                {count.toLocaleString()} events
                              </div>
                            </div>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>

        <div class="mt-6 flex items-center justify-between rounded-xl border border-white/[0.06] bg-void-800/30 p-4">
          <div class="flex items-center gap-3">
            <div class="w-8 h-8 rounded-lg flex items-center justify-center" style={{ background: 'rgba(34, 211, 211, 0.15)' }}>
              <Flame size={14} style={{ color: 'var(--color-electric-400)' }} />
            </div>
            <div>
              <p class="text-2xs text-nebula-500">Peak Activity</p>
              <p class="text-sm font-bold text-nebula-200">
                {DAYS[peakActivity().day]} {peakActivity().hour}:00
                <span class="ml-2" style={{ color: 'var(--color-electric-400)' }}>
                  ({peakActivity().count.toLocaleString()})
                </span>
              </p>
            </div>
          </div>

          <div class="flex items-center gap-2">
            <span class="text-xs text-nebula-600">Low</span>
            <div class="flex gap-0.5">
              <For each={[1, 2, 3, 4, 5, 6, 7, 8]}>
                {(level) => (
                  <div
                    class="h-4 w-4 rounded transition-all hover:scale-110"
                    style={{
                      background: heatmapColors[level].bg,
                      'box-shadow': `0 0 4px ${heatmapColors[level].glow}`,
                    }}
                  />
                )}
              </For>
            </div>
            <span class="text-xs text-nebula-600">High</span>
          </div>
        </div>

        <Show when={isExpanded()}>
          <div class={cn('mt-6 grid grid-cols-2 gap-4 transition-all duration-500', mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4')}>
            <div class="rounded-xl border border-white/[0.06] bg-void-800/30 p-4">
              <div class="flex items-center gap-2 mb-3">
                <Calendar size={14} style={{ color: 'var(--color-electric-400)' }} />
                <h4 class="text-xs font-bold uppercase tracking-wider text-nebula-500">Busiest Days</h4>
              </div>
              <div class="space-y-2">
                <For each={busyDays()}>
                  {(item, index) => (
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-2">
                        <span
                          class="flex h-5 w-5 items-center justify-center rounded text-2xs font-black"
                          style={{
                            background: index() === 0 ? 'rgba(34, 211, 211, 0.2)' : 'rgba(255, 255, 255, 0.05)',
                            color: index() === 0 ? 'var(--color-electric-400)' : 'var(--color-nebula-400)',
                          }}
                        >
                          {index() + 1}
                        </span>
                        <span class="text-sm font-medium text-nebula-200">{DAYS[item.day]}</span>
                      </div>
                      <span class="font-mono text-sm tabular-nums text-nebula-400">{item.total.toLocaleString()}</span>
                    </div>
                  )}
                </For>
              </div>
            </div>

            <div class="rounded-xl border border-white/[0.06] bg-void-800/30 p-4">
              <div class="flex items-center gap-2 mb-3">
                <Clock size={14} style={{ color: 'var(--color-indigo-400)' }} />
                <h4 class="text-xs font-bold uppercase tracking-wider text-nebula-500">Peak Hours</h4>
              </div>
              <div class="space-y-2">
                <For each={busyHours()}>
                  {(item, index) => (
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-2">
                        <span
                          class="flex h-5 w-5 items-center justify-center rounded text-2xs font-black"
                          style={{
                            background: index() === 0 ? 'rgba(99, 102, 241, 0.2)' : 'rgba(255, 255, 255, 0.05)',
                            color: index() === 0 ? 'var(--color-indigo-400)' : 'var(--color-nebula-400)',
                          }}
                        >
                          {index() + 1}
                        </span>
                        <span class="text-sm font-medium text-nebula-200">{item.hour}:00</span>
                      </div>
                      <span class="font-mono text-sm tabular-nums text-nebula-400">{item.total.toLocaleString()}</span>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </div>
        </Show>
      </Show>
    </div>
  );
};
