import type {
  AdminOverview,
  AdminAdvancedMetrics,
  AdminUser,
  ExecutiveKPI,
  AdvancedMetrics,
  FirehoseEvent,
  GeoDistribution,
  CRMCustomer,
  CustomerHealth,
} from '~/types';

export function transformToExecutiveKPI(
  dashboard: AdminOverview | undefined,
  metrics: AdminAdvancedMetrics | undefined
): ExecutiveKPI {
  const mrr = dashboard?.overview?.mrr || 0;
  const mau = metrics?.engagement?.mau || 0;
  const highRiskCount = metrics?.churn_risk_segments?.reduce(
    (acc, s) => s.risk_segment === 'high' || s.risk_segment === 'critical' ? acc + s.user_count : acc, 
    0
  ) || 0;
  
  return {
    mrr,
    mrr_change: 0, // TODO: Calculate from historical data
    arr: mrr * 12,
    dau: metrics?.engagement?.dau || dashboard?.daily_active_users?.[0]?.active_users || 0,
    wau: metrics?.engagement?.wau || 0,
    mau,
    stickiness: parseFloat(metrics?.engagement?.stickiness?.daily_to_monthly?.replace('%', '') || '0'),
    churn_rate: mau > 0 ? (highRiskCount / mau) * 100 : 0,
    at_risk_count: highRiskCount,
    expansion_pipeline: metrics?.revenue_metrics?.expansion_mrr_12m || 0,
  };
}

export function transformToAdvancedMetrics(metrics: AdminAdvancedMetrics | undefined): AdvancedMetrics | undefined {
  if (!metrics) return undefined;
  return {
    engagement: {
      dau: metrics.engagement?.dau || 0,
      wau: metrics.engagement?.wau || 0,
      mau: metrics.engagement?.mau || 0,
      stickiness: {
        daily_to_monthly: metrics.engagement?.stickiness?.daily_to_monthly || '0%',
        daily_to_weekly: metrics.engagement?.stickiness?.weekly_to_monthly || '0%',
      },
    },
    retention: {
      cohorts: metrics.retention?.cohorts?.map(c => ({
        cohort_date: c.cohort_date,
        week_number: c.week_number,
        retained_users: c.retained_users,
        retention_rate: c.retention_rate || 0,
      })) || [],
    },
    ltv_by_tier: metrics.ltv_by_tier || [],
    feature_adoption: {
      install_adopters: metrics.feature_adoption?.install_adopters || 0,
      search_adopters: metrics.feature_adoption?.search_adopters || 0,
      runtime_adopters: metrics.feature_adoption?.runtime_adopters || 0,
      total_users: metrics.feature_adoption?.total_active_users || 0,
    },
    command_heatmap: metrics.command_heatmap || [],
    runtime_adoption: metrics.runtime_adoption?.map(r => ({
      runtime: r.runtime,
      unique_users: r.unique_users,
      total_uses: r.total_uses,
      growth_rate: r.growth_rate || 0,
    })) || [],
    churn_risk_segments: metrics.churn_risk_segments?.map(s => ({
      risk_segment: s.risk_segment as 'low' | 'medium' | 'high' | 'critical',
      user_count: s.user_count,
      tier: s.tier,
      avg_days_inactive: s.avg_days_inactive || 0,
    })) || [],
    expansion_opportunities: metrics.expansion_opportunities?.map(o => ({
      email: o.email,
      tier: o.tier,
      opportunity_type: o.opportunity_type as 'usage_based' | 'feature_gate' | 'team_growth' | 'enterprise',
      priority: o.priority as 'low' | 'medium' | 'high' | 'urgent',
      potential_arr: o.potential_arr || 0,
    })) || [],
    time_to_value: {
      avg_days_to_activation: metrics.time_to_value?.avg_days_to_activation || 0,
      pct_activated_week1: metrics.time_to_value?.pct_activated_week1 || 0,
      pct_activated_month1: metrics.time_to_value?.pct_activated_month1 || 0,
    },
    revenue_metrics: {
      current_mrr: metrics.revenue_metrics?.current_mrr || 0,
      projected_arr: metrics.revenue_metrics?.projected_arr || 0,
      expansion_mrr_12m: metrics.revenue_metrics?.expansion_mrr_12m || 0,
      net_revenue_retention: metrics.revenue_metrics?.net_revenue_retention || 0,
    },
  };
}

