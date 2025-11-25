use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Error, Ident, Result, Token, TraitItemFn,
};

/// Configuration for method variant generation
#[derive(Debug, Clone)]
pub struct VariantConfig {
    pub variant_type: VariantType,
}

#[derive(Debug, Clone)]
pub enum VariantType {
    Pool,      // Adds pool: &sqlx_data::Pool, suffix: _with_pool
    Tx,        // Adds transaction: &mut sqlx_data::Transaction<'_>, suffix: _with_tx
    Conn,      // Adds connection: &mut sqlx::Connection, suffix: _with_conn
    Exec,      // Adds executor: impl sqlx_data::Executor<'_>, suffix: _with_executor
}

impl Parse for VariantConfig {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        let variant_type = match ident.to_string().as_str() {
            "pool" => VariantType::Pool,
            "tx" => VariantType::Tx,
            "conn" => VariantType::Conn,
            "exec" => VariantType::Exec,
            _ => {
                return Err(Error::new(
                    ident.span(),
                    "Invalid variant type. Expected: pool, tx, conn, or exec",
                ))
            }
        };
        Ok(VariantConfig { variant_type })
    }
}



/// Generate a single method variant
fn generate_single_variant(
    original: &TraitItemFn,
    variant: &VariantConfig,
) -> Result<TraitItemFn> {
    let mut new_method = original.clone();

    // Remove the generate_versions attribute from the new method
    new_method.attrs.retain(|attr| !attr.path().is_ident("generate_versions"));

    // Generate new method name with suffix
    let original_name = &original.sig.ident;
    let suffix = get_variant_suffix(&variant.variant_type);
    let new_name = format_ident!("{}{}", original_name, suffix);
    new_method.sig.ident = new_name;

    // Add variant-specific parameter
    let new_param = generate_variant_parameter(&variant.variant_type)?;

    // Insert the new parameter after &self
    let mut new_inputs = Punctuated::new();

    // Keep &self as first parameter
    if let Some(first_input) = original.sig.inputs.first() {
        new_inputs.push(first_input.clone());
    }

    // Add the variant parameter
    new_inputs.push(new_param);

    // Add remaining original parameters
    for input in original.sig.inputs.iter().skip(1) {
        new_inputs.push(input.clone());
    }

    new_method.sig.inputs = new_inputs;

    Ok(new_method)
}

/// Get the suffix for each variant type
fn get_variant_suffix(variant_type: &VariantType) -> &'static str {
    match variant_type {
        VariantType::Pool => "_with_pool",
        VariantType::Tx => "_with_tx",
        VariantType::Conn => "_with_conn",
        VariantType::Exec => "_with_executor",
    }
}

/// Generate the parameter for each variant type
fn generate_variant_parameter(variant_type: &VariantType) -> Result<syn::FnArg> {
    let param_tokens = match variant_type {
        VariantType::Pool => {
            quote! { pool: &sqlx_data::Pool }
        }
        VariantType::Tx => {
            quote! { transaction: &mut sqlx_data::Transaction<'_> }
        }
        VariantType::Conn => {
            quote! { connection: &mut sqlx_data::Connection }
        }
        VariantType::Exec => {
            quote! { executor: impl sqlx_data::Executor<'_> }
        }
    };

    syn::parse2(param_tokens).map_err(|e| Error::new(e.span(), "Failed to parse generated parameter"))
}



/// Main entry point for the generate_versions macro applied to individual methods
pub fn expand_method_variants(input_method: TraitItemFn, args: TokenStream) -> Result<TokenStream> {
    // Parse the variant types from args
    let variant_configs = if args.is_empty() {
        Vec::new()
    } else {
        let variants = syn::parse::Parser::parse2(
            syn::punctuated::Punctuated::<VariantConfig, Token![,]>::parse_terminated,
            args,
        )?;
        variants.into_iter().collect()
    };

    // Generate the original method and all variants
    let mut all_methods = vec![];

    // Add original method (unchanged)
    all_methods.push(quote! { #input_method });

    // Generate variants
    for variant in &variant_configs {
        let variant_method = generate_single_variant(&input_method, variant)?;
        all_methods.push(quote! { #variant_method });
    }

    Ok(quote! {
        #(#all_methods)*
    })
}



#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_parse_variant_config() {
        let config: VariantConfig = syn::parse_str("pool").unwrap();
        assert!(matches!(config.variant_type, VariantType::Pool));

        let config: VariantConfig = syn::parse_str("tx").unwrap();
        assert!(matches!(config.variant_type, VariantType::Tx));

        let config: VariantConfig = syn::parse_str("conn").unwrap();
        assert!(matches!(config.variant_type, VariantType::Conn));

        let config: VariantConfig = syn::parse_str("exec").unwrap();
        assert!(matches!(config.variant_type, VariantType::Exec));
    }

    #[test]
    fn test_variant_suffix() {
        assert_eq!(get_variant_suffix(&VariantType::Pool), "_with_pool");
        assert_eq!(get_variant_suffix(&VariantType::Tx), "_with_tx");
        assert_eq!(get_variant_suffix(&VariantType::Conn), "_with_conn");
        assert_eq!(get_variant_suffix(&VariantType::Exec), "_with_executor");
    }

    #[test]
    fn test_generate_variant_parameter() {
        let pool_param = generate_variant_parameter(&VariantType::Pool).unwrap();
        let expected: syn::FnArg = parse_quote! { pool: &sqlx_data::Pool };
        assert_eq!(quote! { #pool_param }.to_string(), quote! { #expected }.to_string());

        let tx_param = generate_variant_parameter(&VariantType::Tx).unwrap();
        let expected: syn::FnArg = parse_quote! { transaction: &mut sqlx_data::Transaction<'_> };
        assert_eq!(quote! { #tx_param }.to_string(), quote! { #expected }.to_string());
    }

    #[test]
    fn test_single_variant_generation() {
        let original_method: TraitItemFn = parse_quote! {
            #[dml("DELETE FROM users WHERE id = ?")]
            async fn delete_user(&self, id: i64) -> Result<QueryResult>;
        };

        let pool_variant = VariantConfig { variant_type: VariantType::Pool };
        let pool_method = generate_single_variant(&original_method, &pool_variant).unwrap();

        // Check generated pool method
        assert_eq!(pool_method.sig.ident.to_string(), "delete_user_with_pool");
        assert_eq!(pool_method.sig.inputs.len(), 3); // &self, pool, id

        // Should still have dml attribute
        assert!(pool_method.attrs.iter().any(|attr| attr.path().is_ident("dml")));

        let tx_variant = VariantConfig { variant_type: VariantType::Tx };
        let tx_method = generate_single_variant(&original_method, &tx_variant).unwrap();

        // Check generated tx method
        assert_eq!(tx_method.sig.ident.to_string(), "delete_user_with_tx");
        assert_eq!(tx_method.sig.inputs.len(), 3); // &self, transaction, id
    }
}