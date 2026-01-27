import { Component, Show, For, createSignal } from 'solid-js';
import { useAdminAdvancedMetrics } from '../../../../lib/api-hooks';
import { CardSkeleton } from '../../../ui/Skeleton';
import { EngagementMetrics } from './EngagementMetrics';
import { ChurnRiskSegments } from './ChurnRiskSegments';
import { ExpansionOpportunities } from './ExpansionOpportunities';
import { TimeToValueMetrics } from './TimeToValueMetrics';
import { FeatureAdoptionChart } from './FeatureAdoptionChart';
import { CommandHeatmap } from './CommandHeatmap';
import { RuntimeAdoptionChart } from './RuntimeAdoptionChart';
import {
  Lightbulb,
  RefreshCw,
  Bookmark,
  Share2,
  Download,
  MessageSquare,
  Sparkles,
  Search,
} from 'lucide-solid';

type InsightCategory = 'all' | 'engagement' | 'revenue' | 'risk' | 'growth';

const INSIGHT_CATEGORIES: { id: InsightCategory; label: string; color: string }[] = [
  { id: 'all', label: 'All Insights', color: 'text-white' },
  { id: 'engagement', label: 'Engagement', color: 'text-indigo-400' },
  { id: 'revenue', label: 'Revenue', color: 'text-aurora-400' },
  { id: 'risk', label: 'Risk', color: 'text-flare-400' },
  { id: 'growth', label: 'Growth', color: 'text-solar-400' },
];

