use crate::connection::DbConnection;
use crate::error::{Result, RsqlError};
use crate::output::{OutputFormat, QueryResult};
use colored::Colorize;
use std::fs;
use std::time::Instant;

pub struct AppState {
    pub format: OutputFormat,
    pub timing: bool,
    pub quiet: bool,
    pub echo_all: bool,
    pub last_query: String,
    pub vars: std::collections::HashMap<String, String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            format: OutputFormat::Aligned,
            timing: false,
            quiet: false,
            echo_all: false,
            last_query: String::new(),
            vars: std::collections::HashMap::new(),
        }
    }
}

pub enum MetaCommand {
    Quit,
    HelpGeneral,
    HelpSql(Option<String>),
    ListDatabases,
    ListSchemas(Option<String>),
    ListTables(Option<String>),
    ListViews(Option<String>),
    ListExternalTables(Option<String>),
    ListExternalPartitions(Option<String>, String),
    ListDatashares,
    Describe(Option<String>, String),
    ListUsers,
    ToggleExpanded,
    ToggleTiming,
    Connect(String, Option<String>, Option<String>, Option<u16>),
    ConnInfo,
    Edit(Option<String>),
    Include(String),
    Set(String, String),
    Pset(String, Option<String>),
    Echo(String),
}

pub fn parse_meta_command(line: &str) -> Option<MetaCommand> {
    let trimmed = line.trim();
    if !trimmed.starts_with('\\') {
        return None;
    }

    let without_slash = &trimmed[1..];
    let parts: Vec<&str> = without_slash.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let cmd = parts[0];
    let args = &parts[1..];

    match cmd {
        "q" | "quit" => Some(MetaCommand::Quit),
        "?" => Some(MetaCommand::HelpGeneral),
        "h" | "help" => Some(MetaCommand::HelpSql(args.first().map(|s| s.to_string()))),
        "l" | "list" | "l+" => Some(MetaCommand::ListDatabases),
        "dn" | "dn+" => Some(MetaCommand::ListSchemas(args.first().map(|s| s.to_string()))),
        "dt" | "dt+" => Some(MetaCommand::ListTables(args.first().map(|s| s.to_string()))),
        "dv" | "dv+" => Some(MetaCommand::ListViews(args.first().map(|s| s.to_string()))),
        "dex" => Some(MetaCommand::ListExternalTables(args.first().map(|s| s.to_string()))),
        "dep" => {
            if let Some(target) = args.first() {
                if target.contains('.') {
                    let parts: Vec<&str> = target.splitn(2, '.').collect();
                    Some(MetaCommand::ListExternalPartitions(Some(parts[0].to_string()), parts[1].to_string()))
                } else {
                    Some(MetaCommand::ListExternalPartitions(None, target.to_string()))
                }
            } else {
                Some(MetaCommand::HelpGeneral)
            }
        }
        "ds" => Some(MetaCommand::ListDatashares),
        "d" | "d+" => {
            if let Some(target) = args.first() {
                if target.contains('.') {
                    let parts: Vec<&str> = target.splitn(2, '.').collect();
                    Some(MetaCommand::Describe(Some(parts[0].to_string()), parts[1].to_string()))
                } else {
                    Some(MetaCommand::Describe(None, target.to_string()))
                }
            } else {
                // If just \d without args, list tables and views
                Some(MetaCommand::ListTables(None))
            }
        }
        "du" | "du+" | "dg" | "dg+" => Some(MetaCommand::ListUsers),
        "x" => Some(MetaCommand::ToggleExpanded),
        "timing" => Some(MetaCommand::ToggleTiming),
        "c" | "connect" => {
            if let Some(db) = args.first() {
                let user = args.get(1).map(|s| s.to_string());
                let host = args.get(2).map(|s| s.to_string());
                let port = args.get(3).and_then(|s| s.parse::<u16>().ok());
                Some(MetaCommand::Connect(db.to_string(), user, host, port))
            } else {
                Some(MetaCommand::ConnInfo)
            }
        }
        "conninfo" => Some(MetaCommand::ConnInfo),
        "e" | "edit" => Some(MetaCommand::Edit(args.first().map(|s| s.to_string()))),
        "i" | "include" => args.first().map(|s| MetaCommand::Include(s.to_string())),
        "set" => {
            if args.len() >= 2 {
                Some(MetaCommand::Set(args[0].to_string(), args[1..].join(" ")))
            } else {
                None
            }
        }
        "pset" => {
            if let Some(opt) = args.first() {
                let val = args.get(1).map(|s| s.to_string());
                Some(MetaCommand::Pset(opt.to_string(), val))
            } else {
                None
            }
        }
        "echo" => Some(MetaCommand::Echo(args.join(" "))),
        _ => None,
    }
}

