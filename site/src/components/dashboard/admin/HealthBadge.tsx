import { Component, createMemo, Switch, Match } from 'solid-js';
import {
  Sparkles,
  Zap,
  Activity,
  AlertTriangle,
  TrendingDown,
  XCircle,
  RefreshCw,
  Rocket,
  Crown,
} from 'lucide-solid';

interface HealthBadgeProps {
  score: number;
  stage: string;
  size?: 'sm' | 'md' | 'lg';
  showGlow?: boolean;
  variant?: 'badge' | 'pill' | 'ring';
}

type HealthConfig = {
  text: string;
  bg: string;
  border: string;
  glow: string;
  ring: string;
  label: string;
};

type StageConfig = {
  label: string;
  color: string;
  bg: string;
};

const HEALTH_THRESHOLDS = {
  EXCELLENT: 81,
  GOOD: 61,
  FAIR: 41,
  POOR: 21,
} as const;

const SIZE_CONFIG = {
  sm: { badge: 'px-2 py-0.5 text-2xs', icon: 10, gap: 'gap-1.5' },
  md: { badge: 'px-2.5 py-1 text-xs', icon: 12, gap: 'gap-2' },
  lg: { badge: 'px-3 py-1.5 text-sm', icon: 14, gap: 'gap-2.5' },
} as const;

const STAGE_CONFIG: Record<string, StageConfig> = {
  new: { label: 'New', color: 'text-[var(--lifecycle-new)]', bg: 'bg-[var(--lifecycle-new-bg)]' },
  onboarding: { label: 'Onboarding', color: 'text-[var(--lifecycle-onboarding)]', bg: 'bg-[var(--lifecycle-onboarding-bg)]' },
  activated: { label: 'Activated', color: 'text-[var(--lifecycle-activated)]', bg: 'bg-[var(--lifecycle-activated-bg)]' },
  engaged: { label: 'Engaged', color: 'text-[var(--lifecycle-engaged)]', bg: 'bg-[var(--lifecycle-engaged-bg)]' },
  power_user: { label: 'Power User', color: 'text-[var(--lifecycle-power-user)]', bg: 'bg-[var(--lifecycle-power-user-bg)]' },
  at_risk: { label: 'At Risk', color: 'text-[var(--lifecycle-at-risk)]', bg: 'bg-[var(--lifecycle-at-risk-bg)]' },
  churning: { label: 'Churning', color: 'text-[var(--lifecycle-churning)]', bg: 'bg-[var(--lifecycle-churning-bg)]' },
  churned: { label: 'Churned', color: 'text-[var(--lifecycle-churned)]', bg: 'bg-[var(--lifecycle-churned-bg)]' },
  reactivated: { label: 'Reactivated', color: 'text-[var(--lifecycle-reactivated)]', bg: 'bg-[var(--lifecycle-reactivated-bg)]' },
  default: { label: 'Active', color: 'text-[var(--lifecycle-engaged)]', bg: 'bg-[var(--lifecycle-engaged-bg)]' },
};

function getHealthConfig(score: number, showGlow: boolean): HealthConfig {
  if (score >= HEALTH_THRESHOLDS.EXCELLENT) {
    return {
      text: 'text-[var(--color-aurora-400)]',
      bg: 'bg-[var(--health-excellent-bg)]',
      border: 'border-[var(--color-aurora-500)]/25',
      glow: showGlow ? 'shadow-[0_0_12px_var(--health-excellent-glow)]' : '',
      ring: 'ring-[var(--color-aurora-500)]',
      label: 'Excellent',
    };
  }
  if (score >= HEALTH_THRESHOLDS.GOOD) {
    return {
      text: 'text-[var(--color-electric-400)]',
      bg: 'bg-[var(--health-good-bg)]',
      border: 'border-[var(--color-electric-500)]/25',
      glow: showGlow ? 'shadow-[0_0_10px_var(--health-good-glow)]' : '',
      ring: 'ring-[var(--color-electric-500)]',
      label: 'Good',
    };
  }
  if (score >= HEALTH_THRESHOLDS.FAIR) {
    return {
      text: 'text-[var(--color-solar-400)]',
      bg: 'bg-[var(--health-fair-bg)]',
      border: 'border-[var(--color-solar-500)]/25',
      glow: showGlow ? 'shadow-[0_0_8px_var(--health-fair-glow)]' : '',
      ring: 'ring-[var(--color-solar-500)]',
      label: 'Fair',
    };
  }
  if (score >= HEALTH_THRESHOLDS.POOR) {
    return {
      text: 'text-[var(--color-flare-400)]',
      bg: 'bg-[var(--health-poor-bg)]',
      border: 'border-[var(--color-flare-400)]/25',
      glow: showGlow ? 'shadow-[0_0_10px_var(--health-poor-glow)]' : '',
      ring: 'ring-[var(--color-flare-400)]',
      label: 'Poor',
    };
  }
  return {
    text: 'text-[var(--color-flare-500)]',
    bg: 'bg-[var(--health-critical-bg)]',
    border: 'border-[var(--color-flare-500)]/30',
    glow: showGlow ? 'shadow-[0_0_16px_var(--health-critical-glow)] animate-pulse' : '',
    ring: 'ring-[var(--color-flare-500)]',
    label: 'Critical',
  };
}

function getStrokeColor(score: number): string {
  if (score >= HEALTH_THRESHOLDS.EXCELLENT) return 'var(--color-aurora-500)';
  if (score >= HEALTH_THRESHOLDS.GOOD) return 'var(--color-electric-500)';
  if (score >= HEALTH_THRESHOLDS.FAIR) return 'var(--color-solar-500)';
  return 'var(--color-flare-500)';
}

