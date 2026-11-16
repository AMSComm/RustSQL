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
