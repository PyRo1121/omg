-- Add admin column to customers table
-- Run with: bunx wrangler d1 execute omg-licensing --remote --file=./migrations/009_add_admin_column.sql

ALTER TABLE customers ADD COLUMN admin INTEGER DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_customers_admin ON customers(admin);
