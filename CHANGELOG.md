# Changelog

All notable changes to **RustSQL (`ruql`)** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
