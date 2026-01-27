import { Component, For, Show, createMemo, createSignal, onMount } from 'solid-js';
import { TrendingUp, ArrowUpCircle, Users, Clock, DollarSign, Mail, ChevronRight, Target, Crown, Zap, Star, Sparkles } from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface ExpansionOpportunity {
  customer_id: string;
  email: string;
  company: string | null;
  tier: string;
  active_machines: number;
  max_seats: number;
  total_commands_30d: number;
  hours_saved_30d: number;
  opportunity_type: string;
  priority: string;
}

interface ExpansionOpportunitiesProps {
  data: ExpansionOpportunity[];
  onOpportunityClick?: (customerId: string) => void;
}

type PriorityLevel = 'urgent' | 'high' | 'medium' | 'low';

interface PriorityConfig {
  color: string;
  bgColor: string;
  borderColor: string;
  glowColor: string;
  gradient: string;
  label: string;
  rank: number;
}

const PRIORITY_CONFIG: Record<PriorityLevel, PriorityConfig> = {
  urgent: {
    color: 'var(--color-flare-400)',
    bgColor: 'rgba(239, 68, 68, 0.1)',
    borderColor: 'rgba(239, 68, 68, 0.25)',
    glowColor: 'rgba(239, 68, 68, 0.3)',
    gradient: 'linear-gradient(135deg, var(--color-flare-600), var(--color-flare-400))',
    label: 'Urgent',
    rank: 4,
  },
  high: {
    color: 'var(--color-aurora-400)',
    bgColor: 'rgba(16, 185, 129, 0.1)',
    borderColor: 'rgba(16, 185, 129, 0.25)',
    glowColor: 'rgba(16, 185, 129, 0.3)',
    gradient: 'linear-gradient(135deg, var(--color-aurora-600), var(--color-aurora-400))',
    label: 'High',
    rank: 3,
  },
  medium: {
    color: 'var(--color-solar-400)',
    bgColor: 'rgba(245, 158, 11, 0.1)',
    borderColor: 'rgba(245, 158, 11, 0.2)',
    glowColor: 'rgba(245, 158, 11, 0.25)',
    gradient: 'linear-gradient(135deg, var(--color-solar-600), var(--color-solar-400))',
    label: 'Medium',
    rank: 2,
  },
  low: {
    color: 'var(--color-nebula-400)',
    bgColor: 'rgba(161, 161, 170, 0.08)',
    borderColor: 'rgba(255, 255, 255, 0.08)',
    glowColor: 'rgba(161, 161, 170, 0.15)',
    gradient: 'linear-gradient(135deg, var(--color-nebula-600), var(--color-nebula-400))',
    label: 'Low',
    rank: 1,
  },
};

type OpportunityType = 'upsell_to_pro' | 'upsell_to_team' | 'upsell_to_enterprise' | 'seat_expansion';

interface OpportunityConfig {
  icon: typeof TrendingUp;
  label: string;
  potentialMRR: number;
  color: string;
  gradient: string;
}

const OPPORTUNITY_TYPE_CONFIG: Record<OpportunityType, OpportunityConfig> = {
  upsell_to_pro: {
    icon: Zap,
    label: 'Upgrade to Pro',
    potentialMRR: 9,
    color: 'var(--color-indigo-400)',
    gradient: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))',
  },
  upsell_to_team: {
    icon: Users,
    label: 'Upgrade to Team',
    potentialMRR: 29,
    color: 'var(--color-electric-400)',
    gradient: 'linear-gradient(135deg, var(--color-electric-600), var(--color-electric-400))',
  },
  upsell_to_enterprise: {
    icon: Crown,
    label: 'Upgrade to Enterprise',
    potentialMRR: 199,
    color: 'var(--color-solar-400)',
    gradient: 'linear-gradient(135deg, var(--color-solar-600), var(--color-solar-400))',
  },
  seat_expansion: {
    icon: Users,
    label: 'Add More Seats',
    potentialMRR: 15,
    color: 'var(--color-photon-400)',
    gradient: 'linear-gradient(135deg, var(--color-photon-600), var(--color-photon-400))',
  },
};

