"""
lazyTrino seed script – populates Trino with realistic bulk data.

Catalogs / schemas / tables seeded
───────────────────────────────────
iceberg.tpch
    orders          ~2 000 rows   partitioned by order_year, region
    lineitem        ~8 000 rows   partitioned by ship_year
    customers       ~500  rows
    suppliers       ~200  rows
    nations         25    rows    (static reference table)

iceberg.analytics
    web_events      ~3 000 rows   partitioned by event_date (month bucket)
    page_views      ~4 000 rows   partitioned by view_date
    sessions        ~1 000 rows

iceberg.ecom
    products        ~300  rows    partitioned by category
    orders          ~2 500 rows   partitioned by region, order_year
    order_items     ~7 500 rows
    reviews         ~1 500 rows   partitioned by rating

memory.dev
    customers       50    rows
    employees       100   rows
    departments     10    rows
"""

import random
import string
import time
from datetime import date, timedelta, datetime
import trino

HOST = "trino"
PORT = 8080
USER = "trino"

REGIONS = ["NORTH_AMERICA", "EUROPE", "ASIA", "SOUTH_AMERICA", "AFRICA", "OCEANIA"]
STATUSES = ["PENDING", "PROCESSING", "SHIPPED", "DELIVERED", "CANCELLED", "RETURNED"]
PRIORITIES = ["LOW", "MEDIUM", "HIGH", "URGENT", "CRITICAL"]
SEGMENTS = ["BUILDING", "AUTOMOBILE", "MACHINERY", "HOUSEHOLD", "FURNITURE"]
CATEGORIES = ["Electronics", "Clothing", "Books", "Sports", "Home", "Garden", "Toys", "Food", "Beauty", "Automotive"]
BROWSERS = ["Chrome", "Firefox", "Safari", "Edge", "Opera"]
OS_LIST = ["Windows", "macOS", "Linux", "iOS", "Android"]
EVENT_TYPES = ["page_view", "click", "purchase", "add_to_cart", "checkout", "search", "login", "logout", "signup", "share"]

random.seed(42)


def rnd_str(n=8):
    return "".join(random.choices(string.ascii_lowercase, k=n))


def rnd_date(start="2023-01-01", end="2026-07-31"):
    s = date.fromisoformat(start)
    e = date.fromisoformat(end)
    return s + timedelta(days=random.randint(0, (e - s).days))


def rnd_ts(start="2023-01-01", end="2026-07-31"):
    d = rnd_date(start, end)
    h = random.randint(0, 23)
    m = random.randint(0, 59)
    s = random.randint(0, 59)
    return f"{d} {h:02d}:{m:02d}:{s:02d}"


def connect():
    conn = trino.dbapi.connect(host=HOST, port=PORT, user=USER, request_timeout=120)
    return conn.cursor()


def run(cur, sql, quiet=False):
    if not quiet:
        preview = sql[:120].replace("\n", " ")
        print(f"  SQL: {preview}{'…' if len(sql) > 120 else ''}")
    cur.execute(sql)
    try:
        cur.fetchall()
    except Exception:
        pass


def batch_insert(cur, table, columns, rows, batch=200):
    col_str = ", ".join(columns)
    for i in range(0, len(rows), batch):
        chunk = rows[i : i + batch]
        val_strs = []
        for row in chunk:
            parts = []
            for v in row:
                if v is None:
                    parts.append("NULL")
                elif isinstance(v, bool):
                    parts.append("true" if v else "false")
                elif isinstance(v, (int, float)):
                    parts.append(str(v))
                elif isinstance(v, date) and not isinstance(v, datetime):
                    parts.append(f"DATE '{v}'")
                else:
                    escaped = str(v).replace("'", "''")
                    parts.append(f"'{escaped}'")
            val_strs.append(f"({', '.join(parts)})")
        sql = f"INSERT INTO {table} ({col_str}) VALUES {', '.join(val_strs)}"
        run(cur, sql, quiet=True)
    print(f"    ✓ {len(rows)} rows → {table}")


