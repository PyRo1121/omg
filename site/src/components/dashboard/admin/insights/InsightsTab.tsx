import { Component, Show } from 'solid-js';
import { useAdminAdvancedMetrics } from '../../../../lib/api-hooks';
import { CardSkeleton } from '../../../ui/Skeleton';
import { EngagementMetrics } from './EngagementMetrics';
import { ChurnRiskSegments } from './ChurnRiskSegments';
import { ExpansionOpportunities } from './ExpansionOpportunities';
import { TimeToValueMetrics } from './TimeToValueMetrics';
import { FeatureAdoptionChart } from './FeatureAdoptionChart';
import { CommandHeatmap } from './CommandHeatmap';
import { RuntimeAdoptionChart } from './RuntimeAdoptionChart';
import { Lightbulb, RefreshCw } from 'lucide-solid';

export const InsightsTab: Component = () => {
  const metricsQuery = useAdminAdvancedMetrics();

  return (
    <div class="animate-in fade-in slide-in-from-bottom-4 space-y-8 duration-500">
      {/* Header */}
      <div class="flex items-center justify-between">
        <div>
          <h2 class="flex items-center gap-3 text-3xl font-black tracking-tight text-white">
            <Lightbulb size={32} class="text-amber-400" />
            Business Intelligence
          </h2>
          <p class="mt-2 text-sm text-slate-400">
            Advanced analytics, customer health, and growth opportunities
          </p>
        </div>

        <button
          onClick={() => metricsQuery.refetch()}
          disabled={metricsQuery.isRefetching}
          class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm font-bold text-white transition-all hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <RefreshCw size={16} class={metricsQuery.isRefetching ? 'animate-spin' : ''} />
          Refresh
        </button>
      </div>

      <Show when={metricsQuery.isLoading}>
        <div class="grid gap-6 md:grid-cols-2">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={metricsQuery.isError}>
        <div class="rounded-xl border border-rose-500/30 bg-rose-500/10 p-8 text-center">
          <p class="text-lg font-bold text-rose-400">Failed to load advanced metrics</p>
          <p class="mt-2 text-sm text-slate-400">{metricsQuery.error?.message}</p>
          <button
            onClick={() => metricsQuery.refetch()}
            class="mt-4 rounded-lg bg-rose-500 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-rose-600"
          >
            Try Again
          </button>
        </div>
      </Show>

      <Show when={metricsQuery.isSuccess && metricsQuery.data}>
        <div class="space-y-8">
          {/* Engagement Overview */}
          <Show when={metricsQuery.data!.engagement}>
            <EngagementMetrics data={metricsQuery.data!.engagement!} />
          </Show>

          {/* Critical Business Metrics - Two Column */}
          <div class="grid gap-6 lg:grid-cols-2">
            <Show when={metricsQuery.data!.churn_risk_segments}>
              <ChurnRiskSegments data={metricsQuery.data!.churn_risk_segments!} />
            </Show>
            <Show when={metricsQuery.data!.expansion_opportunities}>
              <ExpansionOpportunities data={metricsQuery.data!.expansion_opportunities!} />
            </Show>
          </div>

          {/* Onboarding Success */}
          <Show when={metricsQuery.data!.time_to_value}>
            <TimeToValueMetrics data={metricsQuery.data!.time_to_value!} />
          </Show>

          {/* Feature Analytics - Two Column */}
          <div class="grid gap-6 lg:grid-cols-2">
            <Show when={metricsQuery.data!.feature_adoption}>
              <FeatureAdoptionChart data={metricsQuery.data!.feature_adoption!} />
            </Show>
            <Show when={metricsQuery.data!.command_heatmap}>
              <CommandHeatmap data={metricsQuery.data!.command_heatmap!} />
            </Show>
          </div>

          {/* Runtime Adoption */}
          <Show when={metricsQuery.data!.runtime_adoption}>
            <RuntimeAdoptionChart data={metricsQuery.data!.runtime_adoption!} />
          </Show>

          {/* Summary Card */}
          <div class="rounded-3xl border border-white/5 bg-gradient-to-br from-indigo-500/10 to-purple-500/5 p-8">
            <h3 class="mb-4 text-xl font-bold text-white">Key Insights Summary</h3>
            <div class="grid gap-4 md:grid-cols-3">
              <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                <p class="text-xs text-slate-400">Current MRR</p>
                <p class="mt-1 text-2xl font-black text-emerald-400">
                  ${(metricsQuery.data!.revenue_metrics?.current_mrr || 0).toLocaleString()}
                </p>
                <p class="mt-1 text-xs text-slate-500">
                  ${(metricsQuery.data!.revenue_metrics?.projected_arr || 0).toLocaleString()} ARR
                </p>
              </div>

              <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                <p class="text-xs text-slate-400">Expansion MRR (12m)</p>
                <p class="mt-1 text-2xl font-black text-indigo-400">
                  ${(metricsQuery.data!.revenue_metrics?.expansion_mrr_12m || 0).toLocaleString()}
                </p>
                <p class="mt-1 text-xs text-slate-500">New revenue from upgrades</p>
              </div>

              <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                <p class="text-xs text-slate-400">Product Stickiness</p>
                <p class="mt-1 text-2xl font-black text-purple-400">
                  {metricsQuery.data!.retention?.product_stickiness?.daily_active_pct?.toFixed(1) ||
                    0}
                  %
                </p>
                <p class="mt-1 text-xs text-slate-500">Daily active users</p>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};
