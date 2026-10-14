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
+----------------------------+---------------------------------------------------------------+
| Menu - <table_name>        | Table Query Bar [Press q or : to write query]                 |
| (Resizable Split)          | SQL > SELECT * FROM "iceberg"."sales"."orders" LIMIT 100      |
|                            +---------------------------------------------------------------+
| > [v] Table View Mode      | iceberg > sales > orders | 100 records | sorted by id desc    |
|                            +---------------------------------------------------------------+
|   [c] Table DDL            | +----+-------------+-------------------------+--------------+ |
|   [s] Schema & Info        | | id | customer_id | order_date              | total_amount | |
|   [t] Show Stats           | +----+-------------+-------------------------+--------------+ |
|   [m] Sample (20 rows)     | | 99 | C-1042      | 2026-10-14 09:12:00 UTC |     1,420.50 | |
|   [p] Partitions           | | 98 | C-0819      | 2026-10-14 08:45:22 UTC |       310.00 | |
|                            | +----+-------------+-------------------------+--------------+ |
+----------------------------+---------------------------------------------------------------+
| Query Inspector & Audit Logs [Status: SUCCESS | Duration: 38ms | Rows: 100]                |
+--------------------------------------------------------------------------------------------+
| Footer Hints [j/k:rows  h/l:cols  V:line  ^V:block  y:yank  C:cols  F:filter  S:sort]      |
+--------------------------------------------------------------------------------------------+
```

---

## Installation & Setup

### Option 1: One-Line Installer (macOS & Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/Codesmith28/lazyTrino/main/install.sh | bash
```

The installer automatically detects your operating system and architecture (`arm64` / `x86_64`) and installs to `~/.local/bin/lazyTrino`.

### Option 2: Pre-built Binaries

Download pre-compiled binaries for macOS (Apple Silicon / Intel), Linux, or Windows from [GitHub Releases](https://github.com/Codesmith28/lazyTrino/releases).

### Option 3: Built-in Self-Updater

Update to the latest release at any time:

```bash
lazyTrino --update
# or
lazyTrino -u
```

### Option 4: Build from Source

```bash
cargo install --path .
```

---

## Usage

```bash
# Connect using default URL (http://localhost:8080)
lazyTrino

# Connect to custom server
lazyTrino --url https://trino.example.com:8443 --user admin --password secret

# Connect using a named profile
lazyTrino --profile prod
```

---

## Key Features

- 🗂️ **Catalog & Schema Hierarchy**: Drill down from Catalogs $\rightarrow$ Schemas $\rightarrow$ Tables $\rightarrow$ Actions.
- 🌲 **Partition Drill-Down (`cd`/`ls` style)**: Browse partitioned tables one partition level at a time with instant counts and no split errors.
- 📑 **Saved Table Views (`Ctrl+S` / `Ctrl+O` / `B`)**: Name and persist custom view states (filters, hidden columns, sort orders) with multi-select and batch deletion.
- 🔍 **Nested JSON & Map Flattening (`C`)**: Interactive column modal with tree-view drilldown that generates unnesting Trino SQL queries.
- 🎯 **Advanced Vim Visual Modes**: Visual Line (`V`), Visual Block (`Ctrl+V`), cursor/anchor swap (`o`/`O`), and Shift-drag mouse selection.
- 📋 **Context-Aware Clipboard & Export**: Single cell copy, multi-cell/row CSV export (`y`), and full dataset CSV file dump (`Y`).
- 🔃 **Column-wise Sorting (`S`)**: Cycle active column sort: Ascending (`▲`) $\rightarrow$ Descending (`▼`) $\rightarrow$ None.
- 🎛️ **Column Filtering (`F`)**: Numeric comparator filters (`>`, `<`, `>=`, `<=`, `=`, `!=`, `BETWEEN`) and distinct categorical value checklists.
- 👁️ **Column Visibility Manager (`C`)**: Hide or display specific columns in wide tables.
- ✍️ **Interactive SQL Query Bar (`q` / `:`)**: Buffer editing with text selection, clipboard support, and instant query execution.
- 🖱️ **Mouse & Resizable Split**: Click-to-select, wheel scrolling, and draggable vertical pane divider.
- 📊 **Audit Logs & Status Bar**: Real-time query execution monitoring, latency (ms), row counts, and traceback errors.

---

## ⌨️ Keybindings & Controls

<details>
  <summary><b>Click to expand full keybindings and shortcuts table</b></summary>

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

</details>

---

## ⚙️ Configuration Profiles & CLI Options

<details>
  <summary><b>Click to expand CLI flags and configuration profile options</b></summary>

### CLI Options

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

### Configuration File

Saved in:
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

</details>

---

## License

Licensed under the [Apache 2.0](LICENSE) License.
