-- Migration: Add analytics tables for comprehensive telemetry
-- Run with: bunx wrangler d1 execute omg-licensing --remote --file=./migrations/004_analytics_tables.sql

-- Analytics events (raw event stream)
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

-- Analytics daily aggregates
CREATE TABLE IF NOT EXISTS analytics_daily (
  date TEXT NOT NULL,
  metric TEXT NOT NULL,
  dimension TEXT NOT NULL,
  value INTEGER DEFAULT 0,
  PRIMARY KEY (date, metric, dimension)
);

-- Analytics active users (for DAU/WAU/MAU)
CREATE TABLE IF NOT EXISTS analytics_active_users (
  date TEXT NOT NULL,
  machine_id TEXT NOT NULL,
  PRIMARY KEY (date, machine_id)
);

-- Analytics performance metrics
CREATE TABLE IF NOT EXISTS analytics_performance (
  id TEXT PRIMARY KEY,
  operation TEXT NOT NULL,
  duration_ms INTEGER NOT NULL,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_analytics_events_type ON analytics_events(event_type);
CREATE INDEX IF NOT EXISTS idx_analytics_events_session ON analytics_events(session_id);
CREATE INDEX IF NOT EXISTS idx_analytics_events_machine ON analytics_events(machine_id);
CREATE INDEX IF NOT EXISTS idx_analytics_events_created ON analytics_events(created_at);
CREATE INDEX IF NOT EXISTS idx_analytics_daily_date ON analytics_daily(date);
CREATE INDEX IF NOT EXISTS idx_analytics_daily_metric ON analytics_daily(metric);
CREATE INDEX IF NOT EXISTS idx_analytics_performance_op ON analytics_performance(operation);
CREATE INDEX IF NOT EXISTS idx_analytics_performance_created ON analytics_performance(created_at);
