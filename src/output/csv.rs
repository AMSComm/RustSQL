use super::QueryResult;
use crate::error::Result;

pub fn format_csv(result: &QueryResult) -> Result<String> {
    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);

    // Write header
    wtr.write_record(&result.columns)
        .map_err(|e| crate::error::RsqlError::Format(e.to_string()))?;

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
