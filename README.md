# lazyTrino

**lazyTrino** is a terminal user interface (TUI) for browsing Trino and Presto catalogs, schemas, tables, metadata, and query results with keyboard-driven navigation, vim shortcuts, and interactive mouse support.

Built with **Rust**, **[Ratatui](https://github.com/ratatui/ratatui)**, and **Crossterm**.

---

## Key Features

- 🗂️ **Hierarchical Catalog Navigation**: Seamlessly drill down from Catalogs $\rightarrow$ Schemas $\rightarrow$ Tables $\rightarrow$ Table Actions.
- 🔍 **Centralized Real-Time Search (`/`)**: Filter catalogs, schemas, tables, and columns instantly across the active hierarchy.
- ⚡ **Quick Table Actions**:
  - `v` **Table View Mode**: Full interactive table browser with infinite scroll pagination.
  - `d` **Describe Table**: Inspect schema column definitions, data types, and nullability.
  - `c` **Table DDL**: View `SHOW CREATE TABLE` statements.
  - `i` **Information Schema**: Inspect detailed column metadata from `information_schema.columns`.
  - `s` **Show Statistics**: View Trino table statistics (`SHOW STATS FOR table`).
  - `n` **Row Count**: Quick `COUNT(*)` execution.
  - `p` **Sample Mode**: Sample preview of 20 rows.
  - `P` **Partition Tree**: View hierarchical partition distributions and file metadata.
  - `S` **Vertical Schema Inspector**: Vertical column-by-column schema viewer.
- ✍️ **Interactive SQL Query Bar (`q` / `:`)**:
  - Scope custom SQL queries directly against the selected table.
  - Features real-time query text editing, text selection, multi-line scrolling, and inline target validation.
- ♾️ **Infinite Scroll Pagination**: Automatically fetches and appends subsequent batches of records as you scroll through query result sets.
- 🖱️ **Full Mouse & Resizable UI Support**:
  - Mouse click navigation and scroll wheel support.
  - Dynamic panel resizing: drag the vertical divider to adjust split proportions between list menus and preview panes.
- 📊 **Query Inspector & Execution Audit Log**: Real-time status monitoring panel tracking running queries, execution duration (ms), total rows fetched, and detailed error tracebacks.
- 🔑 **Interactive Connection Screen**: Built-in login screen with URL, username, and password fields, complete with loading spinners and error feedback.

---

## Installation & Setup

### Prerequisites

- [Rust](https://www.rust-lang.org/) (Rust 1.85+ / Edition 2024)
- Access to a running Trino cluster (HTTP REST interface)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/Codesmith28/lazyTrino.git
cd lazyTrino

# Build release binary using Makefile (outputs executable directly to ./lazyTrino)
make build

# Or build directly with Cargo
cargo build --release
```

### Makefile Targets

| Target | Description |
| --- | --- |
| `make` / `make build` | Compiles the release binary and copies it to `./lazyTrino` (default target) |
| `make dev` | Compiles a debug build and copies it to `./lazyTrino` |
| `make run` | Builds and executes the root `./lazyTrino` binary |
| `make clean` | Cleans Cargo build artifacts and removes the root `./lazyTrino` executable |

---

## Usage

Launch `lazyTrino` using the generated binary or via `make run`:

```bash
# Connect using default configuration (http://localhost:8080)
./lazyTrino

# Connect to a custom Trino coordinator server
./lazyTrino --url http://trino.example.com:8080 --user admin

# Connect with password authentication
./lazyTrino --url https://trino.example.com:8443 --user admin --password secret
```

### Command-Line Options

| Flag | Short / Alias | Default | Description |
| --- | --- | --- | --- |
| `--url` | | `http://localhost:8080` | Trino coordinator REST server URL |
| `--user` | | `$USER` (or `trino`) | Trino username |
| `--password` | `--pass` | *(none)* | Trino password (optional) |
| `-h`, `--help` | | | Print CLI help information |
| `-V`, `--version` | | | Print version information |

---

## Keybindings & Controls

### Navigation & Hierarchy

| Key | Action |
| --- | --- |
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `l` / `→` / `Enter` | Select / drill down into item |
| `h` / `←` / `Esc` | Navigate back to parent level / previous screen |
| `g` / `G` | Jump to top / bottom of list |
| `<number> + Enter` | Jump directly to item number |
| `/` | Open Centralized Search bar |
| `Tab` | Toggle focus between Menu panel and Main Preview pane |
| `?` | Toggle Help overlay |
| `q` | Quit application (when not in Table View) |

### Table Actions Menu

When a table is selected, press the corresponding action key or select it from the menu:

| Key | Action | Description |
| --- | --- | --- |
| `v` | Table View Mode | Open full interactive dataset viewer with infinite scroll |
| `d` | Describe Table | Execute `DESCRIBE catalog.schema.table` |
| `c` | Table DDL | Execute `SHOW CREATE TABLE catalog.schema.table` |
| `i` | Information Schema | Query `information_schema.columns` for column details |
| `s` | Show Stats | Execute `SHOW STATS FOR catalog.schema.table` |
| `n` | Row Count | Execute `SELECT COUNT(*) FROM catalog.schema.table` |
| `p` | Sample Mode | Execute `SELECT * FROM catalog.schema.table LIMIT 20` |
| `P` | Partition Tree | Render interactive partition hierarchy tree |
| `S` | Vertical Schema | View vertical column metadata inspector |

### Results Viewer & Table Navigation

| Key | Action |
| --- | --- |
| `j` / `k` or `↓` / `↑` | Scroll table vertically (triggers infinite scroll at list bottom) |
| `h` / `l` or `←` / `→` | Scroll table horizontally across columns |
| `g` / `G` | Jump to top / bottom of current result set |
| `q` or `:` | Focus Interactive SQL Query Bar to write custom queries |
| `Esc` / `h` | Exit table view and return to table actions menu |

### SQL Query Bar (`q` / `:` in Table View)

| Key | Action |
| --- | --- |
| `Enter` | Validate and execute custom SQL query |
| `Esc` | Cancel query editing and return focus to results viewer |
| `←` / `→` | Move text cursor left / right |
| `Home` / `End` | Jump cursor to start / end of query string |
| `Shift + ← / →` | Select text within query buffer |
| `Ctrl+C` / `Cmd+C` | Copy selected query text to system clipboard |

### Mouse & Resizer Controls

| Input | Action |
| --- | --- |
| Left Click | Select catalog, schema, table, or action menu item |
| Mouse Wheel Up / Down | Scroll lists, partition trees, and query result rows |
| Mouse Wheel Left / Right | Scroll query result tables horizontally |
| Click & Drag Vertical Border | Dynamically resize split ratio between Menu panel and Main Preview pane |

---

## Layout Overview

```
+-----------------------------------------------------------------------------------+
|  Centralized Search Bar [/ to search]                                              |
+-----------------------------------+-----------------------------------------------+
|  Catalog / Schema / Table Menu    |  Main Preview Pane / Results Viewer           |
|                                   |                                               |
|  - tpch                           |  +-----+------------------+---------------+   |
|  - sf1                            |  | id  | name             | status        |   |
|  - customer                       |  +-----+------------------+---------------+   |
|  - [v] Table View Mode            |  | 1   | Alice            | ACTIVE        |   |
|  - [d] Describe                   |  | 2   | Bob              | INACTIVE      |   |
|  - [c] Table DDL                  |  +-----+------------------+---------------+   |
|  - [P] Partition Tree             |                                               |
|                                   |  SQL > SELECT * FROM customer WHERE id > 10   |
+-----------------------------------+-----------------------------------------------+
|  Query Inspector & Audit Logs [Status: SUCCESS | Duration: 42ms | Rows: 100]       |
+-----------------------------------------------------------------------------------+
```

---

## License

[Apache 2.0](LICENSE)
