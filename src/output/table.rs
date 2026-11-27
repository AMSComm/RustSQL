use super::QueryResult;
use crate::error::Result;
use unicode_width::UnicodeWidthStr;

#[inline]
fn display_width(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        UnicodeWidthStr::width(s)
    }
}

#[inline]
fn push_chars(out: &mut String, ch: char, count: usize) {
    for _ in 0..count {
        out.push(ch);
    }
}

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

    // Determine column display widths (accounting for multi-byte & wide CJK characters)
    let mut widths = Vec::with_capacity(num_cols);
    for (col_idx, col_name) in result.columns.iter().enumerate() {
        let mut max_w = if tuples_only { 0 } else { display_width(col_name) };
        for row in &result.rows {
            if let Some(val) = row.get(col_idx) {
                max_w = max_w.max(display_width(val));
            }
        }
        widths.push(max_w);
    }

    // Pre-allocate buffer based on estimated row width to avoid reallocations on large datasets
    let est_row_len = widths.iter().sum::<usize>() + (num_cols * 3) + 2;
    let est_total_capacity = (result.rows.len() + 4) * est_row_len;
    let mut out = String::with_capacity(est_total_capacity);

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
            let col_w = display_width(col_name);
            let diff = w.saturating_sub(col_w);
            out.push(' ');
            if is_numeric[i] {
                push_chars(&mut out, ' ', diff);
                out.push_str(col_name);
            } else {
                let left_pad = diff / 2;
                let right_pad = diff - left_pad;
                push_chars(&mut out, ' ', left_pad);
                out.push_str(col_name);
                push_chars(&mut out, ' ', right_pad);
            }
            out.push(' ');
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
            let pad = w.saturating_sub(display_width(val));
            out.push(' ');
            if is_numeric[i] {
                push_chars(&mut out, ' ', pad);
                out.push_str(val);
            } else {
                out.push_str(val);
                push_chars(&mut out, ' ', pad);
            }
            out.push(' ');
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

pub fn estimate_table_width(result: &QueryResult, tuples_only: bool, border: u8) -> usize {
    if result.columns.is_empty() {
        return 0;
    }
    let num_cols = result.columns.len();
    let mut total_w = 0;
    for (col_idx, col_name) in result.columns.iter().enumerate() {
        let mut max_w = if tuples_only { 0 } else { display_width(col_name) };
        for row in &result.rows {
            if let Some(val) = row.get(col_idx) {
                max_w = max_w.max(display_width(val));
            }
        }
        total_w += max_w;
    }

    match border {
        0 => total_w + num_cols.saturating_sub(1),
        _ => total_w + 1 + (num_cols * 3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_japanese_and_vietnamese_column_alignment() {
        let result = QueryResult {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec!["1".to_string(), "東京".to_string()],     // display width 4
                vec!["2".to_string(), "Việt".to_string()],     // display width 4
                vec!["3".to_string(), "abcdef".to_string()],   // display width 6
            ],
            command_tag: None,
            rows_affected: None,
            duration: std::time::Duration::from_millis(1),
        };

        let table = format_table(&result, false, 1).unwrap();
        let lines: Vec<&str> = table.lines().collect();
        // Header, separator, and all 3 data rows must have identical display width
        let expected_w = display_width(lines[1]); // ----+--------
        assert_eq!(display_width(lines[0]), expected_w);
        assert_eq!(display_width(lines[2]), expected_w);
        assert_eq!(display_width(lines[3]), expected_w);
        assert_eq!(display_width(lines[4]), expected_w);
    }
}
