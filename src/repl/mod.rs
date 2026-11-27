pub mod completer;
pub mod highlighter;
pub mod prompt;

use crate::connection::DbConnection;
use crate::dialect::Dialect;
use crate::error::Result;
use crate::meta::{execute_meta_command, parse_meta_command, AppState};
use colored::Colorize;
use completer::{LoadRequest, SchemaLoader, SqlCompleter};
use highlighter::SqlHighlighter;
use prompt::PromptBuilder;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{
    Cmd, CompletionType, Config, Context, EditMode, Editor, Helper, KeyCode, KeyEvent, Modifiers,
    Movement,
};
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone)]
pub struct RsqlHelper {
    pub completer: SqlCompleter,
    pub highlighter: SqlHighlighter,
}

impl Helper for RsqlHelper {}

impl Completer for RsqlHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.completer.complete(line, pos, ctx)
    }
}

impl Highlighter for RsqlHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: CmdKind) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for RsqlHelper {}

impl Hinter for RsqlHelper {
    type Hint = String;
}

/// Configures psql-style history navigation and multiline cursor movement.
fn configure_keybindings(rl: &mut Editor<RsqlHelper, rustyline::history::DefaultHistory>) {
    // Match psql: Up/Down always navigate history even in multiline buffers.
    rl.bind_sequence(KeyEvent(KeyCode::Up, Modifiers::NONE), Cmd::PreviousHistory);
    rl.bind_sequence(KeyEvent(KeyCode::Down, Modifiers::NONE), Cmd::NextHistory);

    // Move cursor up/down within multiline queries using Alt+Up/Down or Ctrl+Up/Down.
    rl.bind_sequence(
        KeyEvent(KeyCode::Up, Modifiers::ALT),
        Cmd::Move(Movement::LineUp(1)),
    );
    rl.bind_sequence(
        KeyEvent(KeyCode::Down, Modifiers::ALT),
        Cmd::Move(Movement::LineDown(1)),
    );
    rl.bind_sequence(
        KeyEvent(KeyCode::Up, Modifiers::CTRL),
        Cmd::Move(Movement::LineUp(1)),
    );
    rl.bind_sequence(
        KeyEvent(KeyCode::Down, Modifiers::CTRL),
        Cmd::Move(Movement::LineDown(1)),
    );
}

