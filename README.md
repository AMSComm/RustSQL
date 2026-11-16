# rsql - Next-Generation PostgreSQL & Amazon Redshift CLI

A blazing-fast, dependency-free command-line interface (CLI) for **PostgreSQL** and **Amazon Redshift**, designed as a drop-in replacement and enhancement for `psql`.

---

## 🌟 Why rsql?

Standard `psql` (especially versions 14 to 17) encounters severe issues in heterogeneous database environments:
1. **Catalog Incompatibility**: Modern `psql` executes queries using catalog attributes (`c.relispartition`, `c.reloftype`, `pg_table_is_visible`) that cause syntax or catalog errors on **Amazon Redshift** (based on PostgreSQL 8.0.2 catalog) and older PostgreSQL versions (8.x, 9.x, 10, 11).
2. **Blind to Redshift Lakehouse & External Tables**: Standard `psql` meta-commands (`\dt`, `\d`, `\dn`, `\l`) query standard `pg_class`, completely ignoring Redshift Spectrum external tables (S3/Glue/Hive), Data Sharing databases/schemas, and Federated queries to RDS/Aurora.
3. **Heavyweight Dependencies**: Installing official `psql` requires PostgreSQL client packages or dynamic linking to `libpq`. `rsql` compiles into a **single, standalone, statically linked binary** (< 4MB) with **zero external dependencies**.

---

## 🚀 Key Features

- **Zero-Dependency Single Binary**:
  - Pure Rust PostgreSQL wire protocol (`tokio-postgres` + native TLS / rustls).
  - Runs anywhere on **macOS** (Apple Silicon & Intel), **Linux** (x86_64, aarch64), and **Windows** (`rsql.exe`).
  - No `libpq` or PostgreSQL client installation required.
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
- **High-Performance CSV & TSV Export**:
  - `--csv`: Export query output formatted as standard RFC 4180 CSV (requires `-c`).
  - `--tsv`: Export query output formatted as TSV (Tab-Separated Values, requires `-c`).
  - By default includes column header row.
  - `--no-header` / `-t` / `--tuples-only`: Suppresses column header row.
- **Startup Configuration Files**:
  - Automatically loads and executes settings from `~/.psqlrc`, `~/.redshiftrc`, and `~/.rsqlrc` (or `$PSQLRC`).
  - Support for `-X` / `--no-psqlrc` to skip loading startup files.
- **Modern Developer Ergonomics**:
  - **Live Syntax Highlighting**: Colors SQL keywords, literals, numbers, comments, and meta-commands in real-time.
  - **Context-Aware Tab Completion**: Auto-completes SQL keywords, meta-commands, schemas, local tables, and external Spectrum tables.
  - **Multi-Line Editing**: Seamless multi-line statement assembly until `;` is entered.
  - **Rich Output Formats**: Aligned Unicode tables, Expanded `\x` record display, CSV, TSV, JSON, and Markdown tables.
  - **Adaptive Pager**: Automatically activates scrollable pager for wide or tall result sets.
  - **Command History**: Persistent history saved to `~/.rsql_history`.

---

## 📦 Installation & Build

### Build from source (Single Binary)

```bash
cd rsql
cargo build --release
```

The resulting standalone executable will be at `./target/release/rsql`. You can move it anywhere into your `PATH`:

```bash
cp ./target/release/rsql /usr/local/bin/
```

### Cross-Compilation

Because `rsql` has zero external C library dependencies, it can be cross-compiled effortlessly:
- **macOS Apple Silicon**: `cargo build --release --target aarch64-apple-darwin`
- **Linux (musl static)**: `cargo build --release --target x86_64-unknown-linux-musl`
- **Windows**: `cargo build --release --target x86_64-pc-windows-msvc`

---

## 💻 Usage & CLI Flags

`rsql` mirrors standard `psql` command-line flags:

