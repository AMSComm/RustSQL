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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandedMode {
    Off,
    On,
    Auto,
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
    pub expanded: ExpandedMode,
    pub output_file: Option<String>,
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
            expanded: ExpandedMode::Off,
            output_file: None,
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
    ListMaterializedViews(Option<String>),
    ListIndexes(Option<String>, bool),
    ListSequences(Option<String>),
    ListFunctions(Option<String>, bool),
    ListTypes(Option<String>, bool),
    ListPrivileges(Option<String>),
    ListExtensions(Option<String>),
    ListExternalTables(Option<String>),
    ListExternalPartitions(Option<String>, String),
    ListDatashares,
    Describe(Option<String>, String),
    ShowFunction(String, bool),
    ListUsers,
    ToggleExpanded(Option<String>),
    ToggleTiming(Option<bool>),
    ToggleTuplesOnly(Option<bool>),
    Connect(String, Option<String>, Option<String>, Option<u16>),
    ConnInfo,
    Edit(Option<String>),
    Include(String),
    Set(String, String),
    Pset(String, Option<String>),
    Echo(String),
    ChangeDirectory(Option<String>),
    Shell(Option<String>),
    Out(Option<String>),
    Watch(Option<u64>),
}

pub fn parse_meta_command(line: &str) -> Option<MetaCommand> {
    let trimmed = line.trim();
    if !trimmed.starts_with('\\') {
        return None;
    }

    let without_slash = &trimmed[1..];
    if without_slash.starts_with('!') {
        let shell_cmd = without_slash[1..].trim();
        return Some(MetaCommand::Shell(if shell_cmd.is_empty() {
            None
        } else {
            Some(shell_cmd.to_string())
        }));
    }

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
        "dm" | "dm+" => Some(MetaCommand::ListMaterializedViews(args.first().map(|s| s.to_string()))),
        "di" | "di+" => {
            let verbose = cmd == "di+";
            Some(MetaCommand::ListIndexes(args.first().map(|s| s.to_string()), verbose))
        }
        "df" | "df+" => {
            let verbose = cmd == "df+";
            Some(MetaCommand::ListFunctions(args.first().map(|s| s.to_string()), verbose))
        }
        "ds" | "ds+" => Some(MetaCommand::ListSequences(args.first().map(|s| s.to_string()))),
        "dsh" => Some(MetaCommand::ListDatashares),
        "dT" | "dT+" | "dt_type" => {
            let verbose = cmd.ends_with('+');
            Some(MetaCommand::ListTypes(args.first().map(|s| s.to_string()), verbose))
        }
        "dp" | "z" => Some(MetaCommand::ListPrivileges(args.first().map(|s| s.to_string()))),
        "dx" | "dx+" => Some(MetaCommand::ListExtensions(args.first().map(|s| s.to_string()))),
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
        "d" | "d+" => {
            if let Some(target) = args.first() {
                if target.contains('*') || target.contains('?') {
                    Some(MetaCommand::ListTables(Some(target.to_string())))
                } else if target.contains('.') {
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
        "sf" | "sf+" => {
            if let Some(target) = args.first() {
                let verbose = cmd == "sf+";
                Some(MetaCommand::ShowFunction(target.to_string(), verbose))
            } else {
                println!("{}", "Usage: \\sf[+] <function_or_procedure_name>".yellow());
                None
            }
        }
        "du" | "du+" | "dg" | "dg+" => Some(MetaCommand::ListUsers),
        "cd" => Some(MetaCommand::ChangeDirectory(args.first().map(|s| s.to_string()))),
        "o" | "out" => Some(MetaCommand::Out(args.first().map(|s| s.to_string()))),
        "watch" => {
            let sec = args.first().and_then(|s| s.parse::<u64>().ok());
            Some(MetaCommand::Watch(sec))
        }
        "x" => Some(MetaCommand::ToggleExpanded(args.first().map(|s| s.to_string()))),
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
    // Auto-reconnect before DB-backed meta-commands if idle connection dropped
    let needs_db = !matches!(
        meta,
        MetaCommand::Quit
            | MetaCommand::HelpGeneral
            | MetaCommand::HelpSql(_)
            | MetaCommand::Connect(..)
            | MetaCommand::ConnInfo
            | MetaCommand::ToggleExpanded(_)
            | MetaCommand::ToggleTiming(_)
            | MetaCommand::ToggleTuplesOnly(_)
            | MetaCommand::Pset(..)
            | MetaCommand::Set(..)
            | MetaCommand::Echo(_)
            | MetaCommand::ChangeDirectory(_)
            | MetaCommand::Shell(_)
            | MetaCommand::Out(_)
    );
    if needs_db && conn.is_closed() {
        let _ = conn.reconnect().await;
    }

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
        MetaCommand::ListMaterializedViews(pattern) => {
            let sql = conn.dialect.list_materialized_views_query(pattern.as_deref());
            run_and_display_meta_query(conn, state, &sql).await?;
        }
        MetaCommand::ListIndexes(pattern, verbose) => {
            let sql = conn.dialect.list_indexes_query(pattern.as_deref(), verbose);
            run_and_display_meta_query(conn, state, &sql).await?;
        }
        MetaCommand::ListSequences(pattern) => {
            if conn.dialect.has_external_support() {
                if let Some(sql) = conn.dialect.list_datashares_query() {
                    println!("{}", "Listing Amazon Redshift Data Shares (use \\dsh or \\ds):".cyan().bold());
                    run_and_display_meta_query(conn, state, sql).await?;
                }
            } else {
                let sql = conn.dialect.list_sequences_query(pattern.as_deref());
                run_and_display_meta_query(conn, state, &sql).await?;
            }
        }
        MetaCommand::ListFunctions(pattern, verbose) => {
            let sql = conn.dialect.list_functions_query(pattern.as_deref(), verbose);
            match run_and_display_meta_query(conn, state, &sql).await {
                Ok(_) => {}
                Err(_) if conn.dialect.has_external_support() => {
                    let fallback = crate::dialect::redshift::fallback_redshift_functions_query(pattern.as_deref(), verbose);
                    run_and_display_meta_query(conn, state, &fallback).await?;
                }
                Err(e) => return Err(e),
            }
        }
        MetaCommand::ListTypes(pattern, verbose) => {
            let sql = conn.dialect.list_types_query(pattern.as_deref(), verbose);
            run_and_display_meta_query(conn, state, &sql).await?;
        }
        MetaCommand::ListPrivileges(pattern) => {
            let sql = conn.dialect.list_privileges_query(pattern.as_deref());
            run_and_display_meta_query(conn, state, &sql).await?;
        }
        MetaCommand::ListExtensions(_pattern) => {
            if let Some(sql) = conn.dialect.list_extensions_query() {
                run_and_display_meta_query(conn, state, sql).await?;
            } else {
                println!("{}", "Extensions are not supported on Amazon Redshift.".yellow());
            }
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
        MetaCommand::ShowFunction(func_name, verbose) => {
            handle_show_function(&func_name, verbose, conn).await?;
        }
        MetaCommand::ListUsers => {
            let sql = conn.dialect.list_users_query();
            run_and_display_meta_query(conn, state, sql).await?;
        }
        MetaCommand::ToggleExpanded(val) => {
            handle_expanded(val.as_deref(), state);
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
        MetaCommand::ChangeDirectory(dir) => {
            handle_cd(dir.as_deref())?;
        }
        MetaCommand::Shell(cmd) => {
            handle_shell(cmd.as_deref())?;
        }
        MetaCommand::Out(file) => {
            handle_out(file.as_deref(), state)?;
        }
        MetaCommand::Watch(seconds) => {
            handle_watch(seconds, conn, state).await?;
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
    res.display_target(
        state.format,
        state.timing,
        state.tuples_only,
        state.border,
        state.pager,
        state.expanded,
        state.output_file.as_deref(),
    )?;
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
    // 1. Try dialect describe table query
    let local_sql = conn.dialect.describe_table_query(schema, table);
    match conn.client.query(&local_sql, &[]).await {
        Ok(rows) if !rows.is_empty() => {
            let full_name = match schema {
                Some(s) => format!("{}.{}", s, table),
                None => table.to_string(),
            };
            println!("{}", format!("Table \"{}\":", full_name).cyan().bold());
            let res = QueryResult::from_rows(&rows, Duration::from_millis(0), None, &state.null_str);
            res.display_target(state.format, state.timing, state.tuples_only, state.border, PagerMode::Off, state.expanded, state.output_file.as_deref())?;
            return Ok(());
        }
        _ => {}
    }

    // 2. Fallback: Pure postgres catalog query (works on any PG and Redshift without dialect-specific columns)
    let schema_filter = if let Some(s) = schema {
        format!("AND (n.nspname = lower('{}') OR n.nspname = '{}' OR n.nspname ILIKE '{}')", s, s, s)
    } else {
        "AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg_temp_%'".to_string()
    };
    let fallback_sql = format!(
        "SELECT \
            a.attname AS \"Column\", \
            pg_catalog.format_type(a.atttypid, a.atttypmod) AS \"Type\", \
            CASE WHEN a.attnotnull THEN 'not null' ELSE '' END AS \"Nullable\", \
            COALESCE(pg_catalog.pg_get_expr(d.adbin, d.adrelid), '') AS \"Default\" \
         FROM pg_catalog.pg_attribute a \
         JOIN pg_catalog.pg_class c ON c.oid = a.attrelid \
         JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
         WHERE (c.relname = lower('{}') OR c.relname = '{}' OR c.relname ILIKE '{}') \
           {} \
           AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY \
            CASE WHEN pg_catalog.pg_table_is_visible(c.oid) THEN 0 ELSE 1 END, \
            a.attnum;",
        table, table, table, schema_filter
    );
    if let Ok(rows) = conn.client.query(&fallback_sql, &[]).await {
        if !rows.is_empty() {
            let full_name = match schema {
                Some(s) => format!("{}.{}", s, table),
                None => table.to_string(),
            };
            println!("{}", format!("Table \"{}\":", full_name).cyan().bold());
            let res = QueryResult::from_rows(&rows, Duration::from_millis(0), None, &state.null_str);
            res.display_target(state.format, state.timing, state.tuples_only, state.border, PagerMode::Off, state.expanded, state.output_file.as_deref())?;
            return Ok(());
        }
    }

    // 3. If Redshift, check external tables (Spectrum / Athena / Lakehouse)
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
                    res.display_target(state.format, state.timing, state.tuples_only, state.border, PagerMode::Off, state.expanded, state.output_file.as_deref())?;

                    // Also show partition location
                    if let Some(part_sql) = conn.dialect.list_external_partitions_query(schema, table) {
                        if let Ok(part_rows) = conn.client.query(&part_sql, &[]).await {
                            if !part_rows.is_empty() {
                                println!("\n{}", "Sample External Partitions:".cyan());
                                let part_res = QueryResult::from_rows(&part_rows, Duration::from_millis(0), None, &state.null_str);
                                part_res.display_target(state.format, false, state.tuples_only, state.border, PagerMode::Off, state.expanded, state.output_file.as_deref())?;
                            }
                        }
                    }
                    return Ok(());
                }
            }
        }

        // Also check svv_all_columns for Redshift (unifies local, view, external)
        let svv_sql = match schema {
            Some(s) => format!(
                "SELECT \
                    column_name AS \"Column\", \
                    data_type AS \"Type\", \
                    CASE WHEN is_nullable = 'NO' THEN 'not null' ELSE '' END AS \"Nullable\", \
                    COALESCE(column_default, '') AS \"Default\" \
                 FROM svv_all_columns \
                 WHERE (table_name = lower('{}') OR table_name = '{}' OR table_name ILIKE '{}') \
                   AND (schema_name = lower('{}') OR schema_name = '{}' OR schema_name ILIKE '{}') \
                 ORDER BY ordinal_position;",
                table, table, table, s, s, s
            ),
            None => format!(
                "SELECT \
                    schema_name AS \"Schema\", \
                    column_name AS \"Column\", \
                    data_type AS \"Type\", \
                    CASE WHEN is_nullable = 'NO' THEN 'not null' ELSE '' END AS \"Nullable\", \
                    COALESCE(column_default, '') AS \"Default\" \
                 FROM svv_all_columns \
                 WHERE (table_name = lower('{}') OR table_name = '{}' OR table_name ILIKE '{}') \
                   AND schema_name NOT IN ('pg_catalog', 'information_schema') \
                   AND schema_name NOT LIKE 'pg_temp_%' \
                 ORDER BY schema_name, ordinal_position;",
                table, table, table
            ),
        };
        if let Ok(rows) = conn.client.query(&svv_sql, &[]).await {
            if !rows.is_empty() {
                let full_name = match schema {
                    Some(s) => format!("{}.{}", s, table),
                    None => table.to_string(),
                };
                println!("{}", format!("Table \"{}\":", full_name).cyan().bold());
                let res = QueryResult::from_rows(&rows, Duration::from_millis(0), None, &state.null_str);
                res.display_target(state.format, state.timing, state.tuples_only, state.border, PagerMode::Off, state.expanded, state.output_file.as_deref())?;
                return Ok(());
            }
        }
    }

    // 4. Not found
    println!(
        "{}",
        format!("Did not find any relation named \"{}\".", table).yellow()
    );

    Ok(())
}

async fn handle_show_function(
    func_name: &str,
    verbose: bool,
    conn: &DbConnection,
) -> Result<()> {
    let sql = conn.dialect.show_function_query(func_name);
    if let Ok(rows) = conn.client.query(&sql, &[]).await {
        if !rows.is_empty() {
            let src: &str = rows[0].get(0);
            if verbose {
                for (idx, line) in src.lines().enumerate() {
                    println!("{:4}: {}", idx + 1, line);
                }
            } else {
                println!("{}", src);
            }
            return Ok(());
        }
    }

    // If on Redshift and not found in pg_proc, try SHOW PROCEDURE func_name
    if conn.dialect.has_external_support() {
        let show_proc = format!("SHOW PROCEDURE {};", func_name);
        if let Ok(rows) = conn.client.query(&show_proc, &[]).await {
            if !rows.is_empty() {
                let mut full_ddl = String::new();
                for r in rows {
                    let part: &str = r.get(0);
                    full_ddl.push_str(part);
                    full_ddl.push('\n');
                }
                if verbose {
                    for (idx, line) in full_ddl.lines().enumerate() {
                        println!("{:4}: {}", idx + 1, line);
                    }
                } else {
                    println!("{}", full_ddl);
                }
                return Ok(());
            }
        }
    }

    println!("{}", format!("Did not find any function or procedure named \"{}\".", func_name).yellow());
    Ok(())
}

fn handle_out(filepath: Option<&str>, state: &mut AppState) -> Result<()> {
    match filepath {
        Some(f) if !f.trim().is_empty() => {
            let path = f.trim();
            fs::File::create(path).map_err(RsqlError::Io)?;
            println!("Output redirected to \"{}\".", path);
            state.output_file = Some(path.to_string());
        }
        _ => {
            state.output_file = None;
            println!("Output redirected to stdout.");
        }
    }
    Ok(())
}

fn handle_shell(cmd: Option<&str>) -> Result<()> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut process = std::process::Command::new(&shell);
    if let Some(command) = cmd {
        process.arg("-c").arg(command);
    }
    let status = process.status().map_err(RsqlError::Io)?;
    if !status.success() {
        if let Some(code) = status.code() {
            eprintln!("Command failed with exit code {}", code);
        }
    }
    Ok(())
}

fn handle_cd(dir: Option<&str>) -> Result<()> {
    let target = match dir {
        Some(d) => {
            if d.starts_with('~') {
                if let Ok(home) = std::env::var("HOME") {
                    d.replacen('~', &home, 1)
                } else {
                    d.to_string()
                }
            } else {
                d.to_string()
            }
        }
        None => std::env::var("HOME").unwrap_or_else(|_| ".".to_string()),
    };
    std::env::set_current_dir(&target).map_err(RsqlError::Io)?;
    if let Ok(current) = std::env::current_dir() {
        println!("Current directory is {}", current.display());
    }
    Ok(())
}

async fn handle_watch(
    seconds: Option<u64>,
    conn: &mut DbConnection,
    state: &AppState,
) -> Result<()> {
    if state.last_query.trim().is_empty() {
        println!("No query buffer to watch.");
        return Ok(());
    }
    let interval = Duration::from_secs(seconds.unwrap_or(2));
    let sql = state.last_query.clone();

    println!("Watching query every {:?} (press Ctrl+C to cancel)...", interval);
    loop {
        let now_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        println!("{}", format!("---- {} (every {:?}) ----", now_str, interval).cyan());

        if let Err(e) = crate::output::execute_and_display_query(&conn.client, &sql, state, false).await {
            eprintln!("{}", format!("Query error: {}", e).red());
        }

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            _ = tokio::signal::ctrl_c() => {
                println!("\nWatch cancelled.");
                break;
            }
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
    println!("Client: ruql (RustSQL) {} (psql 17.4 compatibility)", env!("CARGO_PKG_VERSION"));
    println!("Server: {}", conn.server_version.trim());
    println!("Engine Dialect: {}", conn.dialect.dialect_name().cyan().bold());
}

fn print_general_help(has_external: bool) {
    println!("{}", "General ruql / psql meta-commands:".bold());
    println!("  \\q                 quit ruql");
    println!("  \\?                 show help on backslash commands");
    println!("  \\h [cmd]           help on SQL syntax");
    println!("  \\conninfo          display information about current connection");
    println!("  \\c [db] [usr] [h]  connect to new database");
    println!("  \\timing [on|off]   toggle timing of commands");
    println!("  \\x [on|off]        toggle expanded table output");
    println!("  \\t [on|off]        toggle tuples only (no headers/footers)");
    println!("  \\e [file]          edit the current query buffer in $EDITOR");
    println!("  \\i <file>          execute commands from file");
    println!("  \\o [file]          redirect query output to file (or stdout)");
    println!("  \\watch [sec]       execute query repeatedly every N seconds (default: 2)");
    println!("  \\cd [dir]          change current working directory");
    println!("  \\! [cmd]           execute shell command");
    println!("  \\pset <opt> [val]  set table options (format, null, tuples_only, fieldsep, border)");
    println!();
    println!("{}", "Informational Queries:".bold());
    println!("  \\l, \\list          list all databases");
    println!("  \\dn [pattern]      list schemas");
    println!("  \\dt, \\dt+ [pat]    list tables");
    println!("  \\dv, \\dv+ [pat]    list views");
    println!("  \\dm, \\dm+ [pat]    list materialized views");
    println!("  \\di, \\di+ [pat]    list indexes");
    println!("  \\ds, \\ds+ [pat]    list sequences");
    println!("  \\df, \\df+ [pat]    list functions and stored procedures");
    println!("  \\sf, \\sf+ <name>   show function / stored procedure source code");
    println!("  \\dT, \\dT+ [pat]    list data types");
    println!("  \\dp, \\z [pat]      list privileges / access grants");
    println!("  \\dx, \\dx+          list installed extensions");
    println!("  \\d <relation>      describe table columns, types, nullability");
    println!("  \\du, \\dg           list users and roles");

    if has_external {
        println!();
        println!("{}", "Amazon Redshift Lakehouse & Spectrum commands:".cyan().bold());
        println!("  \\dex [pattern]     list Redshift Spectrum external tables & S3 locations");
        println!("  \\dep <schema.tbl>  list partitions for Redshift external table");
        println!("  \\dsh, \\ds          list Redshift Data Shares (cross-cluster/lakehouse)");
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
            handle_expanded(val, state);
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

pub fn handle_expanded(val: Option<&str>, state: &mut AppState) {
    if let Some(v) = val {
        match v.to_lowercase().as_str() {
            "auto" => {
                state.expanded = ExpandedMode::Auto;
                println!("Expanded display is used automatically.");
            }
            "on" | "1" | "true" | "yes" => {
                state.expanded = ExpandedMode::On;
                println!("Expanded display is on.");
            }
            "off" | "0" | "false" | "no" => {
                state.expanded = ExpandedMode::Off;
                println!("Expanded display is off.");
            }
            _ => {
                println!("Unrecognized \\x option: {} (expected: on, off, auto).", v);
            }
        }
    } else {
        state.expanded = match state.expanded {
            ExpandedMode::Off => ExpandedMode::On,
            _ => ExpandedMode::Off,
        };
        match state.expanded {
            ExpandedMode::Off => println!("Expanded display is off."),
            ExpandedMode::On => println!("Expanded display is on."),
            ExpandedMode::Auto => println!("Expanded display is used automatically."),
        }
    }
}

pub async fn execute_script(
    content: &str,
    conn: &mut DbConnection,
    state: &mut AppState,
) -> Result<()> {
    let mut pending_sql = String::new();

    for line in content.lines() {
        if state.echo_all {
            println!("{}", line);
        }

        let trimmed = line.trim();
        if trimmed.starts_with('\\') {
            // Flush any pending SQL statements before executing meta command
            if !pending_sql.trim().is_empty() {
                let sql_to_run = std::mem::take(&mut pending_sql);
                crate::output::execute_and_display_query(&conn.client, &sql_to_run, state, false).await?;
            }

            if let Some(meta) = parse_meta_command(trimmed) {
                Box::pin(execute_meta_command(meta, conn, state)).await?;
            }
        } else {
            pending_sql.push_str(line);
            pending_sql.push('\n');
        }
    }

    // Flush any remaining SQL statements
    if !pending_sql.trim().is_empty() {
        crate::output::execute_and_display_query(&conn.client, &pending_sql, state, false).await?;
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
    execute_script(&content, conn, state).await
}
