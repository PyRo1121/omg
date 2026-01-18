-- OMG SaaS Licensing Database Schema
-- Run with: wrangler d1 execute omg-licensing --file=./schema.sql

CREATE TABLE IF NOT EXISTS customers (
  id TEXT PRIMARY KEY,
  stripe_customer_id TEXT UNIQUE,
  email TEXT NOT NULL,
  company TEXT,
  tier TEXT DEFAULT 'free',
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS licenses (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  license_key TEXT UNIQUE NOT NULL,
  tier TEXT NOT NULL DEFAULT 'pro',
  status TEXT DEFAULT 'active',
  expires_at DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id)
);

CREATE TABLE IF NOT EXISTS subscriptions (
  id TEXT PRIMARY KEY,
  customer_id TEXT NOT NULL,
  stripe_subscription_id TEXT UNIQUE,
  status TEXT DEFAULT 'active',
  current_period_start DATETIME,
  current_period_end DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (customer_id) REFERENCES customers(id)
);

CREATE TABLE IF NOT EXISTS usage (
  id TEXT PRIMARY KEY,
  license_key TEXT NOT NULL,
  feature TEXT NOT NULL,
  count INTEGER DEFAULT 1,
  machine_id TEXT,
  timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS install_stats (
  id TEXT PRIMARY KEY,
  date TEXT UNIQUE NOT NULL,
  count INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS install_details (
  id TEXT PRIMARY KEY,
  timestamp TEXT NOT NULL,
  version TEXT,
  platform TEXT,
  backend TEXT
);

CREATE INDEX IF NOT EXISTS idx_licenses_key ON licenses(license_key);
CREATE INDEX IF NOT EXISTS idx_customers_stripe ON customers(stripe_customer_id);
CREATE INDEX IF NOT EXISTS idx_usage_license ON usage(license_key);
