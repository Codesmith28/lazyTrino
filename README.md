# lazyTrino

[![Release](https://img.shields.io/github/v/release/Codesmith28/lazyTrino?style=flat-square&color=blue)](https://github.com/Codesmith28/lazyTrino/releases)
[![CI/CD](https://img.shields.io/github/actions/workflow/status/Codesmith28/lazyTrino/ci.yml?branch=main&style=flat-square&label=CI%2FCD)](https://github.com/Codesmith28/lazyTrino/actions)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg?style=flat-square)](LICENSE)

**lazyTrino** is a terminal user interface (TUI) for browsing Trino and Presto catalogs, schemas, tables, metadata, and query results with keyboard-driven navigation, vim shortcuts, interactive modals, and mouse support.

### ⚡ Quick Install

```bash
curl -fsSL --connect-timeout 2 https://raw.githubusercontent.com/Codesmith28/lazyTrino/main/install.sh | bash
```

> Built with **Rust**, **[Ratatui](https://github.com/ratatui/ratatui)**, and **[Crossterm](https://github.com/crossterm-rs/crossterm)**. Pre-compiled binaries for macOS, Linux, and Windows are available on [GitHub Releases](https://github.com/Codesmith28/lazyTrino/releases).

---

## Demo

![lazyTrino demo](demos/demo.gif)

---

## Layout Overview

```
+------------------------+---------------------------------------------------------------+
| Menu - orders          | Table Query Bar [Press q or : to write query]                 |
| (Resizable Split)      | SQL > SELECT * FROM "iceberg"."sales"."orders" LIMIT 100      |
|                        +---------------------------------------------------------------+
| > [v] Table View       | iceberg > sales > orders | 100 records | sorted by id desc    |
|                        +---------------------------------------------------------------+
|   [c] Table DDL        | +----+-------------+-------------------------+--------------+ |
|   [s] Schema & Info    | | id | customer_id | order_date              | total_amount | |
|   [t] Show Stats       | +----+-------------+-------------------------+--------------+ |
|   [m] Sample (20 rows) | | 99 | C-1042      | 2026-10-14 09:12:00 UTC |     1,420.50 | |
|   [p] Partitions       | | 98 | C-0819      | 2026-10-14 08:45:22 UTC |       310.00 | |
|                        | +----+-------------+-------------------------+--------------+ |
+------------------------+---------------------------------------------------------------+
| Query Inspector & Audit Logs [Status: SUCCESS | Duration: 38ms | Rows: 100]            |
+----------------------------------------------------------------------------------------+
| Footer Hints [j/k:rows  h/l:cols  V:line  ^V:block  y:yank  C:cols  F:filter  S:sort]  |
+----------------------------------------------------------------------------------------+
```

---

## Installation & Setup

### Option 1: One-Line Installer (macOS & Linux)

```bash
curl -fsSL --connect-timeout 2 https://raw.githubusercontent.com/Codesmith28/lazyTrino/main/install.sh | bash
```

The installer automatically detects your operating system and architecture (`arm64` / `x86_64`) and installs to `~/.local/bin/lazyTrino`.

### Option 2: Pre-built Binaries (Manual Install)

Download the archive for your architecture from [GitHub Releases](https://github.com/Codesmith28/lazyTrino/releases), extract it, and move the binary into your `PATH`:

```bash
# Extract archive (macOS & Linux)
tar -xzf lazyTrino-*.tar.gz

# Move binary to local bin
chmod +x lazyTrino
mv lazyTrino ~/.local/bin/ # or sudo mv lazyTrino /usr/local/bin/
```

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

### Uninstallation

To remove `lazyTrino` from your machine:

```bash
# Run the uninstaller script
curl -fsSL https://raw.githubusercontent.com/Codesmith28/lazyTrino/main/uninstall.sh | bash

# Or remove the binary directly
rm -f ~/.local/bin/lazyTrino
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
- 📋 **Copy Anything, Export Everything**: Select with the mouse or `v`/`V`/`Ctrl+V`/`Shift+arrows` and copy (`y`, `Cmd+C`); `Ctrl+E` copies or exports the *entire* view (to the clipboard, or to `~/Downloads/lazytrino/*.csv`).
- 🔃 **Column-wise Sorting (`S`)**: Cycle active column sort: Ascending (`▲`) $\rightarrow$ Descending (`▼`) $\rightarrow$ None.
- 🎛️ **Column Filtering (`F`)**: Numeric comparator filters (`>`, `<`, `>=`, `<=`, `=`, `!=`, `BETWEEN`) and distinct categorical value checklists.
- 👁️ **Column Visibility Manager (`C`)**: Hide or display specific columns in wide tables.
- ✍️ **Interactive SQL Query Bar (`q` / `:`)**: Buffer editing with text selection, clipboard support, and instant query execution.
- 🖱️ **Mouse & Resizable Split**: Click-to-select, wheel scrolling, and draggable vertical pane divider.
- 🎨 **Terminal-Native Colors**: Uses only your terminal's own colors (default text plus its 16 ANSI colors), so it follows your light or dark profile. Catalogs, schemas and tables get their own colors; grid values are tinted by type (numbers, booleans, dates, NULL); the SQL bar is syntax-highlighted; footer keys stand out.
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
| `Esc` | Clear selection if any; otherwise return focus to Menu Pane / exit drilldown level / close modal |
| `Ctrl+E` | Copy or export the entire current view (clipboard / CSV file) |

### Selection, Copy & Export

Works in the catalog / schema / table lists, partition drill-down, and the results grid. The Partitions tree and Schema views have no row cursor, so use mouse drag there.

| Key | Mode / Action |
| --- | --- |
| Mouse drag | Select anything visible (including record counts, DDL, banners); copied on release and kept highlighted (reverse video in your terminal's own colors) until you click, scroll, type or press `Esc` |
| `V` | **Visual Line**: select whole rows |
| `v` / `Ctrl+V` | **Visual / Visual Block**: select a range (grid: rectangular cell block) |
| `Shift + Arrow keys` | Start and extend a selection (grid: one cell per press; `Ctrl/Alt + ←/→` or `H`/`L` jump 5 columns) |
| `o` / `O` | Swap cursor and anchor end (grid) |
| `y` | Copy selection (grid selections are CSV and always include the column header) and leave visual mode |
| `Y` | Copy the active grid row(s) as CSV |
| `Cmd+C` / `Ctrl+Shift+C` | Copy selection, else the focused cell / row. Only reaches lazyTrino if your terminal forwards it (kitty protocol terminals: kitty, WezTerm, Ghostty, foot, recent iTerm2/Alacritty) |
| `Shift`/`Option` + drag | Your terminal's native selection, with its native copy shortcut |
| `Esc` | Deselect. Only when something is selected; otherwise `Esc` keeps its usual meaning |
| `Ctrl+E` | **Copy / export entire view**: choose *Copy to clipboard* or *Export to CSV* (`<Downloads>/lazytrino/<name>_<UTC timestamp>.csv`, never overwrites). Large views default to CSV because a big clipboard copy can freeze the UI |

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
