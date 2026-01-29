-- Migration: Add Telemetry Tables
-- Description: Adds license, machine, usage_daily, achievement_definition, and user_achievement tables
-- Date: 2026-01-29

-- License table: Stores user license information
CREATE TABLE IF NOT EXISTS license (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES user(id) ON DELETE CASCADE,
  license_key TEXT NOT NULL UNIQUE,
  tier TEXT NOT NULL DEFAULT 'free' CHECK(tier IN ('free', 'team', 'enterprise')),
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'suspended', 'expired', 'cancelled')),
  max_machines INTEGER NOT NULL DEFAULT 1,
  expires_at INTEGER,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer)),
  updated_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS license_userId_idx ON license(user_id);
CREATE INDEX IF NOT EXISTS license_key_idx ON license(license_key);
CREATE INDEX IF NOT EXISTS license_status_idx ON license(status);

-- Machine table: Stores CLI machine information
CREATE TABLE IF NOT EXISTS machine (
  id TEXT PRIMARY KEY NOT NULL,
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  machine_id TEXT NOT NULL,
  hostname TEXT,
  os TEXT,
  arch TEXT,
  omg_version TEXT,
  is_active INTEGER NOT NULL DEFAULT 1,
  first_seen_at INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS machine_licenseId_idx ON machine(license_id);
CREATE INDEX IF NOT EXISTS machine_machineId_idx ON machine(machine_id);
CREATE INDEX IF NOT EXISTS machine_isActive_idx ON machine(is_active);
CREATE INDEX IF NOT EXISTS machine_lastSeenAt_idx ON machine(last_seen_at);

-- Usage Daily table: Stores daily usage statistics
CREATE TABLE IF NOT EXISTS usage_daily (
  id TEXT PRIMARY KEY NOT NULL,
  license_id TEXT NOT NULL REFERENCES license(id) ON DELETE CASCADE,
  date TEXT NOT NULL,
  commands_run INTEGER NOT NULL DEFAULT 0,
  packages_installed INTEGER NOT NULL DEFAULT 0,
  packages_searched INTEGER NOT NULL DEFAULT 0,
  runtimes_switched INTEGER NOT NULL DEFAULT 0,
  sbom_generated INTEGER NOT NULL DEFAULT 0,
  vulnerabilities_found INTEGER NOT NULL DEFAULT 0,
  time_saved_ms INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer)),
  updated_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer)),
  UNIQUE(license_id, date)
);

CREATE INDEX IF NOT EXISTS usage_licenseId_date_idx ON usage_daily(license_id, date);
CREATE INDEX IF NOT EXISTS usage_date_idx ON usage_daily(date);

-- Achievement Definition table: Stores predefined achievements
CREATE TABLE IF NOT EXISTS achievement_definition (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  icon TEXT NOT NULL,
  category TEXT NOT NULL CHECK(category IN ('milestone', 'speed', 'explorer', 'master', 'special')),
  requirement TEXT NOT NULL,
  points INTEGER NOT NULL DEFAULT 10,
  is_hidden INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer))
);

CREATE INDEX IF NOT EXISTS achievement_category_idx ON achievement_definition(category);
CREATE INDEX IF NOT EXISTS achievement_sortOrder_idx ON achievement_definition(sort_order);

-- User Achievement table: Tracks user achievement progress
CREATE TABLE IF NOT EXISTS user_achievement (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES user(id) ON DELETE CASCADE,
  achievement_id TEXT NOT NULL REFERENCES achievement_definition(id) ON DELETE CASCADE,
  progress INTEGER NOT NULL DEFAULT 0,
  is_unlocked INTEGER NOT NULL DEFAULT 0,
  unlocked_at INTEGER,
  created_at INTEGER NOT NULL DEFAULT (cast(unixepoch('subsecond') * 1000 as integer)),
  UNIQUE(user_id, achievement_id)
);

CREATE INDEX IF NOT EXISTS userAchievement_userId_idx ON user_achievement(user_id);
CREATE INDEX IF NOT EXISTS userAchievement_achievementId_idx ON user_achievement(achievement_id);
CREATE INDEX IF NOT EXISTS userAchievement_isUnlocked_idx ON user_achievement(is_unlocked);

-- Drop old user_license table if it exists (replaced by license table)
DROP TABLE IF EXISTS user_license;
