import { Component, For, Show, createSignal, createEffect, createMemo, onCleanup } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import type { FirehoseEvent, GeoDistribution, CommandHealth, AdvancedMetrics } from './types';
import {
  Terminal,
  Globe,
  AlertCircle,
  CheckCircle,
  Search,
  Package,
  RefreshCw,
  Activity,
} from 'lucide-solid';
import { LiveIndicator } from '../../../design-system';
import { Heatmap } from '../../../design-system/components/Charts';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface RealTimeCommandCenterProps {
  events: FirehoseEvent[];
  geoDistribution: GeoDistribution[];
  commandHealth: CommandHealth;
  commandHeatmap?: AdvancedMetrics['command_heatmap'];
  isLive?: boolean;
  onRefresh?: () => void;
}

const EVENT_TYPE_CONFIG: Record<string, { icon: typeof Terminal; color: string; bg: string; label: string }> = {
  command: { icon: Terminal, color: 'text-indigo-400', bg: 'bg-indigo-500/10', label: 'CMD' },
  install: { icon: Package, color: 'text-aurora-400', bg: 'bg-aurora-500/10', label: 'INSTALL' },
  search: { icon: Search, color: 'text-electric-400', bg: 'bg-electric-500/10', label: 'SEARCH' },
  runtime_switch: { icon: RefreshCw, color: 'text-photon-400', bg: 'bg-photon-500/10', label: 'RUNTIME' },
  error: { icon: AlertCircle, color: 'text-flare-400', bg: 'bg-flare-500/10', label: 'ERROR' },
};

