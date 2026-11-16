use clap::Parser;
use rsql::cli::CliArgs;
use rsql::config::ConnectionConfig;
use rsql::connection::DbConnection;
use rsql::error::Result;
use rsql::meta::{execute_meta_command, parse_meta_command, AppState};
use rsql::output::{execute_and_display_query, OutputFormat};
use rsql::rc::load_startup_rc_files;
use rsql::repl::run_interactive_repl;
use std::fs;
use std::process::exit;

#[tokio::main]
async fn main() {
    let bin_name = std::env::args()
        .next()
        .and_then(|p| {
            std::path::Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "psql".to_string());

    let args = CliArgs::parse();

    // Validation: --csv and --tsv require -c/--command
    if (args.csv || args.tsv) && args.command.is_none() {
        eprintln!("{}: error: --csv and --tsv require -c/--command to be specified", bin_name);
        exit(1);
    }

    if let Err(e) = run_app(args).await {
        eprintln!("{}: error: {}", bin_name, e);
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

    if args.no_pager {
        state.pager = rsql::meta::PagerMode::Off;
    } else if let Some(p) = &args.pager {
        match p.to_lowercase().as_str() {
            "off" | "0" | "false" => state.pager = rsql::meta::PagerMode::Off,
            "always" => state.pager = rsql::meta::PagerMode::Always,
            _ => state.pager = rsql::meta::PagerMode::On,
        }
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

        execute_and_display_query(&conn.client, trimmed, &state, false).await?;
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
                execute_and_display_query(&conn.client, trimmed, &state, false).await?;
            }
        }
        return Ok(());
    }

    // Mode 3: Interactive REPL
    run_interactive_repl(conn, state).await?;

    Ok(())
}
