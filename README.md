# lazyTrino

**lazyTrino** is a terminal user interface (TUI) for browsing Trino catalogs, schemas, and tables, running quick metadata queries, and previewing table data with keyboard-driven navigation.

Built with Rust and [Ratatui](https://github.com/ratatui/ratatui).

---

## Features

- **Hierarchical Browsing**: Easily navigate through Catalogs $\rightarrow$ Schemas $\rightarrow$ Tables $\rightarrow$ Table Actions.
- **Quick Table Actions**: One-key shortcuts to inspect table metadata and preview data.
  - Describe schema, Show `CREATE TABLE`, Show column stats, Count rows.
  - Preview data (`LIMIT 10`).
  - Inspect iceberg metadata tables (`$files`, `$partitions`, `$snapshots`, `$history`, `$properties`, `$manifests`).
- **Vim-style Navigation**: Navigate using `j`/`k`/`h`/`l`, jump with `g`/`G`, or jump to line numbers.
- **Interactive Results Viewer**: Scroll vertically and horizontally through query output tables.
- **Quick Search**: Filter table lists using `/`.

---

## Installation & Setup

### Prerequisites

- [Rust](https://www.rust-lang.org/) (edition 2024 / MSRV supporting Rust 1.85+)
- Access to a running Trino cluster (HTTP REST interface)

### Building from Source

```bash
# Build release binary and copy it to project root (./lazyTrino)
make

# Or using Cargo directly
cargo build --release
```

The executable binary is placed directly in the project root (`./lazyTrino`).

---

## Usage

Run `lazyTrino` using the packaged binary or via `make run`:

```bash
# Connect with default options (localhost:57574)
./lazyTrino

# Or build and run via make
make run

# Connect to a specific Trino server
./lazyTrino --url http://trino.example.com:8080 --user admin

# Connect with password authentication
./lazyTrino --url https://trino.example.com:8443 --user admin --password secret
```

### CLI Arguments

| Argument | Default | Description |
| --- | --- | --- |
| `--url` | `http://localhost:57574` | Trino coordinator server URL |
| `--user` | `sarthak` | Trino username |
| `--password` | *(none)* | Trino password (optional) |

---

## Keybindings

### Navigation

| Key | Action |
| --- | --- |
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `l` / `→` / `Enter` | Select / drill into item |
| `h` / `←` / `Esc` | Go back to parent view |
| `g` / `G` | Jump to top / bottom |
| `<number> + Enter` | Jump directly to item number |
| `/` | Filter/search tables |
| `?` | Toggle Help screen |
| `q` | Quit application |

### Leader Actions (`<space> <key>`)

On the Table or Action screen, press `Space` followed by an action key:

| Leader Key | Action |
| --- | --- |
| `<space> d` | Describe table schema |
| `<space> c` | Show `CREATE TABLE` statement |
| `<space> s` | Show table statistics |
| `<space> p` | Preview table rows (`LIMIT 10`) |
| `<space> n` | Count total table rows |
| `<space> f` | Query `$files` metadata |
| `<space> P` | Query `$partitions` metadata |
| `<space> S` | Query `$snapshots` metadata |
| `<space> h` | Query `$history` metadata |
| `<space> m` | Query metadata log |

### Results Viewer

| Key | Action |
| --- | --- |
| `j` / `k` | Scroll vertically |
| `h` / `l` | Scroll horizontally |
| `g` / `G` | Jump to top / bottom |
| `Esc` / `h` | Return to action menu |

---

## License

MIT
