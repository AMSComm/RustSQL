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
use rustyline::{Context, Editor, Helper};
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

pub async fn run_interactive_repl(
    mut conn: DbConnection,
    mut state: AppState,
) -> Result<()> {
    let history_path = dirs::home_dir().map(|h| h.join(".rsql_history"));

    let completer = SqlCompleter::new();
    let helper = RsqlHelper {
        completer: completer.clone(),
        highlighter: SqlHighlighter,
    };

    let mut rl = Editor::new().map_err(|e| crate::error::RsqlError::Other(e.to_string()))?;
    rl.set_helper(Some(helper));

    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    // Print welcome banner
    println!(
        "{} (0.1.0)",
        "rsql - Next-Gen PostgreSQL & Amazon Redshift CLI".bold()
    );
    println!("Type \"help\" or \"\\?\" for help. Type \"\\q\" to exit.");
    println!(
        "Connected to [{}] as user \"{}\".",
        conn.dialect.dialect_name().cyan().bold(),
        conn.config.user.green().bold()
    );
    println!();

    // Background preload auto-complete cache
    let completer_clone = completer.clone();
    let tables_sql = conn.dialect.list_tables_query(None);
    if let Ok(rows) = conn.client.query(&tables_sql, &[]).await {
        let mut objs = Vec::new();
        for r in rows {
            if let Ok(name) = r.try_get::<_, String>(1) {
                objs.push(name);
            }
        }
        completer_clone.add_objects(objs);
    }

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
                        match execute_meta_command(meta, &mut conn, &mut state).await {
                            Ok(true) => continue,
                            Ok(false) => break, // Quit
                            Err(e) => {
                                eprintln!("{}", format!("Error: {}", e).red());
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

                    if let Err(e) = crate::output::execute_and_display_query(&conn.client, &full_query, &state, true).await {
                        eprintln!("{}", format!("ERROR: {}", e).red().bold());
                    }
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
