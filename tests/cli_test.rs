use clap::Parser;
use rsql::cli::CliArgs;
use rsql::config::ConnectionConfig;

#[test]
fn test_cli_args_parsing() {
    let args = CliArgs::parse_from(&[
        "rsql",
        "-h", "redshift-cluster.xyz.redshift.amazonaws.com",
        "-p", "5439",
        "-U", "awsuser",
        "-d", "dev",
        "-x",
        "--csv",
        "-c", "SELECT * FROM my_table;",
    ]);

    assert_eq!(args.host.unwrap(), "redshift-cluster.xyz.redshift.amazonaws.com");
    assert_eq!(args.port.unwrap(), 5439);
    assert_eq!(args.username.unwrap(), "awsuser");
    assert_eq!(args.dbname.unwrap(), "dev");
    assert!(args.expanded);
    assert!(args.csv);
    assert_eq!(args.command.unwrap(), "SELECT * FROM my_table;");
}

#[test]
fn test_connection_config_from_uri() {
    let args = CliArgs::parse_from(&[
        "rsql",
        "postgres://analyst:secretpass@datawarehouse.company.com:5432/analytics?sslmode=require",
    ]);

    let config = ConnectionConfig::from_cli(&args).unwrap();
    assert_eq!(config.host, "datawarehouse.company.com");
    assert_eq!(config.port, 5432);
    assert_eq!(config.user, "analyst");
    assert_eq!(config.password.unwrap(), "secretpass");
    assert_eq!(config.dbname, "analytics");
    assert_eq!(config.ssl_mode, rsql::config::SslMode::Require);
}
