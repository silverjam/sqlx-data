use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{dml::DmlMethod, type_system::FetchMethod};

/// Deref pattern for different parameter types
#[derive(Debug, Clone)]
enum DerefPattern {
    None,        // Regular pool: pool
    SingleDeref, // Connection: &mut *conn
    DoubleDeref, // Transaction: &mut **tx
}

pub fn generate_fetch_call_expr(
    fetch_method: &FetchMethod,
    pool_expr: &TokenStream,
) -> TokenStream {
    match fetch_method {
        FetchMethod::Execute => quote! { .execute(#pool_expr) },
        FetchMethod::FetchOne => quote! { .fetch_one(#pool_expr) },
        FetchMethod::FetchAll => quote! { .fetch_all(#pool_expr) },
        FetchMethod::FetchOptional => quote! { .fetch_optional(#pool_expr) },
        FetchMethod::Fetch => quote! { .fetch(#pool_expr) },
    }
}

/// Generate pool expression - use provided pool or self.get_pool()
/// Automatically handles deref for Transaction parameters
pub fn generate_pool_expr(method: &DmlMethod) -> TokenStream {
    if let Some(pool_param) = method.parameters.iter().find(|p| p.is_pool) {
        let pool_name = format_ident!("{}", pool_param.name);

        // Check the specific type to determine dereferencing pattern
        match get_deref_pattern(&pool_param.type_) {
            DerefPattern::DoubleDeref => quote! { &mut **#pool_name }, // Transaction
            DerefPattern::SingleDeref => quote! { &mut *#pool_name },  // Connection
            DerefPattern::None => quote! { #pool_name },               // Pool
        }
    } else {
        quote! { self.get_pool() }
    }
}

/// Determine the deref pattern needed for different parameter types
fn get_deref_pattern(ty: &syn::Type) -> DerefPattern {
    let syn::Type::Reference(type_ref) = ty else {
        return DerefPattern::None;
    };

    // Check if it's a mutable reference
    if type_ref.mutability.is_none() {
        return DerefPattern::None;
    }

    let syn::Type::Path(type_path) = &*type_ref.elem else {
        return DerefPattern::None;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return DerefPattern::None;
    };

    let segment_name = segment.ident.to_string();

    match segment_name.as_str() {
        "Transaction" => DerefPattern::DoubleDeref, // &mut **tx
        // SqliteConnection | PgConnection | MySqlConnection | etc.
        name if name.ends_with("Connection") => DerefPattern::SingleDeref, // &mut *conn
        _ => DerefPattern::None, // Regular pool or other types
    }
}
