use clap::Parser;
use colored::Colorize;
use rustsql::cli::CliArgs;
use rustsql::config::ConnectionConfig;
use rustsql::connection::DbConnection;
use rustsql::error::Result;
use rustsql::meta::{execute_meta_command, parse_meta_command, AppState};
use rustsql::output::{execute_and_display_query, OutputFormat};
use rustsql::rc::load_startup_rc_files;
use rustsql::repl::run_interactive_repl;
use rustsql::updater;
use std::fs;
use std::process::exit;

#[tokio::main]
async fn main() {
    // Initialize panic hook and error logging
    rustsql::logger::init();

    let bin_name = std::env::args()
        .next()
        .and_then(|p| {
            std::path::Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "ruql".to_string());

    let args = CliArgs::parse();

    // Check for self-update
    if args.update {
        if let Err(e) = updater::update_cli().await {
            eprintln!("{}: update error: {}", bin_name, e);
            exit(1);
        }
        exit(0);
    }

    // Check for update availability
    if args.check_update {
        match updater::check_latest_release().await {
            Ok(Some(release)) => {
                let current_version = env!("CARGO_PKG_VERSION");
                if updater::is_newer_version(&release.tag_name, current_version) {
                    println!(
                        "New version available: {} (current: v{}). Run \"ruql --update\" to install.",
                        release.tag_name.green().bold(),
                        current_version
                    );
                } else {
                    println!("ruql is up to date (version v{}).", current_version);
                }
            }
            Ok(None) => println!("No published releases found yet."),
            Err(e) => eprintln!("Failed to check for updates: {}", e),
        }
        exit(0);
    }

    // Validation: --csv and --tsv require -c/--command
    if (args.csv || args.tsv) && args.command.is_none() {
        eprintln!("{}: error: --csv and --tsv require -c/--command to be specified", bin_name);
        exit(1);
    }

    if let Err(e) = run_app(args).await {
        eprintln!("{}: error: {}", bin_name, e);
        rustsql::logger::log_error("Application startup/execution error", &e);
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
        state.expanded = rustsql::meta::ExpandedMode::On;
    }

    if args.no_header {
        state.tuples_only = true;
    }

    if args.no_pager {
        state.pager = rustsql::meta::PagerMode::Off;
    } else if let Some(p) = &args.pager {
        match p.to_lowercase().as_str() {
            "off" | "0" | "false" => state.pager = rustsql::meta::PagerMode::Off,
            "always" => state.pager = rustsql::meta::PagerMode::Always,
            _ => state.pager = rustsql::meta::PagerMode::On,
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
        rustsql::meta::execute_script(&content, &mut conn, &mut state).await?;
        return Ok(());
    }

    // Mode 3: Interactive REPL
    run_interactive_repl(conn, state).await?;

    Ok(())
}
