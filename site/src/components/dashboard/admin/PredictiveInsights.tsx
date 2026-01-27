import { Component, For, Show, createSignal, createMemo } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import {
  AlertTriangle,
  TrendingUp,
  TrendingDown,
  Zap,
  Target,
  Shield,
  Bell,
  ChevronRight,
  ArrowUpRight,
  Brain,
  Activity,
  Mail,
  MessageSquare,
  RefreshCw,
} from 'lucide-solid';
import { useAdminAdvancedMetrics, useAdminCRMUsers } from '../../../lib/api-hooks';
import { CardSkeleton } from '../../ui/Skeleton';
import { ProgressRing } from '../../../design-system/components/Charts';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

type PredictionType = 'churn' | 'expansion' | 'anomaly' | 'health';
type TimeHorizon = '30d' | '60d' | '90d';
type Priority = 'urgent' | 'high' | 'medium' | 'low';

interface ChurnPrediction {
  customerId: string;
  email: string;
  company: string | null;
  tier: string;
  probability: number;
  riskFactors: string[];
  lastActive: string;
  mrrAtRisk: number;
  recommendedAction: string;
}

interface ExpansionPrediction {
  customerId: string;
  email: string;
  company: string | null;
  tier: string;
  probability: number;
  signals: string[];
  potentialUpgrade: string;
  potentialArr: number;
  recommendedAction: string;
}

interface AnomalyAlert {
  id: string;
  type: 'usage_spike' | 'usage_drop' | 'error_surge' | 'unusual_pattern';
  severity: 'critical' | 'high' | 'medium' | 'low';
  description: string;
  affectedUsers: number;
  detectedAt: string;
  status: 'active' | 'acknowledged' | 'resolved';
}

interface HealthTrend {
  customerId: string;
  email: string;
  currentScore: number;
  predictedScore: number;
  trend: 'improving' | 'stable' | 'declining';
  trendStrength: number;
  actionRecommendation: string;
}

const PRIORITY_CONFIG: Record<Priority, { color: string; bg: string; label: string }> = {
  urgent: { color: 'text-flare-400', bg: 'bg-flare-500/10', label: 'Urgent' },
  high: { color: 'text-solar-400', bg: 'bg-solar-500/10', label: 'High' },
  medium: { color: 'text-indigo-400', bg: 'bg-indigo-500/10', label: 'Medium' },
  low: { color: 'text-nebula-400', bg: 'bg-nebula-500/10', label: 'Low' },
};

const PriorityBadge: Component<{ priority: Priority }> = (props) => {
  const config = () => PRIORITY_CONFIG[props.priority];
  return (
    <span
      class={cn(
        'rounded-full px-2 py-0.5 text-2xs font-black uppercase tracking-widest',
        config().bg,
        config().color
      )}
    >
      {config().label}
    </span>
  );
};

