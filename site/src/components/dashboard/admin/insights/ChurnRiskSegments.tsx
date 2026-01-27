import { Component, For, Show, createMemo, createSignal, onMount } from 'solid-js';
import { AlertTriangle, AlertCircle, Info, CheckCircle, ChevronRight, Mail, Phone, Users, TrendingDown } from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface ChurnRiskSegment {
  risk_segment: string;
  user_count: number;
  avg_monthly_commands: number;
  tier: string;
}

interface ChurnRiskSegmentsProps {
  data: ChurnRiskSegment[];
  onSegmentClick?: (segment: string) => void;
}

type RiskLevel = 'critical' | 'high' | 'medium' | 'low' | 'healthy';

interface RiskConfig {
  icon: typeof AlertTriangle;
  color: string;
  bgColor: string;
  borderColor: string;
  glowColor: string;
  gradient: string;
  label: string;
  priority: number;
}

const RISK_CONFIG: Record<RiskLevel, RiskConfig> = {
  critical: {
    icon: AlertTriangle,
    color: 'var(--color-flare-400)',
    bgColor: 'rgba(239, 68, 68, 0.1)',
    borderColor: 'rgba(239, 68, 68, 0.2)',
    glowColor: 'rgba(239, 68, 68, 0.4)',
    gradient: 'linear-gradient(135deg, var(--color-flare-600), var(--color-flare-400))',
    label: 'Critical',
    priority: 5,
  },
  high: {
    icon: AlertCircle,
    color: 'var(--color-solar-400)',
    bgColor: 'rgba(245, 158, 11, 0.1)',
    borderColor: 'rgba(245, 158, 11, 0.2)',
    glowColor: 'rgba(245, 158, 11, 0.35)',
    gradient: 'linear-gradient(135deg, var(--color-solar-600), var(--color-solar-400))',
    label: 'High',
    priority: 4,
  },
  medium: {
    icon: Info,
    color: 'var(--color-indigo-400)',
    bgColor: 'rgba(99, 102, 241, 0.1)',
    borderColor: 'rgba(99, 102, 241, 0.2)',
    glowColor: 'rgba(99, 102, 241, 0.3)',
    gradient: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))',
    label: 'Medium',
    priority: 3,
  },
  low: {
    icon: Info,
    color: 'var(--color-plasma-400)',
    bgColor: 'rgba(90, 154, 232, 0.1)',
    borderColor: 'rgba(90, 154, 232, 0.15)',
    glowColor: 'rgba(90, 154, 232, 0.2)',
    gradient: 'linear-gradient(135deg, var(--color-plasma-600), var(--color-plasma-400))',
    label: 'Low',
    priority: 2,
  },
  healthy: {
    icon: CheckCircle,
    color: 'var(--color-aurora-400)',
    bgColor: 'rgba(16, 185, 129, 0.1)',
    borderColor: 'rgba(16, 185, 129, 0.2)',
    glowColor: 'rgba(16, 185, 129, 0.3)',
    gradient: 'linear-gradient(135deg, var(--color-aurora-600), var(--color-aurora-400))',
    label: 'Healthy',
    priority: 1,
  },
};

function getRiskLevel(segment: string): RiskLevel {
  const lower = segment.toLowerCase();
  if (lower.includes('critical')) return 'critical';
  if (lower.includes('high')) return 'high';
  if (lower.includes('medium')) return 'medium';
  if (lower.includes('low')) return 'low';
  return 'healthy';
}

