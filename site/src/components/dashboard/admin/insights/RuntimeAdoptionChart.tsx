import { Component, For, Show } from 'solid-js';
import { Repeat, Users, Activity, Clock } from 'lucide-solid';

interface RuntimeData {
  runtime: string;
  unique_users: number;
  total_uses: number;
  avg_duration_ms: number;
}

interface RuntimeAdoptionChartProps {
  data: RuntimeData[];
}

export const RuntimeAdoptionChart: Component<RuntimeAdoptionChartProps> = (props) => {
  const sortedRuntimes = () =>
    [...(props.data || [])].sort((a, b) => (b.unique_users ?? 0) - (a.unique_users ?? 0)).slice(0, 8);

  const maxUsers = () => Math.max(...(props.data || []).map((r) => r.unique_users ?? 0), 1);

  const formatDuration = (ms: number | undefined | null) => {
    const val = ms ?? 0;
    if (val < 1000) return `${Math.round(val)}ms`;
    return `${(val / 1000).toFixed(1)}s`;
  };

  const totalUsers = () => (props.data || []).reduce((sum, r) => sum + (r.unique_users ?? 0), 0);
  const totalSwitches = () => (props.data || []).reduce((sum, r) => sum + (r.total_uses ?? 0), 0);

  return (
    <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
      <div class="mb-6 flex items-center justify-between">
        <div>
          <h3 class="flex items-center gap-2 text-2xl font-black tracking-tight text-white">
            <Repeat size={24} class="text-purple-400" />
            Runtime Adoption
          </h3>
          <p class="mt-1 text-sm text-slate-500">
            {totalSwitches().toLocaleString()} runtime switches across{' '}
            {totalUsers().toLocaleString()} users
          </p>
        </div>
      </div>

      <Show when={sortedRuntimes().length === 0}>
        <div class="py-12 text-center text-slate-400">No runtime adoption data available</div>
      </Show>

      <div class="space-y-3">
        <For each={sortedRuntimes()}>
          {(runtime, index) => {
            const percentage = ((runtime.unique_users / maxUsers()) * 100).toFixed(1);

            return (
              <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                <div class="mb-3 flex items-center justify-between">
                  <div class="flex items-center gap-3">
                    <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-purple-500/20 text-xs font-black text-purple-400">
                      {index() + 1}
                    </div>
                    <div>
                      <p class="text-sm font-bold text-white capitalize">{runtime.runtime}</p>
                      <div class="mt-1 flex items-center gap-3 text-xs text-slate-400">
                        <span class="flex items-center gap-1">
                          <Users size={12} />
                          {runtime.unique_users} users
                        </span>
                        <span class="flex items-center gap-1">
                          <Activity size={12} />
                          {(runtime.total_uses ?? 0).toLocaleString()} uses
                        </span>
                        <span class="flex items-center gap-1">
                          <Clock size={12} />
                          {formatDuration(runtime.avg_duration_ms)} avg
                        </span>
                      </div>
                    </div>
                  </div>
                  <div class="text-right">
                    <p class="text-2xl font-black text-purple-400">{percentage}%</p>
                  </div>
                </div>

                <div class="h-2 overflow-hidden rounded-full bg-white/5">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-purple-600 to-purple-400 transition-all duration-500"
                    style={{ width: `${percentage}%` }}
                  />
                </div>
              </div>
            );
          }}
        </For>
      </div>

      <Show when={props.data.length > 8}>
        <div class="mt-4 text-center text-sm text-slate-500">
          Showing top 8 of {props.data.length} runtimes
        </div>
      </Show>
    </div>
  );
};
