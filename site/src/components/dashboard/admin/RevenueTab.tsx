import { Component, For, Show, createSignal } from 'solid-js';
import {
  TrendingUp,
  TrendingDown,
  DollarSign,
  CreditCard,
  Users,
  Calendar,
  Sparkles,
} from '../../ui/Icons';
import { useAdminRevenue, useAdminCohorts } from '../../../lib/api-hooks';
import { CardSkeleton } from '../../ui/Skeleton';

const _formatCurrency = (cents: number) => {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(cents / 100);
};

const formatCompactCurrency = (value: number) => {
  if (value >= 1000000) return `$${(value / 1000000).toFixed(1)}M`;
  if (value >= 1000) return `$${(value / 1000).toFixed(1)}K`;
  return `$${value}`;
};

export const RevenueTab: Component = () => {
  const revenueQuery = useAdminRevenue();
  const cohortsQuery = useAdminCohorts();
  const [selectedPeriod, setSelectedPeriod] = createSignal<'7d' | '30d' | '90d' | '1y'>('30d');

  const revenue = () => revenueQuery.data;
  const _cohorts = () => cohortsQuery.data?.cohorts || [];

  const maxMonthlyRevenue = () => {
    const monthly = revenue()?.monthly_revenue || [];
    return Math.max(...monthly.map(m => m.revenue), 1);
  };

  return (
    <div class="animate-in fade-in slide-in-from-bottom-4 space-y-8 duration-500">
      <Show when={revenueQuery.isLoading}>
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={revenueQuery.isSuccess}>
        <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
          <div class="group relative overflow-hidden rounded-3xl border border-emerald-500/20 bg-gradient-to-br from-emerald-500/10 via-emerald-500/5 to-transparent p-8 shadow-2xl transition-all hover:border-emerald-500/40 hover:shadow-emerald-500/10">
            <div class="absolute -top-8 -right-8 h-32 w-32 rounded-full bg-emerald-500/10 blur-3xl transition-all group-hover:bg-emerald-500/20" />
            <div class="relative">
              <div class="mb-4 flex items-center justify-between">
                <div class="rounded-2xl bg-emerald-500/20 p-3">
                  <DollarSign size={20} class="text-emerald-400" />
                </div>
                <div class="flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-black text-emerald-400 uppercase">
                  <TrendingUp size={12} />
                  <span>Live</span>
                </div>
              </div>
              <p class="text-[10px] font-black tracking-widest text-emerald-400/60 uppercase">
                Monthly Recurring
              </p>
              <p class="mt-2 text-4xl font-black tracking-tight text-white">
                ${(revenue()?.mrr ?? 0).toLocaleString()}
              </p>
            </div>
          </div>

          <div class="group relative overflow-hidden rounded-3xl border border-indigo-500/20 bg-gradient-to-br from-indigo-500/10 via-indigo-500/5 to-transparent p-8 shadow-2xl transition-all hover:border-indigo-500/40">
            <div class="absolute -top-8 -right-8 h-32 w-32 rounded-full bg-indigo-500/10 blur-3xl transition-all group-hover:bg-indigo-500/20" />
            <div class="relative">
              <div class="mb-4 flex items-center justify-between">
                <div class="rounded-2xl bg-indigo-500/20 p-3">
                  <Calendar size={20} class="text-indigo-400" />
                </div>
              </div>
              <p class="text-[10px] font-black tracking-widest text-indigo-400/60 uppercase">
                Annual Run Rate
              </p>
              <p class="mt-2 text-4xl font-black tracking-tight text-white">
                ${(revenue()?.arr ?? 0).toLocaleString()}
              </p>
            </div>
          </div>

          <div class="group relative overflow-hidden rounded-3xl border border-amber-500/20 bg-gradient-to-br from-amber-500/10 via-amber-500/5 to-transparent p-8 shadow-2xl transition-all hover:border-amber-500/40">
            <div class="absolute -top-8 -right-8 h-32 w-32 rounded-full bg-amber-500/10 blur-3xl transition-all group-hover:bg-amber-500/20" />
            <div class="relative">
              <div class="mb-4 flex items-center justify-between">
                <div class="rounded-2xl bg-amber-500/20 p-3">
                  <CreditCard size={20} class="text-amber-400" />
                </div>
              </div>
              <p class="text-[10px] font-black tracking-widest text-amber-400/60 uppercase">
                Avg Transaction
              </p>
              <p class="mt-2 text-4xl font-black tracking-tight text-white">
                $
                {Math.round(
                  (revenue()?.monthly_revenue?.[0]?.revenue || 0) /
                    Math.max(1, revenue()?.monthly_revenue?.[0]?.transactions || 1)
                )}
              </p>
            </div>
          </div>

          <div class="group relative overflow-hidden rounded-3xl border border-violet-500/20 bg-gradient-to-br from-violet-500/10 via-violet-500/5 to-transparent p-8 shadow-2xl transition-all hover:border-violet-500/40">
            <div class="absolute -top-8 -right-8 h-32 w-32 rounded-full bg-violet-500/10 blur-3xl transition-all group-hover:bg-violet-500/20" />
            <div class="relative">
              <div class="mb-4 flex items-center justify-between">
                <div class="rounded-2xl bg-violet-500/20 p-3">
                  <Users size={20} class="text-violet-400" />
                </div>
              </div>
              <p class="text-[10px] font-black tracking-widest text-violet-400/60 uppercase">
                Paying Customers
              </p>
              <p class="mt-2 text-4xl font-black tracking-tight text-white">
                {(
                  revenue()?.monthly_revenue?.reduce((sum, m) => sum + m.transactions, 0) || 0
                ).toLocaleString()}
              </p>
            </div>
          </div>
        </div>

        <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
          <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl lg:col-span-2">
            <div class="mb-8 flex flex-col justify-between gap-4 sm:flex-row sm:items-center">
              <div>
                <h3 class="text-xl font-black tracking-tight text-white">Revenue Trend</h3>
                <p class="mt-1 text-xs font-medium text-slate-500">
                  Monthly revenue over the past 12 months
                </p>
              </div>
              <div class="flex items-center gap-1 rounded-2xl border border-white/10 bg-white/[0.02] p-1">
                <For each={['7d', '30d', '90d', '1y'] as const}>
                  {period => (
                    <button
                      onClick={() => setSelectedPeriod(period)}
                      class={`rounded-xl px-4 py-2 text-xs font-bold transition-all ${
                        selectedPeriod() === period
                          ? 'bg-white text-black'
                          : 'text-slate-400 hover:text-white'
                      }`}
                    >
                      {period}
                    </button>
                  )}
                </For>
              </div>
            </div>

            <div class="flex h-64 items-end gap-2">
              <For each={revenue()?.monthly_revenue?.slice(-12) || []}>
                {(month, index) => {
                  const height = (month.revenue / maxMonthlyRevenue()) * 100;
                  const isLast = index() === (revenue()?.monthly_revenue?.length || 0) - 1;
                  return (
                    <div class="group flex flex-1 flex-col items-center gap-2">
                      <div class="relative w-full">
                        <div
                          class={`w-full rounded-t-xl transition-all duration-500 ${
                            isLast
                              ? 'bg-gradient-to-t from-emerald-600 to-emerald-400'
                              : 'bg-gradient-to-t from-slate-700 to-slate-600 group-hover:from-indigo-600 group-hover:to-indigo-400'
                          }`}
                          style={{ height: `${Math.max(height, 8)}%`, 'min-height': '16px' }}
                        />
                        <div class="absolute -top-10 left-1/2 -translate-x-1/2 rounded-lg bg-white px-3 py-1.5 text-xs font-bold whitespace-nowrap text-black opacity-0 shadow-lg transition-opacity group-hover:opacity-100">
                          ${month.revenue.toLocaleString()}
                          <div class="absolute -bottom-1 left-1/2 h-2 w-2 -translate-x-1/2 rotate-45 bg-white" />
                        </div>
                      </div>
                      <span class="text-[9px] font-bold tracking-wider text-slate-500 uppercase">
                        {month.month.slice(5, 7)}
                      </span>
                    </div>
                  );
                }}
              </For>
            </div>
          </div>

          <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
            <h3 class="mb-6 text-xl font-black tracking-tight text-white">Revenue by Tier</h3>
            <div class="space-y-6">
              <div class="space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-sm font-bold text-white">Enterprise</span>
                  <span class="text-sm font-black text-amber-400">
                    {formatCompactCurrency((revenue()?.mrr || 0) * 0.6)}
                  </span>
                </div>
                <div class="h-3 overflow-hidden rounded-full bg-white/5">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-amber-600 to-amber-400 transition-all duration-1000"
                    style={{ width: '60%' }}
                  />
                </div>
              </div>
              <div class="space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-sm font-bold text-white">Team</span>
                  <span class="text-sm font-black text-purple-400">
                    {formatCompactCurrency((revenue()?.mrr || 0) * 0.3)}
                  </span>
                </div>
                <div class="h-3 overflow-hidden rounded-full bg-white/5">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-purple-600 to-purple-400 transition-all duration-1000"
                    style={{ width: '30%' }}
                  />
                </div>
              </div>
              <div class="space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-sm font-bold text-white">Pro</span>
                  <span class="text-sm font-black text-indigo-400">
                    {formatCompactCurrency((revenue()?.mrr || 0) * 0.1)}
                  </span>
                </div>
                <div class="h-3 overflow-hidden rounded-full bg-white/5">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-indigo-600 to-indigo-400 transition-all duration-1000"
                    style={{ width: '10%' }}
                  />
                </div>
              </div>
            </div>

            <div class="mt-8 border-t border-white/5 pt-6">
              <div class="flex items-center gap-3 rounded-2xl bg-gradient-to-r from-emerald-500/10 to-transparent p-4">
                <div class="rounded-xl bg-emerald-500/20 p-2">
                  <Sparkles size={16} class="text-emerald-400" />
                </div>
                <div>
                  <p class="text-[10px] font-black tracking-widest text-emerald-400/60 uppercase">
                    Growth Rate
                  </p>
                  <p class="text-lg font-black text-white">+12.5%</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
          <h3 class="mb-2 text-xl font-black tracking-tight text-white">Recent Transactions</h3>
          <p class="mb-6 text-xs font-medium text-slate-500">
            Latest subscription payments and upgrades
          </p>

          <div class="overflow-x-auto">
            <table class="w-full text-left">
              <thead>
                <tr class="border-b border-white/5 text-[10px] font-black tracking-widest text-slate-500 uppercase">
                  <th class="px-4 py-3">Month</th>
                  <th class="px-4 py-3">Revenue</th>
                  <th class="px-4 py-3">Transactions</th>
                  <th class="px-4 py-3">Avg Value</th>
                  <th class="px-4 py-3">Trend</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-white/5">
                <For each={revenue()?.monthly_revenue?.slice(-6).reverse() || []}>
                  {(month, index) => {
                    const monthlyRevenue = revenue()?.monthly_revenue || [];
                    const prevIndex = monthlyRevenue.length - 2 - index();
                    const prev = prevIndex >= 0 ? monthlyRevenue[prevIndex] : undefined;
                    const trend = prev ? ((month.revenue - prev.revenue) / prev.revenue) * 100 : 0;
                    return (
                      <tr class="group transition-colors hover:bg-white/[0.02]">
                        <td class="px-4 py-4">
                          <span class="text-sm font-bold text-white">{month.month}</span>
                        </td>
                        <td class="px-4 py-4">
                          <span class="text-sm font-black text-emerald-400">
                            ${month.revenue.toLocaleString()}
                          </span>
                        </td>
                        <td class="px-4 py-4">
                          <span class="text-sm text-slate-300">{month.transactions}</span>
                        </td>
                        <td class="px-4 py-4">
                          <span class="text-sm text-slate-300">
                            ${Math.round(month.revenue / Math.max(1, month.transactions))}
                          </span>
                        </td>
                        <td class="px-4 py-4">
                          <div
                            class={`flex items-center gap-1 text-xs font-bold ${trend >= 0 ? 'text-emerald-400' : 'text-rose-400'}`}
                          >
                            {trend >= 0 ? <TrendingUp size={14} /> : <TrendingDown size={14} />}
                            <span>{Math.abs(trend).toFixed(1)}%</span>
                          </div>
                        </td>
                      </tr>
                    );
                  }}
                </For>
              </tbody>
            </table>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default RevenueTab;
