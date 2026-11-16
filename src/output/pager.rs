use crate::error::Result;
use crate::meta::PagerMode;
use crossterm::terminal::size;
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

pub fn display_with_pager(content: &str, pager_mode: PagerMode) -> Result<()> {
    if pager_mode == PagerMode::Off {
        print!("{}", content);
        return Ok(());
    }

    let line_count = content.lines().count();
    let term_height = size().map(|(_, h)| h as usize).unwrap_or(24);

    // If output fits in terminal, and pager_mode is not Always, just print directly
    if pager_mode != PagerMode::Always && line_count <= term_height {
        print!("{}", content);
        return Ok(());
    }

    // Try system pager ($PSQL_PAGER, $PAGER, or less)
    let pager_cmd = env::var("PSQL_PAGER")
        .or_else(|_| env::var("PAGER"))
        .unwrap_or_else(|_| "less".to_string());

    if pager_cmd == "cat" || pager_cmd == "off" || pager_cmd.is_empty() {
        print!("{}", content);
        return Ok(());
    }

    let child = if pager_cmd == "less" {
        Command::new("less")
            .arg("-R") // Raw ANSI color codes
            .arg("-S") // Chop long lines without wrapping
            .arg("-M") // Detailed line count & percentage prompt at bottom
            .arg("-e") // Automatically exit at second EOF
            .arg("-F") // Quit if one screen
            .stdin(Stdio::piped())
            .spawn()
    } else {
        Command::new(&pager_cmd).stdin(Stdio::piped()).spawn()
    };

    if let Ok(mut child_proc) = child {
        if let Some(mut stdin) = child_proc.stdin.take() {
            let _ = stdin.write_all(content.as_bytes());
            let _ = stdin.flush();
        }
        let _ = child_proc.wait();
        return Ok(());
    }

    // Fallback if pager fails
    print!("{}", content);
    Ok(())
}
