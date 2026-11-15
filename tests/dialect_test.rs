use rsql::dialect::postgres::PostgresDialect;
use rsql::dialect::redshift::RedshiftDialect;
use rsql::dialect::Dialect;

#[test]
fn test_redshift_dialect_queries() {
    let dialect = RedshiftDialect;
    assert_eq!(dialect.has_external_support(), true);

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

    // External tables query uses svv_external_tables
    let ext_tbl_sql = dialect.list_external_tables_query(None).unwrap();
    assert!(ext_tbl_sql.contains("svv_external_tables"));
    assert!(ext_tbl_sql.contains("input_format"));

    // External columns describe
    let ext_desc = dialect.describe_external_table_query(Some("ext_schema"), "events").unwrap();
    assert!(ext_desc.contains("svv_external_columns"));
    assert!(ext_desc.contains("part_key"));

    // External partitions query
    let part_sql = dialect.list_external_partitions_query(Some("ext_schema"), "events").unwrap();
    assert!(part_sql.contains("svv_external_partitions"));

    // Local table describe uses pg_table_def with Redshift distkey & sortkey
    let local_desc = dialect.describe_table_query(Some("public"), "users");
    assert!(local_desc.contains("pg_table_def"));
    assert!(local_desc.contains("distkey"));
    assert!(local_desc.contains("sortkey"));
    assert!(local_desc.contains("encoding"));

    // Data shares query
    let ds_sql = dialect.list_datashares_query().unwrap();
    assert!(ds_sql.contains("svv_datashares"));
}

#[test]
fn test_postgres_dialects_queries() {
    let modern = PostgresDialect::new(140000, true);
    let legacy = PostgresDialect::new(90600, false);

    assert_eq!(modern.has_external_support(), false);
    assert_eq!(legacy.has_external_support(), false);

    // Modern PG includes partitioned tables 'p'
    let mod_tbl = modern.list_tables_query(None);
    assert!(mod_tbl.contains("'p' THEN 'partitioned table'"));

    // Legacy PG excludes partitioned tables from relkind
    let leg_tbl = legacy.list_tables_query(None);
    assert!(!leg_tbl.contains("'p' THEN 'partitioned table'"));
    assert!(leg_tbl.contains("c.relkind IN ('r', 'f')"));

    // Roles query
    let mod_users = modern.list_users_query();
    assert!(mod_users.contains("pg_catalog.pg_roles"));

    let leg_users = legacy.list_users_query();
    assert!(leg_users.contains("pg_catalog.pg_roles"));

    let ancient = PostgresDialect::new(80000, false);
    let ancient_users = ancient.list_users_query();
    assert!(ancient_users.contains("pg_user"));
}
