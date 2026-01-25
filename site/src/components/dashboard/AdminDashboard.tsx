import { Component, createSignal, createEffect, onCleanup, For, Show, onMount, Switch, Match } from 'solid-js';
import {
  Activity,
  Map,
  Server,
  Terminal,
  Users,
  ShieldAlert,
  Zap,
  Globe,
  Clock,
  CheckCircle2,
  TrendingUp,
  Search,
  ChevronRight,
  Filter,
  Download,
  BarChart3,
  CreditCard,
  History,
  UserCheck,
  AlertCircle,
  ZapOff
} from 'lucide-solid';
import * as api from '../../lib/api';

// Types for the dashboard
interface FirehoseEvent {
  id: string;
  event_type: string;
  event_name: string;
  properties: any;
  created_at: string;
  platform: string;
  version: string;
  duration_ms?: number;
}

interface FleetStat {
  version: string;
  count: number;
  status: 'healthy' | 'warning' | 'critical';
}

type AdminTab = 'overview' | 'crm' | 'analytics' | 'revenue' | 'audit';

export const AdminDashboard: Component = () => {
  // State
  const [activeTab, setActiveTab] = createSignal<AdminTab>('overview');
  const [events, setEvents] = createSignal<FirehoseEvent[]>([]);
  const [adminData, setAdminData] = createSignal<api.AdminOverview | null>(null);
  const [analytics, setAnalytics] = createSignal<api.AdminAnalytics | null>(null);
  const [fleetStats, setFleetStats] = createSignal<FleetStat[]>([]);
  
  // CRM State
  const [crmUsers, setCrmUsers] = createSignal<any[]>([]);
  const [crmSearch, setCrmSearch] = createSignal('');
  const [crmLoading, setCrmLoading] = createSignal(false);
  const [crmPagination, setCrmPagination] = createSignal({ page: 1, total: 0, pages: 1 });

  // Revenue State
  const [revenueData, setRevenueData] = createSignal<api.AdminRevenue | null>(null);

  // Real-time polling
  let pollInterval: ReturnType<typeof setInterval>;
  let dataInterval: ReturnType<typeof setInterval>;

  const fetchData = async () => {
    try {
      const [data, analyticsData] = await Promise.all([
        api.getAdminDashboard(),
        api.getAdminAnalytics()
      ]);
      setAdminData(data);
      setAnalytics(analyticsData);

      // Process fleet stats from versions
      if (data.fleet?.versions) {
        const stats: FleetStat[] = data.fleet.versions.map(v => ({
          version: v.omg_version,
          count: v.count,
          status: v.omg_version.startsWith('1.2') ? 'healthy' : 'warning'
        }));
        setFleetStats(stats);
      }
    } catch (e) {
      console.error('Failed to fetch admin data:', e);
    }
  };

  const fetchCRM = async () => {
    setCrmLoading(true);
    try {
      const res = await api.get('/api/admin/crm/users?search=' + encodeURIComponent(crmSearch()) + '&page=' + crmPagination().page);
      const data = res as any;
      setCrmUsers(data.users || []);
      if (data.pagination) {
        setCrmPagination(data.pagination);
      }
    } catch (e) {
      console.error('CRM fetch error:', e);
    } finally {
      setCrmLoading(false);
    }
  };

  const fetchRevenue = async () => {
    try {
      const data = await api.getAdminRevenue();
      setRevenueData(data);
    } catch (e) {
      console.error('Revenue fetch error:', e);
    }
  };

  const handleCrmSearch = (e: any) => {
    setCrmSearch(e.target.value);
    // Debounce search would be good here, but for now just wait for Enter
  };

  const fetchFirehose = async () => {
    try {
      const response = await api.getAdminFirehose(20);
      if (response.events) {
        setEvents(response.events);
      }
    } catch (e) {
      console.error('Failed to fetch firehose:', e);
    }
  };

  onMount(() => {
    fetchData();
    fetchFirehose();

    pollInterval = setInterval(fetchFirehose, 3000);
    dataInterval = setInterval(fetchData, 30000);
  });

  onCleanup(() => {
    clearInterval(pollInterval);
    clearInterval(dataInterval);
  });

  createEffect(() => {
    const tab = activeTab();
    if (tab === 'crm') {
      fetchCRM();
    } else if (tab === 'revenue') {
      fetchRevenue();
    }
  });

  // Computed metrics
  const activeUsers = () => analytics()?.dau || adminData()?.overview.total_users || 0;

  const commandsPerSec = () => {
    const eventsList = events();
    if (eventsList.length < 2) return 0;
    
    const newest = new Date(eventsList[0].created_at).getTime();
    const oldest = new Date(eventsList[eventsList.length - 1].created_at).getTime();
    const durationSec = (newest - oldest) / 1000;
    
    if (durationSec <= 0) return 0;
    return Math.round((eventsList.length / durationSec) * 10) / 10;
  };

  const errorRate = () => {
    const health = adminData()?.overview.command_health;
    if (!health || (health.success + health.failure === 0)) return 0;
    return health.failure / (health.success + health.failure);
  };

  const fleetHealth = () => {
    if (fleetStats().length === 0) return 100;
    const total = fleetStats().reduce((acc, curr) => acc + curr.count, 0);
    const healthy = fleetStats().filter(s => s.status === 'healthy').reduce((acc, curr) => acc + curr.count, 0);
    return (healthy / total) * 100;
  };

  const crmExportUrl = () => api.getAdminExportUsersUrl();

  // Styles
  const glassCard = "bg-slate-900/40 backdrop-blur-xl border border-slate-700/50 rounded-2xl p-6 shadow-xl relative overflow-hidden group hover:border-slate-600/50 transition-colors duration-300";
  const glow = "absolute -inset-px bg-gradient-to-r from-blue-500/10 to-purple-500/10 opacity-0 group-hover:opacity-100 transition-opacity duration-500 rounded-2xl pointer-events-none";
  const tabBtn = (id: AdminTab) => `px-6 py-2 rounded-xl font-medium transition-all flex items-center gap-2 ${activeTab() === id ? 'bg-blue-600 text-white shadow-lg shadow-blue-600/20' : 'text-slate-400 hover:text-white hover:bg-white/5'}`;

  return (
    <div class="space-y-8 animate-fade-in pb-12">
      
      <div class="flex items-center gap-2 bg-slate-950/50 p-1.5 rounded-2xl border border-white/5 w-fit">
        <button onClick={() => setActiveTab('overview')} class={tabBtn('overview')}>
          <BarChart3 class="w-4 h-4" /> Overview
        </button>
        <button onClick={() => setActiveTab('crm')} class={tabBtn('crm')}>
          <Users class="w-4 h-4" /> User CRM
        </button>
        <button onClick={() => setActiveTab('analytics')} class={tabBtn('analytics')}>
          <Activity class="w-4 h-4" /> Analytics
        </button>
        <button onClick={() => setActiveTab('revenue')} class={tabBtn('revenue')}>
          <CreditCard class="w-4 h-4" /> Revenue
        </button>
        <button onClick={() => setActiveTab('audit')} class={tabBtn('audit')}>
          <History class="w-4 h-4" /> Audit Log
        </button>
      </div>

      <Switch>
        <Match when={activeTab() === 'overview'}>
          <div class="space-y-8 animate-fade-in">
            {/* Header Stats */}
            <div class="grid grid-cols-1 md:grid-cols-5 gap-6">
              <div class={glassCard}>
                <div class={glow} />
                <div class="flex items-center justify-between mb-4">
                  <span class="text-slate-400 text-sm font-medium uppercase tracking-wider">Active Users</span>
                  <Users class="w-5 h-5 text-blue-400" />
                </div>
                <div class="text-3xl font-bold font-mono text-white flex items-end gap-2">
                  {activeUsers().toLocaleString()}
                  <span class="text-xs text-green-400 mb-1 font-sans font-medium flex items-center gap-1">
                    <Activity class="w-3 h-3" /> Live
                  </span>
                </div>
              </div>

              <div class={glassCard}>
                <div class={glow} />
                <div class="flex items-center justify-between mb-4">
                  <span class="text-slate-400 text-sm font-medium uppercase tracking-wider">Avg Cmds/Sec</span>
                  <Zap class="w-5 h-5 text-yellow-400" />
                </div>
                <div class="text-3xl font-bold font-mono text-white">
                  {commandsPerSec()}
                </div>
              </div>

              <div class={glassCard}>
                <div class={glow} />
                <div class="flex items-center justify-between mb-4">
                  <span class="text-slate-400 text-sm font-medium uppercase tracking-wider">Error Rate</span>
                  <ShieldAlert class="w-5 h-5 text-red-400" />
                </div>
                <div class="text-3xl font-bold font-mono text-white">
                  {(errorRate() * 100).toFixed(2)}%
                </div>
              </div>

              <div class={glassCard}>
                <div class={glow} />
                <div class="flex items-center justify-between mb-4">
                  <span class="text-slate-400 text-sm font-medium uppercase tracking-wider">Retention Rate</span>
                  <TrendingUp class="w-5 h-5 text-purple-400" />
                </div>
                <div class="text-3xl font-bold font-mono text-white">
                  {analytics()?.retention_rate || 0}%
                </div>
              </div>

              <div class={glassCard}>
                <div class={glow} />
                <div class="flex items-center justify-between mb-4">
                  <span class="text-slate-400 text-sm font-medium uppercase tracking-wider">Fleet Consistency</span>
                  <Server class="w-5 h-5 text-emerald-400" />
                </div>
                <div class="text-3xl font-bold font-mono text-white">
                  {fleetHealth().toFixed(1)}%
                </div>
              </div>
            </div>

            {/* Main Grid */}
            <div class="grid grid-cols-1 lg:grid-cols-3 gap-8">
              {/* Live Map Section */}
              <div class={`${glassCard} lg:col-span-2 min-h-[400px] flex flex-col`}>
                <div class={glow} />
                <div class="flex items-center gap-3 mb-6 border-b border-slate-700/50 pb-4">
                  <Globe class="w-5 h-5 text-indigo-400" />
                  <h2 class="text-lg font-bold text-white tracking-tight">Global Activity Map</h2>
                </div>
                <div class="flex-1 rounded-xl bg-slate-950/50 border border-slate-800 relative overflow-hidden flex items-center justify-center group/map">
                  <div class="absolute inset-0 opacity-20 bg-[radial-gradient(circle_at_50%_50%,rgba(56,189,248,0.1),transparent_70%)]" />
                  <div class="text-slate-500 font-mono text-sm flex flex-col items-center gap-2">
                    <Map class="w-12 h-12 opacity-50 mb-2" />
                    <span>Real-time Global Telemetry</span>
                    <span class="text-xs opacity-60">Visualizing {events().length} recent events from {adminData()?.geo_distribution?.length || 0} regions</span>
                  </div>

                  <For each={adminData()?.geo_distribution || []}>
                    {(geo) => (
                      <div
                        class="absolute w-2 h-2 bg-blue-400 rounded-full animate-ping"
                        style={{
                          top: `${Math.random() * 60 + 20}%`,
                          left: `${Math.random() * 80 + 10}%`,
                          'animation-duration': `${Math.random() * 2 + 1}s`
                        }}
                        title={`${geo.dimension}: ${geo.count} events`}
                      />
                    )}
                  </For>
                </div>
              </div>

              {/* Fleet Versions */}
              <div class={`${glassCard} flex flex-col`}>
                <div class={glow} />
                <div class="flex items-center gap-3 mb-6 border-b border-slate-700/50 pb-4">
                  <Server class="w-5 h-5 text-emerald-400" />
                  <h2 class="text-lg font-bold text-white tracking-tight">Fleet Versions</h2>
                </div>
                <div class="space-y-4 flex-1 overflow-auto pr-2 custom-scrollbar">
                  <Show when={fleetStats().length > 0} fallback={
                    <div class="text-center text-slate-500 italic py-8">No active fleet data</div>
                  }>
                    <For each={fleetStats()}>
                      {(stat) => (
                        <div class="bg-slate-950/30 rounded-xl p-4 border border-slate-800/50 hover:border-slate-700 transition-colors flex items-center justify-between">
                          <div>
                            <div class="flex items-center gap-2 mb-1">
                              <h3 class="font-medium text-slate-200">v{stat.version}</h3>
                              <Show when={stat.status === 'healthy'}>
                                <CheckCircle2 class="w-3 h-3 text-emerald-400" />
                              </Show>
                            </div>
                            <div class="text-xs text-slate-500 font-mono">
                              {stat.count} machines
                            </div>
                          </div>
                          <span class={`px-2 py-0.5 rounded text-xs font-mono font-bold uppercase ${
                            stat.status === 'healthy' ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' :
                            'bg-yellow-500/10 text-yellow-400 border border-yellow-500/20'
                          }`}>
                            {stat.status}
                          </span>
                        </div>
                      )}
                    </For>
                  </Show>
                </div>
              </div>

              {/* Live Command Firehose */}
              <div class={`${glassCard} lg:col-span-3 min-h-[500px] flex flex-col`}>
                <div class={glow} />
                <div class="flex items-center justify-between mb-6 border-b border-slate-700/50 pb-4">
                  <div class="flex items-center gap-3">
                    <Terminal class="w-5 h-5 text-pink-400" />
                    <h2 class="text-lg font-bold text-white tracking-tight">Live Command Firehose</h2>
                  </div>
                  <div class="flex items-center gap-2 text-xs font-mono text-emerald-400 bg-emerald-500/10 px-3 py-1 rounded-full border border-emerald-500/20 animate-pulse">
                    <span class="w-2 h-2 bg-emerald-400 rounded-full" />
                    LIVE
                  </div>
                </div>

                <div class="flex-1 bg-slate-950 rounded-xl border border-slate-800 p-4 font-mono text-xs md:text-sm overflow-hidden flex flex-col shadow-inner">
                  <div class="grid grid-cols-12 text-slate-500 uppercase text-[10px] tracking-wider mb-2 px-2 font-bold select-none">
                    <div class="col-span-2">Timestamp</div>
                    <div class="col-span-2">Event</div>
                    <div class="col-span-2">Platform</div>
                    <div class="col-span-4">Details</div>
                    <div class="col-span-2 text-right">Duration</div>
                  </div>

                  <div class="flex-1 overflow-y-auto custom-scrollbar space-y-1 pr-2">
                    <Show when={events().length > 0} fallback={
                      <div class="flex items-center justify-center h-full text-slate-600 italic">
                        Waiting for telemetry data...
                      </div>
                    }>
                      <For each={events()}>
                        {(event) => (
                          <div class="grid grid-cols-12 gap-2 p-2 hover:bg-slate-900 rounded transition-colors group items-center border-l-2 border-transparent hover:border-blue-500">
                            <div class="col-span-2 text-slate-500 whitespace-nowrap overflow-hidden">
                              {new Date(event.created_at).toLocaleTimeString()}
                            </div>
                            <div class="col-span-2 font-bold text-blue-400 group-hover:text-blue-300">
                              {event.event_name}
                            </div>
                            <div class="col-span-2 text-slate-400 flex items-center gap-1.5">
                              <span class={`w-1.5 h-1.5 rounded-full ${
                                event.platform.includes('linux') ? 'bg-orange-400' :
                                event.platform.includes('darwin') ? 'bg-white' : 'bg-blue-400'
                              }`} />
                              {event.platform}
                            </div>
                            <div class="col-span-4 text-slate-300 truncate opacity-80 group-hover:opacity-100">
                              {JSON.stringify(event.properties).replace(/[{}"]/g, '')}
                            </div>
                            <div class="col-span-2 text-right text-slate-500 font-medium">
                              {event.duration_ms ? `${event.duration_ms}ms` : '-'}
                            </div>
                          </div>
                        )}
                      </For>
                    </Show>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Match>

        <Match when={activeTab() === 'crm'}>
          <div class="space-y-6 animate-fade-in">
            <div class="flex flex-col md:flex-row gap-4 items-center justify-between">
              <div class="relative w-full md:w-96">
                <Search class="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
                <input 
                  type="text" 
                  placeholder="Search users by email or company..." 
                  class="w-full bg-slate-900/50 border border-slate-700 rounded-xl py-2.5 pl-11 pr-4 text-white focus:outline-none focus:border-blue-500 transition-all"
                  value={crmSearch()}
                  onInput={handleCrmSearch}
                  onKeyDown={(e) => e.key === 'Enter' && fetchCRM()}
                />
              </div>
              <div class="flex items-center gap-3">
                <button class="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-slate-900/50 border border-slate-700 text-slate-300 hover:text-white transition-all">
                  <Filter class="w-4 h-4" /> Filter
                </button>
                <button 
                  onClick={() => window.open(crmExportUrl(), '_blank')}
                  class="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-blue-600 text-white hover:bg-blue-500 transition-all shadow-lg shadow-blue-600/20"
                >
                  <Download class="w-4 h-4" /> Export CSV
                </button>
              </div>
            </div>

            <div class={`${glassCard} p-0 overflow-hidden`}>
              <div class="overflow-x-auto">
                <table class="w-full text-left border-collapse">
                  <thead>
                    <tr class="bg-white/5 text-slate-400 text-[10px] uppercase tracking-widest font-bold">
                      <th class="px-6 py-4">User / Company</th>
                      <th class="px-6 py-4">Engagement</th>
                      <th class="px-6 py-4">Lifecycle</th>
                      <th class="px-6 py-4">Fleet</th>
                      <th class="px-6 py-4">Total Ops</th>
                      <th class="px-6 py-4">Last Active</th>
                      <th class="px-6 py-4 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-white/5">
                    <For each={crmUsers()} fallback={
                      <tr>
                        <td colspan="7" class="px-6 py-12 text-center text-slate-500 italic">
                          {crmLoading() ? 'Loading userbase...' : 'No users found matching your search.'}
                        </td>
                      </tr>
                    }>
                      {(user) => (
                        <tr class="hover:bg-white/[0.02] transition-colors group">
                          <td class="px-6 py-4">
                            <div class="flex flex-col">
                              <span class="text-sm font-bold text-white">{user.email}</span>
                              <span class="text-xs text-slate-500">{user.company || 'Individual'}</span>
                            </div>
                          </td>
                          <td class="px-6 py-4">
                            <div class="flex items-center gap-3">
                              <div class="flex-1 h-1.5 w-24 bg-slate-800 rounded-full overflow-hidden">
                                <div 
                                  class={`h-full rounded-full ${user.engagement_score > 70 ? 'bg-emerald-500' : user.engagement_score > 30 ? 'bg-blue-500' : 'bg-slate-600'}`}
                                  style={{ width: `${user.engagement_score}%` }}
                                />
                              </div>
                              <span class="text-xs font-mono font-bold text-slate-300">{user.engagement_score}</span>
                            </div>
                          </td>
                          <td class="px-6 py-4">
                            <span class={`px-2 py-0.5 rounded text-[10px] font-black uppercase border ${
                              user.lifecycle_stage === 'power_user' ? 'bg-purple-500/10 text-purple-400 border-purple-500/20' :
                              user.lifecycle_stage === 'active' ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20' :
                              user.lifecycle_stage === 'at_risk' ? 'bg-amber-500/10 text-amber-400 border-amber-500/20' :
                              'bg-slate-500/10 text-slate-400 border-slate-500/20'
                            }`}>
                              {user.lifecycle_stage.replace('_', ' ')}
                            </span>
                          </td>
                          <td class="px-6 py-4">
                            <div class="flex items-center gap-1.5 text-xs text-slate-300">
                              <Server class="w-3 h-3 text-slate-500" />
                              {user.machine_count} nodes
                            </div>
                          </td>
                          <td class="px-6 py-4">
                            <span class="text-xs font-mono text-slate-300">{(user.total_commands || 0).toLocaleString()}</span>
                          </td>
                          <td class="px-6 py-4">
                            <span class="text-xs text-slate-400">{user.last_active_date ? api.formatRelativeTime(user.last_active_date) : 'Never'}</span>
                          </td>
                          <td class="px-6 py-4 text-right">
                            <button 
                              onClick={() => window.open(`mailto:${user.email}`, '_blank')}
                              class="p-2 rounded-lg hover:bg-white/5 text-slate-500 hover:text-white transition-all"
                            >
                              <ChevronRight class="w-4 h-4" />
                            </button>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>
            </div>
            
            {/* Pagination */}
            <Show when={crmPagination().pages > 1}>
              <div class="flex justify-center gap-2">
                <button 
                  disabled={crmPagination().page === 1}
                  onClick={() => setCrmPagination(p => ({ ...p, page: p.page - 1 }))}
                  class="px-4 py-2 rounded-xl bg-slate-900 border border-slate-700 text-slate-400 disabled:opacity-50"
                >
                  Prev
                </button>
                <span class="px-4 py-2 text-slate-400">Page {crmPagination().page} of {crmPagination().pages}</span>
                <button 
                  disabled={crmPagination().page === crmPagination().pages}
                  onClick={() => setCrmPagination(p => ({ ...p, page: p.page + 1 }))}
                  class="px-4 py-2 rounded-xl bg-slate-900 border border-slate-700 text-slate-400 disabled:opacity-50"
                >
                  Next
                </button>
              </div>
            </Show>
          </div>
        </Match>

        <Match when={activeTab() === 'analytics'}>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-8 animate-fade-in">
            <div class={glassCard}>
              <h3 class="text-lg font-bold text-white mb-6 flex items-center gap-2">
                <Zap class="w-5 h-5 text-yellow-400" /> Top Commands (7d)
              </h3>
              <div class="space-y-4">
                <For each={analytics()?.commands_by_type || []}>
                  {(cmd) => (
                    <div class="flex items-center justify-between p-3 rounded-xl bg-white/5 border border-white/5">
                      <span class="text-sm font-mono text-blue-400">{cmd.command}</span>
                      <span class="text-xs font-bold text-slate-500">{cmd.count.toLocaleString()} calls</span>
                    </div>
                  )}
                </For>
              </div>
            </div>

            <div class={glassCard}>
              <h3 class="text-lg font-bold text-white mb-6 flex items-center gap-2">
                <ShieldAlert class="w-5 h-5 text-red-400" /> System Errors (7d)
              </h3>
              <div class="space-y-4">
                <For each={analytics()?.errors_by_type || []}>
                  {(err) => (
                    <div class="flex items-center justify-between p-3 rounded-xl bg-red-500/5 border border-red-500/10">
                      <span class="text-sm text-red-400 truncate max-w-[200px]">{err.error_type}</span>
                      <span class="text-xs font-bold text-red-900/60">{err.count} events</span>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </div>
        </Match>

        <Match when={activeTab() === 'revenue'}>
          <div class="space-y-8 animate-fade-in">
            <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
              <div class={glassCard}>
                <span class="text-slate-500 text-[10px] uppercase font-black tracking-widest">Monthly Recurring Revenue</span>
                <div class="text-4xl font-black text-white mt-2">${revenueData()?.mrr || 0} <span class="text-sm text-slate-500 font-normal">USD</span></div>
              </div>
              <div class={glassCard}>
                <span class="text-slate-500 text-[10px] uppercase font-black tracking-widest">Growth Rate (7d)</span>
                <div class="text-4xl font-black text-emerald-400 mt-2">+{analytics()?.growth?.growth_rate || 0}%</div>
              </div>
              <div class={glassCard}>
                <span class="text-slate-500 text-[10px] uppercase font-black tracking-widest">Churn Risk</span>
                <div class="text-4xl font-black text-amber-400 mt-2">{analytics()?.churn_risk?.at_risk_users || 0} <span class="text-sm text-slate-500 font-normal">USERS</span></div>
              </div>
            </div>

            <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
              <div class={glassCard}>
                <h3 class="text-lg font-bold text-white mb-6 flex items-center gap-2">
                  <CreditCard class="w-5 h-5 text-indigo-400" /> Revenue by Tier
                </h3>
                <div class="space-y-4">
                  <For each={adminData()?.tiers || []}>
                    {(tier) => (
                      <div class="flex items-center justify-between p-4 rounded-xl bg-white/5 border border-white/5">
                        <div class="flex items-center gap-3">
                          <div class={`w-2 h-2 rounded-full ${tier.tier === 'enterprise' ? 'bg-amber-500' : tier.tier === 'team' ? 'bg-purple-500' : 'bg-blue-500'}`} />
                          <span class="text-sm font-bold text-white uppercase tracking-widest">{tier.tier}</span>
                        </div>
                        <div class="text-right">
                          <div class="text-sm font-mono text-white">{tier.count} customers</div>
                          <div class="text-[10px] text-slate-500 font-bold uppercase">Active Subscriptions</div>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>

              <div class={glassCard}>
                <h3 class="text-lg font-bold text-white mb-6 flex items-center gap-2">
                  <TrendingUp class="w-5 h-5 text-emerald-400" /> Growth Funnel
                </h3>
                <div class="space-y-6">
                  <div class="relative">
                    <div class="flex justify-between text-[10px] font-black text-slate-500 uppercase mb-2">
                      <span>Installs</span>
                      <span>{analytics()?.funnel?.installs || 0}</span>
                    </div>
                    <div class="h-2 bg-slate-800 rounded-full overflow-hidden">
                      <div class="h-full bg-blue-500 w-full" />
                    </div>
                  </div>
                  <div class="relative">
                    <div class="flex justify-between text-[10px] font-black text-slate-500 uppercase mb-2">
                      <span>Activated</span>
                      <span>{analytics()?.funnel?.activated || 0}</span>
                    </div>
                    <div class="h-2 bg-slate-800 rounded-full overflow-hidden">
                      <div class="h-full bg-indigo-500" style={{ width: `${((analytics()?.funnel?.activated || 0) / (analytics()?.funnel?.installs || 1)) * 100}%` }} />
                    </div>
                  </div>
                  <div class="relative">
                    <div class="flex justify-between text-[10px] font-black text-slate-500 uppercase mb-2">
                      <span>Power Users</span>
                      <span>{analytics()?.funnel?.power_users || 0}</span>
                    </div>
                    <div class="h-2 bg-slate-800 rounded-full overflow-hidden">
                      <div class="h-full bg-purple-500" style={{ width: `${((analytics()?.funnel?.power_users || 0) / (analytics()?.funnel?.installs || 1)) * 100}%` }} />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Match>

        <Match when={activeTab() === 'audit'}>
          <div class={`${glassCard} p-0 animate-fade-in`}>
            <div class="p-6 border-b border-white/5 flex justify-between items-center">
              <h3 class="font-bold text-white">System Audit Trail</h3>
              <span class="text-xs text-slate-500">Showing last 100 events</span>
            </div>
            <div class="max-h-[600px] overflow-y-auto custom-scrollbar">
              <table class="w-full text-left border-collapse">
                <thead class="sticky top-0 bg-slate-900 z-10">
                  <tr class="text-slate-500 text-[10px] uppercase font-bold border-b border-white/5">
                    <th class="px-6 py-3">Timestamp</th>
                    <th class="px-6 py-3">Action</th>
                    <th class="px-6 py-3">User</th>
                    <th class="px-6 py-3">Details</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-white/5">
                  <For each={adminData()?.recent_signups || []}>
                    {(log) => (
                      <tr class="text-xs">
                        <td class="px-6 py-3 text-slate-500 font-mono">{log.date}</td>
                        <td class="px-6 py-3"><span class="text-blue-400 font-bold">user.signup</span></td>
                        <td class="px-6 py-3 text-slate-300">New User Joined</td>
                        <td class="px-6 py-3 text-slate-500 font-mono">---.---.---.---</td>
                      </tr>
                    )}
                  </For>
                  <Show when={!adminData()?.recent_signups?.length}>
                    <tr>
                      <td colspan="4" class="px-6 py-12 text-center text-slate-500 italic">No recent audit events found.</td>
                    </tr>
                  </Show>
                </tbody>
              </table>
            </div>
          </div>
        </Match>
      </Switch>
    </div>
  );
};
