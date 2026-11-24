use rustsql::dialect::postgres::PostgresDialect;
use rustsql::dialect::redshift::RedshiftDialect;
use rustsql::dialect::Dialect;

#[test]
fn test_redshift_dialect_queries() {
    let dialect = RedshiftDialect;
    assert!(dialect.has_external_support());

    // Databases query uses svv_redshift_databases
    let db_sql = dialect.list_databases_query();
    assert!(db_sql.contains("svv_redshift_databases"));

    // Schemas query uses svv_all_schemas
    let schema_sql = dialect.list_schemas_query(None);
    assert!(schema_sql.contains("svv_all_schemas"));
    assert!(schema_sql.contains("schema_type"));

    // Schemas query with pattern
    let schema_pat_sql = dialect.list_schemas_query(Some("spectrum*"));
    assert!(schema_pat_sql.contains("ILIKE 'spectrum%'"));

    // Tables query uses svv_all_tables
    let tbl_sql = dialect.list_tables_query(None);
    assert!(tbl_sql.contains("svv_all_tables"));
    assert!(tbl_sql.contains("EXTERNAL TABLE"));

    // Functions query uses pg_proc_info
    let fn_sql = dialect.list_functions_query(None, false);
    assert!(fn_sql.contains("pg_proc_info"));
    assert!(fn_sql.contains("proargtypes"));

    // Functions verbose query includes source code & owner
    let fn_verbose_sql = dialect.list_functions_query(Some("maindata.p_*"), true);
    assert!(fn_verbose_sql.contains("Source code"));
    assert!(fn_verbose_sql.contains("Owner"));
    assert!(fn_verbose_sql.contains("ILIKE 'maindata'"));
    assert!(fn_verbose_sql.contains("ILIKE 'p_%'"));

    // Redshift fallback functions query uses pg_proc
    let fb_sql = rustsql::dialect::redshift::fallback_redshift_functions_query(None, false);
    assert!(fb_sql.contains("pg_catalog.pg_proc"));

    // Show function query
    let sf_sql = dialect.show_function_query("maindata.my_proc");
    assert!(sf_sql.contains("SELECT p.prosrc"));
    assert!(sf_sql.contains("ILIKE 'maindata'"));
    assert!(sf_sql.contains("ILIKE 'my_proc'"));

    // Materialized views query
    let mv_sql = dialect.list_materialized_views_query(None);
    assert!(mv_sql.contains("MATERIALIZED VIEW"));

    // Indexes query
    let idx_sql = dialect.list_indexes_query(None, false);
    assert!(idx_sql.contains("c.relkind = 'i'"));

    // Types query
    let type_sql = dialect.list_types_query(None, false);
    assert!(type_sql.contains("pg_catalog.pg_type"));

    // External tables query uses svv_external_tables
    let ext_tbl_sql = dialect.list_external_tables_query(None).unwrap();
    assert!(ext_tbl_sql.contains("svv_external_tables"));
    assert!(ext_tbl_sql.contains("input_format"));

    // External columns describe
    let ext_desc = dialect.describe_external_table_query(Some("ext_schema"), "events").unwrap();
    assert!(ext_desc.contains("svv_external_columns"));
    assert!(ext_desc.contains("is_partition_key"));

    // External partitions query
    let part_sql = dialect.list_external_partitions_query(Some("ext_schema"), "events").unwrap();
    assert!(part_sql.contains("svv_external_partitions"));

    // Local table describe uses pg_attribute with Redshift distkey, sortkey & encoding
    let local_desc = dialect.describe_table_query(Some("public"), "users");
    assert!(local_desc.contains("pg_attribute"));
    assert!(local_desc.contains("DistKey"));
    assert!(local_desc.contains("SortKey"));
    assert!(local_desc.contains("Encoding"));

    // Data shares query
    let ds_sql = dialect.list_datashares_query().unwrap();
    assert!(ds_sql.contains("svv_datashares"));
}

#[test]
fn test_postgres_dialects_queries() {
    let modern = PostgresDialect::new(140000, true);
    let legacy = PostgresDialect::new(90600, false);

    assert!(!modern.has_external_support());
    assert!(!legacy.has_external_support());

    // Modern PG includes partitioned tables 'p'
    let mod_tbl = modern.list_tables_query(None);
    assert!(mod_tbl.contains("'p' THEN 'partitioned table'"));

    // Legacy PG excludes partitioned tables from relkind
    let leg_tbl = legacy.list_tables_query(None);
    assert!(!leg_tbl.contains("'p' THEN 'partitioned table'"));
    assert!(leg_tbl.contains("c.relkind IN ('r', 'f')"));

    // Modern PG functions query uses pg_get_function_result and prokind
    let mod_fn = modern.list_functions_query(None, false);
    assert!(mod_fn.contains("pg_catalog.pg_get_function_result"));
    assert!(mod_fn.contains("p.prokind"));

    // Modern PG functions verbose query includes description & volatility
    let mod_fn_v = modern.list_functions_query(Some("users*"), true);
    assert!(mod_fn_v.contains("Volatility"));
    assert!(mod_fn_v.contains("Description"));
    assert!(mod_fn_v.contains("ILIKE 'users%'"));

    // Legacy PG functions query uses oidvectortypes
    let leg_fn = legacy.list_functions_query(None, false);
    assert!(leg_fn.contains("oidvectortypes"));
    assert!(leg_fn.contains("proisagg"));

    // Materialized views query
    let mv_sql = modern.list_materialized_views_query(None);
    assert!(mv_sql.contains("c.relkind = 'm'"));

    // Indexes query
    let idx_sql = modern.list_indexes_query(None, true);
    assert!(idx_sql.contains("c.relkind = 'i'"));
    assert!(idx_sql.contains("pg_catalog.pg_get_indexdef"));

    // Sequences query
    let seq_sql = modern.list_sequences_query(None);
    assert!(seq_sql.contains("c.relkind = 'S'"));

    // Types query
    let type_sql = modern.list_types_query(None, true);
    assert!(type_sql.contains("pg_catalog.pg_type"));
    assert!(type_sql.contains("typowner"));

    // Privileges query
    let priv_sql = modern.list_privileges_query(None);
    assert!(priv_sql.contains("relacl"));

    // Extensions query
    let ext_sql = modern.list_extensions_query().unwrap();
    assert!(ext_sql.contains("pg_catalog.pg_extension"));

    // Show function query
    let sf_sql = modern.show_function_query("my_func");
    assert!(sf_sql.contains("SELECT p.prosrc"));
    assert!(sf_sql.contains("ILIKE 'my_func'"));

    // Roles query
    let mod_users = modern.list_users_query();
    assert!(mod_users.contains("pg_catalog.pg_roles"));

    let leg_users = legacy.list_users_query();
    assert!(leg_users.contains("pg_catalog.pg_roles"));

    let ancient = PostgresDialect::new(80000, false);
    let ancient_users = ancient.list_users_query();
    assert!(ancient_users.contains("pg_user"));
}