function getRingDimensions(size: 'sm' | 'md' | 'lg') {
  const dimensions = {
    sm: { width: 32, height: 32, cx: 16, cy: 16, r: 12, strokeWidth: 3, factor: 0.67 },
    md: { width: 40, height: 40, cx: 20, cy: 20, r: 16, strokeWidth: 3, factor: 0.89 },
    lg: { width: 48, height: 48, cx: 24, cy: 24, r: 20, strokeWidth: 4, factor: 1.11 },
  };
  return dimensions[size];
}

const StageIcon: Component<{ stage: string; size: number; class: string }> = (props) => {
  return (
    <Switch fallback={<Activity size={props.size} class={props.class} />}>
      <Match when={props.stage === 'new'}>
        <Sparkles size={props.size} class={props.class} />
      </Match>
      <Match when={props.stage === 'onboarding'}>
        <Rocket size={props.size} class={props.class} />
      </Match>
      <Match when={props.stage === 'activated'}>
        <Zap size={props.size} class={props.class} />
      </Match>
      <Match when={props.stage === 'engaged'}>
        <Activity size={props.size} class={props.class} />
      </Match>
      <Match when={props.stage === 'power_user'}>
        <Crown size={props.size} class={props.class} />
      </Match>
      <Match when={props.stage === 'at_risk'}>
        <AlertTriangle size={props.size} class={props.class} />
      </Match>
      <Match when={props.stage === 'churning'}>
        <TrendingDown size={props.size} class={props.class} />
      </Match>
      <Match when={props.stage === 'churned'}>
        <XCircle size={props.size} class={props.class} />
      </Match>
      <Match when={props.stage === 'reactivated'}>
        <RefreshCw size={props.size} class={props.class} />
      </Match>
    </Switch>
  );
};

export const HealthBadge: Component<HealthBadgeProps> = (props) => {
  const size = () => props.size || 'md';
  const showGlow = () => props.showGlow !== false;
  const variant = () => props.variant || 'badge';

  const healthConfig = createMemo(() => getHealthConfig(props.score, showGlow()));
  const stageConfig = createMemo(() => STAGE_CONFIG[props.stage] || STAGE_CONFIG.default);
  const sizes = createMemo(() => SIZE_CONFIG[size()]);

  if (variant() === 'ring') {
    const CIRCUMFERENCE = 2 * Math.PI * 18;
    const offset = createMemo(() => CIRCUMFERENCE - (props.score / 100) * CIRCUMFERENCE);
    const ringDims = createMemo(() => getRingDimensions(size()));

    return (
      <div class={`relative flex items-center ${sizes().gap}`}>
        <div class="relative">
          <svg width={ringDims().width} height={ringDims().height} class="-rotate-90">
            <circle
              cx={ringDims().cx}
              cy={ringDims().cy}
              r={ringDims().r}
              stroke="var(--color-void-700)"
              stroke-width={ringDims().strokeWidth}
              fill="none"
            />
            <circle
              cx={ringDims().cx}
              cy={ringDims().cy}
              r={ringDims().r}
              stroke={getStrokeColor(props.score)}
              stroke-width={ringDims().strokeWidth}
              fill="none"
              stroke-linecap="round"
              stroke-dasharray={`${CIRCUMFERENCE * ringDims().factor}`}
              stroke-dashoffset={offset() * ringDims().factor}
              class={`transition-all duration-700 ${showGlow() ? 'drop-shadow-[0_0_4px_currentColor]' : ''}`}
            />
          </svg>
          <span class={`absolute inset-0 flex items-center justify-center font-mono font-black tabular-nums ${healthConfig().text} ${size() === 'lg' ? 'text-sm' : 'text-2xs'}`}>
            {props.score}
          </span>
        </div>
        <div class={`flex items-center justify-center rounded-lg p-1 ${stageConfig().bg}`} title={stageConfig().label}>
          <StageIcon stage={props.stage} size={sizes().icon} class={stageConfig().color} />
        </div>
      </div>
    );
  }

  if (variant() === 'pill') {
    return (
      <div class={`inline-flex items-center ${sizes().gap} rounded-full border ${healthConfig().border} ${healthConfig().bg} ${healthConfig().glow} ${sizes().badge} transition-all duration-300`}>
        <span class={`font-mono font-black tabular-nums ${healthConfig().text}`}>
          {props.score}
        </span>
        <div class={`flex items-center gap-1 rounded-full px-1.5 py-0.5 ${stageConfig().bg}`}>
          <StageIcon stage={props.stage} size={sizes().icon - 2} class={stageConfig().color} />
          <span class={`text-2xs font-bold ${stageConfig().color}`}>
            {stageConfig().label}
          </span>
        </div>
      </div>
    );
  }

  return (
    <div class={`inline-flex items-center ${sizes().gap}`}>
      <div
        class={`rounded-full border ${healthConfig().border} ${healthConfig().bg} ${healthConfig().glow} ${sizes().badge} font-mono font-black tabular-nums ${healthConfig().text} transition-all duration-300`}
        title={`Health Score: ${props.score}/100 (${healthConfig().label})`}
      >
        {props.score}
      </div>
      <div 
        class={`flex items-center justify-center rounded-lg p-1 ${stageConfig().bg} transition-all duration-200 hover:scale-110`} 
        title={stageConfig().label}
      >
        <StageIcon stage={props.stage} size={sizes().icon} class={stageConfig().color} />
      </div>
    </div>
  );
};