```bash
# Connect using connection URI
rsql postgresql://user:pass@my-redshift-cluster.xyz.redshift.amazonaws.com:5439/dev

# Connect using traditional flags
rsql -h localhost -p 5432 -U postgres -d mydb

# Run a query and output as CSV (with header)
rsql -h localhost -d mydb --csv -c "SELECT id, name, created_at FROM users LIMIT 10;"

# Run a query and output as CSV without headers
rsql -h localhost -d mydb --csv --no-header -c "SELECT id, name FROM users LIMIT 10;"
# Or using psql short alias -t:
rsql -h localhost -d mydb --csv -t -c "SELECT id, name FROM users LIMIT 10;"

# Run a query and output as TSV (Tab-Separated Values)
rsql -h localhost -d mydb --tsv -c "SELECT id, name FROM users LIMIT 10;"

# Run a query and output as TSV without header
rsql -h localhost -d mydb --tsv -t -c "SELECT id, name FROM users LIMIT 10;"

# Run a single query and output as JSON
rsql -h localhost -d mydb --json -c "SELECT * FROM users LIMIT 10;"

# Skip reading ~/.psqlrc, ~/.rsqlrc, ~/.redshiftrc
rsql -X -h localhost -d mydb -c "SELECT 1;"

# Run a SQL script file
rsql -h localhost -d mydb -f migrations.sql

# Force password prompt
rsql -h remote-host -U user -d db -W
```

### Environment Variables Supported
- `PGHOST`
- `PGPORT` (defaults to `5439` for Redshift hosts, `5432` for Postgres)
- `PGUSER`
- `PGDATABASE`
- `PGPASSWORD`
- `PGSSLMODE` (`disable`, `prefer`, `require`)
- `PSQLRC` (path to custom rc file)
- `DATABASE_URL`
- `~/.pgpass` file is automatically read if password is not supplied in CLI or env.

---

## ⚙️ Configuration via `~/.psqlrc` or `~/.redshiftrc`

`rsql` reads configuration commands from `~/.psqlrc`, `~/.redshiftrc` (when connecting to Redshift), or `~/.rsqlrc`:

```sql
-- ~/.psqlrc or ~/.redshiftrc
\timing on
\pset null '[NULL]'
\pset format aligned
\pset tuples_only off
\set PROMPT1 '%n@%m:%/%R%# '
```

---

## ⚡ Interactive Meta-Commands Reference

| Command | Description | Redshift Lakehouse Feature |
|---|---|---|
| `\q` | Quit `rsql` | |
| `\?` | Show general meta-command help | Includes Redshift commands |
| `\h [topic]` | Show SQL syntax help (e.g. `\h select`, `\h spectrum`) | |
| `\conninfo` | Display current connection, engine dialect, and SSL status | Shows detected dialect |
| `\l` / `\list` | List databases | Displays datashares via `SVV_REDSHIFT_DATABASES` |
| `\dn [pattern]` | List schemas | Displays local, external Glue, and shared schemas |
| `\dt [pattern]` | List tables | Displays local, external Spectrum, and shared tables |
| `\dv [pattern]` | List views and materialized views | |
| `\dex [pattern]` | List Redshift Spectrum external tables | S3 locations, format, SerDe |
| `\dep <tbl>` | List partitions of external table | S3 partition values & locations |
| `\ds` | List Redshift Data Shares | Producer / consumer shares |
| `\d <tbl>` | Describe table columns & metadata | Shows Redshift `DISTKEY`, `SORTKEY`, `ENCODE` or Spectrum external columns |
| `\du` / `\dg` | List database roles/users | Safe across modern PG, legacy PG, and Redshift |
| `\x [on\|off]` | Toggle expanded vertical record display | |
| `\t [on\|off]` | Toggle tuples-only mode (hide headers and row counts) | |
| `\timing [on\|off]` | Toggle query execution timer | |
| `\c <db> [usr] [host] [port]` | Connect to a new database without leaving `rsql` | |
| `\pset <opt> [val]` | Set table options (`format`, `null`, `tuples_only`, `fieldsep`, `border`) | |
| `\e [file]` | Edit query buffer in `$EDITOR` | |
| `\i <file>` | Execute commands from file | |

---

## 🧪 Testing

Run the full automated test suite:

```bash
cargo test
```
