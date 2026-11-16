pub mod csv;
pub mod expanded;
pub mod json;
pub mod markdown;
pub mod pager;
pub mod table;

use crate::error::{Result, RsqlError};
use crate::meta::AppState;
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
        use_pager: bool,
    ) -> Result<()> {
        let output = match format {
            OutputFormat::Aligned => table::format_table(self, tuples_only, border)?,
            OutputFormat::Expanded => expanded::format_expanded(self)?,
            OutputFormat::Csv => csv::format_csv(self, !tuples_only)?,
            OutputFormat::Tsv => csv::format_tsv(self, !tuples_only)?,
            OutputFormat::Json => json::format_json(self)?,
            OutputFormat::Markdown => markdown::format_markdown(self)?,
        };

        let mut final_output = output;
        if show_timing && !tuples_only {
            let timing_str = format!("Time: {:.3} ms\n", self.duration.as_secs_f64() * 1000.0);
            final_output.push_str(&timing_str);
        }

        if use_pager {
            pager::display_with_pager(&final_output)?;
        } else {
            print!("{}", final_output);
        }

        Ok(())
    }
}

pub async fn execute_and_display_query(
    client: &Client,
    sql: &str,
    state: &AppState,
    use_pager: bool,
) -> Result<()> {
    let start = Instant::now();
    let messages = client.simple_query(sql).await.map_err(RsqlError::Database)?;
    let duration = start.elapsed();

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
                if returned_any_row {
                    let res = QueryResult {
                        columns: current_cols.clone(),
                        rows: current_rows.clone(),
                        command_tag: None,
                        rows_affected: Some(rows_affected),
                        duration,
                    };
                    res.display(
                        state.format,
                        state.timing,
                        state.tuples_only,
                        state.border,
                        use_pager,
                    )?;
                    current_cols.clear();
                    current_rows.clear();
                    returned_any_row = false;
                } else if !state.quiet && !state.tuples_only {
                    // DDL or non-row statement: print standard psql command tag
                    let upper = sql.trim().to_uppercase();
                    let first_word = upper.split_whitespace().next().unwrap_or("");
                    let cmd_tag = match first_word {
                        "INSERT" => format!("INSERT 0 {}", rows_affected),
                        "UPDATE" => format!("UPDATE {}", rows_affected),
                        "DELETE" => format!("DELETE {}", rows_affected),
                        "SELECT" => {
                            // 0 rows returned
                            let res = QueryResult {
                                columns: current_cols.clone(),
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
                                use_pager,
                            )?;
                            "".to_string()
                        }
                        "CREATE" => {
                            if upper.starts_with("CREATE TABLE") {
                                "CREATE TABLE".to_string()
                            } else if upper.starts_with("CREATE VIEW") {
                                "CREATE VIEW".to_string()
                            } else if upper.starts_with("CREATE INDEX") {
                                "CREATE INDEX".to_string()
                            } else if upper.starts_with("CREATE SCHEMA") {
                                "CREATE SCHEMA".to_string()
                            } else if upper.starts_with("CREATE DATABASE") {
                                "CREATE DATABASE".to_string()
                            } else {
                                "CREATE".to_string()
                            }
                        }
                        "DROP" => {
                            if upper.starts_with("DROP TABLE") {
                                "DROP TABLE".to_string()
                            } else if upper.starts_with("DROP VIEW") {
                                "DROP VIEW".to_string()
                            } else if upper.starts_with("DROP SCHEMA") {
                                "DROP SCHEMA".to_string()
                            } else if upper.starts_with("DROP DATABASE") {
                                "DROP DATABASE".to_string()
                            } else {
                                "DROP".to_string()
                            }
                        }
                        "ALTER" => {
                            if upper.starts_with("ALTER TABLE") {
                                "ALTER TABLE".to_string()
                            } else {
                                "ALTER".to_string()
                            }
                        }
                        "SET" => "SET".to_string(),
                        "RESET" => "RESET".to_string(),
                        "BEGIN" => "BEGIN".to_string(),
                        "COMMIT" => "COMMIT".to_string(),
                        "ROLLBACK" => "ROLLBACK".to_string(),
                        "VACUUM" => "VACUUM".to_string(),
                        "TRUNCATE" => "TRUNCATE TABLE".to_string(),
                        _ => first_word.to_string(),
                    };

                    if !cmd_tag.is_empty() {
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
