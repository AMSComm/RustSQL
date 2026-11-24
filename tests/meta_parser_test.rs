use rustsql::meta::{handle_pset, parse_meta_command, AppState, MetaCommand};
use rustsql::output::OutputFormat;

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
    assert!(matches!(parse_meta_command("\\ds"), Some(MetaCommand::ListSequences(None))));
    assert!(matches!(parse_meta_command("\\dsh"), Some(MetaCommand::ListDatashares)));
    assert!(matches!(parse_meta_command("\\dex"), Some(MetaCommand::ListExternalTables(None))));

    // \df and \df+
    assert!(matches!(parse_meta_command("\\df"), Some(MetaCommand::ListFunctions(None, false))));
    assert!(matches!(parse_meta_command("\\df+"), Some(MetaCommand::ListFunctions(None, true))));
    if let Some(MetaCommand::ListFunctions(pat, verbose)) = parse_meta_command("\\df p_export*") {
        assert_eq!(pat.unwrap(), "p_export*");
        assert!(!verbose);
    } else {
        panic!("Expected \\df with pattern");
    }
    if let Some(MetaCommand::ListFunctions(pat, verbose)) = parse_meta_command("\\df+ maindata.*") {
        assert_eq!(pat.unwrap(), "maindata.*");
        assert!(verbose);
    } else {
        panic!("Expected \\df+ with pattern");
    }

    // \sf and \sf+
    if let Some(MetaCommand::ShowFunction(name, verbose)) = parse_meta_command("\\sf p_export_kogakuhin_data") {
        assert_eq!(name, "p_export_kogakuhin_data");
        assert!(!verbose);
    } else {
        panic!("Expected \\sf command");
    }
    if let Some(MetaCommand::ShowFunction(name, verbose)) = parse_meta_command("\\sf+ maindata.proc1") {
        assert_eq!(name, "maindata.proc1");
        assert!(verbose);
    } else {
        panic!("Expected \\sf+ command");
    }

    // \di, \dm, \dT, \dp, \z, \dx
    assert!(matches!(parse_meta_command("\\di"), Some(MetaCommand::ListIndexes(None, false))));
    assert!(matches!(parse_meta_command("\\di+"), Some(MetaCommand::ListIndexes(None, true))));
    assert!(matches!(parse_meta_command("\\dm"), Some(MetaCommand::ListMaterializedViews(None))));
    assert!(matches!(parse_meta_command("\\dT"), Some(MetaCommand::ListTypes(None, false))));
    assert!(matches!(parse_meta_command("\\dT+"), Some(MetaCommand::ListTypes(None, true))));
    assert!(matches!(parse_meta_command("\\dp"), Some(MetaCommand::ListPrivileges(None))));
    assert!(matches!(parse_meta_command("\\z"), Some(MetaCommand::ListPrivileges(None))));
    assert!(matches!(parse_meta_command("\\dx"), Some(MetaCommand::ListExtensions(None))));

    // Operating system / session commands: \cd, \!, \o, \watch
    assert!(matches!(parse_meta_command("\\cd"), Some(MetaCommand::ChangeDirectory(None))));
    if let Some(MetaCommand::ChangeDirectory(Some(dir))) = parse_meta_command("\\cd /tmp") {
        assert_eq!(dir, "/tmp");
    } else {
        panic!("Expected \\cd /tmp");
    }

    assert!(matches!(parse_meta_command("\\!"), Some(MetaCommand::Shell(None))));
    if let Some(MetaCommand::Shell(Some(cmd))) = parse_meta_command("\\! ls -la") {
        assert_eq!(cmd, "ls -la");
    } else {
        panic!("Expected \\! ls -la");
    }

    assert!(matches!(parse_meta_command("\\o"), Some(MetaCommand::Out(None))));
    if let Some(MetaCommand::Out(Some(f))) = parse_meta_command("\\o /tmp/out.csv") {
        assert_eq!(f, "/tmp/out.csv");
    } else {
        panic!("Expected \\o /tmp/out.csv");
    }

    assert!(matches!(parse_meta_command("\\watch"), Some(MetaCommand::Watch(None))));
    assert!(matches!(parse_meta_command("\\watch 5"), Some(MetaCommand::Watch(Some(5)))));

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
    assert!(state.tuples_only);

    // Test \pset tuples_only off
    handle_pset("tuples_only", Some("off"), &mut state);
    assert!(!state.tuples_only);

    // Test \pset fieldsep
    handle_pset("fieldsep", Some("','"), &mut state);
    assert_eq!(state.field_sep, ",");

    // Test \pset border
    handle_pset("border", Some("2"), &mut state);
    assert_eq!(state.border, 2);
    handle_pset("border", Some("0"), &mut state);
    assert_eq!(state.border, 0);

    // Test \pset pager
    handle_pset("pager", Some("off"), &mut state);
    assert_eq!(state.pager, rustsql::meta::PagerMode::Off);
    handle_pset("pager", Some("on"), &mut state);
    assert_eq!(state.pager, rustsql::meta::PagerMode::On);

    // Test \pset expanded
    handle_pset("expanded", Some("on"), &mut state);
    assert_eq!(state.expanded, rustsql::meta::ExpandedMode::On);
}
