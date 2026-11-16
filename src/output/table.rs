use super::QueryResult;
use crate::error::Result;

pub fn format_table(result: &QueryResult, tuples_only: bool, border: u8) -> Result<String> {
    if result.columns.is_empty() {
        if tuples_only {
            return Ok("".to_string());
        }
        if let Some(tag) = &result.command_tag {
            return Ok(format!("{}\n", tag));
        } else {
            return Ok("(empty)\n".to_string());
        }
    }

    let num_cols = result.columns.len();

    // Determine numeric alignment per column (right-align if all non-empty values are numeric)
    let mut is_numeric = vec![true; num_cols];
    for (col_idx, is_num) in is_numeric.iter_mut().enumerate() {
        let mut has_non_empty = false;
        for row in &result.rows {
            if let Some(val) = row.get(col_idx) {
                let trimmed = val.trim();
                if trimmed.is_empty() || trimmed == "[NULL]" || trimmed == "NULL" {
                    continue;
                }
                has_non_empty = true;
                if trimmed.parse::<f64>().is_err() {
                    *is_num = false;
                    break;
                }
            }
        }
        if !has_non_empty {
            *is_num = false;
        }
    }

    // Determine column widths
    let mut widths = Vec::with_capacity(num_cols);
    for (col_idx, col_name) in result.columns.iter().enumerate() {
        let mut max_w = if tuples_only { 0 } else { col_name.len() };
        for row in &result.rows {
            if let Some(val) = row.get(col_idx) {
                max_w = max_w.max(val.len());
            }
        }
        widths.push(max_w);
    }

    let mut out = String::new();

    // 1. Top border (only if border == 2 and not tuples_only)
    if border == 2 && !tuples_only {
        out.push('+');
        for &w in &widths {
            out.push_str(&"-".repeat(w + 2));
            out.push('+');
        }
        out.push('\n');
    }

    // 2. Header row (only if not tuples_only)
    if !tuples_only {
        if border == 2 {
            out.push('|');
        }
        for (i, col_name) in result.columns.iter().enumerate() {
            let w = widths[i];
            let formatted_col = if is_numeric[i] {
                format!(" {:>width$} ", col_name, width = w)
            } else {
                let diff = w.saturating_sub(col_name.len());
                let left_pad = diff / 2;
                let right_pad = diff - left_pad;
                format!(" {}{}{} ", " ".repeat(left_pad), col_name, " ".repeat(right_pad))
            };
            out.push_str(&formatted_col);
            if (border >= 1 && i + 1 < num_cols) || (border == 2 && i + 1 == num_cols) {
                out.push('|');
            }
        }
        out.push('\n');

        // 3. Separator line below header
        if border >= 1 {
            if border == 2 {
                out.push('+');
            }
            for (i, &w) in widths.iter().enumerate() {
                out.push_str(&"-".repeat(w + 2));
                if (i + 1 < num_cols) || (border == 2 && i + 1 == num_cols) {
                    out.push('+');
                }
            }
            out.push('\n');
        }
    }

    // 4. Data rows
    for row in &result.rows {
        if border == 2 && !tuples_only {
            out.push('|');
        }
        for (i, &w) in widths.iter().enumerate() {
            let val = row.get(i).map(|s| s.as_str()).unwrap_or("");
            let formatted = if is_numeric[i] {
                format!(" {:>width$} ", val, width = w)
            } else {
                format!(" {:<width$} ", val, width = w)
            };
            out.push_str(&formatted);
            if ((border >= 1 || tuples_only) && i + 1 < num_cols)
                || (border == 2 && !tuples_only && i + 1 == num_cols)
            {
                out.push('|');
            }
        }
        out.push('\n');
    }

    // 5. Bottom border (only if border == 2 and not tuples_only)
    if border == 2 && !tuples_only {
        out.push('+');
        for &w in &widths {
            out.push_str(&"-".repeat(w + 2));
            out.push('+');
        }
        out.push('\n');
    }

    // 6. Footer (only if not tuples_only)
    if !tuples_only {
        let row_count = result.rows.len();
        out.push_str(&format!(
            "({} {})\n",
            row_count,
            if row_count == 1 { "row" } else { "rows" }
        ));
    }

    Ok(out)
}
