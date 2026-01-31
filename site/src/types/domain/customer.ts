export interface CustomerHealth {
  overall_score: number;
  engagement_score: number;
  activation_score: number;
  growth_score: number;
  risk_score: number;
  lifecycle_stage: 'trial' | 'active' | 'power_user' | 'at_risk' | 'churned';
  predicted_churn_probability: number;
  predicted_upgrade_probability: number;
  expansion_readiness_score: number;
  command_velocity_7d: number;
  command_velocity_trend: 'growing' | 'stable' | 'declining';
}

export interface CRMCustomer {
  id: string;
  email: string;
  company?: string;
  tier: string;
  status: 'active' | 'suspended' | 'cancelled';
  health: CustomerHealth;
  tags: string[];
  created_at: string;
  last_activity_at: string;
  total_commands: number;
  machine_count: number;
  mrr: number;
}
