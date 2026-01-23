-- Migration: Team Intelligence & Fleet Observability
-- Enables per-member productivity tracking and ROI analysis

-- Track usage per machine (member) for true team insights
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

-- Track team-level compliance and security posture
CREATE TABLE IF NOT EXISTS team_compliance (
  license_id TEXT PRIMARY KEY,
  enforce_signed_packages INTEGER DEFAULT 0,
  min_omg_version TEXT,
  allowed_runtimes TEXT, -- JSON array
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (license_id) REFERENCES licenses(id) ON DELETE CASCADE
);

-- Index for fast team lookups
CREATE INDEX IF NOT EXISTS idx_usage_member_date ON usage_member_daily(license_id, date);
CREATE INDEX IF NOT EXISTS idx_usage_member_machine ON usage_member_daily(machine_id);