const formatTimestamp = (timestamp: string): string => {
  const date = new Date(timestamp);
  return date.toLocaleTimeString('en-US', {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
};

const formatDuration = (ms: number): string => {
  if (ms < 1) return '<1ms';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
};

const CommandStreamItem: Component<{ event: FirehoseEvent; index: number }> = (props) => {
  const config = createMemo(() => EVENT_TYPE_CONFIG[props.event.event_type] || EVENT_TYPE_CONFIG.command);

  return (
    <div
      class="group flex items-start gap-3 border-l-2 border-transparent py-2 pl-3 pr-2 transition-all duration-200 hover:border-indigo-500 hover:bg-white/[0.02]"
      style={{ 'animation-delay': `${props.index * 30}ms` }}
    >
      <span class="shrink-0 font-mono text-2xs tabular-nums text-nebula-600">
        {formatTimestamp(props.event.timestamp)}
      </span>

      <div class={cn('shrink-0 rounded-lg p-1.5', config().bg)}>
        {(() => {
          const Icon = config().icon;
          return <Icon size={12} class={config().color} />;
        })()}
      </div>

      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <span class={cn('text-2xs font-black uppercase tracking-wider', config().color)}>
            {config().label}
          </span>
          <span class="truncate font-mono text-xs font-medium text-white">
            {props.event.event_name}
          </span>
        </div>
        <div class="mt-0.5 flex items-center gap-2 text-2xs text-nebula-500">
          <span class="truncate">{props.event.hostname || props.event.machine_id.slice(0, 8)}</span>
          <span class="text-nebula-700">|</span>
          <span>{props.event.platform}</span>
          <span class="text-nebula-700">|</span>
          <span class={props.event.duration_ms < 100 ? 'text-aurora-400' : 'text-solar-400'}>
            {formatDuration(props.event.duration_ms)}
          </span>
        </div>
      </div>

      <Show when={props.event.success !== undefined}>
        {props.event.success ? (
          <CheckCircle size={14} class="shrink-0 text-aurora-400" />
        ) : (
          <AlertCircle size={14} class="shrink-0 text-flare-400" />
        )}
      </Show>
    </div>
  );
};

interface GlobeVisualizationProps {
  data: GeoDistribution[];
  totalNodes: number;
}

const GlobeVisualization: Component<GlobeVisualizationProps> = (props) => {
  const topCountries = createMemo(() => props.data.slice(0, 8));
  const maxCount = createMemo(() => Math.max(...props.data.map(d => d.count), 1));

  const getCountryFlag = (code: string): string => {
    if (!code || code.length !== 2) return '🌍';
    const codePoints = code
      .toUpperCase()
      .split('')
      .map(char => 127397 + char.charCodeAt(0));
    return String.fromCodePoint(...codePoints);
  };

  return (
    <div class="relative overflow-hidden rounded-2xl border border-white/5 bg-gradient-to-br from-indigo-500/5 to-photon-500/5 p-6">
      <div class="pointer-events-none absolute -right-16 -top-16 h-48 w-48 rounded-full bg-indigo-500/10 blur-[60px]" />

      <div class="relative">
        <div class="mb-6 flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div class="rounded-xl bg-indigo-500/10 p-2">
              <Globe size={18} class="text-indigo-400" />
            </div>
            <div>
              <h3 class="text-sm font-black uppercase tracking-wider text-white">Global Presence</h3>
              <p class="text-2xs font-medium text-nebula-500">
                {props.totalNodes.toLocaleString()} active nodes worldwide
              </p>
            </div>
          </div>
        </div>

        <div class="space-y-3">
          <For each={topCountries()}>
            {(country, i) => {
              const percentage = (country.count / maxCount()) * 100;
              const hue = 230 + i() * 15;

              return (
                <div class="group">
                  <div class="mb-1 flex items-center justify-between">
                    <div class="flex items-center gap-2">
                      <span class="text-lg">{getCountryFlag(country.country_code)}</span>
                      <span class="text-xs font-bold text-nebula-300">{country.country}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <span class="font-mono text-xs font-black tabular-nums text-white">
                        {country.count.toLocaleString()}
                      </span>
                      <span class="text-2xs font-medium text-nebula-500">
                        ({country.percentage.toFixed(1)}%)
                      </span>
                    </div>
                  </div>
                  <div class="h-2 overflow-hidden rounded-full bg-void-700">
                    <div
                      class="h-full rounded-full transition-all duration-700 ease-out"
                      style={{
                        width: `${percentage}%`,
                        background: `linear-gradient(90deg, hsl(${hue}, 80%, 60%), hsl(${hue + 20}, 70%, 50%))`,
                      }}
                    />
                  </div>
                </div>
              );
            }}
          </For>
        </div>
      </div>
    </div>
  );
};

interface CommandHealthPulseProps {
  health: CommandHealth;
}

const CommandHealthPulse: Component<CommandHealthPulseProps> = (props) => {
  const [pulsePhase, setPulsePhase] = createSignal(0);

  createEffect(() => {
    const interval = setInterval(() => {
      setPulsePhase((p) => (p + 1) % 100);
    }, 50);
    onCleanup(() => clearInterval(interval));
  });

  const successAngle = () => (props.health.success / 100) * 360;
  const isHealthy = () => props.health.success >= 95;

  return (
    <div class="relative overflow-hidden rounded-2xl border border-white/5 bg-void-850 p-6">
      <div class="flex items-center gap-6">
        <div class="relative">
          <svg width="100" height="100" viewBox="0 0 100 100">
            <defs>
              <linearGradient id="health-success" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#10b981" />
                <stop offset="100%" stop-color="#34d399" />
              </linearGradient>
              <linearGradient id="health-failure" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#ef4444" />
                <stop offset="100%" stop-color="#f87171" />
              </linearGradient>
              <filter id="health-glow">
                <feGaussianBlur stdDeviation="2" result="blur" />
                <feMerge>
                  <feMergeNode in="blur" />
                  <feMergeNode in="SourceGraphic" />
                </feMerge>
              </filter>
            </defs>

            <circle
              cx="50"
              cy="50"
              r="42"
              fill="none"
              stroke="rgba(239, 68, 68, 0.2)"
              stroke-width="8"
            />

            <circle
              cx="50"
              cy="50"
              r="42"
              fill="none"
              stroke="url(#health-success)"
              stroke-width="8"
              stroke-linecap="round"
              stroke-dasharray={`${successAngle() * 0.733} 264`}
              transform="rotate(-90 50 50)"
              filter="url(#health-glow)"
              class="transition-all duration-500"
            />

            <circle
              cx="50"
              cy="50"
              r="30"
              fill={isHealthy() ? 'rgba(16, 185, 129, 0.1)' : 'rgba(239, 68, 68, 0.1)'}
              class="transition-all duration-500"
            />

            <circle
              cx="50"
              cy="50"
              r="20"
              fill={isHealthy() ? '#10b981' : '#ef4444'}
              opacity={0.3 + Math.sin(pulsePhase() * 0.1) * 0.2}
              class="transition-all"
            />

            <circle
              cx="50"
              cy="50"
              r="8"
              fill={isHealthy() ? '#10b981' : '#ef4444'}
            />
          </svg>
        </div>

        <div class="flex-1">
          <h3 class="text-sm font-black uppercase tracking-wider text-white">Command Health</h3>
          <p class="mt-1 text-2xs text-nebula-500">Real-time success rate monitoring</p>

          <div class="mt-4 grid grid-cols-2 gap-4">
            <div>
              <div class="flex items-baseline gap-1">
                <span class="font-display text-3xl font-black text-aurora-400">{props.health.success.toFixed(1)}</span>
                <span class="text-sm font-bold text-aurora-400/50">%</span>
              </div>
              <span class="text-2xs font-bold uppercase tracking-wider text-nebula-500">Success</span>
            </div>
            <div>
              <div class="flex items-baseline gap-1">
                <span class="font-display text-3xl font-black text-flare-400">{props.health.failure.toFixed(1)}</span>
                <span class="text-sm font-bold text-flare-400/50">%</span>
              </div>
              <span class="text-2xs font-bold uppercase tracking-wider text-nebula-500">Failure</span>
            </div>
          </div>
        </div>
      </div>

      <div
        class="absolute bottom-0 left-0 h-1 transition-all duration-300"
        style={{
          width: `${props.health.success}%`,
          background: 'linear-gradient(90deg, #10b981, #34d399)',
        }}
      />
    </div>
  );
};

interface ActivityHeatmapWrapperProps {
  heatmapData: AdvancedMetrics['command_heatmap'];
}

const ActivityHeatmapWrapper: Component<ActivityHeatmapWrapperProps> = (props) => {
  const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  const hours = Array.from({ length: 24 }, (_, i) => i.toString().padStart(2, '0'));

  const formattedData = createMemo(() => {
    if (!props.heatmapData || props.heatmapData.length === 0) return [];
    
    return props.heatmapData.map(item => ({
      x: parseInt(item.hour, 10),
      y: parseInt(item.day_of_week, 10),
      value: item.event_count,
    }));
  });

  return (
    <div class="overflow-hidden rounded-2xl border border-white/5 bg-void-850 p-6">
      <div class="mb-6 flex items-center gap-3">
        <div class="rounded-xl bg-photon-500/10 p-2">
          <Activity size={18} class="text-photon-400" />
        </div>
        <div>
          <h3 class="text-sm font-black uppercase tracking-wider text-white">Activity Heatmap</h3>
          <p class="text-2xs font-medium text-nebula-500">Command distribution by time (real data)</p>
        </div>
      </div>

      <Show
        when={formattedData().length > 0}
        fallback={
          <div class="flex flex-col items-center justify-center py-8 text-center">
            <Activity size={32} class="mb-3 text-nebula-600" />
            <p class="text-sm text-nebula-500">No activity data available</p>
          </div>
        }
      >
        <Heatmap
          data={formattedData()}
          xLabels={hours}
          yLabels={days}
          colorScale="indigo"
          cellSize="sm"
          showLegend
        />
      </Show>
    </div>
  );
};

export const RealTimeCommandCenter: Component<RealTimeCommandCenterProps> = (props) => {
  const [filter, setFilter] = createSignal<string>('all');
  const [searchQuery, setSearchQuery] = createSignal('');
  let streamRef: HTMLDivElement | undefined;

  const filteredEvents = createMemo(() => {
    let events = props.events;

    if (filter() !== 'all') {
      events = events.filter(e => e.event_type === filter());
    }

    if (searchQuery()) {
      const query = searchQuery().toLowerCase();
      events = events.filter(
        e =>
          e.event_name.toLowerCase().includes(query) ||
          e.hostname?.toLowerCase().includes(query) ||
          e.platform.toLowerCase().includes(query)
      );
    }

    return events.slice(0, 100);
  });

  createEffect(() => {
    if (streamRef && props.events.length > 0) {
      streamRef.scrollTop = 0;
    }
  });

  return (
    <div class="space-y-6">
      <div class="relative overflow-hidden rounded-4xl border border-white/5 bg-void-850 shadow-2xl">
        <div class="pointer-events-none absolute -left-32 -top-32 h-64 w-64 rounded-full bg-electric-500/10 blur-[100px]" />

        <div class="relative border-b border-white/5 bg-gradient-to-r from-white/[0.02] to-transparent p-6">
          <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
            <div class="flex items-center gap-4">
              <div class="flex h-12 w-12 items-center justify-center rounded-2xl bg-gradient-to-br from-electric-500 to-plasma-600">
                <Terminal size={24} class="text-white" />
              </div>
              <div>
                <h2 class="font-display text-xl font-black tracking-tight text-white">Command Center</h2>
                <p class="text-xs text-nebula-400">Real-time telemetry stream</p>
              </div>
            </div>

            <div class="flex items-center gap-3">
              <LiveIndicator
                label={props.isLive ? 'Live' : 'Paused'}
                color={props.isLive ? 'success' : 'warning'}
                variant="pulse"
              />

              <div class="relative">
                <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-nebula-500" />
                <input
                  type="text"
                  placeholder="Filter events..."
                  value={searchQuery()}
                  onInput={(e) => setSearchQuery(e.currentTarget.value)}
                  class="w-48 rounded-xl border border-white/10 bg-white/5 py-2 pl-9 pr-4 text-xs text-white placeholder-nebula-500 transition-all focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
                />
              </div>

              <select
                value={filter()}
                onChange={(e) => setFilter(e.currentTarget.value)}
                class="rounded-xl border border-white/10 bg-white/5 px-3 py-2 text-xs font-bold text-white transition-all focus:border-indigo-500 focus:outline-none"
              >
                <option value="all">All Events</option>
                <option value="command">Commands</option>
                <option value="install">Installs</option>
                <option value="search">Searches</option>
                <option value="runtime_switch">Runtime</option>
                <option value="error">Errors</option>
              </select>
            </div>
          </div>
        </div>

        <div
          ref={streamRef}
          class="relative h-[400px] overflow-y-auto font-mono text-xs"
          style={{ 'scrollbar-width': 'none', '-ms-overflow-style': 'none' }}
        >
          <div class="sticky top-0 z-10 border-b border-white/5 bg-void-850/95 px-4 py-2 backdrop-blur-xl">
            <div class="flex items-center gap-2 text-2xs font-bold uppercase tracking-wider text-nebula-600">
              <Terminal size={12} />
              <span>Live Stream</span>
              <span class="ml-auto text-nebula-500">{filteredEvents().length} events</span>
            </div>
          </div>

          <div class="divide-y divide-white/[0.02] px-2">
            <For each={filteredEvents()}>
              {(event, i) => <CommandStreamItem event={event} index={i()} />}
            </For>
          </div>

          <Show when={filteredEvents().length === 0}>
            <div class="flex flex-col items-center justify-center py-16 text-center">
              <Terminal size={48} class="mb-4 text-nebula-600" />
              <p class="font-sans text-sm font-bold text-nebula-500">No events to display</p>
              <p class="mt-1 font-sans text-xs text-nebula-600">
                {searchQuery() || filter() !== 'all' ? 'Try adjusting your filters' : 'Waiting for incoming events...'}
              </p>
            </div>
          </Show>

          <div class="sticky bottom-0 flex items-center gap-2 border-t border-white/5 bg-void-850/95 px-4 py-2 backdrop-blur-xl">
            <div class="h-1.5 w-1.5 animate-pulse rounded-full bg-aurora-500" />
            <span class="text-2xs italic text-nebula-600">Streaming telemetry data...</span>
          </div>
        </div>
      </div>

      <div class="grid gap-6 lg:grid-cols-2">
        <GlobeVisualization 
          data={props.geoDistribution} 
          totalNodes={props.geoDistribution.reduce((sum, g) => sum + g.count, 0)} 
        />
        <CommandHealthPulse health={props.commandHealth} />
      </div>

      <ActivityHeatmapWrapper heatmapData={props.commandHeatmap || []} />
    </div>
  );
};

export default RealTimeCommandCenter;
