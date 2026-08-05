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