const ChurnPredictionCard: Component<{
  prediction: ChurnPrediction;
  onAction: (action: string) => void;
}> = (props) => {
  const riskLevel = createMemo((): Priority => {
    if (props.prediction.probability >= 0.7) return 'urgent';
    if (props.prediction.probability >= 0.5) return 'high';
    if (props.prediction.probability >= 0.3) return 'medium';
    return 'low';
  });

  return (
    <div class="group relative overflow-hidden rounded-2xl border border-white/5 bg-void-850 p-5 transition-all duration-300 hover:border-flare-500/30">
      <div
        class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full bg-flare-500/10 opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-100"
      />

      <div class="relative">
        <div class="mb-4 flex items-start justify-between">
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-flare-500/10">
              <AlertTriangle size={18} class="text-flare-400" />
            </div>
            <div>
              <h4 class="font-bold text-white">{props.prediction.email}</h4>
              <p class="text-2xs text-nebula-500">
                {props.prediction.company || props.prediction.tier}
              </p>
            </div>
          </div>
          <PriorityBadge priority={riskLevel()} />
        </div>

        <div class="mb-4 flex items-center gap-4">
          <ProgressRing
            value={props.prediction.probability * 100}
            size={60}
            strokeWidth={5}
            color="#ef4444"
            showValue
            label="Churn Risk"
          />
          <div class="flex-1">
            <p class="text-xs font-bold text-nebula-400">MRR at Risk</p>
            <p class="font-display text-xl font-black text-flare-400">
              ${props.prediction.mrrAtRisk.toLocaleString()}
            </p>
          </div>
        </div>

        <div class="mb-4 space-y-2">
          <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">Risk Factors</p>
          <div class="flex flex-wrap gap-1.5">
            <For each={props.prediction.riskFactors.slice(0, 3)}>
              {(factor) => (
                <span class="rounded-full bg-flare-500/10 px-2 py-0.5 text-2xs text-flare-400">
                  {factor}
                </span>
              )}
            </For>
          </div>
        </div>

        <div class="rounded-xl border border-aurora-500/20 bg-aurora-500/5 p-3">
          <div class="flex items-start gap-2">
            <Zap size={14} class="mt-0.5 shrink-0 text-aurora-400" />
            <div class="flex-1">
              <p class="text-2xs font-bold uppercase tracking-widest text-aurora-400">
                Recommended Action
              </p>
              <p class="mt-1 text-xs text-aurora-300">{props.prediction.recommendedAction}</p>
            </div>
          </div>
        </div>

        <div class="mt-4 flex gap-2">
          <button
            onClick={() => props.onAction('email')}
            class="flex flex-1 items-center justify-center gap-2 rounded-lg border border-white/10 bg-white/5 py-2 text-xs font-bold text-white transition-all hover:bg-white/10"
          >
            <Mail size={14} />
            Reach Out
          </button>
          <button
            onClick={() => props.onAction('call')}
            class="flex flex-1 items-center justify-center gap-2 rounded-lg border border-white/10 bg-white/5 py-2 text-xs font-bold text-white transition-all hover:bg-white/10"
          >
            <MessageSquare size={14} />
            Schedule Call
          </button>
        </div>
      </div>
    </div>
  );
};

const ExpansionOpportunityCard: Component<{
  prediction: ExpansionPrediction;
  onAction: (action: string) => void;
}> = (props) => {
  const opportunityLevel = createMemo((): Priority => {
    if (props.prediction.probability >= 0.7) return 'urgent';
    if (props.prediction.probability >= 0.5) return 'high';
    if (props.prediction.probability >= 0.3) return 'medium';
    return 'low';
  });

  return (
    <div class="group relative overflow-hidden rounded-2xl border border-white/5 bg-void-850 p-5 transition-all duration-300 hover:border-aurora-500/30">
      <div
        class="pointer-events-none absolute -right-8 -top-8 h-24 w-24 rounded-full bg-aurora-500/10 opacity-0 blur-[40px] transition-opacity duration-500 group-hover:opacity-100"
      />

      <div class="relative">
        <div class="mb-4 flex items-start justify-between">
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-aurora-500/10">
              <TrendingUp size={18} class="text-aurora-400" />
            </div>
            <div>
              <h4 class="font-bold text-white">{props.prediction.email}</h4>
              <p class="text-2xs text-nebula-500">
                {props.prediction.company || `${props.prediction.tier} → ${props.prediction.potentialUpgrade}`}
              </p>
            </div>
          </div>
          <PriorityBadge priority={opportunityLevel()} />
        </div>

        <div class="mb-4 flex items-center gap-4">
          <ProgressRing
            value={props.prediction.probability * 100}
            size={60}
            strokeWidth={5}
            color="#10b981"
            showValue
            label="Upgrade Likely"
          />
          <div class="flex-1">
            <p class="text-xs font-bold text-nebula-400">Potential ARR</p>
            <p class="font-display text-xl font-black text-aurora-400">
              +${props.prediction.potentialArr.toLocaleString()}
            </p>
          </div>
        </div>

        <div class="mb-4 space-y-2">
          <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">Expansion Signals</p>
          <div class="flex flex-wrap gap-1.5">
            <For each={props.prediction.signals.slice(0, 3)}>
              {(signal) => (
                <span class="rounded-full bg-aurora-500/10 px-2 py-0.5 text-2xs text-aurora-400">
                  {signal}
                </span>
              )}
            </For>
          </div>
        </div>

        <div class="rounded-xl border border-indigo-500/20 bg-indigo-500/5 p-3">
          <div class="flex items-start gap-2">
            <Target size={14} class="mt-0.5 shrink-0 text-indigo-400" />
            <div class="flex-1">
              <p class="text-2xs font-bold uppercase tracking-widest text-indigo-400">
                Recommended Action
              </p>
              <p class="mt-1 text-xs text-indigo-300">{props.prediction.recommendedAction}</p>
            </div>
          </div>
        </div>

        <div class="mt-4 flex gap-2">
          <button
            onClick={() => props.onAction('demo')}
            class="flex flex-1 items-center justify-center gap-2 rounded-lg bg-aurora-500 py-2 text-xs font-bold text-white transition-all hover:bg-aurora-600"
          >
            <ArrowUpRight size={14} />
            Start Upgrade
          </button>
        </div>
      </div>
    </div>
  );
};

