//! Repository trait processing system
//!
//! This module handles the processing of traits annotated with #[repo],
//! including alias and scope management, attribute injection, and trait augmentation.

use crate::alias_system::AliasManager;
use crate::scope_system::ScopeManager;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{ItemTrait, TraitItem};

/// Handles processing of repository traits
pub struct RepoProcessor;

impl RepoProcessor {
    pub fn process_trait_with_args(
        mut input_trait: ItemTrait,
        args: proc_macro::TokenStream,
    ) -> syn::Result<proc_macro2::TokenStream> {
        // Parse trait-level instrument flag
        let trait_instrument = Self::parse_instrument_flag(args)?;
        // Parse aliases from trait attributes
        let alias_manager = AliasManager::parse_from_attributes(&input_trait.attrs)?;

        // Parse scopes from trait attributes
        let scope_manager = ScopeManager::parse_from_attributes(&input_trait.attrs, &[])?;

        // Remove alias and scope attributes from the trait (they're processed)
        input_trait
            .attrs
            .retain(|attr| !attr.path().is_ident("alias") && !attr.path().is_ident("scope"));

        // Inject alias, scope and instrument attributes into DML methods
        Self::inject_attributes_into_methods(
            &mut input_trait,
            &alias_manager,
            &scope_manager,
            trait_instrument,
        )?;

        // Add get_pool method to the trait with default implementation
        Self::add_get_pool_method(&mut input_trait);

        // Instead of using quote! which loses spans, convert to TokenStream directly
        Ok(input_trait.into_token_stream())
    }

    /// Parse instrument flag from repo arguments
    fn parse_instrument_flag(args: proc_macro::TokenStream) -> syn::Result<bool> {
        use syn::{Expr, Token, parse::Parser, punctuated::Punctuated};

        if args.is_empty() {
            return Ok(false);
        }

        let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
        let args = parser.parse(args)?;

        for arg in &args {
            if let Expr::Assign(assign) = arg {
                let Expr::Path(path) = &*assign.left else {
                    return Err(syn::Error::new_spanned(
                        &assign.left,
                        "Invalid parameter name",
                    ));
                };

                if path.path.is_ident("instrument") {
                    let Expr::Lit(expr_lit) = &*assign.right else {
                        return Err(syn::Error::new_spanned(
                            &assign.right,
                            "instrument parameter must be a boolean literal",
                        ));
                    };

                    let syn::Lit::Bool(lit_bool) = &expr_lit.lit else {
                        return Err(syn::Error::new_spanned(
                            &assign.right,
                            "instrument parameter must be true or false",
                        ));
                    };

                    return Ok(lit_bool.value);
                }
            }
        }

        Ok(false)
    }

    /// Inject alias, scope and instrument attributes into DML methods
    fn inject_attributes_into_methods(
        input_trait: &mut ItemTrait,
        alias_manager: &AliasManager,
        scope_manager: &ScopeManager,
        trait_instrument: bool,
    ) -> syn::Result<()> {
        for item in &mut input_trait.items {
            if let TraitItem::Fn(method) = item {
                // Check if method has #[dml] attribute
                let has_dml_attr = method.attrs.iter().any(|attr| attr.path().is_ident("dml"));

                if has_dml_attr {
                    Self::process_dml_method(
                        method,
                        alias_manager,
                        scope_manager,
                        trait_instrument,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Process a single DML method - inject aliases, scopes and instrument flag
    fn process_dml_method(
        method: &mut syn::TraitItemFn,
        alias_manager: &AliasManager,
        scope_manager: &ScopeManager,
        trait_instrument: bool,
    ) -> syn::Result<()> {
        // Parse method-specific scope_ignore attributes
        let method_scope_manager = ScopeManager::parse_from_attributes(&[], &method.attrs)?;

        // Merge trait scopes with method ignores
        let mut final_scope_manager = scope_manager.clone();
        for ignored_scope in method_scope_manager.get_ignored_scope_names() {
            final_scope_manager.add_ignored_scope(ignored_scope);
        }

        // Apply alias substitution to scope SQL content
        final_scope_manager = final_scope_manager.substitute_scope_aliases(alias_manager)?;

        // Add hidden alias attribute if aliases exist
        if alias_manager.has_aliases() {
            let alias_data = alias_manager.serialize_for_injection();
            let alias_attr: syn::Attribute = syn::parse_quote_spanned! { method.span() =>
                #[sqlx_data_aliases = #alias_data]
            };
            method.attrs.push(alias_attr);
        }

        // Add hidden scope attribute if scopes exist
        if final_scope_manager.has_active_scopes() {
            let scope_data = final_scope_manager.serialize_for_injection();
            let scope_attr: syn::Attribute = syn::parse_quote_spanned! { method.span() =>
                #[sqlx_data_scopes = #scope_data]
            };
            method.attrs.push(scope_attr);
        }

        // Add hidden instrument attribute if trait-level instrument is enabled
        if trait_instrument {
            let instrument_attr: syn::Attribute = syn::parse_quote_spanned! { method.span() =>
                #[sqlx_data_trait_instrument = true]
            };
            method.attrs.push(instrument_attr);
        }

        // Remove scope_ignore attributes from method (they're processed)
        method
            .attrs
            .retain(|attr| !attr.path().is_ident("scope_ignore"));

        Ok(())
    }

    /// Add the required get_pool method to the trait
    fn add_get_pool_method(input_trait: &mut ItemTrait) {
        let method: TraitItem = syn::parse_quote! {
            fn get_pool(&self) -> &sqlx_data::Pool{
                unimplemented!("Implement get_pool() to use methods without pool parameters, or pass pool explicitly via method parameters")
            }
        };
        input_trait.items.push(method);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_repo_processor_basic() {
        let input_trait: ItemTrait = parse_quote! {
            #[alias(user_table = "users")]
            trait UserRepo {
                #[dml("SELECT * FROM {{user_table}} WHERE id = $1")]
                async fn find_by_id(&self, id: i64) -> Result<User>;
            }
        };

        let result =
            RepoProcessor::process_trait_with_args(input_trait, proc_macro::TokenStream::new());
        assert!(result.is_ok());

        let generated_code = result.unwrap().to_string();
        assert!(generated_code.contains("get_pool"));
        assert!(generated_code.contains("UserRepo"));
    }

    #[test]
    fn test_repo_processor_with_scopes() {
        let input_trait: ItemTrait = parse_quote! {
            #[alias(user_table = "users")]
            #[scope(active = "active = true")]
            trait UserRepo {
                #[dml("SELECT * FROM {{user_table}}")]
                async fn find_active(&self) -> Result<Vec<User>>;

                #[scope_ignore(active)]
                #[dml("SELECT * FROM {{user_table}} WHERE id = $1")]
                async fn find_by_id(&self, id: i64) -> Result<User>;
            }
        };

        let result =
            RepoProcessor::process_trait_with_args(input_trait, proc_macro::TokenStream::new());
        assert!(result.is_ok());

        let generated_code = result.unwrap().to_string();
        assert!(generated_code.contains("get_pool"));
        assert!(!generated_code.contains("#[alias(")); // Should be removed from trait
        assert!(!generated_code.contains("#[scope(")); // Should be removed from trait
    }
}
