use super::QueryResult;
use crate::error::Result;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Table};

pub fn format_table(result: &QueryResult, tuples_only: bool) -> Result<String> {
    if result.columns.is_empty() {
        if tuples_only {
            return Ok("".to_string());
        }
        if let Some(tag) = &result.command_tag {
            return Ok(format!("{}\n", tag));
        } else if let Some(affected) = result.rows_affected {
            return Ok(format!("({} rows affected)\n", affected));
        } else {
            return Ok("(empty)\n".to_string());
        }
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    // Headers (only if not tuples_only)
    if !tuples_only {
        let header_cells: Vec<Cell> = result
            .columns
            .iter()
            .map(Cell::new)
            .collect();
        table.set_header(header_cells);
    }

    // Rows
    for row in &result.rows {
        let cells: Vec<Cell> = row.iter().map(Cell::new).collect();
        table.add_row(cells);
    }

    let mut out = table.to_string();

    // Footer (only if not tuples_only)
    if !tuples_only {
        let row_count = result.rows.len();
        out.push_str(&format!(
            "\n({} {})\n",
            row_count,
            if row_count == 1 { "row" } else { "rows" }
        ));
    } else {
        out.push('\n');
    }

    Ok(out)
}