# ─────────────────────────────────────────────────────────────
# NATIONS (reference)
# ─────────────────────────────────────────────────────────────
NATIONS_DATA = [
    (0, "ALGERIA", 0, "haggle. carefully final deposits detect slyly agai"),
    (1, "ARGENTINA", 1, "al foxes promise slyly according to the regular accounts"),
    (2, "BRAZIL", 1, "y alongside of the pending deposits"),
    (3, "CANADA", 1, "eas hang ironic, silent packages"),
    (4, "EGYPT", 4, "y above the carefully unusual theodolites"),
    (5, "ETHIOPIA", 0, "ven packages wake quickly. regu"),
    (6, "FRANCE", 3, "refully final requests"),
    (7, "GERMANY", 3, "l platelets. regular accounts x-ray"),
    (8, "INDIA", 2, "ss excuses cajole slyly across the packages"),
    (9, "INDONESIA", 2, "slyly express asymptotes"),
    (10, "IRAN", 4, "efully alongside of the slyly final dependencies"),
    (11, "IRAQ", 4, "nic deposits boost atop the quickly final requests"),
    (12, "JAPAN", 2, "ously. final, express gifts cajole a"),
    (13, "JORDAN", 4, "ic deposits are blithely about the carefully ironic hockey players"),
    (14, "KENYA", 0, " pending excuses haggle furiously deposits"),
    (15, "MOROCCO", 0, "rns. blithely bold courts among the closely regular packages use furiously bold platelets"),
    (16, "MOZAMBIQUE", 0, "s. ironic, unusual asymptotes wake blithely r"),
    (17, "PERU", 1, "platelets. blithely pending dependencies use fluffily across the even pinto beans"),
    (18, "CHINA", 2, "c dependencies. furiously express notornis sleep slyly regular accounts"),
    (19, "ROMANIA", 3, "ular asymptotes are about the furious multipliers"),
    (20, "SAUDI ARABIA", 4, "ts. silent requests haggle"),
    (21, "VIETNAM", 2, "hely enticingly express accounts"),
    (22, "RUSSIA", 3, " requests against the platelets use never according to the quickly regular pint"),
    (23, "UNITED KINGDOM", 3, "eans boost carefully special requests"),
    (24, "UNITED STATES", 1, "y final packages. slow foxes cajole quickly"),
]


