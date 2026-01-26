-- ============================================================================
-- OMG CRM ENHANCEMENTS MIGRATION
-- Run: bunx wrangler d1 execute omg-licensing --remote --file=./migrations/010_crm_enhancements.sql
-- ============================================================================

-- ============================================================================
-- CUSTOMER HEALTH & SCORING
-- Pre-computed health metrics for fast dashboard queries
-- ============================================================================

CREATE TABLE IF NOT EXISTS customer_health (
  id TEXT PRIMARY KEY,
  customer_id TEXT UNIQUE NOT NULL,
  
  overall_score INTEGER DEFAULT 50,
  engagement_score INTEGER DEFAULT 50,
  activation_score INTEGER DEFAULT 50,
  growth_score INTEGER DEFAULT 50,
  risk_score INTEGER DEFAULT 0,
  
  lifecycle_stage TEXT DEFAULT 'new' CHECK (lifecycle_stage IN (
    'new', 'onboarding', 'activated', 'engaged', 'power_user', 
    'at_risk', 'churning', 'churned', 'reactivated'
  )),
  
  predicted_churn_probability REAL DEFAULT 0.0,
  predicted_upgrade_probability REAL DEFAULT 0.0,
  expansion_readiness_score INTEGER DEFAULT 0,
  
  first_value_date DATETIME,
  activation_date DATETIME,
  power_user_date DATETIME,
  last_healthy_activity DATETIME,
  churn_risk_flagged_at DATETIME,
  
  days_to_activation INTEGER,
  days_since_last_activity INTEGER,
  command_velocity_7d REAL DEFAULT 0.0,
  command_velocity_trend TEXT DEFAULT 'stable',
  
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_customer_health_score ON customer_health(overall_score);
CREATE INDEX IF NOT EXISTS idx_customer_health_lifecycle ON customer_health(lifecycle_stage);
CREATE INDEX IF NOT EXISTS idx_customer_health_churn ON customer_health(predicted_churn_probability);
CREATE INDEX IF NOT EXISTS idx_customer_health_customer ON customer_health(customer_id);

-- ============================================================================
-- CRM NOTES
-- Internal notes attached to customers
-- ============================================================================

CREATE TABLE IF NOT EXISTS customer_notes (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  author_id TEXT NOT NULL,
  note_type TEXT DEFAULT 'general' CHECK (note_type IN (
    'general', 'call', 'email', 'meeting', 'support', 'sales', 'success'
  )),
  content TEXT NOT NULL,
  is_pinned INTEGER DEFAULT 0,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE,
  FOREIGN KEY (author_id) REFERENCES customers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_notes_customer ON customer_notes(customer_id);
CREATE INDEX IF NOT EXISTS idx_notes_author ON customer_notes(author_id);
CREATE INDEX IF NOT EXISTS idx_notes_created ON customer_notes(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notes_pinned ON customer_notes(customer_id, is_pinned DESC);

-- ============================================================================
-- CRM TAGS
-- Tag system for customer segmentation
-- ============================================================================

CREATE TABLE IF NOT EXISTS customer_tags (
  id TEXT PRIMARY KEY,
  name TEXT UNIQUE NOT NULL,
  color TEXT DEFAULT '#6366f1',
  description TEXT,
  created_by TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_tags_name ON customer_tags(name);

CREATE TABLE IF NOT EXISTS customer_tag_assignments (
  customer_id TEXT NOT NULL,
  tag_id TEXT NOT NULL,
  assigned_by TEXT,
  assigned_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (customer_id, tag_id),
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE,
  FOREIGN KEY (tag_id) REFERENCES customer_tags(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tag_assignments_customer ON customer_tag_assignments(customer_id);
CREATE INDEX IF NOT EXISTS idx_tag_assignments_tag ON customer_tag_assignments(tag_id);

-- ============================================================================
-- CRM TASKS
-- Follow-up tasks for customer success
-- ============================================================================

CREATE TABLE IF NOT EXISTS crm_tasks (
  id TEXT PRIMARY KEY,
  customer_id TEXT,
  assigned_to TEXT,
  title TEXT NOT NULL,
  description TEXT,
  task_type TEXT DEFAULT 'followup' CHECK (task_type IN (
    'followup', 'onboarding', 'renewal', 'upsell', 'support', 'custom'
  )),
  priority TEXT DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
  status TEXT DEFAULT 'pending' CHECK (status IN ('pending', 'in_progress', 'completed', 'cancelled')),
  due_date DATETIME,
  completed_at DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_crm_tasks_customer ON crm_tasks(customer_id);
CREATE INDEX IF NOT EXISTS idx_crm_tasks_assigned ON crm_tasks(assigned_to);
CREATE INDEX IF NOT EXISTS idx_crm_tasks_due ON crm_tasks(due_date);
CREATE INDEX IF NOT EXISTS idx_crm_tasks_status ON crm_tasks(status);
CREATE INDEX IF NOT EXISTS idx_crm_tasks_priority ON crm_tasks(priority, status);

-- ============================================================================
-- COMMUNICATION HISTORY
-- Track customer interactions across channels
-- ============================================================================

CREATE TABLE IF NOT EXISTS customer_communications (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  channel TEXT NOT NULL CHECK (channel IN ('email', 'chat', 'phone', 'meeting', 'support_ticket')),
  direction TEXT NOT NULL CHECK (direction IN ('inbound', 'outbound')),
  subject TEXT,
  summary TEXT,
  external_id TEXT,
  metadata TEXT,
  occurred_at DATETIME NOT NULL,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  recorded_by TEXT,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_comms_customer ON customer_communications(customer_id);
CREATE INDEX IF NOT EXISTS idx_comms_occurred ON customer_communications(occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_comms_channel ON customer_communications(channel);

-- ============================================================================
-- SUBSCRIPTION CHANGES
-- Track tier upgrades, downgrades, cancellations
-- ============================================================================

CREATE TABLE IF NOT EXISTS subscription_changes (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  subscription_id TEXT,
  change_type TEXT NOT NULL CHECK (change_type IN ('upgrade', 'downgrade', 'cancel', 'reactivate', 'renewal')),
  old_tier TEXT,
  new_tier TEXT,
  old_price_cents INTEGER,
  new_price_cents INTEGER,
  reason TEXT,
  changed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_sub_changes_customer ON subscription_changes(customer_id);
CREATE INDEX IF NOT EXISTS idx_sub_changes_type ON subscription_changes(change_type);
CREATE INDEX IF NOT EXISTS idx_sub_changes_date ON subscription_changes(changed_at);

-- ============================================================================
-- STRIPE EVENTS ARCHIVE
-- Store all Stripe webhook events for replay/audit
-- ============================================================================

CREATE TABLE IF NOT EXISTS stripe_events (
  id TEXT PRIMARY KEY,
  stripe_event_id TEXT UNIQUE NOT NULL,
  event_type TEXT NOT NULL,
  customer_id TEXT,
  stripe_customer_id TEXT,
  event_data TEXT,
  processed INTEGER DEFAULT 0,
  processed_at DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_stripe_events_type ON stripe_events(event_type);
CREATE INDEX IF NOT EXISTS idx_stripe_events_customer ON stripe_events(customer_id);
CREATE INDEX IF NOT EXISTS idx_stripe_events_processed ON stripe_events(processed);
CREATE INDEX IF NOT EXISTS idx_stripe_events_stripe_id ON stripe_events(stripe_event_id);

-- ============================================================================
-- CUSTOMER REVENUE METRICS
-- Aggregated revenue data for fast queries
-- ============================================================================

CREATE TABLE IF NOT EXISTS customer_revenue (
  id TEXT PRIMARY KEY,
  customer_id TEXT UNIQUE NOT NULL,
  
  ltv_total_cents INTEGER DEFAULT 0,
  ltv_currency TEXT DEFAULT 'usd',
  total_payments INTEGER DEFAULT 0,
  total_refunds_cents INTEGER DEFAULT 0,
  first_payment_at DATETIME,
  last_payment_at DATETIME,
  
  current_mrr_cents INTEGER DEFAULT 0,
  current_plan_price_id TEXT,
  
  expansion_mrr_cents INTEGER DEFAULT 0,
  contraction_mrr_cents INTEGER DEFAULT 0,
  
  failed_payments_30d INTEGER DEFAULT 0,
  dunning_attempts INTEGER DEFAULT 0,
  last_failed_payment_at DATETIME,
  
  seats_utilization_pct REAL DEFAULT 0.0,
  feature_limit_hits_30d INTEGER DEFAULT 0,
  
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_customer_revenue_mrr ON customer_revenue(current_mrr_cents);
CREATE INDEX IF NOT EXISTS idx_customer_revenue_ltv ON customer_revenue(ltv_total_cents);
CREATE INDEX IF NOT EXISTS idx_customer_revenue_customer ON customer_revenue(customer_id);

-- ============================================================================
-- ADMIN ALERTS
-- Automated alerts for admins
-- ============================================================================

CREATE TABLE IF NOT EXISTS admin_alerts (
  id TEXT PRIMARY KEY,
  alert_type TEXT NOT NULL CHECK (alert_type IN (
    'churn_risk', 'expansion_opportunity', 'failed_payment', 
    'high_value_signup', 'usage_spike', 'usage_drop', 'github_integration',
    'security', 'system'
  )),
  severity TEXT DEFAULT 'info' CHECK (severity IN ('info', 'warning', 'critical')),
  customer_id TEXT,
  title TEXT NOT NULL,
  description TEXT,
  action_url TEXT,
  is_read INTEGER DEFAULT 0,
  is_resolved INTEGER DEFAULT 0,
  resolved_at DATETIME,
  resolved_by TEXT,
  metadata TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_admin_alerts_type ON admin_alerts(alert_type);
CREATE INDEX IF NOT EXISTS idx_admin_alerts_unread ON admin_alerts(is_read) WHERE is_read = 0;
CREATE INDEX IF NOT EXISTS idx_admin_alerts_unresolved ON admin_alerts(is_resolved) WHERE is_resolved = 0;
CREATE INDEX IF NOT EXISTS idx_admin_alerts_customer ON admin_alerts(customer_id);
CREATE INDEX IF NOT EXISTS idx_admin_alerts_severity ON admin_alerts(severity);

-- ============================================================================
-- GITHUB INTEGRATION
-- ============================================================================

CREATE TABLE IF NOT EXISTS github_connections (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL UNIQUE,
  github_user_id INTEGER NOT NULL,
  github_username TEXT NOT NULL,
  github_access_token TEXT,
  github_avatar_url TEXT,
  scopes TEXT,
  connected_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  last_synced_at DATETIME,
  sync_status TEXT DEFAULT 'active' CHECK (sync_status IN ('active', 'expired', 'revoked')),
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_github_customer ON github_connections(customer_id);
CREATE INDEX IF NOT EXISTS idx_github_username ON github_connections(github_username);
CREATE INDEX IF NOT EXISTS idx_github_status ON github_connections(sync_status);

CREATE TABLE IF NOT EXISTS github_repos (
  id TEXT PRIMARY KEY,
  github_connection_id TEXT NOT NULL,
  github_repo_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  full_name TEXT NOT NULL,
  is_private INTEGER DEFAULT 0,
  primary_language TEXT,
  stars INTEGER DEFAULT 0,
  forks INTEGER DEFAULT 0,
  open_issues INTEGER DEFAULT 0,
  last_push_at DATETIME,
  created_at DATETIME,
  omg_detected INTEGER DEFAULT 0,
  omg_config_path TEXT,
  synced_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (github_connection_id) REFERENCES github_connections(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_repos_connection ON github_repos(github_connection_id);
CREATE INDEX IF NOT EXISTS idx_repos_language ON github_repos(primary_language);
CREATE INDEX IF NOT EXISTS idx_repos_omg ON github_repos(omg_detected);

CREATE TABLE IF NOT EXISTS github_activity (
  id TEXT PRIMARY KEY,
  github_connection_id TEXT NOT NULL,
  customer_id TEXT NOT NULL,
  activity_type TEXT NOT NULL CHECK (activity_type IN (
    'push', 'pull_request', 'issue', 'release', 'workflow_run', 
    'star', 'fork', 'commit'
  )),
  repo_full_name TEXT,
  activity_data TEXT,
  activity_timestamp DATETIME,
  omg_related INTEGER DEFAULT 0,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (github_connection_id) REFERENCES github_connections(id) ON DELETE CASCADE,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_github_activity_connection ON github_activity(github_connection_id);
CREATE INDEX IF NOT EXISTS idx_github_activity_customer ON github_activity(customer_id);
CREATE INDEX IF NOT EXISTS idx_github_activity_type ON github_activity(activity_type);
CREATE INDEX IF NOT EXISTS idx_github_activity_timestamp ON github_activity(activity_timestamp);
CREATE INDEX IF NOT EXISTS idx_github_activity_omg ON github_activity(omg_related);

-- ============================================================================
-- CUSTOMER SEGMENTS
-- Dynamic segmentation for targeting
-- ============================================================================

CREATE TABLE IF NOT EXISTS customer_segments (
  id TEXT PRIMARY KEY,
  name TEXT UNIQUE NOT NULL,
  description TEXT,
  filter_rules TEXT NOT NULL,
  is_dynamic INTEGER DEFAULT 1,
  member_count INTEGER DEFAULT 0,
  created_by TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_segments_name ON customer_segments(name);

CREATE TABLE IF NOT EXISTS segment_memberships (
  segment_id TEXT NOT NULL,
  customer_id TEXT NOT NULL,
  added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (segment_id, customer_id),
  FOREIGN KEY (segment_id) REFERENCES customer_segments(id) ON DELETE CASCADE,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_segment_members_segment ON segment_memberships(segment_id);
CREATE INDEX IF NOT EXISTS idx_segment_members_customer ON segment_memberships(customer_id);

-- ============================================================================
-- SCHEDULED REPORTS
-- ============================================================================

CREATE TABLE IF NOT EXISTS scheduled_reports (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  report_type TEXT NOT NULL,
  schedule TEXT NOT NULL,
  recipients TEXT NOT NULL,
  filters TEXT,
  format TEXT DEFAULT 'csv',
  is_active INTEGER DEFAULT 1,
  last_run_at DATETIME,
  next_run_at DATETIME,
  created_by TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_scheduled_reports_active ON scheduled_reports(is_active, next_run_at);

-- ============================================================================
-- WEBHOOK DEAD LETTER QUEUE
-- Failed webhooks for retry
-- ============================================================================

CREATE TABLE IF NOT EXISTS webhook_dlq (
  id TEXT PRIMARY KEY,
  webhook_id TEXT NOT NULL,
  webhook_type TEXT NOT NULL,
  error_message TEXT,
  raw_payload TEXT,
  retry_count INTEGER DEFAULT 0,
  last_retry_at DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_webhook_dlq_type ON webhook_dlq(webhook_type);
CREATE INDEX IF NOT EXISTS idx_webhook_dlq_created ON webhook_dlq(created_at);

-- ============================================================================
-- INSERT DEFAULT TAGS
-- ============================================================================

INSERT OR IGNORE INTO customer_tags (id, name, color, description) VALUES 
  ('tag_vip', 'VIP', '#f59e0b', 'High-value customers requiring white-glove service'),
  ('tag_at_risk', 'At Risk', '#ef4444', 'Customers showing signs of churn'),
  ('tag_expansion', 'Expansion Ready', '#10b981', 'Customers likely to upgrade'),
  ('tag_enterprise', 'Enterprise Prospect', '#8b5cf6', 'Potential enterprise conversion'),
  ('tag_advocate', 'Advocate', '#06b6d4', 'Active community advocates'),
  ('tag_beta_tester', 'Beta Tester', '#ec4899', 'Participants in beta programs');

-- ============================================================================
-- INSERT DEFAULT SEGMENTS
-- ============================================================================

INSERT OR IGNORE INTO customer_segments (id, name, description, filter_rules) VALUES 
  ('seg_power_users', 'Power Users', 'Users with high engagement scores', '{"lifecycle_stage": "power_user"}'),
  ('seg_at_risk', 'Churn Risk', 'Users with elevated churn probability', '{"churn_probability_gt": 0.6}'),
  ('seg_new_paid', 'New Paid', 'Recently converted paid customers', '{"tier_not": "free", "days_since_signup_lt": 30}'),
  ('seg_dormant', 'Dormant', 'Users inactive for 14+ days', '{"days_inactive_gt": 14}');
