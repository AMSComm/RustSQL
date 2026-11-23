pub mod csv;
pub mod expanded;
pub mod json;
pub mod markdown;
pub mod pager;
pub mod table;

use crate::error::{Result, RsqlError};
use crate::meta::{AppState, ExpandedMode, PagerMode};
use std::time::{Duration, Instant};
use tokio_postgres::{Client, Row, SimpleQueryMessage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Aligned,
    Expanded,
    Csv,
    Tsv,
    Json,
    Markdown,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub command_tag: Option<String>,
    pub rows_affected: Option<u64>,
    pub duration: Duration,
}

impl QueryResult {
    pub fn from_rows(
        rows: &[Row],
        duration: Duration,
        command_tag: Option<String>,
        null_str: &str,
    ) -> Self {
        if rows.is_empty() {
            return Self {
                columns: Vec::new(),
                rows: Vec::new(),
                command_tag,
                rows_affected: Some(0),
                duration,
            };
        }

        let first = &rows[0];
        let columns: Vec<String> = first
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();

        let mut data_rows = Vec::with_capacity(rows.len());
        for row in rows {
            let mut row_values = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                let val_str = extract_column_string(row, i, null_str);
                row_values.push(val_str);
            }
            data_rows.push(row_values);
        }

        let count = data_rows.len() as u64;
        Self {
            columns,
            rows: data_rows,
            command_tag,
            rows_affected: Some(count),
            duration,
        }
    }

    pub fn display(
        &self,
        format: OutputFormat,
        show_timing: bool,
        tuples_only: bool,
        border: u8,
        pager_mode: PagerMode,
        expanded: ExpandedMode,
    ) -> Result<()> {
        let is_expanded = match expanded {
            ExpandedMode::On => true,
            ExpandedMode::Auto => {
                if format == OutputFormat::Aligned {
                    let term_w = crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(80);
                    let table_w = table::estimate_table_width(self, tuples_only, border);
                    table_w > term_w
                } else {
                    false
                }
            }
            ExpandedMode::Off => format == OutputFormat::Expanded,
        };

        let output = if is_expanded {
            expanded::format_expanded(self)?
        } else {
            match format {
                OutputFormat::Aligned => table::format_table(self, tuples_only, border)?,
                OutputFormat::Expanded => expanded::format_expanded(self)?,
                OutputFormat::Csv => csv::format_csv(self, !tuples_only)?,
                OutputFormat::Tsv => csv::format_tsv(self, !tuples_only)?,
                OutputFormat::Json => json::format_json(self)?,
                OutputFormat::Markdown => markdown::format_markdown(self)?,
            }
        };

        let mut final_output = output;
        if show_timing && !tuples_only {
            let timing_str = format!("Time: {:.3} ms\n", self.duration.as_secs_f64() * 1000.0);
            final_output.push_str(&timing_str);
        }

        pager::display_with_pager(&final_output, pager_mode)?;
        Ok(())
    }
}

/// Split SQL into individual statements by semicolon, correctly respecting
/// single quotes ('...'), double quotes ("..."), dollar quotes ($$...$$ or $tag$...$tag$),
/// line comments (--), and block comments (/* ... */).
pub fn split_statements(sql: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut start = 0;
    let mut i = 0;

    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_line_comment = false;
    let mut block_comment_depth = 0;
    let mut dollar_tag: Option<&[u8]> = None;

    while i < len {
        if in_line_comment {
            if bytes[i] == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if block_comment_depth > 0 {
            if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                block_comment_depth -= 1;
                i += 2;
                continue;
            } else if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                block_comment_depth += 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if let Some(tag) = dollar_tag {
            if bytes[i..].starts_with(tag) {
                let tag_len = tag.len();
                dollar_tag = None;
                i += tag_len;
            } else {
                i += 1;
            }
            continue;
        }

        if in_single_quote {
            if bytes[i] == b'\'' {
                if i + 1 < len && bytes[i + 1] == b'\'' {
                    i += 2; // escaped ''
                    continue;
                }
                in_single_quote = false;
            }
            i += 1;
            continue;
        }

        if in_double_quote {
            if bytes[i] == b'"' {
                if i + 1 < len && bytes[i + 1] == b'"' {
                    i += 2; // escaped ""
                    continue;
                }
                in_double_quote = false;
            }
            i += 1;
            continue;
        }

        // Check for start of comments
        if i + 1 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            in_line_comment = true;
            i += 2;
            continue;
        }

        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            block_comment_depth = 1;
            i += 2;
            continue;
        }

        // Check for quotes
        if bytes[i] == b'\'' {
            in_single_quote = true;
            i += 1;
            continue;
        }

        if bytes[i] == b'"' {
            in_double_quote = true;
            i += 1;
            continue;
        }

        // Check for dollar-quoted strings: $tag$ or $$
        if bytes[i] == b'$' {
            let rest = &bytes[i + 1..];
            if let Some(pos) = rest.iter().position(|&b| b == b'$') {
                let inner = &bytes[i + 1..i + 1 + pos];
                if inner.is_empty() || inner.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_') {
                    let tag_slice = &bytes[i..=i + 1 + pos];
                    dollar_tag = Some(tag_slice);
                    i += tag_slice.len();
                    continue;
                }
            }
        }

        // Semicolon outside of any quote or comment marks end of statement
        if bytes[i] == b';' {
            if let Ok(stmt) = std::str::from_utf8(&bytes[start..i]) {
                let trimmed = stmt.trim();
                if !trimmed.is_empty() {
                    statements.push(trimmed);
                }
            }
            start = i + 1;
        }

        i += 1;
    }

    if start < len {
        if let Ok(stmt) = std::str::from_utf8(&bytes[start..]) {
            let trimmed = stmt.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed);
            }
        }
    }

    statements
}

