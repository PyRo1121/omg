-- OMG SaaS Licensing Database Schema (Fresh Start)
-- Unified schema with consistent naming: customer_id everywhere
-- Run with: bunx wrangler d1 execute omg-licensing --remote --file=./schema-fresh.sql

-- Drop all existing tables
DROP TABLE IF EXISTS audit_log;
DROP TABLE IF EXISTS invoices;
DROP TABLE IF EXISTS achievements;
DROP TABLE IF EXISTS usage_daily;
DROP TABLE IF EXISTS machines;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS auth_codes;
DROP TABLE IF EXISTS subscriptions;
DROP TABLE IF EXISTS licenses;
DROP TABLE IF EXISTS usage;
DROP TABLE IF EXISTS install_stats;
DROP TABLE IF EXISTS install_details;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS customers;

-- Core customer table
CREATE TABLE customers (
  id TEXT PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  company TEXT,
  stripe_customer_id TEXT UNIQUE,
  tier TEXT DEFAULT 'free',
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Licenses (one per customer)
CREATE TABLE licenses (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL REFERENCES customers(id),
  license_key TEXT UNIQUE NOT NULL,
  tier TEXT NOT NULL DEFAULT 'free',
  status TEXT DEFAULT 'active',
  max_seats INTEGER DEFAULT 1,
  expires_at DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Subscriptions (Stripe)
CREATE TABLE subscriptions (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL REFERENCES customers(id),
  stripe_subscription_id TEXT UNIQUE,
  status TEXT DEFAULT 'active',
  current_period_start DATETIME,
  current_period_end DATETIME,
  cancel_at_period_end INTEGER DEFAULT 0,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Invoices
CREATE TABLE invoices (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL REFERENCES customers(id),
  stripe_invoice_id TEXT UNIQUE,
  amount_cents INTEGER NOT NULL,
  currency TEXT DEFAULT 'usd',
  status TEXT DEFAULT 'pending',
  paid_at DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Auth codes (OTP)
CREATE TABLE auth_codes (
  id TEXT PRIMARY KEY,
  email TEXT NOT NULL,
  code TEXT NOT NULL,
  expires_at DATETIME NOT NULL,
  used INTEGER DEFAULT 0,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Sessions
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL REFERENCES customers(id),
  token TEXT UNIQUE NOT NULL,
  ip_address TEXT,
  user_agent TEXT,
  expires_at DATETIME NOT NULL,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Machines (activated devices)
CREATE TABLE machines (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL REFERENCES licenses(id),
  machine_id TEXT NOT NULL,
  hostname TEXT,
  os TEXT,
  arch TEXT,
  omg_version TEXT,
  user_name TEXT,
  user_email TEXT,
  is_active INTEGER DEFAULT 1,
  first_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(license_id, machine_id)
);

-- Daily usage stats
CREATE TABLE usage_daily (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL REFERENCES licenses(id),
  date TEXT NOT NULL,
  commands_run INTEGER DEFAULT 0,
  packages_installed INTEGER DEFAULT 0,
  packages_searched INTEGER DEFAULT 0,
  runtimes_switched INTEGER DEFAULT 0,
  sbom_generated INTEGER DEFAULT 0,
  vulnerabilities_found INTEGER DEFAULT 0,
  time_saved_ms INTEGER DEFAULT 0,
  UNIQUE(license_id, date)
);

-- Achievements
CREATE TABLE achievements (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL REFERENCES customers(id),
  achievement_id TEXT NOT NULL,
  unlocked_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(customer_id, achievement_id)
);

-- Audit log
CREATE TABLE audit_log (
  id TEXT PRIMARY KEY,
  customer_id TEXT,
  action TEXT NOT NULL,
  resource_type TEXT,
  resource_id TEXT,
  ip_address TEXT,
  user_agent TEXT,
  metadata TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Install statistics (anonymous telemetry)
CREATE TABLE install_stats (
  id TEXT PRIMARY KEY,
  install_id TEXT,
  platform TEXT,
  arch TEXT,
  version TEXT,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Analytics events (raw event stream)
CREATE TABLE analytics_events (
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

-- Analytics daily aggregates
CREATE TABLE analytics_daily (
  date TEXT NOT NULL,
  metric TEXT NOT NULL,
  dimension TEXT NOT NULL,
  value INTEGER DEFAULT 0,
  PRIMARY KEY (date, metric, dimension)
);

-- Analytics active users (for DAU/WAU/MAU)
CREATE TABLE analytics_active_users (
  date TEXT NOT NULL,
  machine_id TEXT NOT NULL,
  PRIMARY KEY (date, machine_id)
);

-- Analytics performance metrics
CREATE TABLE analytics_performance (
  id TEXT PRIMARY KEY,
  operation TEXT NOT NULL,
  duration_ms INTEGER NOT NULL,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX idx_customers_email ON customers(email);
CREATE INDEX idx_customers_stripe ON customers(stripe_customer_id);
CREATE INDEX idx_licenses_customer ON licenses(customer_id);
CREATE INDEX idx_licenses_key ON licenses(license_key);
CREATE INDEX idx_subscriptions_customer ON subscriptions(customer_id);
CREATE INDEX idx_sessions_token ON sessions(token);
CREATE INDEX idx_sessions_customer ON sessions(customer_id);
CREATE INDEX idx_auth_codes_email ON auth_codes(email);
CREATE INDEX idx_machines_license ON machines(license_id);
CREATE INDEX idx_usage_daily_license ON usage_daily(license_id);
CREATE INDEX idx_usage_daily_date ON usage_daily(date);
CREATE INDEX idx_achievements_customer ON achievements(customer_id);
CREATE INDEX idx_audit_log_customer ON audit_log(customer_id);
CREATE INDEX idx_audit_log_action ON audit_log(action);
CREATE INDEX idx_invoices_customer ON invoices(customer_id);
CREATE INDEX idx_analytics_events_type ON analytics_events(event_type);
CREATE INDEX idx_analytics_events_session ON analytics_events(session_id);
CREATE INDEX idx_analytics_events_machine ON analytics_events(machine_id);
CREATE INDEX idx_analytics_events_created ON analytics_events(created_at);
CREATE INDEX idx_analytics_daily_date ON analytics_daily(date);
CREATE INDEX idx_analytics_daily_metric ON analytics_daily(metric);
CREATE INDEX idx_analytics_performance_op ON analytics_performance(operation);
CREATE INDEX idx_analytics_performance_created ON analytics_performance(created_at);
