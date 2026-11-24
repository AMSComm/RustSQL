pub mod postgres;
pub mod redshift;

pub use postgres::PostgresDialect;
pub use redshift::RedshiftDialect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineDialect {
    Redshift,
    PostgresModern(u32),
    PostgresLegacy(u32),
}

pub trait Dialect: Send + Sync {
    fn dialect_name(&self) -> &'static str;
    fn has_external_support(&self) -> bool;

    fn list_databases_query(&self) -> &'static str;
    fn list_schemas_query(&self, pattern: Option<&str>) -> String;
    fn list_tables_query(&self, pattern: Option<&str>) -> String;
    fn list_views_query(&self, pattern: Option<&str>) -> String;
    fn list_materialized_views_query(&self, pattern: Option<&str>) -> String;
    fn list_indexes_query(&self, pattern: Option<&str>, verbose: bool) -> String;
    fn list_sequences_query(&self, pattern: Option<&str>) -> String;
    fn list_functions_query(&self, pattern: Option<&str>, verbose: bool) -> String;
    fn list_types_query(&self, pattern: Option<&str>, verbose: bool) -> String;
    fn list_privileges_query(&self, pattern: Option<&str>) -> String;
    fn list_extensions_query(&self) -> Option<&'static str>;
    fn show_function_query(&self, name: &str) -> String;
    fn list_users_query(&self) -> &'static str;

    fn describe_table_query(&self, schema: Option<&str>, table: &str) -> String;
    fn describe_external_table_query(&self, schema: Option<&str>, table: &str) -> Option<String>;
    fn list_external_tables_query(&self, pattern: Option<&str>) -> Option<String>;
    fn list_external_partitions_query(&self, schema: Option<&str>, table: &str) -> Option<String>;
    fn list_datashares_query(&self) -> Option<&'static str>;
}

pub fn create_dialect(engine: &EngineDialect) -> Box<dyn Dialect> {
    match engine {
        EngineDialect::Redshift => Box::new(RedshiftDialect),
        EngineDialect::PostgresModern(v) => Box::new(PostgresDialect::new(*v, true)),
        EngineDialect::PostgresLegacy(v) => Box::new(PostgresDialect::new(*v, false)),
    }
}
