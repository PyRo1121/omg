export interface AdminOverview {
  overview?: {
    mrr: number;
    command_health?: {
      success: number;
      failure: number;
    };
  };
  daily_active_users?: Array<{
    active_users: number;
    date: string;
  }>;
  geo_distribution?: Array<{
    dimension: string;
    count: number;
  }>;
}

export interface AdminAdvancedMetrics {
  engagement?: {
    dau: number;
    wau: number;
    mau: number;
    stickiness?: {
      daily_to_monthly: string;
      weekly_to_monthly: string;
    };
  };
  retention?: {
    cohorts?: Array<{
      cohort_date: string;
      week_number: number;
      retained_users: number;
    }>;
  };
  ltv_by_tier?: Array<{
    tier: string;
    avg_ltv: number;
    user_count: number;
  }>;
  feature_adoption?: {
    install_adopters: number;
    search_adopters: number;
    runtime_adopters: number;
    total_active_users: number;
  };
  command_heatmap?: Array<{
    hour: number;
    command_count: number;
  }>;
  runtime_adoption?: Array<{
    runtime: string;
    unique_users: number;
    total_uses: number;
  }>;
  churn_risk_segments?: Array<{
    risk_segment: string;
    user_count: number;
    tier: string;
  }>;
  expansion_opportunities?: Array<{
    email: string;
    tier: string;
    opportunity_type: string;
    priority: string;
  }>;
  time_to_value?: {
    avg_days_to_activation: number;
    pct_activated_week1: number;
  };
  revenue_metrics?: {
    current_mrr: number;
    projected_arr: number;
    expansion_mrr_12m: number;
  };
}

export interface AdminUser {
  id: string;
  email: string;
  company?: string;
  tier?: string;
  status?: string;
  engagement_score?: number;
  lifecycle_stage?: string;
  created_at: string;
  last_active?: string;
  total_commands?: number;
  machine_count?: number;
}

export interface AdminFirehose {
  events: Array<{
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
  }>;
}

export interface AdminCRMResponse {
  users: AdminUser[];
  pagination: {
    total: number;
    pages: number;
    page: number;
    limit: number;
  };
}
