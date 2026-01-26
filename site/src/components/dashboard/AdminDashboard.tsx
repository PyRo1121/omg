import { Component, createSignal, For, Show, Switch, Match } from 'solid-js';
import {
  Activity,
  Terminal,
  Users,
  Zap,
  Globe,
  Clock,
  TrendingUp,
  Search,
  Filter,
  Download,
  BarChart3,
  CreditCard,
  History,
  AlertCircle,
  Eye,
} from '../ui/Icons';
import * as api from '../../lib/api';
import { useAdminDashboard, useAdminFirehose, useAdminCRMUsers } from '../../lib/api-hooks';
import { StatCard } from './analytics/StatCard';
import { ActivityHeatmap } from '../ui/Chart';
import { CardSkeleton } from '../ui/Skeleton';
import { CommandStream } from './admin/CommandStream';
import { GlobalPresence } from './admin/GlobalPresence';
import { DocsAnalytics } from './admin/DocsAnalytics';
import { RevenueTab } from './admin/RevenueTab';
import { AuditLogTab } from './admin/AuditLogTab';
import { CustomerDetailDrawer } from './admin/CustomerDetailDrawer';

type AdminTab = 'overview' | 'crm' | 'analytics' | 'revenue' | 'audit';

export const AdminDashboard: Component = () => {
  const [activeTab, setActiveTab] = createSignal<AdminTab>('overview');
  const [crmPage, setCrmPage] = createSignal(1);
  const [crmSearch, setCrmSearch] = createSignal('');
  const [selectedUserId, setSelectedUserId] = createSignal<string | null>(null);

  // TanStack Queries
  const dashboardQuery = useAdminDashboard();
  const firehoseQuery = useAdminFirehose(50);
  const crmUsersQuery = useAdminCRMUsers(crmPage(), 25, crmSearch());

  const adminData = () => dashboardQuery.data;
  const events = () => firehoseQuery.data?.events || [];
  const crmUsers = () => crmUsersQuery.data?.users || [];
  const crmPagination = () => crmUsersQuery.data?.pagination;

  const TabButton = (props: { id: AdminTab; icon: any; label: string }) => (
    <button
      onClick={() => setActiveTab(props.id)}
      class={`flex items-center gap-3 rounded-xl px-6 py-3 font-bold transition-all ${
        activeTab() === props.id
          ? 'scale-[1.02] bg-white text-black shadow-lg'
          : 'text-slate-400 hover:bg-white/5 hover:text-white'
      }`}
    >
      <props.icon size={18} />
      <span>{props.label}</span>
    </button>
  );

  return (
    <div class="space-y-8 pb-20">
      {/* Header */}
      <div class="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h1 class="text-4xl font-black tracking-tight text-white">System Command</h1>
          <p class="mt-2 font-medium text-slate-400">
            Global infrastructure, revenue, and fleet telemetry.
          </p>
        </div>

        <div class="flex items-center gap-3">
          <button class="flex items-center gap-2 rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-3 text-sm font-bold text-white transition-all hover:bg-white/[0.08]">
            <Download size={16} />
            Export Data
          </button>
        </div>
      </div>

      {/* Navigation */}
      <div class="no-scrollbar flex items-center gap-2 overflow-x-auto rounded-2xl border border-white/5 bg-white/[0.02] p-1.5">
        <TabButton id="overview" icon={Activity} label="Overview" />
        <TabButton id="crm" icon={Users} label="CRM" />
        <TabButton id="analytics" icon={BarChart3} label="Analytics" />
        <TabButton id="revenue" icon={CreditCard} label="Revenue" />
        <TabButton id="audit" icon={History} label="Audit Log" />
      </div>

      <Show when={dashboardQuery.isLoading}>
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={dashboardQuery.isSuccess}>
        <Switch>
          <Match when={activeTab() === 'overview'}>
            <div class="animate-in fade-in slide-in-from-bottom-4 space-y-8 duration-500">
              {/* Metrics Grid */}
              <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
                <StatCard
                  title="Total Users"
                  value={(adminData()?.overview?.total_users ?? 0).toLocaleString()}
                  icon={<Users size={20} />}
                  trend={{ value: 8.2, isUp: true }}
                />
                <StatCard
                  title="Monthly Revenue"
                  value={`$${(adminData()?.overview?.mrr ?? 0).toLocaleString()}`}
                  icon={<CreditCard size={20} />}
                  trend={{ value: 12.5, isUp: true }}
                />
                <StatCard
                  title="Active Fleet"
                  value={(adminData()?.overview?.active_machines ?? 0).toLocaleString()}
                  icon={<Globe size={20} />}
                />
                <StatCard
                  title="Command Volume"
                  value={(adminData()?.overview?.total_commands ?? 0).toLocaleString()}
                  icon={<Zap size={20} />}
                  trend={{ value: 4.1, isUp: true }}
                />
              </div>

              <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
                {/* Real-time Command Stream */}
                <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl lg:col-span-2">
                  <div class="mb-8 flex items-center justify-between">
                    <div>
                      <h3 class="text-xl font-bold tracking-widest text-white uppercase">
                        System Command Stream
                      </h3>
                      <p class="mt-1 text-xs font-medium text-slate-500">
                        Live telemetry and global CLI execution flow
                      </p>
                    </div>
                    <div class="flex items-center gap-2 rounded-full bg-emerald-500/10 px-3 py-1 text-[10px] font-black text-emerald-400 uppercase">
                      <div class="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-500" />
                      Live
                    </div>
                  </div>

                  <CommandStream events={events()} />
                </div>

                {/* Global Presence & Health */}
                <div class="space-y-6">
                  <GlobalPresence
                    data={adminData()?.geo_distribution || []}
                    totalNodes={adminData()?.overview?.active_machines || 1}
                  />

                  <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                    <h3 class="mb-6 text-lg font-bold tracking-widest text-white uppercase">
                      Global Activity
                    </h3>
                    <ActivityHeatmap
                      data={
                        adminData()?.daily_active_users?.map(d => ({
                          day: new Date(d.date).getDay(),
                          hour: 12, // Aggregate for simplicity
                          value: d.commands,
                        })) || []
                      }
                    />
                  </div>

                  <div class="rounded-3xl border border-white/5 bg-gradient-to-br from-indigo-500/10 to-transparent p-8 shadow-2xl">
                    <h3 class="mb-4 text-lg font-bold tracking-widest text-white uppercase">
                      Command Health
                    </h3>
                    <div class="flex h-32 items-end gap-2">
                      <div
                        class="group relative flex-1 rounded-t-lg bg-emerald-500/20"
                        style={{
                          height: `${adminData()?.overview?.command_health?.success || 94}%`,
                        }}
                      >
                        <div class="absolute inset-x-0 bottom-full mb-2 text-center text-[10px] font-bold text-emerald-400 opacity-0 transition-opacity group-hover:opacity-100">
                          {adminData()?.overview?.command_health?.success || 94}% Success
                        </div>
                      </div>
                      <div
                        class="group relative flex-1 rounded-t-lg bg-rose-500/20"
                        style={{
                          height: `${adminData()?.overview?.command_health?.failure || 6}%`,
                        }}
                      >
                        <div class="absolute inset-x-0 bottom-full mb-2 text-center text-[10px] font-bold text-rose-400 opacity-0 transition-opacity group-hover:opacity-100">
                          {adminData()?.overview?.command_health?.failure || 6}% Error
                        </div>
                      </div>
                    </div>
                    <div class="mt-4 flex justify-between text-[10px] font-black tracking-widest text-slate-500 uppercase">
                      <span>Operations</span>
                      <span>Fault Rate</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </Match>

          <Match when={activeTab() === 'crm'}>
            <div class="animate-in fade-in slide-in-from-bottom-4 rounded-3xl border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl duration-500">
              <div class="mb-10 flex flex-col justify-between gap-6 md:flex-row md:items-center">
                <div>
                  <h3 class="text-2xl font-black tracking-tight text-white">Customer CRM</h3>
                  <p class="text-sm font-medium text-slate-500">
                    {crmPagination()?.total || 0} customers • Manage subscriptions and engagement
                  </p>
                </div>
                <div class="relative w-full max-w-md">
                  <Search
                    class="absolute top-1/2 left-4 -translate-y-1/2 text-slate-500"
                    size={18}
                  />
                  <input
                    type="text"
                    placeholder="Search by email, company or ID..."
                    value={crmSearch()}
                    onInput={e => {
                      setCrmSearch(e.currentTarget.value);
                      setCrmPage(1);
                    }}
                    class="w-full rounded-2xl border border-white/10 bg-white/5 py-3 pr-4 pl-12 text-white placeholder-slate-500 transition-all focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
                  />
                </div>
              </div>

              <Show when={crmUsersQuery.isLoading}>
                <div class="space-y-4">
                  <CardSkeleton />
                  <CardSkeleton />
                  <CardSkeleton />
                </div>
              </Show>

              <Show when={crmUsersQuery.isSuccess}>
                <div class="overflow-x-auto">
                  <table class="w-full text-left">
                    <thead>
                      <tr class="border-b border-white/5 text-[10px] font-black tracking-widest text-slate-500 uppercase">
                        <th class="px-6 py-4">User</th>
                        <th class="px-6 py-4">Tier</th>
                        <th class="px-6 py-4">Status</th>
                        <th class="px-6 py-4">Machines</th>
                        <th class="px-6 py-4">Commands</th>
                        <th class="px-6 py-4">Joined</th>
                        <th class="px-6 py-4"></th>
                      </tr>
                    </thead>
                    <tbody class="divide-y divide-white/5">
                      <For each={crmUsers()}>
                        {user => {
                          const tierColors: Record<string, string> = {
                            enterprise: 'text-amber-400',
                            team: 'text-purple-400',
                            pro: 'text-indigo-400',
                            free: 'text-slate-400',
                          };
                          const statusColors: Record<string, string> = {
                            active: 'bg-emerald-500/10 text-emerald-400',
                            suspended: 'bg-amber-500/10 text-amber-400',
                            cancelled: 'bg-rose-500/10 text-rose-400',
                          };
                          return (
                            <tr class="group transition-colors hover:bg-white/[0.02]">
                              <td class="px-6 py-4">
                                <div class="text-sm font-bold text-white">{user.email}</div>
                                <div class="font-mono text-[10px] text-slate-500 uppercase">
                                  {user.company || user.id.slice(0, 8)}
                                </div>
                              </td>
                              <td class="px-6 py-4">
                                <span
                                  class={`text-xs font-black uppercase ${tierColors[user.tier] || 'text-slate-400'}`}
                                >
                                  {user.tier}
                                </span>
                              </td>
                              <td class="px-6 py-4">
                                <span
                                  class={`rounded-full px-2 py-0.5 text-[10px] font-black uppercase ${statusColors[user.status] || statusColors.active}`}
                                >
                                  {user.status}
                                </span>
                              </td>
                              <td class="px-6 py-4 text-sm text-slate-300">
                                {user.machine_count || 0}
                              </td>
                              <td class="px-6 py-4 text-sm text-slate-300">
                                {(user.total_commands || 0).toLocaleString()}
                              </td>
                              <td class="px-6 py-4 text-sm text-slate-500">
                                {user.created_at ? api.formatRelativeTime(user.created_at) : 'N/A'}
                              </td>
                              <td class="px-6 py-4">
                                <button
                                  onClick={() => setSelectedUserId(user.id)}
                                  class="rounded-lg p-2 text-slate-500 transition-colors hover:bg-white/5 hover:text-white"
                                >
                                  <Eye size={16} />
                                </button>
                              </td>
                            </tr>
                          );
                        }}
                      </For>
                    </tbody>
                  </table>
                </div>

                <Show when={crmUsers().length === 0}>
                  <div class="py-12 text-center">
                    <Users size={48} class="mx-auto mb-4 text-slate-600" />
                    <p class="font-medium text-slate-500">No customers found</p>
                    <p class="mt-1 text-xs text-slate-600">
                      {crmSearch() ? 'Try a different search term' : 'Customers will appear here'}
                    </p>
                  </div>
                </Show>

                <Show when={(crmPagination()?.pages || 1) > 1}>
                  <div class="mt-8 flex items-center justify-between border-t border-white/5 pt-6">
                    <p class="text-sm text-slate-500">
                      Page {crmPage()} of {crmPagination()?.pages || 1}
                    </p>
                    <div class="flex items-center gap-2">
                      <button
                        onClick={() => setCrmPage(Math.max(1, crmPage() - 1))}
                        disabled={crmPage() === 1}
                        class="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-30"
                      >
                        Previous
                      </button>
                      <button
                        onClick={() =>
                          setCrmPage(Math.min(crmPagination()?.pages || 1, crmPage() + 1))
                        }
                        disabled={crmPage() === (crmPagination()?.pages || 1)}
                        class="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-30"
                      >
                        Next
                      </button>
                    </div>
                  </div>
                </Show>
              </Show>
            </div>
          </Match>

          <Match when={activeTab() === 'analytics'}>
            <div class="animate-in fade-in slide-in-from-bottom-4 duration-500">
              <DocsAnalytics />
            </div>
          </Match>

          <Match when={activeTab() === 'revenue'}>
            <RevenueTab />
          </Match>

          <Match when={activeTab() === 'audit'}>
            <AuditLogTab />
          </Match>
        </Switch>
      </Show>

      <CustomerDetailDrawer userId={selectedUserId()} onClose={() => setSelectedUserId(null)} />
    </div>
  );
};

export default AdminDashboard;
