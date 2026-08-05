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

#### Runtime
- Access to a running **Trino** or **Presto** cluster (HTTP REST interface)

#### Build toolchain (required for all targets)
- **[rustup](https://rustup.rs/)** — the Rust toolchain manager
  - **Do not install Rust via Homebrew** (`brew install rust`). Homebrew's Rust bottle only includes the native target's std library and will cause cross-compilation to fail with `can't find crate for std`.
  - Install via the official script: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Rust stable** (1.85+ / Edition 2024) — installed automatically by rustup

#### Cross-compilation targets (install with rustup)

| Target | Install command | Required for |
| --- | --- | --- |
| `aarch64-apple-darwin` | `rustup target add aarch64-apple-darwin` | `make build-darwin-arm` (Apple Silicon) |
| `x86_64-apple-darwin` | `rustup target add x86_64-apple-darwin` | `make build-darwin` (Intel Mac) |
| `x86_64-unknown-linux-gnu` | `rustup target add x86_64-unknown-linux-gnu` | `make build-linux` |
| `x86_64-pc-windows-gnu` | `rustup target add x86_64-pc-windows-gnu` | `make build-windows` |

#### macOS cross-compilation linker (required for `build-darwin` and `build-darwin-arm`)

The standard `cargo` linker cannot cross-compile between macOS architectures without an `osxcross` toolchain. Instead, this project uses **[cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild)**, which routes linking through [Zig](https://ziglang.org/)'s bundled cross-linker — no Docker or osxcross needed.

```bash
# 1. Install Zig (provides the cross-linker)
brew install zig

# 2. Install cargo-zigbuild
cargo install cargo-zigbuild --locked
```

#### Linux cross-compilation linker (required for `build-linux` on macOS)

```bash
# Install the mingw-w64 / musl cross-toolchain via Homebrew
brew install musl-cross        # for x86_64-unknown-linux-gnu
brew install mingw-w64         # for x86_64-pc-windows-gnu (also needed for build-windows)
```

Alternatively, install **[cross](https://github.com/cross-rs/cross)** (requires Docker) for all non-native targets:

```bash
cargo install cross --locked
# Then use: cross build --release --target <target>
```

> **Note on `.cargo/config.toml`:** The project ships with linker overrides for `x86_64-apple-darwin` and `aarch64-apple-darwin` commented out. These are only needed when cross-compiling from a Linux host using `osxcross`. On macOS, `cargo-zigbuild` and Xcode's native `clang` handle linking automatically — do not uncomment these entries.

### Building from Source

```bash
# Clone the repository
git clone https://github.com/Codesmith28/lazyTrino.git
cd lazyTrino

# Install cross-compilation targets (one-time setup)
rustup target add aarch64-apple-darwin   # Apple Silicon
rustup target add x86_64-apple-darwin    # Intel Mac
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-pc-windows-gnu

# Native release build (auto-detects your host OS/arch)
make build

# Or build directly with Cargo
cargo build --release
```

### Makefile Targets

| Target | Description |
| --- | --- |
| `make` / `make build` | Native release binary (auto-detects host OS) → `dist/` |
| `make build-darwin-arm` | Cross-compile → macOS ARM (`aarch64-apple-darwin`) — requires `cargo-zigbuild` + `zig` |
| `make build-darwin` | Cross-compile → macOS Intel (`x86_64-apple-darwin`) — requires `cargo-zigbuild` + `zig` |
| `make build-linux` | Cross-compile → Linux x86_64 — requires `musl-cross` or `cross` |
| `make build-windows` | Cross-compile → Windows x86_64 — requires `mingw-w64` or `cross` |
| `make build-all` | Build all four platform targets at once |
| `make dev` | Debug build → `./lazyTrino` |
| `make run` | Build (native) and run |
| `make install` | Install native binary to `~/.local/bin` |
| `make clean` | Remove all build artifacts and `dist/` |

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
| `--profile <NAME>` | | *(none)* | Load connection defaults from a named config profile |
| `--log-level <LEVEL>` | | `info` | Override the default log level when `RUST_LOG` is not set (`trace`, `debug`, `info`, `warn`, `error`) |
| `--no-log` | | `false` | Disable file logging |
| `-h`, `--help` | | | Print CLI help information |
| `-V`, `--version` | | | Print version information |

Logs are written to an OS-specific cache directory by default (for example `~/.cache/lazytrino/lazytrino.log` on Linux or `~/Library/Caches/lazytrino/lazytrino.log` on macOS). Use `--no-log` to disable file logging entirely.

### Configuration

`lazyTrino` can read persistent connection defaults from an OS-specific config file resolved via `dirs::config_dir()`, for example:

- macOS: `~/Library/Application Support/lazytrino/config.toml`
- Linux: `~/.config/lazytrino/config.toml`
- Windows: `%APPDATA%\lazytrino\config.toml`

Example:

```toml
default_profile = "local"

[profiles.local]
url = "http://localhost:8080"
user = "trino"

[profiles.prod]
url = "https://trino.example.com:8443"
user = "admin"
# Optional but insecure: prefer LAZYTRINO_PASSWORD, --password, or entering it interactively.
password = "secret"

[last_used]
url = "http://localhost:8080"
user = "trino"
```

- `--profile <NAME>` selects a profile from `[profiles.<NAME>]`.
- If `--profile` is not provided, `default_profile` is used when present.
- Environment variable overrides are supported with `LAZYTRINO_URL`, `LAZYTRINO_USER`, and `LAZYTRINO_PASSWORD`.
- Passwords are **never** written to `[last_used]`. After a successful connection, `lazyTrino` persists only the last-used URL and user so the Connect screen can be prefilled next time.
- Storing `password = "..."` in a profile is supported for convenience, but it is insecure because it keeps the password in plaintext. Prefer environment variables, `--password`, or interactive entry.

Connection settings are resolved in this order: CLI flags → `LAZYTRINO_*` environment variables → selected config profile → `[last_used]` URL/user → built-in defaults.

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
| `Shift+H` / `Shift+←` | Focus the Menu Pane from the Main Preview pane (Table Actions / Results views) |
| `Shift+L` / `Shift+→` / `Tab` | Toggle focus between the Menu Pane and Main Preview pane (Table Actions / Results views) |
| `?` | Toggle Help overlay |
| `Ctrl+C` | Quit application |

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
| `y` | Copy the current loaded result grid (header + rows) to the system clipboard as TSV |
| `Y` | Export the current loaded result grid (header + rows) to a CSV file in the current working directory |
| `q` or `:` | Focus Interactive SQL Query Bar to write custom queries |
| `Esc` | Focus the Menu Pane, or return to the table actions menu if the Menu Pane is already focused |

### SQL Query Bar (`q` / `:` in Table View)

| Key | Action |
| --- | --- |
| `Enter` | Validate and execute custom SQL query |
| `Esc` | Cancel query editing and return to normal navigation on the current table screen |
| `←` / `→` | Move text cursor left / right |
| `Home` / `End` | Jump cursor to start / end of query string |
| `Shift + ← / →` | Select text within query buffer |
| `Ctrl+A` / `Cmd+A` | Select the full query buffer |
| `Ctrl+C` / `Cmd+C` | Copy selected query text to system clipboard (while the Query Bar is focused) |
| `Ctrl+V` / `Cmd+V` | Paste clipboard text at the cursor / over the selection |
| `Backspace` / `Delete` | Remove the current selection (or delete adjacent text) |
| `Alt + ← / →` | Jump by word while editing |

> **Note:** `Ctrl+C` is context-sensitive — it copies the selected query text while the Query Bar
> is focused, and quits lazyTrino everywhere else (see the Navigation table above).

### Mouse & Resizer Controls

| Input | Action |
| --- | --- |
| Left Click | Select catalog, schema, table, or action menu item |
| Click / Drag Query Bar | Place the query cursor and select SQL text while editing |
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