const AnomalyAlertCard: Component<{
  alert: AnomalyAlert;
  onAcknowledge: () => void;
  onResolve: () => void;
}> = (props) => {
  const severityConfig: Record<string, { color: string; bg: string; icon: typeof AlertTriangle }> = {
    critical: { color: 'text-flare-400', bg: 'bg-flare-500/10', icon: AlertTriangle },
    high: { color: 'text-solar-400', bg: 'bg-solar-500/10', icon: Bell },
    medium: { color: 'text-indigo-400', bg: 'bg-indigo-500/10', icon: Activity },
    low: { color: 'text-nebula-400', bg: 'bg-nebula-500/10', icon: Activity },
  };

  const config = () => severityConfig[props.alert.severity] || severityConfig.medium;
  const IconComponent = config().icon;

  return (
    <div
      class={cn(
        'group relative overflow-hidden rounded-xl border p-4 transition-all duration-300',
        props.alert.status === 'active'
          ? 'border-flare-500/30 bg-flare-500/5'
          : 'border-white/5 bg-void-850'
      )}
    >
      <div class="flex items-start gap-3">
        <div class={cn('flex h-8 w-8 items-center justify-center rounded-lg', config().bg)}>
          <IconComponent size={16} class={config().color} />
        </div>

        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2">
            <span
              class={cn(
                'rounded-full px-2 py-0.5 text-2xs font-black uppercase tracking-widest',
                config().bg,
                config().color
              )}
            >
              {props.alert.severity}
            </span>
            <span class="text-2xs text-nebula-500">{props.alert.detectedAt}</span>
          </div>
          <p class="mt-1 text-sm font-bold text-white">{props.alert.description}</p>
          <p class="mt-0.5 text-2xs text-nebula-500">
            {props.alert.affectedUsers.toLocaleString()} users affected
          </p>
        </div>

        <Show when={props.alert.status === 'active'}>
          <div class="flex gap-1">
            <button
              onClick={props.onAcknowledge}
              class="rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-2xs font-bold text-white transition-all hover:bg-white/10"
            >
              Acknowledge
            </button>
            <button
              onClick={props.onResolve}
              class="rounded-lg bg-aurora-500 px-3 py-1.5 text-2xs font-bold text-white transition-all hover:bg-aurora-600"
            >
              Resolve
            </button>
          </div>
        </Show>
      </div>
    </div>
  );
};