function getPriorityLevel(priority: string): PriorityLevel {
  const lower = priority.toLowerCase();
  if (lower === 'urgent') return 'urgent';
  if (lower === 'high') return 'high';
  if (lower === 'medium') return 'medium';
  return 'low';
}

function getOpportunityTypeConfig(type: string): OpportunityConfig {
  return (
    OPPORTUNITY_TYPE_CONFIG[type as OpportunityType] || {
      icon: TrendingUp,
      label: type
        .replace(/_/g, ' ')
        .replace(/\b\w/g, (c) => c.toUpperCase()),
      potentialMRR: 0,
      color: 'var(--color-aurora-400)',
      gradient: 'linear-gradient(135deg, var(--color-aurora-600), var(--color-aurora-400))',
    }
  );
}

export const ExpansionOpportunities: Component<ExpansionOpportunitiesProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [hoveredId, setHoveredId] = createSignal<string | null>(null);
  const [showAll, setShowAll] = createSignal(false);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const sortedOpportunities = createMemo(() =>
    [...props.data].sort((a, b) => {
      return (PRIORITY_CONFIG[getPriorityLevel(b.priority)]?.rank || 0) - (PRIORITY_CONFIG[getPriorityLevel(a.priority)]?.rank || 0);
    })
  );

  const displayedOpportunities = createMemo(() => (showAll() ? sortedOpportunities() : sortedOpportunities().slice(0, 6)));

  const highPriorityCount = createMemo(() => props.data.filter((o) => ['high', 'urgent'].includes(getPriorityLevel(o.priority))).length);

  const totalPotentialMRR = createMemo(() =>
    props.data.reduce((sum, opp) => {
      const config = getOpportunityTypeConfig(opp.opportunity_type);
      return sum + config.potentialMRR;
    }, 0)
  );

  const totalPotentialARR = createMemo(() => totalPotentialMRR() * 12);

  return (
    <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl relative overflow-hidden">
      <div
        class="absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl opacity-15 transition-opacity duration-500"
        style={{ background: 'var(--color-aurora-500)' }}
      />

      <div class="mb-6 flex items-start justify-between relative">
        <div>
          <div class="flex items-center gap-3 mb-1">
            <div
              class="w-10 h-10 rounded-xl flex items-center justify-center"
              style={{
                background: 'linear-gradient(135deg, var(--color-aurora-600), var(--color-aurora-400))',
                'box-shadow': '0 0 15px rgba(16, 185, 129, 0.3)',
              }}
            >
              <Target size={20} class="text-white" />
            </div>
            <div>
              <h3 class="text-lg font-bold tracking-tight text-nebula-100">Expansion Pipeline</h3>
              <p class="text-xs text-nebula-500">
                <span class="font-bold text-nebula-300">{props.data.length}</span> opportunities
                <Show when={highPriorityCount() > 0}>
                  {' '}
                  •{' '}
                  <span class="font-bold" style={{ color: 'var(--color-aurora-400)' }}>
                    {highPriorityCount()} high priority
                  </span>
                </Show>
              </p>
            </div>
          </div>
        </div>

        <Show when={totalPotentialMRR() > 0}>
          <div
            class="rounded-xl border px-4 py-2"
            style={{
              background: 'rgba(16, 185, 129, 0.1)',
              'border-color': 'rgba(16, 185, 129, 0.25)',
              'box-shadow': '0 0 15px rgba(16, 185, 129, 0.2)',
            }}
          >
            <div class="flex items-center gap-3">
              <div
                class="w-8 h-8 rounded-lg flex items-center justify-center"
                style={{ background: 'rgba(16, 185, 129, 0.2)' }}
              >
                <DollarSign size={14} style={{ color: 'var(--color-aurora-400)' }} />
              </div>
              <div>
                <p class="text-2xs font-bold uppercase tracking-wider text-nebula-500">Potential ARR</p>
                <p class="text-xl font-black tabular-nums" style={{ color: 'var(--color-aurora-400)' }}>
                  ${totalPotentialARR().toLocaleString()}
                </p>
              </div>
            </div>
          </div>
        </Show>
      </div>

      <Show when={props.data.length === 0}>
        <div class="flex flex-col items-center justify-center py-12">
          <div class="w-16 h-16 rounded-full bg-void-800 flex items-center justify-center mb-4">
            <ArrowUpCircle size={32} class="text-nebula-600" />
          </div>
          <p class="text-lg font-bold text-nebula-200">No Opportunities Yet</p>
          <p class="mt-1 text-sm text-nebula-500">Expansion opportunities will appear as customers grow</p>
        </div>
      </Show>

      <div class="space-y-2">
        <For each={displayedOpportunities()}>
          {(opp, index) => {
            const priorityLevel = getPriorityLevel(opp.priority);
            const priorityConfig = PRIORITY_CONFIG[priorityLevel];
            const typeConfig = getOpportunityTypeConfig(opp.opportunity_type);
            const IconComponent = typeConfig.icon;
            const isHovered = () => hoveredId() === opp.customer_id;
            const isHighPriority = priorityLevel === 'high' || priorityLevel === 'urgent';
            const seatUtilization = opp.max_seats > 0 ? (opp.active_machines / opp.max_seats) * 100 : 0;

            return (
              <div
                class={cn(
                  'group relative cursor-pointer overflow-hidden rounded-xl border p-4',
                  'transition-all duration-300',
                  isHighPriority && isHovered() && 'scale-[1.01]',
                  mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
                )}
                style={{
                  background: 'rgba(255, 255, 255, 0.02)',
                  'border-color': isHovered() ? priorityConfig.borderColor : 'rgba(255, 255, 255, 0.04)',
                  'box-shadow': isHovered() && isHighPriority ? `0 0 20px ${priorityConfig.glowColor}` : undefined,
                  'animation-delay': `${index() * 50}ms`,
                }}
                onMouseEnter={() => setHoveredId(opp.customer_id)}
                onMouseLeave={() => setHoveredId(null)}
                onClick={() => props.onOpportunityClick?.(opp.customer_id)}
              >
                <div class="flex items-start justify-between gap-4">
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2">
                      <div
                        class={cn(
                          'w-9 h-9 rounded-lg flex items-center justify-center text-xs font-black text-white',
                          'transition-transform duration-300',
                          isHovered() && 'scale-110'
                        )}
                        style={{
                          background: typeConfig.gradient,
                          'box-shadow': isHovered() ? `0 0 12px ${typeConfig.color}80` : undefined,
                        }}
                      >
                        {opp.email.charAt(0).toUpperCase()}
                      </div>
                      <div class="min-w-0 flex-1">
                        <p class="truncate text-sm font-semibold text-nebula-200">{opp.email}</p>
                        <Show when={opp.company}>
                          <p class="truncate text-2xs text-nebula-500">{opp.company}</p>
                        </Show>
                      </div>
                      <span
                        class="shrink-0 rounded-full border px-2 py-0.5 text-2xs font-black uppercase"
                        style={{
                          color: priorityConfig.color,
                          background: priorityConfig.bgColor,
                          'border-color': priorityConfig.borderColor,
                        }}
                      >
                        {priorityConfig.label}
                      </span>
                    </div>

                    <div class="mt-3 flex flex-wrap items-center gap-2 text-xs">
                      <div
                        class="flex items-center gap-1.5 rounded-lg px-2 py-1"
                        style={{
                          background: `${typeConfig.color}15`,
                          color: typeConfig.color,
                        }}
                      >
                        <IconComponent size={12} />
                        <span class="font-bold">{typeConfig.label}</span>
                      </div>

                      <div class="flex items-center gap-1 text-nebula-500">
                        <span class="capitalize">{opp.tier}</span>
                      </div>

                      <div class="flex items-center gap-1 text-nebula-500">
                        <Users size={12} />
                        <span class="font-mono tabular-nums">
                          {opp.active_machines}/{opp.max_seats}
                        </span>
                        <Show when={seatUtilization >= 80}>
                          <span
                            class="ml-1 rounded px-1 text-2xs font-bold"
                            style={{
                              background: 'rgba(245, 158, 11, 0.1)',
                              color: 'var(--color-solar-400)',
                            }}
                          >
                            {Math.round(seatUtilization)}%
                          </span>
                        </Show>
                      </div>

                      <div class="flex items-center gap-1 text-nebula-500">
                        <span class="font-mono tabular-nums">{(opp.total_commands_30d ?? 0).toLocaleString()}</span>
                        <span>cmds</span>
                      </div>

                      <div class="flex items-center gap-1 text-nebula-500">
                        <Clock size={12} />
                        <span class="font-mono tabular-nums">{opp.hours_saved_30d ?? 0}h</span>
                      </div>
                    </div>
                  </div>

                  <div
                    class={cn(
                      'flex items-center gap-2 transition-all duration-200',
                      isHovered() ? 'opacity-100 translate-x-0' : 'opacity-0 translate-x-2'
                    )}
                  >
                    <button
                      class="rounded-lg bg-void-700/50 p-2 text-nebula-400 transition-all hover:bg-void-600/50 hover:text-white hover:scale-110"
                      onClick={(e) => {
                        e.stopPropagation();
                        window.open(`mailto:${opp.email}`);
                      }}
                      title="Send email"
                    >
                      <Mail size={14} />
                    </button>
                    <ChevronRight size={16} class="text-nebula-600" />
                  </div>
                </div>

                <Show when={typeConfig.potentialMRR > 0}>
                  <div
                    class="mt-3 flex items-center justify-between rounded-lg px-3 py-2"
                    style={{ background: 'rgba(16, 185, 129, 0.05)' }}
                  >
                    <span class="text-2xs text-nebula-500">Potential MRR increase</span>
                    <span class="font-mono text-sm font-bold" style={{ color: 'var(--color-aurora-400)' }}>
                      +${typeConfig.potentialMRR}/mo
                    </span>
                  </div>
                </Show>
              </div>
            );
          }}
        </For>
      </div>

      <Show when={props.data.length > 6}>
        <button
          onClick={() => setShowAll(!showAll())}
          class={cn(
            'mt-4 w-full py-2.5 text-sm font-medium rounded-xl',
            'border border-white/[0.06] bg-void-800/30',
            'text-nebula-400 hover:text-nebula-200',
            'transition-all duration-200 hover:bg-void-750/50'
          )}
        >
          {showAll() ? 'Show Less' : `Show All ${props.data.length} Opportunities`}
        </button>
      </Show>

      <Show when={highPriorityCount() > 0}>
        <div
          class={cn(
            'mt-6 rounded-xl border p-4 transition-all duration-500',
            mounted() ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4'
          )}
          style={{
            background: 'linear-gradient(135deg, rgba(16, 185, 129, 0.1), rgba(34, 211, 211, 0.05))',
            'border-color': 'rgba(16, 185, 129, 0.25)',
          }}
        >
          <div class="flex items-center gap-3">
            <div
              class="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0"
              style={{ background: 'rgba(16, 185, 129, 0.2)' }}
            >
              <Sparkles size={18} style={{ color: 'var(--color-aurora-400)' }} />
            </div>
            <div class="flex-1">
              <p class="text-sm font-semibold text-nebula-100">
                {highPriorityCount()} high-priority expansion opportunities identified
              </p>
              <p class="mt-0.5 text-xs text-nebula-500">
                Estimated ${(totalPotentialMRR() * 0.6).toFixed(0)}/mo if 60% convert
              </p>
            </div>
            <button
              class="rounded-xl px-4 py-2 text-sm font-bold text-white transition-all hover:brightness-110 hover:scale-105"
              style={{
                background: 'linear-gradient(135deg, var(--color-aurora-600), var(--color-aurora-400))',
                'box-shadow': '0 0 15px rgba(16, 185, 129, 0.3)',
              }}
            >
              Start Campaigns
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
};
