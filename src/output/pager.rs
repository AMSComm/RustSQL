use crate::error::Result;
use crossterm::terminal::size;
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

pub fn display_with_pager(content: &str) -> Result<()> {
    let line_count = content.lines().count();
    let term_height = size().map(|(_, h)| h as usize).unwrap_or(24);

    // If output fits in terminal, just print directly
    if line_count <= term_height {
        print!("{}", content);
        return Ok(());
    }

    // Try system pager ($PAGER or less -R -F -X)
    let pager_cmd = env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let child = if pager_cmd == "less" {
        Command::new("less")
            .arg("-R")
            .arg("-F")
            .arg("-X")
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
