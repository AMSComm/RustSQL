use crate::cli::CliArgs;
use crate::error::{Result, RsqlError};
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub dbname: String,
    pub password: Option<String>,
    pub ssl_mode: SslMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl std::str::FromStr for SslMode {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "disable" => Ok(SslMode::Disable),
            "allow" => Ok(SslMode::Allow),
            "prefer" => Ok(SslMode::Prefer),
            "require" => Ok(SslMode::Require),
            "verify-ca" => Ok(SslMode::VerifyCa),
            "verify-full" => Ok(SslMode::VerifyFull),
            _ => Ok(SslMode::Prefer),
        }
    }
}

impl std::fmt::Display for SslMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SslMode::Disable => write!(f, "disable"),
            SslMode::Allow => write!(f, "allow"),
            SslMode::Prefer => write!(f, "prefer"),
            SslMode::Require => write!(f, "require"),
            SslMode::VerifyCa => write!(f, "verify-ca"),
            SslMode::VerifyFull => write!(f, "verify-full"),
        }
    }
}

impl SslMode {
    pub fn parse_mode(s: &str) -> Self {
        s.parse().unwrap_or(SslMode::Prefer)
    }
}

impl ConnectionConfig {
    pub fn from_cli(cli: &CliArgs) -> Result<Self> {
        // Priority 1: Positional target (if URI or dbname)
        let mut uri_parsed = None;
        let mut target_dbname = None;

        if let Some(target) = &cli.conn_target {
            if target.starts_with("postgres://") || target.starts_with("postgresql://") {
                uri_parsed = Some(parse_postgres_uri(target)?);
            } else {
                target_dbname = Some(target.clone());
            }
        }

        // Priority 2: DATABASE_URL env var if no URI target
        if uri_parsed.is_none() {
            if let Ok(db_url) = env::var("DATABASE_URL") {
                if db_url.starts_with("postgres://") || db_url.starts_with("postgresql://") {
                    uri_parsed = parse_postgres_uri(&db_url).ok();
                }
            }
        }

        let default_host = uri_parsed
            .as_ref()
            .map(|u| u.host.clone())
            .or_else(|| cli.host.clone())
            .or_else(|| env::var("PGHOST").ok())
            .unwrap_or_else(|| "localhost".to_string());

        let is_redshift = default_host.contains(".redshift.amazonaws.com")
            || default_host.contains(".redshift-serverless.amazonaws.com");

        let default_port = uri_parsed
            .as_ref()
            .and_then(|u| u.port)
            .or(cli.port)
            .or_else(|| env::var("PGPORT").ok().and_then(|p| p.parse().ok()))
            .unwrap_or(if is_redshift { 5439 } else { 5432 });

        let default_user = uri_parsed
            .as_ref()
            .and_then(|u| u.user.clone())
            .or_else(|| cli.username.clone())
            .or_else(|| env::var("PGUSER").ok())
            .or_else(|| env::var("USER").ok())
            .unwrap_or_else(|| "postgres".to_string());

        let default_dbname = target_dbname
            .or_else(|| uri_parsed.as_ref().and_then(|u| u.dbname.clone()))
            .or_else(|| cli.dbname.clone())
            .or_else(|| env::var("PGDATABASE").ok())
            .unwrap_or_else(|| default_user.clone());

        let mut password = uri_parsed
            .as_ref()
            .and_then(|u| u.password.clone())
            .or_else(|| env::var("PGPASSWORD").ok());

        // Check ~/.pgpass if no password yet
        if password.is_none() && !cli.no_password {
            password = find_pgpass_password(&default_host, default_port, &default_dbname, &default_user);
        }

        // Force password prompt if requested and still missing
        if (cli.password_prompt || (password.is_none() && !cli.no_password && cli.command.is_none() && cli.file.is_none()))
            && password.is_none()
        {
            if let Ok(entered) = rpassword::prompt_password(format!("Password for user {}: ", default_user)) {
                if !entered.is_empty() {
                    password = Some(entered);
                }
            }
        }

        let ssl_mode = cli
            .sslmode
            .as_deref()
            .map(SslMode::parse_mode)
            .or_else(|| uri_parsed.as_ref().and_then(|u| u.ssl_mode))
            .or_else(|| env::var("PGSSLMODE").ok().map(|s| SslMode::parse_mode(&s)))
            .unwrap_or(SslMode::Prefer);

        Ok(ConnectionConfig {
            host: default_host,
            port: default_port,
            user: default_user,
            dbname: default_dbname,
            password,
            ssl_mode,
        })
    }
}

#[derive(Default)]
struct ParsedUri {
    host: String,
    port: Option<u16>,
    user: Option<String>,
    password: Option<String>,
    dbname: Option<String>,
    ssl_mode: Option<SslMode>,
}

fn parse_postgres_uri(uri_str: &str) -> Result<ParsedUri> {
    // Basic parser for postgres://user:password@host:port/dbname?sslmode=...
    let stripped = uri_str
        .strip_prefix("postgresql://")
        .or_else(|| uri_str.strip_prefix("postgres://"))
        .ok_or_else(|| RsqlError::Cli("Invalid connection URI scheme".to_string()))?;

    let (auth_host, query) = match stripped.split_once('?') {
        Some((ah, q)) => (ah, Some(q)),
        None => (stripped, None),
    };

    let (user_info, host_db) = match auth_host.split_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, auth_host),
    };

    let mut user = None;
    let mut password = None;
    if let Some(u) = user_info {
        if let Some((uname, pass)) = u.split_once(':') {
            user = Some(uname.to_string());
            password = Some(pass.to_string());
        } else {
            user = Some(u.to_string());
        }
    }

    let (host_port, dbname) = match host_db.split_once('/') {
        Some((hp, db)) => (hp, if db.is_empty() { None } else { Some(db.to_string()) }),
        None => (host_db, None),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (
            if h.is_empty() { "localhost".to_string() } else { h.to_string() },
            p.parse::<u16>().ok(),
        ),
        None => (
            if host_port.is_empty() { "localhost".to_string() } else { host_port.to_string() },
            None,
        ),
    };

    let mut ssl_mode = None;
    if let Some(q) = query {
        for param in q.split('&') {
            if let Some((k, v)) = param.split_once('=') {
                if k.eq_ignore_ascii_case("sslmode") {
                    ssl_mode = Some(SslMode::parse_mode(v));
                }
            }
        }
    }

    Ok(ParsedUri {
        host,
        port,
        user,
        password,
        dbname,
        ssl_mode,
    })
}

fn find_pgpass_password(host: &str, port: u16, dbname: &str, user: &str) -> Option<String> {
    let pgpass_path = env::var("PGPASSFILE")
        .map(PathBuf::from)
        .ok()
        .or_else(|| dirs::home_dir().map(|h| h.join(".pgpass")))?;

    if !pgpass_path.exists() {
        return None;
    }

    let file = File::open(pgpass_path).ok()?;
    let reader = BufReader::new(file);

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => continue,
        };
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Format: hostname:port:database:username:password
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 5 {
            let h = parts[0];
            let p = parts[1];
            let d = parts[2];
            let u = parts[3];
            let pass = parts[4..].join(":");

            let host_match = h == "*" || h == host;
            let port_match = p == "*" || p == port.to_string();
            let db_match = d == "*" || d == dbname;
            let user_match = u == "*" || u == user;

            if host_match && port_match && db_match && user_match {
                return Some(pass);
            }
        }
    }

    None
}
