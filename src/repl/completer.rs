use rustyline::completion::{Completer, Pair};
use rustyline::Context;
use std::sync::{Arc, RwLock};

#[derive(Clone, Default, Debug)]
pub struct CompletionCache {
    pub tables: Vec<String>,
    pub schemas: Vec<String>,
    pub views: Vec<String>,
    pub databases: Vec<String>,
    pub columns: Vec<String>,
}

#[derive(Clone, Default)]
pub struct SqlCompleter {
    pub cache: Arc<RwLock<CompletionCache>>,
}

pub const SQL_KEYWORDS: &[&str] = &[
    // Basic Query & DML
    "SELECT", "FROM", "WHERE", "GROUP BY", "HAVING", "ORDER BY", "LIMIT", "OFFSET",
    "JOIN", "LEFT JOIN", "RIGHT JOIN", "FULL JOIN", "INNER JOIN", "CROSS JOIN",
    "ON", "USING", "AS", "DISTINCT", "ALL",
    "INSERT INTO", "VALUES", "UPDATE", "SET", "DELETE FROM", "TRUNCATE",
    "UNION", "UNION ALL", "INTERSECT", "EXCEPT",
    "RETURNING", "FETCH FIRST", "ROWS ONLY",

    // Conditions & Operators
    "AND", "OR", "NOT", "IN", "IS NULL", "IS NOT NULL", "IS TRUE", "IS FALSE",
    "LIKE", "ILIKE", "SIMILAR TO", "BETWEEN", "EXISTS", "ANY", "SOME",
    "CASE", "WHEN", "THEN", "ELSE", "END",
    "ASC", "DESC", "NULLS FIRST", "NULLS LAST",

    // DDL & Object Management
    "CREATE TABLE", "CREATE TABLE IF NOT EXISTS", "CREATE TEMP TABLE",
    "CREATE VIEW", "CREATE OR REPLACE VIEW", "CREATE MATERIALIZED VIEW",
    "CREATE SCHEMA", "CREATE SCHEMA IF NOT EXISTS", "CREATE DATABASE",
    "CREATE INDEX", "CREATE UNIQUE INDEX",
    "DROP TABLE", "DROP TABLE IF EXISTS", "DROP VIEW", "DROP VIEW IF EXISTS",
    "DROP SCHEMA", "DROP SCHEMA IF EXISTS", "DROP DATABASE", "DROP INDEX",
    "ALTER TABLE", "ALTER SCHEMA", "ALTER DATABASE", "ALTER VIEW",
    "ADD COLUMN", "DROP COLUMN", "RENAME COLUMN", "RENAME TO",
    "CASCADE", "RESTRICT",

    // Constraints & Keys
    "PRIMARY KEY", "FOREIGN KEY", "REFERENCES", "CONSTRAINT", "CHECK", "UNIQUE",
    "DEFAULT", "NOT NULL", "NULL",

    // Data Types
    "BIGINT", "BIGSERIAL", "BOOLEAN", "BOOL", "BYTEA", "CHAR", "CHARACTER VARYING",
    "DATE", "DECIMAL", "DOUBLE PRECISION", "FLOAT", "FLOAT8", "INT", "INTEGER",
    "INTERVAL", "JSON", "JSONB", "NUMERIC", "REAL", "SERIAL", "SMALLINT",
    "TEXT", "TIME", "TIMESTAMP", "TIMESTAMPTZ", "UUID", "VARCHAR",

    // Aggregate & Window Functions
    "AVG", "COUNT", "MAX", "MIN", "SUM",
    "ARRAY_AGG", "STRING_AGG",
    "ROW_NUMBER() OVER ()", "RANK() OVER ()", "DENSE_RANK() OVER ()",
    "LEAD", "LAG", "FIRST_VALUE", "LAST_VALUE", "PARTITION BY", "OVER",

    // Scalar Functions
    "ABS", "CAST", "CEIL", "COALESCE", "CONCAT", "CURRENT_DATE", "CURRENT_TIME",
    "CURRENT_TIMESTAMP", "DATE_TRUNC", "EXTRACT", "FLOOR", "GEN_RANDOM_UUID()",
    "GREATEST", "LEAST", "LENGTH", "LOWER", "NOW()", "NULLIF", "REPLACE",
    "ROUND", "SUBSTRING", "TRIM", "UPPER",

    // Transaction & Session Controls
    "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION", "START TRANSACTION",
    "SAVEPOINT", "RELEASE SAVEPOINT", "ROLLBACK TO SAVEPOINT",
    "SHOW", "RESET", "VACUUM", "ANALYZE", "EXPLAIN", "EXPLAIN ANALYZE",
    "VERBOSE", "BUFFERS", "GRANT", "REVOKE", "TO", "ALL PRIVILEGES",
    "FOR UPDATE", "FOR SHARE",

    // Amazon Redshift Specific
    "AUTO", "COMPOUND SORTKEY", "COPY", "DATA CATALOG", "DATA SHARE",
    "DISTKEY", "DISTSTYLE", "ENCODE", "EVEN", "EXTERNAL SCHEMA",
    "EXTERNAL TABLE", "FORMAT AS CSV", "FORMAT AS JSON", "FORMAT AS PARQUET",
    "IAM_ROLE", "INTERLEAVED SORTKEY", "KEY", "LZO", "RAW", "REGION",
    "RUNLENGTH", "SORTKEY", "UNLOAD", "VACUUM FULL", "ZSTD",
];

