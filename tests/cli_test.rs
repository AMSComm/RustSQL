use clap::Parser;
use rsql::cli::CliArgs;
use rsql::config::ConnectionConfig;

#[test]
fn test_cli_args_parsing() {
    let args = CliArgs::parse_from([
        "rsql",
        "-h",
        "redshift-cluster.xyz.redshift.amazonaws.com",
        "-p",
        "5439",
        "-U",
        "awsuser",
        "-d",
        "dev",
        "-x",
        "--csv",
        "-c",
        "SELECT * FROM my_table;",
    ]);

    assert_eq!(
        args.host.unwrap(),
        "redshift-cluster.xyz.redshift.amazonaws.com"
    );
    assert_eq!(args.port.unwrap(), 5439);
    assert_eq!(args.username.unwrap(), "awsuser");
    assert_eq!(args.dbname.unwrap(), "dev");
    assert!(args.expanded);
    assert!(args.csv);
    assert!(!args.tsv);
    assert!(!args.no_header);
    assert_eq!(args.command.unwrap(), "SELECT * FROM my_table;");
}

#[test]
fn test_cli_tsv_and_no_header_flag() {
    let args = CliArgs::parse_from([
        "rsql",
        "--tsv",
        "--no-header",
        "-c",
        "SELECT id, name FROM users;",
    ]);

    assert!(args.tsv);
    assert!(!args.csv);
    assert!(args.no_header);
    assert_eq!(args.command.unwrap(), "SELECT id, name FROM users;");

    // Test alias -t
    let args_short_t = CliArgs::parse_from(["rsql", "--csv", "-t", "-c", "SELECT 1;"]);
    assert!(args_short_t.no_header);

    // Test alias --tuples-only
    let args_tuples_only =
        CliArgs::parse_from(["rsql", "--csv", "--tuples-only", "-c", "SELECT 1;"]);
    assert!(args_tuples_only.no_header);
}

#[test]
fn test_cli_no_psqlrc_flag() {
    let args = CliArgs::parse_from(["rsql", "-X", "-c", "SELECT 1;"]);
    assert!(args.no_psqlrc);

    let args_long = CliArgs::parse_from(["rsql", "--no-psqlrc", "-c", "SELECT 1;"]);
    assert!(args_long.no_psqlrc);
}

#[test]
fn test_cli_csv_tsv_conflict() {
    let res = CliArgs::try_parse_from(["rsql", "--csv", "--tsv", "-c", "SELECT 1;"]);
    assert!(res.is_err());
}

#[test]
fn test_connection_config_from_uri() {
    let args = CliArgs::parse_from([
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

#[test]
fn test_cli_sslmode_flags_and_precedence() {
    // Default is Prefer
    let args_default =
        CliArgs::parse_from(["rsql", "-h", "redshift.company.com", "-d", "dev", "-w"]);
    let config_default = ConnectionConfig::from_cli(&args_default).unwrap();
    assert_eq!(config_default.ssl_mode, rsql::config::SslMode::Prefer);

    // Explicit --sslmode disable
    let args_disable = CliArgs::parse_from([
        "rsql",
        "-h",
        "localhost",
        "-d",
        "test",
        "--sslmode",
        "disable",
        "-w",
    ]);
    let config_disable = ConnectionConfig::from_cli(&args_disable).unwrap();
    assert_eq!(config_disable.ssl_mode, rsql::config::SslMode::Disable);

    // Explicit --sslmode require
    let args_require = CliArgs::parse_from([
        "rsql",
        "-h",
        "db.internal",
        "-d",
        "prod",
        "--sslmode",
        "require",
        "-w",
    ]);
    let config_require = ConnectionConfig::from_cli(&args_require).unwrap();
    assert_eq!(config_require.ssl_mode, rsql::config::SslMode::Require);

    // CLI flag overrides URI sslmode
    let args_override = CliArgs::parse_from([
        "rsql",
        "postgres://analyst@warehouse.com:5432/analytics?sslmode=require",
        "--sslmode",
        "disable",
        "-w",
    ]);
    let config_override = ConnectionConfig::from_cli(&args_override).unwrap();
    assert_eq!(config_override.ssl_mode, rsql::config::SslMode::Disable);
}
