use rsql::meta::{handle_pset, parse_meta_command, AppState, MetaCommand};
use rsql::output::OutputFormat;

#[test]
fn test_parse_meta_commands() {
    assert!(matches!(parse_meta_command("\\q"), Some(MetaCommand::Quit)));
    assert!(matches!(parse_meta_command("\\quit"), Some(MetaCommand::Quit)));
    assert!(matches!(parse_meta_command("\\?"), Some(MetaCommand::HelpGeneral)));
    assert!(matches!(parse_meta_command("\\l"), Some(MetaCommand::ListDatabases)));
    assert!(matches!(parse_meta_command("\\list"), Some(MetaCommand::ListDatabases)));
    assert!(matches!(parse_meta_command("\\x"), Some(MetaCommand::ToggleExpanded(None))));
    assert!(matches!(parse_meta_command("\\timing"), Some(MetaCommand::ToggleTiming(None))));
    assert!(matches!(parse_meta_command("\\t"), Some(MetaCommand::ToggleTuplesOnly(None))));
    assert!(matches!(parse_meta_command("\\t on"), Some(MetaCommand::ToggleTuplesOnly(Some(true)))));
    assert!(matches!(parse_meta_command("\\t off"), Some(MetaCommand::ToggleTuplesOnly(Some(false)))));
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

#[test]
fn test_pset_options() {
    let mut state = AppState::default();

    // Test \pset format tsv
    handle_pset("format", Some("tsv"), &mut state);
    assert_eq!(state.format, OutputFormat::Tsv);

    // Test \pset format csv
    handle_pset("format", Some("csv"), &mut state);
    assert_eq!(state.format, OutputFormat::Csv);

    // Test \pset null 'NULL'
    handle_pset("null", Some("'[NULL]'"), &mut state);
    assert_eq!(state.null_str, "[NULL]");

    // Test \pset null ''
    handle_pset("null", Some("''"), &mut state);
    assert_eq!(state.null_str, "");

    // Test \pset tuples_only on
    handle_pset("tuples_only", Some("on"), &mut state);
    assert_eq!(state.tuples_only, true);

    // Test \pset tuples_only off
    handle_pset("tuples_only", Some("off"), &mut state);
    assert_eq!(state.tuples_only, false);

    // Test \pset fieldsep
    handle_pset("fieldsep", Some("','"), &mut state);
    assert_eq!(state.field_sep, ",");

    // Test \pset border
    handle_pset("border", Some("2"), &mut state);
    assert_eq!(state.border, 2);
    handle_pset("border", Some("0"), &mut state);
    assert_eq!(state.border, 0);
}
