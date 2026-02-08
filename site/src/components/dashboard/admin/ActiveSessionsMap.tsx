import { Component, For, Show, createSignal, createMemo, createEffect, onMount, onCleanup } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { Globe, MapPin, Users, Activity, Laptop, Server, Clock } from 'lucide-solid';
import type { SessionEvent } from '../../../hooks/useRealtimeData';
import { LiveIndicator } from '../../../design-system';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// ============================================================================
// Types
// ============================================================================

interface ActiveSessionsMapProps {
  sessions: SessionEvent[];
  isLive?: boolean;
  class?: string;
}

interface MapPoint {
  id: string;
  x: number;
  y: number;
  session: SessionEvent;
  isRecent: boolean;
}

interface TooltipData {
  session: SessionEvent;
  x: number;
  y: number;
}

// ============================================================================
// Constants - Simple world map projection coordinates
// ============================================================================

// Approximate center points for countries (normalized to 0-100 scale for SVG)
const COUNTRY_COORDINATES: Record<string, { x: number; y: number }> = {
  // North America
  US: { x: 22, y: 42 },
  CA: { x: 20, y: 32 },
  MX: { x: 18, y: 52 },

  // Europe
  GB: { x: 47, y: 32 },
  DE: { x: 52, y: 34 },
  FR: { x: 49, y: 38 },
  ES: { x: 46, y: 42 },
  IT: { x: 52, y: 42 },
  NL: { x: 50, y: 33 },
  SE: { x: 54, y: 26 },
  NO: { x: 52, y: 24 },
  DK: { x: 51, y: 30 },
  FI: { x: 57, y: 24 },
  PL: { x: 55, y: 34 },
  CH: { x: 51, y: 38 },
  AT: { x: 53, y: 38 },
  BE: { x: 50, y: 35 },
  IE: { x: 45, y: 32 },
  PT: { x: 44, y: 42 },
  CZ: { x: 54, y: 35 },
  UA: { x: 60, y: 35 },
  RO: { x: 57, y: 40 },

  // Asia
  RU: { x: 75, y: 28 },
  JP: { x: 88, y: 40 },
  CN: { x: 78, y: 42 },
  IN: { x: 72, y: 52 },
  KR: { x: 86, y: 42 },
  SG: { x: 78, y: 62 },
  HK: { x: 82, y: 50 },
  TW: { x: 84, y: 50 },
  TH: { x: 77, y: 56 },
  VN: { x: 79, y: 54 },
  PH: { x: 84, y: 56 },
  ID: { x: 82, y: 64 },
  MY: { x: 78, y: 60 },
  IL: { x: 62, y: 46 },
  TR: { x: 60, y: 42 },

  // Oceania
  AU: { x: 86, y: 74 },
  NZ: { x: 94, y: 78 },

  // South America
  BR: { x: 30, y: 68 },
  AR: { x: 28, y: 78 },
  CL: { x: 26, y: 76 },
  CO: { x: 24, y: 60 },
  PE: { x: 24, y: 66 },

  // Africa
  ZA: { x: 57, y: 76 },

  // Default for unknown
  UNKNOWN: { x: 50, y: 50 },
};

const TIER_COLORS: Record<string, string> = {
  free: 'var(--color-nebula-400)',
  pro: 'var(--color-electric-400)',
  team: 'var(--color-photon-400)',
  enterprise: 'var(--color-solar-400)',
};

const RECENT_THRESHOLD_MS = 30000; // 30 seconds

// ============================================================================
// Helper Functions
// ============================================================================

const getCountryCoords = (countryCode?: string): { x: number; y: number } => {
  if (!countryCode) return COUNTRY_COORDINATES.UNKNOWN;
  return COUNTRY_COORDINATES[countryCode.toUpperCase()] || COUNTRY_COORDINATES.UNKNOWN;
};

const isRecentActivity = (lastActivity: string): boolean => {
  const now = Date.now();
  const activityTime = new Date(lastActivity).getTime();
  return now - activityTime < RECENT_THRESHOLD_MS;
};

const formatLastSeen = (timestamp: string): string => {
  const now = Date.now();
  const time = new Date(timestamp).getTime();
  const diffMs = now - time;

  if (diffMs < 60000) return 'Just now';
  if (diffMs < 3600000) return `${Math.floor(diffMs / 60000)}m ago`;
  if (diffMs < 86400000) return `${Math.floor(diffMs / 3600000)}h ago`;
  return `${Math.floor(diffMs / 86400000)}d ago`;
};

const getCountryFlag = (code?: string): string => {
  if (!code || code.length !== 2) return '';
  const codePoints = code
    .toUpperCase()
    .split('')
    .map((char) => 127397 + char.charCodeAt(0));
  return String.fromCodePoint(...codePoints);
};

