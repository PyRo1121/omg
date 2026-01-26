-- OMG SaaS Production Schema - CONSOLIDATED
-- Includes base schema + all migrations (003-007)
-- Run with: bunx wrangler d1 execute omg-licensing --remote --file=./schema-production.sql

-- ============================================================================
-- CORE CUSTOMER & LICENSE TABLES
-- ============================================================================

CREATE TABLE IF NOT EXISTS customers (
  id TEXT PRIMARY KEY,
  stripe_customer_id TEXT UNIQUE,
  email TEXT NOT NULL,
  company TEXT,
  tier TEXT DEFAULT 'free',
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_customers_email ON customers(email);
CREATE INDEX IF NOT EXISTS idx_customers_stripe ON customers(stripe_customer_id);

CREATE TABLE IF NOT EXISTS licenses (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  license_key TEXT UNIQUE NOT NULL,
  tier TEXT NOT NULL DEFAULT 'free',
  status TEXT DEFAULT 'active',
  max_seats INTEGER,
  used_seats INTEGER DEFAULT 0,
  max_machines INTEGER DEFAULT 1,
  expires_at DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_licenses_key ON licenses(license_key);
CREATE INDEX IF NOT EXISTS idx_licenses_customer ON licenses(customer_id);

-- ============================================================================
-- MACHINES (TEAM MEMBERS)
-- ============================================================================

CREATE TABLE IF NOT EXISTS machines (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL,
  machine_id TEXT NOT NULL,
  hostname TEXT,
  os TEXT,
  arch TEXT,
  omg_version TEXT,
  user_name TEXT,
  user_email TEXT,
  last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  first_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  is_active INTEGER DEFAULT 1,
  revoked_at DATETIME,
  UNIQUE(license_id, machine_id),
  FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_machines_license ON machines(license_id);
CREATE INDEX IF NOT EXISTS idx_machines_active ON machines(is_active);

-- ============================================================================
-- USAGE TRACKING
-- ============================================================================

-- License-level daily usage (aggregated)
CREATE TABLE IF NOT EXISTS usage_daily (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL,
  date TEXT NOT NULL,
  commands_run INTEGER DEFAULT 0,
  packages_installed INTEGER DEFAULT 0,
  packages_searched INTEGER DEFAULT 0,
  runtimes_switched INTEGER DEFAULT 0,
  sbom_generated INTEGER DEFAULT 0,
  vulnerabilities_found INTEGER DEFAULT 0,
  time_saved_ms INTEGER DEFAULT 0,
  UNIQUE(license_id, date),
  FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_usage_daily_license_date ON usage_daily(license_id, date);

-- Per-member daily usage (for team insights)
CREATE TABLE IF NOT EXISTS usage_member_daily (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL,
  machine_id TEXT NOT NULL,
  date TEXT NOT NULL,
  commands_run INTEGER DEFAULT 0,
  packages_installed INTEGER DEFAULT 0,
  runtimes_switched INTEGER DEFAULT 0,
  time_saved_ms INTEGER DEFAULT 0,
  UNIQUE(license_id, machine_id, date),
  FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_usage_member_date ON usage_member_daily(license_id, date);
CREATE INDEX IF NOT EXISTS idx_usage_member_machine ON usage_member_daily(machine_id);

-- Legacy usage table (kept for compatibility)
CREATE TABLE IF NOT EXISTS usage (
  id TEXT PRIMARY KEY,
  license_key TEXT NOT NULL,
  feature TEXT NOT NULL,
  count INTEGER DEFAULT 1,
  machine_id TEXT,
  timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_usage_license ON usage(license_key);

-- ============================================================================
-- ANALYTICS TABLES
-- ============================================================================

-- Raw analytics events
CREATE TABLE IF NOT EXISTS analytics_events (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  event_name TEXT NOT NULL,
  properties TEXT,
  timestamp TEXT NOT NULL,
  session_id TEXT NOT NULL,
  machine_id TEXT NOT NULL,
  license_key TEXT,
  version TEXT,
  platform TEXT,
  duration_ms INTEGER,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_analytics_events_type ON analytics_events(event_type);
CREATE INDEX IF NOT EXISTS idx_analytics_events_session ON analytics_events(session_id);
CREATE INDEX IF NOT EXISTS idx_analytics_events_machine ON analytics_events(machine_id);
CREATE INDEX IF NOT EXISTS idx_analytics_events_created ON analytics_events(created_at);

-- Daily aggregates
CREATE TABLE IF NOT EXISTS analytics_daily (
  date TEXT NOT NULL,
  metric TEXT NOT NULL,
  dimension TEXT NOT NULL,
  value INTEGER DEFAULT 0,
  PRIMARY KEY (date, metric, dimension)
);

CREATE INDEX IF NOT EXISTS idx_analytics_daily_date ON analytics_daily(date);
CREATE INDEX IF NOT EXISTS idx_analytics_daily_metric ON analytics_daily(metric);

-- Active users tracking
CREATE TABLE IF NOT EXISTS analytics_active_users (
  date TEXT NOT NULL,
  machine_id TEXT NOT NULL,
  PRIMARY KEY (date, machine_id)
);

-- Performance metrics
CREATE TABLE IF NOT EXISTS analytics_performance (
  id TEXT PRIMARY KEY,
  operation TEXT NOT NULL,
  duration_ms INTEGER NOT NULL,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_analytics_performance_op ON analytics_performance(operation);
CREATE INDEX IF NOT EXISTS idx_analytics_performance_created ON analytics_performance(created_at);

-- Package install/search tracking
CREATE TABLE IF NOT EXISTS analytics_packages (
  package_name TEXT PRIMARY KEY,
  install_count INTEGER DEFAULT 0,
  search_count INTEGER DEFAULT 0,
  last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Error tracking
CREATE TABLE IF NOT EXISTS analytics_errors (
  error_message TEXT PRIMARY KEY,
  occurrences INTEGER DEFAULT 0,
  last_occurred_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Regional performance
CREATE TABLE IF NOT EXISTS analytics_regional_perf (
  region TEXT NOT NULL,
  operation TEXT NOT NULL,
  avg_duration_ms INTEGER NOT NULL,
  count INTEGER DEFAULT 1,
  PRIMARY KEY (region, operation)
);

-- ============================================================================
-- BILLING & SUBSCRIPTIONS
-- ============================================================================

CREATE TABLE IF NOT EXISTS subscriptions (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  stripe_subscription_id TEXT UNIQUE,
  stripe_price_id TEXT,
  status TEXT DEFAULT 'active',
  current_period_start DATETIME,
  current_period_end DATETIME,
  cancel_at_period_end INTEGER DEFAULT 0,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_subscriptions_customer ON subscriptions(customer_id);

CREATE TABLE IF NOT EXISTS invoices (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  stripe_invoice_id TEXT UNIQUE,
  amount_cents INTEGER NOT NULL,
  currency TEXT DEFAULT 'usd',
  status TEXT,
  invoice_url TEXT,
  invoice_pdf TEXT,
  period_start DATETIME,
  period_end DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_invoices_customer ON invoices(customer_id);

-- ============================================================================
-- AUTHENTICATION
-- ============================================================================

CREATE TABLE IF NOT EXISTS auth_codes (
  id TEXT PRIMARY KEY,
  email TEXT NOT NULL,
  code TEXT NOT NULL,
  expires_at DATETIME NOT NULL,
  used INTEGER DEFAULT 0,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_auth_codes_email ON auth_codes(email);

CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  token TEXT UNIQUE NOT NULL,
  ip_address TEXT,
  user_agent TEXT,
  expires_at DATETIME NOT NULL,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token);
CREATE INDEX IF NOT EXISTS idx_sessions_customer ON sessions(customer_id);

-- ============================================================================
-- AUDIT LOGGING
-- ============================================================================

CREATE TABLE IF NOT EXISTS audit_log (
  id TEXT PRIMARY KEY,
  customer_id TEXT,
  action TEXT NOT NULL,
  resource_type TEXT,
  resource_id TEXT,
  ip_address TEXT,
  user_agent TEXT,
  metadata TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_customer ON audit_log(customer_id);
CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at);

-- ============================================================================
-- ACHIEVEMENTS
-- ============================================================================

CREATE TABLE IF NOT EXISTS achievements (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  achievement_id TEXT NOT NULL,
  unlocked_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(customer_id, achievement_id),
  FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_achievements_customer ON achievements(customer_id);

-- ============================================================================
-- INSTALL TRACKING
-- ============================================================================

CREATE TABLE IF NOT EXISTS install_stats (
  id TEXT PRIMARY KEY,
  install_id TEXT UNIQUE NOT NULL,
  version TEXT,
  platform TEXT,
  backend TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_install_stats_install_id ON install_stats(install_id);
CREATE INDEX IF NOT EXISTS idx_install_stats_created ON install_stats(created_at);

-- ============================================================================
-- TEAM CONTROLS (Team/Enterprise)
-- ============================================================================

CREATE TABLE IF NOT EXISTS team_compliance (
  license_id TEXT PRIMARY KEY,
  enforce_signed_packages INTEGER DEFAULT 0,
  min_omg_version TEXT,
  allowed_runtimes TEXT,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS policies (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('runtime', 'package', 'security', 'network')),
  rule TEXT NOT NULL,
  value TEXT NOT NULL,
  enforced INTEGER DEFAULT 1,
  created_at TEXT DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(license_id, scope, rule),
  FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_policies_license ON policies(license_id);
CREATE INDEX IF NOT EXISTS idx_policies_scope ON policies(license_id, scope);

CREATE TABLE IF NOT EXISTS notification_settings (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL,
  type TEXT NOT NULL,
  enabled INTEGER DEFAULT 1,
  threshold INTEGER,
  channels TEXT DEFAULT '["dashboard"]',
  created_at TEXT DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(license_id, type),
  FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_notification_settings_license ON notification_settings(license_id);

CREATE TABLE IF NOT EXISTS alert_thresholds (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL,
  threshold_type TEXT NOT NULL,
  value INTEGER NOT NULL,
  created_at TEXT DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(license_id, threshold_type),
  FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_alert_thresholds_license ON alert_thresholds(license_id);
