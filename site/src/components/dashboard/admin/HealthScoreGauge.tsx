import { Component, Show, createSignal, createMemo, createEffect, onMount, onCleanup } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { TrendingUp, TrendingDown, Minus, Activity, Heart, Users, ThumbsUp, Info } from 'lucide-solid';
import type { HealthUpdate } from '../../../hooks/useRealtimeData';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// ============================================================================
// Types
// ============================================================================

interface HealthScoreGaugeProps {
  health: HealthUpdate | null;
  size?: 'sm' | 'md' | 'lg' | 'xl';
  animated?: boolean;
  showBreakdown?: boolean;
  class?: string;
}

interface SubScore {
  label: string;
  value: number;
  icon: typeof Activity;
  color: string;
}

type HealthZone = 'critical' | 'poor' | 'fair' | 'good' | 'excellent';

// ============================================================================
// Constants
// ============================================================================

const SIZE_CONFIG = {
  sm: { diameter: 120, strokeWidth: 8, fontSize: 'text-2xl', labelSize: 'text-xs', arrowSize: 14 },
  md: { diameter: 180, strokeWidth: 10, fontSize: 'text-4xl', labelSize: 'text-sm', arrowSize: 18 },
  lg: { diameter: 240, strokeWidth: 12, fontSize: 'text-5xl', labelSize: 'text-base', arrowSize: 22 },
  xl: { diameter: 300, strokeWidth: 14, fontSize: 'text-6xl', labelSize: 'text-lg', arrowSize: 26 },
};

const HEALTH_ZONES: Record<HealthZone, {
  label: string;
  range: [number, number];
  color: string;
  bgColor: string;
  glowColor: string;
}> = {
  critical: {
    label: 'Critical',
    range: [0, 20],
    color: 'var(--color-flare-500, #ef4444)',
    bgColor: 'bg-flare-500/10',
    glowColor: 'rgba(239, 68, 68, 0.5)',
  },
  poor: {
    label: 'Poor',
    range: [21, 33],
    color: 'var(--color-flare-400, #f87171)',
    bgColor: 'bg-flare-400/10',
    glowColor: 'rgba(248, 113, 113, 0.4)',
  },
  fair: {
    label: 'Fair',
    range: [34, 66],
    color: 'var(--color-solar-500, #f59e0b)',
    bgColor: 'bg-solar-500/10',
    glowColor: 'rgba(245, 158, 11, 0.4)',
  },
  good: {
    label: 'Good',
    range: [67, 85],
    color: 'var(--color-electric-500, #22d3d3)',
    bgColor: 'bg-electric-500/10',
    glowColor: 'rgba(34, 211, 211, 0.4)',
  },
  excellent: {
    label: 'Excellent',
    range: [86, 100],
    color: 'var(--color-aurora-500, #10b981)',
    bgColor: 'bg-aurora-500/10',
    glowColor: 'rgba(16, 185, 129, 0.5)',
  },
};

// ============================================================================
// Helper Functions
// ============================================================================

const getHealthZone = (score: number): HealthZone => {
  if (score <= 20) return 'critical';
  if (score <= 33) return 'poor';
  if (score <= 66) return 'fair';
  if (score <= 85) return 'good';
  return 'excellent';
};

const getGradientStops = (): string => {
  return `
    <stop offset="0%" stop-color="${HEALTH_ZONES.critical.color}" />
    <stop offset="20%" stop-color="${HEALTH_ZONES.poor.color}" />
    <stop offset="50%" stop-color="${HEALTH_ZONES.fair.color}" />
    <stop offset="75%" stop-color="${HEALTH_ZONES.good.color}" />
    <stop offset="100%" stop-color="${HEALTH_ZONES.excellent.color}" />
  `;
};

const getTrendDiff = (current: number, previous?: number): number | null => {
  if (previous === undefined) return null;
  return current - previous;
};

// ============================================================================
// Sub-Components
// ============================================================================

interface TrendArrowProps {
  trend: 'up' | 'down' | 'stable';
  diff: number | null;
  size: number;
}

const TrendArrow: Component<TrendArrowProps> = (props) => {
  const color = createMemo(() => {
    switch (props.trend) {
      case 'up':
        return 'text-aurora-400';
      case 'down':
        return 'text-flare-400';
      default:
        return 'text-nebula-500';
    }
  });

  const Icon = createMemo(() => {
    switch (props.trend) {
      case 'up':
        return TrendingUp;
      case 'down':
        return TrendingDown;
      default:
        return Minus;
    }
  });

  return (
    <div class={cn('flex items-center gap-1', color())}>
      {(() => {
        const IconComponent = Icon();
        return <IconComponent size={props.size} />;
      })()}
      <Show when={props.diff !== null}>
        <span class="font-mono text-xs font-bold tabular-nums">
          {props.diff! > 0 ? '+' : ''}{props.diff!.toFixed(0)}
        </span>
      </Show>
    </div>
  );
};