pub async fn run_interactive_repl(
    mut conn: DbConnection,
    mut state: AppState,
) -> Result<()> {
    let history_path = dirs::home_dir().map(|h| h.join(".ruql_history"));

    let (schema_tx, mut schema_rx) = tokio::sync::mpsc::unbounded_channel::<LoadRequest>();
    let schema_notify = Arc::new((Mutex::new(()), Condvar::new()));
    let completer = SqlCompleter::with_loader(SchemaLoader {
        tx: schema_tx,
        notify: Arc::clone(&schema_notify),
    });

    let (conn_watch_tx, conn_watch_rx) =
        tokio::sync::watch::channel((Arc::clone(&conn.client), Arc::clone(&conn.dialect)));

    let is_query_running = Arc::new(AtomicBool::new(false));
    let keepalive_needed = Arc::new(AtomicBool::new(false));
    let mut session_tracker = SessionTracker::default();

    // Application-level keepalive heartbeat: only active while session holds ephemeral state
    // (temp tables or open transactions); otherwise allow idle disconnect and auto-reconnect.
    let hb_interval_secs = std::env::var("PGKEEPALIVE_QUERY_INTERVAL")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(45);
    if hb_interval_secs > 0 {
        let hb_watch_rx = conn_watch_rx.clone();
        let hb_running = Arc::clone(&is_query_running);
        let hb_needed = Arc::clone(&keepalive_needed);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(hb_interval_secs));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval.tick().await; // skip immediate first tick
            loop {
                interval.tick().await;
                if hb_running.load(Ordering::Relaxed) || !hb_needed.load(Ordering::Relaxed) {
                    continue;
                }
                let client = {
                    let guard = hb_watch_rx.borrow();
                    Arc::clone(&guard.0)
                };
                if !client.is_closed() {
                    let _ = client.simple_query("").await;
                }
            }
        });
    }

    // Background lazy loader for non-default schemas and table columns
    let bg_completer = completer.clone();
    let bg_notify = Arc::clone(&schema_notify);
    tokio::spawn(async move {
        while let Some(req) = schema_rx.recv().await {
            let (client, dialect) = {
                let guard = conn_watch_rx.borrow();
                (Arc::clone(&guard.0), Arc::clone(&guard.1))
            };

            match req {
                LoadRequest::Schema(schema) => {
                    let schema_lower = schema.trim().to_lowercase();
                    let skip = bg_completer
                        .cache
                        .read()
                        .map(|c| c.catalog_access_denied || c.loaded_schemas.contains(&schema_lower))
                        .unwrap_or(true);
                    if !skip {
                        match fetch_schema_objects(&client, &*dialect, &schema, false).await {
                            Ok((tables, views, functions)) => {
                                bg_completer.merge_schema_objects(&schema, tables, views, functions);
                            }
                            Err(true) => bg_completer.mark_catalog_access_denied(),
                            Err(false) => bg_completer.mark_schema_loaded(&schema),
                        }
                    }
                }
                LoadRequest::Table(table) => {
                    let table_lower = table.trim().to_lowercase();
                    let skip = bg_completer
                        .cache
                        .read()
                        .map(|c| c.catalog_access_denied || c.loaded_tables.contains(&table_lower))
                        .unwrap_or(true);
                    if !skip {
                        match fetch_table_columns(&client, &*dialect, &table).await {
                            Ok(cols) => bg_completer.set_table_columns(&table, cols),
                            Err(true) => bg_completer.mark_catalog_access_denied(),
                            Err(false) => bg_completer.mark_table_loaded(&table),
                        }
                    }
                }
            }
            bg_notify.1.notify_all();
        }
    });

    let helper = RsqlHelper {
        completer: completer.clone(),
        highlighter: SqlHighlighter,
    };

    let config = Config::builder()
        .completion_type(CompletionType::List)
        .completion_prompt_limit(100)
        .edit_mode(EditMode::Emacs)
        .auto_add_history(false)
        .build();

    let mut rl = Editor::with_config(config)
        .map_err(|e| crate::error::RsqlError::Other(e.to_string()))?;
    rl.set_helper(Some(helper));
    configure_keybindings(&mut rl);

    if let Some(ref path) = history_path {
        if path.exists() {
            let _ = rl.load_history(path);
        } else if let Some(home) = dirs::home_dir() {
            let rsql_hist = home.join(".rsql_history");
            let psql_hist = home.join(".psql_history");
            if rsql_hist.exists() {
                let _ = rl.load_history(&rsql_hist);
            } else if psql_hist.exists() {
                let _ = rl.load_history(&psql_hist);
            }
        }
    }

    // Print welcome banner (matching psql 17.4 compatibility)
    let client_version = "17.4";
    let server_ver = crate::connection::parse_server_short_version(
        &conn.server_version,
        conn.dialect.has_external_support(),
    );
    if server_ver == client_version {
        println!("psql ({})", client_version);
    } else {
        println!("psql ({}, server {})", client_version, server_ver);
    }
    println!("Type \"help\" for help.");
    println!();

    // Check for updates non-blockingly in background
    tokio::spawn(async move {
        if let Ok(Some(release)) = crate::updater::check_latest_release().await {
            let cur = env!("CARGO_PKG_VERSION");
            if crate::updater::is_newer_version(&release.tag_name, cur) {
                eprintln!(
                    "{} A new version of ruql is available: {} (current: v{}). Run \"ruql --update\" to update.\n",
                    "[UPDATE]".yellow().bold(),
                    release.tag_name.green().bold(),
                    cur
                );
            }
        }
    });

    // Preload auto-complete cache
    preload_cache(&conn, &completer).await;

    let mut query_buffer = String::new();

    loop {
        let prompt_builder = PromptBuilder::new(conn.config.dbname.clone(), conn.is_superuser);
        let prompt_str = if query_buffer.is_empty() {
            prompt_builder.primary_prompt()
        } else {
            prompt_builder.continuation_prompt()
        };

        match rl.readline(&prompt_str) {
            Ok(line) => {
                let trimmed = line.trim();

                // If empty buffer and empty line, continue
                if query_buffer.is_empty() && trimmed.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line.as_str());

                // Check meta-command if at start of statement
                if query_buffer.is_empty() && trimmed.starts_with('\\') {
                    if let Some(meta) = parse_meta_command(trimmed) {
                        let is_connect = matches!(meta, crate::meta::MetaCommand::Connect(..));
                        match execute_meta_command(meta, &mut conn, &mut state).await {
                            Ok(true) => {
                                if is_connect {
                                    session_tracker.on_reconnect();
                                    keepalive_needed.store(false, Ordering::Relaxed);
                                    let _ = conn_watch_tx.send((
                                        Arc::clone(&conn.client),
                                        Arc::clone(&conn.dialect),
                                    ));
                                    preload_cache(&conn, &completer).await;
                                }
                                continue;
                            }
                            Ok(false) => break, // Quit
                            Err(e) => {
                                eprintln!("{}", format!("{}", e).red());
                                continue;
                            }
                        }
                    } else {
                        eprintln!("Invalid command \\{}. Try \\? for help.", &trimmed[1..]);
                        continue;
                    }
                }

                // If "help", treat as \?
                if query_buffer.is_empty() && trimmed.eq_ignore_ascii_case("help") {
                    let _ = execute_meta_command(
                        crate::meta::MetaCommand::HelpGeneral,
                        &mut conn,
                        &mut state,
                    )
                    .await;
                    continue;
                }

                // Append line to buffer
                if !query_buffer.is_empty() {
                    query_buffer.push(' ');
                }
                query_buffer.push_str(trimmed);

                // Check if query is complete (ends with semicolon)
                if query_buffer.ends_with(';') {
                    let full_query = query_buffer.trim().to_string();
                    query_buffer.clear();
                    state.last_query = full_query.clone();

                    is_query_running.store(true, Ordering::Relaxed);
                    let outcome = execute_query_with_session(
                        &mut conn,
                        &full_query,
                        &state,
                        session_tracker.needs_keepalive(),
                    )
                    .await;
                    is_query_running.store(false, Ordering::Relaxed);

                    if outcome.reconnected {
                        session_tracker.on_reconnect();
                    }
                    if outcome.succeeded {
                        session_tracker.on_query(&full_query);
                    }
                    keepalive_needed.store(session_tracker.needs_keepalive(), Ordering::Relaxed);

                    let _ = conn_watch_tx.send((
                        Arc::clone(&conn.client),
                        Arc::clone(&conn.dialect),
                    ));
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C: Cancel current multi-line input
                if !query_buffer.is_empty() {
                    query_buffer.clear();
                    println!("{}", "^C (query buffer cleared)".yellow());
                } else {
                    println!("{}", "^C".yellow());
                }
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D: Exit
                println!("\\q");
                break;
            }
            Err(err) => {
                eprintln!("Readline error: {:?}", err);
                break;
            }
        }
    }

    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }

    Ok(())
}

