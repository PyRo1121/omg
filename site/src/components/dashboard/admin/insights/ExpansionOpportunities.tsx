import { Component, For, Show } from 'solid-js';
import { TrendingUp, ArrowUpCircle, Users, Clock } from 'lucide-solid';

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
}

export const ExpansionOpportunities: Component<ExpansionOpportunitiesProps> = (props) => {
  const getPriorityColor = (priority: string) => {
    switch (priority) {
      case 'high':
        return 'border-emerald-500/30 bg-emerald-500/10 text-emerald-400';
      case 'medium':
        return 'border-amber-500/30 bg-amber-500/10 text-amber-400';
      default:
        return 'border-slate-500/30 bg-slate-500/10 text-slate-400';
    }
  };

  const formatOpportunityType = (type: string) => {
    switch (type) {
      case 'upsell_to_pro':
        return 'Upgrade to Pro';
      case 'upsell_to_team':
        return 'Upgrade to Team';
      case 'upsell_to_enterprise':
        return 'Upgrade to Enterprise';
      case 'seat_expansion':
        return 'Add More Seats';
      default:
        return type;
    }
  };

  const getOpportunityIcon = (type: string) => {
    if (type === 'seat_expansion') {
      return <Users size={16} class="text-indigo-400" />;
    }
    return <TrendingUp size={16} class="text-emerald-400" />;
  };

  const highPriorityCount = () => props.data.filter((o) => o.priority === 'high').length;

  return (
    <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
      <div class="mb-6 flex items-center justify-between">
        <div>
          <h3 class="flex items-center gap-2 text-2xl font-black tracking-tight text-white">
            <ArrowUpCircle size={24} class="text-emerald-400" />
            Expansion Opportunities
          </h3>
          <p class="mt-1 text-sm text-slate-500">
            {props.data.length} ready for upsell ({highPriorityCount()} high priority)
          </p>
        </div>
      </div>

      <Show when={props.data.length === 0}>
        <div class="py-12 text-center text-slate-400">No expansion opportunities found</div>
      </Show>

      <div class="space-y-3">
        <For each={props.data.slice(0, 10)}>
          {(opp) => (
            <div class="group rounded-xl border border-white/5 bg-white/5 p-4 transition-all hover:border-white/10 hover:bg-white/10">
              <div class="flex items-start justify-between gap-4">
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <p class="truncate text-sm font-bold text-white">{opp.email}</p>
                    <span
                      class={`shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-black uppercase ${getPriorityColor(opp.priority)}`}
                    >
                      {opp.priority}
                    </span>
                  </div>
                  <Show when={opp.company}>
                    <p class="mt-0.5 text-xs text-slate-400">{opp.company}</p>
                  </Show>

                  <div class="mt-3 flex flex-wrap items-center gap-4 text-xs">
                    <div class="flex items-center gap-1.5">
                      {getOpportunityIcon(opp.opportunity_type)}
                      <span class="font-medium text-slate-300">
                        {formatOpportunityType(opp.opportunity_type)}
                      </span>
                    </div>
                    <div class="text-slate-500">
                      <span class="capitalize">{opp.tier}</span> tier
                    </div>
                    <div class="text-slate-500">
                      {opp.active_machines}/{opp.max_seats} seats
                    </div>
                    <div class="text-slate-500">
                      {(opp.total_commands_30d ?? 0).toLocaleString()} cmds/30d
                    </div>
                    <div class="flex items-center gap-1 text-slate-500">
                      <Clock size={12} />
                      {opp.hours_saved_30d ?? 0}h saved
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}
        </For>
      </div>

      <Show when={props.data.length > 10}>
        <div class="mt-4 text-center text-sm text-slate-500">
          Showing top 10 of {props.data.length} opportunities
        </div>
      </Show>

      <Show when={highPriorityCount() > 0}>
        <div class="mt-6 rounded-xl border border-emerald-500/30 bg-emerald-500/5 p-4">
          <p class="text-sm font-medium text-emerald-400">
            💰 High Priority: {highPriorityCount()} customers showing strong expansion signals
          </p>
        </div>
      </Show>
    </div>
  );
};