export const InsightsTab: Component = () => {
  const metricsQuery = useAdminAdvancedMetrics();
  const [activeCategory, setActiveCategory] = createSignal<InsightCategory>('all');
  const [bookmarkedInsights, setBookmarkedInsights] = createSignal<string[]>([]);
  const [aiQuery, setAiQuery] = createSignal('');
  const [showAiPanel, setShowAiPanel] = createSignal(false);

  const toggleBookmark = (insightId: string) => {
    setBookmarkedInsights((prev) =>
      prev.includes(insightId) ? prev.filter((id) => id !== insightId) : [...prev, insightId]
    );
  };

  const exportInsight = (format: 'pdf' | 'png') => {
    const a = document.createElement('a');
    a.download = `insights-${Date.now()}.${format}`;
    a.click();
  };

  return (
    <div class="animate-in fade-in slide-in-from-bottom-4 space-y-6 duration-500">
      <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h2 class="flex items-center gap-3 text-3xl font-black tracking-tight text-white">
            <Lightbulb size={32} class="text-amber-400" />
            Business Intelligence
          </h2>
          <p class="mt-2 text-sm text-slate-400">
            Advanced analytics, customer health, and growth opportunities
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <button
            onClick={() => setShowAiPanel(!showAiPanel())}
            class={`flex items-center gap-2 rounded-xl border px-4 py-2.5 text-sm font-bold transition-all ${
              showAiPanel()
                ? 'border-photon-500/50 bg-photon-500/10 text-photon-400'
                : 'border-white/10 bg-white/5 text-white hover:bg-white/10'
            }`}
          >
            <Sparkles size={16} />
            Ask AI
          </button>

          <button
            onClick={() => exportInsight('pdf')}
            class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm font-bold text-white transition-all hover:bg-white/10"
          >
            <Download size={16} />
            Export
          </button>

          <button
            onClick={() => metricsQuery.refetch()}
            disabled={metricsQuery.isRefetching}
            class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm font-bold text-white transition-all hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <RefreshCw size={16} class={metricsQuery.isRefetching ? 'animate-spin' : ''} />
            Refresh
          </button>
        </div>
      </div>

      <Show when={showAiPanel()}>
        <div class="rounded-2xl border border-photon-500/30 bg-photon-500/5 p-6">
          <div class="flex items-start gap-4">
            <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-photon-500/20">
              <Sparkles size={20} class="text-photon-400" />
            </div>
            <div class="flex-1">
              <h3 class="mb-2 font-bold text-white">Ask AI about your data</h3>
              <div class="relative">
                <Search size={16} class="absolute left-4 top-1/2 -translate-y-1/2 text-nebula-500" />
                <input
                  type="text"
                  value={aiQuery()}
                  onInput={(e) => setAiQuery(e.currentTarget.value)}
                  placeholder="e.g., Why is churn higher this month? Which segments are growing fastest?"
                  class="w-full rounded-xl border border-white/10 bg-void-850 py-3 pl-12 pr-4 text-white placeholder-nebula-500 focus:outline-none focus:ring-2 focus:ring-photon-500/20"
                />
              </div>
              <div class="mt-3 flex flex-wrap gap-2">
                <button class="rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs text-nebula-400 transition-all hover:bg-white/10 hover:text-white">
                  Top churn reasons
                </button>
                <button class="rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs text-nebula-400 transition-all hover:bg-white/10 hover:text-white">
                  Revenue forecast
                </button>
                <button class="rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs text-nebula-400 transition-all hover:bg-white/10 hover:text-white">
                  Fastest growing segment
                </button>
              </div>
            </div>
          </div>
        </div>
      </Show>

      <div class="flex items-center gap-2 overflow-x-auto rounded-xl border border-white/5 bg-white/[0.02] p-1">
        <For each={INSIGHT_CATEGORIES}>
          {(cat) => (
            <button
              onClick={() => setActiveCategory(cat.id)}
              class={`flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-bold transition-all ${
                activeCategory() === cat.id
                  ? 'bg-white text-black'
                  : 'text-nebula-400 hover:bg-white/5 hover:text-white'
              }`}
            >
              {cat.label}
            </button>
          )}
        </For>
      </div>

      <Show when={bookmarkedInsights().length > 0}>
        <div class="flex items-center gap-2 rounded-xl border border-solar-500/20 bg-solar-500/5 px-4 py-3">
          <Bookmark size={16} class="text-solar-400" />
          <span class="text-sm text-solar-400">
            {bookmarkedInsights().length} insights bookmarked
          </span>
          <button class="ml-auto flex items-center gap-1 rounded-lg bg-solar-500/20 px-3 py-1 text-xs font-bold text-solar-400 hover:bg-solar-500/30">
            <Share2 size={12} />
            Share
          </button>
        </div>
      </Show>

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
          <Show when={activeCategory() === 'all' || activeCategory() === 'engagement'}>
            <Show when={metricsQuery.data!.engagement}>
              <div class="group relative">
                <div class="absolute right-4 top-4 z-10 flex gap-2 opacity-0 transition-opacity group-hover:opacity-100">
                  <button
                    onClick={() => toggleBookmark('engagement')}
                    class={`rounded-lg p-2 transition-all ${
                      bookmarkedInsights().includes('engagement')
                        ? 'bg-solar-500/20 text-solar-400'
                        : 'bg-void-800 text-nebula-400 hover:text-white'
                    }`}
                  >
                    <Bookmark size={14} />
                  </button>
                  <button class="rounded-lg bg-void-800 p-2 text-nebula-400 transition-all hover:text-white">
                    <MessageSquare size={14} />
                  </button>
                </div>
                <EngagementMetrics data={metricsQuery.data!.engagement!} />
              </div>
            </Show>
          </Show>

          <Show when={activeCategory() === 'all' || activeCategory() === 'risk' || activeCategory() === 'growth'}>
            <div class="grid gap-6 lg:grid-cols-2">
              <Show when={(activeCategory() === 'all' || activeCategory() === 'risk') && metricsQuery.data!.churn_risk_segments}>
                <ChurnRiskSegments data={metricsQuery.data!.churn_risk_segments!} />
              </Show>
              <Show when={(activeCategory() === 'all' || activeCategory() === 'growth') && metricsQuery.data!.expansion_opportunities}>
                <ExpansionOpportunities data={metricsQuery.data!.expansion_opportunities!} />
              </Show>
            </div>
          </Show>

          <Show when={activeCategory() === 'all' || activeCategory() === 'engagement'}>
            <Show when={metricsQuery.data!.time_to_value}>
              <TimeToValueMetrics data={metricsQuery.data!.time_to_value!} />
            </Show>
          </Show>

          <Show when={activeCategory() === 'all' || activeCategory() === 'engagement'}>
            <div class="grid gap-6 lg:grid-cols-2">
              <Show when={metricsQuery.data!.feature_adoption}>
                <FeatureAdoptionChart data={metricsQuery.data!.feature_adoption!} />
              </Show>
              <Show when={metricsQuery.data!.command_heatmap}>
                <CommandHeatmap data={metricsQuery.data!.command_heatmap!} />
              </Show>
            </div>
          </Show>

          <Show when={activeCategory() === 'all' || activeCategory() === 'engagement'}>
            <Show when={metricsQuery.data!.runtime_adoption}>
              <RuntimeAdoptionChart data={metricsQuery.data!.runtime_adoption!} />
            </Show>
          </Show>

          <div class="rounded-3xl border border-white/5 bg-gradient-to-br from-indigo-500/10 to-purple-500/5 p-8">
            <div class="mb-4 flex items-center justify-between">
              <h3 class="text-xl font-bold text-white">Key Insights Summary</h3>
              <div class="flex gap-2">
                <button
                  onClick={() => exportInsight('pdf')}
                  class="flex items-center gap-1 rounded-lg bg-white/5 px-3 py-1.5 text-xs font-bold text-white hover:bg-white/10"
                >
                  <Download size={12} />
                  PDF
                </button>
                <button
                  onClick={() => exportInsight('png')}
                  class="flex items-center gap-1 rounded-lg bg-white/5 px-3 py-1.5 text-xs font-bold text-white hover:bg-white/10"
                >
                  <Download size={12} />
                  PNG
                </button>
              </div>
            </div>
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
