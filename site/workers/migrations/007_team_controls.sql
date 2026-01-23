-- Migration 007: Team Controls Tables
-- Adds policies, notification settings, and alert thresholds for Team/Enterprise tiers

CREATE TABLE IF NOT EXISTS policies (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL REFERENCES licenses(id),
  scope TEXT NOT NULL CHECK (scope IN ('runtime', 'package', 'security', 'network')),
  rule TEXT NOT NULL,
  value TEXT NOT NULL,
  enforced INTEGER DEFAULT 1,
  created_at TEXT DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(license_id, scope, rule)
);

CREATE INDEX IF NOT EXISTS idx_policies_license ON policies(license_id);
CREATE INDEX IF NOT EXISTS idx_policies_scope ON policies(license_id, scope);

CREATE TABLE IF NOT EXISTS notification_settings (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL REFERENCES licenses(id),
  type TEXT NOT NULL,
  enabled INTEGER DEFAULT 1,
  threshold INTEGER,
  channels TEXT DEFAULT '["dashboard"]',
  created_at TEXT DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(license_id, type)
);

CREATE INDEX IF NOT EXISTS idx_notification_settings_license ON notification_settings(license_id);

CREATE TABLE IF NOT EXISTS alert_thresholds (
  id TEXT PRIMARY KEY,
  license_id TEXT NOT NULL REFERENCES licenses(id),
  threshold_type TEXT NOT NULL,
  value INTEGER NOT NULL,
  created_at TEXT DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(license_id, threshold_type)
);

CREATE INDEX IF NOT EXISTS idx_alert_thresholds_license ON alert_thresholds(license_id);
