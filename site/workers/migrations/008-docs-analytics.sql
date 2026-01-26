-- Migration 008: Docs Site Analytics
-- Web analytics for omg-docs.pages.dev
-- Tracks pageviews, user journeys, UTM campaigns, and user behavior

-- ============================================================================
-- DOCS ANALYTICS - RAW EVENTS
-- ============================================================================

-- Raw events from docs site (high-volume, short retention)
CREATE TABLE IF NOT EXISTS docs_analytics_events (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL CHECK (event_type IN ('pageview', 'interaction', 'navigation', 'performance')),
  event_name TEXT NOT NULL,
  properties TEXT NOT NULL,  -- JSON blob: url, referrer, utm_*, viewport, etc.
  timestamp TEXT NOT NULL,   -- ISO 8601
  session_id TEXT NOT NULL,
  duration_ms INTEGER,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_docs_events_type ON docs_analytics_events(event_type);
CREATE INDEX IF NOT EXISTS idx_docs_events_session ON docs_analytics_events(session_id);
CREATE INDEX IF NOT EXISTS idx_docs_events_created ON docs_analytics_events(created_at);
CREATE INDEX IF NOT EXISTS idx_docs_events_name ON docs_analytics_events(event_name);

-- ============================================================================
-- DOCS ANALYTICS - DAILY AGGREGATES
-- ============================================================================

-- Daily pageview aggregates (long retention)
CREATE TABLE IF NOT EXISTS docs_analytics_pageviews_daily (
  date TEXT NOT NULL,
  path TEXT NOT NULL,
  views INTEGER DEFAULT 0,
  unique_sessions INTEGER DEFAULT 0,
  avg_time_on_page_ms INTEGER DEFAULT 0,
  bounce_rate REAL DEFAULT 0.0,
  PRIMARY KEY (date, path)
);

CREATE INDEX IF NOT EXISTS idx_docs_pageviews_date ON docs_analytics_pageviews_daily(date);
CREATE INDEX IF NOT EXISTS idx_docs_pageviews_path ON docs_analytics_pageviews_daily(path);

-- Daily UTM campaign tracking
CREATE TABLE IF NOT EXISTS docs_analytics_utm_daily (
  date TEXT NOT NULL,
  utm_source TEXT,
  utm_medium TEXT,
  utm_campaign TEXT,
  sessions INTEGER DEFAULT 0,
  pageviews INTEGER DEFAULT 0,
  PRIMARY KEY (date, utm_source, utm_medium, utm_campaign)
);

CREATE INDEX IF NOT EXISTS idx_docs_utm_date ON docs_analytics_utm_daily(date);

-- Daily referrer tracking
CREATE TABLE IF NOT EXISTS docs_analytics_referrers_daily (
  date TEXT NOT NULL,
  referrer TEXT NOT NULL,
  sessions INTEGER DEFAULT 0,
  pageviews INTEGER DEFAULT 0,
  PRIMARY KEY (date, referrer)
);

CREATE INDEX IF NOT EXISTS idx_docs_referrers_date ON docs_analytics_referrers_daily(date);

-- Daily interaction tracking (clicks, copy, scroll)
CREATE TABLE IF NOT EXISTS docs_analytics_interactions_daily (
  date TEXT NOT NULL,
  interaction_type TEXT NOT NULL,  -- click, copy, scroll
  target TEXT NOT NULL,             -- button_id, code_block, etc.
  count INTEGER DEFAULT 0,
  PRIMARY KEY (date, interaction_type, target)
);

CREATE INDEX IF NOT EXISTS idx_docs_interactions_date ON docs_analytics_interactions_daily(date);

-- ============================================================================
-- DOCS ANALYTICS - SESSION TRACKING
-- ============================================================================

-- Active sessions (for real-time tracking, cleared daily)
CREATE TABLE IF NOT EXISTS docs_analytics_sessions (
  session_id TEXT PRIMARY KEY,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  page_count INTEGER DEFAULT 1,
  utm_source TEXT,
  utm_medium TEXT,
  utm_campaign TEXT,
  referrer TEXT,
  entry_page TEXT,
  exit_page TEXT,
  total_time_ms INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_docs_sessions_first_seen ON docs_analytics_sessions(first_seen_at);

-- ============================================================================
-- DOCS ANALYTICS - GEOGRAPHIC DATA
-- ============================================================================

-- Geographic distribution (from CF-IPCountry header)
CREATE TABLE IF NOT EXISTS docs_analytics_geo_daily (
  date TEXT NOT NULL,
  country_code TEXT NOT NULL,
  sessions INTEGER DEFAULT 0,
  pageviews INTEGER DEFAULT 0,
  PRIMARY KEY (date, country_code)
);

CREATE INDEX IF NOT EXISTS idx_docs_geo_date ON docs_analytics_geo_daily(date);

-- ============================================================================
-- DOCS ANALYTICS - PERFORMANCE METRICS
-- ============================================================================

-- Page load performance
CREATE TABLE IF NOT EXISTS docs_analytics_performance_daily (
  date TEXT NOT NULL,
  path TEXT NOT NULL,
  avg_load_time_ms INTEGER DEFAULT 0,
  p50_load_time_ms INTEGER DEFAULT 0,
  p95_load_time_ms INTEGER DEFAULT 0,
  p99_load_time_ms INTEGER DEFAULT 0,
  sample_count INTEGER DEFAULT 0,
  PRIMARY KEY (date, path)
);

CREATE INDEX IF NOT EXISTS idx_docs_performance_date ON docs_analytics_performance_daily(date);
