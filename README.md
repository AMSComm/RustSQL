# RustSQL (ruql) - Next-Generation PostgreSQL & Amazon Redshift CLI

A blazing-fast, dependency-free command-line interface (CLI) for **PostgreSQL** and **Amazon Redshift**, designed as a modern drop-in replacement and enhancement for `psql`.

- **Repository**: [https://github.com/AMSComm/RustSQL](https://github.com/AMSComm/RustSQL)
- **Binary / Command**: `ruql` (with optional `psql` alias/symlink)

---

## ⚡ Quick Install

### macOS & Linux (One-line Installer)
```bash
curl -fsSL https://raw.githubusercontent.com/AMSComm/RustSQL/main/install.sh | bash
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/AMSComm/RustSQL/main/install.ps1 | iex
```

### In-Place Auto Update
To update `ruql` to the latest release at any time:
```bash
ruql --update
```

---

## 🌟 Why RustSQL?

Standard `psql` (especially versions 14 to 17) encounters severe issues in heterogeneous database environments:
1. **Catalog Incompatibility**: Modern `psql` executes queries using catalog attributes (`c.relispartition`, `c.reloftype`, `pg_table_is_visible`) that cause syntax or catalog errors on **Amazon Redshift** (based on PostgreSQL 8.0.2 catalog) and older PostgreSQL versions (8.x, 9.x, 10, 11).
2. **Blind to Redshift Lakehouse & External Tables**: Standard `psql` meta-commands (`\dt`, `\d`, `\dn`, `\l`) query standard `pg_class`, completely ignoring Redshift Spectrum external tables (S3/Glue/Hive), Data Sharing databases/schemas, and Federated queries to RDS/Aurora.
3. **Heavyweight Dependencies**: Installing official `psql` requires PostgreSQL client packages or dynamic linking to `libpq`. `ruql` compiles into a **single, standalone binary** with **zero external dependencies**.

---

## 🚀 Key Features

- **Zero-Dependency Standalone Binary**:
  - Pure Rust PostgreSQL wire protocol (`tokio-postgres` + TLS).
  - Pre-built releases for **macOS** (Apple Silicon & Intel), **Linux** (x86_64, aarch64), and **Windows** (`ruql.exe`).
  - No `libpq` or PostgreSQL client installation required.
- **Self-Updating**:
  - Single command `ruql --update` fetches, verifies, and installs the latest release in-place.
- **Adaptive Dialect Engine**:
  - Automatically identifies target server upon connection via `SELECT version()`.
  - Seamlessly chooses the exact compatible catalog queries:
    - **Amazon Redshift**: Uses `SVV_ALL_*`, `SVV_EXTERNAL_*`, `SVV_REDSHIFT_DATABASES`, `PG_TABLE_DEF`.
    - **PostgreSQL Modern (12+)**: Full modern catalog queries with partitioned tables.
    - **PostgreSQL Legacy (8.x - 11.x)**: Safe backward-compatible catalog queries.
- **First-Class Redshift Lakehouse & External Database Support**:
  - `\l` / `\list`: Displays local databases, datashare databases, and external databases (`SVV_REDSHIFT_DATABASES`).
  - `\dn`: Lists local schemas, external schemas (AWS Glue / Hive Metastore), and datashare schemas (`SVV_ALL_SCHEMAS`).
  - `\dt`: Lists standard tables, external Spectrum tables, and datashare tables (`SVV_ALL_TABLES`).
  - `\dex`: Dedicated view for Redshift Spectrum external tables, S3 URIs, and formats (Parquet, ORC, CSV).
  - `\dep <schema.table>`: Inspects partition keys and partition values (`SVV_EXTERNAL_PARTITIONS`).
  - `\ds`: Lists Redshift Data Shares (cross-cluster and AWS Data Exchange).
  - `\d <table>`: Automatically detects if a table is external (queries `SVV_EXTERNAL_COLUMNS`) or local (shows Redshift `DISTKEY`, `SORTKEY`, and column `ENCODE`).
- **Standard psql Pager with 'R' (Show All)**:
  - Built-in in-process pager (`--More-- [R: show all, Space: page, Enter: line, Q: quit]`).
  - Pressing **`R`** shows all remaining rows continuously ("bung ra hết") and keeps the entire table in the terminal scrollback buffer.
- **High-Performance Batching & I/O**:
  - Batched script execution sends multiple statements in a single network round-trip.
  - Zero-reallocation string buffer and locked stdout for maximum throughput on large datasets.
- **High-Performance CSV & TSV Export**:
  - `--csv`: Export query output formatted as standard RFC 4180 CSV (requires `-c`).
  - `--tsv`: Export query output formatted as TSV (requires `-c`).
  - `--no-header` / `-t` / `--tuples-only`: Suppresses column header row.
- **Startup Configuration Files**:
  - Automatically loads and executes settings from `~/.ruqlrc`, `~/.psqlrc`, `~/.redshiftrc`, and `~/.rsqlrc`.
  - Support for `-X` / `--no-psqlrc` to skip loading startup files.
- **Modern Developer Ergonomics**:
  - **Live Syntax Highlighting**: Colors SQL keywords, literals, numbers, comments, and meta-commands in real-time.
  - **Context-Aware Tab Completion**: Auto-completes SQL keywords, meta-commands, schemas, local tables, and external Spectrum tables.
  - **Multi-Line Editing**: Seamless multi-line statement assembly until `;` is entered.
  - **Rich Output Formats**: Aligned ASCII tables, Expanded `\x` record display (`auto`, `on`, `off`), CSV, TSV, JSON, and Markdown tables.
  - **Command History**: Persistent history saved to `~/.ruql_history`.

---

## 🛠️ Build from Source

```bash
git clone https://github.com/AMSComm/RustSQL.git
cd RustSQL
cargo build --release
```

The resulting standalone executable will be at `./target/release/ruql`.

---

## 📋 Command-Line Options

```text
Usage: ruql [OPTIONS] [URI_OR_DBNAME]

Arguments:
  [URI_OR_DBNAME]  Connection URI or dbname (e.g. postgresql://user:pass@host:port/dbname)

Options:
  -h, --host <HOST>          Database server host or socket directory [env: PGHOST=]
  -p, --port <PORT>          Database server port (default: 5432 for Postgres, 5439 for Redshift) [env: PGPORT=]
  -U, --username <USERNAME>  Database user name [env: PGUSER=]
  -d, --dbname <DBNAME>      Database name to connect to [env: PGDATABASE=]
  -c, --command <COMMAND>    Run only single command (SQL or internal) and exit
  -f, --file <FILE>          Execute commands from file, then exit
  -a, --echo-all             Echo all input from script
  -q, --quiet                Run quietly (no messages, only query output)
  -W, --password             Force password prompt
  -w, --no-password          Never prompt for password
  -x, --expanded             Turn on expanded table display mode (like \x)
      --sslmode <SSLMODE>    SSL connection mode: disable, prefer, require, verify-ca, verify-full
      --csv                  Output query results as CSV (requires -c/--command)
      --tsv                  Output query results as TSV (requires -c/--command)
  -t, --no-header            Do not print column headers (tuples only)
  -X, --no-psqlrc            Do not read startup files (~/.ruqlrc, ~/.psqlrc, ~/.rsqlrc, ~/.redshiftrc)
      --no-pager             Do not use pager to display query results
      --pager <PAGER>        When to use pager: off, on, always
      --json                 Output query results as JSON
      --markdown             Output query results as Markdown table
      --update               Self-update ruql to the latest release from GitHub
      --check-update         Check if a newer version is available without updating
  -v, --set <SET_VARS>       Set ruql/psql variable NAME=VALUE
  -V, --version              Print version
      --help                 Print help
```

---

## 📜 License

MIT License. Copyright (c) 2026 AMSComm.
