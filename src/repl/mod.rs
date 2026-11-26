pub mod completer;
pub mod highlighter;
pub mod prompt;

use crate::connection::DbConnection;
use crate::error::Result;
use crate::meta::{execute_meta_command, parse_meta_command, AppState};
use colored::Colorize;
use completer::SqlCompleter;
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

    let completer = SqlCompleter::new();
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

pub async fn preload_cache(conn: &DbConnection, completer: &SqlCompleter) {
    let tbl_sql = conn.dialect.list_tables_query(None);
    let view_sql = conn.dialect.list_views_query(None);
    let schema_sql = conn.dialect.list_schemas_query(None);
    let db_sql = conn.dialect.list_databases_query();
    let col_sql = "SELECT DISTINCT attname FROM pg_attribute WHERE attnum > 0 AND NOT attisdropped ORDER BY 1 LIMIT 500;";

    let (res_tbl, res_view, res_schema, res_db, res_col) = tokio::join!(
        conn.client.query(&tbl_sql, &[]),
        conn.client.query(&view_sql, &[]),
        conn.client.query(&schema_sql, &[]),
        conn.client.query(db_sql, &[]),
        conn.client.query(col_sql, &[]),
    );

    let mut tables = Vec::new();
    if let Ok(rows) = res_tbl {
        for r in rows {
            let schema: Option<String> = r.try_get(0).ok();
            let name: Option<String> = r.try_get(1).ok();
            if let Some(n) = name {
                tables.push(n.clone());
                if let Some(s) = schema {
                    tables.push(format!("{}.{}", s, n));
                }
            }
        }
    }

    let mut views = Vec::new();
    if let Ok(rows) = res_view {
        for r in rows {
            let schema: Option<String> = r.try_get(0).ok();
            let name: Option<String> = r.try_get(1).ok();
            if let Some(n) = name {
                views.push(n.clone());
                if let Some(s) = schema {
                    views.push(format!("{}.{}", s, n));
                }
            }
        }
    }

    let mut schemas = Vec::new();
    if let Ok(rows) = res_schema {
        for r in rows {
            if let Ok(s) = r.try_get::<_, String>(0) {
                schemas.push(s);
            }
        }
    }

    let mut databases = Vec::new();
    if let Ok(rows) = res_db {
        for r in rows {
            if let Ok(db) = r.try_get::<_, String>(0) {
                databases.push(db);
            }
        }
    }

    let mut columns = Vec::new();
    if let Ok(rows) = res_col {
        for r in rows {
            if let Ok(c) = r.try_get::<_, String>(0) {
                columns.push(c);
            }
        }
    }

    completer.update_cache(tables, schemas, views, databases, columns);
}
