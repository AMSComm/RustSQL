use crate::connection::DbConnection;
use crate::error::{Result, RsqlError};
use crate::output::{OutputFormat, QueryResult};
use colored::Colorize;
use std::fs;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerMode {
    Off,
    On,
    Always,
}

pub struct AppState {
    pub format: OutputFormat,
    pub timing: bool,
    pub quiet: bool,
    pub echo_all: bool,
    pub tuples_only: bool,
    pub border: u8,
    pub null_str: String,
    pub field_sep: String,
    pub last_query: String,
    pub vars: std::collections::HashMap<String, String>,
    pub pager: PagerMode,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            format: OutputFormat::Aligned,
            timing: false,
            quiet: false,
            echo_all: false,
            tuples_only: false,
            border: 1,
            null_str: String::new(),
            field_sep: "|".to_string(),
            last_query: String::new(),
            vars: std::collections::HashMap::new(),
            pager: PagerMode::On,
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
    ToggleExpanded(Option<bool>),
    ToggleTiming(Option<bool>),
    ToggleTuplesOnly(Option<bool>),
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
        "x" => Some(MetaCommand::ToggleExpanded(parse_bool_arg(args.first().copied()))),
        "timing" => Some(MetaCommand::ToggleTiming(parse_bool_arg(args.first().copied()))),
        "t" => Some(MetaCommand::ToggleTuplesOnly(parse_bool_arg(args.first().copied()))),
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
                let val = if args.len() > 1 {
                    Some(args[1..].join(" "))
                } else {
                    None
                };
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
            match run_and_display_meta_query(conn, state, &sql).await {
                Ok(_) => {}
                Err(_) if conn.dialect.has_external_support() => {
                    let fallback = fallback_postgres_tables_query(pattern.as_deref());
                    run_and_display_meta_query(conn, state, &fallback).await?;
                }
                Err(e) => return Err(e),
            }
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
        MetaCommand::ToggleExpanded(explicit) => {
            let next = explicit.unwrap_or(state.format != OutputFormat::Expanded);
            if next {
                state.format = OutputFormat::Expanded;
                println!("Expanded display is on.");
            } else {
                state.format = OutputFormat::Aligned;
                println!("Expanded display is off.");
            }
        }
        MetaCommand::ToggleTiming(explicit) => {
            state.timing = explicit.unwrap_or(!state.timing);
            println!(
                "Timing is {}.",
                if state.timing { "on" } else { "off" }
            );
        }
        MetaCommand::ToggleTuplesOnly(explicit) => {
            state.tuples_only = explicit.unwrap_or(!state.tuples_only);
            println!(
                "Tuples only is {}.",
                if state.tuples_only { "on" } else { "off" }
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
    let res = QueryResult::from_rows(&rows, duration, None, &state.null_str);
    res.display(state.format, state.timing, state.tuples_only, state.border, state.pager)?;
    Ok(())
}

fn fallback_postgres_tables_query(pattern: Option<&str>) -> String {
    let base = "SELECT \
        n.nspname AS \"Schema\", \
        c.relname AS \"Name\", \
        CASE c.relkind \
            WHEN 'r' THEN 'table' \
            WHEN 'v' THEN 'view' \
            ELSE 'other' \
        END AS \"Type\", \
        pg_catalog.pg_get_userbyid(c.relowner) AS \"Owner\" \
     FROM pg_catalog.pg_class c \
     LEFT JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
     WHERE c.relkind IN ('r', 'v')";

    if let Some(p) = pattern {
        let pat = p.replace('*', "%");
        if pat == "%" || pat == "%.%" {
            format!("{} AND n.nspname NOT LIKE 'pg_temp_%' ORDER BY 1, 2;", base)
        } else if pat.contains('.') {
            let parts: Vec<&str> = pat.splitn(2, '.').collect();
            format!(
                "{} AND n.nspname ILIKE '{}' AND c.relname ILIKE '{}' ORDER BY 1, 2;",
                base, parts[0], parts[1]
            )
        } else {
            format!(
                "{} AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema' \
                   AND c.relname ILIKE '{}' \
                 ORDER BY 1, 2;",
                base, pat
            )
        }
    } else {
        format!(
            "{} AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema' \
             ORDER BY 1, 2;",
            base
        )
    }
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
                    let res = QueryResult::from_rows(&rows, Duration::from_millis(0), None, &state.null_str);
                    res.display(state.format, state.timing, state.tuples_only, state.border, PagerMode::Off)?;

                    // Also show partition location
                    if let Some(part_sql) = conn.dialect.list_external_partitions_query(schema, table) {
                        if let Ok(part_rows) = conn.client.query(&part_sql, &[]).await {
                            if !part_rows.is_empty() {
                                println!("\n{}", "Sample External Partitions:".cyan());
                                let part_res = QueryResult::from_rows(&part_rows, Duration::from_millis(0), None, &state.null_str);
                                part_res.display(state.format, false, state.tuples_only, state.border, PagerMode::Off)?;
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
                let res = QueryResult::from_rows(&rows, Duration::from_millis(0), None, &state.null_str);
                res.display(state.format, state.timing, state.tuples_only, state.border, PagerMode::Off)?;
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Error describing relation: {}", e).red());
        }
    }

    Ok(())
}

fn print_conninfo(conn: &DbConnection) {
    let ssl_info = if conn.is_ssl {
        "SSL encryption".green().to_string()
    } else {
        "an unencrypted connection".yellow().to_string()
    };
    println!(
        "You are connected to database \"{}\" as user \"{}\" on host \"{}\" (port {}) with {}.",
        conn.config.dbname.bold(),
        conn.config.user.bold(),
        conn.config.host,
        conn.config.port,
        ssl_info
    );
    println!("Client: psql (PostgreSQL) 17.4 (rsql 0.1.0)");
    println!("Server: {}", conn.server_version.trim());
    println!("Engine Dialect: {}", conn.dialect.dialect_name().cyan().bold());
}

fn print_general_help(has_external: bool) {
    println!("{}", "General rsql meta-commands:".bold());
    println!("  \\q                 quit rsql");
    println!("  \\?                 show help on backslash commands");
    println!("  \\h [cmd]           help on SQL syntax");
    println!("  \\conninfo          display information about current connection");
    println!("  \\c [db] [usr] [h]  connect to new database");
    println!("  \\timing [on|off]   toggle timing of commands");
    println!("  \\x [on|off]        toggle expanded table output");
    println!("  \\t [on|off]        toggle tuples only (no headers/footers)");
    println!("  \\e [file]          edit the current query buffer in $EDITOR");
    println!("  \\i <file>          execute commands from file");
    println!("  \\pset <opt> [val]  set table options (format, null, tuples_only, fieldsep, border)");
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

pub fn handle_pset(opt: &str, val: Option<&str>, state: &mut AppState) {
    match opt.to_lowercase().as_str() {
        "format" => {
            if let Some(v) = val {
                match v.to_lowercase().as_str() {
                    "csv" => {
                        state.format = OutputFormat::Csv;
                        println!("Output format is csv.");
                    }
                    "tsv" => {
                        state.format = OutputFormat::Tsv;
                        println!("Output format is tsv.");
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
        "null" => {
            if let Some(v) = val {
                let stripped = strip_quotes(v);
                println!("Null display is \"{}\".", stripped);
                state.null_str = stripped;
            } else {
                println!("Null display is \"{}\".", state.null_str);
            }
        }
        "tuples_only" | "t" => {
            let next = match val {
                Some(_) => parse_bool_arg(val).unwrap_or(!state.tuples_only),
                None => !state.tuples_only,
            };
            state.tuples_only = next;
            println!("Tuples only is {}.", if state.tuples_only { "on" } else { "off" });
        }
        "fieldsep" | "f" => {
            if let Some(v) = val {
                let stripped = strip_quotes(v);
                println!("Field separator is \"{}\".", stripped);
                state.field_sep = stripped;
            } else {
                println!("Field separator is \"{}\".", state.field_sep);
            }
        }
        "border" => {
            if let Some(v) = val {
                if let Ok(b) = v.trim().parse::<u8>() {
                    state.border = b;
                    println!("Border style is {}.", b);
                } else {
                    println!("Border style must be a number (0, 1, 2).");
                }
            } else {
                println!("Border style is {}.", state.border);
            }
        }
        "expanded" | "x" => {
            let next = match val {
                Some(_) => parse_bool_arg(val).unwrap_or(state.format != OutputFormat::Expanded),
                None => state.format != OutputFormat::Expanded,
            };
            if next {
                state.format = OutputFormat::Expanded;
                println!("Expanded display is on.");
            } else {
                state.format = OutputFormat::Aligned;
                println!("Expanded display is off.");
            }
        }
        "pager" => {
            if let Some(v) = val {
                match v.to_lowercase().as_str() {
                    "always" => {
                        state.pager = PagerMode::Always;
                        println!("Pager is always used.");
                    }
                    "on" | "1" | "auto" | "true" | "yes" => {
                        state.pager = PagerMode::On;
                        println!("Pager is used for long output.");
                    }
                    "off" | "0" | "false" | "no" => {
                        state.pager = PagerMode::Off;
                        println!("Pager is off.");
                    }
                    _ => {
                        println!("Unrecognized \\pset pager option: {} (expected: on, off, always).", v);
                    }
                }
            } else {
                state.pager = match state.pager {
                    PagerMode::Off => PagerMode::On,
                    _ => PagerMode::Off,
                };
                match state.pager {
                    PagerMode::Off => println!("Pager is off."),
                    PagerMode::On => println!("Pager is used for long output."),
                    PagerMode::Always => println!("Pager is always used."),
                }
            }
        }
        _ => {
            println!("Unrecognized \\pset option: {}", opt);
        }
    }
}

fn strip_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if ((trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (trimmed.starts_with('"') && trimmed.ends_with('"')))
        && trimmed.len() >= 2
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

fn parse_bool_arg(arg: Option<&str>) -> Option<bool> {
    match arg.map(|s| s.to_lowercase()).as_deref() {
        Some("on") | Some("true") | Some("1") | Some("yes") => Some(true),
        Some("off") | Some("false") | Some("0") | Some("no") => Some(false),
        _ => None,
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
        .map_err(RsqlError::Io)?;

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
        } else if let Err(e) = crate::output::execute_and_display_query(&conn.client, trimmed, state, false).await {
            eprintln!("{}", format!("Error: {}", e).red());
        }
    }
    Ok(())
}
