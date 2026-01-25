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
  AlertCircle
} from '../ui/Icons';
import * as api from '../../lib/api';
import { useAdminDashboard, useAdminFirehose } from '../../lib/api-hooks';
import { StatCard } from './analytics/StatCard';
import { CardSkeleton } from '../ui/Skeleton';

type AdminTab = 'overview' | 'crm' | 'analytics' | 'revenue' | 'audit';

export const AdminDashboard: Component = () => {
  const [activeTab, setActiveTab] = createSignal<AdminTab>('overview');
  
  // TanStack Queries
  const dashboardQuery = useAdminDashboard();
  const firehoseQuery = useAdminFirehose(50);

  const adminData = () => dashboardQuery.data;
  const events = () => firehoseQuery.data?.events || [];

  const TabButton = (props: { id: AdminTab; icon: any; label: string }) => (
    <button
      onClick={() => setActiveTab(props.id)}
      class={`flex items-center gap-3 px-6 py-3 rounded-xl font-bold transition-all ${
        activeTab() === props.id
          ? 'bg-white text-black shadow-lg scale-[1.02]'
          : 'text-slate-400 hover:text-white hover:bg-white/5'
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
          <p class="mt-2 text-slate-400 font-medium">Global infrastructure, revenue, and fleet telemetry.</p>
        </div>
        
        <div class="flex items-center gap-3">
          <button class="flex items-center gap-2 rounded-2xl border border-white/10 bg-white/[0.03] px-5 py-3 text-sm font-bold text-white transition-all hover:bg-white/[0.08]">
            <Download size={16} />
            Export Data
          </button>
        </div>
      </div>

      {/* Navigation */}
      <div class="flex items-center gap-2 overflow-x-auto no-scrollbar rounded-2xl border border-white/5 bg-white/[0.02] p-1.5">
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
            <div class="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
              {/* Metrics Grid */}
              <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
                <StatCard
                  title="Total Users"
                  value={adminData()?.overview?.total_users?.toLocaleString() || '0'}
                  icon={<Users size={20} />}
                  trend={{ value: 8.2, isUp: true }}
                />
                <StatCard
                  title="Monthly Revenue"
                  value={`$${(adminData()?.overview?.mrr || 0).toLocaleString()}`}
                  icon={<CreditCard size={20} />}
                  trend={{ value: 12.5, isUp: true }}
                />
                <StatCard
                  title="Active Fleet"
                  value={adminData()?.overview?.active_machines?.toLocaleString() || '0'}
                  icon={<Globe size={20} />}
                />
                <StatCard
                  title="Command Volume"
                  value={adminData()?.overview?.total_commands?.toLocaleString() || '0'}
                  icon={<Zap size={20} />}
                  trend={{ value: 4.1, isUp: true }}
                />
              </div>

              <div class="grid grid-cols-1 gap-6 lg:grid-cols-3">
                {/* Firehose */}
                <div class="lg:col-span-2 rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                  <div class="mb-8 flex items-center justify-between">
                    <div>
                      <h3 class="text-xl font-bold text-white uppercase tracking-widest">Real-time Firehose</h3>
                      <p class="text-xs text-slate-500 font-medium mt-1">Live stream of global CLI events</p>
                    </div>
                    <div class="flex items-center gap-2 rounded-full bg-emerald-500/10 px-3 py-1 text-[10px] font-black uppercase text-emerald-400">
                      <div class="h-1.5 w-1.5 rounded-full bg-emerald-500 animate-pulse" />
                      Live
                    </div>
                  </div>

                  <div class="space-y-3">
                    <For each={events()}>
                      {(event) => (
                        <div class="flex items-center gap-4 rounded-2xl bg-white/[0.02] p-4 border border-white/5 hover:bg-white/[0.04] transition-all">
                          <div class={`h-10 w-10 rounded-xl flex items-center justify-center ${
                            event.event_name === 'command_run' ? 'bg-indigo-500/10 text-indigo-400' : 'bg-amber-500/10 text-amber-400'
                          }`}>
                            <Terminal size={18} />
                          </div>
                          <div class="flex-1 min-w-0">
                            <div class="flex items-center justify-between">
                              <span class="text-sm font-bold text-white truncate">{event.event_name}</span>
                              <span class="text-[10px] font-mono text-slate-500">{api.formatRelativeTime(event.created_at)}</span>
                            </div>
                            <div class="text-[10px] text-slate-500 truncate mt-0.5">
                              {event.platform} • {event.version} • {event.properties?.command || 'internal'}
                            </div>
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>

                {/* Platform Health */}
                <div class="space-y-6">
                  <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
                    <h3 class="text-lg font-bold text-white uppercase tracking-widest mb-6">Fleet Distribution</h3>
                    <div class="space-y-4">
                      <For each={adminData()?.fleet?.versions}>
                        {(v) => (
                          <div>
                            <div class="mb-2 flex justify-between text-[11px] font-black uppercase tracking-widest">
                              <span class="text-slate-500">v{v.omg_version}</span>
                              <span class="text-white">{v.count} nodes</span>
                            </div>
                            <div class="h-1.5 overflow-hidden rounded-full bg-white/[0.03]">
                              <div
                                class="h-full bg-indigo-500"
                                style={{ width: `${(v.count / (adminData()?.overview?.active_machines || 1)) * 100}%` }}
                              />
                            </div>
                          </div>
                        )}
                      </For>
                    </div>
                  </div>

                  <div class="rounded-3xl border border-white/5 bg-gradient-to-br from-indigo-500/10 to-transparent p-8 shadow-2xl">
                    <h3 class="text-lg font-bold text-white uppercase tracking-widest mb-4">Command Health</h3>
                    <div class="flex items-end gap-2 h-32">
                       <div class="flex-1 bg-emerald-500/20 rounded-t-lg relative group" style={{ height: '94%' }}>
                          <div class="absolute inset-x-0 bottom-full mb-2 opacity-0 group-hover:opacity-100 transition-opacity text-center text-[10px] font-bold text-emerald-400">94% Success</div>
                       </div>
                       <div class="flex-1 bg-rose-500/20 rounded-t-lg relative group" style={{ height: '6%' }}>
                          <div class="absolute inset-x-0 bottom-full mb-2 opacity-0 group-hover:opacity-100 transition-opacity text-center text-[10px] font-bold text-rose-400">6% Error</div>
                       </div>
                    </div>
                    <div class="mt-4 flex justify-between text-[10px] font-black uppercase tracking-widest text-slate-500">
                      <span>Operations</span>
                      <span>Fault Rate</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </Match>

          <Match when={activeTab() === 'crm'}>
            <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-10 shadow-2xl">
              <div class="flex flex-col md:flex-row md:items-center justify-between gap-6 mb-10">
                <div>
                  <h3 class="text-2xl font-black text-white tracking-tight">User CRM</h3>
                  <p class="text-sm font-medium text-slate-500">Manage customers and their subscription tiers.</p>
                </div>
                <div class="relative max-w-md w-full">
                  <Search class="absolute left-4 top-1/2 -translate-y-1/2 text-slate-500" size={18} />
                  <input 
                    type="text" 
                    placeholder="Search by email, company or ID..."
                    class="w-full bg-white/5 border border-white/10 rounded-2xl py-3 pl-12 pr-4 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 transition-all"
                  />
                </div>
              </div>

              <div class="overflow-x-auto">
                <table class="w-full text-left">
                  <thead>
                    <tr class="border-b border-white/5 text-[10px] font-black uppercase tracking-widest text-slate-500">
                      <th class="px-6 py-4">User</th>
                      <th class="px-6 py-4">Tier</th>
                      <th class="px-6 py-4">Status</th>
                      <th class="px-6 py-4">Activity</th>
                      <th class="px-6 py-4">Joined</th>
                      <th class="px-6 py-4"></th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-white/5">
                    <For each={adminData()?.recent_signups || []}>
                      {() => (
                         <tr class="group hover:bg-white/[0.01] transition-colors">
                            <td class="px-6 py-4">
                              <div class="text-sm font-bold text-white">dev@example.com</div>
                              <div class="text-[10px] font-mono text-slate-500 uppercase">USR-928374</div>
                            </td>
                            <td class="px-6 py-4 font-bold text-indigo-400 text-xs">TEAM</td>
                            <td class="px-6 py-4">
                              <span class="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-black uppercase text-emerald-400">Active</span>
                            </td>
                            <td class="px-6 py-4 text-sm text-slate-300">1,240 ops</td>
                            <td class="px-6 py-4 text-sm text-slate-500">2d ago</td>
                            <td class="px-6 py-4">
                              <button class="p-2 rounded-lg hover:bg-white/5 text-slate-500 transition-colors">
                                <Search size={16} />
                              </button>
                            </td>
                         </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>
            </div>
          </Match>
        </Switch>
      </Show>
    </div>
  );
};

export default AdminDashboard;
