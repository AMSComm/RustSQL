# Changelog

All notable changes to **RustSQL (`ruql`)** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.0.8] - 2026-11-27

### 🛠️ Keepalive Options & Autocomplete Fixes

- **Default Keepalive with Standard Opt-Out**: Keepalive (TCP + 45s protocol heartbeat) is enabled by default; disable via `--no-keepalive`, `PGKEEPALIVES=0`, or `?keepalives=0` in the connection URI.
- **`\d` Schema Lazy-Load & Partial Permission Resilience**: `\d` / `\dt` / `\dv` now lazy-load `<schema>.` on Tab, partial permission errors on one catalog view (e.g., functions) no longer discard tables/views/schemas, and closed clients no longer mark schemas as loaded.

---

## [v0.0.7] - 2026-11-27

### 🛡️ Connection Stability & Smart Auto-Reconnect

- **Detailed DB Error Formatting**: Replaced generic `"Connection warning: db error"` with full PostgreSQL/Redshift `DbError` severity, message, and SQLSTATE formatting; expected idle closes are logged to `~/.ruql/error.log` without interrupting the prompt.
- **Session-Aware Keepalive**: Automatically enables lightweight protocol keepalive (`simple_query("")` every 45s) only while a session holds temp tables (`CREATE TEMP TABLE`, `INTO TEMP`) or an open transaction (`BEGIN`/`START TRANSACTION`), allowing clean idle disconnects otherwise.
- **Smart Read-Only Auto-Retry & Meta Auto-Reconnect**: Automatically reconnects before meta-commands (`\dt`, `\df`, etc.) if closed, and retries read-only queries (`SELECT`, `WITH`, `SHOW`, `EXPLAIN`) once on transport disconnects without retrying writes or server-killed queries.

---

## [v0.0.6] - 2026-11-26

### ⚡ Autocomplete Improvements

- **Context-Aware Column Autocomplete (`WHERE`, `ON`, `SET`, `ORDER BY`, `GROUP BY`, `SELECT`)**: Automatically extracts referenced tables and aliases from the current query, lazily loads their columns on demand (250ms fast-path wait, cached per table), and suggests matching columns.
- **Alias & Table-Qualified Column Completion**: Supports `<alias>.<column>` and `<table>.<column>` completion (e.g., `SELECT * FROM public.users u JOIN orders o ON u.<Tab>`).

---

## [v0.0.5] - 2026-11-26

### ⚡ Performance & Autocomplete Improvements

- **Default-Schema-Only Preload**: On connect, `ruql` now fetches tables, views, and functions only for `current_schema()` instead of scanning all schemas and `pg_attribute`.
- **Lazy On-Demand Schema Loading**: Typing `<schema>.` and pressing `Tab` dynamically loads objects for that schema in the background (with a 250ms fast-path wait) and caches them for subsequent completions.
- **Silent Permission-Denied Handling**: When catalog access is restricted (`42501` / permission denied), `ruql` stops further catalog preload queries immediately without printing warnings.
- **Function & Procedure Autocomplete**: Added function and stored procedure completion for SQL (`SELECT`, `CALL`, `EXECUTE`, `<schema>.<name>`) and meta-commands (`\df`, `\df+`, `\sf`, `\sf+`).

---

## [v0.0.4] - 2026-11-26

### 🐛 Fixes & Improvements

- **psql-Style History Navigation (`Up` / `Down`)**: Pressing `Up` or `Down` arrows now always navigates to the previous/next query in history, even when the current buffer contains a multiline query.
- **Multiline Cursor Movement**: Added `Alt+Up` / `Alt+Down` (Option+Up/Down on macOS) and `Ctrl+Up` / `Ctrl+Down` keybindings to move the cursor up and down across lines within multiline queries.

---

## [v0.0.3] - 2026-11-25

### 🚀 Highlights

- **Full Function & Stored Procedure Inspection (`\df`, `\df+`)**:
  - `\df [pattern]`: Inspect all functions and stored procedures with schema, name, return type, argument types, and routine kind (`func`, `proc`, `agg`, `window`).
  - `\df+ [pattern]`: Verbose view including routine owner, security (`definer`/`invoker`), language, source code body, volatility, and description.
  - Native dual-engine adaptation: leverages `pg_get_function_result` and `pg_get_function_arguments` on PostgreSQL, and `pg_catalog.pg_proc_info` with `oidvectortypes` on Amazon Redshift with seamless fallback to `pg_proc`.
- **Direct Source Code Viewer (`\sf`, `\sf+`)**:
  - `\sf <name>`: Instantly print the complete source code or PL/pgSQL body of any stored procedure or function without writing manual catalog queries.
  - `\sf+ <name>`: Show source code with line numbering. Includes automatic fallback to Redshift's native `SHOW PROCEDURE` command.
- **Comprehensive PSQL Parity Meta-Commands Suite**:
  - `\di`, `\di+ [pattern]`: List indexes (with disk size and index DDL on verbose mode).
  - `\dm`, `\dm+ [pattern]`: List materialized views.
  - `\ds`, `\ds+ [pattern]`: List sequences on PostgreSQL / datashares on Redshift (`\dsh` also supported).
  - `\dT`, `\dT+ [pattern]`: List data types and user-defined enums.
  - `\dp`, `\z [pattern]`: List access privileges and permission grants on relations.
  - `\dx`, `\dx+`: List installed database extensions.
