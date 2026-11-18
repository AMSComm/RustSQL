use thiserror::Error;

pub fn format_db_error(err: &tokio_postgres::Error) -> String {
    if let Some(db_err) = err.as_db_error() {
        let mut msg = format!("ERROR:  {}", db_err.message());
        if let Some(detail) = db_err.detail() {
            msg.push_str(&format!("\nDETAIL:  {}", detail));
        }
        if let Some(hint) = db_err.hint() {
            msg.push_str(&format!("\nHINT:  {}", hint));
        }
        if let Some(where_clause) = db_err.where_() {
            msg.push_str(&format!("\nCONTEXT:  {}", where_clause));
        }
        msg
    } else {
        err.to_string()
    }
}

pub fn format_notice_parts(
    severity: &str,
    message: &str,
    detail: Option<&str>,
    hint: Option<&str>,
    context: Option<&str>,
) -> String {
    let mut msg = format!("{}:  {}", severity, message);
    if let Some(detail) = detail {
        msg.push_str(&format!("\nDETAIL:  {}", detail));
    }
    if let Some(hint) = hint {
        msg.push_str(&format!("\nHINT:  {}", hint));
    }
    if let Some(where_clause) = context {
        msg.push_str(&format!("\nCONTEXT:  {}", where_clause));
    }
    msg
}

pub fn format_notice(db_err: &tokio_postgres::error::DbError) -> String {
    format_notice_parts(
        db_err.severity(),
        db_err.message(),
        db_err.detail(),
        db_err.hint(),
        db_err.where_(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_notice_parts_basic() {
        let formatted = format_notice_parts("NOTICE", "relation \"test\" already exists, skipping", None, None, None);
        assert_eq!(formatted, "NOTICE:  relation \"test\" already exists, skipping");
    }

    #[test]
    fn test_format_notice_parts_with_detail_hint_context() {
        let formatted = format_notice_parts(
            "NOTICE",
            "table created",
            Some("more info"),
            Some("try this instead"),
            Some("PL/pgSQL function inline_code_block line 3"),
        );
        let expected = "NOTICE:  table created\nDETAIL:  more info\nHINT:  try this instead\nCONTEXT:  PL/pgSQL function inline_code_block line 3";
        assert_eq!(formatted, expected);
    }
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum RsqlError {
    #[error("{0}")]
    Connection(String),

    #[error("{}", format_db_error(.0))]
    Database(#[from] tokio_postgres::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Command line error: {0}")]
    Cli(String),

    #[error("Meta command error: {0}")]
    MetaCommand(String),

    #[error("Formatting error: {0}")]
    Format(String),

    #[error("General error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RsqlError>;
