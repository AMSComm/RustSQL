use rustyline::completion::{Completer, Pair};
use rustyline::Context;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone, Default, Debug)]
pub struct CompletionCache {
    pub tables: Vec<String>,
    pub schemas: Vec<String>,
    pub views: Vec<String>,
    pub functions: Vec<String>,
    pub databases: Vec<String>,
    pub columns: Vec<String>,
    pub table_columns: HashMap<String, Vec<String>>,
    pub loaded_schemas: HashSet<String>,
    pub loaded_tables: HashSet<String>,
    pub pending_schemas: HashSet<String>,
    pub pending_tables: HashSet<String>,
    pub catalog_access_denied: bool,
}

#[derive(Clone, Debug)]
pub enum LoadRequest {
    Schema(String),
    Table(String),
}

#[derive(Clone)]
pub struct SchemaLoader {
    pub tx: tokio::sync::mpsc::UnboundedSender<LoadRequest>,
    pub notify: Arc<(Mutex<()>, Condvar)>,
}

#[derive(Clone, Default)]
pub struct SqlCompleter {
    pub cache: Arc<RwLock<CompletionCache>>,
    pub loader: Option<SchemaLoader>,
    pub query_prefix: Arc<RwLock<String>>,
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
            loader: None,
            query_prefix: Arc::new(RwLock::new(String::new())),
        }
    }

    pub fn with_loader(loader: SchemaLoader) -> Self {
        Self {
            cache: Arc::new(RwLock::new(CompletionCache::default())),
            loader: Some(loader),
            query_prefix: Arc::new(RwLock::new(String::new())),
        }
    }

    pub fn set_query_prefix(&self, prefix: &str) {
        if let Ok(mut qp) = self.query_prefix.write() {
            *qp = prefix.to_string();
        }
    }

    pub fn reset_cache(&self) {
        if let Ok(mut c) = self.cache.write() {
            *c = CompletionCache::default();
        }
    }

    pub fn mark_catalog_access_denied(&self) {
        if let Ok(mut c) = self.cache.write() {
            c.catalog_access_denied = true;
        }
    }

    pub fn set_functions(&self, functions: Vec<String>) {
        if let Ok(mut c) = self.cache.write() {
            c.functions = functions;
            c.functions.sort();
            c.functions.dedup();
        }
    }

    pub fn merge_schema_objects(
        &self,
        schema: &str,
        tables: Vec<String>,
        views: Vec<String>,
        functions: Vec<String>,
    ) {
        if let Ok(mut c) = self.cache.write() {
            c.tables.extend(tables);
            c.tables.sort();
            c.tables.dedup();

            c.views.extend(views);
            c.views.sort();
            c.views.dedup();

            c.functions.extend(functions);
            c.functions.sort();
            c.functions.dedup();

            c.loaded_schemas.insert(schema.to_lowercase());
            c.pending_schemas.remove(&schema.to_lowercase());
        }
    }

    pub fn mark_schema_loaded(&self, schema: &str) {
        if let Ok(mut c) = self.cache.write() {
            let key = schema.to_lowercase();
            c.loaded_schemas.insert(key.clone());
            c.pending_schemas.remove(&key);
        }
    }

    pub fn set_table_columns(&self, table: &str, columns: Vec<String>) {
        if let Ok(mut c) = self.cache.write() {
            let key = table.to_lowercase();
            let mut cols = columns;
            cols.sort();
            cols.dedup();
            if let Some((_, short_name)) = key.split_once('.') {
                c.table_columns
                    .entry(short_name.to_string())
                    .or_insert_with(|| cols.clone());
            }
            c.table_columns.insert(key.clone(), cols);
            c.loaded_tables.insert(key.clone());
            c.pending_tables.remove(&key);
        }
    }

    pub fn mark_table_loaded(&self, table: &str) {
        if let Ok(mut c) = self.cache.write() {
            let key = table.to_lowercase();
            c.loaded_tables.insert(key.clone());
            c.pending_tables.remove(&key);
        }
    }

    /// Non-blocking prefetch for schemas (`schema.`) and completed tables (`FROM tbl `, `JOIN tbl `)
    /// triggered on every keystroke or paste.
    pub fn prefetch_for_input(&self, line: &str) {
        let Some(ref loader) = self.loader else {
            return;
        };

        let full_input = {
            let qp = self.query_prefix.read().map(|q| q.clone()).unwrap_or_default();
            if qp.is_empty() {
                line.to_string()
            } else {
                format!("{} {}", qp, line)
            }
        };

        // 1. Prefetch any `schema.` qualifier typed or pasted outside quotes
        for qualifier in extract_dot_qualifiers(&full_input) {
            let q_lower = qualifier.to_lowercase();
            let should_fetch = self
                .cache
                .write()
                .map(|mut c| {
                    if c.catalog_access_denied
                        || c.loaded_schemas.contains(&q_lower)
                        || c.pending_schemas.contains(&q_lower)
                    {
                        return false;
                    }
                    let is_known_schema = c.schemas.is_empty()
                        || c.schemas.iter().any(|s| s.eq_ignore_ascii_case(&qualifier));
                    if is_known_schema {
                        c.pending_schemas.insert(q_lower);
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
            if should_fetch {
                let _ = loader.tx.send(LoadRequest::Schema(qualifier));
            }
        }

        // 2. Prefetch columns for completed table references in FROM / JOIN / UPDATE / INTO
        for tbl in extract_completed_table_refs(&full_input, &self.cache) {
            let t_lower = tbl.to_lowercase();
            let should_fetch = self
                .cache
                .write()
                .map(|mut c| {
                    if c.catalog_access_denied
                        || c.loaded_tables.contains(&t_lower)
                        || c.pending_tables.contains(&t_lower)
                    {
                        return false;
                    }
                    c.pending_tables.insert(t_lower);
                    true
                })
                .unwrap_or(false);
            if should_fetch {
                let _ = loader.tx.send(LoadRequest::Table(tbl));
            }
        }
    }

    fn wait_for_load<F>(&self, req: LoadRequest, is_done: F)
    where
        F: Fn(&CompletionCache) -> bool,
    {
        if let Some(ref loader) = self.loader {
            let _ = loader.tx.send(req);
            let (lock, cvar) = &*loader.notify;
            if let Ok(mut guard) = lock.lock() {
                let deadline = Instant::now() + Duration::from_millis(250);
                loop {
                    let done = self
                        .cache
                        .read()
                        .map(|c| c.catalog_access_denied || is_done(&c))
                        .unwrap_or(true);
                    if done {
                        break;
                    }
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    let (new_guard, wait_res) = cvar.wait_timeout(guard, remaining).unwrap();
                    guard = new_guard;
                    if wait_res.timed_out() {
                        break;
                    }
                }
            }
        }
    }

    fn ensure_schema_loaded(&self, schema: &str) {
        let schema_lower = schema.trim().to_lowercase();
        if schema_lower.is_empty() {
            return;
        }

        let should_load = self
            .cache
            .read()
            .map(|c| !c.catalog_access_denied && !c.loaded_schemas.contains(&schema_lower))
            .unwrap_or(false);

        if should_load {
            self.wait_for_load(LoadRequest::Schema(schema.to_string()), |c| {
                c.loaded_schemas.contains(&schema_lower)
            });
        }
    }

    fn ensure_table_loaded(&self, table: &str) {
        let table_lower = table.trim().to_lowercase();
        if table_lower.is_empty() {
            return;
        }

        let should_load = self
            .cache
            .read()
            .map(|c| !c.catalog_access_denied && !c.loaded_tables.contains(&table_lower))
            .unwrap_or(false);

        if should_load {
            self.wait_for_load(LoadRequest::Table(table.to_string()), |c| {
                c.loaded_tables.contains(&table_lower)
            });
        }
    }

    fn ensure_tables_loaded(&self, refs: &[(String, Option<String>)]) {
        for (table, _) in refs {
            self.ensure_table_loaded(table);
        }
    }

    fn columns_for_tables(&self, refs: &[(String, Option<String>)]) -> Vec<String> {
        let mut cols = Vec::new();
        if let Ok(c) = self.cache.read() {
            for (table, _) in refs {
                let key = table.to_lowercase();
                if let Some(tcols) = c.table_columns.get(&key) {
                    cols.extend(tcols.clone());
                } else if let Some((_, short_name)) = key.split_once('.') {
                    if let Some(tcols) = c.table_columns.get(short_name) {
                        cols.extend(tcols.clone());
                    }
                }
            }
            if cols.is_empty() {
                cols.extend(c.columns.clone());
            } else {
                cols.extend(c.columns.clone());
            }
        }
        cols.sort();
        cols.dedup();
        cols
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
                    if let Some((schema, _)) = arg_trimmed.split_once('.') {
                        self.ensure_schema_loaded(schema);
                    }
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
                    if let Some((schema, _)) = arg_trimmed.split_once('.') {
                        self.ensure_schema_loaded(schema);
                    }
                    let mut candidates = Vec::new();
                    if let Ok(c) = self.cache.read() {
                        candidates.extend(c.functions.clone());
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
        let full_sql_prefix = {
            let qp = self.query_prefix.read().map(|q| q.clone()).unwrap_or_default();
            if qp.is_empty() {
                prefix.to_string()
            } else {
                format!("{} {}", qp, prefix)
            }
        };
        let table_refs = extract_table_refs(&full_sql_prefix);

        // Case A: Dot-qualified completion (e.g. "u.col", "users.col", or "public.users")
        if let Some((qualifier, _obj_prefix)) = word.split_once('.') {
            // Check if qualifier matches a table alias or table name in FROM/JOIN/UPDATE
            let matched_table = table_refs.iter().find_map(|(tbl, alias)| {
                if alias.as_ref().is_some_and(|a| a.eq_ignore_ascii_case(qualifier)) {
                    return Some(tbl.clone());
                }
                let short = tbl.split('.').next_back().unwrap_or(tbl);
                if tbl.eq_ignore_ascii_case(qualifier) || short.eq_ignore_ascii_case(qualifier) {
                    return Some(tbl.clone());
                }
                None
            });

            if let Some(tbl) = matched_table {
                self.ensure_table_loaded(&tbl);
                let mut candidates = Vec::new();
                if let Ok(c) = self.cache.read() {
                    let key = tbl.to_lowercase();
                    let short_key = key.split('.').next_back().unwrap_or(&key);
                    if let Some(cols) = c.table_columns.get(&key).or_else(|| c.table_columns.get(short_key)) {
                        for col in cols {
                            candidates.push(format!("{}.{}", qualifier, col));
                        }
                    }
                }
                candidates.sort();
                candidates.dedup();
                if !candidates.is_empty() {
                    return (word_start, filter_candidates(&candidates, word, true));
                }
            }

            self.ensure_schema_loaded(qualifier);
            let mut candidates = Vec::new();
            if let Ok(c) = self.cache.read() {
                for t in &c.tables {
                    if let Some((s, name)) = t.split_once('.') {
                        if s.eq_ignore_ascii_case(qualifier) {
                            candidates.push(format!("{}.{}", s, name));
                        }
                    }
                }
                for v in &c.views {
                    if let Some((s, name)) = v.split_once('.') {
                        if s.eq_ignore_ascii_case(qualifier) {
                            candidates.push(format!("{}.{}", s, name));
                        }
                    }
                }
                for f in &c.functions {
                    if let Some((s, name)) = f.split_once('.') {
                        if s.eq_ignore_ascii_case(qualifier) {
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
            let mut words_rev = before_space.split_whitespace().rev();
            let prev_keyword = words_rev.next().unwrap_or("").to_uppercase();
            let prev_prev_keyword = words_rev.next().unwrap_or("").to_uppercase();

            match prev_keyword.as_str() {
                "FROM" | "JOIN" | "INTO" | "UPDATE" | "TABLE" => {
                    let mut candidates = Vec::new();
                    if let Ok(c) = self.cache.read() {
                        candidates.extend(c.tables.clone());
                        candidates.extend(c.views.clone());
                        candidates.extend(c.schemas.clone());
                    }
                    candidates.sort();
                    candidates.dedup();
                    return (prefix.len(), filter_candidates(&candidates, "", true));
                }
                "SELECT" => {
                    self.ensure_tables_loaded(&table_refs);
                    let mut candidates = vec![
                        "*".to_string(),
                        "DISTINCT".to_string(),
                        "COUNT(*)".to_string(),
                    ];
                    candidates.extend(self.columns_for_tables(&table_refs));
                    if let Ok(c) = self.cache.read() {
                        candidates.extend(c.functions.clone());
                    }
                    return (prefix.len(), filter_candidates(&candidates, "", true));
                }
                "CALL" | "EXECUTE" => {
                    let mut candidates = Vec::new();
                    if let Ok(c) = self.cache.read() {
                        candidates.extend(c.functions.clone());
                        candidates.extend(c.schemas.clone());
                    }
                    candidates.sort();
                    candidates.dedup();
                    return (prefix.len(), filter_candidates(&candidates, "", true));
                }
                "WHERE" | "AND" | "OR" | "ON" | "HAVING" => {
                    self.ensure_tables_loaded(&table_refs);
                    let mut candidates = vec!["NOT".to_string(), "EXISTS".to_string()];
                    candidates.extend(self.columns_for_tables(&table_refs));
                    return (prefix.len(), filter_candidates(&candidates, "", true));
                }
                "SET" => {
                    self.ensure_tables_loaded(&table_refs);
                    let candidates = self.columns_for_tables(&table_refs);
                    return (prefix.len(), filter_candidates(&candidates, "", true));
                }
                "BY" if prev_prev_keyword == "ORDER" || prev_prev_keyword == "GROUP" => {
                    self.ensure_tables_loaded(&table_refs);
                    let candidates = self.columns_for_tables(&table_refs);
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
        if !table_refs.is_empty() {
            self.ensure_tables_loaded(&table_refs);
        }
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
            for func in &c.functions {
                let name_only = func.split('.').next_back().unwrap_or(func);
                if name_only.to_lowercase().starts_with(&word_lower)
                    || func.to_lowercase().starts_with(&word_lower)
                {
                    matches.push(Pair {
                        display: func.clone(),
                        replacement: format!("{} ", func),
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
            for col in self.columns_for_tables(&table_refs) {
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

fn extract_dot_qualifiers(sql: &str) -> Vec<String> {
    let mut qualifiers = Vec::new();
    let mut in_quote = false;
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            in_quote = !in_quote;
            i += 1;
            continue;
        }
        if !in_quote && bytes[i] == b'.' && i > 0 {
            let mut start = i;
            while start > 0 {
                let b = bytes[start - 1];
                if b.is_ascii_alphanumeric() || b == b'_' {
                    start -= 1;
                } else {
                    break;
                }
            }
            if start < i {
                let q = &sql[start..i];
                if !qualifiers.iter().any(|existing: &String| existing.eq_ignore_ascii_case(q)) {
                    qualifiers.push(q.to_string());
                }
            }
        }
        i += 1;
    }
    qualifiers
}

fn extract_completed_table_refs(sql: &str, cache: &Arc<RwLock<CompletionCache>>) -> Vec<String> {
    let refs = extract_table_refs(sql);
    if refs.is_empty() {
        return Vec::new();
    }

    let ended_mid_token = sql
        .chars()
        .last()
        .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
    let last_token = sql
        .split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')' || c == ';')
        .filter(|t| !t.is_empty())
        .last()
        .unwrap_or("")
        .trim_matches('"');

    let guard = cache.read().ok();
    let mut result = Vec::new();
    for (idx, (tbl, _)) in refs.iter().enumerate() {
        let is_last = idx == refs.len() - 1;
        if is_last && ended_mid_token && tbl.eq_ignore_ascii_case(last_token) {
            // Still typing this table name: only prefetch if it's already a known table/view
            let is_known = guard.as_ref().is_some_and(|c| {
                c.tables.iter().any(|t| t.eq_ignore_ascii_case(tbl))
                    || c.views.iter().any(|v| v.eq_ignore_ascii_case(tbl))
            });
            if !is_known {
                continue;
            }
        }
        result.push(tbl.clone());
    }
    result
}

fn extract_table_refs(sql: &str) -> Vec<(String, Option<String>)> {
    const CLAUSE_KEYWORDS: &[&str] = &[
        "WHERE", "ON", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS", "NATURAL",
        "GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "UNION", "INTERSECT", "EXCEPT", "SET",
        "VALUES", "RETURNING", "USING", "AS", "INTO", "FROM", "SELECT",
    ];

    let tokens: Vec<&str> = sql
        .split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')' || c == ';')
        .filter(|t| !t.is_empty())
        .collect();

    let mut refs = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let upper = tokens[i].to_uppercase();
        if matches!(upper.as_str(), "FROM" | "JOIN" | "UPDATE" | "INTO") {
            if i + 1 < tokens.len() {
                let tbl = tokens[i + 1].trim_matches('"');
                if !tbl.is_empty() && !CLAUSE_KEYWORDS.contains(&tbl.to_uppercase().as_str()) {
                    let mut alias = None;
                    if i + 2 < tokens.len() {
                        let next_upper = tokens[i + 2].to_uppercase();
                        if next_upper == "AS" && i + 3 < tokens.len() {
                            let a = tokens[i + 3].trim_matches('"');
                            if !CLAUSE_KEYWORDS.contains(&a.to_uppercase().as_str()) {
                                alias = Some(a.to_string());
                            }
                        } else if !CLAUSE_KEYWORDS.contains(&next_upper.as_str()) {
                            alias = Some(tokens[i + 2].trim_matches('"').to_string());
                        }
                    }
                    refs.push((tbl.to_string(), alias));
                }
            }
        }
        i += 1;
    }
    refs
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