// ============================================================================
// Sub-Components
// ============================================================================

interface SessionTooltipProps {
  data: TooltipData | null;
}

const SessionTooltip: Component<SessionTooltipProps> = (props) => {
  return (
    <Show when={props.data}>
      {(data) => (
        <div
          class="pointer-events-none absolute z-50 min-w-[200px] rounded-xl border border-white/10 bg-void-900/95 p-3 shadow-2xl backdrop-blur-xl"
          style={{
            left: `${data().x}%`,
            top: `${data().y}%`,
            transform: 'translate(-50%, -110%)',
          }}
        >
          {/* Header */}
          <div class="mb-2 flex items-center gap-2">
            <div class="text-lg">{getCountryFlag(data().session.geo?.country_code)}</div>
            <div>
              <div class="text-xs font-bold text-white">
                {data().session.geo?.city || data().session.geo?.country || 'Unknown Location'}
              </div>
              <div class="text-2xs text-nebula-500">
                {data().session.geo?.region && `${data().session.geo.region}, `}
                {data().session.geo?.country}
              </div>
            </div>
          </div>

          {/* Session Details */}
          <div class="space-y-1.5 border-t border-white/5 pt-2">
            <div class="flex items-center gap-2 text-2xs">
              <Laptop size={10} class="text-nebula-500" />
              <span class="text-nebula-400">{data().session.hostname || data().session.machine_id.slice(0, 8)}</span>
            </div>
            <div class="flex items-center gap-2 text-2xs">
              <Server size={10} class="text-nebula-500" />
              <span class="text-nebula-400">{data().session.platform} v{data().session.version}</span>
            </div>
            <div class="flex items-center gap-2 text-2xs">
              <Activity size={10} class="text-nebula-500" />
              <span class="text-nebula-400">{data().session.command_count} commands</span>
            </div>
            <div class="flex items-center gap-2 text-2xs">
              <Clock size={10} class="text-nebula-500" />
              <span class="text-nebula-400">Last active: {formatLastSeen(data().session.last_activity_at)}</span>
            </div>
          </div>

          {/* Tier Badge */}
          <div class="mt-2 flex items-center justify-between">
            <span
              class="rounded-full px-2 py-0.5 text-2xs font-bold uppercase"
              style={{
                background: `color-mix(in srgb, ${TIER_COLORS[data().session.license_tier]} 15%, transparent)`,
                color: TIER_COLORS[data().session.license_tier],
              }}
            >
              {data().session.license_tier}
            </span>
            <Show when={isRecentActivity(data().session.last_activity_at)}>
              <span class="flex items-center gap-1 text-2xs text-aurora-400">
                <span class="h-1.5 w-1.5 animate-pulse rounded-full bg-aurora-500" />
                Active
              </span>
            </Show>
          </div>
        </div>
      )}
    </Show>
  );
};

// ============================================================================
// Map Visualization Component
// ============================================================================

interface WorldMapProps {
  points: MapPoint[];
  onHover: (data: TooltipData | null) => void;
}

const WorldMap: Component<WorldMapProps> = (props) => {
  // Simple world map outline paths (simplified for performance)
  const continentPaths = `
    M 10,35 Q 15,30 25,32 L 30,40 Q 35,55 28,65 Q 20,75 28,80 L 26,82 Q 20,78 22,70 Q 18,55 15,50 Q 8,42 10,35 Z
    M 44,28 Q 55,22 60,25 L 65,30 Q 68,35 62,45 Q 55,50 48,48 Q 42,45 42,38 Q 42,32 44,28 Z
    M 55,48 L 62,46 Q 68,50 70,55 Q 72,62 65,70 Q 58,78 55,75 Q 52,68 53,60 Q 54,52 55,48 Z
    M 68,30 Q 85,22 92,28 L 90,45 Q 88,52 80,58 L 72,60 Q 68,55 70,48 Q 72,38 68,30 Z
    M 75,58 Q 82,60 88,68 L 92,75 Q 90,82 85,80 Q 78,75 75,65 Q 74,60 75,58 Z
  `;

  return (
    <svg
      viewBox="0 0 100 90"
      class="h-full w-full"
      preserveAspectRatio="xMidYMid meet"
    >
      {/* Grid lines */}
      <defs>
        <pattern id="map-grid" width="10" height="10" patternUnits="userSpaceOnUse">
          <path d="M 10 0 L 0 0 0 10" fill="none" stroke="var(--color-void-700)" stroke-width="0.1" />
        </pattern>
        <radialGradient id="point-glow">
          <stop offset="0%" stop-color="currentColor" stop-opacity="0.8" />
          <stop offset="100%" stop-color="currentColor" stop-opacity="0" />
        </radialGradient>
      </defs>

      {/* Background grid */}
      <rect width="100" height="90" fill="url(#map-grid)" />

      {/* Continent outlines */}
      <path
        d={continentPaths}
        fill="var(--color-void-750)"
        stroke="var(--color-nebula-700)"
        stroke-width="0.3"
        class="transition-all duration-500"
      />

      {/* Session points */}
      <For each={props.points}>
        {(point) => (
          <g
            class="cursor-pointer transition-all duration-300"
            onMouseEnter={() => props.onHover({ session: point.session, x: point.x, y: point.y })}
            onMouseLeave={() => props.onHover(null)}
          >
            {/* Glow effect for recent activity */}
            <Show when={point.isRecent}>
              <circle
                cx={point.x}
                cy={point.y}
                r="3"
                fill="none"
                stroke={TIER_COLORS[point.session.license_tier]}
                stroke-width="0.5"
                opacity="0.5"
                class="animate-ping"
              />
              <circle
                cx={point.x}
                cy={point.y}
                r="2"
                fill={TIER_COLORS[point.session.license_tier]}
                opacity="0.3"
                class="animate-pulse"
              />
            </Show>

            {/* Main point */}
            <circle
              cx={point.x}
              cy={point.y}
              r={point.isRecent ? 1.2 : 0.8}
              fill={TIER_COLORS[point.session.license_tier]}
              class="transition-all duration-300 hover:r-[1.5]"
              style={{
                filter: point.isRecent ? `drop-shadow(0 0 2px ${TIER_COLORS[point.session.license_tier]})` : undefined,
              }}
            />
          </g>
        )}
      </For>
    </svg>
  );
};