#[derive(Default, Debug, Clone)]
pub struct SessionTracker {
    pub has_temp_table: bool,
    pub in_transaction: bool,
}

impl SessionTracker {
    pub fn needs_keepalive(&self) -> bool {
        self.has_temp_table || self.in_transaction
    }

    pub fn on_reconnect(&mut self) {
        self.has_temp_table = false;
        self.in_transaction = false;
    }

    pub fn on_query(&mut self, query: &str) {
        let upper = query.to_uppercase();
        if upper.contains("CREATE TEMP ")
            || upper.contains("CREATE TEMPORARY ")
            || upper.contains("INTO TEMP ")
            || upper.contains("INTO TEMPORARY ")
        {
            self.has_temp_table = true;
        }

        // Check transaction boundaries across statements in the batch
        for stmt in upper.split(';') {
            let trimmed = strip_leading_comments(stmt).trim_start();
            if trimmed.starts_with("BEGIN") || trimmed.starts_with("START TRANSACTION") {
                self.in_transaction = true;
            } else if trimmed.starts_with("COMMIT")
                || trimmed.starts_with("ROLLBACK")
                || trimmed.starts_with("END")
            {
                self.in_transaction = false;
            }
        }
    }
}

fn strip_leading_comments(mut s: &str) -> &str {
    loop {
        s = s.trim_start();
        if let Some(rest) = s.strip_prefix("--") {
            match rest.find('\n') {
                Some(idx) => s = &rest[idx + 1..],
                None => return "",
            }
        } else if let Some(rest) = s.strip_prefix("/*") {
            match rest.find("*/") {
                Some(idx) => s = &rest[idx + 2..],
                None => return "",
            }
        } else {
            return s;
        }
    }
}

