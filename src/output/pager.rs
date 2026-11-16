use crate::error::Result;
use crate::meta::PagerMode;
use crossterm::terminal::size;
use std::env;
use std::io::{stdout, IsTerminal, Write};
use std::process::{Command, Stdio};

/// Determine the pager command to use based on environment variables and defaults.
pub fn resolve_pager_cmd() -> Option<String> {
    if let Ok(val) = env::var("PSQL_PAGER") {
        let trimmed = val.trim();
        if trimmed.eq_ignore_ascii_case("cat") || trimmed.eq_ignore_ascii_case("off") {
            return None;
        }
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if let Ok(val) = env::var("PAGER") {
        let trimmed = val.trim();
        if trimmed.eq_ignore_ascii_case("cat") || trimmed.eq_ignore_ascii_case("off") || trimmed.is_empty() {
            // If global PAGER is 'cat' or 'off' (very common in developer shell configs for git),
            // fallback to "less" so interactive database queries are properly paged.
            return Some("less".to_string());
        }
        return Some(trimmed.to_string());
    }

    Some("less".to_string())
}

/// Calculate estimated visual lines rendered on a terminal with given width.
pub fn calculate_visual_lines(content: &str, term_width: usize) -> usize {
    let mut visual_lines = 0;
    for line in content.lines() {
        let len = line.chars().count();
        let wrapped = if term_width > 0 {
            len.div_ceil(term_width)
        } else {
            1
        };
        visual_lines += wrapped.max(1);
    }
    visual_lines
}

/// Display query result using an external pager (less/more) when appropriate,
/// matching standard PostgreSQL psql behavior.
pub fn display_with_pager(content: &str, pager_mode: PagerMode) -> Result<()> {
    // 1. If pager is explicitly turned off, print directly
    if pager_mode == PagerMode::Off {
        print!("{}", content);
        return Ok(());
    }

    // 2. If stdout is not connected to a terminal (e.g. piped or redirected), do not page
    if !stdout().is_terminal() {
        print!("{}", content);
        return Ok(());
    }

    // 3. Resolve terminal dimensions
    let (term_width, term_height) = size()
        .map(|(w, h)| (w as usize, h as usize))
        .unwrap_or((80, 24));

    // 4. If not PagerMode::Always, check if content fits in screen
    if pager_mode != PagerMode::Always {
        let visual_lines = calculate_visual_lines(content, term_width);
        if visual_lines <= term_height {
            print!("{}", content);
            return Ok(());
        }
    }

    // 5. Resolve pager program
    let pager_cmd = match resolve_pager_cmd() {
        Some(cmd) => cmd,
        None => {
            print!("{}", content);
            return Ok(());
        }
    };

    // 6. Spawn pager process
    // Set LESS="-R" if LESS is not already set in environment to support ANSI colors
    // while keeping alternate screen and natural line wrapping (like standard psql).
    let mut command = if cfg!(unix) {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&pager_cmd);
        cmd
    } else {
        let mut parts = pager_cmd.split_whitespace();
        let prog = parts.next().unwrap_or("more");
        let args: Vec<&str> = parts.collect();
        let mut cmd = Command::new(prog);
        cmd.args(&args);
        cmd
    };

    if env::var("LESS").is_err() {
        command.env("LESS", "-R");
    }

    command.stdin(Stdio::piped());

    if let Ok(mut child) = command.spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(content.as_bytes());
            let _ = stdin.flush();
            drop(stdin);
        }
        let _ = child.wait();
        return Ok(());
    }

    // Fallback if pager fails to spawn
    print!("{}", content);
    Ok(())
}
