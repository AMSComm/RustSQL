use crate::error::Result;
use crate::meta::PagerMode;
use crossterm::terminal::size;
use std::env;
use std::io::{stdout, IsTerminal, Write};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerAction {
    NextPage,
    NextLine,
    HalfPage,
    ShowAll,
    Quit,
}

/// Parse a key code and modifiers into a pager action matching standard psql/more behavior:
/// - R / r: Show all remaining results without pausing (Bung ra hết)
/// - Space / PageDown: Next page
/// - Enter / Down / j: Next line
/// - d / D: Half page
/// - q / Q / Esc / Ctrl+C: Quit pager
pub fn parse_pager_key(
    code: crossterm::event::KeyCode,
    modifiers: crossterm::event::KeyModifiers,
) -> Option<PagerAction> {
    use crossterm::event::{KeyCode, KeyModifiers};
    match code {
        KeyCode::Char('r') | KeyCode::Char('R') => Some(PagerAction::ShowAll),
        KeyCode::Char(' ') | KeyCode::PageDown => Some(PagerAction::NextPage),
        KeyCode::Enter | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
            Some(PagerAction::NextLine)
        }
        KeyCode::Char('d') | KeyCode::Char('D') => Some(PagerAction::HalfPage),
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Some(PagerAction::Quit),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(PagerAction::Quit),
        _ => None,
    }
}

/// Read a single key action from the terminal in raw mode.
fn read_pager_action() -> PagerAction {
    struct RawModeGuard;
    impl RawModeGuard {
        fn new() -> Option<Self> {
            crossterm::terminal::enable_raw_mode().ok().map(|_| Self)
        }
    }
    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }

    let _guard = RawModeGuard::new();

    loop {
        match crossterm::event::read() {
            Ok(crossterm::event::Event::Key(key)) => {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    continue;
                }
                if let Some(action) = parse_pager_key(key.code, key.modifiers) {
                    return action;
                }
            }
            Err(_) => return PagerAction::ShowAll,
            _ => {}
        }
    }
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

/// Built-in interactive pager matching psql (more) behavior with full support for:
/// - Pressing 'R' to show all remaining output without pausing ("bung ra hết")
/// - Space for next page
/// - Enter for next line
/// - Q for quit
/// - Output stays in normal terminal scrollback buffer so users can scroll back up smoothly.
pub fn display_with_builtin_pager(
    content: &str,
    mut term_width: usize,
    mut term_height: usize,
) -> Result<()> {
    let mut lines = content.lines().peekable();
    let mut page_size = term_height.saturating_sub(1).max(1);
    let mut show_all = false;
    let mut visual_count = 0;
    let mut prompt_lines_needed = page_size;

    let stdout_handle = stdout();
    let mut out = stdout_handle.lock();

    while let Some(line) = lines.next() {
        writeln!(out, "{}", line)?;

        if show_all {
            continue;
        }

        let line_visual = if term_width > 0 {
            line.chars().count().div_ceil(term_width).max(1)
        } else {
            1
        };
        visual_count += line_visual;

        if visual_count >= prompt_lines_needed && lines.peek().is_some() {
            // Update terminal size in case the user resized their window
            if let Ok((w, h)) = size() {
                if w > 0 {
                    term_width = w as usize;
                }
                if h > 1 {
                    term_height = h as usize;
                    page_size = term_height.saturating_sub(1).max(1);
                }
            }

            let prompt = "--More-- [R: show all, Space: page, Enter: line, Q: quit]";
            write!(out, "{}", prompt)?;
            out.flush()?;

            let action = read_pager_action();

            // Erase the prompt line cleanly (\r + erase line)
            write!(out, "\r\x1b[2K")?;
            out.flush()?;

            match action {
                PagerAction::ShowAll => {
                    show_all = true;
                }
                PagerAction::NextPage => {
                    visual_count = 0;
                    prompt_lines_needed = page_size;
                }
                PagerAction::NextLine => {
                    visual_count = 0;
                    prompt_lines_needed = 1;
                }
                PagerAction::HalfPage => {
                    visual_count = 0;
                    prompt_lines_needed = (page_size / 2).max(1);
                }
                PagerAction::Quit => {
                    break;
                }
            }
        }
    }

    out.flush()?;
    Ok(())
}

/// Fallback for when external PSQL_PAGER is explicitly configured (e.g. pspg or custom less flags)
pub fn display_with_external_pager(content: &str, pager_cmd: &str) -> Result<()> {
    let mut command = if cfg!(unix) {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(pager_cmd);
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

    print!("{}", content);
    Ok(())
}

/// Display query result using the pager when appropriate, matching standard PostgreSQL psql behavior.
pub fn display_with_pager(content: &str, pager_mode: PagerMode) -> Result<()> {
    // 1. If pager is explicitly turned off, print directly
    if pager_mode == PagerMode::Off {
        print!("{}", content);
        return Ok(());
    }

    // 2. If stdin or stdout is not connected to a terminal (e.g. piped, redirected, or batch script), do not page
    if !std::io::stdin().is_terminal() || !stdout().is_terminal() {
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

    // 5. If user explicitly specified PSQL_PAGER (and not "builtin", "more", "cat", "off"), use external pager
    if let Ok(val) = env::var("PSQL_PAGER") {
        let trimmed = val.trim();
        if trimmed.eq_ignore_ascii_case("cat") || trimmed.eq_ignore_ascii_case("off") {
            print!("{}", content);
            return Ok(());
        }
        if !trimmed.is_empty()
            && !trimmed.eq_ignore_ascii_case("builtin")
            && !trimmed.eq_ignore_ascii_case("more")
        {
            return display_with_external_pager(content, trimmed);
        }
    }

    // 6. Built-in interactive pager (supports 'R' to show all, Space for page, Enter for line, Q for quit)
    display_with_builtin_pager(content, term_width, term_height)
}
