use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "ruql",
    author = "Huy <dev@amsoftware.com.vn>",
    version = concat!(env!("CARGO_PKG_VERSION"), " (psql 17.4 compatibility)"),
    about = "ruql (RustSQL) - Next-Generation PostgreSQL and Amazon Redshift CLI\nFull psql compatibility with Redshift Lakehouse/External DB & Legacy Postgres support",
    disable_help_flag = true
)]
pub struct CliArgs {
    /// Print help
    #[arg(long = "help", action = clap::ArgAction::Help)]
    pub help: Option<bool>,

    /// Database server host or socket directory
    #[arg(short = 'h', long = "host", env = "PGHOST")]
    pub host: Option<String>,

    /// Database server port (default: 5432 for Postgres, 5439 for Redshift)
    #[arg(short = 'p', long = "port", env = "PGPORT")]
    pub port: Option<u16>,

    /// Database user name
    #[arg(short = 'U', long = "username", env = "PGUSER")]
    pub username: Option<String>,

    /// Database name to connect to
    #[arg(short = 'd', long = "dbname", env = "PGDATABASE")]
    pub dbname: Option<String>,

    /// Run only single command (SQL or internal) and exit
    #[arg(short = 'c', long = "command")]
    pub command: Option<String>,

    /// Execute commands from file, then exit
    #[arg(short = 'f', long = "file")]
    pub file: Option<String>,

    /// Echo all input from script
    #[arg(short = 'a', long = "echo-all")]
    pub echo_all: bool,

    /// Run quietly (no messages, only query output)
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Force password prompt
    #[arg(short = 'W', long = "password")]
    pub password_prompt: bool,

    /// Never prompt for password
    #[arg(short = 'w', long = "no-password")]
    pub no_password: bool,

    /// Turn on expanded table display mode (like \x)
    #[arg(short = 'x', long = "expanded")]
    pub expanded: bool,

    /// SSL connection mode: disable, prefer, require, verify-ca, verify-full
    #[arg(long = "sslmode")]
    pub sslmode: Option<String>,

    /// Disable TCP and session keepalive probes (or use PGKEEPALIVES=0 / ?keepalives=0)
    #[arg(long = "no-keepalive")]
    pub no_keepalive: bool,

    /// Output query results as CSV (requires -c/--command)
    #[arg(long = "csv", conflicts_with = "tsv")]
    pub csv: bool,

    /// Output query results as TSV (requires -c/--command)
    #[arg(long = "tsv", conflicts_with = "csv")]
    pub tsv: bool,

    /// Do not print column headers (tuples only)
    #[arg(
        long = "no-header",
        visible_alias = "no-headers",
        visible_alias = "tuples-only",
        short = 't'
    )]
    pub no_header: bool,

    /// Do not read startup files (~/.psqlrc, ~/.rsqlrc, ~/.redshiftrc)
    #[arg(short = 'X', long = "no-psqlrc")]
    pub no_psqlrc: bool,

    /// Do not use pager to display query results
    #[arg(long = "no-pager")]
    pub no_pager: bool,

    /// When to use pager: off, on, always
    #[arg(long = "pager")]
    pub pager: Option<String>,

    /// Output query results as JSON
    #[arg(long = "json")]
    pub json: bool,

    /// Output query results as Markdown table
    #[arg(long = "markdown")]
    pub markdown: bool,

    /// Self-update ruql to the latest release from GitHub
    #[arg(long = "update")]
    pub update: bool,

    /// Check if a newer version is available without updating
    #[arg(long = "check-update")]
    pub check_update: bool,

    /// Set rsql/psql variable NAME=VALUE
    #[arg(short = 'v', long = "set")]
    pub set_vars: Vec<String>,

    /// Connection URI or dbname (e.g. postgresql://user:pass@host:port/dbname)
    #[arg(value_name = "URI_OR_DBNAME")]
    pub conn_target: Option<String>,
}