const HealthTrendCard: Component<{ trend: HealthTrend }> = (props) => {
  const trendConfig = {
    improving: { color: 'text-aurora-400', bg: 'bg-aurora-500/10', icon: TrendingUp },
    stable: { color: 'text-indigo-400', bg: 'bg-indigo-500/10', icon: Activity },
    declining: { color: 'text-flare-400', bg: 'bg-flare-500/10', icon: TrendingDown },
  };

  const config = () => trendConfig[props.trend.trend];
  const IconComponent = config().icon;

  return (
    <div class="group relative overflow-hidden rounded-xl border border-white/5 bg-void-850 p-4 transition-all duration-300 hover:border-white/10">
      <div class="flex items-center gap-4">
        <ProgressRing
          value={props.trend.currentScore}
          size={48}
          strokeWidth={4}
          color={props.trend.trend === 'declining' ? '#ef4444' : props.trend.trend === 'improving' ? '#10b981' : '#6366f1'}
          showValue
        />

        <div class="flex-1 min-w-0">
          <p class="truncate text-sm font-bold text-white">{props.trend.email}</p>
          <div class="mt-1 flex items-center gap-2">
            <div class={cn('flex items-center gap-1 rounded-full px-2 py-0.5', config().bg)}>
              <IconComponent size={10} class={config().color} />
              <span class={cn('text-2xs font-bold', config().color)}>
                {props.trend.trendStrength > 0 ? '+' : ''}{props.trend.trendStrength}%
              </span>
            </div>
            <span class="text-2xs text-nebula-500">→ {props.trend.predictedScore}</span>
          </div>
        </div>

        <ChevronRight size={16} class="text-nebula-600 transition-colors group-hover:text-white" />
      </div>
    </div>
  );
};