pub fn is_read_only_query(query: &str) -> bool {
    let cleaned = strip_leading_comments(query).trim_start();
    let first_word = cleaned
        .split(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .next()
        .unwrap_or("")
        .to_uppercase();

    match first_word.as_str() {
        "SELECT" | "SHOW" | "VALUES" => true,
        "WITH" => {
            let upper = cleaned.to_uppercase();
            !upper.contains("INSERT ") && !upper.contains("UPDATE ") && !upper.contains("DELETE ")
        }
        "EXPLAIN" => {
            let upper = cleaned.to_uppercase();
            !upper.contains("INSERT ") && !upper.contains("UPDATE ") && !upper.contains("DELETE ")
        }
        _ => false,
    }
}

pub fn is_transport_disconnect_error(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    // Never treat server-side query kills / statement timeouts as transport disconnects
    if lower.contains("canceling statement")
        || lower.contains("57014")
        || lower.contains("statement timeout")
        || lower.contains("administrator command")
        || lower.contains("out of memory")
    {
        return false;
    }
    lower.contains("connection closed")
        || lower.contains("broken pipe")
        || lower.contains("connection reset")
        || lower.contains("unexpected eof")
        || lower.contains("closed")
}

struct QueryOutcome {
    succeeded: bool,
    reconnected: bool,
}

pub async fn execute_query_with_cancel(
    conn: &mut DbConnection,
    query: &str,
    state: &AppState,
) {
    let _ = execute_query_with_session(conn, query, state, false).await;
}

async fn run_cancellable_query(
    conn: &DbConnection,
    query: &str,
    state: &AppState,
) -> Option<std::result::Result<(), crate::error::RsqlError>> {
    let cancel_token = conn.client.cancel_token();
    let is_ssl = conn.is_ssl;
    let ssl_mode = conn.config.ssl_mode;
    let mut cancel_sent = false;

    let mut query_fut = std::pin::pin!(crate::output::execute_and_display_query(
        &conn.client,
        query,
        state,
        true
    ));
    let mut res = None;

    while res.is_none() {
        tokio::select! {
            r = &mut query_fut => {
                res = Some(r);
            }
            _ = tokio::signal::ctrl_c(), if !cancel_sent => {
                cancel_sent = true;
                eprintln!("{}", "Cancel request sent".yellow());
                let cancel_token_clone = cancel_token.clone();
                tokio::spawn(async move {
                    if is_ssl {
                        if let Ok(connector) = crate::connection::create_tls_connector(ssl_mode) {
                            let _ = cancel_token_clone.cancel_query(connector).await;
                        } else {
                            let _ = cancel_token_clone.cancel_query(tokio_postgres::NoTls).await;
                        }
                    } else {
                        let _ = cancel_token_clone.cancel_query(tokio_postgres::NoTls).await;
                    }
                });
            }
            _ = tokio::signal::ctrl_c(), if cancel_sent => {
                eprintln!("{}", "^C (forced abort)".red());
                break;
            }
        }
    }
    res
}

async fn execute_query_with_session(
    conn: &mut DbConnection,
    query: &str,
    state: &AppState,
    had_ephemeral_state: bool,
) -> QueryOutcome {
    let mut reconnected = false;

    if conn.is_closed() {
        eprintln!("{}", "The connection to the server was lost. Attempting reset: ...".yellow());
        match conn.reconnect().await {
            Ok(()) => {
                reconnected = true;
                eprintln!("{}", "Reset: Succeeded.".green().bold());
                if had_ephemeral_state {
                    eprintln!(
                        "{}",
                        "Note: Previous session state (temp tables / transaction) was lost on disconnect."
                            .yellow()
                    );
                }
            }
            Err(e) => {
                eprintln!("{}", format!("Reset: Failed: {}", e).red().bold());
                crate::logger::log_error("Connection reset failed", &e);
                return QueryOutcome {
                    succeeded: false,
                    reconnected: false,
                };
            }
        }
    }

    let query_res = run_cancellable_query(conn, query, state).await;

    match query_res {
        Some(Ok(())) => QueryOutcome {
            succeeded: true,
            reconnected,
        },
        None => QueryOutcome {
            succeeded: false,
            reconnected,
        },
        Some(Err(e)) => {
            let err_msg = format!("{}", e);
            if err_msg.contains("canceling statement due to user request")
                || err_msg.contains("57014")
                || err_msg.contains("query was canceled")
            {
                eprintln!("{}", "Query canceled (Ctrl+C)".yellow().bold());
                return QueryOutcome {
                    succeeded: false,
                    reconnected,
                };
            }

            let lost_conn = conn.is_closed() || is_transport_disconnect_error(&err_msg);
            if lost_conn {
                eprintln!("{}", "The connection to the server was lost. Attempting reset: ...".yellow());
                match conn.reconnect().await {
                    Ok(()) => {
                        reconnected = true;
                        let can_auto_retry = !had_ephemeral_state && is_read_only_query(query);
                        if can_auto_retry {
                            eprintln!(
                                "{}",
                                "Reset: Succeeded. Retrying read-only query...".green().bold()
                            );
                            return match run_cancellable_query(conn, query, state).await {
                                Some(Ok(())) => QueryOutcome {
                                    succeeded: true,
                                    reconnected: true,
                                },
                                Some(Err(retry_err)) => {
                                    eprintln!("{}", format!("ERROR: {}", retry_err).red().bold());
                                    crate::logger::log_error(
                                        "Query execution error (after retry)",
                                        &format!("{}\nSQL: {}", retry_err, query),
                                    );
                                    QueryOutcome {
                                        succeeded: false,
                                        reconnected: true,
                                    }
                                }
                                None => QueryOutcome {
                                    succeeded: false,
                                    reconnected: true,
                                },
                            };
                        } else {
                            eprintln!("{}", format!("ERROR: {}", e).red().bold());
                            eprintln!("{}", "Reset: Succeeded.".green().bold());
                            if had_ephemeral_state {
                                eprintln!(
                                    "{}",
                                    "Note: Previous session state (temp tables / transaction) was lost; please re-run your statements."
                                        .yellow()
                                );
                            }
                        }
                    }
                    Err(re_err) => {
                        eprintln!("{}", format!("ERROR: {}", e).red().bold());
                        eprintln!("{}", format!("Reset: Failed: {}", re_err).red().bold());
                        crate::logger::log_error("Connection reset failed", &re_err);
                    }
                }
            } else {
                eprintln!("{}", format!("ERROR: {}", e).red().bold());
                crate::logger::log_error("Query execution error", &format!("{}\nSQL: {}", e, query));
            }

            QueryOutcome {
                succeeded: false,
                reconnected,
            }
        }
    }
}

fn is_permission_error(err: &tokio_postgres::Error) -> bool {
    if err.code() == Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE) {
        return true;
    }
    let msg = err.to_string().to_lowercase();
    msg.contains("permission denied")
        || msg.contains("must be superuser")
        || msg.contains("access denied")
}

