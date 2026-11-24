use super::Dialect;

pub struct PostgresDialect {
    pub version_num: u32,
    pub is_modern: bool,
}

impl PostgresDialect {
    pub fn new(version_num: u32, is_modern: bool) -> Self {
        Self {
            version_num,
            is_modern,
        }
    }
}

impl Dialect for PostgresDialect {
    fn dialect_name(&self) -> &'static str {
        if self.is_modern {
            "PostgreSQL (Modern 12+)"
        } else {
            "PostgreSQL (Legacy 8.x-11)"
        }
    }

    fn has_external_support(&self) -> bool {
        false
    }

    fn list_databases_query(&self) -> &'static str {
        "SELECT \
            d.datname AS \"Name\", \
            pg_catalog.pg_get_userbyid(d.datdba) AS \"Owner\", \
            pg_catalog.pg_encoding_to_char(d.encoding) AS \"Encoding\", \
            d.datcollate AS \"Collate\", \
            d.datctype AS \"Ctype\" \
         FROM pg_catalog.pg_database d \
         ORDER BY 1;"
    }

    fn list_schemas_query(&self, pattern: Option<&str>) -> String {
        let base = "SELECT \
            n.nspname AS \"Name\", \
            pg_catalog.pg_get_userbyid(n.nspowner) AS \"Owner\" \
         FROM pg_catalog.pg_namespace n \
         WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'";

        if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            format!("{} AND n.nspname ILIKE '{}' ORDER BY 1;", base, pat)
        } else {
            format!("{} ORDER BY 1;", base)
        }
    }

    fn list_tables_query(&self, pattern: Option<&str>) -> String {
        let base = if self.is_modern {
            "SELECT \
                n.nspname AS \"Schema\", \
                c.relname AS \"Name\", \
                CASE c.relkind \
                    WHEN 'r' THEN 'table' \
                    WHEN 'p' THEN 'partitioned table' \
                    WHEN 'f' THEN 'foreign table' \
                    ELSE 'other' \
                END AS \"Type\", \
                pg_catalog.pg_get_userbyid(c.relowner) AS \"Owner\" \
             FROM pg_catalog.pg_class c \
             LEFT JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('r', 'p', 'f') \
               AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
        } else {
            // Legacy PG (no partitioned tables 'p' in relkind)
            "SELECT \
                n.nspname AS \"Schema\", \
                c.relname AS \"Name\", \
                CASE c.relkind \
                    WHEN 'r' THEN 'table' \
                    WHEN 'f' THEN 'foreign table' \
                    ELSE 'other' \
                END AS \"Type\", \
                pg_catalog.pg_get_userbyid(c.relowner) AS \"Owner\" \
             FROM pg_catalog.pg_class c \
             LEFT JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('r', 'f') \
               AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
        };

        apply_pattern(base, pattern, "n.nspname", "c.relname")
    }

    fn list_views_query(&self, pattern: Option<&str>) -> String {
        let base = "SELECT \
            n.nspname AS \"Schema\", \
            c.relname AS \"Name\", \
            CASE c.relkind \
                WHEN 'v' THEN 'view' \
                WHEN 'm' THEN 'materialized view' \
                ELSE 'other' \
            END AS \"Type\", \
            pg_catalog.pg_get_userbyid(c.relowner) AS \"Owner\" \
         FROM pg_catalog.pg_class c \
         LEFT JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind IN ('v', 'm') \
           AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'";

        apply_pattern(base, pattern, "n.nspname", "c.relname")
    }

    fn list_materialized_views_query(&self, pattern: Option<&str>) -> String {
        let base = "SELECT \
            n.nspname AS \"Schema\", \
            c.relname AS \"Name\", \
            'materialized view' AS \"Type\", \
            pg_catalog.pg_get_userbyid(c.relowner) AS \"Owner\" \
         FROM pg_catalog.pg_class c \
         LEFT JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind = 'm' \
           AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'";

        apply_pattern(base, pattern, "n.nspname", "c.relname")
    }

    fn list_indexes_query(&self, pattern: Option<&str>, verbose: bool) -> String {
        let base = if verbose {
            "SELECT \
                n.nspname AS \"Schema\", \
                c.relname AS \"Name\", \
                'index' AS \"Type\", \
                pg_catalog.pg_get_userbyid(c.relowner) AS \"Owner\", \
                c2.relname AS \"Table\", \
                pg_catalog.pg_size_pretty(pg_catalog.pg_table_size(c.oid)) AS \"Size\", \
                pg_catalog.pg_get_indexdef(i.indexrelid, 0, true) AS \"Definition\" \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_catalog.pg_index i ON i.indexrelid = c.oid \
             JOIN pg_catalog.pg_class c2 ON c2.oid = i.indrelid \
             WHERE c.relkind = 'i' \
               AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
        } else {
            "SELECT \
                n.nspname AS \"Schema\", \
                c.relname AS \"Name\", \
                'index' AS \"Type\", \
                pg_catalog.pg_get_userbyid(c.relowner) AS \"Owner\", \
                c2.relname AS \"Table\" \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_catalog.pg_index i ON i.indexrelid = c.oid \
             JOIN pg_catalog.pg_class c2 ON c2.oid = i.indrelid \
             WHERE c.relkind = 'i' \
               AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
        };

        apply_pattern(base, pattern, "n.nspname", "c.relname")
    }

    fn list_sequences_query(&self, pattern: Option<&str>) -> String {
        let base = "SELECT \
            n.nspname AS \"Schema\", \
            c.relname AS \"Name\", \
            'sequence' AS \"Type\", \
            pg_catalog.pg_get_userbyid(c.relowner) AS \"Owner\" \
         FROM pg_catalog.pg_class c \
         LEFT JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind = 'S' \
           AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'";

        apply_pattern(base, pattern, "n.nspname", "c.relname")
    }

    fn list_functions_query(&self, pattern: Option<&str>, verbose: bool) -> String {
        let base = if self.version_num >= 110000 {
            if verbose {
                "SELECT \
                    n.nspname AS \"Schema\", \
                    p.proname AS \"Name\", \
                    pg_catalog.pg_get_function_result(p.oid) AS \"Result data type\", \
                    pg_catalog.pg_get_function_arguments(p.oid) AS \"Argument data types\", \
                    CASE p.prokind \
                        WHEN 'a' THEN 'agg' \
                        WHEN 'w' THEN 'window' \
                        WHEN 'p' THEN 'proc' \
                        ELSE 'func' \
                    END AS \"Type\", \
                    CASE p.provolatile \
                        WHEN 'i' THEN 'immutable' \
                        WHEN 's' THEN 'stable' \
                        WHEN 'v' THEN 'volatile' \
                    END AS \"Volatility\", \
                    pg_catalog.pg_get_userbyid(p.proowner) AS \"Owner\", \
                    CASE WHEN p.prosecdef THEN 'definer' ELSE 'invoker' END AS \"Security\", \
                    l.lanname AS \"Language\", \
                    p.prosrc AS \"Source code\", \
                    pg_catalog.obj_description(p.oid, 'pg_proc') AS \"Description\" \
                 FROM pg_catalog.pg_proc p \
                 LEFT JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
                 LEFT JOIN pg_catalog.pg_language l ON l.oid = p.prolang \
                 WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
            } else {
                "SELECT \
                    n.nspname AS \"Schema\", \
                    p.proname AS \"Name\", \
                    pg_catalog.pg_get_function_result(p.oid) AS \"Result data type\", \
                    pg_catalog.pg_get_function_arguments(p.oid) AS \"Argument data types\", \
                    CASE p.prokind \
                        WHEN 'a' THEN 'agg' \
                        WHEN 'w' THEN 'window' \
                        WHEN 'p' THEN 'proc' \
                        ELSE 'func' \
                    END AS \"Type\" \
                 FROM pg_catalog.pg_proc p \
                 LEFT JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
            }
        } else {
            // Legacy Postgres (< 11)
            if verbose {
                "SELECT \
                    n.nspname AS \"Schema\", \
                    p.proname AS \"Name\", \
                    pg_catalog.format_type(p.prorettype, NULL) AS \"Result data type\", \
                    pg_catalog.oidvectortypes(p.proargtypes) AS \"Argument data types\", \
                    CASE WHEN p.proisagg THEN 'agg' ELSE 'func' END AS \"Type\", \
                    pg_catalog.pg_get_userbyid(p.proowner) AS \"Owner\", \
                    CASE WHEN p.prosecdef THEN 'definer' ELSE 'invoker' END AS \"Security\", \
                    l.lanname AS \"Language\", \
                    p.prosrc AS \"Source code\" \
                 FROM pg_catalog.pg_proc p \
                 LEFT JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
                 LEFT JOIN pg_catalog.pg_language l ON l.oid = p.prolang \
                 WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
            } else {
                "SELECT \
                    n.nspname AS \"Schema\", \
                    p.proname AS \"Name\", \
                    pg_catalog.format_type(p.prorettype, NULL) AS \"Result data type\", \
                    pg_catalog.oidvectortypes(p.proargtypes) AS \"Argument data types\", \
                    CASE WHEN p.proisagg THEN 'agg' ELSE 'func' END AS \"Type\" \
                 FROM pg_catalog.pg_proc p \
                 LEFT JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
            }
        };

        if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            if pat.contains('.') {
                let parts: Vec<&str> = pat.splitn(2, '.').collect();
                format!(
                    "{} AND n.nspname ILIKE '{}' AND p.proname ILIKE '{}' ORDER BY 1, 2, 4;",
                    base, parts[0], parts[1]
                )
            } else {
                format!("{} AND p.proname ILIKE '{}' ORDER BY 1, 2, 4;", base, pat)
            }
        } else {
            format!("{} ORDER BY 1, 2, 4;", base)
        }
    }

    fn list_types_query(&self, pattern: Option<&str>, verbose: bool) -> String {
        let base = if verbose {
            "SELECT \
                n.nspname AS \"Schema\", \
                pg_catalog.format_type(t.oid, NULL) AS \"Name\", \
                pg_catalog.pg_get_userbyid(t.typowner) AS \"Owner\", \
                pg_catalog.obj_description(t.oid, 'pg_type') AS \"Description\" \
             FROM pg_catalog.pg_type t \
             LEFT JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace \
             WHERE (t.typrelid = 0 OR (SELECT c.relkind = 'c' FROM pg_catalog.pg_class c WHERE c.oid = t.typrelid)) \
               AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_type el WHERE el.oid = t.typelem AND el.typarray = t.oid) \
               AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
        } else {
            "SELECT \
                n.nspname AS \"Schema\", \
                pg_catalog.format_type(t.oid, NULL) AS \"Name\", \
                pg_catalog.obj_description(t.oid, 'pg_type') AS \"Description\" \
             FROM pg_catalog.pg_type t \
             LEFT JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace \
             WHERE (t.typrelid = 0 OR (SELECT c.relkind = 'c' FROM pg_catalog.pg_class c WHERE c.oid = t.typrelid)) \
               AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_type el WHERE el.oid = t.typelem AND el.typarray = t.oid) \
               AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'"
        };

        if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            if pat.contains('.') {
                let parts: Vec<&str> = pat.splitn(2, '.').collect();
                format!(
                    "{} AND n.nspname ILIKE '{}' AND pg_catalog.format_type(t.oid, NULL) ILIKE '{}' ORDER BY 1, 2;",
                    base, parts[0], parts[1]
                )
            } else {
                format!("{} AND pg_catalog.format_type(t.oid, NULL) ILIKE '{}' ORDER BY 1, 2;", base, pat)
            }
        } else {
            format!("{} ORDER BY 1, 2;", base)
        }
    }

    fn list_privileges_query(&self, pattern: Option<&str>) -> String {
        let base = "SELECT \
            n.nspname AS \"Schema\", \
            c.relname AS \"Name\", \
            CASE c.relkind \
                WHEN 'r' THEN 'table' \
                WHEN 'v' THEN 'view' \
                WHEN 'm' THEN 'materialized view' \
                WHEN 'S' THEN 'sequence' \
                WHEN 'f' THEN 'foreign table' \
                WHEN 'p' THEN 'partitioned table' \
                ELSE 'other' \
            END AS \"Type\", \
            pg_catalog.array_to_string(c.relacl, E'\n') AS \"Access privileges\" \
         FROM pg_catalog.pg_class c \
         LEFT JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind IN ('r', 'v', 'm', 'S', 'f', 'p') \
           AND n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'";

        apply_pattern(base, pattern, "n.nspname", "c.relname")
    }

    fn list_extensions_query(&self) -> Option<&'static str> {
        Some(
            "SELECT \
                e.extname AS \"Name\", \
                e.extversion AS \"Version\", \
                n.nspname AS \"Schema\", \
                c.description AS \"Description\" \
             FROM pg_catalog.pg_extension e \
             LEFT JOIN pg_catalog.pg_namespace n ON n.oid = e.extnamespace \
             LEFT JOIN pg_catalog.pg_description c ON c.objoid = e.oid AND c.classoid = 'pg_catalog.pg_extension'::pg_catalog.regclass \
             ORDER BY 1;",
        )
    }

    fn show_function_query(&self, name: &str) -> String {
        if name.contains('.') {
            let parts: Vec<&str> = name.splitn(2, '.').collect();
            format!(
                "SELECT p.prosrc \
                 FROM pg_catalog.pg_proc p \
                 JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname ILIKE '{}' AND p.proname ILIKE '{}' \
                 LIMIT 1;",
                parts[0], parts[1]
            )
        } else {
            format!(
                "SELECT p.prosrc \
                 FROM pg_catalog.pg_proc p \
                 JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
                 WHERE p.proname ILIKE '{}' \
                   AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
                 ORDER BY (n.nspname = 'public') DESC \
                 LIMIT 1;",
                name
            )
        }
    }

    fn list_users_query(&self) -> &'static str {
        if self.version_num >= 80100 {
            "SELECT \
                r.rolname AS \"Role name\", \
                ARRAY[ \
                    CASE WHEN r.rolsuper THEN 'Superuser' ELSE NULL END, \
                    CASE WHEN r.rolcreaterole THEN 'Create role' ELSE NULL END, \
                    CASE WHEN r.rolcreatedb THEN 'Create DB' ELSE NULL END, \
                    CASE WHEN r.rolcanlogin THEN 'Cannot login' ELSE NULL END \
                ] AS \"Attributes\" \
             FROM pg_catalog.pg_roles r \
             ORDER BY 1;"
        } else {
            "SELECT \
                usename AS \"Role name\", \
                CASE WHEN usesuper THEN 'yes' ELSE 'no' END AS \"Superuser\", \
                CASE WHEN usecreatedb THEN 'yes' ELSE 'no' END AS \"Create DB\" \
             FROM pg_user \
             ORDER BY 1;"
        }
    }

    fn describe_table_query(&self, schema: Option<&str>, table: &str) -> String {
        let schema_filter = if let Some(s) = schema {
            format!("AND (n.nspname = lower('{}') OR n.nspname = '{}' OR n.nspname ILIKE '{}')", s, s, s)
        } else {
            "AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg_temp_%'".to_string()
        };

        format!(
            "SELECT \
                a.attname AS \"Column\", \
                pg_catalog.format_type(a.atttypid, a.atttypmod) AS \"Type\", \
                CASE WHEN a.attnotnull THEN 'not null' ELSE '' END AS \"Nullable\", \
                COALESCE(pg_catalog.pg_get_expr(d.adbin, d.adrelid), '') AS \"Default\" \
             FROM pg_catalog.pg_attribute a \
             JOIN pg_catalog.pg_class c ON c.oid = a.attrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
             WHERE (c.relname = lower('{}') OR c.relname = '{}' OR c.relname ILIKE '{}') {} \
               AND a.attnum > 0 AND NOT a.attisdropped \
             ORDER BY \
                CASE WHEN pg_catalog.pg_table_is_visible(c.oid) THEN 0 ELSE 1 END, \
                a.attnum;",
            table, table, table, schema_filter
        )
    }

    fn describe_external_table_query(&self, _schema: Option<&str>, _table: &str) -> Option<String> {
        None
    }

    fn list_external_tables_query(&self, _pattern: Option<&str>) -> Option<String> {
        None
    }

    fn list_external_partitions_query(&self, _schema: Option<&str>, _table: &str) -> Option<String> {
        None
    }

    fn list_datashares_query(&self) -> Option<&'static str> {
        None
    }
}

fn apply_pattern(base: &str, pattern: Option<&str>, schema_col: &str, name_col: &str) -> String {
    if let Some(p) = pattern {
        let pat = p.replace('*', "%");
        if pat.contains('.') {
            let parts: Vec<&str> = pat.splitn(2, '.').collect();
            format!(
                "{} AND {} ILIKE '{}' AND {} ILIKE '{}' ORDER BY 1, 2;",
                base, schema_col, parts[0], name_col, parts[1]
            )
        } else {
            format!("{} AND {} ILIKE '{}' ORDER BY 1, 2;", base, name_col, pat)
        }
    } else {
        format!("{} ORDER BY 1, 2;", base)
    }
}
