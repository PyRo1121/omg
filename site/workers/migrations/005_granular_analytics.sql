-- Migration: Add granular analytics for packages and errors
-- Run with: bunx wrangler d1 execute omg-licensing --remote --file=./migrations/005_granular_analytics.sql

-- Top packages tracked across all users
CREATE TABLE IF NOT EXISTS analytics_packages (
  package_name TEXT PRIMARY KEY,
  install_count INTEGER DEFAULT 0,
  search_count INTEGER DEFAULT 0,
  last_seen_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Error logs with frequency
CREATE TABLE IF NOT EXISTS analytics_errors (
  error_message TEXT PRIMARY KEY,
  occurrences INTEGER DEFAULT 0,
  last_occurred_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Command performance by region (using Cloudflare headers)
CREATE TABLE IF NOT EXISTS analytics_regional_perf (
  region TEXT NOT NULL,
  operation TEXT NOT NULL,
  avg_duration_ms INTEGER NOT NULL,
  count INTEGER DEFAULT 1,
  PRIMARY KEY (region, operation)
);
