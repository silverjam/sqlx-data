// SQL parsing, analysis and dynamic query building for sqlx-data

// Foundation modules - always available
mod constants;
mod global_cache;

// Core functionality - always available
mod core;

// Re-export core functionality
#[allow(ambiguous_glob_reexports)]
pub use core::*;

// Dynamic functionality - only when database features are enabled
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
mod dynamic;

// Re-export dynamic functionality when available
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub use dynamic::*;
