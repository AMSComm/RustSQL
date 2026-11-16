use rsql::output::csv::{format_csv, format_tsv};
use rsql::output::expanded::format_expanded;
use rsql::output::json::format_json;
use rsql::output::markdown::format_markdown;
use rsql::output::table::format_table;
use rsql::output::QueryResult;
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
fn test_table_formatting() {
    let res = sample_result();
    let output = format_table(&res, false).unwrap();
    assert!(output.contains("Alice"));
    assert!(output.contains("Bob"));
    assert!(output.contains("(2 rows)"));

    // Tuples only (no headers, no footer)
    let output_tuples = format_table(&res, true).unwrap();
    assert!(output_tuples.contains("Alice"));
    assert!(!output_tuples.contains("(2 rows)"));
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