pub async fn execute_meta_command(
    meta: MetaCommand,
    conn: &mut DbConnection,
    state: &mut AppState,
) -> Result<bool> {
    // Returns Ok(true) if REPL should continue, Ok(false) if quit
    match meta {
        MetaCommand::Quit => return Ok(false),
        MetaCommand::HelpGeneral => {
            print_general_help(conn.dialect.has_external_support());
        }
        MetaCommand::HelpSql(topic) => {
            print_sql_help(topic.as_deref());
        }
        MetaCommand::ListDatabases => {
            let sql = conn.dialect.list_databases_query();
            run_and_display_meta_query(conn, state, sql).await?;
        }
        MetaCommand::ListSchemas(pattern) => {
            let sql = conn.dialect.list_schemas_query(pattern.as_deref());
            run_and_display_meta_query(conn, state, &sql).await?;
        }
        MetaCommand::ListTables(pattern) => {
            let sql = conn.dialect.list_tables_query(pattern.as_deref());
            run_and_display_meta_query(conn, state, &sql).await?;
        }
        MetaCommand::ListViews(pattern) => {
            let sql = conn.dialect.list_views_query(pattern.as_deref());
            run_and_display_meta_query(conn, state, &sql).await?;
        }
        MetaCommand::ListExternalTables(pattern) => {
            if let Some(sql) = conn.dialect.list_external_tables_query(pattern.as_deref()) {
                println!("{}", "Listing Amazon Redshift Spectrum External Tables:".cyan().bold());
                run_and_display_meta_query(conn, state, &sql).await?;
            } else {
                println!("{}", "External tables are only supported on Amazon Redshift.".yellow());
            }
        }
        MetaCommand::ListExternalPartitions(schema, table) => {
            if let Some(sql) = conn.dialect.list_external_partitions_query(schema.as_deref(), &table) {
                println!("{}", format!("External Partitions for {}:", table).cyan().bold());
                run_and_display_meta_query(conn, state, &sql).await?;
            } else {
                println!("{}", "External partitions are only supported on Amazon Redshift.".yellow());
            }
        }
        MetaCommand::ListDatashares => {
            if let Some(sql) = conn.dialect.list_datashares_query() {
                println!("{}", "Listing Amazon Redshift Data Shares:".cyan().bold());
                run_and_display_meta_query(conn, state, sql).await?;
            } else {
                println!("{}", "Data shares are only supported on Amazon Redshift.".yellow());
            }
        }
        MetaCommand::Describe(schema, table) => {
            describe_target(conn, state, schema.as_deref(), &table).await?;
        }
        MetaCommand::ListUsers => {
            let sql = conn.dialect.list_users_query();
            run_and_display_meta_query(conn, state, sql).await?;
        }
        MetaCommand::ToggleExpanded => {
            state.format = match state.format {
                OutputFormat::Expanded => {
                    println!("Expanded display is off.");
                    OutputFormat::Aligned
                }
                _ => {
                    println!("Expanded display is on.");
                    OutputFormat::Expanded
                }
            };
        }
        MetaCommand::ToggleTiming => {
            state.timing = !state.timing;
            println!(
                "Timing is {}.",
                if state.timing { "on" } else { "off" }
            );
        }
        MetaCommand::ConnInfo => {
            print_conninfo(conn);
        }
        MetaCommand::Connect(dbname, user, host, port) => {
            let mut new_config = conn.config.clone();
            new_config.dbname = dbname;
            if let Some(u) = user {
                new_config.user = u;
            }
            if let Some(h) = host {
                new_config.host = h;
            }
            if let Some(p) = port {
                new_config.port = p;
            }

            println!("Connecting to database \"{}\" at {}:{}...", new_config.dbname, new_config.host, new_config.port);
            match DbConnection::connect(new_config).await {
                Ok(new_conn) => {
                    *conn = new_conn;
                    print_conninfo(conn);
                }
                Err(e) => {
                    eprintln!("{}", format!("Connection failed: {}", e).red());
                }
            }
        }
        MetaCommand::Edit(filepath) => {
            handle_edit(filepath.as_deref(), state)?;
        }
        MetaCommand::Include(filepath) => {
            handle_include(&filepath, conn, state).await?;
        }
        MetaCommand::Set(var, val) => {
            println!("Variable {} set to {}", var, val);
            state.vars.insert(var, val);
        }
        MetaCommand::Pset(opt, val) => {
            handle_pset(&opt, val.as_deref(), state);
        }
        MetaCommand::Echo(msg) => {
            println!("{}", msg);
        }
    }
    Ok(true)
}

