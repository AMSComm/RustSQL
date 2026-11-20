use rustsql::output::csv::{format_csv, format_tsv};
use rustsql::output::expanded::format_expanded;
use rustsql::output::json::format_json;
use rustsql::output::markdown::format_markdown;
use rustsql::output::table::format_table;
use rustsql::output::QueryResult;
use std::time::Duration;

fn sample_result() -> QueryResult {
    QueryResult {
        columns: vec!["id".to_string(), "name".to_string(), "active".to_string()],
        rows: vec![
            vec!["1".to_string(), "Alice".to_string(), "t".to_string()],
            vec!["2".to_string(), "Bob".to_string(), "f".to_string()],
        ],
        command_tag: None,
        rows_affected: Some(2),
        duration: Duration::from_millis(42),
    }
}

fn sample_result_with_null() -> QueryResult {
    QueryResult {
        columns: vec!["id".to_string(), "nickname".to_string()],
        rows: vec![
            vec!["1".to_string(), "".to_string()],
            vec!["2".to_string(), "Bobby".to_string()],
        ],
        command_tag: None,
        rows_affected: Some(2),
        duration: Duration::from_millis(10),
    }
}

#[test]
fn test_table_formatting_psql_style() {
    let res = sample_result();

    // Default border 1 (standard psql format)
    let output = format_table(&res, false, 1).unwrap();
    assert!(output.contains(" id | name  | active \n"));
    assert!(output.contains("----+-------+--------\n"));
    assert!(output.contains("  1 | Alice | t      \n"));
    assert!(output.contains("  2 | Bob   | f      \n"));
    assert!(output.contains("(2 rows)\n"));
    // Ensure no unicode rounded box corners
    assert!(!output.contains("╭"));
    assert!(!output.contains("╰"));

    // Tuples only (no headers, no separator, no footer)
    let output_tuples = format_table(&res, true, 1).unwrap();
    assert!(!output_tuples.contains("id | name"));
    assert!(!output_tuples.contains("----+-------"));
    assert!(!output_tuples.contains("(2 rows)"));
    assert!(output_tuples.contains(" 1 | Alice | t \n"));
    assert!(output_tuples.contains(" 2 | Bob   | f \n"));

    // Border 2 (box border)
    let output_border2 = format_table(&res, false, 2).unwrap();
    assert!(output_border2.starts_with("+----+-------+--------+\n"));
    assert!(output_border2.contains("| id | name  | active |\n"));

    // Border 0 (no separators)
    let output_border0 = format_table(&res, false, 0).unwrap();
    assert!(!output_border0.contains("----+-------"));
    assert!(output_border0.contains("(2 rows)\n"));
}

#[test]
fn test_expanded_formatting() {
    let res = sample_result();
    let output = format_expanded(&res).unwrap();
    assert!(output.contains("-[ RECORD 1 ]"));
    assert!(output.contains("id     | 1"));
    assert!(output.contains("name   | Alice"));
    assert!(output.contains("-[ RECORD 2 ]"));
    assert!(output.contains("id     | 2"));
    assert!(output.contains("name   | Bob"));
}

#[test]
fn test_csv_formatting_with_and_without_headers() {
    let res = sample_result();

    // Default: with headers
    let with_header = format_csv(&res, true).unwrap();
    assert!(with_header.starts_with("id,name,active\n"));
    assert!(with_header.contains("1,Alice,t\n"));
    assert!(with_header.contains("2,Bob,f\n"));

    // No header (--no-header / -t)
    let no_header = format_csv(&res, false).unwrap();
    assert!(!no_header.contains("id,name,active"));
    assert!(no_header.starts_with("1,Alice,t\n"));
    assert!(no_header.contains("2,Bob,f\n"));
}

#[test]
fn test_tsv_formatting_with_and_without_headers() {
    let res = sample_result();

    // Default: with headers
    let with_header = format_tsv(&res, true).unwrap();
    assert!(with_header.starts_with("id\tname\tactive\n"));
    assert!(with_header.contains("1\tAlice\tt\n"));
    assert!(with_header.contains("2\tBob\tf\n"));

    // No header (--no-header / -t)
    let no_header = format_tsv(&res, false).unwrap();
    assert!(!no_header.contains("id\tname\tactive"));
    assert!(no_header.starts_with("1\tAlice\tt\n"));
    assert!(no_header.contains("2\tBob\tf\n"));
}

#[test]
fn test_csv_with_null_representation() {
    let res = sample_result_with_null();
    let csv_out = format_csv(&res, true).unwrap();
    assert!(csv_out.contains("1,\n"));
    assert!(csv_out.contains("2,Bobby\n"));
}

#[test]
fn test_json_formatting() {
    let res = sample_result();
    let output = format_json(&res).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["name"], "Alice");
    assert_eq!(parsed[1]["name"], "Bob");
}

#[test]
fn test_markdown_formatting() {
    let res = sample_result();
    let output = format_markdown(&res).unwrap();
    assert!(output.contains("| id | name | active |"));
    assert!(output.contains("| 1 | Alice | t |"));
    assert!(output.contains("| 2 | Bob | f |"));
}

