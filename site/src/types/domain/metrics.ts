export interface ExecutiveKPI {
  mrr: number;
  mrr_change: number;
  arr: number;
  dau: number;
  wau: number;
  mau: number;
  stickiness: number;
  churn_rate: number;
  at_risk_count: number;
  expansion_pipeline: number;
}

export interface AdvancedMetrics {
  engagement: {
    dau: number;
    wau: number;
    mau: number;
    stickiness: {
      daily_to_monthly: string;
      daily_to_weekly: string;
    };
  };
  retention: {
    cohorts: Array<{
      cohort_date: string;
      week_number: number;
      retained_users: number;
      retention_rate: number;
    }>;
  };
  ltv_by_tier: Array<{
    tier: string;
    avg_ltv: number;
    user_count: number;
  }>;
  feature_adoption: {
    install_adopters: number;
    search_adopters: number;
    runtime_adopters: number;
    total_users: number;
  };
  command_heatmap: Array<{
    hour: number;
    command_count: number;
  }>;
  runtime_adoption: Array<{
    runtime: string;
    unique_users: number;
    total_uses: number;
    growth_rate: number;
  }>;
  churn_risk_segments: Array<{
    risk_segment: 'low' | 'medium' | 'high' | 'critical';
    user_count: number;
    tier: string;
    avg_days_inactive: number;
  }>;
  expansion_opportunities: Array<{
    email: string;
    tier: string;
    opportunity_type: 'usage_based' | 'feature_gate' | 'team_growth' | 'enterprise';
    priority: 'low' | 'medium' | 'high' | 'urgent';
    potential_arr: number;
  }>;
  time_to_value: {
    avg_days_to_activation: number;
    pct_activated_week1: number;
    pct_activated_month1: number;
  };
  revenue_metrics: {
    current_mrr: number;
    projected_arr: number;
    expansion_mrr_12m: number;
    net_revenue_retention: number;
  };
}

export interface FirehoseEvent {
  id: string;
  event_type: 'command' | 'install' | 'search' | 'runtime_switch' | 'error';
  event_name: string;
  machine_id: string;
  hostname: string;
  platform: string;
  timestamp: string;
  duration_ms: number;
  success: boolean;
}

export interface GeoDistribution {
  country: string;
  country_code: string;
  count: number;
  percentage: number;
}

export interface CommandHealth {
  success: number;
  failure: number;
}
