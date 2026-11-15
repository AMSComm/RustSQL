use super::QueryResult;
use crate::error::Result;

pub fn format_expanded(result: &QueryResult) -> Result<String> {
    if result.columns.is_empty() {
        if let Some(tag) = &result.command_tag {
            return Ok(format!("{}\n", tag));
        } else {
            return Ok("(empty)\n".to_string());
        }
    }

    let mut out = String::new();
    let max_col_len = result.columns.iter().map(|c| c.len()).max().unwrap_or(10);

    for (row_idx, row) in result.rows.iter().enumerate() {
        let record_num = row_idx + 1;
        let header = format!("-[ RECORD {} ]", record_num);
        let padding_dashes = "-".repeat(40usize.saturating_sub(header.len()));
        out.push_str(&format!("{}{} \n", header, padding_dashes));

        for (col_idx, val) in row.iter().enumerate() {
            let col_name = &result.columns[col_idx];
            out.push_str(&format!(
                "{:width$} | {}\n",
                col_name,
                val,
                width = max_col_len
            ));
        }
    }

    let row_count = result.rows.len();
    out.push_str(&format!(
        "({} {})\n",
        row_count,
        if row_count == 1 { "row" } else { "rows" }
    ));

    Ok(out)
}
