-- Migration: Enhanced Telemetry Schema
-- Description: Adds command_event, cli_session, performance_metric, health_score, feature_usage, admin_note, and customer_tag tables
-- Date: 2026-02-07

-- Command Event table: Granular command-level telemetry
CREATE TABLE IF NOT EXISTS command_event (
  id TEXT PRIMARY KEY NOT NULL,
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  machine_id TEXT NOT NULL REFERENCES machine(id) ON DELETE CASCADE,
  command TEXT NOT NULL CHECK(command IN ('install', 'search', 'update', 'remove', 'info', 'list', 'use', 'which', 'sbom', 'audit', 'fleet')),
  package_name TEXT,
  duration_ms INTEGER NOT NULL,
  success INTEGER NOT NULL DEFAULT 1,
  error_code TEXT,
  timestamp INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS command_event_licenseId_timestamp_idx ON command_event(license_id, timestamp);
CREATE INDEX IF NOT EXISTS command_event_command_timestamp_idx ON command_event(command, timestamp);
CREATE INDEX IF NOT EXISTS command_event_machineId_timestamp_idx ON command_event(machine_id, timestamp);

-- CLI Session table: Track CLI user sessions (named cli_session to avoid collision with better-auth session table)
CREATE TABLE IF NOT EXISTS cli_session (
  id TEXT PRIMARY KEY NOT NULL,
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  machine_id TEXT NOT NULL REFERENCES machine(id) ON DELETE CASCADE,
  session_start INTEGER NOT NULL,
  session_end INTEGER,
  commands_count INTEGER NOT NULL DEFAULT 0,
  active_minutes INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS cli_session_licenseId_sessionStart_idx ON cli_session(license_id, session_start);
CREATE INDEX IF NOT EXISTS cli_session_machineId_idx ON cli_session(machine_id);

-- Performance Metric table: Track CLI performance
CREATE TABLE IF NOT EXISTS performance_metric (
  id TEXT PRIMARY KEY NOT NULL,
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  machine_id TEXT NOT NULL REFERENCES machine(id) ON DELETE CASCADE,
  metric_type TEXT NOT NULL CHECK(metric_type IN ('startup_ms', 'search_ms', 'install_ms', 'update_ms', 'remove_ms', 'daemon_startup_ms', 'cache_hit_ms', 'cache_miss_ms')),
  value INTEGER NOT NULL,
  timestamp INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS performance_metric_metricType_timestamp_idx ON performance_metric(metric_type, timestamp);
CREATE INDEX IF NOT EXISTS performance_metric_licenseId_idx ON performance_metric(license_id);
CREATE INDEX IF NOT EXISTS performance_metric_machineId_idx ON performance_metric(machine_id);

-- Health Score table: Customer health scoring for churn prediction
CREATE TABLE IF NOT EXISTS health_score (
  id TEXT PRIMARY KEY NOT NULL,
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  score INTEGER NOT NULL CHECK(score >= 0 AND score <= 100),
  engagement_score INTEGER NOT NULL CHECK(engagement_score >= 0 AND engagement_score <= 100),
  adoption_score INTEGER NOT NULL CHECK(adoption_score >= 0 AND adoption_score <= 100),
  satisfaction_score INTEGER NOT NULL CHECK(satisfaction_score >= 0 AND satisfaction_score <= 100),
  churn_risk TEXT NOT NULL CHECK(churn_risk IN ('low', 'medium', 'high', 'critical')),
  calculated_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS health_score_licenseId_calculatedAt_idx ON health_score(license_id, calculated_at);
CREATE INDEX IF NOT EXISTS health_score_churnRisk_idx ON health_score(churn_risk);
CREATE INDEX IF NOT EXISTS health_score_score_idx ON health_score(score);

-- Feature Usage table: Track feature adoption
CREATE TABLE IF NOT EXISTS feature_usage (
  id TEXT PRIMARY KEY NOT NULL,
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  feature_name TEXT NOT NULL CHECK(feature_name IN ('aur', 'daemon', 'parallel', 'sbom', 'fleet', 'runtimes', 'audit', 'pgp', 'slsa', 'cache')),
  first_used INTEGER NOT NULL,
  last_used INTEGER NOT NULL,
  usage_count INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer)),
  UNIQUE(license_id, feature_name)
);

CREATE INDEX IF NOT EXISTS feature_usage_licenseId_idx ON feature_usage(license_id);
CREATE INDEX IF NOT EXISTS feature_usage_featureName_idx ON feature_usage(feature_name);

-- Admin Note table: CRM notes for customers
CREATE TABLE IF NOT EXISTS admin_note (
  id TEXT PRIMARY KEY NOT NULL,
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  admin_user_id TEXT NOT NULL REFERENCES user(id) ON DELETE CASCADE,
  note_type TEXT NOT NULL CHECK(note_type IN ('call', 'email', 'ticket', 'internal', 'meeting', 'followup')),
  content TEXT NOT NULL,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS admin_note_licenseId_createdAt_idx ON admin_note(license_id, created_at);
CREATE INDEX IF NOT EXISTS admin_note_adminUserId_idx ON admin_note(admin_user_id);
CREATE INDEX IF NOT EXISTS admin_note_noteType_idx ON admin_note(note_type);

-- Customer Tag table: Tagging system for customer segmentation
CREATE TABLE IF NOT EXISTS customer_tag (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL UNIQUE,
  color TEXT NOT NULL DEFAULT '#6b7280',
  description TEXT,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS customer_tag_name_idx ON customer_tag(name);

-- License Tag junction table: Many-to-many relationship between licenses and tags
CREATE TABLE IF NOT EXISTS license_tag (
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  tag_id TEXT NOT NULL REFERENCES customer_tag(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer)),
  PRIMARY KEY (license_id, tag_id)
);

CREATE INDEX IF NOT EXISTS license_tag_licenseId_idx ON license_tag(license_id);
CREATE INDEX IF NOT EXISTS license_tag_tagId_idx ON license_tag(tag_id);

-- Seed some default customer tags
INSERT INTO customer_tag (id, name, color, description) VALUES
('tag_enterprise', 'Enterprise', '#7c3aed', 'Enterprise tier customers'),
('tag_high_value', 'High Value', '#059669', 'High-value customers requiring priority support'),
('tag_at_risk', 'At Risk', '#dc2626', 'Customers showing churn risk signals'),
('tag_champion', 'Champion', '#2563eb', 'Product champions and advocates'),
('tag_early_adopter', 'Early Adopter', '#d97706', 'Early adopters from beta/launch period'),
('tag_power_user', 'Power User', '#7c3aed', 'High-engagement power users'),
('tag_needs_onboarding', 'Needs Onboarding', '#f59e0b', 'Customers requiring onboarding assistance'),
('tag_expansion', 'Expansion Opportunity', '#10b981', 'Potential upsell/expansion candidates');