#[test]
fn test_visual_lines_calculation() {
    use rustsql::output::pager::calculate_visual_lines;

    // 3 short lines on width 80 -> 3 visual lines
    let content = "line 1\nline 2\nline 3";
    assert_eq!(calculate_visual_lines(content, 80), 3);

    // 1 long line of 160 chars on width 80 -> 2 visual lines
    let long_line = "a".repeat(160);
    assert_eq!(calculate_visual_lines(&long_line, 80), 2);

    // 1 line of 81 chars on width 80 -> 2 visual lines
    let line_81 = "a".repeat(81);
    assert_eq!(calculate_visual_lines(&line_81, 80), 2);

    // Empty content -> 0 lines
    assert_eq!(calculate_visual_lines("", 80), 0);
}

#[test]
fn test_parse_pager_key_actions() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use rustsql::output::pager::{parse_pager_key, PagerAction};

    // 'r' and 'R' trigger ShowAll ("bung ra het")
    assert_eq!(
        parse_pager_key(KeyCode::Char('r'), KeyModifiers::NONE),
        Some(PagerAction::ShowAll)
    );
    assert_eq!(
        parse_pager_key(KeyCode::Char('R'), KeyModifiers::NONE),
        Some(PagerAction::ShowAll)
    );

    // Space and PageDown trigger NextPage
    assert_eq!(
        parse_pager_key(KeyCode::Char(' '), KeyModifiers::NONE),
        Some(PagerAction::NextPage)
    );
    assert_eq!(
        parse_pager_key(KeyCode::PageDown, KeyModifiers::NONE),
        Some(PagerAction::NextPage)
    );

    // Enter, Down, j trigger NextLine
    assert_eq!(
        parse_pager_key(KeyCode::Enter, KeyModifiers::NONE),
        Some(PagerAction::NextLine)
    );
    assert_eq!(
        parse_pager_key(KeyCode::Down, KeyModifiers::NONE),
        Some(PagerAction::NextLine)
    );
    assert_eq!(
        parse_pager_key(KeyCode::Char('j'), KeyModifiers::NONE),
        Some(PagerAction::NextLine)
    );

    // 'd' triggers HalfPage
    assert_eq!(
        parse_pager_key(KeyCode::Char('d'), KeyModifiers::NONE),
        Some(PagerAction::HalfPage)
    );

    // 'q', 'Q', Esc, Ctrl+C trigger Quit
    assert_eq!(
        parse_pager_key(KeyCode::Char('q'), KeyModifiers::NONE),
        Some(PagerAction::Quit)
    );
    assert_eq!(
        parse_pager_key(KeyCode::Char('Q'), KeyModifiers::NONE),
        Some(PagerAction::Quit)
    );
    assert_eq!(
        parse_pager_key(KeyCode::Esc, KeyModifiers::NONE),
        Some(PagerAction::Quit)
    );
    assert_eq!(
        parse_pager_key(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(PagerAction::Quit)
    );
}

#[test]
fn test_split_statements_complex() {
    use rustsql::output::split_statements;

    let sql = r#"
        CREATE TABLE t1 (id int);
        INSERT INTO t1 VALUES (1, 'val;with;semicolon');
        INSERT INTO t1 VALUES (2, "quoted;name");
        -- Comment with ; inside
        INSERT INTO t1 VALUES (3, $$dollar;quoted$$);
        /* Block comment with ; inside */
        SELECT * FROM t1;
    "#;

    let stmts = split_statements(sql);
    assert_eq!(stmts.len(), 5);
    assert!(stmts[0].starts_with("CREATE TABLE"));
    assert!(stmts[1].contains("'val;with;semicolon'"));
    assert!(stmts[2].contains("\"quoted;name\""));
    assert!(stmts[3].contains("$$dollar;quoted$$"));
    assert!(stmts[4].contains("SELECT * FROM t1"));
}

#[test]
fn test_extract_command_tag() {
    use rustsql::output::extract_command_tag;

    assert_eq!(extract_command_tag("CREATE TABLE t1 (id int)", 0), "CREATE TABLE");
    assert_eq!(extract_command_tag("CREATE VIEW v1 AS SELECT 1", 0), "CREATE VIEW");
    assert_eq!(extract_command_tag("CREATE INDEX idx ON t1 (id)", 0), "CREATE INDEX");
    assert_eq!(extract_command_tag("INSERT INTO t1 VALUES (1)", 1), "INSERT 0 1");
    assert_eq!(extract_command_tag("UPDATE t1 SET id = 2", 5), "UPDATE 5");
    assert_eq!(extract_command_tag("DELETE FROM t1", 3), "DELETE 3");
    assert_eq!(extract_command_tag("DROP TABLE t1", 0), "DROP TABLE");
    assert_eq!(extract_command_tag("ALTER TABLE t1 ADD col int", 0), "ALTER TABLE");
    assert_eq!(extract_command_tag("SELECT * FROM t1", 10), "SELECT");
    assert_eq!(extract_command_tag("-- comment\nINSERT INTO t1 VALUES (1)", 1), "INSERT 0 1");
}
