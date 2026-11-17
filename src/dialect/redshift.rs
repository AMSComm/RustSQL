use super::Dialect;

pub struct RedshiftDialect;

impl Dialect for RedshiftDialect {
    fn dialect_name(&self) -> &'static str {
        "Amazon Redshift (Lakehouse & Spectrum Enabled)"
    }

    fn has_external_support(&self) -> bool {
        true
    }

    fn list_databases_query(&self) -> &'static str {
        // Redshift unified database view: shows local databases and datashare databases
        "SELECT \
            database_name AS \"Name\", \
            database_owner AS \"Owner\", \
            database_type AS \"Type\", \
            database_isolation_level AS \"Isolation Level\" \
         FROM svv_redshift_databases \
         ORDER BY 1;"
    }

    fn list_schemas_query(&self, pattern: Option<&str>) -> String {
        let base = "SELECT \
            schema_name AS \"Name\", \
            schema_owner AS \"Owner\", \
            schema_type AS \"Type\", \
            source_database AS \"Source Database\" \
         FROM svv_all_schemas \
         WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_internal') \
           AND schema_name NOT LIKE 'pg_temp_%'";

        if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            format!("{} AND schema_name ILIKE '{}' ORDER BY schema_type, schema_name;", base, pat)
        } else {
            format!("{} ORDER BY schema_type, schema_name;", base)
        }
    }

    fn list_tables_query(&self, pattern: Option<&str>) -> String {
        let base = "SELECT \
            schema_name AS \"Schema\", \
            table_name AS \"Name\", \
            table_type AS \"Type\" \
         FROM svv_all_tables";

        if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            if pat == "%" || pat == "%.%" {
                // \dt * or \dt *.*: show all tables across all schemas (excluding only internal temp)
                format!("{} WHERE schema_name NOT LIKE 'pg_temp_%' AND table_type IN ('TABLE', 'EXTERNAL TABLE') ORDER BY 1, 2;", base)
            } else if pat.contains('.') {
                let parts: Vec<&str> = pat.splitn(2, '.').collect();
                format!(
                    "{} WHERE schema_name ILIKE '{}' AND table_name ILIKE '{}' AND table_type IN ('TABLE', 'EXTERNAL TABLE') ORDER BY 1, 2;",
                    base, parts[0], parts[1]
                )
            } else {
                format!(
                    "{} WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_internal') \
                       AND schema_name NOT LIKE 'pg_temp_%' \
                       AND table_type IN ('TABLE', 'EXTERNAL TABLE') \
                       AND table_name ILIKE '{}' \
                     ORDER BY 1, 2;",
                    base, pat
                )
            }
        } else {
            // \dt without pattern: exclude system schemas
            format!(
                "{} WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_internal') \
                   AND schema_name NOT LIKE 'pg_temp_%' \
                   AND table_type IN ('TABLE', 'EXTERNAL TABLE') \
                 ORDER BY 1, 2;",
                base
            )
        }
    }

    fn list_views_query(&self, pattern: Option<&str>) -> String {
        let base = "SELECT \
            schema_name AS \"Schema\", \
            table_name AS \"Name\", \
            table_type AS \"Type\" \
         FROM svv_all_tables \
         WHERE table_type IN ('VIEW', 'MATERIALIZED VIEW', 'SECURE VIEW', 'LATE BINDING VIEW')";

        if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            if pat == "%" || pat == "%.%" {
                format!("{} AND schema_name NOT LIKE 'pg_temp_%' ORDER BY 1, 2;", base)
            } else if pat.contains('.') {
                let parts: Vec<&str> = pat.splitn(2, '.').collect();
                format!(
                    "{} AND schema_name ILIKE '{}' AND table_name ILIKE '{}' ORDER BY 1, 2;",
                    base, parts[0], parts[1]
                )
            } else {
                format!(
                    "{} AND schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_internal') \
                       AND schema_name NOT LIKE 'pg_temp_%' \
                       AND table_name ILIKE '{}' \
                     ORDER BY 1, 2;",
                    base, pat
                )
            }
        } else {
            format!(
                "{} AND schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_internal') \
                   AND schema_name NOT LIKE 'pg_temp_%' \
                 ORDER BY 1, 2;",
                base
            )
        }
    }

    fn list_users_query(&self) -> &'static str {
        "SELECT \
            usename AS \"Role name\", \
            CASE WHEN usesuper THEN 'yes' ELSE 'no' END AS \"Superuser\", \
            CASE WHEN usecreatedb THEN 'yes' ELSE 'no' END AS \"Create DB\", \
            valuntil AS \"Valid until\" \
         FROM pg_user \
         ORDER BY 1;"
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
                CASE WHEN a.attencodingtype = 0 THEN 'none' \
                     WHEN a.attencodingtype = 1 THEN 'raw' \
                     WHEN a.attencodingtype = 2 THEN 'bytedict' \
                     WHEN a.attencodingtype = 3 THEN 'delta' \
                     WHEN a.attencodingtype = 4 THEN 'lzo' \
                     WHEN a.attencodingtype = 5 THEN 'runlength' \
                     WHEN a.attencodingtype = 6 THEN 'text255' \
                     WHEN a.attencodingtype = 7 THEN 'text32k' \
                     WHEN a.attencodingtype = 8 THEN 'zstd' \
                     WHEN a.attencodingtype = 9 THEN 'az64' \
                     ELSE a.attencodingtype::text \
                END AS \"Encoding\", \
                CASE WHEN a.attisdistkey THEN 'true' ELSE 'false' END AS \"DistKey\", \
                CASE WHEN a.attsortkeyord != 0 THEN a.attsortkeyord::text ELSE '' END AS \"SortKey\", \
                CASE WHEN a.attnotnull THEN 'not null' ELSE '' END AS \"Nullable\", \
                COALESCE(pg_catalog.pg_get_expr(d.adbin, d.adrelid), '') AS \"Default\" \
             FROM pg_catalog.pg_attribute a \
             JOIN pg_catalog.pg_class c ON c.oid = a.attrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
             WHERE (c.relname = lower('{}') OR c.relname = '{}' OR c.relname ILIKE '{}') \
               {} \
               AND a.attnum > 0 AND NOT a.attisdropped \
             ORDER BY \
                CASE WHEN pg_catalog.pg_table_is_visible(c.oid) THEN 0 ELSE 1 END, \
                a.attnum;",
            table, table, table, schema_filter
        )
    }

    fn describe_external_table_query(&self, schema: Option<&str>, table: &str) -> Option<String> {
        let query = if let Some(s) = schema {
            format!(
                "SELECT \
                    columnname AS \"Column\", \
                    external_type AS \"Type\", \
                    columnnum AS \"Ordinal\", \
                    CASE WHEN part_key != 0 THEN 'YES (' || part_key || ')' ELSE 'NO' END AS \"Partition Key\" \
                 FROM svv_external_columns \
                 WHERE schemaname = '{}' AND tablename = '{}' \
                 ORDER BY columnnum;",
                s, table
            )
        } else {
            format!(
                "SELECT \
                    schemaname AS \"Schema\", \
                    columnname AS \"Column\", \
                    external_type AS \"Type\", \
                    columnnum AS \"Ordinal\", \
                    CASE WHEN part_key != 0 THEN 'YES (' || part_key || ')' ELSE 'NO' END AS \"Partition Key\" \
                 FROM svv_external_columns \
                 WHERE tablename = '{}' \
                 ORDER BY schemaname, columnnum;",
                table
            )
        };
        Some(query)
    }

    fn list_external_tables_query(&self, pattern: Option<&str>) -> Option<String> {
        let base = "SELECT \
            schemaname AS \"Schema\", \
            tablename AS \"Table\", \
            location AS \"S3 Location\", \
            input_format AS \"Input Format\", \
            serialization_lib AS \"SerDe Lib\" \
         FROM svv_external_tables";

        let q = if let Some(p) = pattern {
            let pat = p.replace('*', "%");
            format!("{} WHERE tablename ILIKE '{}' ORDER BY 1, 2;", base, pat)
        } else {
            format!("{} ORDER BY 1, 2;", base)
        };
        Some(q)
    }

    fn list_external_partitions_query(&self, schema: Option<&str>, table: &str) -> Option<String> {
        let q = if let Some(s) = schema {
            format!(
                "SELECT \
                    schemaname AS \"Schema\", \
                    tablename AS \"Table\", \
                    values AS \"Partition Values\", \
                    location AS \"Location\" \
                 FROM svv_external_partitions \
                 WHERE schemaname = '{}' AND tablename = '{}' \
                 ORDER BY 1, 2 LIMIT 100;",
                s, table
            )
        } else {
            format!(
                "SELECT \
                    schemaname AS \"Schema\", \
                    tablename AS \"Table\", \
                    values AS \"Partition Values\", \
                    location AS \"Location\" \
                 FROM svv_external_partitions \
                 WHERE tablename = '{}' \
                 ORDER BY 1, 2 LIMIT 100;",
                table
            )
        };
        Some(q)
    }

    fn list_datashares_query(&self) -> Option<&'static str> {
        Some(
            "SELECT \
                share_name AS \"Share Name\", \
                share_owner AS \"Owner\", \
                CASE WHEN is_public THEN 'true' ELSE 'false' END AS \"Public\", \
                share_type AS \"Type\" \
             FROM svv_datashares \
             ORDER BY 1;",
        )
    }
}