// ============================================================================
// Stats Panel Component
// ============================================================================

interface StatsPanelProps {
  sessions: SessionEvent[];
  sessionsByCountry: Map<string, SessionEvent[]>;
}

const StatsPanel: Component<StatsPanelProps> = (props) => {
  const topCountries = createMemo(() => {
    const sorted = Array.from(props.sessionsByCountry.entries())
      .sort((a, b) => b[1].length - a[1].length)
      .slice(0, 5);
    return sorted;
  });

  const tierCounts = createMemo(() => {
    const counts = { free: 0, pro: 0, team: 0, enterprise: 0 };
    for (const session of props.sessions) {
      counts[session.license_tier as keyof typeof counts]++;
    }
    return counts;
  });

  const activeCount = createMemo(() =>
    props.sessions.filter((s) => isRecentActivity(s.last_activity_at)).length
  );

  return (
    <div class="space-y-4">
      {/* Summary Stats */}
      <div class="grid grid-cols-2 gap-3">
        <div class="rounded-xl border border-white/5 bg-void-800/50 p-3">
          <div class="flex items-center gap-2">
            <Users size={14} class="text-indigo-400" />
            <span class="text-2xs font-bold uppercase text-nebula-500">Total Sessions</span>
          </div>
          <div class="mt-1 font-display text-2xl font-black text-white">
            {props.sessions.length}
          </div>
        </div>
        <div class="rounded-xl border border-white/5 bg-void-800/50 p-3">
          <div class="flex items-center gap-2">
            <Activity size={14} class="text-aurora-400" />
            <span class="text-2xs font-bold uppercase text-nebula-500">Active Now</span>
          </div>
          <div class="mt-1 font-display text-2xl font-black text-aurora-400">
            {activeCount()}
          </div>
        </div>
      </div>

      {/* Top Countries */}
      <div class="rounded-xl border border-white/5 bg-void-800/50 p-3">
        <div class="mb-2 text-2xs font-bold uppercase text-nebula-500">Top Regions</div>
        <div class="space-y-2">
          <For each={topCountries()}>
            {([countryCode, sessions]) => (
              <div class="flex items-center gap-2">
                <span class="text-sm">{getCountryFlag(countryCode)}</span>
                <span class="flex-1 text-xs font-medium text-nebula-300">
                  {countryCode}
                </span>
                <span class="font-mono text-xs font-bold text-white">
                  {sessions.length}
                </span>
              </div>
            )}
          </For>
          <Show when={topCountries().length === 0}>
            <div class="text-center text-xs text-nebula-500 italic">
              No regional data yet
            </div>
          </Show>
        </div>
      </div>

      {/* Tier Breakdown */}
      <div class="rounded-xl border border-white/5 bg-void-800/50 p-3">
        <div class="mb-2 text-2xs font-bold uppercase text-nebula-500">By Tier</div>
        <div class="space-y-2">
          <For each={Object.entries(tierCounts()) as [string, number][]}>
            {([tier, count]) => (
              <Show when={count > 0}>
                <div class="flex items-center gap-2">
                  <div
                    class="h-2 w-2 rounded-full"
                    style={{ background: TIER_COLORS[tier] }}
                  />
                  <span class="flex-1 text-xs font-medium capitalize text-nebula-300">
                    {tier}
                  </span>
                  <span class="font-mono text-xs font-bold text-white">{count}</span>
                </div>
              </Show>
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

// ============================================================================
// Main Component
// ============================================================================

export const ActiveSessionsMap: Component<ActiveSessionsMapProps> = (props) => {
  const [tooltipData, setTooltipData] = createSignal<TooltipData | null>(null);
  const [mounted, setMounted] = createSignal(false);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  // Convert sessions to map points
  const mapPoints = createMemo((): MapPoint[] => {
    const points: MapPoint[] = [];
    const seen = new Set<string>();

    for (const session of props.sessions) {
      const countryCode = session.geo?.country_code || 'UNKNOWN';
      const coords = getCountryCoords(countryCode);

      // Add slight jitter for sessions in same country to prevent overlap
      const key = `${countryCode}-${session.session_id}`;
      if (!seen.has(key)) {
        seen.add(key);
        const jitterX = (Math.random() - 0.5) * 3;
        const jitterY = (Math.random() - 0.5) * 3;

        points.push({
          id: session.session_id,
          x: coords.x + jitterX,
          y: coords.y + jitterY,
          session,
          isRecent: isRecentActivity(session.last_activity_at),
        });
      }
    }

    return points;
  });

  // Sessions by country for stats
  const sessionsByCountry = createMemo(() => {
    const map = new Map<string, SessionEvent[]>();
    for (const session of props.sessions) {
      const code = session.geo?.country_code || 'UNKNOWN';
      const existing = map.get(code) || [];
      existing.push(session);
      map.set(code, existing);
    }
    return map;
  });

  return (
    <div
      class={cn(
        'relative overflow-hidden rounded-2xl border border-white/[0.06] bg-void-900 shadow-2xl',
        props.class
      )}
    >
      {/* Ambient glow effects */}
      <div class="pointer-events-none absolute -right-20 -top-20 h-48 w-48 rounded-full bg-indigo-500/10 blur-[80px]" />
      <div class="pointer-events-none absolute -bottom-20 -left-20 h-48 w-48 rounded-full bg-photon-500/10 blur-[80px]" />

      {/* Header */}
      <div class="relative border-b border-white/5 bg-gradient-to-r from-white/[0.02] to-transparent p-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-electric-600">
              <Globe size={20} class="text-white" />
            </div>
            <div>
              <h2 class="font-display text-lg font-black tracking-tight text-white">
                Active Sessions Map
              </h2>
              <p class="text-2xs text-nebula-500">
                {props.sessions.length} sessions across {sessionsByCountry().size} regions
              </p>
            </div>
          </div>

          <LiveIndicator
            label={props.isLive ? 'Live' : 'Offline'}
            color={props.isLive ? 'success' : 'warning'}
            variant="pulse"
            size="sm"
          />
        </div>
      </div>

      {/* Content */}
      <div class="flex flex-col lg:flex-row">
        {/* Map Area */}
        <div class="relative flex-1 p-4">
          <div
            class={cn(
              'relative aspect-[16/9] overflow-hidden rounded-xl border border-white/5 bg-void-850',
              'transition-opacity duration-500',
              mounted() ? 'opacity-100' : 'opacity-0'
            )}
          >
            <WorldMap points={mapPoints()} onHover={setTooltipData} />
            <SessionTooltip data={tooltipData()} />

            {/* Legend */}
            <div class="absolute bottom-3 left-3 flex items-center gap-3 rounded-lg bg-void-900/80 px-3 py-2 backdrop-blur-sm">
              <div class="flex items-center gap-1.5">
                <span class="h-2 w-2 animate-pulse rounded-full bg-aurora-500" />
                <span class="text-2xs text-nebula-400">Active</span>
              </div>
              <div class="flex items-center gap-1.5">
                <span class="h-2 w-2 rounded-full bg-nebula-500" />
                <span class="text-2xs text-nebula-400">Idle</span>
              </div>
            </div>

            {/* Empty State */}
            <Show when={props.sessions.length === 0}>
              <div class="absolute inset-0 flex flex-col items-center justify-center bg-void-850/80">
                <MapPin size={32} class="mb-3 text-nebula-600" />
                <p class="text-sm font-bold text-nebula-400">No Active Sessions</p>
                <p class="mt-1 text-xs text-nebula-600">Sessions will appear here as users connect</p>
              </div>
            </Show>
          </div>
        </div>

        {/* Stats Sidebar */}
        <div class="w-full border-t border-white/5 p-4 lg:w-64 lg:border-l lg:border-t-0">
          <StatsPanel sessions={props.sessions} sessionsByCountry={sessionsByCountry()} />
        </div>
      </div>
    </div>
  );
};

export default ActiveSessionsMap;
