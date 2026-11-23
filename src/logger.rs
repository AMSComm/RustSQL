use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// Return the canonical log file path (~/.ruql/error.log)
pub fn get_log_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        let dir = h.join(".ruql");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("error.log")
    })
}

/// Initialize global logger and panic hook.
/// Ensures terminal raw mode is properly cleaned up on panic
/// and complete backtrace is recorded in ~/.ruql/error.log.
pub fn init() {
    std::panic::set_hook(Box::new(|info| {
        // 1. Reset terminal raw mode and show cursor immediately
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);

        // 2. Extract thread and panic message
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("main");

        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<dyn Any>",
            },
        };

        let location = if let Some(loc) = info.location() {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        } else {
            "unknown location".to_string()
        };

        let backtrace = std::backtrace::Backtrace::capture();
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");

        let log_content = format!(
            "[{}] [PANIC] thread '{}' panicked at '{}' ({})\nBacktrace:\n{:?}\n\n",
            now, thread_name, msg, location, backtrace
        );

        if let Some(log_path) = get_log_file_path() {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                let _ = file.write_all(log_content.as_bytes());
            }
        }

        eprintln!();
        eprintln!("An unexpected error occurred: {} (at {})", msg, location);
        if let Some(path) = get_log_file_path() {
            eprintln!("Detailed error log has been saved to: {}", path.display());
        }
        eprintln!("Please report this issue at: https://github.com/AMSComm/RustSQL/issues");
    }));
}

/// Append an error entry to ~/.ruql/error.log with timestamp
pub fn log_error(context: &str, err: &dyn std::fmt::Display) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_content = format!("[{}] [ERROR] {}: {}\n", now, context, err);

    if let Some(log_path) = get_log_file_path() {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = file.write_all(log_content.as_bytes());
        }
    }
}