interface BreakdownTooltipProps {
  engagement: number;
  adoption: number;
  satisfaction: number;
  visible: boolean;
  position: { x: number; y: number };
}

const BreakdownTooltip: Component<BreakdownTooltipProps> = (props) => {
  const subScores: SubScore[] = [
    { label: 'Engagement', value: props.engagement, icon: Activity, color: 'text-electric-400' },
    { label: 'Adoption', value: props.adoption, icon: Users, color: 'text-photon-400' },
    { label: 'Satisfaction', value: props.satisfaction, icon: ThumbsUp, color: 'text-aurora-400' },
  ];

  return (
    <Show when={props.visible}>
      <div
        class="pointer-events-none absolute z-50 min-w-[180px] rounded-xl border border-white/10 bg-void-900/95 p-4 shadow-2xl backdrop-blur-xl"
        style={{
          left: `${props.position.x}px`,
          top: `${props.position.y}px`,
          transform: 'translate(-50%, 10px)',
        }}
      >
        <div class="mb-3 flex items-center gap-2">
          <Heart size={14} class="text-flare-400" />
          <span class="text-xs font-bold uppercase tracking-wider text-nebula-400">
            Health Breakdown
          </span>
        </div>
        <div class="space-y-3">
          {subScores.map((sub) => (
            <div>
              <div class="mb-1 flex items-center justify-between">
                <div class="flex items-center gap-1.5">
                  <sub.icon size={12} class={sub.color} />
                  <span class="text-xs font-medium text-nebula-300">{sub.label}</span>
                </div>
                <span class={cn('font-mono text-xs font-bold tabular-nums', sub.color)}>
                  {sub.value}
                </span>
              </div>
              <div class="h-1.5 overflow-hidden rounded-full bg-void-700">
                <div
                  class="h-full rounded-full transition-all duration-500"
                  style={{
                    width: `${sub.value}%`,
                    background: `linear-gradient(90deg, ${HEALTH_ZONES[getHealthZone(sub.value)].color}, ${HEALTH_ZONES[getHealthZone(sub.value)].color}88)`,
                  }}
                />
              </div>
            </div>
          ))}
        </div>
      </div>
    </Show>
  );
};

// ============================================================================
// Main Component
// ============================================================================

