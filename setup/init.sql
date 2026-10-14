-- Create target schema
CREATE SCHEMA IF NOT EXISTS iceberg.demo
WITH (location = 's3://warehouse/demo');

-- 1. Month-partitioned orders table
CREATE TABLE IF NOT EXISTS iceberg.demo.orders_by_month
WITH (
  format = 'PARQUET',
  partitioning = ARRAY['month(orderdate)']
) AS
SELECT * FROM tpch.sf1.orders;

-- 2. Multi-column partitioned lineitem table (Year + Ship mode)
CREATE TABLE IF NOT EXISTS iceberg.demo.lineitem_partitioned
WITH (
  format = 'PARQUET',
  partitioning = ARRAY['year(shipdate)', 'shipmode']
) AS
SELECT * FROM tpch.sf1.lineitem;

-- 3. Events table with MAP and nested JSON columns (for testing nested flattening)
CREATE TABLE IF NOT EXISTS iceberg.demo.events
WITH (
  format = 'PARQUET'
) AS
SELECT * FROM (
  VALUES
    ('ev-101', 1001, MAP(ARRAY['env', 'region', 'tier'], ARRAY['prod', 'us-east-1', 'gold']), '{"source": "mobile", "device": {"os": "iOS", "version": "17.4"}, "action": "login"}', TIMESTAMP '2026-10-14 09:00:00.000000'),
    ('ev-102', 1002, MAP(ARRAY['env', 'region', 'tier'], ARRAY['staging', 'eu-west-1', 'silver']), '{"source": "web", "device": {"os": "Linux", "version": "6.8"}, "action": "checkout"}', TIMESTAMP '2026-10-14 09:15:00.000000'),
    ('ev-103', 1003, MAP(ARRAY['env', 'region'], ARRAY['prod', 'ap-south-1']), '{"source": "api", "device": {"os": "Android", "version": "14"}, "action": "refresh"}', TIMESTAMP '2026-10-14 09:30:00.000000'),
    ('ev-104', 1004, MAP(ARRAY['env', 'region', 'tier'], ARRAY['dev', 'us-west-2', 'bronze']), '{"source": "mobile", "device": {"os": "iOS", "version": "16.5"}, "action": "logout"}', TIMESTAMP '2026-10-14 09:45:00.000000')
) AS t (event_id, user_id, attributes, payload, created_at);


