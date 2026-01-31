import { Component, Show, For, createMemo, createSignal, onMount } from 'solid-js';
import { 
  DollarSign, 
  TrendingUp, 
  TrendingDown, 
  Calendar, 
  CreditCard, 
  Users, 
  Sparkles,
  ArrowUpRight,
  ArrowDownRight
} from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface MonthlyRevenue {
  month: string;
  revenue: number;
  transactions: number;
}

interface RevenueData {
  mrr: number;
  arr: number;
  monthly_revenue: MonthlyRevenue[];
}

interface RevenueByTier {
  tier: string;
  revenue: number;
  percentage: number;
  customers: number;
}

interface RevenueDashboardProps {
  data: RevenueData;
  revenueByTier?: RevenueByTier[];
  growthRate?: number;
  previousMRR?: number;
}

const TIER_COLORS: Record<string, { gradient: string; glow: string; accent: string }> = {
  enterprise: {
    gradient: 'linear-gradient(135deg, var(--color-solar-600), var(--color-solar-400))',
    glow: 'rgba(245, 158, 11, 0.3)',
    accent: 'var(--color-solar-400)',
  },
  team: {
    gradient: 'linear-gradient(135deg, var(--color-photon-600), var(--color-photon-400))',
    glow: 'rgba(176, 109, 232, 0.3)',
    accent: 'var(--color-photon-400)',
  },
  pro: {
    gradient: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))',
    glow: 'rgba(99, 102, 241, 0.3)',
    accent: 'var(--color-indigo-400)',
  },
  free: {
    gradient: 'linear-gradient(135deg, var(--color-nebula-600), var(--color-nebula-400))',
    glow: 'rgba(113, 113, 122, 0.2)',
    accent: 'var(--color-nebula-400)',
  },
};

function formatCurrency(value: number): string {
  if (value >= 1000000) return `$${(value / 1000000).toFixed(1)}M`;
  if (value >= 1000) return `$${(value / 1000).toFixed(1)}K`;
  return `$${value.toLocaleString()}`;
}

function formatMonth(monthStr: string): string {
  const parts = monthStr.split('-');
  if (parts.length < 2) return monthStr;
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const monthIndex = parseInt(parts[1], 10) - 1;
  return months[monthIndex] || monthStr;
}