fn collect_schema_objects(
    rows: std::result::Result<Vec<tokio_postgres::Row>, tokio_postgres::Error>,
    include_unqualified: bool,
) -> std::result::Result<Vec<String>, bool> {
    match rows {
        Ok(rows) => {
            let mut items = Vec::with_capacity(rows.len() * if include_unqualified { 2 } else { 1 });
            for r in rows {
                let schema: Option<String> = r.try_get(0).ok();
                let name: Option<String> = r.try_get(1).ok();
                if let Some(n) = name {
                    if include_unqualified {
                        items.push(n.clone());
                    }
                    if let Some(s) = schema {
                        items.push(format!("{}.{}", s, n));
                    }
                }
            }
            Ok(items)
        }
        Err(e) => Err(is_permission_error(&e)),
    }
}

/// Fetches column names for a single table (supports `schema.table` or `table`).
/// Returns `Err(true)` if catalog access was denied by permissions.
async fn fetch_table_columns(
    client: &tokio_postgres::Client,
    dialect: &dyn Dialect,
    table: &str,
) -> std::result::Result<Vec<String>, bool> {
    let clean = table.trim().replace('\'', "''");
    let (schema, tbl_name) = match clean.split_once('.') {
        Some((s, t)) => (Some(s), t),
        None => (None, clean.as_str()),
    };

    let sql = dialect.describe_table_query(schema, tbl_name);
    let mut cols = Vec::new();
    match client.query(&sql, &[]).await {
        Ok(rows) => {
            for r in rows {
                if let Ok(col) = r.try_get::<_, String>(0) {
                    cols.push(col);
                }
            }
        }
        Err(e) => return Err(is_permission_error(&e)),
    }

    if cols.is_empty() {
        if let Some(ext_sql) = dialect.describe_external_table_query(schema, tbl_name) {
            if let Ok(rows) = client.query(&ext_sql, &[]).await {
                for r in rows {
                    if let Ok(col) = r.try_get::<_, String>(0) {
                        cols.push(col);
                    }
                }
            }
        }
    }

    Ok(cols)
}

