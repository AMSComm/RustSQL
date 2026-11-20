use crate::connection::DbConnection;
use crate::meta::{execute_meta_command, parse_meta_command, AppState};
use colored::Colorize;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub async fn load_startup_rc_files(
    conn: &mut DbConnection,
    state: &mut AppState,
) {
    let mut files_to_try = Vec::new();

    // 1. Check $RUQLRC or $PSQLRC environment variable
    if let Ok(custom_rc) = env::var("RUQLRC").or_else(|_| env::var("PSQLRC")) {
        files_to_try.push(PathBuf::from(custom_rc));
    } else if let Some(home) = dirs::home_dir() {
        // 2. Default search order: ~/.ruqlrc, then ~/.psqlrc, then ~/.redshiftrc (if Redshift), then ~/.rsqlrc
        let ruqlrc = home.join(".ruqlrc");
        if ruqlrc.exists() {
            files_to_try.push(ruqlrc);
        }

        let psqlrc = home.join(".psqlrc");
        if psqlrc.exists() {
            files_to_try.push(psqlrc);
        }

        if conn.dialect.has_external_support() {
            let redshiftrc = home.join(".redshiftrc");
            if redshiftrc.exists() {
                files_to_try.push(redshiftrc);
            }
        }

        let rsqlrc = home.join(".rsqlrc");
        if rsqlrc.exists() {
            files_to_try.push(rsqlrc);
        }
    }

    for rc_path in files_to_try {
        if rc_path.exists() && rc_path.is_file() {
            if let Err(e) = execute_rc_file(&rc_path, conn, state).await {
                if !state.quiet {
                    eprintln!(
                        "{}",
                        format!("Warning: error reading rc file {:?}: {}", rc_path, e).yellow()
                    );
                }
            }
        }
    }
}

async fn execute_rc_file(
    path: &Path,
    conn: &mut DbConnection,
    state: &mut AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for (line_num, line_res) in reader.lines().enumerate() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }

        if trimmed.starts_with('\\') {
            if let Some(meta) = parse_meta_command(trimmed) {
                if let Err(e) = execute_meta_command(meta, conn, state).await {
                    if !state.quiet {
                        eprintln!(
                            "{}",
                            format!("{}:{}: warning: {}", path.display(), line_num + 1, e).yellow()
                        );
                    }
                }
            }
        } else {
            // SQL command in rc file
            if let Err(e) = conn.client.simple_query(trimmed).await {
                if !state.quiet {
                    eprintln!(
                        "{}",
                        format!("{}:{}: warning: {}", path.display(), line_num + 1, e).yellow()
                    );
                }
            }
        }
    }

    Ok(())
}
