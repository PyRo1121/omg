import { Component, For, createMemo } from 'solid-js';
import { Activity } from 'lucide-solid';

interface HeatmapData {
  hour: string;
  day_of_week: string;
  event_count: number;
}

interface CommandHeatmapProps {
  data: HeatmapData[];
}

export const CommandHeatmap: Component<CommandHeatmapProps> = (props) => {
  const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const hours = Array.from({ length: 24 }, (_, i) => i);

  const maxCount = createMemo(() => {
    if (props.data.length === 0) return 1;
    return Math.max(...props.data.map((d) => d.event_count));
  });

  const getCountForCell = (day: number, hour: number) => {
    const cell = props.data.find(
      (d) => parseInt(d.day_of_week) === day && parseInt(d.hour) === hour
    );
    return cell?.event_count || 0;
  };

  const getHeatColor = (count: number) => {
    if (count === 0) return 'bg-white/5';
    const intensity = count / maxCount();
    if (intensity > 0.75) return 'bg-indigo-500';
    if (intensity > 0.5) return 'bg-indigo-600';
    if (intensity > 0.25) return 'bg-indigo-700';
    return 'bg-indigo-800';
  };

  return (
    <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
      <div class="mb-6">
        <h3 class="flex items-center gap-2 text-2xl font-black tracking-tight text-white">
          <Activity size={24} class="text-indigo-400" />
          Command Heatmap
        </h3>
        <p class="mt-1 text-sm text-slate-500">Usage patterns by day and hour (last 7 days)</p>
      </div>

      <div class="overflow-x-auto">
        <div class="inline-flex flex-col gap-1">
          {/* Hours header */}
          <div class="flex gap-1 pl-12">
            <For each={hours}>
              {(hour) => (
                <div class="flex h-6 w-6 items-center justify-center text-[10px] font-bold text-slate-500">
                  {hour % 6 === 0 ? hour : ''}
                </div>
              )}
            </For>
          </div>

          {/* Heatmap grid */}
          <For each={days.map((_, i) => i)}>
            {(dayIndex) => (
              <div class="flex items-center gap-1">
                <div class="w-10 text-right text-xs font-bold text-slate-400">
                  {days[dayIndex]}
                </div>
                <div class="flex gap-1">
                  <For each={hours}>
                    {(hour) => {
                      const count = getCountForCell(dayIndex, hour);
                      return (
                        <div
                          class={`group relative h-6 w-6 rounded ${getHeatColor(count)} transition-all hover:scale-125 hover:ring-2 hover:ring-indigo-400`}
                          title={`${days[dayIndex]} ${hour}:00 - ${count} events`}
                        >
                          <div class="absolute left-1/2 top-full z-10 mt-2 hidden -translate-x-1/2 whitespace-nowrap rounded bg-black px-2 py-1 text-[10px] font-bold text-white group-hover:block">
                            {days[dayIndex]} {hour}:00
                            <br />
                            {count} events
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

      <div class="mt-6 flex items-center justify-between rounded-xl border border-white/10 bg-white/5 p-4">
        <div>
          <p class="text-xs text-slate-400">Peak Activity</p>
          <p class="mt-1 text-sm font-bold text-white">
            {(() => {
              const peak = props.data.reduce(
                (max, d) => (d.event_count > max.event_count ? d : max),
                props.data[0] || { hour: '0', day_of_week: '0', event_count: 0 }
              );
              return `${days[parseInt(peak?.day_of_week || '0')]} ${peak?.hour || '0'}:00`;
            })()}
          </p>
        </div>
        <div class="flex items-center gap-2">
          <span class="text-xs text-slate-400">Low</span>
          <div class="flex gap-1">
            <div class="h-4 w-4 rounded bg-indigo-800" />
            <div class="h-4 w-4 rounded bg-indigo-700" />
            <div class="h-4 w-4 rounded bg-indigo-600" />
            <div class="h-4 w-4 rounded bg-indigo-500" />
          </div>
          <span class="text-xs text-slate-400">High</span>
        </div>
      </div>
    </div>
  );
};
