use clap::Parser;
use rsql::cli::CliArgs;
use rsql::config::ConnectionConfig;
use rsql::connection::DbConnection;
use rsql::error::Result;
use rsql::meta::{execute_meta_command, parse_meta_command, AppState};
use rsql::output::{OutputFormat, QueryResult};
use rsql::rc::load_startup_rc_files;
use rsql::repl::run_interactive_repl;
use std::fs;
use std::process::exit;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    // Validation: --csv and --tsv require -c/--command
    if (args.csv || args.tsv) && args.command.is_none() {
        eprintln!("rsql: error: --csv and --tsv require -c/--command to be specified");
        exit(1);
    }

    if let Err(e) = run_app(args).await {
        eprintln!("rsql: {}", e);
        exit(1);
    }
}

async fn run_app(args: CliArgs) -> Result<()> {
    let config = ConnectionConfig::from_cli(&args)?;
    let mut conn = DbConnection::connect(config).await?;

    let mut state = AppState::default();

    // Load startup rc files unless -X / --no-psqlrc is specified
    if !args.no_psqlrc {
        load_startup_rc_files(&mut conn, &mut state).await;
    }

    // CLI flags override rc file settings
    if args.csv {
        state.format = OutputFormat::Csv;
    } else if args.tsv {
        state.format = OutputFormat::Tsv;
    } else if args.json {
        state.format = OutputFormat::Json;
    } else if args.markdown {
        state.format = OutputFormat::Markdown;
    } else if args.expanded {
        state.format = OutputFormat::Expanded;
    }

    if args.no_header {
        state.tuples_only = true;
    }

    state.quiet = args.quiet;
    state.echo_all = args.echo_all;

    for var_def in &args.set_vars {
        if let Some((k, v)) = var_def.split_once('=') {
            state.vars.insert(k.to_string(), v.to_string());
        }
    }

    // Mode 1: Run single command (-c)
    if let Some(command) = &args.command {
        let trimmed = command.trim();
        if trimmed.starts_with('\\') {
            if let Some(meta) = parse_meta_command(trimmed) {
                execute_meta_command(meta, &mut conn, &mut state).await?;
                return Ok(());
            }
        }

        let start = Instant::now();
        let messages = conn.client.simple_query(trimmed).await?;
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
                        vals.push(r.get(i).unwrap_or(&state.null_str).to_string());
                    }
                    let res = QueryResult {
                        columns: cols,
                        rows: vec![vals],
                        command_tag: None,
                        rows_affected: Some(1),
                        duration,
                    };
                    res.display(state.format, state.timing, state.tuples_only, false)?;
                }
                tokio_postgres::SimpleQueryMessage::CommandComplete(tag) if !state.quiet && !state.tuples_only => {
                    println!("{}", tag);
                }
                _ => {}
            }
        }
        return Ok(());
    }

    // Mode 2: Run script from file (-f)
    if let Some(file_path) = &args.file {
        let content = fs::read_to_string(file_path)?;
        for stmt in content.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }

            if state.echo_all {
                println!("{}", trimmed);
            }

            if trimmed.starts_with('\\') {
                if let Some(meta) = parse_meta_command(trimmed) {
                    execute_meta_command(meta, &mut conn, &mut state).await?;
                }
            } else {
                let start = Instant::now();
                let messages = conn.client.simple_query(trimmed).await?;
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
                                vals.push(r.get(i).unwrap_or(&state.null_str).to_string());
                            }
                            let res = QueryResult {
                                columns: cols,
                                rows: vec![vals],
                                command_tag: None,
                                rows_affected: Some(1),
                                duration,
                            };
                            res.display(state.format, state.timing, state.tuples_only, false)?;
                        }
                        tokio_postgres::SimpleQueryMessage::CommandComplete(tag) if !state.quiet && !state.tuples_only => {
                            println!("{}", tag);
                        }
                        _ => {}
                    }
                }
            }
        }
        return Ok(());
    }

    // Mode 3: Interactive REPL
    run_interactive_repl(conn, state).await?;

    Ok(())
}