async fn run_and_display_meta_query(
    conn: &DbConnection,
    state: &AppState,
    sql: &str,
) -> Result<()> {
    let start = Instant::now();
    let rows = conn.client.query(sql, &[]).await?;
    let duration = start.elapsed();
    let res = QueryResult::from_rows(&rows, duration, None);
    res.display(state.format, state.timing, true)?;
    Ok(())
}

async fn describe_target(
    conn: &DbConnection,
    state: &AppState,
    schema: Option<&str>,
    table: &str,
) -> Result<()> {
    // If Redshift, first check if it's an external table
    if conn.dialect.has_external_support() {
        if let Some(ext_sql) = conn.dialect.describe_external_table_query(schema, table) {
            if let Ok(rows) = conn.client.query(&ext_sql, &[]).await {
                if !rows.is_empty() {
                    let full_name = match schema {
                        Some(s) => format!("{}.{}", s, table),
                        None => table.to_string(),
                    };
                    println!(
                        "{}",
                        format!("External Table \"{}\" (Redshift Spectrum / Lakehouse):", full_name)
                            .cyan()
                            .bold()
                    );
                    let res = QueryResult::from_rows(&rows, Duration::from_millis(0), None);
                    res.display(state.format, state.timing, false)?;

                    // Also show partition location
                    if let Some(part_sql) = conn.dialect.list_external_partitions_query(schema, table) {
                        if let Ok(part_rows) = conn.client.query(&part_sql, &[]).await {
                            if !part_rows.is_empty() {
                                println!("\n{}", "Sample External Partitions:".cyan());
                                let part_res = QueryResult::from_rows(&part_rows, Duration::from_millis(0), None);
                                part_res.display(state.format, false, false)?;
                            }
                        }
                    }
                    return Ok(());
                }
            }
        }
    }

    // Local table
    let local_sql = conn.dialect.describe_table_query(schema, table);
    match conn.client.query(&local_sql, &[]).await {
        Ok(rows) => {
            if rows.is_empty() {
                println!(
                    "{}",
                    format!("Did not find any relation named \"{}\".", table).yellow()
                );
            } else {
                let full_name = match schema {
                    Some(s) => format!("{}.{}", s, table),
                    None => table.to_string(),
                };
                println!("{}", format!("Table \"{}\":", full_name).cyan().bold());
                let res = QueryResult::from_rows(&rows, Duration::from_millis(0), None);
                res.display(state.format, state.timing, false)?;
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Error describing relation: {}", e).red());
        }
    }

    Ok(())
}

fn print_conninfo(conn: &DbConnection) {
    let ssl_info = match conn.config.ssl_mode {
        crate::config::SslMode::Disable => "unencrypted".yellow().to_string(),
        _ => "SSL encrypted (TLS)".green().to_string(),
    };
    println!(
        "You are connected to database \"{}\" as user \"{}\" on host \"{}\" (port {}) via {}.",
        conn.config.dbname.bold(),
        conn.config.user.bold(),
        conn.config.host,
        conn.config.port,
        ssl_info
    );
    println!("Engine Dialect: {}", conn.dialect.dialect_name().cyan().bold());
    println!("Server Version: {}", conn.server_version.trim());
}

fn print_general_help(has_external: bool) {
    println!("{}", "General rsql meta-commands:".bold());
    println!("  \\q                 quit rsql");
    println!("  \\?                 show help on backslash commands");
    println!("  \\h [cmd]           help on SQL syntax");
    println!("  \\conninfo          display information about current connection");
    println!("  \\c [db] [usr] [h]  connect to new database");
    println!("  \\timing            toggle timing of commands");
    println!("  \\x                 toggle expanded table output (on/off)");
    println!("  \\e [file]          edit the current query buffer in $EDITOR");
    println!("  \\i <file>          execute commands from file");
    println!("  \\pset format <fmt> set format (aligned, expanded, csv, json, markdown)");
    println!();
    println!("{}", "Informational Queries:".bold());
    println!("  \\l, \\list          list all databases (including datashares)");
    println!("  \\dn [pattern]      list schemas (local, external, shared)");
    println!("  \\dt [pattern]      list tables (local + external)");
    println!("  \\dv [pattern]      list views and materialized views");
    println!("  \\d <relation>      describe table columns, types, nullability");
    println!("  \\du, \\dg           list users and roles");

    if has_external {
        println!();
        println!("{}", "Amazon Redshift Lakehouse & Spectrum commands:".cyan().bold());
        println!("  \\dex [pattern]     list Redshift Spectrum external tables & S3 locations");
        println!("  \\dep <schema.tbl>  list partitions for Redshift external table");
        println!("  \\ds                list Redshift Data Shares (cross-cluster/lakehouse)");
    }
    println!();
}

fn print_sql_help(topic: Option<&str>) {
    match topic {
        Some("select") => {
            println!("Syntax: SELECT [ ALL | DISTINCT ] select_expr [, ...] [ FROM ... ] [ WHERE ... ] [ GROUP BY ... ] [ HAVING ... ] [ ORDER BY ... ] [ LIMIT ... ]");
        }
        Some("insert") => {
            println!("Syntax: INSERT INTO table_name [ ( column_name [, ...] ) ] {{ VALUES ( {{ expression }} [, ...] ) [, ...] | query }}");
        }
        Some("create external schema") | Some("spectrum") => {
            println!("Syntax (Redshift Spectrum):");
            println!("  CREATE EXTERNAL SCHEMA schema_name");
            println!("  FROM DATA CATALOG");
            println!("  DATABASE 'glue_or_hive_database'");
            println!("  IAM_ROLE 'arn:aws:iam::123456789012:role/MyRedshiftRole'");
            println!("  REGION 'us-west-2';");
        }
        _ => {
            println!("Type \\h <command> for help on a specific SQL statement (e.g. \\h select, \\h insert, \\h spectrum).");
        }
    }
}

fn handle_pset(opt: &str, val: Option<&str>, state: &mut AppState) {
    match opt {
        "format" => {
            if let Some(v) = val {
                match v.to_lowercase().as_str() {
                    "csv" => {
                        state.format = OutputFormat::Csv;
                        println!("Output format is csv.");
                    }
                    "json" => {
                        state.format = OutputFormat::Json;
                        println!("Output format is json.");
                    }
                    "markdown" | "md" => {
                        state.format = OutputFormat::Markdown;
                        println!("Output format is markdown.");
                    }
                    "expanded" => {
                        state.format = OutputFormat::Expanded;
                        println!("Output format is expanded.");
                    }
                    _ => {
                        state.format = OutputFormat::Aligned;
                        println!("Output format is aligned.");
                    }
                }
            } else {
                println!("Current output format is {:?}.", state.format);
            }
        }
        _ => {
            println!("Unrecognized \\pset option: {}", opt);
        }
    }
}

fn handle_edit(filepath: Option<&str>, state: &mut AppState) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let temp_path = filepath
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("rsql_buffer.sql"));

    // Write last query to temp file
    fs::write(&temp_path, &state.last_query)?;

    let status = std::process::Command::new(editor)
        .arg(&temp_path)
        .status()?;

    if status.success() {
        if let Ok(content) = fs::read_to_string(&temp_path) {
            state.last_query = content;
            println!("Query buffer updated.");
        }
    }
    Ok(())
}

