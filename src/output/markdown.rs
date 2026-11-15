use super::QueryResult;
use crate::error::Result;

pub fn format_markdown(result: &QueryResult) -> Result<String> {
    if result.columns.is_empty() {
        return Ok("".to_string());
    }

    let mut out = String::new();

    // Header row
    out.push_str("| ");
    out.push_str(&result.columns.join(" | "));
    out.push_str(" |\n");

    // Separator row
    out.push_str("| ");
    let separators: Vec<String> = result.columns.iter().map(|c| "-".repeat(c.len().max(3))).collect();
    out.push_str(&separators.join(" | "));
    out.push_str(" |\n");

    // Data rows
    for row in &result.rows {
        out.push_str("| ");
        let escaped: Vec<String> = row.iter().map(|val| val.replace('|', "\\|")).collect();
        out.push_str(&escaped.join(" | "));
        out.push_str(" |\n");
    }

    out.push('\n');
    Ok(out)
}
