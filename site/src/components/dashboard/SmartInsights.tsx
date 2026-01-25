import { Component, createResource, Show, createSignal } from 'solid-js';
import * as api from '../../lib/api';
import { Lightbulb, Sparkles, RefreshCw, Zap, Shield, Users, Target } from '../ui/Icons';

interface SmartInsightsProps {
  target: 'user' | 'team' | 'admin';
}

type InsightCategory = 'efficiency' | 'security' | 'collaboration' | 'optimization' | 'health';

export const SmartInsights: Component<SmartInsightsProps> = (props) => {
  const [insight, { refetch }] = createResource(() => api.getSmartInsights(props.target));
  const [showFull, setShowFull] = createSignal(false);

  const getInsightCategory = (text: string): InsightCategory => {
    const lower = text.toLowerCase();
    if (lower.includes('time') || lower.includes('speed') || lower.includes('efficient')) return 'efficiency';
    if (lower.includes('security') || lower.includes('vulnerability') || lower.includes('compliance')) return 'security';
    if (lower.includes('team') || lower.includes('fleet') || lower.includes('member')) return 'collaboration';
    if (lower.includes('optimize') || lower.includes('improve') || lower.includes('reduce')) return 'optimization';
    return 'health';
  };

  const getCategoryIcon = (category: InsightCategory) => {
    switch (category) {
      case 'efficiency': return Zap;
      case 'security': return Shield;
      case 'collaboration': return Users;
      case 'optimization': return Target;
      default: return Lightbulb;
    }
  };

  const getCategoryColor = (category: InsightCategory) => {
    switch (category) {
      case 'efficiency': return 'bg-cyan-500/20 text-cyan-400';
      case 'security': return 'bg-rose-500/20 text-rose-400';
      case 'collaboration': return 'bg-purple-500/20 text-purple-400';
      case 'optimization': return 'bg-amber-500/20 text-amber-400';
      default: return 'bg-indigo-500/20 text-indigo-400';
    }
  };

  return (
    <div class="relative overflow-hidden rounded-2xl border border-indigo-500/30 bg-indigo-500/5 p-6 backdrop-blur-sm">
      {/* Background Sparkle Decoration */}
      <div class="absolute -right-4 -top-4 text-indigo-500/10 rotate-12">
        <Sparkles size={120} />
      </div>

      <div class="relative z-10">
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-indigo-500/20 text-indigo-400">
              <Lightbulb size={18} />
            </div>
            <h3 class="text-lg font-semibold text-white">AI Smart Insight</h3>
          </div>
          <button
            onClick={refetch}
            class="rounded-full p-1.5 text-slate-400 hover:bg-slate-800 hover:text-white transition-colors"
            title="Refresh Insight"
          >
            <RefreshCw size={14} class={insight.loading ? 'animate-spin' : ''} />
          </button>
        </div>

        <Show
          when={!insight.loading}
          fallback={
            <div class="space-y-2 animate-pulse">
              <div class="h-4 w-3/4 rounded bg-slate-800" />
              <div class="h-4 w-1/2 rounded bg-slate-800" />
            </div>
          }
        >
          <Show when={!insight.error}>
            <div class="space-y-3">
              <div class="flex items-start gap-3">
                <div class={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl ${getCategoryColor(getInsightCategory(insight()?.insight || ''))}`}>
                  <Show when={insight()?.insight}>
                    {(insightText) => {
                      const Icon = getCategoryIcon(getInsightCategory(insightText() || ''));
                      return <Icon size={20} />;
                    }}
                  </Show>
                </div>
                <div class="flex-1">
                  <Show when={insight()?.insight}>
                    {(insightText) => (
                      <div>
                        <p class={`text-sm leading-relaxed ${showFull() ? '' : 'line-clamp-2'}`}>
                          {insightText()}
                        </p>
                        <Show when={(insightText()?.length || 0) > 100}>
                          <button
                            onClick={() => setShowFull(!showFull())}
                            class="mt-2 text-xs font-medium text-indigo-400 hover:text-indigo-300 transition-colors"
                          >
                            {showFull() ? 'Show less' : 'Read more'}
                          </button>
                        </Show>
                      </div>
                    )}
                  </Show>
                </div>
              </div>
                <div class="flex items-center justify-between pt-2">
                <div class="flex items-center gap-1.5 text-[10px] font-medium text-indigo-400/80 uppercase tracking-widest">
                  <Sparkles size={10} />
                  <span>{insight()?.generated_by || 'Workers AI'}</span>
                </div>
                <span class="text-[10px] text-slate-500 italic">
                  {insight()?.timestamp ? new Date(insight()?.timestamp || '').toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : ''}
                </span>
              </div>
            </div>
          </Show>

          <Show when={insight.error}>
            <div class="rounded-xl bg-rose-500/10 border border-rose-500/20 p-4">
              <div class="flex items-start gap-3">
                <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-rose-500/20 text-rose-400">
                  <RefreshCw size={16} />
                </div>
                <div class="flex-1">
                  <p class="text-sm font-medium text-rose-300">Failed to load insight</p>
                  <p class="mt-1 text-xs text-slate-400">
                    The AI service is temporarily unavailable. Try refreshing or check back later.
                  </p>
                  <button
                    onClick={refetch}
                    class="mt-2 rounded-lg bg-rose-500/20 px-3 py-1.5 text-xs font-medium text-rose-400 hover:bg-rose-500/30 transition-colors"
                  >
                    Retry
                  </button>
                </div>
              </div>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
};
