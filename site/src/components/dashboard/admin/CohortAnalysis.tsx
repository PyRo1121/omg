import { Component, For, Show, createMemo } from 'solid-js';
import { useAdminCohorts } from '../../../lib/api-hooks';
import { TrendingUp } from 'lucide-solid';
import { CardSkeleton } from '../../ui/Skeleton';

interface CohortData {
  cohort_week: string;
  weeks_since_signup: number;
  active_users: number;
}

export const CohortAnalysis: Component = () => {
  const cohortsQuery = useAdminCohorts();

  // Group cohorts by cohort_week and calculate retention rates
  const cohortMap = createMemo(() => {
    const data = cohortsQuery.data?.cohorts || [];
    const map = new Map<string, CohortData[]>();

    data.forEach((item) => {
      if (!map.has(item.cohort_week)) {
        map.set(item.cohort_week, []);
      }
      map.get(item.cohort_week)!.push(item);
    });

    // Sort cohorts by date (newest first) and limit to 12
    return Array.from(map.entries())
      .sort((a, b) => b[0].localeCompare(a[0]))
      .slice(0, 12);
  });

  const getRetentionColor = (rate: number) => {
    if (rate >= 80) return 'bg-emerald-500';
    if (rate >= 60) return 'bg-cyan-500';
    if (rate >= 40) return 'bg-amber-500';
    if (rate >= 20) return 'bg-orange-500';
    return 'bg-rose-500';
  };

  const getRetentionRate = (cohortData: CohortData[], weekIndex: number) => {
    const weekData = cohortData.find((d) => d.weeks_since_signup === weekIndex);
    const week0Data = cohortData.find((d) => d.weeks_since_signup === 0);

    if (!weekData || !week0Data || week0Data.active_users === 0) return null;

    return Math.round((weekData.active_users / week0Data.active_users) * 100);
  };

  return (
    <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
      <div class="mb-6 flex items-center gap-3">
        <TrendingUp size={24} class="text-purple-400" />
        <div>
          <h3 class="text-2xl font-black tracking-tight text-white">Cohort Retention Analysis</h3>
          <p class="text-sm text-slate-500">User retention by signup week over 12 weeks</p>
        </div>
      </div>

      <Show when={cohortsQuery.isLoading}>
        <CardSkeleton />
      </Show>

      <Show when={cohortsQuery.isError}>
        <div class="rounded-xl border border-rose-500/30 bg-rose-500/10 p-6 text-center">
          <p class="text-rose-400">Failed to load cohort data</p>
          <p class="mt-2 text-sm text-slate-400">{cohortsQuery.error?.message}</p>
        </div>
      </Show>

      <Show when={cohortsQuery.isSuccess && cohortMap().length > 0}>
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-white/10">
                <th class="sticky left-0 z-10 bg-[#0d0d0e] px-4 py-3 text-left text-xs font-bold uppercase text-slate-400">
                  Cohort
                </th>
                <For each={Array.from({ length: 13 }, (_, i) => i)}>
                  {(week) => (
                    <th class="px-2 py-3 text-center text-xs font-bold text-slate-400">
                      W{week}
                    </th>
                  )}
                </For>
              </tr>
            </thead>
            <tbody>
              <For each={cohortMap()}>
                {([cohortWeek, data]) => {
                  const week0 = data.find((d) => d.weeks_since_signup === 0);
                  return (
                    <tr class="border-b border-white/5 transition-colors hover:bg-white/5">
                      <td class="sticky left-0 z-10 bg-[#0d0d0e] px-4 py-3 font-mono text-xs text-white">
                        <div class="font-bold">{cohortWeek}</div>
                        <div class="text-slate-500">{week0?.active_users || 0} users</div>
                      </td>
                      <For each={Array.from({ length: 13 }, (_, i) => i)}>
                        {(weekIndex) => {
                          const rate = getRetentionRate(data, weekIndex);
                          return (
                            <td class="px-2 py-3 text-center">
                              <Show when={rate !== null} fallback={<span class="text-slate-700">-</span>}>
                                <div
                                  class={`inline-block rounded px-2 py-1 text-xs font-bold text-white ${getRetentionColor(rate!)}`}
                                  title={`${rate}% retention (${data.find((d) => d.weeks_since_signup === weekIndex)?.active_users || 0} users)`}
                                >
                                  {rate}%
                                </div>
                              </Show>
                            </td>
                          );
                        }}
                      </For>
                    </tr>
                  );
                }}
              </For>
            </tbody>
          </table>
        </div>

        <div class="mt-6 flex flex-wrap gap-4 rounded-xl border border-white/5 bg-white/5 p-4">
          <div class="flex items-center gap-2">
            <div class="h-4 w-4 rounded bg-emerald-500" />
            <span class="text-xs text-slate-400">80%+ Excellent</span>
          </div>
          <div class="flex items-center gap-2">
            <div class="h-4 w-4 rounded bg-cyan-500" />
            <span class="text-xs text-slate-400">60-79% Good</span>
          </div>
          <div class="flex items-center gap-2">
            <div class="h-4 w-4 rounded bg-amber-500" />
            <span class="text-xs text-slate-400">40-59% Fair</span>
          </div>
          <div class="flex items-center gap-2">
            <div class="h-4 w-4 rounded bg-orange-500" />
            <span class="text-xs text-slate-400">20-39% Poor</span>
          </div>
          <div class="flex items-center gap-2">
            <div class="h-4 w-4 rounded bg-rose-500" />
            <span class="text-xs text-slate-400">&lt;20% Critical</span>
          </div>
        </div>
      </Show>

      <Show when={cohortsQuery.isSuccess && cohortMap().length === 0}>
        <div class="py-12 text-center text-slate-400">
          No cohort data available yet. Data will appear once users sign up.
        </div>
      </Show>
    </div>
  );
};