- **Operating System & Session Tools**:
  - `\cd [dir]`: Change current working directory directly from the REPL (supports `~`).
  - `\! [cmd]`: Execute shell commands without quitting `ruql` (or launch subshell if called without args).
  - `\o [file]`: Redirect all query and meta-query results to a file, or revert back to standard output with `\o`.
  - `\watch [sec]`: Repeatedly execute the last query every N seconds (default: 2s) with timestamp headers and graceful `Ctrl+C` cancellation.
- **Interactive Auto-Completion**:
  - Upgraded tab-completion (`SqlCompleter`) covering all newly added meta-commands and arguments.

---

## [v0.0.2] - 2026-11-24

### 🚀 Highlights

- **Multi-Byte UTF-8 Statement Parsing Stability**: Fixed critical char boundary slice panic when parsing complex queries, comments, or stored procedures containing Japanese (CJK) characters or emojis inside dollar-quoted blocks (`$$`).
- **Interactive Query Cancellation (Ctrl+C / Cmd+C)**: Pressing Ctrl+C while a query is executing now sends a standard PostgreSQL `CancelRequest` to abort the running query safely, returning to the REPL prompt without terminating the `ruql` process.
- **Bastion Host & SSH Tunnel Keepalive**: Enabled proactive TCP Keepalive probes by default to maintain persistent connections across bastion hosts, SSH tunnels, AWS NAT Gateways, and stateful firewalls.
- **Automatic Connection Recovery**: Added automatic reconnection recovery (`reset`) when an idle or remote connection is lost.
- **Error & Panic Logging**: Introduced global panic hook and logging system writing full backtraces and runtime errors to `~/.ruql/error.log` while properly resetting raw terminal state.

### 🐛 Bug Fixes & Improvements

- **Parser**: Replaced byte-by-byte string slicing with byte-level slice matching (`bytes[i..].starts_with(...)`) in `split_statements`.
- **REPL**: Added `execute_query_with_cancel` using `tokio::select!` and `CancelToken` to handle `SIGINT` gracefully.
- **Network**: Configured TCP keepalive defaults (`keepalives = true`, `idle = 15s`, `interval = 5s`, `count = 3`, `tcp_user_timeout = 30s`) with support for standard `PGKEEPALIVES_*` environment variables.
- **Installer**: Enhanced `install.sh` to gracefully fallback to `~/.local/bin` when `/usr/local/bin` requires non-interactive `sudo`.

---

## [v0.0.1] - 2026-11-20

### 🚀 Highlights

- **Initial Official Release of RustSQL (`ruql`)**: Next-generation, high-performance PostgreSQL and Amazon Redshift command-line interface built entirely in Rust.
- **Ultra-Fast & Resource-Efficient**: Instant startup (<10ms), sub-megabyte memory baseline, zero JVM/Python runtime requirements, and native binary execution.
- **Built-in Auto-Update**: In-place self-replacement via `ruql --update` and `--check-update`, plus non-blocking background update notification during interactive sessions.
- **Multi-Platform Availability**: Automated releases with pre-built native binaries for macOS (Apple Silicon & Intel), Linux (x86_64 & ARM64), and Windows (x86_64).
- **Universal Install Scripts**: One-line install scripts for macOS/Linux (`install.sh`) and Windows PowerShell (`install.ps1`).

### ✨ Core Features & Enhancements

- **PostgreSQL & Amazon Redshift Engine Support**:
  - Full compatibility with PostgreSQL 10+ and Amazon Redshift clusters.
  - Native SSL/TLS encryption with certificate verification and auto-fallback.
  - Detailed connection status (`\conninfo`), database listings (`\l`), schemas (`\dn`), and user roles (`\du`).
  - Notice and warning message capture directly from database server connections.

- **Advanced Schema & Relation Inspection**:
  - Schema-aware table and view inspection (`\d [name]`, `\dt [*]`, `\di`, `\dv`).
  - Automatic fallback between standard PostgreSQL `pg_catalog` and Amazon Redshift `svv_tables`/`svv_all_columns`.
  - Case-insensitive relation resolution with automatic schema prefix detection.

- **Interactive REPL & Productive CLI**:
  - Rich interactive terminal powered by `rustyline` with persistent history (`~/.ruql_history`).
  - Context-aware tab completion for SQL keywords, slash commands, and database tables.
  - Configuration file autoloading from `~/.ruqlrc`, `~/.psqlrc`, `~/.redshiftrc`, or `~/.rsqlrc`.
  - Multiple output format modes: formatted grid tables, CSV (`\format csv`), JSON (`\format json`), and expanded display (`\x`).
  - Execution timing toggle (`\timing`) with millisecond precision benchmarking.

- **In-Process Smart Paging**:
  - Integrated terminal pager that activates automatically when query outputs exceed terminal height.
  - Seamless navigation with arrow keys, PageUp/PageDown, and `q` to dismiss.
  - Press `R` to instantly dump and view all remaining records without truncation.
  - Screen buffer preservation: scrollback is kept intact in standard terminal history upon exit.

- **Multi-Statement Batch Execution**:
  - Support for executing batch SQL files (`-f file.sql`) and chained multi-statement query strings.
  - Accurate statement reporting displaying individual counts for `CREATE TABLE`, `INSERT`, `UPDATE`, `DELETE`, etc.
