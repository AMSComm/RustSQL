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

                    execute_query_with_cancel(&mut conn, &full_query, &state).await;
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

pub async fn execute_query_with_cancel(
    conn: &mut DbConnection,
    query: &str,
    state: &AppState,
) {
    if conn.is_closed() {
        eprintln!("{}", "The connection to the server was lost. Attempting reset: ...".yellow());
        match conn.reconnect().await {
            Ok(()) => {
                eprintln!("{}", "Reset: Succeeded.".green().bold());
            }
            Err(e) => {
                eprintln!("{}", format!("Reset: Failed: {}", e).red().bold());
                crate::logger::log_error("Connection reset failed", &e);
                return;
            }
        }
    }

    let cancel_token = conn.client.cancel_token();
    let is_ssl = conn.is_ssl;
    let ssl_mode = conn.config.ssl_mode;
    let mut cancel_sent = false;

    let query_res = {
        let mut query_fut = std::pin::pin!(crate::output::execute_and_display_query(&conn.client, query, state, true));
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
    };

    if let Some(Err(e)) = query_res {
        let err_msg = format!("{}", e);
        if err_msg.contains("canceling statement due to user request")
            || err_msg.contains("57014")
            || err_msg.contains("query was canceled")
        {
            eprintln!("{}", "Query canceled (Ctrl+C)".yellow().bold());
        } else {
            eprintln!("{}", format!("ERROR: {}", e).red().bold());
            crate::logger::log_error("Query execution error", &format!("{}\nSQL: {}", e, query));

            if conn.is_closed() {
                eprintln!("{}", "The connection to the server was lost. Attempting reset: ...".yellow());
                match conn.reconnect().await {
                    Ok(()) => eprintln!("{}", "Reset: Succeeded.".green().bold()),
                    Err(re_err) => {
                        eprintln!("{}", format!("Reset: Failed: {}", re_err).red().bold());
                        crate::logger::log_error("Connection reset failed", &re_err);
                    }
                }
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
