CREATE SCHEMA IF NOT EXISTS iceberg.analytics;
CREATE SCHEMA IF NOT EXISTS iceberg.ecom;

CREATE TABLE IF NOT EXISTS iceberg.analytics.web_events (
    event_id VARCHAR,
    user_id BIGINT,
    event_type VARCHAR,
    event_date DATE,
    tags ARRAY(VARCHAR),
    properties MAP(VARCHAR, VARCHAR),
    device_info ROW(
        os VARCHAR,
        browser VARCHAR,
        ip VARCHAR
    )
)
WITH (
    partitioning = ARRAY['event_date']
);

INSERT INTO iceberg.analytics.web_events VALUES
  ('evt_101', 5001, 'page_view', DATE '2026-08-01',
   ARRAY['marketing', 'promo'],
   MAP(ARRAY['referrer', 'campaign'], ARRAY['google.com', 'summer_sale']),
   CAST(ROW('iOS', 'Safari', '192.168.1.1') AS ROW(os VARCHAR, browser VARCHAR, ip VARCHAR))),

  ('evt_102', 5002, 'click_button', DATE '2026-08-01',
   ARRAY['product_page'],
   MAP(ARRAY['button_id'], ARRAY['btn_checkout']),
   CAST(ROW('Windows', 'Chrome', '10.0.0.45') AS ROW(os VARCHAR, browser VARCHAR, ip VARCHAR))),

  ('evt_103', 5001, 'purchase', DATE '2026-08-02',
   ARRAY['checkout', 'conversion'],
   MAP(ARRAY['discount_code', 'currency'], ARRAY['WELCOME10', 'USD']),
   CAST(ROW('iOS', 'Safari', '192.168.1.1') AS ROW(os VARCHAR, browser VARCHAR, ip VARCHAR)));

CREATE TABLE IF NOT EXISTS iceberg.ecom.orders (
    order_id BIGINT,
    customer_id BIGINT,
    region VARCHAR,
    order_year INT,
    total_amount DOUBLE,
    status VARCHAR,
    items ARRAY(
        ROW(
            product_id BIGINT,
            quantity INT,
            unit_price DOUBLE
        )
    )
)
WITH (
    partitioning = ARRAY['region', 'order_year']
);

INSERT INTO iceberg.ecom.orders VALUES
  (9001, 101, 'NORTH_AMERICA', 2025, 249.98, 'COMPLETED',
   ARRAY[
     CAST(ROW(101, 2, 49.99) AS ROW(product_id BIGINT, quantity INT, unit_price DOUBLE)),
     CAST(ROW(102, 1, 149.99) AS ROW(product_id BIGINT, quantity INT, unit_price DOUBLE))
   ]),

  (9002, 102, 'EUROPE', 2026, 599.00, 'PROCESSING',
   ARRAY[
     CAST(ROW(103, 1, 599.00) AS ROW(product_id BIGINT, quantity INT, unit_price DOUBLE))
   ]),

  (9003, 103, 'NORTH_AMERICA', 2026, 89.50, 'SHIPPED',
   ARRAY[
     CAST(ROW(104, 1, 89.50) AS ROW(product_id BIGINT, quantity INT, unit_price DOUBLE))
   ]);

CREATE SCHEMA IF NOT EXISTS memory.dev;

CREATE TABLE IF NOT EXISTS memory.dev.customers (
    custmoer_id BIGINT,
    full_name VARCHAR,
    email VARCHAR,
    account_balance DOUBLE,
    is_active BOOLEAN,
    cretaed_at TIMESTAMP
);

INSERT INTO memory.dev.customers VALUES
  (101, 'Alice Smith', 'alice@example.com', 1250.50, true, TIMESTAMP '2025-01-10 10:00:00'),
  (102, 'Bob Jones', 'bob@example.com', 0.00, false, TIMESTAMP '2025-03-15 14:20:00'),
  (103, 'Charlie Brown', 'charlie@example.com', 450.75, true, TIMESTAMP '2026-02-01 09:11:00');
