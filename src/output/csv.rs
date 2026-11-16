use super::QueryResult;
use crate::error::Result;

pub fn format_delimited(
    result: &QueryResult,
    delimiter: u8,
    include_header: bool,
) -> Result<String> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(vec![]);

    // Write header if requested and columns exist
    if include_header && !result.columns.is_empty() {
        wtr.write_record(&result.columns)
            .map_err(|e| crate::error::RsqlError::Format(e.to_string()))?;
    }

    // Write data rows
    for row in &result.rows {
        wtr.write_record(row)
            .map_err(|e| crate::error::RsqlError::Format(e.to_string()))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| crate::error::RsqlError::Format(e.to_string()))?;
    let s = String::from_utf8(bytes)
        .map_err(|e| crate::error::RsqlError::Format(e.to_string()))?;

    Ok(s)
}

pub fn format_csv(result: &QueryResult, include_header: bool) -> Result<String> {
    format_delimited(result, b',', include_header)
}

pub fn format_tsv(result: &QueryResult, include_header: bool) -> Result<String> {
    format_delimited(result, b'\t', include_header)
}
