-- Analytics schema for OMG package manager
-- Tracks downloads, installations, and usage patterns

-- Download events from the releases worker
CREATE TABLE IF NOT EXISTS downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    package_name TEXT NOT NULL,
    version TEXT NOT NULL,
    platform TEXT NOT NULL,
    architecture TEXT,
    country TEXT,
    user_agent TEXT,
    downloaded_at TEXT DEFAULT (datetime('now')),
    file_size INTEGER,
    success INTEGER DEFAULT 1
);

-- Page views for dashboard and docs
CREATE TABLE IF NOT EXISTS page_views (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    referrer TEXT,
    country TEXT,
    user_agent TEXT,
    viewed_at TEXT DEFAULT (datetime('now'))
);

-- API usage tracking
CREATE TABLE IF NOT EXISTS api_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint TEXT NOT NULL,
    method TEXT NOT NULL,
    status_code INTEGER,
    response_time_ms INTEGER,
    country TEXT,
    called_at TEXT DEFAULT (datetime('now'))
);

-- Daily aggregates for efficient querying
CREATE TABLE IF NOT EXISTS daily_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL UNIQUE,
    total_downloads INTEGER DEFAULT 0,
    unique_ips INTEGER DEFAULT 0,
    total_page_views INTEGER DEFAULT 0,
    total_api_calls INTEGER DEFAULT 0,
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_downloads_date ON downloads(downloaded_at);
CREATE INDEX IF NOT EXISTS idx_downloads_package ON downloads(package_name);
CREATE INDEX IF NOT EXISTS idx_page_views_date ON page_views(viewed_at);
CREATE INDEX IF NOT EXISTS idx_api_calls_date ON api_calls(called_at);
CREATE INDEX IF NOT EXISTS idx_daily_stats_date ON daily_stats(date);
