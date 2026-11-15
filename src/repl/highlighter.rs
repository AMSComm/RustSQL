use colored::Colorize;
use rustyline::highlight::{CmdKind, Highlighter};
use std::borrow::Cow;

#[derive(Clone, Default)]
pub struct SqlHighlighter;

impl Highlighter for SqlHighlighter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let trimmed = line.trim_start();
        if trimmed.starts_with('\\') {
            return Cow::Owned(line.magenta().bold().to_string());
        }

        if trimmed.starts_with("--") {
            return Cow::Owned(line.dimmed().to_string());
        }

        let mut highlighted = String::with_capacity(line.len() * 2);
        let mut in_quote = false;
        let mut current_word = String::new();

        for c in line.chars() {
            if c == '\'' {
                if !current_word.is_empty() {
                    highlighted.push_str(&format_token(&current_word));
                    current_word.clear();
                }
                in_quote = !in_quote;
                highlighted.push(c);
            } else if in_quote {
                highlighted.push(c);
            } else if c.is_alphanumeric() || c == '_' {
                current_word.push(c);
            } else {
                if !current_word.is_empty() {
                    highlighted.push_str(&format_token(&current_word));
                    current_word.clear();
                }
                highlighted.push(c);
            }
        }

        if !current_word.is_empty() {
            highlighted.push_str(&format_token(&current_word));
        }

        Cow::Owned(highlighted)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: CmdKind) -> bool {
        true
    }
}

fn format_token(word: &str) -> String {
    let upper = word.to_uppercase();
    match upper.as_str() {
        "SELECT" | "FROM" | "WHERE" | "JOIN" | "LEFT" | "RIGHT" | "FULL" | "INNER"
        | "ON" | "GROUP" | "BY" | "ORDER" | "HAVING" | "LIMIT" | "OFFSET"
        | "INSERT" | "INTO" | "VALUES" | "UPDATE" | "SET" | "DELETE"
        | "CREATE" | "TABLE" | "SCHEMA" | "DATABASE" | "EXTERNAL" | "DROP" | "ALTER"
        | "UNION" | "ALL" | "DISTINCT" | "AS" | "CASE" | "WHEN" | "THEN" | "ELSE"
        | "END" | "AND" | "OR" | "NOT" | "IN" | "IS" | "NULL" | "LIKE" | "ILIKE"
        | "WITH" | "DATA" | "CATALOG" | "IAM_ROLE" | "SPECTRUM" => {
            word.cyan().bold().to_string()
        }
        _ => {
            if word.chars().all(|c| c.is_ascii_digit()) {
                word.yellow().to_string()
            } else {
                word.to_string()
            }
        }
    }
}
