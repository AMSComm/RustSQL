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
    assert!(!args.tsv);
    assert!(!args.no_header);
    assert_eq!(args.command.unwrap(), "SELECT * FROM my_table;");
}

#[test]
fn test_cli_tsv_and_no_header_flag() {
    let args = CliArgs::parse_from(&[
        "rsql",
        "--tsv",
        "--no-header",
        "-c", "SELECT id, name FROM users;",
    ]);

    assert!(args.tsv);
    assert!(!args.csv);
    assert!(args.no_header);
    assert_eq!(args.command.unwrap(), "SELECT id, name FROM users;");

    // Test alias -t
    let args_short_t = CliArgs::parse_from(&[
        "rsql",
        "--csv",
        "-t",
        "-c", "SELECT 1;",
    ]);
    assert!(args_short_t.no_header);

    // Test alias --tuples-only
    let args_tuples_only = CliArgs::parse_from(&[
        "rsql",
        "--csv",
        "--tuples-only",
        "-c", "SELECT 1;",
    ]);
    assert!(args_tuples_only.no_header);
}

#[test]
fn test_cli_no_psqlrc_flag() {
    let args = CliArgs::parse_from(&[
        "rsql",
        "-X",
        "-c", "SELECT 1;",
    ]);
    assert!(args.no_psqlrc);

    let args_long = CliArgs::parse_from(&[
        "rsql",
        "--no-psqlrc",
        "-c", "SELECT 1;",
    ]);
    assert!(args_long.no_psqlrc);
}

#[test]
fn test_cli_csv_tsv_conflict() {
    let res = CliArgs::try_parse_from(&[
        "rsql",
        "--csv",
        "--tsv",
        "-c", "SELECT 1;",
    ]);
    assert!(res.is_err());
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
