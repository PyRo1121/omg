import { Component, createSignal, createEffect, onCleanup, For, Show, onMount } from 'solid-js';
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
  TrendingUp
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

export const AdminDashboard: Component = () => {
  // State
  const [events, setEvents] = createSignal<FirehoseEvent[]>([]);
  const [adminData, setAdminData] = createSignal<api.AdminOverview | null>(null);
  const [analytics, setAnalytics] = createSignal<api.AdminAnalytics | null>(null);
  const [fleetStats, setFleetStats] = createSignal<FleetStat[]>([]);

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
          // Simple heuristic: older versions are "warning"
          status: v.omg_version.startsWith('1.2') ? 'healthy' : 'warning'
        }));
        setFleetStats(stats);
      }
    } catch (e) {
      console.error('Failed to fetch admin data:', e);
    }
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

    pollInterval = setInterval(fetchFirehose, 3000); // Poll events every 3s
    dataInterval = setInterval(fetchData, 30000);    // Poll stats every 30s
  });

  onCleanup(() => {
    clearInterval(pollInterval);
    clearInterval(dataInterval);
  });

  // Computed metrics
  const activeUsers = () => analytics()?.dau || adminData()?.overview.active_users || 0;

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

  // Styles
  const glassCard = "bg-slate-900/40 backdrop-blur-xl border border-slate-700/50 rounded-2xl p-6 shadow-xl relative overflow-hidden group hover:border-slate-600/50 transition-colors duration-300";
  const glow = "absolute -inset-px bg-gradient-to-r from-blue-500/10 to-purple-500/10 opacity-0 group-hover:opacity-100 transition-opacity duration-500 rounded-2xl pointer-events-none";

  return (
    <div class="space-y-8 animate-fade-in pb-12">

      {/* Header Stats */}
      <div class="grid grid-cols-1 md:grid-cols-4 gap-6">
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
            {/* Abstract Map Visualization */}
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
  );
};
