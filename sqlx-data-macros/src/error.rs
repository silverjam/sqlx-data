//! Error utilities for sqlx-data-macros
//!
//! This module provides error conversion utilities specific to the macro context,
//! keeping the core module clean and focused.

use sqlx_data_parser::ParserError;

/// Convert any error to syn::Error for proc-macro error reporting
pub fn syn_error(msg: impl std::fmt::Display) -> syn::Error {
    syn::Error::new(proc_macro2::Span::call_site(), msg.to_string())
}

/// Helper to create SQL parsing errors
pub fn sql_parse_error() -> syn::Error {
    syn_error("Failed to parse SQL")
}

/// Helper function to convert core error to syn error - used with map_err
pub fn core_error(err: ParserError) -> syn::Error {
    syn_error(err)
}
