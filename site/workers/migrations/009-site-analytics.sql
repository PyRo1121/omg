-- Migration 009: Site-Wide Analytics
-- Comprehensive analytics for pyro1121.com main site
-- Privacy-first, GDPR compliant, cookieless tracking

-- ============================================================================
-- SITE ANALYTICS - RAW EVENTS (7-day retention)
-- ============================================================================

CREATE TABLE IF NOT EXISTS site_analytics_events (
  id TEXT PRIMARY KEY,
  event_type TEXT NOT NULL CHECK (event_type IN ('pageview', 'click', 'form', 'error', 'performance')),
  event_name TEXT NOT NULL,
  properties TEXT NOT NULL,  -- JSON: url, referrer, utm_*, viewport, etc.
  timestamp INTEGER NOT NULL, -- Unix ms for optimal D1 performance
  session_id TEXT NOT NULL,
  visitor_id TEXT NOT NULL,   -- Privacy-preserving hash (rotated daily)
  country_code TEXT,          -- From CF-IPCountry
  city TEXT,                  -- From CF-City (optional)
  duration_ms INTEGER,
  created_at INTEGER DEFAULT (unixepoch() * 1000)
);

CREATE INDEX IF NOT EXISTS idx_site_events_type ON site_analytics_events(event_type);
CREATE INDEX IF NOT EXISTS idx_site_events_timestamp ON site_analytics_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_site_events_session ON site_analytics_events(session_id);
CREATE INDEX IF NOT EXISTS idx_site_events_visitor ON site_analytics_events(visitor_id);
CREATE INDEX IF NOT EXISTS idx_site_events_country ON site_analytics_events(country_code);

-- ============================================================================
-- SITE ANALYTICS - VISITOR TRACKING (Privacy-preserving)
-- ============================================================================

-- Rotating salts for privacy-preserving visitor IDs (Plausible-style)
CREATE TABLE IF NOT EXISTS analytics_salts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  salt BLOB NOT NULL,
  inserted_at INTEGER DEFAULT (unixepoch() * 1000)
);

CREATE INDEX IF NOT EXISTS idx_salts_inserted ON analytics_salts(inserted_at);

-- ============================================================================
-- SITE ANALYTICS - DAILY AGGREGATES (Long retention)
-- ============================================================================

-- Daily pageview aggregates
CREATE TABLE IF NOT EXISTS site_analytics_pageviews_daily (
  date TEXT NOT NULL,
  path TEXT NOT NULL,
  views INTEGER DEFAULT 0,
  unique_visitors INTEGER DEFAULT 0,
  unique_sessions INTEGER DEFAULT 0,
  avg_time_on_page_ms INTEGER DEFAULT 0,
  bounce_count INTEGER DEFAULT 0,
  entry_count INTEGER DEFAULT 0,
  exit_count INTEGER DEFAULT 0,
  PRIMARY KEY (date, path)
);

CREATE INDEX IF NOT EXISTS idx_site_pageviews_date ON site_analytics_pageviews_daily(date);

-- Daily geographic distribution
CREATE TABLE IF NOT EXISTS site_analytics_geo_daily (
  date TEXT NOT NULL,
  country_code TEXT NOT NULL,
  city TEXT DEFAULT 'Unknown',
  visitors INTEGER DEFAULT 0,
  sessions INTEGER DEFAULT 0,
  pageviews INTEGER DEFAULT 0,
  PRIMARY KEY (date, country_code, city)
);

CREATE INDEX IF NOT EXISTS idx_site_geo_date ON site_analytics_geo_daily(date);
CREATE INDEX IF NOT EXISTS idx_site_geo_country ON site_analytics_geo_daily(country_code);

-- Daily referrer tracking
CREATE TABLE IF NOT EXISTS site_analytics_referrers_daily (
  date TEXT NOT NULL,
  referrer_domain TEXT NOT NULL,
  referrer_path TEXT,
  visitors INTEGER DEFAULT 0,
  sessions INTEGER DEFAULT 0,
  pageviews INTEGER DEFAULT 0,
  PRIMARY KEY (date, referrer_domain)
);

