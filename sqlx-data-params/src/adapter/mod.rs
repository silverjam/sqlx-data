/// Database-specific implementations for SQLx integration.
/// Import sqlx adapter implementations when database features are enabled
/// This ensures the Encode and Type traits are implemented for FilterValue
///
///
/// The module contains adapter code that implements sqlx traits for
/// [`FilterValue`] to enable direct usage in sqlx queries.
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub mod sqlx;