export const PredictiveInsights: Component = () => {
  const [activeTab, setActiveTab] = createSignal<PredictionType>('churn');
  const [timeHorizon, setTimeHorizon] = createSignal<TimeHorizon>('30d');

  const metricsQuery = useAdminAdvancedMetrics();
  const usersQuery = useAdminCRMUsers(1, 100, '');

  const churnPredictions = createMemo((): ChurnPrediction[] => {
    if (!metricsQuery.data?.churn_risk_segments) return [];

    const expansionOps = metricsQuery.data.expansion_opportunities || [];

    return metricsQuery.data.churn_risk_segments
      .filter((s) => s.risk_segment === 'high' || s.risk_segment === 'critical')
      .map((segment, i) => {
        const opportunity = expansionOps[i];
        const probability = segment.risk_segment === 'critical' ? 0.8 : 0.55;
        const mrrPerUser = segment.tier === 'enterprise' ? 199 : segment.tier === 'team' ? 29 : 9;

        return {
          customerId: opportunity?.customer_id || `user-${i}`,
          email: opportunity?.email || `user${i}@example.com`,
          company: opportunity?.company || null,
          tier: segment.tier || 'pro',
          probability,
          riskFactors: [
            `${Math.round(segment.avg_monthly_commands)} commands/month`,
            `${segment.user_count} similar users`,
            segment.risk_segment === 'critical' ? 'No activity 14+ days' : 'Declining usage',
          ],
          lastActive: '5 days ago',
          mrrAtRisk: mrrPerUser * segment.user_count,
          recommendedAction:
            segment.risk_segment === 'critical'
              ? 'Immediate outreach with personalized value demo'
              : 'Schedule success check-in within 48 hours',
        };
      })
      .slice(0, 6);
  });

  const expansionPredictions = createMemo((): ExpansionPrediction[] => {
    if (!metricsQuery.data?.expansion_opportunities) return [];

    return metricsQuery.data.expansion_opportunities.map((opp) => {
      const probability =
        opp.priority === 'urgent' ? 0.85 : opp.priority === 'high' ? 0.65 : 0.4;
      const potentialUpgrade =
        opp.tier === 'pro' ? 'team' : opp.tier === 'team' ? 'enterprise' : 'enterprise';
      const potentialArr =
        potentialUpgrade === 'enterprise' ? 2388 : potentialUpgrade === 'team' ? 348 : 108;

      return {
        customerId: opp.customer_id,
        email: opp.email,
        company: opp.company,
        tier: opp.tier,
        probability,
        signals: [
          `${opp.active_machines}/${opp.max_seats} seats used`,
          `${Math.round(opp.hours_saved_30d)}h saved/month`,
          opp.opportunity_type.replace('_', ' '),
        ],
        potentialUpgrade,
        potentialArr,
        recommendedAction:
          opp.priority === 'urgent'
            ? 'Present upgrade path with team-based pricing'
            : 'Share ROI report and schedule demo of premium features',
      };
    });
  });

  const anomalyAlerts = createMemo((): AnomalyAlert[] => {
    const dau = metricsQuery.data?.engagement?.dau || 0;
    const mau = metricsQuery.data?.engagement?.mau || 1;
    const stickiness = (dau / mau) * 100;

    const alerts: AnomalyAlert[] = [];

    if (stickiness > 25) {
      alerts.push({
        id: 'a1',
        type: 'usage_spike',
        severity: 'medium',
        description: 'Unusual spike in daily active users detected',
        affectedUsers: Math.round(dau * 0.3),
        detectedAt: '2 hours ago',
        status: 'active',
      });
    }

    if (stickiness < 10) {
      alerts.push({
        id: 'a2',
        type: 'usage_drop',
        severity: 'high',
        description: 'Significant drop in daily engagement ratio',
        affectedUsers: Math.round(mau * 0.15),
        detectedAt: '6 hours ago',
        status: 'active',
      });
    }

    const errorRate = 100 - (metricsQuery.data?.time_to_value?.pct_activated_week1 || 0);
    if (errorRate > 50) {
      alerts.push({
        id: 'a3',
        type: 'error_surge',
        severity: 'critical',
        description: 'Elevated activation failure rate detected',
        affectedUsers: Math.round((metricsQuery.data?.engagement?.mau || 0) * 0.1),
        detectedAt: '1 hour ago',
        status: 'active',
      });
    }

    return alerts;
  });

  const healthTrends = createMemo((): HealthTrend[] => {
    if (!usersQuery.data?.users) return [];

    return usersQuery.data.users.slice(0, 8).map((user) => {
      const score = user.engagement_score || 50;
      const trend: 'improving' | 'stable' | 'declining' =
        score > 70 ? 'improving' : score > 40 ? 'stable' : 'declining';

      return {
        customerId: user.id,
        email: user.email,
        currentScore: score,
        predictedScore: trend === 'improving' ? Math.min(100, score + 8) : trend === 'declining' ? Math.max(0, score - 12) : score,
        trend,
        trendStrength: trend === 'improving' ? 8 : trend === 'declining' ? -12 : 0,
        actionRecommendation:
          trend === 'declining'
            ? 'Proactive outreach recommended'
            : trend === 'improving'
              ? 'Identify upsell opportunity'
              : 'Maintain regular check-ins',
      };
    });
  });

  const handleAction = (_action: string) => {};

  const tabs: { id: PredictionType; label: string; icon: typeof Brain; count: () => number }[] = [
    { id: 'churn', label: 'Churn Risk', icon: AlertTriangle, count: () => churnPredictions().length },
    { id: 'expansion', label: 'Expansion', icon: TrendingUp, count: () => expansionPredictions().length },
    { id: 'anomaly', label: 'Anomalies', icon: Bell, count: () => anomalyAlerts().length },
    { id: 'health', label: 'Health Trends', icon: Activity, count: () => healthTrends().length },
  ];

  return (
    <div class="space-y-8">
      <div class="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h2 class="flex items-center gap-3 text-2xl font-black tracking-tight text-white">
            <div class="rounded-xl bg-photon-500/10 p-2">
              <Brain size={24} class="text-photon-400" />
            </div>
            Predictive Insights
          </h2>
          <p class="mt-2 text-sm text-nebula-500">
            AI-powered predictions and actionable recommendations
          </p>
        </div>

        <div class="flex items-center gap-3">
          <select
            value={timeHorizon()}
            onChange={(e) => setTimeHorizon(e.currentTarget.value as TimeHorizon)}
            class="rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm font-bold text-white transition-all focus:outline-none"
          >
            <option value="30d">Next 30 Days</option>
            <option value="60d">Next 60 Days</option>
            <option value="90d">Next 90 Days</option>
          </select>

          <button
            onClick={() => {
              metricsQuery.refetch();
              usersQuery.refetch();
            }}
            disabled={metricsQuery.isRefetching || usersQuery.isRefetching}
            class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm font-bold text-white transition-all hover:bg-white/10"
          >
            <RefreshCw
              size={16}
              class={metricsQuery.isRefetching || usersQuery.isRefetching ? 'animate-spin' : ''}
            />
            Refresh
          </button>
        </div>
      </div>

      <div class="flex items-center gap-2 rounded-2xl border border-white/5 bg-white/[0.02] p-1.5">
        <For each={tabs}>
          {(tab) => (
            <button
              onClick={() => setActiveTab(tab.id)}
              class={cn(
                'flex items-center gap-2 rounded-xl px-4 py-2.5 text-sm font-bold transition-all',
                activeTab() === tab.id
                  ? 'bg-white text-black shadow-lg'
                  : 'text-nebula-400 hover:bg-white/5 hover:text-white'
              )}
            >
              <tab.icon size={16} />
              {tab.label}
              <Show when={tab.count() > 0}>
                <span
                  class={cn(
                    'rounded-full px-2 py-0.5 text-2xs font-black',
                    activeTab() === tab.id ? 'bg-black/10 text-black' : 'bg-white/10 text-white'
                  )}
                >
                  {tab.count()}
                </span>
              </Show>
            </button>
          )}
        </For>
      </div>

      <Show when={metricsQuery.isLoading || usersQuery.isLoading}>
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={metricsQuery.isSuccess && usersQuery.isSuccess}>
        <Show when={activeTab() === 'churn'}>
          <div class="space-y-4">
            <div class="flex items-center justify-between">
              <p class="text-sm text-nebula-400">
                <span class="font-bold text-flare-400">{churnPredictions().length}</span> customers predicted to churn within {timeHorizon()}
              </p>
              <p class="font-mono text-sm font-bold text-flare-400">
                ${churnPredictions().reduce((sum, p) => sum + p.mrrAtRisk, 0).toLocaleString()} MRR at risk
              </p>
            </div>

            <Show
              when={churnPredictions().length > 0}
              fallback={
                <div class="rounded-2xl border border-aurora-500/20 bg-aurora-500/5 p-8 text-center">
                  <Shield size={32} class="mx-auto mb-3 text-aurora-400" />
                  <p class="font-bold text-aurora-400">No High-Risk Churn Detected</p>
                  <p class="mt-2 text-sm text-nebula-500">
                    All customers appear healthy within the selected timeframe
                  </p>
                </div>
              }
            >
              <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
                <For each={churnPredictions()}>
                  {(prediction) => (
                    <ChurnPredictionCard prediction={prediction} onAction={handleAction} />
                  )}
                </For>
              </div>
            </Show>
          </div>
        </Show>

        <Show when={activeTab() === 'expansion'}>
          <div class="space-y-4">
            <div class="flex items-center justify-between">
              <p class="text-sm text-nebula-400">
                <span class="font-bold text-aurora-400">{expansionPredictions().length}</span> expansion opportunities identified
              </p>
              <p class="font-mono text-sm font-bold text-aurora-400">
                +${expansionPredictions().reduce((sum, p) => sum + p.potentialArr, 0).toLocaleString()} potential ARR
              </p>
            </div>

            <Show
              when={expansionPredictions().length > 0}
              fallback={
                <div class="rounded-2xl border border-nebula-500/20 bg-nebula-500/5 p-8 text-center">
                  <Target size={32} class="mx-auto mb-3 text-nebula-400" />
                  <p class="font-bold text-nebula-400">No Expansion Opportunities</p>
                  <p class="mt-2 text-sm text-nebula-500">
                    No customers are currently showing expansion signals
                  </p>
                </div>
              }
            >
              <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
                <For each={expansionPredictions()}>
                  {(prediction) => (
                    <ExpansionOpportunityCard prediction={prediction} onAction={handleAction} />
                  )}
                </For>
              </div>
            </Show>
          </div>
        </Show>

        <Show when={activeTab() === 'anomaly'}>
          <div class="space-y-4">
            <Show
              when={anomalyAlerts().length > 0}
              fallback={
                <div class="rounded-2xl border border-aurora-500/20 bg-aurora-500/5 p-8 text-center">
                  <Shield size={32} class="mx-auto mb-3 text-aurora-400" />
                  <p class="font-bold text-aurora-400">All Systems Normal</p>
                  <p class="mt-2 text-sm text-nebula-500">No anomalies detected in the current monitoring period</p>
                </div>
              }
            >
              <div class="space-y-3">
                <For each={anomalyAlerts()}>
                  {(alert) => (
                    <AnomalyAlertCard
                      alert={alert}
                      onAcknowledge={() => {}}
                      onResolve={() => {}}
                    />
                  )}
                </For>
              </div>
            </Show>
          </div>
        </Show>

        <Show when={activeTab() === 'health'}>
          <div class="space-y-4">
            <p class="text-sm text-nebula-400">
              Predicted health score changes for the next {timeHorizon()}
            </p>

            <div class="grid gap-3 md:grid-cols-2 lg:grid-cols-4">
              <For each={healthTrends()}>
                {(trend) => <HealthTrendCard trend={trend} />}
              </For>
            </div>
          </div>
        </Show>
      </Show>

      <div class="rounded-3xl border border-white/5 bg-gradient-to-br from-photon-500/5 to-indigo-500/5 p-6">
        <h3 class="mb-4 flex items-center gap-2 text-lg font-black text-white">
          <Brain size={20} class="text-photon-400" />
          AI-Powered Summary
        </h3>
        <div class="grid gap-4 md:grid-cols-3">
          <div class="rounded-xl border border-white/5 bg-void-850/50 p-4">
            <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              Revenue at Risk
            </p>
            <p class="mt-1 font-display text-3xl font-black text-flare-400">
              ${churnPredictions().reduce((sum, p) => sum + p.mrrAtRisk, 0).toLocaleString()}
            </p>
            <p class="mt-1 text-xs text-nebula-500">
              {churnPredictions().length} customers need attention
            </p>
          </div>
          <div class="rounded-xl border border-white/5 bg-void-850/50 p-4">
            <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              Expansion Potential
            </p>
            <p class="mt-1 font-display text-3xl font-black text-aurora-400">
              +${expansionPredictions().reduce((sum, p) => sum + p.potentialArr, 0).toLocaleString()}
            </p>
            <p class="mt-1 text-xs text-nebula-500">
              {expansionPredictions().length} upgrade opportunities
            </p>
          </div>
          <div class="rounded-xl border border-white/5 bg-void-850/50 p-4">
            <p class="text-2xs font-bold uppercase tracking-widest text-nebula-500">
              Active Alerts
            </p>
            <p class="mt-1 font-display text-3xl font-black text-solar-400">
              {anomalyAlerts().filter((a) => a.status === 'active').length}
            </p>
            <p class="mt-1 text-xs text-nebula-500">
              {anomalyAlerts().filter((a) => a.severity === 'critical').length} critical
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

export default PredictiveInsights;
