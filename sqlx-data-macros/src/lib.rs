use proc_macro::TokenStream;
use syn::{TraitItemFn, parse_macro_input};

mod alias_system;
mod code_generator;
mod constants;
mod dml;
mod error;
mod fetch;
mod method_variants;
mod repo_system;
mod scope_system;
mod type_analyzer;
mod type_system;

#[cfg(test)]
mod test_framework;

use code_generator::CodeGenerator;
use dml::DmlParser;

/// Attribute macro for DML (Data Manipulation Language) statements with compile-time validation
///
/// This macro generates type-safe, high-performance database operations with automatic parameter binding,
/// pagination injection, and query optimization. All SQL is validated at compile time using SQLx's
/// compile-time verification.
///
/// # Features
/// - **Compile-time SQL validation** - Catches SQL errors during compilation
/// - **Type-safe parameter binding** - Automatic conversion and validation of parameters
/// - **Pagination support** - Automatic injection of LIMIT/OFFSET clauses
/// - **Query optimization** - Smart query planning and execution
/// - **Error handling** - Comprehensive error types with context
/// - **Generated documentation** - Auto-documented methods with query details
///
/// # Usage
/// ```ignore
/// use sqlx_data_macros::{dml, repo, Pool, Result};
/// use sqlx::FromRow;
///
/// #[derive(FromRow)]
/// struct User {
///     id: i64,
///     name: String,
///     email: String,
/// }
///
/// #[repo]
/// trait UserRepo {
///     // Simple query returning a single record
///     #[dml("SELECT * FROM users WHERE id = $1")]
///     async fn find_by_id(&self, id: i64) -> Result<Option<User>>;
///
///     // Query returning multiple records
///     #[dml("SELECT * FROM users WHERE active = $1")]
///     async fn find_active_users(&self, active: bool) -> Result<Vec<User>>;
///
///     // Insert/Update/Delete operations
///     #[dml("INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id")]
///     async fn create_user(&self, name: String, email: String) -> Result<i64>;
///
///     // Complex queries with joins
///     #[dml("SELECT u.* FROM users u JOIN roles r ON u.role_id = r.id WHERE r.name = $1")]
///     async fn find_users_by_role(&self, role_name: String) -> Result<Vec<User>>;
///
///     // Pagination-enabled queries (automatic LIMIT/OFFSET injection)
///     #[dml("SELECT * FROM users ORDER BY created_at")]
///     async fn find_all_paged(&self) -> Result<Vec<User>>;
/// }
/// ```
///
/// # Required Imports
/// ```ignore
/// use sqlx_data_macros::{dml, repo};
/// use sqlx_data_integration::{Pool, Result, Database}; // Core database types
/// use sqlx::FromRow; // For result mapping
/// ```
#[proc_macro_attribute]
pub fn dml(args: TokenStream, input: TokenStream) -> TokenStream {
    let method = parse_macro_input!(input as TraitItemFn);

    // Parse the DML arguments directly
    let parsed_method = match DmlParser::parse_dml_method_with_args(method, args, false) {
        Ok(method) => method,
        Err(error) => return error.to_compile_error().into(),
    };

    // Generate code
    match CodeGenerator::generate_dml_methods(&parsed_method) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Attribute macro for repositories - processes aliases, scopes and adds get_pool method
#[proc_macro_attribute]
pub fn repo(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_trait = parse_macro_input!(input as syn::ItemTrait);

    match repo_system::RepoProcessor::process_trait_with_args(input_trait, args) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Attribute macro for generating method variants with different executor types
///
/// This macro generates additional method variants that accept different executor types.
/// Apply this macro to individual methods that need variants.
///
/// # Supported Variant Types
/// - `pool` - Adds `pool: &sqlx_data::Pool` parameter and `_with_pool` suffix
/// - `tx` - Adds `transaction: &mut sqlx_data::Transaction<'_>` parameter and `_with_tx` suffix
/// - `conn` - Adds `connection: &mut sqlx_data::Connection` parameter and `_with_conn` suffix
/// - `exec` - Adds `executor: impl sqlx_data::Executor<'_>` parameter and `_with_executor` suffix
///
/// # Usage
/// ```ignore
/// use sqlx_data_macros::{generate_versions, dml, repo};
///
/// #[repo]
/// trait UserRepo {
///     // Original method with variants
///     #[generate_versions(pool, tx)]
///     #[dml("DELETE FROM users WHERE id = $1")]
///     async fn delete_user(&self, id: i64) -> Result<QueryResult>;
/// }
/// ```
///
/// # Generated Output
/// The macro generates additional methods alongside the original:
/// ```ignore
/// // Original (preserved)
/// #[dml("DELETE FROM users WHERE id = $1")]
/// async fn delete_user(&self, id: i64) -> Result<QueryResult>;
///
/// // Generated variants
/// #[dml("DELETE FROM users WHERE id = $1")]
/// async fn delete_user_with_pool(&self, pool: &sqlx_data::Pool, id: i64) -> Result<QueryResult>;
///
/// #[dml("DELETE FROM users WHERE id = $1")]
/// async fn delete_user_with_tx(&self, transaction: &mut sqlx_data::Transaction<'_>, id: i64) -> Result<QueryResult>;
/// ```
#[proc_macro_attribute]
pub fn generate_versions(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_method = parse_macro_input!(input as TraitItemFn);
    let args_tokens = proc_macro2::TokenStream::from(args);

    match method_variants::expand_method_variants(input_method, args_tokens) {
        Ok(tokens) => TokenStream::from(tokens),
        Err(error) => error.to_compile_error().into(),
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    // Helper function to create a DmlMethod for testing
    fn create_test_dml_method(
        method_name: &str,
        sql: &str,
        parameters: Vec<crate::dml::DmlParameter>,
        return_type: syn::Type,
    ) -> crate::dml::DmlMethod {
        use syn::{FnArg, Pat, PatIdent, PatType, Signature, TraitItemFn};

        // Create function signature
        let mut inputs = syn::punctuated::Punctuated::new();

        // Add self parameter
        inputs.push(FnArg::Receiver(syn::Receiver {
            attrs: vec![],
            reference: Some((syn::Token![&](proc_macro2::Span::call_site()), None)),
            mutability: None,
            self_token: syn::Token![self](proc_macro2::Span::call_site()),
            colon_token: None,
            ty: Box::new(parse_quote! { Self }),
        }));

        // Add other parameters
        for param in &parameters {
            let pat = PatIdent {
                attrs: vec![],
                by_ref: None,
                mutability: None,
                ident: syn::Ident::new(&param.name, proc_macro2::Span::call_site()),
                subpat: None,
            };

            inputs.push(FnArg::Typed(PatType {
                attrs: vec![],
                pat: Box::new(Pat::Ident(pat)),
                colon_token: syn::Token![:](proc_macro2::Span::call_site()),
                ty: Box::new(param.type_.clone()),
            }));
        }

        // Don't add async for Stream return types
        let is_stream_type = matches!(&return_type, syn::Type::ImplTrait(impl_trait)
            if impl_trait.bounds.iter().any(|bound| {
                if let syn::TypeParamBound::Trait(trait_bound) = bound {
                    trait_bound.path.segments.last()
                        .map_or(false, |seg| seg.ident == "Stream")
                } else {
                    false
                }
            })
        );

        let sig = Signature {
            constness: None,
            asyncness: if is_stream_type {
                None
            } else {
                Some(syn::Token![async](proc_macro2::Span::call_site()))
            },
            unsafety: None,
            abi: None,
            fn_token: syn::Token![fn](proc_macro2::Span::call_site()),
            ident: syn::Ident::new(method_name, proc_macro2::Span::call_site()),
            generics: syn::Generics::default(),
            paren_token: syn::token::Paren::default(),
            inputs,
            variadic: None,
            output: syn::ReturnType::Type(
                syn::Token![->](proc_macro2::Span::call_site()),
                Box::new(return_type),
            ),
        };

        let trait_method = TraitItemFn {
            attrs: vec![],
            sig,
            default: None,
            semi_token: Some(syn::Token![;](proc_macro2::Span::call_site())),
        };

        crate::dml::DmlMethod {
            method: trait_method,
            sql_content: sql.to_string(),
            parameters,
            statement: sqlx_data_parser::parse_sql(sql).unwrap(),
            kind: sqlx_data_parser::SqlStatementType::Select,
            is_json_query: false,
            is_multi_insert: false,
            is_unchecked: false,
            has_explicit_instrument: false,
            trait_instrument: false,
            return_info_cache: std::sync::OnceLock::new(),
        }
    }

    #[test]
    fn test_dml_macro_basic() {
        use crate::code_generator::CodeGenerator;
        use crate::dml::DmlParameter;
        use syn::parse_quote;

        let method = create_test_dml_method(
            "find_by_id",
            "SELECT * FROM users WHERE id = $1",
            vec![DmlParameter {
                name: "id".to_string(),
                type_: parse_quote! { i64 },
                is_pool: false,
                is_dynamic_param: false,
                is_generic: false,
            }],
            parse_quote! { Result<User> },
        );

        let result = CodeGenerator::generate_dml_methods(&method);
        assert!(result.is_ok());

        let generated_code = result.unwrap().to_string();
        assert!(generated_code.contains("find_by_id_query"));
        assert!(generated_code.contains("find_by_id"));
        assert!(generated_code.contains("sqlx::query_as!"));
    }

    #[test]
    fn test_dml_macro_with_flatten() {
        use crate::code_generator::CodeGenerator;
        use crate::dml::DmlParameter;
        use syn::parse_quote;

        let method = create_test_dml_method(
            "get_birth_year",
            "SELECT birth_year FROM users WHERE id = $1",
            vec![DmlParameter {
                name: "id".to_string(),
                type_: parse_quote! { i64 },
                is_pool: false,
                is_dynamic_param: false,
                is_generic: false,
            }],
            parse_quote! { Result<Option<i64>> },
        );

        let result = CodeGenerator::generate_dml_methods(&method);
        assert!(result.is_ok());

        let generated_code = result.unwrap().to_string();
        assert!(generated_code.contains("get_birth_year_query"));
        assert!(generated_code.contains("get_birth_year"));
        assert!(generated_code.contains("sqlx::query_scalar!"));
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_tuple_f32_casting() {
        use crate::code_generator::CodeGenerator;
        use syn::parse_quote;

        let method = create_test_dml_method(
            "group_avg",
            "SELECT birth_year, AVG(age) as avg_age FROM users GROUP BY birth_year",
            vec![],
            parse_quote! { Result<Vec<(Option<u16>, f32)>> },
        );

        let result = CodeGenerator::generate_dml_methods(&method);
        assert!(result.is_ok());

        let generated_code = result.unwrap().to_string();
        eprintln!("Generated Code for tuple casting:\n{}", generated_code);

        // Verify that f64 -> f32 casting is present (SQLite returns f64 for AVG)
        assert!(generated_code.contains("as f32"));
        // Verify that i64 -> u16 casting is present for Option<u16> (SQLite uses i64 for integers)
        assert!(generated_code.contains("as u16"));
        assert!(generated_code.contains("group_avg_query"));
        assert!(generated_code.contains("QueryTuple"));
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_tuple_f64_casting() {
        use crate::code_generator::CodeGenerator;
        use crate::dml::DmlParameter;
        use syn::parse_quote;

        let method = create_test_dml_method(
            "group_having_avg",
            "SELECT birth_year, AVG(age) as avg_age FROM users WHERE birth_year IS NOT NULL GROUP BY birth_year HAVING AVG(age) > $1",
            vec![DmlParameter {
                name: "min_avg".to_string(),
                type_: parse_quote! { f32 },
                is_pool: false,
                is_dynamic_param: false,
                is_generic: false,
            }],
            parse_quote! { Result<Vec<(Option<u16>, f64)>> },
        );

        let result = CodeGenerator::generate_dml_methods(&method);
        assert!(result.is_ok());

        let generated_code = result.unwrap().to_string();
        eprintln!("Generated Code for f64 casting:\\n{}", generated_code);

        // Verify that i64 -> u16 casting is present for Option<u16> (SQLite uses i64 for integers)
        assert!(generated_code.contains("as u16"));
        assert!(generated_code.contains("group_having_avg_query"));
        assert!(generated_code.contains("QueryTuple"));
    }

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_tuple_i64_usize_casting() {
        use crate::code_generator::CodeGenerator;
        use syn::parse_quote;

        let method = create_test_dml_method(
            "count_by_year",
            "SELECT birth_year, COUNT(*) as count FROM users GROUP BY birth_year",
            vec![],
            parse_quote! { Result<Vec<(Option<i64>, usize)>> },
        );

        let result = CodeGenerator::generate_dml_methods(&method);
        assert!(result.is_ok());

        let generated_code = result.unwrap().to_string();
        eprintln!("Generated Code for i64/usize casting:\\n{}", generated_code);

        // i64 shouldn't need casting (i64 -> i64)
        // usize should need casting (i64 -> usize) in SQLite
        assert!(generated_code.contains("as usize"));
        assert!(generated_code.contains("count_by_year_query"));
    }

    #[test]
    fn test_documentation_generation() {
        use crate::code_generator::CodeGenerator;
        use syn::parse_quote;

        let method = create_test_dml_method(
            "find_by_id",
            "SELECT * FROM users WHERE id = $1",
            vec![],
            parse_quote! { Result<User> },
        );

        let result = CodeGenerator::generate_dml_methods(&method);
        assert!(result.is_ok());

        let generated_code = result.unwrap().to_string();
        eprintln!("Generated Code:\n{}", generated_code);

        // Verify that documentation comment is generated on the public method
        assert!(generated_code.contains("# [doc = "));
        assert!(generated_code.contains("Generated by #[dml] macro:"));
        assert!(generated_code.contains("```rust"));
        assert!(generated_code.contains("find_by_id_query"));
        assert!(generated_code.contains("sqlx :: query_as !"));
    }
}