async fn handle_include(
    filepath: &str,
    conn: &mut DbConnection,
    state: &mut AppState,
) -> Result<()> {
    let content = fs::read_to_string(filepath)
        .map_err(|e| RsqlError::Io(e))?;

    println!("Executing script from \"{}\"...", filepath);
    for stmt in content.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('\\') {
            if let Some(meta) = parse_meta_command(trimmed) {
                Box::pin(execute_meta_command(meta, conn, state)).await?;
            }
        } else {
            let start = Instant::now();
            match conn.client.simple_query(trimmed).await {
                Ok(messages) => {
                    let duration = start.elapsed();
                    for msg in messages {
                        match msg {
                            tokio_postgres::SimpleQueryMessage::Row(r) => {
                                let mut cols = Vec::new();
                                for i in 0..r.columns().len() {
                                    cols.push(r.columns()[i].name().to_string());
                                }
                                let mut vals = Vec::new();
                                for i in 0..cols.len() {
                                    vals.push(r.get(i).unwrap_or("NULL").to_string());
                                }
                                let res = QueryResult {
                                    columns: cols,
                                    rows: vec![vals],
                                    command_tag: None,
                                    rows_affected: Some(1),
                                    duration,
                                };
                                res.display(state.format, state.timing, false)?;
                            }
                            tokio_postgres::SimpleQueryMessage::CommandComplete(n) => {
                                if !state.quiet {
                                    println!("{} ({:.3} ms)", n, duration.as_secs_f64() * 1000.0);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}", format!("Error: {}", e).red());
                }
            }
        }
    }
    Ok(())
}

use std::time::Duration;
