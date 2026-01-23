import { Component, createResource, Show } from 'solid-js';
import * as api from '../../lib/api';
import { Lightbulb, Sparkles, RefreshCw } from '../ui/Icons';

interface SmartInsightsProps {
  target: 'user' | 'team' | 'admin';
}

export const SmartInsights: Component<SmartInsightsProps> = (props) => {
  const [insight, { refetch }] = createResource(() => api.getSmartInsights(props.target));

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
          <div class="space-y-3">
            <p class="text-sm leading-relaxed text-slate-300">
              {insight()?.insight || "Continue leveraging OMG's advanced features to maximize your development velocity."}
            </p>
            <div class="flex items-center justify-between pt-2">
              <div class="flex items-center gap-1.5 text-[10px] font-medium text-indigo-400/80 uppercase tracking-widest">
                <Sparkles size={10} />
                <span>Powered by Workers AI</span>
              </div>
              <span class="text-[10px] text-slate-500 italic">
                Generated {new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
              </span>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
};