CREATE INDEX IF NOT EXISTS idx_site_referrers_date ON site_analytics_referrers_daily(date);

-- Daily device/browser breakdown
CREATE TABLE IF NOT EXISTS site_analytics_devices_daily (
  date TEXT NOT NULL,
  device_type TEXT NOT NULL,  -- desktop, mobile, tablet
  browser TEXT NOT NULL,
  os TEXT NOT NULL,
  visitors INTEGER DEFAULT 0,
  sessions INTEGER DEFAULT 0,
  PRIMARY KEY (date, device_type, browser, os)
);

CREATE INDEX IF NOT EXISTS idx_site_devices_date ON site_analytics_devices_daily(date);

-- Daily UTM campaign tracking
CREATE TABLE IF NOT EXISTS site_analytics_utm_daily (
  date TEXT NOT NULL,
  utm_source TEXT,
  utm_medium TEXT,
  utm_campaign TEXT,
  utm_content TEXT,
  utm_term TEXT,
  visitors INTEGER DEFAULT 0,
  sessions INTEGER DEFAULT 0,
  conversions INTEGER DEFAULT 0,
  PRIMARY KEY (date, utm_source, utm_medium, utm_campaign)
);

CREATE INDEX IF NOT EXISTS idx_site_utm_date ON site_analytics_utm_daily(date);

-- ============================================================================
-- SITE ANALYTICS - REALTIME (Short-lived, for live dashboard)
-- ============================================================================

CREATE TABLE IF NOT EXISTS site_analytics_realtime (
  visitor_id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  page_path TEXT NOT NULL,
  country_code TEXT,
  city TEXT,
  referrer TEXT,
  last_seen_at INTEGER NOT NULL,  -- Unix ms
  page_count INTEGER DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_site_realtime_seen ON site_analytics_realtime(last_seen_at);

-- ============================================================================
-- SITE ANALYTICS - HOURLY AGGREGATES (For trend analysis)
-- ============================================================================

CREATE TABLE IF NOT EXISTS site_analytics_hourly (
  date TEXT NOT NULL,
  hour INTEGER NOT NULL CHECK (hour >= 0 AND hour <= 23),
  visitors INTEGER DEFAULT 0,
  sessions INTEGER DEFAULT 0,
  pageviews INTEGER DEFAULT 0,
  PRIMARY KEY (date, hour)
);

CREATE INDEX IF NOT EXISTS idx_site_hourly_date ON site_analytics_hourly(date);

-- ============================================================================
-- COMBINED GEO VIEW (For dashboard - aggregates all sources)
-- ============================================================================

-- View that combines geo data from all sources
CREATE VIEW IF NOT EXISTS analytics_geo_combined AS
SELECT 
  country_code,
  SUM(visitors) as total_visitors,
  SUM(sessions) as total_sessions,
  SUM(pageviews) as total_pageviews,
  'site' as source
FROM site_analytics_geo_daily
WHERE date >= date('now', '-30 days')
GROUP BY country_code

UNION ALL

SELECT 
  country_code,
  0 as total_visitors,
  SUM(sessions) as total_sessions,
  SUM(pageviews) as total_pageviews,
  'docs' as source
FROM docs_analytics_geo_daily
WHERE date >= date('now', '-30 days')
GROUP BY country_code

UNION ALL

SELECT 
  json_extract(metadata, '$.country') as country_code,
  COUNT(DISTINCT json_extract(metadata, '$.machine_id')) as total_visitors,
  0 as total_sessions,
  0 as total_pageviews,
  'cli' as source
FROM audit_log
WHERE action = 'machine.registered' 
  AND created_at >= datetime('now', '-30 days')
  AND json_extract(metadata, '$.country') IS NOT NULL
GROUP BY json_extract(metadata, '$.country');
