import { Component, Show, For, createSignal } from 'solid-js';
import {
  X,
  User,
  Mail,
  Calendar,
  Globe,
  Zap,
  Clock,
  CreditCard,
  Monitor,
  TrendingUp,
  TrendingDown,
  AlertTriangle,
  CheckCircle,
  Star,
} from '../../ui/Icons';
import { useAdminUserDetail } from '../../../lib/api-hooks';
import {
  formatRelativeTime,
  formatDate,
  formatTimeSaved,
  getTierBadgeColor,
} from '../../../lib/api';

interface CustomerDetailDrawerProps {
  userId: string | null;
  onClose: () => void;
}

export const CustomerDetailDrawer: Component<CustomerDetailDrawerProps> = props => {
  const [activeSection, setActiveSection] = createSignal<
    'overview' | 'machines' | 'usage' | 'billing'
  >('overview');

  const userDetailQuery = useAdminUserDetail(props.userId || '');
  const detail = () => userDetailQuery.data;

  const healthScoreColor = (score: number) => {
    if (score >= 80) return 'text-emerald-400 bg-emerald-500/10';
    if (score >= 60) return 'text-amber-400 bg-amber-500/10';
    return 'text-rose-400 bg-rose-500/10';
  };

  const SectionButton = (sectionProps: {
    id: typeof activeSection extends () => infer T ? T : never;
    label: string;
  }) => (
    <button
      onClick={() => setActiveSection(sectionProps.id)}
      class={`rounded-xl px-4 py-2 text-sm font-bold transition-all ${
        activeSection() === sectionProps.id
          ? 'bg-white text-black'
          : 'text-slate-400 hover:bg-white/5 hover:text-white'
      }`}
    >
      {sectionProps.label}
    </button>
  );

  return (
    <Show when={props.userId}>
      <div class="fixed inset-0 z-50 flex justify-end">
        <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={props.onClose} />

        <div class="animate-in slide-in-from-right relative w-full max-w-2xl overflow-y-auto border-l border-white/10 bg-[#0a0a0b] shadow-2xl duration-300">
          <div class="sticky top-0 z-10 border-b border-white/5 bg-[#0a0a0b]/95 p-6 backdrop-blur-xl">
            <div class="flex items-start justify-between">
              <div>
                <h2 class="text-2xl font-black tracking-tight text-white">Customer Detail</h2>
                <p class="mt-1 text-sm text-slate-500">360° view of customer activity</p>
              </div>
              <button
                onClick={props.onClose}
                class="rounded-xl p-2 text-slate-400 transition-colors hover:bg-white/5 hover:text-white"
              >
                <X size={20} />
              </button>
            </div>
          </div>

          <Show when={userDetailQuery.isLoading}>
            <div class="space-y-6 p-6">
              <div class="h-32 animate-pulse rounded-2xl bg-white/5" />
              <div class="h-48 animate-pulse rounded-2xl bg-white/5" />
              <div class="h-64 animate-pulse rounded-2xl bg-white/5" />
            </div>
          </Show>

          <Show when={userDetailQuery.isSuccess && detail()}>
            <div class="space-y-6 p-6">
              <div class="rounded-2xl border border-white/10 bg-gradient-to-br from-white/[0.03] to-transparent p-6">
                <div class="flex items-start gap-4">
                  <div class="flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-purple-600 text-2xl font-black text-white">
                    {detail()?.user.email.charAt(0).toUpperCase()}
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-3">
                      <h3 class="truncate text-xl font-black text-white">{detail()?.user.email}</h3>
                      <span
                        class={`shrink-0 rounded-full border px-2.5 py-0.5 text-[10px] font-black uppercase ${getTierBadgeColor(detail()?.license?.tier || 'free')}`}
                      >
                        {detail()?.license?.tier || 'free'}
                      </span>
                    </div>
                    <Show when={detail()?.user.company}>
                      <p class="mt-1 text-sm text-slate-400">{detail()?.user.company}</p>
                    </Show>
                    <div class="mt-3 flex items-center gap-4 text-xs text-slate-500">
                      <span class="flex items-center gap-1.5">
                        <Calendar size={12} />
                        Joined{' '}
                        {detail()?.user.created_at_relative ||
                          formatRelativeTime(detail()?.user.created_at || '')}
                      </span>
                      <Show when={detail()?.user.stripe_customer_id}>
                        <span class="flex items-center gap-1.5">
                          <CreditCard size={12} />
                          Stripe linked
                        </span>
                      </Show>
                    </div>
                  </div>
                </div>
              </div>

              <div class="grid grid-cols-4 gap-3">
                <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4 text-center">
                  <p class="text-2xl font-black text-white">{detail()?.machines.length || 0}</p>
                  <p class="mt-1 text-[10px] font-bold tracking-widest text-slate-500 uppercase">
                    Machines
                  </p>
                </div>
                <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4 text-center">
                  <p class="text-2xl font-black text-emerald-400">
                    {(detail()?.usage.summary?.total_commands || 0).toLocaleString()}
                  </p>
                  <p class="mt-1 text-[10px] font-bold tracking-widest text-slate-500 uppercase">
                    Commands
                  </p>
                </div>
                <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4 text-center">
                  <p class="text-2xl font-black text-indigo-400">
                    {formatTimeSaved(detail()?.usage.summary?.total_time_saved_ms || 0)}
                  </p>
                  <p class="mt-1 text-[10px] font-bold tracking-widest text-slate-500 uppercase">
                    Time Saved
                  </p>
                </div>
                <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4 text-center">
                  <p class="text-2xl font-black text-amber-400">
                    ${((detail()?.ltv.total_paid || 0) / 100).toFixed(0)}
                  </p>
                  <p class="mt-1 text-[10px] font-bold tracking-widest text-slate-500 uppercase">
                    LTV
                  </p>
                </div>
              </div>

              <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4">
                <div class="mb-4 flex items-center justify-between">
                  <h4 class="text-sm font-bold text-white">Engagement Health</h4>
                  <Show when={detail()?.engagement.is_power_user}>
                    <span class="flex items-center gap-1 rounded-full bg-amber-500/10 px-2 py-0.5 text-[10px] font-black text-amber-400 uppercase">
                      <Star size={10} />
                      Power User
                    </span>
                  </Show>
                  <Show when={detail()?.engagement.is_at_risk}>
                    <span class="flex items-center gap-1 rounded-full bg-rose-500/10 px-2 py-0.5 text-[10px] font-black text-rose-400 uppercase">
                      <AlertTriangle size={10} />
                      At Risk
                    </span>
                  </Show>
                </div>
                <div class="grid grid-cols-2 gap-4">
                  <div>
                    <p class="text-xs text-slate-500">Commands (7d)</p>
                    <p class="text-lg font-bold text-white">
                      {detail()?.engagement.commands_last_7d || 0}
                    </p>
                  </div>
                  <div>
                    <p class="text-xs text-slate-500">Commands (30d)</p>
                    <p class="text-lg font-bold text-white">
                      {detail()?.engagement.commands_last_30d || 0}
                    </p>
                  </div>
                  <div>
                    <p class="text-xs text-slate-500">Active Days (30d)</p>
                    <p class="text-lg font-bold text-white">
                      {detail()?.engagement.active_days_last_30d || 0}
                    </p>
                  </div>
                  <div>
                    <p class="text-xs text-slate-500">Avg Daily</p>
                    <p class="text-lg font-bold text-white">
                      {(detail()?.engagement.avg_daily_commands || 0).toFixed(1)}
                    </p>
                  </div>
                </div>
              </div>

              <div class="flex items-center gap-2 rounded-xl border border-white/5 bg-white/[0.02] p-1">
                <SectionButton id="overview" label="Overview" />
                <SectionButton id="machines" label="Machines" />
                <SectionButton id="usage" label="Usage" />
                <SectionButton id="billing" label="Billing" />
              </div>

              <Show when={activeSection() === 'machines'}>
                <div class="space-y-3">
                  <For each={detail()?.machines || []}>
                    {machine => (
                      <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4">
                        <div class="flex items-start justify-between">
                          <div class="flex items-center gap-3">
                            <div class="rounded-xl bg-slate-800 p-2">
                              <Monitor size={16} class="text-slate-400" />
                            </div>
                            <div>
                              <p class="text-sm font-bold text-white">
                                {machine.hostname || 'Unknown'}
                              </p>
                              <p class="text-xs text-slate-500">
                                {machine.os} • {machine.arch} • v{machine.omg_version}
                              </p>
                            </div>
                          </div>
                          <div
                            class={`rounded-full px-2 py-0.5 text-[10px] font-black uppercase ${
                              machine.is_active
                                ? 'bg-emerald-500/10 text-emerald-400'
                                : 'bg-slate-500/10 text-slate-400'
                            }`}
                          >
                            {machine.is_active ? 'Active' : 'Inactive'}
                          </div>
                        </div>
                        <div class="mt-3 flex items-center gap-4 text-xs text-slate-500">
                          <span>First seen: {formatRelativeTime(machine.first_seen_at)}</span>
                          <span>Last seen: {formatRelativeTime(machine.last_seen_at)}</span>
                        </div>
                      </div>
                    )}
                  </For>
                  <Show when={!detail()?.machines.length}>
                    <div class="py-8 text-center text-slate-500">No machines registered</div>
                  </Show>
                </div>
              </Show>

              <Show when={activeSection() === 'usage'}>
                <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4">
                  <h4 class="mb-4 text-sm font-bold text-white">Recent Activity (30 days)</h4>
                  <div class="flex h-32 items-end gap-1">
                    <For each={detail()?.usage.daily?.slice(-30) || []}>
                      {day => {
                        const maxCmd = Math.max(
                          ...(detail()?.usage.daily?.map(d => d.commands_run) || [1])
                        );
                        const height = (day.commands_run / maxCmd) * 100;
                        return (
                          <div class="group relative flex-1">
                            <div
                              class="w-full rounded-t bg-gradient-to-t from-indigo-600 to-indigo-400 transition-all group-hover:from-indigo-500 group-hover:to-indigo-300"
                              style={{ height: `${Math.max(height, 4)}%`, 'min-height': '4px' }}
                            />
                            <div class="absolute -top-8 left-1/2 -translate-x-1/2 rounded bg-white px-2 py-1 text-[10px] font-bold whitespace-nowrap text-black opacity-0 transition-opacity group-hover:opacity-100">
                              {day.commands_run}
                            </div>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                </div>
              </Show>

              <Show when={activeSection() === 'billing'}>
                <div class="space-y-4">
                  <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4">
                    <div class="mb-4 flex items-center justify-between">
                      <h4 class="text-sm font-bold text-white">Subscription</h4>
                      <span
                        class={`rounded-full px-2 py-0.5 text-[10px] font-black uppercase ${
                          detail()?.license?.status === 'active'
                            ? 'bg-emerald-500/10 text-emerald-400'
                            : 'bg-slate-500/10 text-slate-400'
                        }`}
                      >
                        {detail()?.license?.status || 'N/A'}
                      </span>
                    </div>
                    <div class="grid grid-cols-2 gap-4 text-sm">
                      <div>
                        <p class="text-slate-500">Current Plan</p>
                        <p class="font-bold text-white capitalize">
                          {detail()?.license?.tier || 'Free'}
                        </p>
                      </div>
                      <div>
                        <p class="text-slate-500">Max Seats</p>
                        <p class="font-bold text-white">{detail()?.license?.max_seats || 1}</p>
                      </div>
                      <div>
                        <p class="text-slate-500">Invoices</p>
                        <p class="font-bold text-white">{detail()?.ltv.invoice_count || 0}</p>
                      </div>
                      <div>
                        <p class="text-slate-500">Months Active</p>
                        <p class="font-bold text-white">{detail()?.ltv.months_subscribed || 0}</p>
                      </div>
                    </div>
                  </div>
                </div>
              </Show>

              <Show when={activeSection() === 'overview'}>
                <div class="rounded-2xl border border-white/5 bg-white/[0.02] p-4">
                  <h4 class="mb-4 text-sm font-bold text-white">License Details</h4>
                  <div class="space-y-3 text-sm">
                    <div class="flex items-center justify-between">
                      <span class="text-slate-500">License Key</span>
                      <code class="rounded bg-black/20 px-2 py-1 font-mono text-xs text-slate-300">
                        {detail()?.license?.license_key?.slice(0, 8)}...
                      </code>
                    </div>
                    <div class="flex items-center justify-between">
                      <span class="text-slate-500">Expires</span>
                      <span class="text-white">
                        {detail()?.license?.expires_at
                          ? formatDate(detail()?.license?.expires_at)
                          : 'Never'}
                      </span>
                    </div>
                    <div class="flex items-center justify-between">
                      <span class="text-slate-500">First Active</span>
                      <span class="text-white">
                        {detail()?.usage.summary?.first_active
                          ? formatDate(detail()?.usage.summary?.first_active)
                          : 'N/A'}
                      </span>
                    </div>
                    <div class="flex items-center justify-between">
                      <span class="text-slate-500">Last Active</span>
                      <span class="text-white">
                        {detail()?.usage.summary?.last_active
                          ? formatRelativeTime(detail()!.usage.summary!.last_active!)
                          : 'N/A'}
                      </span>
                    </div>
                  </div>
                </div>
              </Show>
            </div>
          </Show>
        </div>
      </div>
    </Show>
  );
};

export default CustomerDetailDrawer;
