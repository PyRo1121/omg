-- Install tracking tables for telemetry

-- Daily install statistics
CREATE TABLE IF NOT EXISTS install_stats (
  id TEXT PRIMARY KEY,
  date DATE NOT NULL,
  count INTEGER NOT NULL DEFAULT 0,
  UNIQUE(date)
);

-- Detailed install records
CREATE TABLE IF NOT EXISTS install_details (
  id TEXT PRIMARY KEY,
  timestamp DATETIME NOT NULL,
  version TEXT,
  platform TEXT,
  backend TEXT,
  opt_out BOOLEAN DEFAULT FALSE
);

-- Index for faster queries
CREATE INDEX IF NOT EXISTS idx_install_details_timestamp ON install_details(timestamp);
CREATE INDEX IF NOT EXISTS idx_install_stats_date ON install_stats(date);
