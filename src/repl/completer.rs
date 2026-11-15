use rustyline::completion::{Completer, Pair};
use rustyline::Context;
use std::sync::{Arc, RwLock};

#[derive(Clone, Default)]
pub struct SqlCompleter {
    pub cached_objects: Arc<RwLock<Vec<String>>>,
}

const SQL_KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "JOIN", "LEFT JOIN", "RIGHT JOIN", "FULL JOIN",
    "INNER JOIN", "CROSS JOIN", "ON", "GROUP BY", "ORDER BY", "HAVING", "LIMIT",
    "OFFSET", "INSERT INTO", "VALUES", "UPDATE", "SET", "DELETE FROM",
    "CREATE TABLE", "CREATE EXTERNAL TABLE", "CREATE SCHEMA", "CREATE EXTERNAL SCHEMA",
    "CREATE DATABASE", "DROP TABLE", "DROP SCHEMA", "ALTER TABLE", "TRUNCATE",
    "UNION", "UNION ALL", "INTERSECT", "EXCEPT", "DISTINCT", "AS", "CASE", "WHEN",
    "THEN", "ELSE", "END", "AND", "OR", "NOT", "IN", "IS NULL", "IS NOT NULL",
    "LIKE", "ILIKE", "BETWEEN", "EXISTS", "WITH", "VACUUM", "ANALYZE",
    "DATA CATALOG", "IAM_ROLE", "REGION", "DATA SHARE", "MATERIALIZED VIEW",
];

const META_COMMANDS: &[&str] = &[
    "\\q", "\\?", "\\h", "\\l", "\\dn", "\\dt", "\\dv", "\\d", "\\dex", "\\dep",
    "\\ds", "\\du", "\\x", "\\timing", "\\conninfo", "\\c", "\\e", "\\i", "\\pset",
    "\\set", "\\echo",
];

impl SqlCompleter {
    pub fn new() -> Self {
        Self {
            cached_objects: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_objects(&self, objects: Vec<String>) {
        if let Ok(mut lock) = self.cached_objects.write() {
            lock.extend(objects);
            lock.sort();
            lock.dedup();
        }
    }
}

impl Completer for SqlCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let prefix = &line[..pos];

        // Meta-command completion
        if prefix.starts_with('\\') {
            let mut matches = Vec::new();
            for &cmd in META_COMMANDS {
                if cmd.starts_with(prefix) {
                    matches.push(Pair {
                        display: cmd.to_string(),
                        replacement: cmd.to_string(),
                    });
                }
            }
            return Ok((0, matches));
        }

        // Word-based completion
        let last_word_start = prefix
            .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '.')
            .map(|idx| idx + 1)
            .unwrap_or(0);

        let word = &prefix[last_word_start..];
        if word.is_empty() {
            return Ok((pos, Vec::new()));
        }

        let word_upper = word.to_uppercase();
        let word_lower = word.to_lowercase();
        let mut candidates = Vec::new();

        // Check SQL keywords
        for &kw in SQL_KEYWORDS {
            if kw.starts_with(&word_upper) {
                candidates.push(Pair {
                    display: kw.to_string(),
                    replacement: kw.to_string(),
                });
            }
        }

        // Check cached database objects (tables, schemas, external tables, columns)
        if let Ok(objs) = self.cached_objects.read() {
            for obj in objs.iter() {
                if obj.to_lowercase().starts_with(&word_lower) {
                    candidates.push(Pair {
                        display: obj.clone(),
                        replacement: obj.clone(),
                    });
                }
            }
        }

        Ok((last_word_start, candidates))
    }
}
