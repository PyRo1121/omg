import { Component } from 'solid-js';
import { Rocket, Clock, Award, TrendingUp, Target } from 'lucide-solid';

interface TimeToValueData {
  avg_days_to_activation: number;
  avg_days_to_power_user: number;
  pct_activated_day1: number;
  pct_activated_week1: number;
  pct_became_power_users: number;
}

interface TimeToValueMetricsProps {
  data: TimeToValueData;
}

export const TimeToValueMetrics: Component<TimeToValueMetricsProps> = (props) => {
  return (
    <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
      <div class="mb-6">
        <h3 class="flex items-center gap-2 text-2xl font-black tracking-tight text-white">
          <Rocket size={24} class="text-indigo-400" />
          Time to Value
        </h3>
        <p class="mt-1 text-sm text-slate-500">Onboarding success and activation metrics</p>
      </div>

      <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-5">
        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <div class="mb-3 flex items-center gap-2">
            <div class="rounded-lg bg-indigo-500/20 p-2">
              <Clock size={16} class="text-indigo-400" />
            </div>
            <span class="text-xs font-bold uppercase tracking-wider text-slate-400">
              Activation
            </span>
          </div>
          <div class="text-2xl font-black text-white">
            {props.data.avg_days_to_activation?.toFixed(1) || 'N/A'}
          </div>
          <p class="mt-1 text-xs text-slate-500">days avg</p>
        </div>

        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <div class="mb-3 flex items-center gap-2">
            <div class="rounded-lg bg-purple-500/20 p-2">
              <Award size={16} class="text-purple-400" />
            </div>
            <span class="text-xs font-bold uppercase tracking-wider text-slate-400">
              Power User
            </span>
          </div>
          <div class="text-2xl font-black text-white">
            {props.data.avg_days_to_power_user?.toFixed(1) || 'N/A'}
          </div>
          <p class="mt-1 text-xs text-slate-500">days avg</p>
        </div>

        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <div class="mb-3 flex items-center gap-2">
            <div class="rounded-lg bg-emerald-500/20 p-2">
              <Target size={16} class="text-emerald-400" />
            </div>
            <span class="text-xs font-bold uppercase tracking-wider text-slate-400">Day 1</span>
          </div>
          <div class="text-2xl font-black text-white">
            {props.data.pct_activated_day1?.toFixed(0) || 0}%
          </div>
          <p class="mt-1 text-xs text-slate-500">activated</p>
        </div>

        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <div class="mb-3 flex items-center gap-2">
            <div class="rounded-lg bg-cyan-500/20 p-2">
              <TrendingUp size={16} class="text-cyan-400" />
            </div>
            <span class="text-xs font-bold uppercase tracking-wider text-slate-400">Week 1</span>
          </div>
          <div class="text-2xl font-black text-white">
            {props.data.pct_activated_week1?.toFixed(0) || 0}%
          </div>
          <p class="mt-1 text-xs text-slate-500">activated</p>
        </div>

        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <div class="mb-3 flex items-center gap-2">
            <div class="rounded-lg bg-amber-500/20 p-2">
              <Award size={16} class="text-amber-400" />
            </div>
            <span class="text-xs font-bold uppercase tracking-wider text-slate-400">
              Conversion
            </span>
          </div>
          <div class="text-2xl font-black text-white">
            {props.data.pct_became_power_users?.toFixed(0) || 0}%
          </div>
          <p class="mt-1 text-xs text-slate-500">to power user</p>
        </div>
      </div>

      <div class="mt-6 rounded-xl border border-indigo-500/30 bg-indigo-500/5 p-4">
        <p class="text-sm font-medium text-indigo-400">
          📊 Insight: {props.data.pct_activated_day1 > 50 ? 'Strong' : 'Moderate'} day-1
          activation rate suggests{' '}
          {props.data.pct_activated_day1 > 50 ? 'excellent' : 'room to improve'} onboarding
          experience
        </p>
      </div>
    </div>
  );
};