def seed_iceberg_tpch(cur):
    print("\n── iceberg.tpch ──")

    run(cur, "CREATE SCHEMA IF NOT EXISTS iceberg.tpch")

    # nations
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.tpch.nations (
            nation_key  BIGINT,
            name        VARCHAR,
            region_key  INT,
            comment     VARCHAR
        )
    """)
    run(cur, "DELETE FROM iceberg.tpch.nations")
    batch_insert(cur, "iceberg.tpch.nations",
                 ["nation_key", "name", "region_key", "comment"],
                 NATIONS_DATA)

    # suppliers
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.tpch.suppliers (
            supplier_key    BIGINT,
            name            VARCHAR,
            address         VARCHAR,
            nation_key      BIGINT,
            phone           VARCHAR,
            acct_bal        DOUBLE,
            comment         VARCHAR
        )
    """)
    run(cur, "DELETE FROM iceberg.tpch.suppliers")
    suppliers = []
    for i in range(1, 201):
        nk = random.randint(0, 24)
        phone = f"{random.randint(10,99)}-{random.randint(100,999)}-{random.randint(1000,9999)}-{random.randint(1000,9999)}"
        suppliers.append((
            i, f"Supplier#{i:09d}", f"{rnd_str(10)} {rnd_str(6)}, {rnd_str(5)} {random.randint(1,99999)}",
            nk, phone, round(random.uniform(-1000, 9999), 2),
            rnd_str(40)
        ))
    batch_insert(cur, "iceberg.tpch.suppliers",
                 ["supplier_key", "name", "address", "nation_key", "phone", "acct_bal", "comment"],
                 suppliers)

    # customers
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.tpch.customers (
            customer_key    BIGINT,
            name            VARCHAR,
            address         VARCHAR,
            nation_key      BIGINT,
            phone           VARCHAR,
            acct_bal        DOUBLE,
            market_segment  VARCHAR,
            comment         VARCHAR
        )
    """)
    run(cur, "DELETE FROM iceberg.tpch.customers")
    customers = []
    for i in range(1, 501):
        nk = random.randint(0, 24)
        phone = f"{random.randint(10,99)}-{random.randint(100,999)}-{random.randint(1000,9999)}-{random.randint(1000,9999)}"
        customers.append((
            i, f"Customer#{i:09d}", f"{rnd_str(12)} {random.randint(1,99999)}",
            nk, phone, round(random.uniform(-999, 9999), 2),
            random.choice(SEGMENTS), rnd_str(50)
        ))
    batch_insert(cur, "iceberg.tpch.customers",
                 ["customer_key", "name", "address", "nation_key", "phone", "acct_bal", "market_segment", "comment"],
                 customers)

    # orders  (partitioned by region, order_year)
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.tpch.orders (
            order_key       BIGINT,
            customer_key    BIGINT,
            status          VARCHAR,
            total_price     DOUBLE,
            order_date      DATE,
            order_priority  VARCHAR,
            clerk           VARCHAR,
            ship_priority   INT,
            comment         VARCHAR,
            region          VARCHAR,
            order_year      INT
        )
        WITH (
            partitioning = ARRAY['region', 'order_year']
        )
    """)
    run(cur, "DELETE FROM iceberg.tpch.orders")
    orders = []
    for i in range(1, 2001):
        od = rnd_date("2023-01-01", "2026-06-30")
        region = random.choice(REGIONS)
        orders.append((
            i, random.randint(1, 500),
            random.choice(["F", "O", "P"]),
            round(random.uniform(1000, 500000), 2),
            od, random.choice(PRIORITIES),
            f"Clerk#{random.randint(1,1000):09d}",
            random.randint(0, 2), rnd_str(44),
            region, od.year
        ))
    batch_insert(cur, "iceberg.tpch.orders",
                 ["order_key", "customer_key", "status", "total_price", "order_date",
                  "order_priority", "clerk", "ship_priority", "comment", "region", "order_year"],
                 orders)

    # lineitem  (partitioned by ship_year)
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.tpch.lineitem (
            order_key       BIGINT,
            part_key        BIGINT,
            supplier_key    BIGINT,
            line_number     INT,
            quantity        DOUBLE,
            extended_price  DOUBLE,
            discount        DOUBLE,
            tax             DOUBLE,
            return_flag     VARCHAR,
            line_status     VARCHAR,
            ship_date       DATE,
            commit_date     DATE,
            receipt_date    DATE,
            ship_instruct   VARCHAR,
            ship_mode       VARCHAR,
            comment         VARCHAR,
            ship_year       INT
        )
        WITH (
            partitioning = ARRAY['ship_year']
        )
    """)
    run(cur, "DELETE FROM iceberg.tpch.lineitem")
    ship_modes = ["AIR", "SHIP", "RAIL", "TRUCK", "MAIL", "REG AIR", "FOB"]
    ship_instrcts = ["DELIVER IN PERSON", "COLLECT COD", "NONE", "TAKE BACK RETURN"]
    lineitem = []
    for i in range(1, 8001):
        sd = rnd_date("2023-01-01", "2026-07-15")
        lineitem.append((
            random.randint(1, 2000),
            random.randint(1, 2000),
            random.randint(1, 200),
            random.randint(1, 7),
            round(random.uniform(1, 50), 2),
            round(random.uniform(900, 104949), 2),
            round(random.uniform(0, 0.1), 2),
            round(random.uniform(0, 0.08), 2),
            random.choice(["A", "N", "R"]),
            random.choice(["O", "F"]),
            sd,
            sd + timedelta(days=random.randint(10, 30)),
            sd + timedelta(days=random.randint(31, 60)),
            random.choice(ship_instrcts),
            random.choice(ship_modes),
            rnd_str(44),
            sd.year,
        ))
    batch_insert(cur, "iceberg.tpch.lineitem",
                 ["order_key", "part_key", "supplier_key", "line_number",
                  "quantity", "extended_price", "discount", "tax",
                  "return_flag", "line_status", "ship_date", "commit_date",
                  "receipt_date", "ship_instruct", "ship_mode", "comment", "ship_year"],
                 lineitem)


# ─────────────────────────────────────────────────────────────
# iceberg.analytics
# ─────────────────────────────────────────────────────────────
def seed_iceberg_analytics(cur):
    print("\n── iceberg.analytics ──")

    run(cur, "CREATE SCHEMA IF NOT EXISTS iceberg.analytics")

    # sessions
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.analytics.sessions (
            session_id      VARCHAR,
            user_id         BIGINT,
            started_at      TIMESTAMP(6),
            ended_at        TIMESTAMP(6),
            device_type     VARCHAR,
            os              VARCHAR,
            browser         VARCHAR,
            country         VARCHAR,
            referrer        VARCHAR,
            page_count      INT,
            duration_secs   INT,
            is_bounce       BOOLEAN
        )
    """)
    run(cur, "DELETE FROM iceberg.analytics.sessions")
    countries = ["US", "GB", "IN", "DE", "FR", "BR", "JP", "CA", "AU", "SG"]
    referrers = ["google.com", "bing.com", "direct", "twitter.com", "linkedin.com", "facebook.com", "email", "reddit.com"]
    sessions = []
    for i in range(1, 1001):
        ts = rnd_ts("2024-01-01", "2026-07-31")
        dur = random.randint(5, 3600)
        sessions.append((
            f"sess_{i:06d}",
            random.randint(1001, 9999),
            ts,
            ts,  # simplified – same for ended_at
            random.choice(["desktop", "mobile", "tablet"]),
            random.choice(OS_LIST),
            random.choice(BROWSERS),
            random.choice(countries),
            random.choice(referrers),
            random.randint(1, 30),
            dur,
            dur < 30,
        ))
    batch_insert(cur, "iceberg.analytics.sessions",
                 ["session_id", "user_id", "started_at", "ended_at", "device_type",
                  "os", "browser", "country", "referrer", "page_count", "duration_secs", "is_bounce"],
                 sessions)

    # web_events  (partitioned by event_date)
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.analytics.web_events (
            event_id        VARCHAR,
            session_id      VARCHAR,
            user_id         BIGINT,
            event_type      VARCHAR,
            event_date      DATE,
            event_ts        TIMESTAMP(6),
            page_url        VARCHAR,
            referrer        VARCHAR,
            os              VARCHAR,
            browser         VARCHAR,
            device_type     VARCHAR,
            country         VARCHAR,
            ip_address      VARCHAR,
            duration_ms     INT,
            is_bot          BOOLEAN
        )
        WITH (
            partitioning = ARRAY['event_date']
        )
    """)
    run(cur, "DELETE FROM iceberg.analytics.web_events")
    pages = ["/home", "/products", "/checkout", "/cart", "/about", "/blog", "/contact",
             "/search", "/account", "/wishlist", "/deals", "/new-arrivals"]
    web_events = []
    for i in range(1, 3001):
        ed = rnd_date("2024-01-01", "2026-07-31")
        web_events.append((
            f"evt_{i:07d}",
            f"sess_{random.randint(1,1000):06d}",
            random.randint(1001, 9999),
            random.choice(EVENT_TYPES),
            ed,
            f"{ed} {random.randint(0,23):02d}:{random.randint(0,59):02d}:{random.randint(0,59):02d}.000000",
            random.choice(pages),
            random.choice(referrers),
            random.choice(OS_LIST),
            random.choice(BROWSERS),
            random.choice(["desktop", "mobile", "tablet"]),
            random.choice(countries),
            f"{random.randint(1,254)}.{random.randint(0,255)}.{random.randint(0,255)}.{random.randint(1,254)}",
            random.randint(50, 15000),
            random.random() < 0.03,
        ))
    batch_insert(cur, "iceberg.analytics.web_events",
                 ["event_id", "session_id", "user_id", "event_type", "event_date", "event_ts",
                  "page_url", "referrer", "os", "browser", "device_type", "country",
                  "ip_address", "duration_ms", "is_bot"],
                 web_events)

    # page_views  (partitioned by view_date)
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.analytics.page_views (
            view_id         VARCHAR,
            user_id         BIGINT,
            session_id      VARCHAR,
            page_url        VARCHAR,
            view_date       DATE,
            viewed_at       TIMESTAMP(6),
            time_on_page_s  INT,
            scroll_depth    INT,
            cta_clicked     BOOLEAN,
            utm_source      VARCHAR,
            utm_medium      VARCHAR,
            utm_campaign    VARCHAR
        )
        WITH (
            partitioning = ARRAY['view_date']
        )
    """)
    run(cur, "DELETE FROM iceberg.analytics.page_views")
    utms = ["google", "bing", "email", "social", "direct", "affiliate"]
    mediums = ["cpc", "organic", "email", "referral", "social", "none"]
    campaigns = ["summer_sale", "black_friday", "new_year", "back_to_school", "brand", "retargeting", "none"]
    page_views = []
    for i in range(1, 4001):
        vd = rnd_date("2024-01-01", "2026-07-31")
        page_views.append((
            f"pv_{i:07d}",
            random.randint(1001, 9999),
            f"sess_{random.randint(1,1000):06d}",
            random.choice(pages),
            vd,
            f"{vd} {random.randint(0,23):02d}:{random.randint(0,59):02d}:{random.randint(0,59):02d}.000000",
            random.randint(5, 600),
            random.randint(0, 100),
            random.random() < 0.15,
            random.choice(utms),
            random.choice(mediums),
            random.choice(campaigns),
        ))
    batch_insert(cur, "iceberg.analytics.page_views",
                 ["view_id", "user_id", "session_id", "page_url", "view_date", "viewed_at",
                  "time_on_page_s", "scroll_depth", "cta_clicked",
                  "utm_source", "utm_medium", "utm_campaign"],
                 page_views)