/// Fetches tables, views, and functions/procedures for a single schema.
/// Returns `Err(true)` if catalog access was denied by permissions.
async fn fetch_schema_objects(
    client: &tokio_postgres::Client,
    dialect: &dyn Dialect,
    schema: &str,
    include_unqualified: bool,
) -> std::result::Result<(Vec<String>, Vec<String>, Vec<String>), bool> {
    let pattern = format!("{}.*", schema.replace('\'', "''"));
    let tbl_sql = dialect.list_tables_query(Some(&pattern));
    let view_sql = dialect.list_views_query(Some(&pattern));
    let fn_sql = dialect.list_functions_query(Some(&pattern), false);

    let (res_tbl, res_view, res_fn) = tokio::join!(
        client.query(&tbl_sql, &[]),
        client.query(&view_sql, &[]),
        client.query(&fn_sql, &[]),
    );

    let tables = match collect_schema_objects(res_tbl, include_unqualified) {
        Ok(v) => v,
        Err(true) => return Err(true),
        Err(false) => Vec::new(),
    };
    let views = match collect_schema_objects(res_view, include_unqualified) {
        Ok(v) => v,
        Err(true) => return Err(true),
        Err(false) => Vec::new(),
    };
    let functions = match collect_schema_objects(res_fn, include_unqualified) {
        Ok(v) => v,
        Err(true) => return Err(true),
        Err(false) => Vec::new(),
    };

    Ok((tables, views, functions))
}

