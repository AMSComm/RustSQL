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

        let (client, version_str) = match config.ssl_mode {
            SslMode::Disable => {
                let (client, conn) = pg_config.connect(NoTls).await.map_err(|e| {
                    RsqlError::Connection(format!("Failed to connect without SSL: {}", e))
                })?;
                tokio::spawn(async move {
                    if let Err(e) = conn.await {
                        eprintln!("Connection warning: {}", e);
                    }
                });
                let v = query_version_string(&client).await?;
                (client, v)
            }
            SslMode::Require => {
                let tls_builder = TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build()
                    .map_err(|e| RsqlError::Connection(format!("Failed to build TLS connector: {}", e)))?;
                let connector = MakeTlsConnector::new(tls_builder);

                let (client, conn) = pg_config.connect(connector).await.map_err(|e| {
                    RsqlError::Connection(format!("SSL connection required but failed: {}", e))
                })?;
                tokio::spawn(async move {
                    if let Err(e) = conn.await {
                        eprintln!("Connection warning: {}", e);
                    }
                });
                let v = query_version_string(&client).await?;
                (client, v)
            }
            SslMode::Prefer => {
                // Try SSL first, fall back to NoTls if SSL fails
                let mut ssl_client = None;
                if let Ok(tls_builder) = TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build()
                {
                    let connector = MakeTlsConnector::new(tls_builder);
                    if let Ok((client, conn)) = pg_config.connect(connector).await {
                        tokio::spawn(async move {
                            if let Err(e) = conn.await {
                                eprintln!("Connection warning: {}", e);
                            }
                        });
                        if let Ok(v) = query_version_string(&client).await {
                            ssl_client = Some((client, v));
                        }
                    }
                }

                match ssl_client {
                    Some(pair) => pair,
                    None => {
                        // Fallback to plain connection
                        let (client, conn) = pg_config.connect(NoTls).await.map_err(|e| {
                            RsqlError::Connection(format!("Fallback unencrypted connection failed: {}", e))
                        })?;
                        tokio::spawn(async move {
                            if let Err(e) = conn.await {
                                eprintln!("Connection warning: {}", e);
                            }
                        });
                        let v = query_version_string(&client).await?;
                        (client, v)
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
