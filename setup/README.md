# lazyTrino — Local Test Environment

This directory contains a fully self-contained **Trino + Iceberg + MinIO** stack for developing and testing lazyTrino locally. No cloud account or external database required.

---

## Architecture

```
  lazyTrino (your binary)
         │
         │  HTTP :8080
         ▼
  ┌─────────────┐        ┌───────────────────────┐
  │    Trino    │───────▶│  Iceberg REST Catalog │
  │  (port 8080)│        │     (port 8181)       │
  └─────────────┘        └──────────┬────────────┘
                                    │ S3 API
                                    ▼
                         ┌───────────────────────┐
                         │        MinIO          │
                         │  S3-compatible store  │
                         │  API  :9000           │
                         │  Console :9001        │
                         └───────────────────────┘
```

| Service | Image | Role |
|---|---|---|
| `trino` | `trinodb/trino:latest` | Query engine — the main entry point |
| `iceberg-rest` | `apache/iceberg-rest-fixture` | In-memory Iceberg REST catalog (full REST spec) |
| `minio` | `minio/minio` | S3-compatible object store — holds Parquet data files |
| `minio-init` | `minio/mc` | One-shot: creates the `warehouse` bucket on first boot |
| `init-db` | `trinodb/trino:latest` | One-shot: creates schemas and seeds tables from `init.sql` |

> **Note:** The Iceberg catalog is in-memory. Table metadata resets if `iceberg-rest` is restarted. Run `docker compose up init-db` again to reseed.

---

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) with Docker Compose v2 (`docker compose` — not the legacy `docker-compose`)

---

## Directory Structure

```
setup/
├── docker-compose.yml          # Full stack definition
├── catalog/
│   └── iceberg.properties      # Iceberg connector config (mounted into Trino)
├── init.sql                    # Schema + table seed script
└── README.md                   # This file
```

---

## Quickstart

### 1. Start the stack

```bash
cd setup/
docker compose up -d minio minio-init iceberg-rest trino
```

This starts all services in the background. Trino takes ~15–20 seconds to become healthy.

### 2. Wait for Trino to be ready

```bash
docker compose ps
```

Wait until `trino` shows `(healthy)` in the STATUS column. Or watch it:

```bash
watch -n2 'docker compose ps'
```

### 3. Seed the database

```bash
docker compose up init-db
```

This runs `init.sql` against the live Trino instance and exits. It creates:

| Table | Rows | Partition strategy |
|---|---|---|
| `iceberg.demo.orders_by_month` | 1,500,000 | `month(orderdate)` |
| `iceberg.demo.lineitem_partitioned` | 6,001,215 | `year(shipdate)` + `shipmode` |

You'll see output like:

```
init-db-1  | CREATE SCHEMA
init-db-1  | CREATE TABLE: 1500000 rows
init-db-1  | CREATE TABLE: 6001215 rows
init-db-1 exited with code 0
```

### 4. Connect lazyTrino

```bash
# From the project root:
./lazyTrino --url http://localhost:8080 --user admin
```

Or just `./lazyTrino` — the default URL is already `http://localhost:8080`.

**Credentials:**

| Field | Value |
|---|---|
| URL | `http://localhost:8080` |
| User | `admin` (or any string — no auth enforced) |
| Password | *(leave blank)* |

**Tip:** Save a permanent profile so you never have to type it again.  
On macOS: `~/Library/Application Support/lazytrino/config.toml`

```toml
default_profile = "local"

[profiles.local]
url = "http://localhost:8080"
user = "admin"
```

---

## Web Dashboards

| Dashboard | URL | Credentials |
|---|---|---|
| Trino UI (query history, workers) | http://localhost:8080 | user: `admin`, no password |
| MinIO Console (browse Parquet files) | http://localhost:9001 | `admin` / `password` |

---

## Verify from CLI

```bash
# List catalogs
docker exec trino trino --execute "SHOW CATALOGS;"

# List schemas in the iceberg catalog
docker exec trino trino --execute "SHOW SCHEMAS FROM iceberg;"

# List tables
docker exec trino trino --execute "SHOW TABLES FROM iceberg.demo;"

# Check partitions on orders table
docker exec trino trino --execute "SELECT \$partition, count(*) FROM iceberg.demo.orders_by_month GROUP BY 1 ORDER BY 1 LIMIT 5;"
```

---

## Teardown

```bash
# Stop all containers and remove volumes (wipes MinIO data + catalog state)
docker compose down -v
```

To bring it back up fresh, repeat the Quickstart steps above.

---

## Reseed After Restart

If you restart `iceberg-rest` or run `docker compose down -v`, the in-memory catalog is wiped. Just reseed:

```bash
docker compose up init-db
```

---

## Troubleshooting

**`trino` container keeps restarting**  
Check the logs: `docker logs trino`. Usually a misconfigured `catalog/iceberg.properties`.

**`init-db` fails with "Cannot obtain metadata"**  
`iceberg-rest` may not be fully up. Wait a few seconds and retry: `docker compose up init-db`

**MinIO healthcheck stuck**  
The MinIO image uses `mc ready local` (not `curl`/`wget`). If you see healthcheck errors, confirm you're using the image pinned in `docker-compose.yml` (`RELEASE.2024-03-30T09-41-56Z`).

**Port conflicts**  
If `8080`, `8181`, `9000`, or `9001` are in use, update the port mappings in `docker-compose.yml`.
