# Changelog

All notable changes to **RustSQL (`ruql`)** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
