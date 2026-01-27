import { Component } from 'solid-js';
import { Users, TrendingUp, Calendar, Zap } from 'lucide-solid';

interface EngagementData {
  dau: number;
  wau: number;
  mau: number;
  stickiness: {
    daily_to_monthly: string;
    weekly_to_monthly: string;
  };
}

interface EngagementMetricsProps {
  data: EngagementData;
}

export const EngagementMetrics: Component<EngagementMetricsProps> = (props) => {
  return (
    <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
      <div class="rounded-2xl border border-white/10 bg-gradient-to-br from-indigo-500/10 to-purple-500/5 p-6">
        <div class="mb-4 flex items-center justify-between">
          <div class="rounded-xl bg-indigo-500/20 p-3">
            <Users size={20} class="text-indigo-400" />
          </div>
          <span class="text-xs font-bold uppercase tracking-wider text-indigo-400">
            Daily
          </span>
        </div>
        <div class="text-3xl font-black text-white">{(props.data.dau ?? 0).toLocaleString()}</div>
        <p class="mt-2 text-sm text-slate-400">Daily Active Users</p>
      </div>

      <div class="rounded-2xl border border-white/10 bg-gradient-to-br from-cyan-500/10 to-blue-500/5 p-6">
        <div class="mb-4 flex items-center justify-between">
          <div class="rounded-xl bg-cyan-500/20 p-3">
            <Calendar size={20} class="text-cyan-400" />
          </div>
          <span class="text-xs font-bold uppercase tracking-wider text-cyan-400">Weekly</span>
        </div>
        <div class="text-3xl font-black text-white">{(props.data.wau ?? 0).toLocaleString()}</div>
        <p class="mt-2 text-sm text-slate-400">Weekly Active Users</p>
      </div>

      <div class="rounded-2xl border border-white/10 bg-gradient-to-br from-purple-500/10 to-pink-500/5 p-6">
        <div class="mb-4 flex items-center justify-between">
          <div class="rounded-xl bg-purple-500/20 p-3">
            <TrendingUp size={20} class="text-purple-400" />
          </div>
          <span class="text-xs font-bold uppercase tracking-wider text-purple-400">
            Monthly
          </span>
        </div>
        <div class="text-3xl font-black text-white">{(props.data.mau ?? 0).toLocaleString()}</div>
        <p class="mt-2 text-sm text-slate-400">Monthly Active Users</p>
      </div>

      <div class="rounded-2xl border border-white/10 bg-gradient-to-br from-emerald-500/10 to-teal-500/5 p-6">
        <div class="mb-4 flex items-center justify-between">
          <div class="rounded-xl bg-emerald-500/20 p-3">
            <Zap size={20} class="text-emerald-400" />
          </div>
          <span class="text-xs font-bold uppercase tracking-wider text-emerald-400">
            Stickiness
          </span>
        </div>
        <div class="text-3xl font-black text-white">
          {props.data.stickiness?.daily_to_monthly ?? '0'}%
        </div>
        <p class="mt-2 text-sm text-slate-400">
          DAU/MAU Ratio
          <span class="ml-2 text-xs text-slate-500">
            ({props.data.stickiness?.weekly_to_monthly ?? '0'}% WAU/MAU)
          </span>
        </p>
      </div>
    </div>
  );
};
