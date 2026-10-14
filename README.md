# lazyTrino

[![Release](https://img.shields.io/github/v/release/Codesmith28/lazyTrino?style=flat-square&color=blue)](https://github.com/Codesmith28/lazyTrino/releases)
[![CI/CD](https://img.shields.io/github/actions/workflow/status/Codesmith28/lazyTrino/ci.yml?branch=main&style=flat-square&label=CI%2FCD)](https://github.com/Codesmith28/lazyTrino/actions)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg?style=flat-square)](LICENSE)

**lazyTrino** is a terminal user interface (TUI) for browsing Trino and Presto catalogs, schemas, tables, metadata, and query results with keyboard-driven navigation, vim shortcuts, interactive modals, and mouse support.

Built with **Rust**, **[Ratatui](https://github.com/ratatui/ratatui)**, and **[Crossterm](https://github.com/crossterm-rs/crossterm)**.

---

## Demo

![lazyTrino demo](demos/demo.gif)

---

## Layout Overview

```
+----------------------------------+-------------------------------------------------------------------------+
| Menu — <table_name>              |  Centralized Search Bar [/ to search]                                   |
| (Resizable Panel Split)          +-------------------------------------------------------------------------+
|                                  |  Breadcrumbs: iceberg › sales › orders                                  |
| ▸ [v] Table View Mode            |  Table Query Bar [Press q or : to write query]                          |
|   [c] Table DDL                  |  SQL > SELECT * FROM "iceberg"."sales"."orders" LIMIT 100               |
|   [s] Schema & Info              +-------------------------------------------------------------------------+
|   [t] Show Stats                 |  Table View (100 records | sorted by id desc) [C:cols F:filter S:sort]   |
|   [m] Sample Mode (20 rows)      |  +----+-------------+-------------------------+-----------------------+ |
|   [p] Partitions                 |  | id | customer_id | order_date              | total_amount          | |
|                                  |  +----+-------------+-------------------------+-----------------------+ |
|                                  |  | 99 | C-1042      | 2026-10-14 09:12:00 UTC | 1,420.50              | |
|                                  |  | 98 | C-0819      | 2026-10-14 08:45:22 UTC |   310.00              | |
|                                  |  +----+-------------+-------------------------+-----------------------+ |
+----------------------------------+-------------------------------------------------------------------------+
| Query Inspector & Audit Logs [Status: SUCCESS | Duration: 38ms | Rows: 100]                                |
+------------------------------------------------------------------------------------------------------------+
| Footer Hints [j/k:rows  h/l:cols  V:vis-line  ^V:vis-block  y:yank  C:cols  F:filter  S:sort  ?:help  ^C:quit] |
+------------------------------------------------------------------------------------------------------------+
```

---

## Key Features

- 🗂️ **Hierarchical Catalog Navigation**: Seamlessly navigate through Catalogs $\rightarrow$ Schemas $\rightarrow$ Tables $\rightarrow$ Table Actions.
- 🌲 **Partition Drill-Down (`cd`/`ls` style)**: Browse partitioned datasets one partition level at a time without split timeouts or full table scans.
- 📑 **Saved Table Views (`Ctrl+S` / `Ctrl+O` / `B`)**: Name and persist custom table view states (active filters, hidden columns, sort orders). Manage saved views with search, multi-selection (`Space`), select-all (`a`), and batch delete (`d`).
- 🔍 **Nested JSON & Map Flattening (`C`)**: Interactive column inspection modal with tree-view drilldown for nested structures. Automatically generates equivalent Trino SQL queries to unnest and extract nested fields.
- 🎯 **Advanced Vim Visual Modes**:
  - Visual Line (`V`) for full row selections.
  - Visual Block (`Ctrl+V`) for rectangular cell matrices.
  - Swap cursor and anchor ends (`o` / `O`).
  - Automatic visual mode on `Shift+Arrow` keys or mouse Shift-Click & Drag.
- 📋 **Context-Aware Clipboard & Export**:
  - `y`: Smart clipboard copy — copies single cell value, selected rows, or visual blocks formatted as CSV.
  - `Y`: Yank whole row, or export the loaded dataset to a CSV file.
- 🔃 **Column-wise Cyclic Sorting (`S` / Header Click)**: Cycle sorting on the active column: Ascending (`▲`) $\rightarrow$ Descending (`▼`) $\rightarrow$ Clear.
- 🎛️ **Comparator & Value Filtering (`F`)**:
  - Numeric comparator filters (`>`, `<`, `>=`, `<=`, `=`, `!=`, `BETWEEN`).
  - Distinct categorical value checklist with instant search and toggle selection.
- 👁️ **Column Visibility Manager (`C`)**: Hide or display specific columns to easily inspect wide schemas.
- ✍️ **Interactive SQL Query Bar (`q` / `:`)**: In-line query buffer with cursor editing, selection, clipboard integration (`Ctrl+C`/`Ctrl+V`), and instant execution.
- ♾️ **Incremental Viewport Scrolling**: Smooth incremental horizontal and vertical scrolling with visual scrollbars.
- 🖱️ **Interactive Mouse Support**: Click to select rows/columns/actions, scroll wheel navigation, and click-and-drag vertical pane border resizing.
- 📊 **Query Inspector & Audit Log**: Real-time query execution monitoring, durations (ms), fetched row counts, and traceback details.
- 🔑 **Interactive Connection Screen & Config Profiles**: Interactive login screen with persistent connection profiles via `config.toml`.

---

## Installation & Setup

### Option 1: One-Line Installer (macOS & Linux)

Install the latest pre-compiled binary directly to `~/.local/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/Codesmith28/lazyTrino/main/install.sh | bash
```

The installer automatically detects your operating system (macOS / Linux) and CPU architecture (Apple Silicon ARM64, Intel x86_64, Linux x86_64 / ARM64).

### Option 2: Pre-built Binaries

Download pre-compiled release packages for macOS, Linux, or Windows from [GitHub Releases](https://github.com/Codesmith28/lazyTrino/releases).

| Platform | Target | Archive |
| --- | --- | --- |
| **macOS (Apple Silicon)** | `aarch64-apple-darwin` | `.tar.gz` |
| **macOS (Intel)** | `x86_64-apple-darwin` | `.tar.gz` |
| **Linux (x86_64)** | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| **Windows (x86_64)** | `x86_64-pc-windows-msvc` | `.zip` |

### Option 3: Built-in Self-Updater

If you already have `lazyTrino` installed, update to the latest release at any time:

```bash
lazyTrino --update
# or
lazyTrino -u
```

### Option 4: Build from Source

Requires [Rust](https://rustup.rs/) (stable 1.85+):

```bash
git clone https://github.com/Codesmith28/lazyTrino.git
cd lazyTrino

# Build native release binary
cargo build --release

# Or install directly to ~/.cargo/bin
cargo install --path .
```

---

## Usage

```bash
# Connect using default URL (http://localhost:8080)
lazyTrino

# Connect to a custom server
lazyTrino --url http://trino.example.com:8080 --user admin

# Connect with password authentication
lazyTrino --url https://trino.example.com:8443 --user admin --password secret

# Connect using a named profile
lazyTrino --profile prod

# Update to latest version
lazyTrino --update
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
| `--update` | `-u` | | Check for and install latest GitHub release |
| `-h`, `--help` | | | Print CLI help |
| `-V`, `--version` | | | Print version |

### Configuration Profiles

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

### Global & Hierarchy Navigation

| Key | Action |
| --- | --- |
| `j` / `k` or `↓` / `↑` | Move selection down / up |
| `l` / `→` / `Enter` | Select / drill down into catalog, schema, table, or action |
| `h` / `←` / `Esc` | Back to parent level / previous screen |
| `g` / `G` | Jump to top / bottom of list |
| `<number> + Enter` | Jump directly to item number |
| `/` | Focus search bar (in menus and modals) |
| `Tab` / `Shift+H` / `Shift+L` | Toggle focus between Menu Pane (left) and Preview Pane (right) |
| `?` | Toggle Help overlay |
| `Ctrl+C` | Quit lazyTrino |

### Table Actions Menu

When a table is selected in the menu, press the action hotkey:

| Key | Action | Description |
| --- | --- | --- |
| `v` | Table View Mode | Full interactive table viewer with partition drilldown & live record counts |
| `c` | Table DDL | Execute `SHOW CREATE TABLE` |
| `s` | Schema & Info | Column metadata and type details |
| `t` | Show Stats | Execute `SHOW STATS FOR table` |
| `m` | Sample Mode | Preview sample of 20 rows |
| `p` | Partitions Tree | Render interactive partition tree hierarchy |

### Table View & Results Grid

| Key | Action |
| --- | --- |
| `j` / `k` or `↓` / `↑` | Move cell cursor / viewport down / up |
| `h` / `l` or `←` / `→` | Move cell cursor left / right across columns |
| `PgDn` / `PgUp` | Scroll page down / up (`Ctrl+D` / `Ctrl+U`, `Ctrl+F` / `Ctrl+B`) |
| `Home` / `End` (or `0` / `$`) | Jump to first / last column |
| `H` / `L` | Jump 5 columns left / right |
| `g` / `G` | Jump to top / bottom row |
| `S` | Cycle column sort: Descending (`▼`) $\rightarrow$ Ascending (`▲`) $\rightarrow$ Clear |
| `F` | Open Filter Modal for active column (numeric comparators or value list) |
| `C` | Open Column Modal (toggle column visibility, inspect nested JSON/Map tree) |
| `Ctrl+S` | Save active view state (prompt for view name) |
| `Ctrl+O` or `B` | Browse and restore saved views |
| `q` or `:` | Focus Interactive SQL Query Bar |
| `Esc` | Return focus to Menu Pane / exit drilldown level / close modal |

### Vim Visual Modes & Clipboard

| Key | Mode / Action |
| --- | --- |
| `V` | Enter **Visual Line** mode (select entire rows) |
| `Ctrl+V` | Enter **Visual Block** mode (rectangular cell block selection) |
| `Shift + Arrow keys` | Automatically enter and extend visual selection |
| `o` / `O` | Swap cursor and anchor end in visual selection |
| `y` | Yank selection to clipboard (CSV format for multi-cell/multi-row, raw string for single cell) |
| `Y` | Yank active row to clipboard / export results to CSV file |
| `Esc` | Cancel visual mode |

### Saved Views Modal (`Ctrl+O` / `B`)

| Key | Action |
| --- | --- |
| `j` / `k` or `↓` / `↑` | Move through saved views |
| `Enter` | Restore selected saved view (filters, sorting, column visibility) |
| `Space` | Toggle multi-selection checkbox on active view |
| `a` | Select all / Deselect all views |
| `d` | Delete selected view(s) (prompts `y/n` confirmation) |
| `/` | Search saved view names |
| `Esc` | Close saved views modal |

### Interactive SQL Query Bar (`q` / `:`)

| Key | Action |
| --- | --- |
| `Enter` | Execute SQL query |
| `Shift + ← / →` | Select text within query buffer |
| `Ctrl+C` / `Ctrl+V` | Copy selected SQL text / paste clipboard text |
| `Esc` | Return to results grid / cancel query editing |

### Mouse Controls

| Input | Action |
| --- | --- |
| Left Click | Select catalog, schema, table, action, or table cell |
| Click Column Header | Cycle sort order for clicked column |
| Shift + Click & Drag | Rectangular visual block selection |
| Click / Drag Query Bar | Position query cursor and select SQL text |
| Mouse Wheel | Scroll lists, partition trees, column horizontal scroll, and result tables |
| Drag Vertical Border | Dynamically resize split ratio between Menu and Preview panes |

---

## Contributing & Development

```bash
# Run code formatting check
cargo fmt --check

# Run linter
cargo clippy -- -D warnings

# Run all test suites
cargo test --all
```

---

## License

Licensed under the [Apache 2.0](LICENSE) License.