interface RawFirehoseEvent {
  id?: string;
  event_name?: string;
  action?: string;
  machine_id?: string;
  hostname?: string;
  platform?: string;
  timestamp?: string;
  created_at?: string;
  duration_ms?: number;
  success?: boolean;
  metadata?: {
    hostname?: string;
    platform?: string;
  };
}

export function transformFirehoseEvents(events: RawFirehoseEvent[]): FirehoseEvent[] {
  return events.map((e, i) => ({
    id: e.id || `evt-${i}`,
    event_type: mapEventType(e.event_name || e.action || ''),
    event_name: e.event_name || e.action || 'unknown',
    machine_id: e.machine_id || '',
    hostname: e.hostname || e.metadata?.hostname || '',
    platform: e.platform || e.metadata?.platform || 'unknown',
    timestamp: e.timestamp || e.created_at || new Date().toISOString(),
    duration_ms: e.duration_ms || 0,
    success: e.success !== false,
  }));
}

function mapEventType(eventName: string): FirehoseEvent['event_type'] {
  const lower = eventName.toLowerCase();
  if (lower.includes('install')) return 'install';
  if (lower.includes('search')) return 'search';
  if (lower.includes('runtime') || lower.includes('use ')) return 'runtime_switch';
  if (lower.includes('error') || lower.includes('fail')) return 'error';
  return 'command';
}

export function transformGeoDistribution(data: { dimension: string; count: number }[]): GeoDistribution[] {
  const total = data.reduce((sum, d) => sum + d.count, 0) || 1;
  return data.map(d => ({
    country: getCountryName(d.dimension),
    country_code: d.dimension || 'XX',
    count: d.count,
    percentage: (d.count / total) * 100,
  }));
}

function getCountryName(code: string): string {
  const countries: Record<string, string> = {
    US: 'United States', DE: 'Germany', GB: 'United Kingdom', FR: 'France',
    CA: 'Canada', JP: 'Japan', AU: 'Australia', BR: 'Brazil', IN: 'India',
    NL: 'Netherlands', SE: 'Sweden', ES: 'Spain', IT: 'Italy', KR: 'South Korea',
  };
  return countries[code] || code || 'Unknown';
}

export function transformToCRMCustomer(user: AdminUser): CRMCustomer {
  const score = user.engagement_score || 50;
  const stage = (user.lifecycle_stage || 'active') as CustomerHealth['lifecycle_stage'];
  
  return {
    id: user.id,
    email: user.email,
    company: user.company || undefined,
    tier: user.tier || 'free',
    status: (user.status as 'active' | 'suspended' | 'cancelled') || 'active',
    health: {
      overall_score: score,
      engagement_score: Math.min(100, score + 10),
      activation_score: Math.min(100, score + 5),
      growth_score: Math.max(0, score - 10),
      risk_score: Math.max(0, 100 - score),
      lifecycle_stage: stage,
      predicted_churn_probability: stage === 'at_risk' ? 0.6 : stage === 'churned' ? 0.9 : 0.1,
      predicted_upgrade_probability: score > 70 ? 0.7 : 0.3,
      expansion_readiness_score: score,
      command_velocity_7d: user.total_commands || 0,
      command_velocity_trend: score > 60 ? 'growing' : score > 40 ? 'stable' : 'declining',
    },
    tags: [],
    created_at: user.created_at,
    last_activity_at: user.last_active || user.created_at,
    total_commands: user.total_commands || 0,
    machine_count: user.machine_count || 0,
    mrr: user.tier === 'enterprise' ? 199 : user.tier === 'team' ? 29 : user.tier === 'pro' ? 9 : 0,
  };
}
