use thiserror::Error;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum RsqlError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Database error: {0}")]
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
