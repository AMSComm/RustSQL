use crate::config::{ConnectionConfig, SslMode};
use crate::dialect::{create_dialect, Dialect, EngineDialect};
use crate::error::{Result, RsqlError};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use tokio_postgres::{Client, NoTls};

pub struct DbConnection {
    pub client: Client,
    pub dialect: Box<dyn Dialect>,
    #[allow(dead_code)]
    pub engine: EngineDialect,
    pub config: ConnectionConfig,
    pub server_version: String,
    pub is_superuser: bool,
    pub is_ssl: bool,
}

enum ConnectResult<S> {
    Ok(Box<(Client, tokio_postgres::Connection<tokio_postgres::Socket, S>)>),
    Err(tokio_postgres::Error),
    TimedOut,
}

async fn do_connect<T>(
    pg_config: &tokio_postgres::Config,
    tls: T,
    timeout_dur: std::time::Duration,
) -> ConnectResult<T::Stream>
where
    T: tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket> + Send + 'static,
    T::TlsConnect: Send,
    T::Stream: Send,
{
    match tokio::time::timeout(timeout_dur, pg_config.connect(tls)).await {
        Ok(Ok((client, conn))) => ConnectResult::Ok(Box::new((client, conn))),
        Ok(Err(e)) => ConnectResult::Err(e),
        Err(_) => ConnectResult::TimedOut,
    }
}

fn create_tls_connector(ssl_mode: SslMode) -> Result<MakeTlsConnector> {
    let mut builder = TlsConnector::builder();
    match ssl_mode {
        SslMode::Disable => {
            return Err(RsqlError::Connection(
                "Cannot create TLS connector for Disable mode".to_string(),
            ));
        }
        SslMode::Allow | SslMode::Prefer | SslMode::Require => {
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
        }
        SslMode::VerifyCa => {
            builder.danger_accept_invalid_hostnames(true);
        }
        SslMode::VerifyFull => {
            // Strict verification against system CAs and hostname
        }
    }

    let tls = builder
        .build()
        .map_err(|e| RsqlError::Connection(format!("Failed to build TLS connector: {}", e)))?;
    Ok(MakeTlsConnector::new(tls))
}

pub fn format_error_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = Vec::new();
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = current {
        let msg = e.to_string().trim().to_string();
        if !msg.is_empty() && !parts.contains(&msg) {
            parts.push(msg);
        }
        current = e.source();
    }
    if parts.is_empty() {
        "unknown error".to_string()
    } else {
        parts.join(": ")
    }
}

pub fn get_io_error<'a>(err: &'a (dyn std::error::Error + 'static)) -> Option<&'a std::io::Error> {
    let mut curr: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = curr {
        if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
            return Some(io_err);
        }
        curr = e.source();
    }
    None
}