export const RevenueDashboard: Component<RevenueDashboardProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [hoveredBar, setHoveredBar] = createSignal<number | null>(null);
  const [selectedPeriod, setSelectedPeriod] = createSignal<'6m' | '12m' | 'all'>('12m');

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const monthlyData = createMemo(() => {
    const data = props.data.monthly_revenue || [];
    const periodMonths = selectedPeriod() === '6m' ? 6 : selectedPeriod() === '12m' ? 12 : data.length;
    return data.slice(-periodMonths);
  });

  const maxRevenue = createMemo(() => 
    Math.max(...monthlyData().map(m => m.revenue), 1)
  );

  const mrrChange = createMemo(() => {
    if (!props.previousMRR || props.previousMRR === 0) return null;
    return ((props.data.mrr - props.previousMRR) / props.previousMRR) * 100;
  });

  const avgTransactionValue = createMemo(() => {
    const recent = props.data.monthly_revenue?.[0];
    if (!recent || recent.transactions === 0) return 0;
    return Math.round(recent.revenue / recent.transactions);
  });

  const totalTransactions = createMemo(() => 
    props.data.monthly_revenue?.reduce((sum, m) => sum + m.transactions, 0) || 0
  );

  const defaultTiers = createMemo<RevenueByTier[]>(() => {
    if (props.revenueByTier) return props.revenueByTier;
    const mrr = props.data.mrr || 0;
    return [
      { tier: 'enterprise', revenue: mrr * 0.6, percentage: 60, customers: Math.round(mrr * 0.003) },
      { tier: 'team', revenue: mrr * 0.3, percentage: 30, customers: Math.round(mrr * 0.01) },
      { tier: 'pro', revenue: mrr * 0.1, percentage: 10, customers: Math.round(mrr * 0.011) },
    ];
  });

  return (
    <div class="space-y-6">
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <div class="group relative overflow-hidden rounded-2xl border border-aurora-500/20 bg-gradient-to-br from-aurora-500/10 via-aurora-500/5 to-transparent p-6 shadow-2xl transition-all hover:border-aurora-500/40 hover:shadow-aurora-500/10">
          <div class="absolute -top-8 -right-8 h-32 w-32 rounded-full bg-aurora-500/10 blur-3xl transition-all group-hover:bg-aurora-500/20" />
          <div class="relative">
            <div class="mb-4 flex items-center justify-between">
              <div class="rounded-xl bg-aurora-500/20 p-3">
                <DollarSign size={20} class="text-aurora-400" />
              </div>
              <Show when={mrrChange() !== null}>
                <div
                  class="flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-black"
                  style={{
                    color: mrrChange()! >= 0 ? 'var(--color-aurora-400)' : 'var(--color-flare-400)',
                    background: mrrChange()! >= 0 ? 'rgba(16, 185, 129, 0.1)' : 'rgba(239, 68, 68, 0.1)',
                  }}
                >
                  {mrrChange()! >= 0 ? <ArrowUpRight size={10} /> : <ArrowDownRight size={10} />}
                  <span class="tabular-nums">{Math.abs(mrrChange()!).toFixed(1)}%</span>
                </div>
              </Show>
            </div>
            <p class="text-[10px] font-black tracking-widest text-aurora-400/60 uppercase">
              Monthly Recurring
            </p>
            <p
              class={cn(
                'mt-2 text-4xl font-black tracking-tight text-white transition-all duration-700',
                mounted() ? 'opacity-100' : 'opacity-0'
              )}
            >
              ${(props.data.mrr ?? 0).toLocaleString()}
            </p>
          </div>
        </div>

        <div class="group relative overflow-hidden rounded-2xl border border-indigo-500/20 bg-gradient-to-br from-indigo-500/10 via-indigo-500/5 to-transparent p-6 shadow-2xl transition-all hover:border-indigo-500/40">
          <div class="absolute -top-8 -right-8 h-32 w-32 rounded-full bg-indigo-500/10 blur-3xl transition-all group-hover:bg-indigo-500/20" />
          <div class="relative">
            <div class="mb-4 rounded-xl bg-indigo-500/20 p-3 w-fit">
              <Calendar size={20} class="text-indigo-400" />
            </div>
            <p class="text-[10px] font-black tracking-widest text-indigo-400/60 uppercase">
              Annual Run Rate
            </p>
            <p
              class={cn(
                'mt-2 text-4xl font-black tracking-tight text-white transition-all duration-700',
                mounted() ? 'opacity-100' : 'opacity-0'
              )}
            >
              ${(props.data.arr ?? 0).toLocaleString()}
            </p>
          </div>
        </div>

        <div class="group relative overflow-hidden rounded-2xl border border-solar-500/20 bg-gradient-to-br from-solar-500/10 via-solar-500/5 to-transparent p-6 shadow-2xl transition-all hover:border-solar-500/40">
          <div class="absolute -top-8 -right-8 h-32 w-32 rounded-full bg-solar-500/10 blur-3xl transition-all group-hover:bg-solar-500/20" />
          <div class="relative">
            <div class="mb-4 rounded-xl bg-solar-500/20 p-3 w-fit">
              <CreditCard size={20} class="text-solar-400" />
            </div>
            <p class="text-[10px] font-black tracking-widest text-solar-400/60 uppercase">
              Avg Transaction
            </p>
            <p
              class={cn(
                'mt-2 text-4xl font-black tracking-tight text-white transition-all duration-700',
                mounted() ? 'opacity-100' : 'opacity-0'
              )}
            >
              ${avgTransactionValue()}
            </p>
          </div>
        </div>

        <div class="group relative overflow-hidden rounded-2xl border border-photon-500/20 bg-gradient-to-br from-photon-500/10 via-photon-500/5 to-transparent p-6 shadow-2xl transition-all hover:border-photon-500/40">
          <div class="absolute -top-8 -right-8 h-32 w-32 rounded-full bg-photon-500/10 blur-3xl transition-all group-hover:bg-photon-500/20" />
          <div class="relative">
            <div class="mb-4 rounded-xl bg-photon-500/20 p-3 w-fit">
              <Users size={20} class="text-photon-400" />
            </div>
            <p class="text-[10px] font-black tracking-widest text-photon-400/60 uppercase">
              Total Transactions
            </p>
            <p
              class={cn(
                'mt-2 text-4xl font-black tracking-tight text-white transition-all duration-700',
                mounted() ? 'opacity-100' : 'opacity-0'
              )}
            >
              {totalTransactions().toLocaleString()}
            </p>
          </div>
        </div>
      </div>

      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl lg:col-span-2">
          <div class="mb-6 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div>
              <h3 class="text-xl font-bold tracking-tight text-white">Revenue Trend</h3>
              <p class="mt-1 text-xs font-medium text-nebula-500">Monthly revenue over time</p>
            </div>
            <div class="flex items-center gap-1 rounded-xl border border-white/10 bg-white/[0.02] p-1">
              <For each={['6m', '12m', 'all'] as const}>
                {(period) => (
                  <button
                    onClick={() => setSelectedPeriod(period)}
                    class={cn(
                      'rounded-lg px-4 py-2 text-xs font-bold transition-all',
                      selectedPeriod() === period
                        ? 'bg-white text-black'
                        : 'text-nebula-400 hover:text-white'
                    )}
                  >
                    {period === 'all' ? 'All' : period.toUpperCase()}
                  </button>
                )}
              </For>
            </div>
          </div>

          <div class="h-64 flex items-end gap-2">
            <For each={monthlyData()}>
              {(month, index) => {
                const height = (month.revenue / maxRevenue()) * 100;
                const isLast = index() === monthlyData().length - 1;
                const isHovered = hoveredBar() === index();

                return (
                  <div
                    class="group flex flex-1 flex-col items-center gap-2"
                    onMouseEnter={() => setHoveredBar(index())}
                    onMouseLeave={() => setHoveredBar(null)}
                  >
                    <div class="relative w-full">
                      <div
                        class={cn(
                          'w-full rounded-t-xl transition-all duration-500',
                          isLast
                            ? 'bg-gradient-to-t from-aurora-600 to-aurora-400'
                            : 'bg-gradient-to-t from-nebula-700 to-nebula-600'
                        )}
                        style={{
                          height: mounted() ? `${Math.max(height, 8)}%` : '0%',
                          'min-height': '16px',
                          'box-shadow': isHovered
                            ? isLast
                              ? '0 0 20px rgba(16, 185, 129, 0.4)'
                              : '0 0 20px rgba(99, 102, 241, 0.3)'
                            : undefined,
                          transform: isHovered ? 'scaleY(1.02)' : undefined,
                          'transform-origin': 'bottom',
                        }}
                      />

                      <div
                        class={cn(
                          'absolute -top-16 left-1/2 -translate-x-1/2 rounded-xl border border-white/10 px-3 py-2',
                          'text-xs shadow-2xl backdrop-blur-md whitespace-nowrap',
                          'opacity-0 scale-95 transition-all duration-200',
                          'group-hover:opacity-100 group-hover:scale-100'
                        )}
                        style={{ background: 'var(--bg-overlay, rgba(10, 10, 11, 0.95))' }}
                      >
                        <div class="text-[10px] font-bold uppercase tracking-wider text-nebula-500">
                          {month.month}
                        </div>
                        <div class="text-sm font-black text-white tabular-nums">
                          ${month.revenue.toLocaleString()}
                        </div>
                        <div class="text-[10px] text-nebula-400">
                          {month.transactions} transactions
                        </div>
                        <div class="absolute -bottom-1 left-1/2 h-2 w-2 -translate-x-1/2 rotate-45 border-b border-r border-white/10 bg-[#0a0a0b]" />
                      </div>
                    </div>
                    <span class="text-[9px] font-bold tracking-wider text-nebula-500 uppercase">
                      {formatMonth(month.month)}
                    </span>
                  </div>
                );
              }}
            </For>
          </div>
        </div>

        <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl">
          <h3 class="mb-6 text-xl font-bold tracking-tight text-white">Revenue by Tier</h3>
          
          <div class="space-y-6">
            <For each={defaultTiers()}>
              {(tier) => {
                const colors = TIER_COLORS[tier.tier] || TIER_COLORS.free;
                
                return (
                  <div class="space-y-3">
                    <div class="flex items-center justify-between">
                      <span class="text-sm font-bold text-white capitalize">{tier.tier}</span>
                      <span class="text-sm font-black tabular-nums" style={{ color: colors.accent }}>
                        {formatCurrency(tier.revenue)}
                      </span>
                    </div>
                    <div class="h-3 overflow-hidden rounded-full bg-white/5">
                      <div
                        class="h-full rounded-full transition-all duration-1000"
                        style={{
                          width: mounted() ? `${tier.percentage}%` : '0%',
                          background: colors.gradient,
                          'box-shadow': `0 0 10px ${colors.glow}`,
                        }}
                      />
                    </div>
                    <div class="flex items-center justify-between text-xs text-nebula-500">
                      <span>{tier.percentage}% of MRR</span>
                      <span>{tier.customers} customers</span>
                    </div>
                  </div>
                );
              }}
            </For>
          </div>

          <Show when={props.growthRate !== undefined}>
            <div class="mt-8 border-t border-white/5 pt-6">
              <div
                class="flex items-center gap-3 rounded-xl p-4"
                style={{
                  background: props.growthRate! >= 0
                    ? 'linear-gradient(135deg, rgba(16, 185, 129, 0.1), transparent)'
                    : 'linear-gradient(135deg, rgba(239, 68, 68, 0.1), transparent)',
                }}
              >
                <div
                  class="rounded-xl p-2"
                  style={{
                    background: props.growthRate! >= 0
                      ? 'rgba(16, 185, 129, 0.2)'
                      : 'rgba(239, 68, 68, 0.2)',
                  }}
                >
                  {props.growthRate! >= 0 ? (
                    <Sparkles size={16} class="text-aurora-400" />
                  ) : (
                    <TrendingDown size={16} class="text-flare-400" />
                  )}
                </div>
                <div>
                  <p class="text-[10px] font-black tracking-widest text-nebula-500 uppercase">
                    Growth Rate
                  </p>
                  <p
                    class="text-lg font-black"
                    style={{
                      color: props.growthRate! >= 0
                        ? 'var(--color-aurora-400)'
                        : 'var(--color-flare-400)',
                    }}
                  >
                    {props.growthRate! >= 0 ? '+' : ''}{props.growthRate!.toFixed(1)}%
                  </p>
                </div>
              </div>
            </div>
          </Show>
        </div>
      </div>

      <Show when={(props.data.monthly_revenue?.length || 0) > 0}>
        <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl">
          <h3 class="mb-2 text-xl font-bold tracking-tight text-white">Recent Transactions</h3>
          <p class="mb-6 text-xs font-medium text-nebula-500">Monthly breakdown</p>

          <div class="overflow-x-auto">
            <table class="w-full text-left">
              <thead>
                <tr class="border-b border-white/5 text-[10px] font-black tracking-widest text-nebula-500 uppercase">
                  <th class="px-4 py-3">Month</th>
                  <th class="px-4 py-3">Revenue</th>
                  <th class="px-4 py-3">Transactions</th>
                  <th class="px-4 py-3">Avg Value</th>
                  <th class="px-4 py-3">Trend</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-white/5">
                <For each={props.data.monthly_revenue?.slice(-6).reverse() || []}>
                  {(month, index) => {
                    const monthlyRevenue = props.data.monthly_revenue || [];
                    const currentIndex = monthlyRevenue.length - 1 - index();
                    const prevMonth = currentIndex > 0 ? monthlyRevenue[currentIndex - 1] : undefined;
                    const trend = prevMonth && prevMonth.revenue > 0
                      ? ((month.revenue - prevMonth.revenue) / prevMonth.revenue) * 100
                      : 0;
                    
                    return (
                      <tr class="group transition-colors hover:bg-white/[0.02]">
                        <td class="px-4 py-4">
                          <span class="text-sm font-bold text-white">{month.month}</span>
                        </td>
                        <td class="px-4 py-4">
                          <span class="text-sm font-black text-aurora-400 tabular-nums">
                            ${month.revenue.toLocaleString()}
                          </span>
                        </td>
                        <td class="px-4 py-4">
                          <span class="text-sm text-nebula-300 tabular-nums">{month.transactions}</span>
                        </td>
                        <td class="px-4 py-4">
                          <span class="text-sm text-nebula-300 tabular-nums">
                            ${Math.round(month.revenue / Math.max(1, month.transactions))}
                          </span>
                        </td>
                        <td class="px-4 py-4">
                          <div
                            class="flex items-center gap-1 text-xs font-bold"
                            style={{
                              color: trend >= 0 ? 'var(--color-aurora-400)' : 'var(--color-flare-400)',
                            }}
                          >
                            {trend >= 0 ? <TrendingUp size={14} /> : <TrendingDown size={14} />}
                            <span class="tabular-nums">{Math.abs(trend).toFixed(1)}%</span>
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

export default RevenueDashboard;
