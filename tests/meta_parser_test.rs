use rsql::meta::{parse_meta_command, MetaCommand};

#[test]
fn test_parse_meta_commands() {
    assert!(matches!(parse_meta_command("\\q"), Some(MetaCommand::Quit)));
    assert!(matches!(parse_meta_command("\\quit"), Some(MetaCommand::Quit)));
    assert!(matches!(parse_meta_command("\\?"), Some(MetaCommand::HelpGeneral)));
    assert!(matches!(parse_meta_command("\\l"), Some(MetaCommand::ListDatabases)));
    assert!(matches!(parse_meta_command("\\list"), Some(MetaCommand::ListDatabases)));
    assert!(matches!(parse_meta_command("\\x"), Some(MetaCommand::ToggleExpanded)));
    assert!(matches!(parse_meta_command("\\timing"), Some(MetaCommand::ToggleTiming)));
    assert!(matches!(parse_meta_command("\\conninfo"), Some(MetaCommand::ConnInfo)));
    assert!(matches!(parse_meta_command("\\ds"), Some(MetaCommand::ListDatashares)));
    assert!(matches!(parse_meta_command("\\dex"), Some(MetaCommand::ListExternalTables(None))));

    // Pattern matching
    if let Some(MetaCommand::ListTables(pat)) = parse_meta_command("\\dt users*") {
        assert_eq!(pat.unwrap(), "users*");
    } else {
        panic!("Expected \\dt with pattern");
    }

    // External partitions
    if let Some(MetaCommand::ListExternalPartitions(schema, table)) = parse_meta_command("\\dep glue_db.sales") {
        assert_eq!(schema.unwrap(), "glue_db");
        assert_eq!(table, "sales");
    } else {
        panic!("Expected \\dep with schema and table");
    }

    // Describe table
    if let Some(MetaCommand::Describe(schema, table)) = parse_meta_command("\\d public.customers") {
        assert_eq!(schema.unwrap(), "public");
        assert_eq!(table, "customers");
    } else {
        panic!("Expected \\d with schema and table");
    }

    // Connect command
    if let Some(MetaCommand::Connect(db, user, host, port)) = parse_meta_command("\\c analytics alice redshifthost 5439") {
        assert_eq!(db, "analytics");
        assert_eq!(user.unwrap(), "alice");
        assert_eq!(host.unwrap(), "redshifthost");
        assert_eq!(port.unwrap(), 5439);
    } else {
        panic!("Expected \\c command");
    }
}
