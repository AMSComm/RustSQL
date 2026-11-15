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

        if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            if pat.contains('.') {
                let parts: Vec<&str> = pat.splitn(2, '.').collect();
                format!(
                    "{} AND n.nspname ILIKE '{}' AND c.relname ILIKE '{}' ORDER BY 1, 2;",
                    base, parts[0], parts[1]
                )
            } else {
                format!("{} AND c.relname ILIKE '{}' ORDER BY 1, 2;", base, pat)
            }
        } else {
            format!("{} ORDER BY 1, 2;", base)
        }
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

        if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            format!("{} AND c.relname ILIKE '{}' ORDER BY 1, 2;", base, pat)
        } else {
            format!("{} ORDER BY 1, 2;", base)
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
            format!("AND n.nspname = '{}'", s)
        } else {
            "AND pg_catalog.pg_table_is_visible(c.oid)".to_string()
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
             WHERE c.relname = '{}' {} \
               AND a.attnum > 0 AND NOT a.attisdropped \
             ORDER BY a.attnum;",
            table, schema_filter
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
