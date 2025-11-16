pub mod database {

    #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
    pub fn get_dialect() -> sqlparser::dialect::AnsiDialect {
        sqlparser::dialect::AnsiDialect {}
    }

    // Parser-specific functions
    #[cfg(feature = "sqlite")]
    pub fn get_dialect() -> sqlparser::dialect::SQLiteDialect {
        sqlparser::dialect::SQLiteDialect {}
    }

    #[cfg(feature = "postgres")]
    pub fn get_dialect() -> sqlparser::dialect::PostgreSqlDialect {
        sqlparser::dialect::PostgreSqlDialect {}
    }

    #[cfg(feature = "mysql")]
    pub fn get_dialect() -> sqlparser::dialect::MySqlDialect {
        sqlparser::dialect::MySqlDialect {}
    }
}

pub mod cache {
    use crate::global_cache::GlobalCache;
    use sqlparser::ast::Statement;
    use std::sync::LazyLock;

    /// Cache size for parsed SQL statements.
    pub const INITIAL_CAPACITY: usize = 128;
    /// Maximum number of cached SQL statements.
    pub const MAX_CAPACITY: u64 = 10_000;
    pub type SqlKey = u64;

    /// Core SQL parser cache - always available
    pub static SQL_PARSER_CACHE: LazyLock<GlobalCache<SqlKey, Statement>> = LazyLock::new(|| {
        GlobalCache::new(INITIAL_CAPACITY, MAX_CAPACITY, None, |_key: &u64, _ast| 1)
    });

    /// Cache for count SQL queries - only available with database features
    #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
    pub static COUNT_SQL_CACHE: LazyLock<GlobalCache<SqlKey, String>> = LazyLock::new(|| {
        GlobalCache::new(
            INITIAL_CAPACITY,
            MAX_CAPACITY,
            None,
            |_key: &u64, sql: &String| sql.len() as u32,
        )
    });
}
