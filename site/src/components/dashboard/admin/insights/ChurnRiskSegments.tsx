import { Component, For, Show } from 'solid-js';
import { AlertTriangle, AlertCircle, Info, CheckCircle } from 'lucide-solid';

interface ChurnRiskSegment {
  risk_segment: string;
  user_count: number;
  avg_monthly_commands: number;
  tier: string;
}

interface ChurnRiskSegmentsProps {
  data: ChurnRiskSegment[];
}

export const ChurnRiskSegments: Component<ChurnRiskSegmentsProps> = (props) => {
  const getRiskColor = (segment: string) => {
    switch (segment) {
      case 'critical_churn_risk':
        return 'border-rose-500/30 bg-rose-500/10';
      case 'high_churn_risk':
        return 'border-orange-500/30 bg-orange-500/10';
      case 'medium_churn_risk':
        return 'border-amber-500/30 bg-amber-500/10';
      case 'low_engagement':
        return 'border-yellow-500/30 bg-yellow-500/10';
      default:
        return 'border-emerald-500/30 bg-emerald-500/10';
    }
  };

  const getRiskIcon = (segment: string) => {
    switch (segment) {
      case 'critical_churn_risk':
        return <AlertTriangle size={18} class="text-rose-400" />;
      case 'high_churn_risk':
        return <AlertCircle size={18} class="text-orange-400" />;
      case 'medium_churn_risk':
        return <Info size={18} class="text-amber-400" />;
      case 'low_engagement':
        return <Info size={18} class="text-yellow-400" />;
      default:
        return <CheckCircle size={18} class="text-emerald-400" />;
    }
  };

  const getRiskTextColor = (segment: string) => {
    switch (segment) {
      case 'critical_churn_risk':
        return 'text-rose-400';
      case 'high_churn_risk':
        return 'text-orange-400';
      case 'medium_churn_risk':
        return 'text-amber-400';
      case 'low_engagement':
        return 'text-yellow-400';
      default:
        return 'text-emerald-400';
    }
  };

  const formatSegmentName = (segment: string) => {
    return segment
      .split('_')
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
  };

  const totalAtRisk = () =>
    props.data
      .filter((s) => s.risk_segment !== 'healthy')
      .reduce((sum, s) => sum + s.user_count, 0);

  return (
    <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
      <div class="mb-6 flex items-center justify-between">
        <div>
          <h3 class="flex items-center gap-2 text-2xl font-black tracking-tight text-white">
            <AlertTriangle size={24} class="text-rose-400" />
            Churn Risk Analysis
          </h3>
          <p class="mt-1 text-sm text-slate-500">
            {totalAtRisk()} customers need attention
          </p>
        </div>
      </div>

      <Show when={props.data.length === 0}>
        <div class="py-12 text-center text-slate-400">No churn risk data available</div>
      </Show>

      <div class="space-y-3">
        <For each={props.data}>
          {(segment) => (
            <div class={`rounded-xl border p-4 ${getRiskColor(segment.risk_segment)}`}>
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  {getRiskIcon(segment.risk_segment)}
                  <div>
                    <p class={`text-sm font-bold ${getRiskTextColor(segment.risk_segment)}`}>
                      {formatSegmentName(segment.risk_segment)}
                    </p>
                    <p class="mt-0.5 text-xs text-slate-400 capitalize">{segment.tier} tier</p>
                  </div>
                </div>
                <div class="text-right">
                  <p class="text-2xl font-black text-white">{segment.user_count}</p>
                  <p class="text-xs text-slate-500">
                    {Math.round(segment.avg_monthly_commands)} avg cmds/mo
                  </p>
                </div>
              </div>
            </div>
          )}
        </For>
      </div>

      <Show when={totalAtRisk() > 0}>
        <div class="mt-6 rounded-xl border border-rose-500/30 bg-rose-500/5 p-4">
          <p class="text-sm font-medium text-rose-400">
            💡 Action Required: Reach out to high-risk customers before they churn
          </p>
        </div>
      </Show>
    </div>
  );
};
