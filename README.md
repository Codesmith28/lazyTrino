# lazyTrino

**lazyTrino** is a terminal user interface (TUI) for browsing Trino catalogs, schemas, tables, metadata, and query results with keyboard-driven navigation, vim shortcuts, and interactive mouse support.

Built with **Rust**, **[Ratatui](https://github.com/ratatui/ratatui)**, and **[Crossterm](https://github.com/crossterm-rs/crossterm)**.

---

## Demo

![lazyTrino demo](demos/demo.gif)

---

## Layout Overview

```
+-----------------------------------+-------------------------------------------------------------------+
|  Menu — <table_name>              |  Centralized Search Bar [/ to search]                             |
|  (Resizable Panel Split)          +-------------------------------------------------------------------+
|                                   |  Table Query Bar [Press q or : to write query]                    |
|  ▸ [v] Table View Mode            |  SQL > SELECT * FROM "catalog"."schema"."table" LIMIT 100         |
|    [c] Table DDL                  +-------------------------------------------------------------------+
|    [i] Info Schema                |  Preview — <table_name> (Table View Mode)                         |
|    [s] Show Stats                 |                                                                   |
|    [n] Count                      |  +----+-------------+--------------+                              |
|    [p] Sample Mode (20 rows)      |  | id | name        | status       |                              |
|    [P] Partitions                 |  +----+-------------+--------------+                              |
|    [S] Schema                     |  | 1  | Alice       | ACTIVE       |                              |
|                                   |  | 2  | Bob         | INACTIVE     |                              |
|                                   |  +----+-------------+--------------+                              |
+-----------------------------------+-------------------------------------------------------------------+
|  Query Inspector & Audit Logs [Status: SUCCESS | Duration: 42ms | Rows: 100]                          |
+-------------------------------------------------------------------------------------------------------+
|  Footer Hints [ j/k:rows  </>:cols  g/G:top/btm  q/:query  Esc:menu  Tab:pane  ?:help  Ctrl+C:quit ]  |
+-------------------------------------------------------------------------------------------------------+
```

---

## Key Features

- 🗂️ **Hierarchical Catalog Navigation**: Seamlessly drill down from Catalogs $\rightarrow$ Schemas $\rightarrow$ Tables $\rightarrow$ Table Actions.
- 🔍 **Centralized Real-Time Search (`/`)**: Filter catalogs, schemas, tables, and columns instantly across the active hierarchy.
- ⚡ **Quick Table Actions**:
  - `v` **Table View Mode**: Full interactive table browser with infinite scroll.
  - `c` **Table DDL**: `SHOW CREATE TABLE` statement.
  - `i` **Information Schema**: Column metadata from `information_schema.columns`.
  - `s` **Show Statistics**: Trino table statistics (`SHOW STATS FOR table`).
  - `n` **Row Count**: `COUNT(*)` execution.
  - `p` **Sample Mode**: Preview sample of 20 rows.
  - `P` **Partition Tree**: Interactive partition hierarchy and file paths.
  - `S` **Vertical Schema**: Column-by-column vertical schema inspector.
- ✍️ **Interactive SQL Query Bar (`q` / `:`)**: Real-time query buffer editing, text selection, clipboard support (`Ctrl+C`/`Ctrl+V`), and custom query execution.
- ♾️ **Infinite Scroll Pagination**: Automatically fetches and appends subsequent batches of records as you scroll.
- 🖱️ **Mouse & Resizable UI**: Click to select, scroll wheel support, and click-and-drag vertical border resizing.
- 📊 **Query Inspector & Audit Log**: Real-time execution status tracking, query durations (ms), fetched row counts, and traceback errors.
- 🔑 **Interactive Connection Screen**: Built-in login screen with URL, username, and password fields.

---

## Installation & Setup

### Option 1: Pre-built Binaries (Recommended)
Download the latest pre-compiled binary for macOS (Apple Silicon / Intel), Linux, or Windows from [GitHub Releases](https://github.com/Codesmith28/lazyTrino/releases).

### Option 2: Build from Source
Requires [Rust](https://rustup.rs/) (stable 1.85+):

```bash
git clone https://github.com/Codesmith28/lazyTrino.git
cd lazyTrino

# Build release binary
cargo build --release

# Or install directly to ~/.cargo/bin
cargo install --path .
```

### Makefile Targets

| Command | Description |
| --- | --- |
| `make build` | Build native release binary → `dist/` |
| `make dev` | Build native debug binary |
| `make run` | Build and launch `lazyTrino` |
| `make install` | Install native binary to `~/.local/bin` |
| `make clean` | Remove build artifacts |

> For cross-compiling to non-native platforms locally (e.g. `make build-all`), install [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild) or [`cross`](https://github.com/cross-rs/cross).

---

## Usage

```bash
# Connect using default URL (http://localhost:8080)
./lazyTrino

# Connect to a custom server
./lazyTrino --url http://trino.example.com:8080 --user admin

# Connect with password authentication
./lazyTrino --url https://trino.example.com:8443 --user admin --password secret
```

### Command-Line Options

| Flag | Short / Alias | Default | Description |
| --- | --- | --- | --- |
| `--url` | | `http://localhost:8080` | Trino coordinator REST server URL |
| `--user` | | `$USER` (or `trino`) | Trino username |
| `--password` | `--pass` | *(none)* | Trino password |
| `--profile <NAME>` | | *(none)* | Load connection defaults from named profile |
| `--log-level <LEVEL>` | | `info` | Set file log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `--no-log` | | `false` | Disable file logging entirely |
| `-h`, `--help` | | | Print CLI help |
| `-V`, `--version` | | | Print version |

### Configuration

Persistent connection profiles can be saved in an OS-specific `config.toml`:

- **macOS**: `~/Library/Application Support/lazytrino/config.toml`
- **Linux**: `~/.config/lazytrino/config.toml`
- **Windows**: `%APPDATA%\lazytrino\config.toml`

```toml
default_profile = "local"

[profiles.local]
url = "http://localhost:8080"
user = "trino"

[profiles.prod]
url = "https://trino.example.com:8443"
user = "admin"
```

**Resolution Precedence**: CLI flags $\rightarrow$ `LAZYTRINO_*` env vars $\rightarrow$ Selected Profile $\rightarrow$ `[last_used]` values $\rightarrow$ Built-in defaults.

---

## Keybindings & Controls

### Navigation & Hierarchy

| Key | Action |
| --- | --- |
| `j` / `k` or `↓` / `↑` | Move selection down / up |
| `l` / `→` / `Enter` | Select / drill down |
| `h` / `←` / `Esc` | Back to parent level / previous screen |
| `g` / `G` | Jump to top / bottom of list |
| `<number> + Enter` | Jump directly to item number |
| `/` | Focus Centralized Search Bar |
| `Tab` / `Shift+H` / `Shift+L` | Toggle focus between Menu Pane and Preview Pane |
| `?` | Toggle Help overlay |
| `Ctrl+C` | Quit application |

### Table Actions Menu

When a table is selected, press the corresponding action hotkey:

| Key | Action | Description |
| --- | --- | --- |
| `v` | Table View | Open interactive dataset viewer with infinite scroll |
| `c` | DDL | Execute `SHOW CREATE TABLE` |
| `i` | Info Schema | Query `information_schema.columns` metadata |
| `s` | Show Stats | Execute `SHOW STATS FOR table` |
| `n` | Row Count | Execute `SELECT COUNT(*)` |
| `p` | Sample | Preview 20 sample rows |
| `P` | Partition Tree | Render interactive partition tree |
| `S` | Vertical Schema | View vertical column metadata inspector |

### Results Viewer & SQL Query Bar

| Key | Action |
| --- | --- |
| `j` / `k` or `↓` / `↑` | Scroll result table vertically |
| `<` / `>` | Scroll result table horizontally |
| `g` / `G` | Jump to top / bottom of loaded results |
| `y` | Copy current results to clipboard as TSV |
| `Y` | Export current results to a CSV file |
| `q` or `:` | Focus Interactive SQL Query Bar |
| `Enter` *(Query Bar)* | Execute SQL query |
| `Shift + ← / →` *(Query Bar)* | Select text in query buffer |
| `Ctrl+C` / `Ctrl+V` *(Query Bar)* | Copy selected query text / paste clipboard content |
| `Esc` | Return focus to Menu Pane / cancel query editing |

> **Note on `h`/`l` vs. `<`/`>`:** `h`/`l` (and `←`/`→`) are always
> *hierarchical* — they move up/down the catalog → schema → table → action
> tree, and in a partitioned Table View's drill-down, `h` steps up one
> partition level. They never scroll a result grid. To scroll a wide
> result grid horizontally (in any results view, including the Table
> View drill-down's leaf record grid), use `<` / `>` instead.

### Mouse Controls

| Input | Action |
| --- | --- |
| Left Click | Select catalog, schema, table, or action menu item |
| Click / Drag Query Bar | Position query cursor and select SQL text |
| Mouse Wheel | Scroll lists, partition trees, and query result tables |
| Drag Vertical Border | Dynamically resize split ratio between Menu and Preview panes |

---

## License

[Apache 2.0](LICENSE)
