import { Component, For } from 'solid-js';
import GlassCard from '../../ui/GlassCard';
import { Shield } from '../../ui/Icons';

interface SecurityScoreProps {
  score: number;
  critical: number;
  high: number;
  medium: number;
  low: number;
  class?: string;
}

export const SecurityScore: Component<SecurityScoreProps> = (props) => {
  return (
    <GlassCard class={`p-8 ${props.class || ''}`}>
      <div class="mb-6 flex items-center justify-between">
        <h3 class="text-lg font-bold text-white uppercase tracking-widest">Fleet Security</h3>
        <Shield size={20} class="text-rose-500" />
      </div>
      
      <div class="space-y-6">
        <div>
          <div class="mb-2 flex justify-between text-[11px] font-black uppercase tracking-widest">
            <span class="text-slate-500">Security Score</span>
            <span class="text-emerald-400">{props.score}%</span>
          </div>
          <div class="h-2 overflow-hidden rounded-full bg-white/[0.03]">
            <div 
              class="h-full bg-gradient-to-r from-emerald-600 to-emerald-400 transition-all duration-1000" 
              style={{ width: `${props.score}%` }} 
            />
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="rounded-2xl bg-white/[0.02] p-4 border border-white/[0.03]">
            <span class="text-[10px] font-bold text-slate-500 uppercase">Critical</span>
            <div class="text-xl font-black text-white">{props.critical}</div>
          </div>
          <div class="rounded-2xl bg-white/[0.02] p-4 border border-white/[0.03]">
            <span class="text-[10px] font-bold text-slate-500 uppercase">High</span>
            <div class="text-xl font-black text-rose-500">{props.high}</div>
          </div>
        </div>

        <div class="space-y-3 pt-2">
          <For each={[
            { label: 'Medium', value: props.medium, color: 'bg-amber-400', text: 'text-amber-400' },
            { label: 'Low', value: props.low, color: 'bg-emerald-500', text: 'text-emerald-500' },
          ]}>
            {(stat) => (
              <div class="flex items-center justify-between">
                <span class="text-[10px] font-bold text-slate-500 uppercase tracking-widest">{stat.label}</span>
                <span class={`text-sm font-black ${stat.text}`}>{stat.value}</span>
              </div>
            )}
          </For>
        </div>
      </div>
    </GlassCard>
  );
};