pub fn is_network_failure(err: &(dyn std::error::Error + 'static)) -> bool {
    if let Some(io_err) = get_io_error(err) {
        matches!(
            io_err.kind(),
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::NetworkUnreachable
                | std::io::ErrorKind::NotFound
        )
    } else {
        let chain = format_error_chain(err).to_lowercase();
        chain.contains("failed to lookup address")
            || chain.contains("nodename nor servname provided")
            || chain.contains("name or service not known")
    }
}

pub fn format_connection_error(
    err: &(dyn std::error::Error + 'static),
    host: &str,
    port: u16,
) -> String {
    // Check for Postgres/Redshift DbError
    let mut curr: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = curr {
        if let Some(pg_err) = e.downcast_ref::<tokio_postgres::Error>() {
            if let Some(db_err) = pg_err.as_db_error() {
                let mut msg = format!("FATAL: {}", db_err.message());
                if let Some(detail) = db_err.detail() {
                    msg.push_str(&format!("\nDETAIL: {}", detail));
                }
                if let Some(hint) = db_err.hint() {
                    msg.push_str(&format!("\nHINT: {}", hint));
                }
                return format!(
                    "connection to server at \"{}\", port {} failed: {}",
                    host, port, msg
                );
            }
        }
        curr = e.source();
    }

    // Check for standard IO network errors
    if let Some(io_err) = get_io_error(err) {
        match io_err.kind() {
            std::io::ErrorKind::ConnectionRefused => {
                return format!(
                    "connection to server at \"{}\", port {} failed: Connection refused\n\tIs the server running on that host and accepting TCP/IP connections?",
                    host, port
                );
            }
            std::io::ErrorKind::TimedOut => {
                return format!(
                    "connection to server at \"{}\", port {} failed: Operation timed out\n\tIs the server running on that host and accepting TCP/IP connections?\n\tCheck your VPN, AWS Security Group, and network routing.",
                    host, port
                );
            }
            std::io::ErrorKind::NetworkUnreachable | std::io::ErrorKind::NotFound => {
                return format!(
                    "connection to server at \"{}\", port {} failed: Network unreachable",
                    host, port
                );
            }
            _ => {}
        }
    }

    let full_chain = format_error_chain(err);
    if full_chain.contains("failed to lookup address")
        || full_chain.contains("nodename nor servname provided")
        || full_chain.contains("name or service not known")
    {
        return format!(
            "could not translate host name \"{}\" to address: nodename nor servname provided, or not known",
            host
        );
    }

    format!(
        "connection to server at \"{}\", port {} failed: {}",
        host, port, full_chain
    )
}

impl DbConnection {
    pub async fn connect(config: ConnectionConfig) -> Result<Self> {
        let app_name = std::env::var("PGAPPNAME").unwrap_or_else(|_| "psql".to_string());

        let mut pg_config = tokio_postgres::Config::new();
        pg_config
            .host(&config.host)
            .port(config.port)
            .user(&config.user)
            .dbname(&config.dbname)
            .application_name(&app_name);

        if let Some(pass) = &config.password {
            pg_config.password(pass);
        }

        let timeout_secs = std::env::var("PGCONNECT_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(20);
        let timeout_dur = std::time::Duration::from_secs(timeout_secs);

        let (client, version_str, is_ssl) = match config.ssl_mode {
            SslMode::Disable => {
                match do_connect(&pg_config, NoTls, timeout_dur).await {
                    ConnectResult::Ok(boxed) => {
                        let (client, conn) = *boxed;
                        tokio::spawn(async move {
                            if let Err(e) = conn.await {
                                eprintln!("Connection warning: {}", e);
                            }
                        });
                        let v = query_version_string(&client).await?;
                        (client, v, false)
                    }
                    ConnectResult::Err(e) => {
                        return Err(RsqlError::Connection(format_connection_error(
                            &e,
                            &config.host,
                            config.port,
                        )));
                    }
                    ConnectResult::TimedOut => {
                        return Err(RsqlError::Connection(format!(
                            "connection to server at \"{}\", port {} failed: Operation timed out\n\tIs the server running on that host and accepting TCP/IP connections?\n\tCheck your VPN, AWS Security Group, and network routing.",
                            config.host, config.port
                        )));
                    }
                }
            }
            SslMode::Require | SslMode::VerifyCa | SslMode::VerifyFull => {
                let connector = create_tls_connector(config.ssl_mode)?;
                match do_connect(&pg_config, connector, timeout_dur).await {
                    ConnectResult::Ok(boxed) => {
                        let (client, conn) = *boxed;
                        tokio::spawn(async move {
                            if let Err(e) = conn.await {
                                eprintln!("Connection warning: {}", e);
                            }
                        });
                        let v = query_version_string(&client).await?;
                        (client, v, true)
                    }
                    ConnectResult::TimedOut => {
                        return Err(RsqlError::Connection(format!(
                            "connection to server at \"{}\", port {} failed: Operation timed out\n\tIs the server running on that host and accepting TCP/IP connections?\n\tCheck your VPN, AWS Security Group, and network routing.",
                            config.host, config.port
                        )));
                    }
                    ConnectResult::Err(e) => {
                        return Err(RsqlError::Connection(format_connection_error(
                            &e,
                            &config.host,
                            config.port,
                        )));
                    }
                }
            }
            SslMode::Prefer => {
                let connector = create_tls_connector(SslMode::Prefer)?;
                match do_connect(&pg_config, connector, timeout_dur).await {
                    ConnectResult::Ok(boxed) => {
                        let (client, conn) = *boxed;
                        tokio::spawn(async move {
                            if let Err(e) = conn.await {
                                eprintln!("Connection warning: {}", e);
                            }
                        });
                        let v = query_version_string(&client).await?;
                        (client, v, true)
                    }
                    ConnectResult::TimedOut => {
                        return Err(RsqlError::Connection(format!(
                            "connection to server at \"{}\", port {} failed: Operation timed out\n\tIs the server running on that host and accepting TCP/IP connections?\n\tCheck your VPN, AWS Security Group, and network routing.",
                            config.host, config.port
                        )));
                    }
                    ConnectResult::Err(err) => {
                        // If network / TCP level failure, don't fall back to NoTls as TCP socket itself cannot connect
                        if is_network_failure(&err) {
                            return Err(RsqlError::Connection(format_connection_error(
                                &err,
                                &config.host,
                                config.port,
                            )));
                        }

                        // Server does not support SSL: fall back to unencrypted
                        match do_connect(&pg_config, NoTls, timeout_dur).await {
                            ConnectResult::Ok(boxed) => {
                                let (client, conn) = *boxed;
                                tokio::spawn(async move {
                                    if let Err(e) = conn.await {
                                        eprintln!("Connection warning: {}", e);
                                    }
                                });
                                let v = query_version_string(&client).await?;
                                (client, v, false)
                            }
                            ConnectResult::TimedOut => {
                                return Err(RsqlError::Connection(format!(
                                    "connection to server at \"{}\", port {} failed: Operation timed out\n\tIs the server running on that host and accepting TCP/IP connections?",
                                    config.host, config.port
                                )));
                            }
                            ConnectResult::Err(fallback_err) => {
                                return Err(RsqlError::Connection(format_connection_error(
                                    &fallback_err,
                                    &config.host,
                                    config.port,
                                )));
                            }
                        }
                    }
                }
            }
            SslMode::Allow => {
                match do_connect(&pg_config, NoTls, timeout_dur).await {
                    ConnectResult::Ok(boxed) => {
                        let (client, conn) = *boxed;
                        tokio::spawn(async move {
                            if let Err(e) = conn.await {
                                eprintln!("Connection warning: {}", e);
                            }
                        });
                        let v = query_version_string(&client).await?;
                        (client, v, false)
                    }
                    ConnectResult::TimedOut => {
                        return Err(RsqlError::Connection(format!(
                            "connection to server at \"{}\", port {} failed: Operation timed out\n\tIs the server running on that host and accepting TCP/IP connections?",
                            config.host, config.port
                        )));
                    }
                    ConnectResult::Err(e) => {
                        if is_network_failure(&e) {
                            return Err(RsqlError::Connection(format_connection_error(
                                &e,
                                &config.host,
                                config.port,
                            )));
                        }
                        let connector = create_tls_connector(SslMode::Allow)?;
                        match do_connect(&pg_config, connector, timeout_dur).await {
                            ConnectResult::Ok(boxed) => {
                                let (client, conn) = *boxed;
                                tokio::spawn(async move {
                                    if let Err(e) = conn.await {
                                        eprintln!("Connection warning: {}", e);
                                    }
                                });
                                let v = query_version_string(&client).await?;
                                (client, v, true)
                            }
                            ConnectResult::TimedOut => {
                                return Err(RsqlError::Connection(format!(
                                    "connection to server at \"{}\", port {} failed: Operation timed out",
                                    config.host, config.port
                                )));
                            }
                            ConnectResult::Err(ssl_err) => {
                                return Err(RsqlError::Connection(format_connection_error(
                                    &ssl_err,
                                    &config.host,
                                    config.port,
                                )));
                            }
                        }
                    }
                }
            }
        };

        // Detect engine and version
        let engine = detect_engine(&client, &version_str).await?;
        let dialect = create_dialect(&engine);
        let is_superuser = check_superuser(&client).await.unwrap_or(false);

        Ok(DbConnection {
            client,
            dialect,
            engine,
            config,
            server_version: version_str,
            is_superuser,
            is_ssl,
        })
    }
}

async fn query_version_string(client: &Client) -> Result<String> {
    let rows = client
        .query("SELECT version();", &[])
        .await
        .map_err(RsqlError::Database)?;

    if let Some(row) = rows.first() {
        let v: String = row.get(0);
        Ok(v)
    } else {
        Ok("Unknown PostgreSQL Server".to_string())
    }
}

async fn detect_engine(client: &Client, version_str: &str) -> Result<EngineDialect> {
    // Check for Amazon Redshift
    let lower_v = version_str.to_lowercase();
    if lower_v.contains("redshift") {
        return Ok(EngineDialect::Redshift);
    }

    // Try SHOW server_version_num;
    if let Ok(rows) = client.query("SHOW server_version_num;", &[]).await {
        if let Some(row) = rows.first() {
            let num_str: String = row.get(0);
            if let Ok(num) = num_str.parse::<u32>() {
                if num >= 120000 {
                    return Ok(EngineDialect::PostgresModern(num));
                } else {
                    return Ok(EngineDialect::PostgresLegacy(num));
                }
            }
        }
    }

    // Fallback: parse version string like "PostgreSQL 14.5"
    if let Some(pos) = lower_v.find("postgresql ") {
        let after = &lower_v[pos + 11..];
        let ver_part = after.split_whitespace().next().unwrap_or("0");
        let major = ver_part.split('.').next().unwrap_or("0").parse::<u32>().unwrap_or(0);
        let num = major * 10000;
        if num >= 120000 {
            return Ok(EngineDialect::PostgresModern(num));
        } else {
            return Ok(EngineDialect::PostgresLegacy(num));
        }
    }

    Ok(EngineDialect::PostgresModern(140000))
}

async fn check_superuser(client: &Client) -> Result<bool> {
    // Redshift or PG check
    if let Ok(rows) = client
        .query(
            "SELECT usesuper FROM pg_user WHERE usename = current_user;",
            &[],
        )
        .await
    {
        if let Some(row) = rows.first() {
            let is_super: bool = row.get(0);
            return Ok(is_super);
        }
    }
    Ok(false)
}

pub fn parse_server_short_version(version_str: &str, is_redshift: bool) -> String {
    let lower = version_str.to_lowercase();
    let num_part = if let Some(pos) = lower.find("postgresql ") {
        let after = &version_str[pos + 11..];
        after.split_whitespace().next().unwrap_or("8.0.2")
    } else {
        "8.0.2"
    };

    if is_redshift || lower.contains("redshift") {
        format!("{} - Amazon Redshift", num_part)
    } else {
        num_part.to_string()
    }
}