function formatSegmentName(segment: string): string {
  return segment
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

export const ChurnRiskSegments: Component<ChurnRiskSegmentsProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [hoveredSegment, setHoveredSegment] = createSignal<string | null>(null);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const sortedSegments = createMemo(() =>
    [...props.data].sort((a, b) => {
      const riskA = RISK_CONFIG[getRiskLevel(a.risk_segment)].priority;
      const riskB = RISK_CONFIG[getRiskLevel(b.risk_segment)].priority;
      return riskB - riskA;
    })
  );

  const totalAtRisk = createMemo(() =>
    props.data
      .filter((s) => getRiskLevel(s.risk_segment) !== 'healthy')
      .reduce((sum, s) => sum + s.user_count, 0)
  );

  const criticalCount = createMemo(() =>
    props.data
      .filter((s) => getRiskLevel(s.risk_segment) === 'critical')
      .reduce((sum, s) => sum + s.user_count, 0)
  );

  const highCount = createMemo(() =>
    props.data
      .filter((s) => getRiskLevel(s.risk_segment) === 'high')
      .reduce((sum, s) => sum + s.user_count, 0)
  );

  const estimatedMRRAtRisk = createMemo(() => {
    return props.data
      .filter((s) => ['critical', 'high'].includes(getRiskLevel(s.risk_segment)))
      .reduce((sum, s) => {
        const tierMultiplier = s.tier === 'enterprise' ? 199 : s.tier === 'team' ? 29 : s.tier === 'pro' ? 9 : 0;
        return sum + s.user_count * tierMultiplier;
      }, 0);
  });

  const riskDistribution = createMemo(() => {
    const total = props.data.reduce((sum, s) => sum + s.user_count, 0);
    if (total === 0) return { critical: 0, high: 0, medium: 0, low: 0, healthy: 0 };

    return {
      critical: (criticalCount() / total) * 100,
      high: (highCount() / total) * 100,
      medium: (props.data.filter((s) => getRiskLevel(s.risk_segment) === 'medium').reduce((sum, s) => sum + s.user_count, 0) / total) * 100,
      low: (props.data.filter((s) => getRiskLevel(s.risk_segment) === 'low').reduce((sum, s) => sum + s.user_count, 0) / total) * 100,
      healthy: (props.data.filter((s) => getRiskLevel(s.risk_segment) === 'healthy').reduce((sum, s) => sum + s.user_count, 0) / total) * 100,
    };
  });

  return (
    <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl relative overflow-hidden">
      <div
        class={cn(
          'absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl transition-opacity duration-500',
          criticalCount() > 0 ? 'opacity-20' : 'opacity-10'
        )}
        style={{
          background: criticalCount() > 0 ? 'var(--color-flare-500)' : 'var(--color-solar-500)',
        }}
      />

      <div class="mb-6 flex items-start justify-between relative">
        <div>
          <div class="flex items-center gap-3 mb-1">
            <div class="relative">
              <div
                class="w-10 h-10 rounded-xl flex items-center justify-center"
                style={{
                  background: RISK_CONFIG.critical.gradient,
                  'box-shadow': criticalCount() > 0 ? `0 0 20px ${RISK_CONFIG.critical.glowColor}` : undefined,
                }}
              >
                <TrendingDown size={20} class="text-white" />
              </div>
              <Show when={criticalCount() > 0}>
                <div class="absolute -right-1 -top-1 flex h-4 w-4 items-center justify-center rounded-full bg-flare-500 animate-pulse">
                  <span class="text-[8px] font-black text-white">!</span>
                </div>
              </Show>
            </div>
            <div>
              <h3 class="text-lg font-bold tracking-tight text-nebula-100">Churn Risk Analysis</h3>
              <p class="text-xs text-nebula-500">
                <span class="font-bold text-flare-400">{totalAtRisk()}</span> customers need attention
              </p>
            </div>
          </div>
        </div>

        <Show when={estimatedMRRAtRisk() > 0}>
          <div
            class="rounded-xl border px-4 py-2 text-center"
            style={{
              background: RISK_CONFIG.critical.bgColor,
              'border-color': RISK_CONFIG.critical.borderColor,
              'box-shadow': `0 0 15px ${RISK_CONFIG.critical.glowColor}`,
            }}
          >
            <p class="text-2xs font-bold uppercase tracking-wider text-nebula-500">MRR at Risk</p>
            <p class="text-xl font-black tabular-nums" style={{ color: RISK_CONFIG.critical.color }}>
              ${estimatedMRRAtRisk().toLocaleString()}
            </p>
          </div>
        </Show>
      </div>

      <div
        class={cn(
          'mb-4 h-2 rounded-full overflow-hidden flex transition-all duration-700',
          mounted() ? 'opacity-100' : 'opacity-0'
        )}
        style={{ background: 'var(--color-void-700)' }}
      >
        <div
          class="h-full transition-all duration-1000"
          style={{ width: `${riskDistribution().critical}%`, background: RISK_CONFIG.critical.color }}
        />
        <div
          class="h-full transition-all duration-1000"
          style={{ width: `${riskDistribution().high}%`, background: RISK_CONFIG.high.color }}
        />
        <div
          class="h-full transition-all duration-1000"
          style={{ width: `${riskDistribution().medium}%`, background: RISK_CONFIG.medium.color }}
        />
        <div
          class="h-full transition-all duration-1000"
          style={{ width: `${riskDistribution().low}%`, background: RISK_CONFIG.low.color }}
        />
        <div
          class="h-full transition-all duration-1000"
          style={{ width: `${riskDistribution().healthy}%`, background: RISK_CONFIG.healthy.color }}
        />
      </div>

      <Show when={props.data.length === 0}>
        <div class="flex flex-col items-center justify-center py-12">
          <div
            class="w-16 h-16 rounded-full flex items-center justify-center mb-4"
            style={{ background: RISK_CONFIG.healthy.bgColor }}
          >
            <CheckCircle size={32} style={{ color: RISK_CONFIG.healthy.color }} />
          </div>
          <p class="text-lg font-bold text-nebula-200">All Clear!</p>
          <p class="mt-1 text-sm text-nebula-500">No churn risk detected</p>
        </div>
      </Show>

      <div class="space-y-2">
        <For each={sortedSegments()}>
          {(segment, index) => {
            const riskLevel = getRiskLevel(segment.risk_segment);
            const config = RISK_CONFIG[riskLevel];
            const IconComponent = config.icon;
            const isHovered = () => hoveredSegment() === segment.risk_segment;
            const isCriticalOrHigh = riskLevel === 'critical' || riskLevel === 'high';

            return (
              <div
                class={cn(
                  'group relative cursor-pointer overflow-hidden rounded-xl border p-4',
                  'transition-all duration-300',
                  isHovered() && isCriticalOrHigh && 'scale-[1.01]',
                  mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
                )}
                style={{
                  background: config.bgColor,
                  'border-color': isHovered() ? config.borderColor : 'rgba(255,255,255,0.04)',
                  'box-shadow': isHovered() && isCriticalOrHigh ? `0 0 25px ${config.glowColor}` : undefined,
                  'animation-delay': `${index() * 50}ms`,
                }}
                onMouseEnter={() => setHoveredSegment(segment.risk_segment)}
                onMouseLeave={() => setHoveredSegment(null)}
                onClick={() => props.onSegmentClick?.(segment.risk_segment)}
              >
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-3">
                    <div
                      class={cn(
                        'w-9 h-9 rounded-lg flex items-center justify-center',
                        'transition-all duration-300',
                        isHovered() && 'scale-110'
                      )}
                      style={{
                        background: config.gradient,
                        'box-shadow': isHovered() ? `0 0 12px ${config.glowColor}` : `0 0 6px ${config.glowColor}`,
                      }}
                    >
                      <IconComponent size={16} class="text-white" />
                    </div>
                    <div>
                      <p class="text-sm font-semibold" style={{ color: config.color }}>
                        {formatSegmentName(segment.risk_segment)}
                      </p>
                      <div class="mt-0.5 flex items-center gap-2 text-2xs text-nebula-500">
                        <span class="capitalize">{segment.tier}</span>
                        <span>•</span>
                        <span class="font-mono tabular-nums">{Math.round(segment.avg_monthly_commands)} cmds/mo</span>
                      </div>
                    </div>
                  </div>

                  <div class="flex items-center gap-3">
                    <div class="text-right">
                      <p class="text-xl font-black text-nebula-100 tabular-nums">{segment.user_count}</p>
                      <p class="text-2xs text-nebula-500">customers</p>
                    </div>

                    <div
                      class={cn(
                        'flex gap-1 transition-all duration-200',
                        isHovered() ? 'opacity-100 translate-x-0' : 'opacity-0 translate-x-2'
                      )}
                    >
                      <button
                        class={cn(
                          'rounded-lg p-2 transition-all hover:scale-110',
                          'bg-void-700/50 hover:bg-void-600/50'
                        )}
                        onClick={(e) => {
                          e.stopPropagation();
                        }}
                        title="Send outreach email"
                      >
                        <Mail size={14} style={{ color: config.color }} />
                      </button>
                      <button
                        class={cn(
                          'rounded-lg p-2 transition-all hover:scale-110',
                          'bg-void-700/50 hover:bg-void-600/50'
                        )}
                        onClick={(e) => {
                          e.stopPropagation();
                        }}
                        title="Schedule call"
                      >
                        <Phone size={14} style={{ color: config.color }} />
                      </button>
                      <ChevronRight size={16} class="text-nebula-600 self-center" />
                    </div>
                  </div>
                </div>

                <Show when={isCriticalOrHigh}>
                  <div class="mt-3 h-1 overflow-hidden rounded-full bg-void-700">
                    <div
                      class={cn(
                        'h-full rounded-full transition-all duration-700',
                        riskLevel === 'critical' && 'animate-pulse'
                      )}
                      style={{
                        width: mounted() ? `${Math.min((segment.user_count / totalAtRisk()) * 100, 100)}%` : '0%',
                        background: config.gradient,
                      }}
                    />
                  </div>
                </Show>
              </div>
            );
          }}
        </For>
      </div>

      <Show when={criticalCount() > 0 || highCount() > 0}>
        <div
          class={cn(
            'mt-6 rounded-xl border p-4 transition-all duration-500',
            mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
          )}
          style={{
            background: `linear-gradient(135deg, ${RISK_CONFIG.critical.bgColor}, ${RISK_CONFIG.high.bgColor})`,
            'border-color': RISK_CONFIG.critical.borderColor,
          }}
        >
          <div class="flex items-center gap-3">
            <div
              class="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0"
              style={{ background: RISK_CONFIG.critical.gradient }}
            >
              <AlertTriangle size={18} class="text-white" />
            </div>
            <div class="flex-1">
              <p class="text-sm font-semibold text-nebula-100">
                {criticalCount() > 0 ? `${criticalCount()} critical` : ''}
                {criticalCount() > 0 && highCount() > 0 ? ' + ' : ''}
                {highCount() > 0 ? `${highCount()} high-risk` : ''} customers need immediate attention
              </p>
              <p class="mt-0.5 text-xs text-nebula-500">Recommended: Personal outreach within 48 hours</p>
            </div>
            <button
              class="rounded-xl px-4 py-2 text-sm font-bold text-white transition-all hover:brightness-110 hover:scale-105"
              style={{
                background: RISK_CONFIG.critical.gradient,
                'box-shadow': `0 0 15px ${RISK_CONFIG.critical.glowColor}`,
              }}
            >
              Start Outreach
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
};