/// Extract PostgreSQL command tag (e.g. "CREATE TABLE", "INSERT 0 1", "UPDATE 5")
/// from statement text and row count.
pub fn extract_command_tag(stmt: &str, rows_affected: u64) -> String {
    let mut s = stmt.trim();
    while s.starts_with("--") || s.starts_with("/*") {
        if s.starts_with("--") {
            if let Some(pos) = s.find('\n') {
                s = s[pos + 1..].trim();
            } else {
                s = "";
                break;
            }
        } else if s.starts_with("/*") {
            if let Some(pos) = s.find("*/") {
                s = s[pos + 2..].trim();
            } else {
                s = "";
                break;
            }
        }
    }

    let upper = s.to_uppercase();
    let words: Vec<&str> = upper.split_whitespace().collect();
    let first = words.first().copied().unwrap_or("");
    let second = words.get(1).copied().unwrap_or("");

    match first {
        "INSERT" => format!("INSERT 0 {}", rows_affected),
        "UPDATE" => format!("UPDATE {}", rows_affected),
        "DELETE" => format!("DELETE {}", rows_affected),
        "SELECT" => "SELECT".to_string(),
        "CREATE" => match second {
            "TABLE" => "CREATE TABLE".to_string(),
            "VIEW" => "CREATE VIEW".to_string(),
            "INDEX" => "CREATE INDEX".to_string(),
            "SCHEMA" => "CREATE SCHEMA".to_string(),
            "DATABASE" => "CREATE DATABASE".to_string(),
            "FUNCTION" => "CREATE FUNCTION".to_string(),
            "PROCEDURE" => "CREATE PROCEDURE".to_string(),
            "TRIGGER" => "CREATE TRIGGER".to_string(),
            "TYPE" => "CREATE TYPE".to_string(),
            "EXTENSION" => "CREATE EXTENSION".to_string(),
            "SEQUENCE" => "CREATE SEQUENCE".to_string(),
            "MATERIALIZED" => "CREATE MATERIALIZED VIEW".to_string(),
            _ => "CREATE".to_string(),
        },
        "DROP" => match second {
            "TABLE" => "DROP TABLE".to_string(),
            "VIEW" => "DROP VIEW".to_string(),
            "INDEX" => "DROP INDEX".to_string(),
            "SCHEMA" => "DROP SCHEMA".to_string(),
            "DATABASE" => "DROP DATABASE".to_string(),
            "FUNCTION" => "DROP FUNCTION".to_string(),
            "PROCEDURE" => "DROP PROCEDURE".to_string(),
            "TRIGGER" => "DROP TRIGGER".to_string(),
            "TYPE" => "DROP TYPE".to_string(),
            "EXTENSION" => "DROP EXTENSION".to_string(),
            "SEQUENCE" => "DROP SEQUENCE".to_string(),
            "MATERIALIZED" => "DROP MATERIALIZED VIEW".to_string(),
            _ => "DROP".to_string(),
        },
        "ALTER" => match second {
            "TABLE" => "ALTER TABLE".to_string(),
            "VIEW" => "ALTER VIEW".to_string(),
            "INDEX" => "ALTER INDEX".to_string(),
            "SCHEMA" => "ALTER SCHEMA".to_string(),
            "DATABASE" => "ALTER DATABASE".to_string(),
            "SEQUENCE" => "ALTER SEQUENCE".to_string(),
            _ => "ALTER".to_string(),
        },
        "TRUNCATE" => "TRUNCATE TABLE".to_string(),
        "VACUUM" => "VACUUM".to_string(),
        "ANALYZE" => "ANALYZE".to_string(),
        "EXPLAIN" => "EXPLAIN".to_string(),
        "COPY" => format!("COPY {}", rows_affected),
        "BEGIN" | "START" => "BEGIN".to_string(),
        "COMMIT" | "END" => "COMMIT".to_string(),
        "ROLLBACK" | "ABORT" => "ROLLBACK".to_string(),
        "SET" => "SET".to_string(),
        "RESET" => "RESET".to_string(),
        "SHOW" => "SHOW".to_string(),
        "GRANT" => "GRANT".to_string(),
        "REVOKE" => "REVOKE".to_string(),
        "LOCK" => "LOCK TABLE".to_string(),
        "CALL" => "CALL".to_string(),
        "DO" => "DO".to_string(),
        _ => {
            if first.is_empty() {
                format!("{}", rows_affected)
            } else {
                first.to_string()
            }
        }
    }
}

