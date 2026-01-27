import { Component, Show } from 'solid-js';

interface HealthBadgeProps {
  score: number;
  stage: string;
}

export const HealthBadge: Component<HealthBadgeProps> = (props) => {
  const getHealthColor = (score: number) => {
    if (score >= 80) return 'text-emerald-400 bg-emerald-500/10 border-emerald-500/20';
    if (score >= 60) return 'text-cyan-400 bg-cyan-500/10 border-cyan-500/20';
    if (score >= 40) return 'text-amber-400 bg-amber-500/10 border-amber-500/20';
    return 'text-rose-400 bg-rose-500/10 border-rose-500/20';
  };

  const getStageIcon = (stage: string) => {
    switch (stage) {
      case 'power_user':
        return { icon: '⚡', label: 'Power User' };
      case 'active':
        return { icon: '✓', label: 'Active' };
      case 'at_risk':
        return { icon: '⚠️', label: 'At Risk' };
      case 'churned':
        return { icon: '×', label: 'Churned' };
      case 'new':
        return { icon: '✨', label: 'New' };
      default:
        return { icon: '·', label: 'Unknown' };
    }
  };

  const stageInfo = () => getStageIcon(props.stage);

  return (
    <div class="flex items-center gap-2">
      <div
        class={`rounded-full border px-2.5 py-1 text-xs font-bold tabular-nums ${getHealthColor(props.score)}`}
        title={`Health Score: ${props.score}/100`}
      >
        {props.score}
      </div>
      <span class="text-sm" title={stageInfo().label}>
        {stageInfo().icon}
      </span>
    </div>
  );
};
