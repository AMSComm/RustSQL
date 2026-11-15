pub mod csv;
pub mod expanded;
pub mod json;
pub mod markdown;
pub mod pager;
pub mod table;

use crate::error::Result;
use std::time::Duration;
use tokio_postgres::Row;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Aligned,
    Expanded,
    Csv,
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
    pub fn from_rows(rows: &[Row], duration: Duration, command_tag: Option<String>) -> Self {
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
                let val_str = extract_column_string(row, i);
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
        use_pager: bool,
    ) -> Result<()> {
        let output = match format {
            OutputFormat::Aligned => table::format_table(self)?,
            OutputFormat::Expanded => expanded::format_expanded(self)?,
            OutputFormat::Csv => csv::format_csv(self)?,
            OutputFormat::Json => json::format_json(self)?,
            OutputFormat::Markdown => markdown::format_markdown(self)?,
        };

        let mut final_output = output;
        if show_timing {
            let timing_str = format!("\nTime: {:.3} ms\n", self.duration.as_secs_f64() * 1000.0);
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

pub fn extract_column_string(row: &Row, idx: usize) -> String {
    // Try string / text / varchar
    if let Ok(v) = row.try_get::<_, Option<String>>(idx) {
        return v.unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try i64 / bigint
    if let Ok(v) = row.try_get::<_, Option<i64>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try i32 / int / integer
    if let Ok(v) = row.try_get::<_, Option<i32>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try i16 / smallint
    if let Ok(v) = row.try_get::<_, Option<i16>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try bool / boolean
    if let Ok(v) = row.try_get::<_, Option<bool>>(idx) {
        return v
            .map(|b| if b { "t".to_string() } else { "f".to_string() })
            .unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try f64 / double precision
    if let Ok(v) = row.try_get::<_, Option<f64>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try f32 / real
    if let Ok(v) = row.try_get::<_, Option<f32>>(idx) {
        return v.map(|x| x.to_string()).unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try chrono NaiveDateTime
    if let Ok(v) = row.try_get::<_, Option<chrono::NaiveDateTime>>(idx) {
        return v
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try chrono NaiveDate
    if let Ok(v) = row.try_get::<_, Option<chrono::NaiveDate>>(idx) {
        return v
            .map(|d| d.to_string())
            .unwrap_or_else(|| "[NULL]".to_string());
    }
    // Try byte slice
    if let Ok(v) = row.try_get::<_, Option<&[u8]>>(idx) {
        return v
            .map(|b| format!("\\x{}", hex_encode(b)))
            .unwrap_or_else(|| "[NULL]".to_string());
    }

    "[UNSUPPORTED TYPE]".to_string()
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