pub async fn execute_and_display_query(
    client: &Client,
    sql: &str,
    state: &AppState,
    use_pager: bool,
) -> Result<()> {
    let pager_mode = if use_pager { state.pager } else { PagerMode::Off };
    let start = Instant::now();
    let messages = client.simple_query(sql).await.map_err(RsqlError::Database)?;
    let duration = start.elapsed();

    let statements = split_statements(sql);
    let mut stmt_idx = 0;

    let mut current_cols: Vec<String> = Vec::new();
    let mut current_rows: Vec<Vec<String>> = Vec::new();
    let mut returned_any_row = false;

    for msg in messages {
        match msg {
            SimpleQueryMessage::Row(r) => {
                if current_cols.is_empty() {
                    for i in 0..r.columns().len() {
                        current_cols.push(r.columns()[i].name().to_string());
                    }
                }
                let mut vals = Vec::with_capacity(current_cols.len());
                for i in 0..current_cols.len() {
                    vals.push(r.get(i).unwrap_or(&state.null_str).to_string());
                }
                current_rows.push(vals);
                returned_any_row = true;
            }
            SimpleQueryMessage::CommandComplete(rows_affected) => {
                let current_stmt = statements.get(stmt_idx).copied().unwrap_or("");
                stmt_idx += 1;

                if returned_any_row {
                    let res = QueryResult {
                        columns: std::mem::take(&mut current_cols),
                        rows: std::mem::take(&mut current_rows),
                        command_tag: None,
                        rows_affected: Some(rows_affected),
                        duration,
                    };
                    res.display(
                        state.format,
                        state.timing,
                        state.tuples_only,
                        state.border,
                        pager_mode,
                        state.expanded,
                    )?;
                    returned_any_row = false;
                } else if !state.quiet && !state.tuples_only {
                    let cmd_tag = extract_command_tag(current_stmt, rows_affected);
                    if cmd_tag == "SELECT" {
                        let res = QueryResult {
                            columns: std::mem::take(&mut current_cols),
                            rows: Vec::new(),
                            command_tag: None,
                            rows_affected: Some(0),
                            duration,
                        };
                        res.display(
                            state.format,
                            state.timing,
                            state.tuples_only,
                            state.border,
                            pager_mode,
                            state.expanded,
                        )?;
                    } else if !cmd_tag.is_empty() {
                        println!("{}", cmd_tag);
                        if state.timing {
                            println!("Time: {:.3} ms", duration.as_secs_f64() * 1000.0);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn extract_column_string(row: &Row, idx: usize, null_str: &str) -> String {
    // Try string / text / varchar
    if let Ok(v) = row.try_get::<_, Option<String>>(idx) {
        return v.unwrap_or_else(|| null_str.to_string());
    }
    // Try i64 / bigint
    if let Ok(v) = row.try_get::<_, Option<i64>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| null_str.to_string());
    }
    // Try i32 / int / integer
    if let Ok(v) = row.try_get::<_, Option<i32>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| null_str.to_string());
    }
    // Try i16 / smallint
    if let Ok(v) = row.try_get::<_, Option<i16>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| null_str.to_string());
    }
    // Try bool / boolean
    if let Ok(v) = row.try_get::<_, Option<bool>>(idx) {
        return v
            .map(|b| if b { "t".to_string() } else { "f".to_string() })
            .unwrap_or_else(|| null_str.to_string());
    }
    // Try f64 / double precision
    if let Ok(v) = row.try_get::<_, Option<f64>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| null_str.to_string());
    }
    // Try f32 / real
    if let Ok(v) = row.try_get::<_, Option<f32>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| null_str.to_string());
    }
    // Try chrono NaiveDateTime
    if let Ok(v) = row.try_get::<_, Option<chrono::NaiveDateTime>>(idx) {
        return v
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| null_str.to_string());
    }
    // Try chrono NaiveDate
    if let Ok(v) = row.try_get::<_, Option<chrono::NaiveDate>>(idx) {
        return v
            .map(|d| d.to_string())
            .unwrap_or_else(|| null_str.to_string());
    }
    // Try byte slice
    if let Ok(v) = row.try_get::<_, Option<&[u8]>>(idx) {
        return v
            .map(|b| format!("\\x{}", hex_encode(b)))
            .unwrap_or_else(|| null_str.to_string());
    }

    null_str.to_string()
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