# ─────────────────────────────────────────────────────────────
# iceberg.ecom
# ─────────────────────────────────────────────────────────────
def seed_iceberg_ecom(cur):
    print("\n── iceberg.ecom ──")

    run(cur, "CREATE SCHEMA IF NOT EXISTS iceberg.ecom")

    # products  (partitioned by category)
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.ecom.products (
            product_id      BIGINT,
            sku             VARCHAR,
            name            VARCHAR,
            category        VARCHAR,
            brand           VARCHAR,
            price           DOUBLE,
            cost            DOUBLE,
            weight_kg       DOUBLE,
            in_stock        BOOLEAN,
            stock_qty       INT,
            rating          DOUBLE,
            review_count    INT,
            created_date    DATE
        )
        WITH (
            partitioning = ARRAY['category']
        )
    """)
    run(cur, "DELETE FROM iceberg.ecom.products")
    brands = ["Apex", "Zenith", "Nova", "Orion", "Pulse", "Vertex", "Echo", "Flux", "Bolt", "Arch"]
    products = []
    for i in range(1, 301):
        cat = random.choice(CATEGORIES)
        price = round(random.uniform(5, 2999), 2)
        products.append((
            i,
            f"SKU-{i:06d}",
            f"{random.choice(brands)} {cat} {rnd_str(4).upper()}",
            cat,
            random.choice(brands),
            price,
            round(price * random.uniform(0.3, 0.7), 2),
            round(random.uniform(0.1, 25), 2),
            random.random() < 0.85,
            random.randint(0, 500),
            round(random.uniform(1, 5), 1),
            random.randint(0, 2000),
            rnd_date("2020-01-01", "2026-01-01"),
        ))
    batch_insert(cur, "iceberg.ecom.products",
                 ["product_id", "sku", "name", "category", "brand", "price", "cost",
                  "weight_kg", "in_stock", "stock_qty", "rating", "review_count", "created_date"],
                 products)

    # orders  (partitioned by region, order_year)
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.ecom.orders (
            order_id        BIGINT,
            customer_id     BIGINT,
            status          VARCHAR,
            region          VARCHAR,
            order_date      DATE,
            shipped_date    DATE,
            delivered_date  DATE,
            subtotal        DOUBLE,
            discount        DOUBLE,
            shipping_cost   DOUBLE,
            total           DOUBLE,
            payment_method  VARCHAR,
            order_year      INT
        )
        WITH (
            partitioning = ARRAY['region', 'order_year']
        )
    """)
    run(cur, "DELETE FROM iceberg.ecom.orders")
    payment_methods = ["CREDIT_CARD", "DEBIT_CARD", "PAYPAL", "CRYPTO", "BANK_TRANSFER", "GIFT_CARD"]
    ecom_orders = []
    for i in range(1, 2501):
        od = rnd_date("2023-01-01", "2026-07-15")
        sd = od + timedelta(days=random.randint(1, 5))
        dd = sd + timedelta(days=random.randint(1, 14))
        sub = round(random.uniform(10, 5000), 2)
        disc = round(sub * random.uniform(0, 0.3), 2)
        ship = round(random.uniform(0, 50), 2)
        region = random.choice(REGIONS)
        ecom_orders.append((
            i, random.randint(1001, 5000),
            random.choice(STATUSES),
            region, od, sd, dd,
            sub, disc, ship,
            round(sub - disc + ship, 2),
            random.choice(payment_methods),
            od.year,
        ))
    batch_insert(cur, "iceberg.ecom.orders",
                 ["order_id", "customer_id", "status", "region", "order_date", "shipped_date",
                  "delivered_date", "subtotal", "discount", "shipping_cost", "total",
                  "payment_method", "order_year"],
                 ecom_orders)

    # order_items
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.ecom.order_items (
            item_id         BIGINT,
            order_id        BIGINT,
            product_id      BIGINT,
            quantity        INT,
            unit_price      DOUBLE,
            line_total      DOUBLE
        )
    """)
    run(cur, "DELETE FROM iceberg.ecom.order_items")
    order_items = []
    item_id = 1
    for order_id in range(1, 2501):
        for _ in range(random.randint(1, 5)):
            qty = random.randint(1, 10)
            price = round(random.uniform(5, 999), 2)
            order_items.append((item_id, order_id, random.randint(1, 300), qty, price, round(qty * price, 2)))
            item_id += 1
            if item_id > 7501:
                break
        if item_id > 7501:
            break
    batch_insert(cur, "iceberg.ecom.order_items",
                 ["item_id", "order_id", "product_id", "quantity", "unit_price", "line_total"],
                 order_items)

    # reviews  (partitioned by rating)
    run(cur, """
        CREATE TABLE IF NOT EXISTS iceberg.ecom.reviews (
            review_id       BIGINT,
            product_id      BIGINT,
            customer_id     BIGINT,
            rating          INT,
            title           VARCHAR,
            body            VARCHAR,
            verified        BOOLEAN,
            helpful_votes   INT,
            review_date     DATE
        )
        WITH (
            partitioning = ARRAY['rating']
        )
    """)
    run(cur, "DELETE FROM iceberg.ecom.reviews")
    review_titles = [
        "Great product!", "Disappointing", "Exactly as described", "Would not buy again",
        "Excellent value", "Poor quality", "Highly recommend", "Average", "Game changer",
        "Not worth it", "Exceeded expectations", "Just ok", "Love it!", "Meh",
    ]
    reviews = []
    for i in range(1, 1501):
        reviews.append((
            i, random.randint(1, 300), random.randint(1001, 5000),
            random.randint(1, 5),
            random.choice(review_titles),
            rnd_str(80),
            random.random() < 0.7,
            random.randint(0, 500),
            rnd_date("2023-01-01", "2026-07-31"),
        ))
    batch_insert(cur, "iceberg.ecom.reviews",
                 ["review_id", "product_id", "customer_id", "rating", "title", "body",
                  "verified", "helpful_votes", "review_date"],
                 reviews)


# ─────────────────────────────────────────────────────────────
# memory.dev
# ─────────────────────────────────────────────────────────────
def seed_memory_dev(cur):
    print("\n── memory.dev ──")

    run(cur, "CREATE SCHEMA IF NOT EXISTS memory.dev")

    run(cur, """
        CREATE TABLE IF NOT EXISTS memory.dev.departments (
            dept_id     BIGINT,
            name        VARCHAR,
            manager_id  BIGINT,
            budget      DOUBLE,
            location    VARCHAR
        )
    """)
    run(cur, "DELETE FROM memory.dev.departments")
    dept_names = ["Engineering", "Marketing", "Sales", "Finance", "HR",
                  "Legal", "Product", "Design", "Data", "Operations"]
    depts = []
    for i, name in enumerate(dept_names, 1):
        depts.append((i, name, random.randint(1, 100), round(random.uniform(100000, 5000000), 2),
                      random.choice(["New York", "London", "Berlin", "Singapore", "Toronto"])))
    batch_insert(cur, "memory.dev.departments",
                 ["dept_id", "name", "manager_id", "budget", "location"],
                 depts)

    run(cur, """
        CREATE TABLE IF NOT EXISTS memory.dev.employees (
            emp_id          BIGINT,
            first_name      VARCHAR,
            last_name       VARCHAR,
            email           VARCHAR,
            dept_id         BIGINT,
            job_title       VARCHAR,
            salary          DOUBLE,
            hire_date       DATE,
            is_active       BOOLEAN,
            manager_id      BIGINT
        )
    """)
    run(cur, "DELETE FROM memory.dev.employees")
    first_names = ["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Hank",
                   "Iris", "Jack", "Karen", "Leo", "Mia", "Noah", "Olivia", "Pete",
                   "Quinn", "Rose", "Sam", "Tina", "Uma", "Victor", "Wendy", "Xander", "Yara"]
    last_names = ["Smith", "Jones", "Williams", "Brown", "Davis", "Miller", "Wilson",
                  "Moore", "Taylor", "Anderson", "Thomas", "Jackson", "White", "Harris",
                  "Martin", "Thompson", "Garcia", "Martinez", "Robinson", "Clark"]
    titles = ["Engineer", "Senior Engineer", "Staff Engineer", "Manager", "Director",
              "Analyst", "Senior Analyst", "Coordinator", "Specialist", "Associate"]
    employees = []
    for i in range(1, 101):
        fn = random.choice(first_names)
        ln = random.choice(last_names)
        did = random.randint(1, 10)
        employees.append((
            i, fn, ln, f"{fn.lower()}.{ln.lower()}{i}@company.com",
            did, random.choice(titles),
            round(random.uniform(40000, 250000), 2),
            rnd_date("2015-01-01", "2026-01-01"),
            random.random() < 0.9,
            random.randint(1, 100) if i > 5 else None,
        ))
    batch_insert(cur, "memory.dev.employees",
                 ["emp_id", "first_name", "last_name", "email", "dept_id", "job_title",
                  "salary", "hire_date", "is_active", "manager_id"],
                 employees)

    run(cur, """
        CREATE TABLE IF NOT EXISTS memory.dev.customers (
            customer_id     BIGINT,
            full_name       VARCHAR,
            email           VARCHAR,
            phone           VARCHAR,
            country         VARCHAR,
            tier            VARCHAR,
            account_balance DOUBLE,
            is_active       BOOLEAN,
            created_at      TIMESTAMP(6)
        )
    """)
    run(cur, "DELETE FROM memory.dev.customers")
    tiers = ["BRONZE", "SILVER", "GOLD", "PLATINUM", "DIAMOND"]
    countries = ["US", "UK", "India", "Germany", "France", "Brazil", "Japan", "Canada", "Australia"]
    mem_customers = []
    for i in range(1001, 1051):
        fn = random.choice(first_names)
        ln = random.choice(last_names)
        ts = rnd_ts("2020-01-01", "2026-07-01")
        mem_customers.append((
            i, f"{fn} {ln}", f"{fn.lower()}.{ln.lower()}{i}@email.com",
            f"+1-{random.randint(200,999)}-{random.randint(100,999)}-{random.randint(1000,9999)}",
            random.choice(countries),
            random.choice(tiers),
            round(random.uniform(0, 50000), 2),
            random.random() < 0.85,
            ts,
        ))
    batch_insert(cur, "memory.dev.customers",
                 ["customer_id", "full_name", "email", "phone", "country", "tier",
                  "account_balance", "is_active", "created_at"],
                 mem_customers)


def wait_for_trino(max_wait=300):
    print("Waiting for Trino to be ready…")
    start = time.time()
    while time.time() - start < max_wait:
        try:
            cur = connect()
            cur.execute("SELECT 1")
            cur.fetchall()
            print("  ✓ Trino is ready")
            return True
        except Exception as e:
            print(f"  … not ready yet ({e}), retrying in 5s")
            time.sleep(5)
    raise RuntimeError("Trino did not become ready in time")


if __name__ == "__main__":
    wait_for_trino()
    cur = connect()

    seed_iceberg_tpch(cur)
    seed_iceberg_analytics(cur)
    seed_iceberg_ecom(cur)
    seed_memory_dev(cur)

    print("\n✅ All seed data loaded successfully!")