pub const META_COMMANDS: &[&str] = &[
    "\\q", "\\?", "\\h", "\\l", "\\l+", "\\dn", "\\dn+", "\\dt", "\\dt+",
    "\\dv", "\\dv+", "\\dm", "\\dm+", "\\di", "\\di+", "\\ds", "\\ds+",
    "\\df", "\\df+", "\\sf", "\\sf+", "\\dT", "\\dT+", "\\dp", "\\z",
    "\\dx", "\\dx+", "\\d", "\\d+", "\\dex", "\\dep", "\\dsh", "\\du", "\\du+",
    "\\dg", "\\dg+", "\\x", "\\timing", "\\conninfo", "\\c", "\\e", "\\i",
    "\\pset", "\\set", "\\echo", "\\copy", "\\edit", "\\cd", "\\o", "\\watch",
];

pub const PSET_VARS: &[&str] = &[
    "border", "format", "tuples_only", "null", "expanded", "pager", "fieldsep",
    "recordsep", "footer", "title", "tableattr",
];

pub const PSET_FORMATS: &[&str] = &[
    "aligned", "unaligned", "csv", "tsv", "json", "markdown", "html", "latex",
];

impl SqlCompleter {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(CompletionCache::default())),
        }
    }

    pub fn update_cache(
        &self,
        tables: Vec<String>,
        schemas: Vec<String>,
        views: Vec<String>,
        databases: Vec<String>,
        columns: Vec<String>,
    ) {
        if let Ok(mut c) = self.cache.write() {
            c.tables = tables;
            c.tables.sort();
            c.tables.dedup();

            c.schemas = schemas;
            c.schemas.sort();
            c.schemas.dedup();

            c.views = views;
            c.views.sort();
            c.views.dedup();

            c.databases = databases;
            c.databases.sort();
            c.databases.dedup();

            c.columns = columns;
            c.columns.sort();
            c.columns.dedup();
        }
    }

    pub fn add_objects(&self, objects: Vec<String>) {
        if let Ok(mut lock) = self.cache.write() {
            lock.tables.extend(objects);
            lock.tables.sort();
            lock.tables.dedup();
        }
    }

    pub fn complete_input(&self, line: &str, pos: usize) -> (usize, Vec<Pair>) {
        let safe_pos = if pos <= line.len() {
            if line.is_char_boundary(pos) {
                pos
            } else {
                (0..=pos).rev().find(|&p| line.is_char_boundary(p)).unwrap_or(0)
            }
        } else {
            line.len()
        };
        let prefix = &line[..safe_pos];

        let trimmed_prefix = prefix.trim_start();

        // 1. Meta-command completion
        if trimmed_prefix.starts_with('\\') {
            return self.complete_meta_command(prefix);
        }

        // 2. SQL query completion
        self.complete_sql(prefix)
    }

    fn complete_meta_command(&self, prefix: &str) -> (usize, Vec<Pair>) {
        let backslash_pos = prefix.rfind('\\').unwrap_or(0);
        let after_slash = &prefix[backslash_pos..];

        if let Some((cmd, arg)) = after_slash.split_once(char::is_whitespace) {
            let arg_trimmed = arg.trim_start();
            let arg_start = prefix.len() - arg.len() + (arg.len() - arg_trimmed.len());

            match cmd {
                "\\c" | "\\connect" => {
                    let dbs = self.cache.read().map(|c| c.databases.clone()).unwrap_or_default();
                    (arg_start, filter_candidates(&dbs, arg_trimmed, true))
                }
                "\\dn" | "\\dn+" => {
                    let schemas = self.cache.read().map(|c| c.schemas.clone()).unwrap_or_default();
                    (arg_start, filter_candidates(&schemas, arg_trimmed, true))
                }
                "\\d" | "\\d+" | "\\dt" | "\\dt+" | "\\dv" | "\\dv+" | "\\dm" | "\\dm+" | "\\di" | "\\di+" | "\\ds" | "\\ds+" | "\\dex" | "\\dep" | "\\dp" | "\\z" => {
                    let mut candidates = Vec::new();
                    if let Ok(c) = self.cache.read() {
                        candidates.extend(c.tables.clone());
                        candidates.extend(c.views.clone());
                        candidates.extend(c.schemas.clone());
                    }
                    candidates.sort();
                    candidates.dedup();
                    (arg_start, filter_candidates(&candidates, arg_trimmed, true))
                }
                "\\df" | "\\df+" | "\\sf" | "\\sf+" => {
                    let mut candidates = Vec::new();
                    if let Ok(c) = self.cache.read() {
                        candidates.extend(c.schemas.clone());
                    }
                    candidates.sort();
                    candidates.dedup();
                    (arg_start, filter_candidates(&candidates, arg_trimmed, true))
                }
                "\\pset" => {
                    let parts: Vec<&str> = arg.split_whitespace().collect();
                    if parts.len() <= 1 && !arg.ends_with(' ') {
                        let pset_vars: Vec<String> = PSET_VARS.iter().map(|s| s.to_string()).collect();
                        (arg_start, filter_candidates(&pset_vars, arg_trimmed, true))
                    } else {
                        let var = parts[0];
                        let val_prefix = if arg.ends_with(' ') {
                            ""
                        } else {
                            parts.get(1).copied().unwrap_or("")
                        };
                        let val_start = prefix.len() - val_prefix.len();
                        match var {
                            "format" => {
                                let formats: Vec<String> = PSET_FORMATS.iter().map(|s| s.to_string()).collect();
                                (val_start, filter_candidates(&formats, val_prefix, true))
                            }
                            "border" => {
                                let borders = vec!["0".to_string(), "1".to_string(), "2".to_string()];
                                (val_start, filter_candidates(&borders, val_prefix, true))
                            }
                            "tuples_only" | "expanded" | "pager" => {
                                let toggles = vec!["on".to_string(), "off".to_string()];
                                (val_start, filter_candidates(&toggles, val_prefix, true))
                            }
                            _ => (val_start, Vec::new()),
                        }
                    }
                }
                _ => (prefix.len(), Vec::new()),
            }
        } else {
            // Completing the meta command itself (e.g. \d, \p, \q)
            let mut matches: Vec<Pair> = META_COMMANDS
                .iter()
                .filter(|cmd| cmd.starts_with(after_slash))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: format!("{} ", cmd),
                })
                .collect();
            matches.sort_by(|a, b| a.display.cmp(&b.display));
            (backslash_pos, matches)
        }
    }

    fn complete_sql(&self, prefix: &str) -> (usize, Vec<Pair>) {
        // Find boundary of the current token being typed
        let word_start = prefix
            .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
            .map(|idx| idx + 1)
            .unwrap_or(0);

        let word = &prefix[word_start..];

        // Case A: Dot-qualified object completion (e.g. "public.u", "schema.")
        if let Some((schema, _obj_prefix)) = word.split_once('.') {
            let mut candidates = Vec::new();
            if let Ok(c) = self.cache.read() {
                for t in &c.tables {
                    if let Some((s, name)) = t.split_once('.') {
                        if s.eq_ignore_ascii_case(schema) {
                            candidates.push(format!("{}.{}", s, name));
                        }
                    }
                }
                for v in &c.views {
                    if let Some((s, name)) = v.split_once('.') {
                        if s.eq_ignore_ascii_case(schema) {
                            candidates.push(format!("{}.{}", s, name));
                        }
                    }
                }
            }
            candidates.sort();
            candidates.dedup();
            return (word_start, filter_candidates(&candidates, word, true));
        }

        // Case B: Word is empty (user hit TAB after a space)
        if word.is_empty() {
            let before_space = prefix.trim_end();
            let prev_keyword = before_space
                .split_whitespace()
                .last()
                .unwrap_or("")
                .to_uppercase();

            let cache = self.cache.read().ok();

            match prev_keyword.as_str() {
                "FROM" | "JOIN" | "INTO" | "UPDATE" | "TABLE" => {
                    let mut candidates = Vec::new();
                    if let Some(ref c) = cache {
                        candidates.extend(c.tables.clone());
                        candidates.extend(c.schemas.clone());
                    }
                    candidates.sort();
                    candidates.dedup();
                    return (prefix.len(), filter_candidates(&candidates, "", true));
                }
                "SELECT" => {
                    let mut candidates = vec![
                        "*".to_string(),
                        "DISTINCT".to_string(),
                        "COUNT(*)".to_string(),
                    ];
                    if let Some(ref c) = cache {
                        candidates.extend(c.columns.clone());
                    }
                    return (prefix.len(), filter_candidates(&candidates, "", true));
                }
                "WHERE" | "AND" | "OR" | "ON" => {
                    let mut candidates = vec!["NOT".to_string(), "EXISTS".to_string()];
                    if let Some(ref c) = cache {
                        candidates.extend(c.columns.clone());
                    }
                    return (prefix.len(), filter_candidates(&candidates, "", true));
                }
                "ORDER" | "GROUP" => {
                    return (
                        prefix.len(),
                        vec![Pair {
                            display: "BY".to_string(),
                            replacement: "BY ".to_string(),
                        }],
                    );
                }
                _ => {
                    // If at the beginning of input, suggest primary statements
                    if before_space.is_empty() {
                        let starters = [
                            "SELECT",
                            "INSERT INTO",
                            "UPDATE",
                            "DELETE FROM",
                            "CREATE TABLE",
                            "DROP TABLE",
                            "ALTER TABLE",
                            "SHOW",
                            "BEGIN",
                            "VACUUM",
                            "EXPLAIN",
                        ];
                        let pairs = starters
                            .iter()
                            .map(|s| Pair {
                                display: s.to_string(),
                                replacement: format!("{} ", s),
                            })
                            .collect();
                        return (prefix.len(), pairs);
                    }
                    return (prefix.len(), Vec::new());
                }
            }
        }

        // Case C: Normal word completion (user is typing a prefix like "sel", "pub", "use")
        let word_upper = word.to_uppercase();
        let word_lower = word.to_lowercase();
        let mut matches = Vec::new();

        // 1. SQL keywords
        for &kw in SQL_KEYWORDS {
            if kw.starts_with(&word_upper) {
                matches.push(Pair {
                    display: kw.to_string(),
                    replacement: format!("{} ", kw),
                });
            }
        }

        // 2. Database objects (tables, views, schemas, columns)
        if let Ok(c) = self.cache.read() {
            for tbl in &c.tables {
                let name_only = tbl.split('.').next_back().unwrap_or(tbl);
                if name_only.to_lowercase().starts_with(&word_lower)
                    || tbl.to_lowercase().starts_with(&word_lower)
                {
                    matches.push(Pair {
                        display: tbl.clone(),
                        replacement: format!("{} ", tbl),
                    });
                }
            }
            for view in &c.views {
                let name_only = view.split('.').next_back().unwrap_or(view);
                if name_only.to_lowercase().starts_with(&word_lower)
                    || view.to_lowercase().starts_with(&word_lower)
                {
                    matches.push(Pair {
                        display: view.clone(),
                        replacement: format!("{} ", view),
                    });
                }
            }
            for sch in &c.schemas {
                if sch.to_lowercase().starts_with(&word_lower) {
                    matches.push(Pair {
                        display: format!("{}.", sch),
                        replacement: format!("{}.", sch),
                    });
                }
            }
            for col in &c.columns {
                if col.to_lowercase().starts_with(&word_lower) {
                    matches.push(Pair {
                        display: col.clone(),
                        replacement: format!("{} ", col),
                    });
                }
            }
        }

        matches.sort_by(|a, b| a.display.cmp(&b.display));
        matches.dedup_by(|a, b| a.display == b.display);

        (word_start, matches)
    }
}

fn filter_candidates(list: &[String], prefix: &str, append_space: bool) -> Vec<Pair> {
    let lower_prefix = prefix.to_lowercase();
    list.iter()
        .filter(|item| item.to_lowercase().starts_with(&lower_prefix))
        .map(|item| {
            let replacement = if append_space {
                format!("{} ", item)
            } else {
                item.clone()
            };
            Pair {
                display: item.clone(),
                replacement,
            }
        })
        .collect()
}

impl Completer for SqlCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        Ok(self.complete_input(line, pos))
    }
}