pub async fn preload_cache(conn: &DbConnection, completer: &SqlCompleter) {
    completer.reset_cache();

    // Determine default schema first; bail out silently if catalog access is denied.
    let default_schema = match conn.client.query("SELECT current_schema();", &[]).await {
        Ok(rows) => rows
            .first()
            .and_then(|r| r.try_get::<_, Option<String>>(0).ok())
            .flatten()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "public".to_string()),
        Err(e) => {
            if is_permission_error(&e) {
                completer.mark_catalog_access_denied();
            }
            return;
        }
    };

    let schema_sql = conn.dialect.list_schemas_query(None);
    let db_sql = conn.dialect.list_databases_query();

    let (obj_res, res_schema, res_db) = tokio::join!(
        fetch_schema_objects(&conn.client, &*conn.dialect, &default_schema, true),
        conn.client.query(&schema_sql, &[]),
        conn.client.query(db_sql, &[]),
    );

    let (tables, views, functions) = match obj_res {
        Ok(objs) => objs,
        Err(true) => {
            completer.mark_catalog_access_denied();
            return;
        }
        Err(false) => (Vec::new(), Vec::new(), Vec::new()),
    };

    let mut schemas = Vec::new();
    match res_schema {
        Ok(rows) => {
            for r in rows {
                if let Ok(s) = r.try_get::<_, String>(0) {
                    schemas.push(s);
                }
            }
        }
        Err(e) if is_permission_error(&e) => {
            completer.mark_catalog_access_denied();
        }
        Err(_) => {}
    }

    let mut databases = Vec::new();
    if let Ok(rows) = res_db {
        for r in rows {
            if let Ok(db) = r.try_get::<_, String>(0) {
                databases.push(db);
            }
        }
    }

    completer.update_cache(tables, schemas, views, databases, Vec::new());
    completer.set_functions(functions);
    completer.mark_schema_loaded(&default_schema);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_tracker_temp_and_tx() {
        let mut tracker = SessionTracker::default();
        assert!(!tracker.needs_keepalive());

        tracker.on_query("SELECT * FROM users;");
        assert!(!tracker.needs_keepalive());

        tracker.on_query("BEGIN;");
        assert!(tracker.needs_keepalive());

        tracker.on_query("COMMIT;");
        assert!(!tracker.needs_keepalive());

        tracker.on_query("CREATE TEMP TABLE tmp_ids (id int);");
        assert!(tracker.needs_keepalive());

        tracker.on_reconnect();
        assert!(!tracker.needs_keepalive());
    }

    #[test]
    fn test_is_read_only_query() {
        assert!(is_read_only_query("SELECT * FROM users;"));
        assert!(is_read_only_query("-- comment\nWITH cte AS (SELECT 1) SELECT * FROM cte;"));
        assert!(is_read_only_query("SHOW search_path;"));
        assert!(is_read_only_query("EXPLAIN SELECT 1;"));

        assert!(!is_read_only_query("INSERT INTO users VALUES (1);"));
        assert!(!is_read_only_query("UPDATE users SET name = 'a';"));
        assert!(!is_read_only_query("DELETE FROM users;"));
        assert!(!is_read_only_query("CREATE TEMP TABLE t (id int);"));
        assert!(!is_read_only_query("WITH d AS (DELETE FROM t) SELECT 1;"));
    }

    #[test]
    fn test_is_transport_disconnect_error() {
        assert!(is_transport_disconnect_error("connection closed"));
        assert!(is_transport_disconnect_error("Broken pipe"));
        assert!(is_transport_disconnect_error("Connection reset by peer"));

        assert!(!is_transport_disconnect_error(
            "ERROR: canceling statement due to statement timeout (57014)"
        ));
        assert!(!is_transport_disconnect_error(
            "FATAL: terminating connection due to administrator command"
        ));
    }
}