export const HealthScoreGauge: Component<HealthScoreGaugeProps> = (props) => {
  let gaugeRef: HTMLDivElement | undefined;

  const [displayScore, setDisplayScore] = createSignal(0);
  const [showTooltip, setShowTooltip] = createSignal(false);
  const [tooltipPos, setTooltipPos] = createSignal({ x: 0, y: 0 });
  const [mounted, setMounted] = createSignal(false);

  const size = () => props.size || 'md';
  const config = () => SIZE_CONFIG[size()];

  const score = createMemo(() => props.health?.overall_score ?? 0);
  const zone = createMemo(() => getHealthZone(score()));
  const zoneConfig = createMemo(() => HEALTH_ZONES[zone()]);

  const diff = createMemo(() => getTrendDiff(score(), props.health?.previous_score));
  const trend = createMemo(() => props.health?.trend ?? 'stable');

  // Gauge calculations
  const radius = createMemo(() => (config().diameter - config().strokeWidth) / 2);
  const circumference = createMemo(() => Math.PI * radius()); // Semi-circle
  const offset = createMemo(() => circumference() - (score() / 100) * circumference());

  // Animated score counter
  createEffect(() => {
    if (!props.animated || !mounted()) {
      setDisplayScore(score());
      return;
    }

    const target = score();
    const start = displayScore();
    const duration = 1000;
    const startTime = performance.now();

    const animate = (currentTime: number) => {
      const elapsed = currentTime - startTime;
      const progress = Math.min(elapsed / duration, 1);

      // Ease out cubic
      const eased = 1 - Math.pow(1 - progress, 3);
      const current = Math.round(start + (target - start) * eased);

      setDisplayScore(current);

      if (progress < 1) {
        requestAnimationFrame(animate);
      }
    };

    requestAnimationFrame(animate);
  });

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const handleMouseEnter = (e: MouseEvent) => {
    if (!props.showBreakdown || !props.health) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setTooltipPos({
      x: rect.left + rect.width / 2,
      y: rect.bottom,
    });
    setShowTooltip(true);
  };

  const handleMouseLeave = () => {
    setShowTooltip(false);
  };

  // Unique gradient ID
  const gradientId = `health-gauge-gradient-${Math.random().toString(36).substr(2, 9)}`;
  const glowId = `health-gauge-glow-${Math.random().toString(36).substr(2, 9)}`;

  return (
    <div
      ref={gaugeRef}
      class={cn('relative inline-flex flex-col items-center', props.class)}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {/* SVG Gauge */}
      <svg
        width={config().diameter}
        height={config().diameter / 2 + 20}
        class="overflow-visible"
      >
        <defs>
          {/* Gradient for the arc */}
          <linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stop-color={HEALTH_ZONES.critical.color} />
            <stop offset="33%" stop-color={HEALTH_ZONES.fair.color} />
            <stop offset="66%" stop-color={HEALTH_ZONES.good.color} />
            <stop offset="100%" stop-color={HEALTH_ZONES.excellent.color} />
          </linearGradient>

          {/* Glow filter */}
          <filter id={glowId} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="4" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Background arc (full semi-circle) */}
        <path
          d={`M ${config().strokeWidth} ${config().diameter / 2} A ${radius()} ${radius()} 0 0 1 ${config().diameter - config().strokeWidth} ${config().diameter / 2}`}
          fill="none"
          stroke="var(--color-void-700)"
          stroke-width={config().strokeWidth}
          stroke-linecap="round"
        />

        {/* Color segments background */}
        <path
          d={`M ${config().strokeWidth} ${config().diameter / 2} A ${radius()} ${radius()} 0 0 1 ${config().diameter - config().strokeWidth} ${config().diameter / 2}`}
          fill="none"
          stroke={`url(#${gradientId})`}
          stroke-width={config().strokeWidth}
          stroke-linecap="round"
          opacity="0.15"
        />

        {/* Active arc (shows current score) */}
        <path
          d={`M ${config().strokeWidth} ${config().diameter / 2} A ${radius()} ${radius()} 0 0 1 ${config().diameter - config().strokeWidth} ${config().diameter / 2}`}
          fill="none"
          stroke={zoneConfig().color}
          stroke-width={config().strokeWidth}
          stroke-linecap="round"
          stroke-dasharray={circumference()}
          stroke-dashoffset={mounted() ? offset() : circumference()}
          filter={`url(#${glowId})`}
          class="transition-all duration-1000 ease-out"
          style={{
            '--gauge-color': zoneConfig().color,
          }}
        />

        {/* Tick marks */}
        {[0, 25, 50, 75, 100].map((tick) => {
          const angle = Math.PI * (1 - tick / 100);
          const innerR = radius() - config().strokeWidth - 4;
          const outerR = radius() - config().strokeWidth + 2;
          const cx = config().diameter / 2;
          const cy = config().diameter / 2;

          return (
            <line
              x1={cx + innerR * Math.cos(angle)}
              y1={cy - innerR * Math.sin(angle)}
              x2={cx + outerR * Math.cos(angle)}
              y2={cy - outerR * Math.sin(angle)}
              stroke="var(--color-nebula-600)"
              stroke-width="2"
              stroke-linecap="round"
            />
          );
        })}

        {/* Needle/indicator */}
        {(() => {
          const angle = Math.PI * (1 - score() / 100);
          const cx = config().diameter / 2;
          const cy = config().diameter / 2;
          const needleLength = radius() - config().strokeWidth - 15;

          return (
            <g
              class="transition-transform duration-1000 ease-out"
              style={{
                'transform-origin': `${cx}px ${cy}px`,
                transform: `rotate(${mounted() ? 0 : 180}deg)`,
              }}
            >
              <circle
                cx={cx}
                cy={cy}
                r="6"
                fill={zoneConfig().color}
                class="transition-colors duration-500"
              />
              <line
                x1={cx}
                y1={cy}
                x2={cx + needleLength * Math.cos(angle)}
                y2={cy - needleLength * Math.sin(angle)}
                stroke={zoneConfig().color}
                stroke-width="3"
                stroke-linecap="round"
                class="transition-all duration-1000 ease-out"
              />
            </g>
          );
        })()}
      </svg>

      {/* Score Display */}
      <div class="absolute bottom-0 left-1/2 -translate-x-1/2 flex flex-col items-center">
        <div class="flex items-center gap-2">
          <span
            class={cn('font-display font-black tabular-nums transition-colors duration-500', config().fontSize)}
            style={{ color: zoneConfig().color }}
          >
            {displayScore()}
          </span>
          <TrendArrow trend={trend()} diff={diff()} size={config().arrowSize} />
        </div>
        <span
          class={cn(
            'font-bold uppercase tracking-wider transition-colors duration-500',
            config().labelSize
          )}
          style={{ color: zoneConfig().color }}
        >
          {zoneConfig().label}
        </span>
      </div>

      {/* Info Icon for breakdown hint */}
      <Show when={props.showBreakdown && props.health}>
        <div class="absolute right-0 top-0">
          <Info size={14} class="text-nebula-500 hover:text-nebula-300 transition-colors cursor-help" />
        </div>
      </Show>

      {/* Breakdown Tooltip */}
      <Show when={props.health}>
        <BreakdownTooltip
          engagement={props.health!.engagement_score}
          adoption={props.health!.adoption_score}
          satisfaction={props.health!.satisfaction_score}
          visible={showTooltip()}
          position={tooltipPos()}
        />
      </Show>

      {/* Empty State */}
      <Show when={!props.health}>
        <div class="absolute inset-0 flex items-center justify-center">
          <div class="text-center">
            <Heart size={24} class="mx-auto mb-2 text-nebula-600" />
            <p class="text-xs text-nebula-500">No health data</p>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default HealthScoreGauge;
