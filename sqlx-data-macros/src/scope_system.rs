use crate::error::{core_error, sql_parse_error, syn_error};
use sqlparser::ast::GroupByExpr;
use std::collections::HashMap;
use syn::{Attribute, LitStr, Result as SynResult};

/// Manages SQL scopes for a repository trait
/// Scopes add conditions to specific SQL clauses with direct SQL content
/// Scopes are applied AFTER alias substitution using SQL parser
#[derive(Debug, Clone)]
pub struct ScopeManager {
    scopes: HashMap<String, ScopeDefinition>,
    ignored_scopes: Vec<String>,
}

/// Represents a scope with its SQL content and target location
#[derive(Debug, Clone)]
pub struct ScopeDefinition {
    #[allow(dead_code)]
    pub name: String,
    pub sql: String,
    pub target: ScopeTarget,
}

/// Target location for scope application
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScopeTarget {
    Select,
    From,
    Join,
    Where, // Default
    GroupBy,
    Having,
    OrderBy,
}

impl ScopeTarget {
    fn from_string(s: &str) -> SynResult<Self> {
        match s.to_lowercase().as_str() {
            "select" => Ok(ScopeTarget::Select),
            "from" => Ok(ScopeTarget::From),
            "join" => Ok(ScopeTarget::Join),
            "where" => Ok(ScopeTarget::Where),
            "group_by" | "groupby" => Ok(ScopeTarget::GroupBy),
            "having" => Ok(ScopeTarget::Having),
            "order_by" | "orderby" => Ok(ScopeTarget::OrderBy),
            _ => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "Invalid scope target: '{}'. Valid targets: select, from, join, where, group_by, having, order_by",
                    s
                ),
            )),
        }
    }
}

impl ScopeManager {
    pub fn new() -> Self {
        Self {
            scopes: HashMap::new(),
            ignored_scopes: Vec::new(),
        }
    }

    /// Parse scope and scope_ignore attributes from trait and method attributes
    pub fn parse_from_attributes(
        trait_attrs: &[Attribute],
        method_attrs: &[Attribute],
    ) -> SynResult<Self> {
        let mut manager = Self::new();

        // Parse scope definitions from trait attributes
        for attr in trait_attrs {
            if attr.path().is_ident("scope") {
                manager.parse_scope_attribute(attr)?;
            }
        }

        // Parse scope_ignore from method attributes
        for attr in method_attrs {
            if attr.path().is_ident("scope_ignore") {
                manager.parse_scope_ignore_attribute(attr)?;
            }
        }

        Ok(manager)
    }

    /// Parse a single scope attribute: #[scope(name = "sql")] or #[scope(name = "sql", target = "where")]
    fn parse_scope_attribute(&mut self, attr: &Attribute) -> SynResult<()> {
        let mut scope_name: Option<String> = None;
        let mut scope_sql: Option<String> = None;
        let mut target = ScopeTarget::Where; // default target

        // Parse each meta item
        attr.parse_nested_meta(|meta| {
            let ident = meta
                .path
                .get_ident()
                .ok_or_else(|| meta.error("Expected a scope name or target"))?;
            let key = ident.to_string();

            // Expect `key = "value"`
            let _ = meta
                .input
                .parse::<syn::Token![=]>()
                .map_err(|_| meta.error("Expected `key = \"value\"` format"))?;

            let lit: LitStr = meta.input.parse()?;
            let value = lit.value();

            match key.as_str() {
                "target" => target = ScopeTarget::from_string(&value)?,
                _ => {
                    scope_name = Some(key);
                    scope_sql = Some(value);
                }
            }

            Ok(())
        })?;

        let scope_name = scope_name.ok_or(syn_error("Scope name is required"))?;
        let scope_sql = scope_sql.ok_or(syn_error("Scope SQL content is required"))?;

        self.add_scope(scope_name, scope_sql, target);
        Ok(())
    }

    /// Parse scope_ignore attribute: #[scope_ignore(versioned)]
    fn parse_scope_ignore_attribute(&mut self, attr: &Attribute) -> SynResult<()> {
        attr.parse_nested_meta(|meta| {
            let ident = meta
                .path
                .get_ident()
                .ok_or_else(|| meta.error("Expected scope name to ignore"))?;
            self.add_ignored_scope(ident.to_string());
            Ok(())
        })
    }

    /// Add a scope with its SQL content and target
    pub fn add_scope(&mut self, scope_name: String, sql: String, target: ScopeTarget) {
        let definition = ScopeDefinition {
            name: scope_name.clone(),
            sql,
            target,
        };
        self.scopes.insert(scope_name, definition);
    }

    /// Add a scope to be ignored
    pub fn add_ignored_scope(&mut self, scope_name: String) {
        self.ignored_scopes.push(scope_name);
    }

    /// Get all active scopes (excluding ignored ones)
    pub fn get_active_scopes(&self) -> HashMap<String, &ScopeDefinition> {
        self.scopes
            .iter()
            .filter(|(scope_name, _)| !self.ignored_scopes.contains(scope_name))
            .map(|(k, v)| (k.clone(), v))
            .collect()
    }

    /// Get active scopes grouped by target
    pub fn get_scopes_by_target(&self) -> HashMap<&ScopeTarget, Vec<&ScopeDefinition>> {
        self.get_active_scopes()
            .into_iter()
            .fold(HashMap::new(), |mut map, (_, scope)| {
                map.entry(&scope.target) // ← reference, no clone!
                    .or_default() // ← shorter and faster
                    .push(scope);
                map
            })
    }

    /// Check if manager has any active scopes
    pub fn has_active_scopes(&self) -> bool {
        !self.get_active_scopes().is_empty()
    }

    /// Get all scope names for debugging
    #[allow(dead_code)]
    pub fn get_scope_names(&self) -> Vec<String> {
        self.scopes.keys().cloned().collect()
    }

    /// Get ignored scope names for debugging
    pub fn get_ignored_scope_names(&self) -> Vec<String> {
        self.ignored_scopes.clone()
    }

    /// Serialize scopes to inject as hidden attribute on methods
    pub fn serialize_for_injection(&self) -> String {
        if self.scopes.is_empty() && self.ignored_scopes.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();

        // Serialize scopes: "scope1=sql_content:where;scope2=sql_content:select"
        if !self.scopes.is_empty() {
            let scopes_str = self
                .scopes
                .iter()
                .map(|(scope_name, definition)| {
                    let target_str = match definition.target {
                        ScopeTarget::Select => "select",
                        ScopeTarget::From => "from",
                        ScopeTarget::Join => "join",
                        ScopeTarget::Where => "where",
                        ScopeTarget::GroupBy => "group_by",
                        ScopeTarget::Having => "having",
                        ScopeTarget::OrderBy => "order_by",
                    };
                    // Escape semicolons and quotes for safe attribute injection
                    let escaped_sql = definition.sql.replace("\"", "\\\"").replace(";", "\\;");
                    format!("{}={}:{}", scope_name, escaped_sql, target_str)
                })
                .collect::<Vec<_>>()
                .join(";");
            parts.push(format!("scopes:{}", scopes_str));
        }

        // Serialize ignored scopes: "ignore:scope1,scope2"
        if !self.ignored_scopes.is_empty() {
            let ignored_str = self.ignored_scopes.join(",");
            parts.push(format!("ignore:{}", ignored_str));
        }

        parts.join("|")
    }

    /// Deserialize scopes from injected attribute
    pub fn deserialize_from_injection(serialized: &str) -> SynResult<Self> {
        let mut manager = Self::new();

        let serialized = serialized.trim();
        if serialized.is_empty() {
            return Ok(manager);
        }

        for part in serialized.split('|') {
            let (prefix, data) = part.split_once(':').ok_or_else(|| {
                syn_error(format!("Invalid scope format in injection: '{}'", part))
            })?;

            match prefix {
                "scopes" => {
                    for scope_entry in data.split(';') {
                        let (scope_name, sql_and_target) = match scope_entry.split_once('=') {
                            Some(pair) => pair,
                            None => continue, // skip invalid entries
                        };

                        let (sql_content, target_str) =
                            sql_and_target.rsplit_once(':').ok_or_else(|| {
                                syn_error(format!(
                                    "Invalid scope format, missing target: '{}'",
                                    scope_entry
                                ))
                            })?;

                        let target = ScopeTarget::from_string(target_str)?;
                        let unescaped_sql = sql_content.replace("\\\"", "\"").replace("\\;", ";");

                        manager.add_scope(scope_name.trim().to_string(), unescaped_sql, target);
                    }
                }
                "ignore" => {
                    for ignored_scope in data.split(',') {
                        let scope = ignored_scope.trim();
                        if !scope.is_empty() {
                            manager.add_ignored_scope(scope.to_string());
                        }
                    }
                }
                _ => {
                    return Err(syn_error(format!(
                        "Invalid scope prefix in injection: '{}'",
                        prefix
                    )));
                }
            }
        }

        Ok(manager)
    }

    /// Extract scopes from method's hidden attribute (used by DML parsing)
    pub fn extract_from_method_attributes(attrs: &[syn::Attribute]) -> SynResult<Self> {
        for attr in attrs {
            if !attr.path().is_ident("sqlx_data_scopes") {
                continue;
            }

            let meta = match &attr.meta {
                syn::Meta::NameValue(meta) => meta,
                _ => continue, // skip invalid meta
            };

            let lit_str = match &meta.value {
                syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
                    syn::Lit::Str(s) => s,
                    _ => continue, // skip non-string literal
                },
                _ => continue,
            };
            return Self::deserialize_from_injection(&lit_str.value());
        }
        // No scopes found
        Ok(Self::new())
    }

    /// Apply scope SQL through alias substitution
    /// This prepares scope SQL by replacing aliases within each scope's SQL content
    pub fn substitute_scope_aliases(
        &self,
        alias_manager: &crate::alias_system::AliasManager,
    ) -> SynResult<ScopeManager> {
        let mut resolved_manager = self.clone();

        // Clear and rebuild scopes with alias-substituted SQL
        resolved_manager.scopes.clear();

        for (scope_name, definition) in &self.scopes {
            // Apply alias substitution to scope SQL content
            let resolved_sql = alias_manager.substitute_aliases(&definition.sql)?;

            resolved_manager.add_scope(scope_name.clone(), resolved_sql, definition.target.clone());
        }

        Ok(resolved_manager)
    }

    /// Apply scopes to SQL after alias substitution using SQL parser.
    /// This method is called AFTER alias_manager.substitute_aliases().
    pub fn apply_scopes_to_sql(&self, sql: &str) -> SynResult<String> {
        use sqlparser::ast::{OrderBy, OrderByKind, SetExpr, Statement};

        if !self.has_active_scopes() {
            return Ok(sql.to_owned());
        }

        // --- Parse SQL ---------------------------------------------------------

        let statement_opt = sqlx_data_parser::parse_sql(sql).map_err(core_error)?;
        let statement_arc = statement_opt.ok_or(sql_parse_error())?;
        let statement = (*statement_arc).clone();

        let Statement::Query(mut query) = statement else {
            return Err(syn_error("Only SELECT queries support scope application"));
        };

        let SetExpr::Select(select) = query.body.as_mut() else {
            return Err(syn_error("Only SELECT queries support scope application"));
        };

        let scopes = self.get_scopes_by_target();

        // --- WHERE scopes ------------------------------------------------------

        if let Some(where_scopes) = scopes.get(&ScopeTarget::Where) {
            let conds = self.build_where_conditions(where_scopes)?;
            if !conds.is_empty() {
                let combined = self.combine_where_conditions(select.selection.as_ref(), &conds)?;
                select.selection = Some(combined);
            }
        }

        // --- SELECT scopes -----------------------------------------------------

        if let Some(select_scopes) = scopes.get(&ScopeTarget::Select) {
            for scope in select_scopes {
                let item = self.parse_scope_as_select_item(&scope.sql)?;
                select.projection.push(item);
            }
        }

        // --- ORDER BY scopes ---------------------------------------------------

        if let Some(order_scopes) = scopes.get(&ScopeTarget::OrderBy) {
            let order_exprs = self.build_order_by_expressions(order_scopes)?;

            if !order_exprs.is_empty() {
                match &mut query.order_by {
                    Some(order) => match &mut order.kind {
                        OrderByKind::Expressions(existing) => {
                            existing.extend(order_exprs);
                        }
                        OrderByKind::All(_) => {
                            order.kind = OrderByKind::Expressions(order_exprs);
                        }
                    },
                    None => {
                        query.order_by = Some(OrderBy {
                            kind: OrderByKind::Expressions(order_exprs),
                            interpolate: None,
                        });
                    }
                }
            }
        }

        // --- FROM scopes -------------------------------------------------------
        if let Some(from_scopes) = scopes.get(&ScopeTarget::From) {
            for scope in from_scopes {
                // Parse "SELECT * FROM {scope.sql}"
                let dummy = format!("SELECT * FROM {}", scope.sql);
                let statement_opt = sqlx_data_parser::parse_sql(&dummy).map_err(core_error)?;
                let statement_arc = statement_opt.ok_or(sql_parse_error())?;
                let Statement::Query(query) = statement_arc.as_ref() else {
                    return Err(syn_error(format!(
                        "Invalid FROM scope (not a query): {}",
                        scope.sql
                    )));
                };
                let SetExpr::Select(parsed_select) = query.body.as_ref() else {
                    return Err(syn_error(format!(
                        "Invalid FROM scope (not a SELECT expr): {}",
                        scope.sql
                    )));
                };

                // Extend select.from with parsed from entries
                for table_with_joins in &parsed_select.from {
                    select.from.push(table_with_joins.clone());
                }
            }
        }

        // --- JOIN scopes -------------------------------------------------------
        if let Some(join_scopes) = scopes.get(&ScopeTarget::Join) {
            for scope in join_scopes {
                // We need a base table to attach joins in parsing; use a dummy table
                let dummy = format!("SELECT * FROM __dummy__ {}", scope.sql);
                let statement_opt = sqlx_data_parser::parse_sql(&dummy).map_err(core_error)?;
                let statement_arc = statement_opt.ok_or(sql_parse_error())?;
                let Statement::Query(query) = statement_arc.as_ref() else {
                    return Err(syn_error(format!(
                        "Invalid JOIN scope (not a query): {}",
                        scope.sql
                    )));
                };
                let SetExpr::Select(parsed_select) = query.body.as_ref() else {
                    return Err(syn_error(format!(
                        "Invalid JOIN scope (not a SELECT expr): {}",
                        scope.sql
                    )));
                };

                // If parsed_select.from has entries, take their joins and attach appropriately.
                // Strategy: if current select.from is non-empty, append parsed joins to the last TableWithJoins;
                // otherwise, push the parsed TableWithJoins (which includes base relation + joins).
                if !parsed_select.from.is_empty() {
                    let parsed_twjs = &parsed_select.from[0];
                    if select.from.is_empty() {
                        // No existing FROM: push the whole parsed TableWithJoins
                        select.from.push(parsed_twjs.clone());
                    } else {
                        // Attach parsed joins to the last existing TableWithJoins
                        let last_idx = select.from.len() - 1;
                        // extend the joins vector
                        select.from[last_idx]
                            .joins
                            .extend(parsed_twjs.joins.clone());
                    }
                } else {
                    // If parser didn't produce a from entry, ignore (safe fail)
                }
            }
        }

        // --- GROUP BY scopes ---------------------------------------------------
        if let Some(group_scopes) = scopes.get(&ScopeTarget::GroupBy) {
            for scope in group_scopes {
                // Parse "SELECT * GROUP BY {scope.sql}"
                let dummy = format!("SELECT * GROUP BY {}", scope.sql);
                let statement_opt = sqlx_data_parser::parse_sql(&dummy).map_err(core_error)?;
                let statement_arc = statement_opt.ok_or(sql_parse_error())?;
                let Statement::Query(query) = statement_arc.as_ref() else {
                    return Err(syn_error(format!(
                        "Invalid GROUP BY scope (not a query): {}",
                        scope.sql
                    )));
                };
                let SetExpr::Select(parsed_select) = query.body.as_ref() else {
                    return Err(syn_error(format!(
                        "Invalid GROUP BY scope (not a SELECT expr): {}",
                        scope.sql
                    )));
                };

                // Extend group_by expressions

                match &parsed_select.group_by {
                    GroupByExpr::Expressions(exprs, modifiers) => match &mut select.group_by {
                        GroupByExpr::Expressions(dest_exprs, dest_modifiers) => {
                            for expr in exprs {
                                dest_exprs.push(expr.clone());
                            }
                            for m in modifiers {
                                dest_modifiers.push(m.clone());
                            }
                        }
                        _ => {
                            select.group_by =
                                GroupByExpr::Expressions(exprs.clone(), modifiers.clone());
                        }
                    },
                    GroupByExpr::All(modifiers) => match &mut select.group_by {
                        GroupByExpr::All(dest_modifiers) => {
                            for m in modifiers {
                                dest_modifiers.push(m.clone());
                            }
                        }
                        _ => {
                            select.group_by = GroupByExpr::All(modifiers.clone());
                        }
                    },
                }
            }
        }

        // --- HAVING scopes ----------------------------------------------------
        if let Some(having_scopes) = scopes.get(&ScopeTarget::Having) {
            // Build having expressions (similar to where)
            let mut result = Vec::new();
            for scope in having_scopes {
                // Ex: SELECT 1 HAVING <expr>
                let sql = format!("SELECT 1 HAVING {}", scope.sql);
                let statement_opt = sqlx_data_parser::parse_sql(&sql).map_err(core_error)?;
                let statement_arc = statement_opt.ok_or(sql_parse_error())?;

                let Statement::Query(query) = statement_arc.as_ref() else {
                    return Err(syn_error(format!(
                        "Scope '{}' is not a valid SELECT query",
                        scope.name
                    )));
                };

                let SetExpr::Select(parsed_select) = &query.body.as_ref() else {
                    return Err(syn_error(format!(
                        "Scope '{}' is not a SELECT expression",
                        scope.name
                    )));
                };

                let Some(having_expr) = &parsed_select.having else {
                    return Err(syn_error(format!(
                        "Scope '{}' produced no HAVING condition",
                        scope.name
                    )));
                };

                result.push(having_expr.clone());
            }

            if !result.is_empty() {
                // Combine existing having with AND using same helper
                let combined = self.combine_where_conditions(select.having.as_ref(), &result)?;
                select.having = Some(combined);
            }
        }

        Ok(query.to_string())
    }

    /// Build WHERE conditions from scope definitions
    fn build_where_conditions(
        &self,
        where_scopes: &[&ScopeDefinition],
    ) -> SynResult<Vec<sqlparser::ast::Expr>> {
        use sqlparser::ast::{SetExpr, Statement};

        let mut result = Vec::new();

        for scope in where_scopes {
            // Ex: SELECT 1 WHERE customer_id = $1
            let sql = format!("SELECT 1 WHERE {}", scope.sql);

            // Parse SQL
            let statement_opt = sqlx_data_parser::parse_sql(&sql).map_err(core_error)?;
            let statement_arc = statement_opt.ok_or(sql_parse_error())?;

            let Statement::Query(query) = statement_arc.as_ref() else {
                return Err(syn_error(format!(
                    "Scope '{}' is not a valid SELECT query",
                    scope.name
                )));
            };

            let SetExpr::Select(select) = &query.body.as_ref() else {
                return Err(syn_error(format!(
                    "Scope '{}' is not a SELECT expression",
                    scope.name
                )));
            };

            let Some(selection) = &select.selection else {
                return Err(syn_error(format!(
                    "Scope '{}' produced no WHERE condition",
                    scope.name
                )));
            };

            result.push(selection.clone());
        }

        Ok(result)
    }

    /// Combine existing WHERE with scope WHERE conditions
    fn combine_where_conditions(
        &self,
        existing_where: Option<&sqlparser::ast::Expr>,
        scope_conditions: &[sqlparser::ast::Expr],
    ) -> SynResult<sqlparser::ast::Expr> {
        use sqlparser::ast::{BinaryOperator, Expr};

        if scope_conditions.is_empty() {
            return existing_where
                .cloned()
                .ok_or(syn_error("No conditions to combine"));
        }

        // Start with scope conditions combined with AND
        let mut combined = scope_conditions[0].clone();
        for condition in scope_conditions.iter().skip(1) {
            combined = Expr::BinaryOp {
                left: Box::new(combined),
                op: BinaryOperator::And,
                right: Box::new(condition.clone()),
            };
        }

        // If there's an existing WHERE, combine with AND
        if let Some(existing) = existing_where {
            combined = Expr::BinaryOp {
                left: Box::new(existing.clone()),
                op: BinaryOperator::And,
                right: Box::new(combined),
            };
        }

        Ok(combined)
    }

    /// Parse scope SQL as a SELECT item for SELECT scopes
    fn parse_scope_as_select_item(&self, scope_sql: &str) -> SynResult<sqlparser::ast::SelectItem> {
        use sqlparser::ast::{SetExpr, Statement};

        let dummy_sql = format!("SELECT {}", scope_sql);

        let statement_opt = sqlx_data_parser::parse_sql(&dummy_sql).map_err(core_error)?;
        let statement_arc = statement_opt.ok_or(sql_parse_error())?;

        let Statement::Query(query) = statement_arc.as_ref() else {
            return Err(syn_error(format!(
                "Invalid SELECT scope SQL (not a query): {scope_sql}"
            )));
        };

        let SetExpr::Select(select) = query.body.as_ref() else {
            return Err(syn_error(format!(
                "Invalid SELECT scope SQL (not a SELECT expr): {scope_sql}"
            )));
        };

        let Some(item) = select.projection.first() else {
            return Err(syn_error(format!(
                "Invalid SELECT scope SQL (no projection): {scope_sql}"
            )));
        };

        Ok(item.clone())
    }

    /// Build ORDER BY expressions from scope definitions
    fn build_order_by_expressions(
        &self,
        order_scopes: &[&ScopeDefinition],
    ) -> SynResult<Vec<sqlparser::ast::OrderByExpr>> {
        use sqlparser::ast::{OrderByKind, Statement};

        let mut order_exprs = Vec::new();

        for scope in order_scopes {
            // Ex: SELECT 1 ORDER BY created_at DESC
            let dummy_sql = format!("SELECT 1 ORDER BY {}", scope.sql);

            let statement_opt = sqlx_data_parser::parse_sql(&dummy_sql).map_err(core_error)?;
            let statement_arc = statement_opt.ok_or(sql_parse_error())?;

            let Statement::Query(query) = statement_arc.as_ref() else {
                return Err(syn_error(format!(
                    "Invalid ORDER BY scope (not a query): {}",
                    scope.sql
                )));
            };

            let Some(order_by) = &query.order_by else {
                // ORDER BY produced nothing — silently ignore (same comportamento atual)
                continue;
            };

            match &order_by.kind {
                OrderByKind::Expressions(exprs) => {
                    order_exprs.extend(exprs.clone());
                }
                OrderByKind::All(_) => {
                    // ALL doesn't allow expression extraction — ignore
                }
            }
        }

        Ok(order_exprs)
    }
}

impl Default for ScopeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alias_system::AliasManager;

    fn create_test_alias_manager() -> AliasManager {
        let mut manager = AliasManager::new();
        manager.add_alias("user_table".to_string(), "users".to_string());
        manager.add_alias("active_filter".to_string(), "active = 1".to_string());
        manager
    }

    #[test]
    fn test_add_scope_with_default_target() {
        let mut manager = ScopeManager::new();
        manager.add_scope(
            "tenantable".to_string(),
            "customer_id = $1".to_string(),
            ScopeTarget::Where,
        );

        let scope_names = manager.get_scope_names();
        assert!(scope_names.contains(&"tenantable".to_string()));

        let active_scopes = manager.get_active_scopes();
        assert_eq!(active_scopes["tenantable"].target, ScopeTarget::Where);
        assert_eq!(active_scopes["tenantable"].sql, "customer_id = $1");
    }

    #[test]
    fn test_add_scope_with_custom_target() {
        let mut manager = ScopeManager::new();
        manager.add_scope(
            "mask_email".to_string(),
            "REPLACE(email, '@', '[at]') AS masked_email".to_string(),
            ScopeTarget::Select,
        );

        let active_scopes = manager.get_active_scopes();
        assert_eq!(active_scopes["mask_email"].target, ScopeTarget::Select);
        assert_eq!(
            active_scopes["mask_email"].sql,
            "REPLACE(email, '@', '[at]') AS masked_email"
        );
    }

    #[test]
    fn test_scope_target_from_string() {
        assert_eq!(
            ScopeTarget::from_string("select").unwrap(),
            ScopeTarget::Select
        );
        assert_eq!(
            ScopeTarget::from_string("where").unwrap(),
            ScopeTarget::Where
        );
        assert_eq!(ScopeTarget::from_string("join").unwrap(), ScopeTarget::Join);
        assert_eq!(
            ScopeTarget::from_string("order_by").unwrap(),
            ScopeTarget::OrderBy
        );
        assert_eq!(
            ScopeTarget::from_string("group_by").unwrap(),
            ScopeTarget::GroupBy
        );

        assert!(ScopeTarget::from_string("invalid").is_err());
    }

    #[test]
    fn test_scope_ignore() {
        let mut manager = ScopeManager::new();
        manager.add_scope(
            "tenantable".to_string(),
            "customer_id = $1".to_string(),
            ScopeTarget::Where,
        );
        manager.add_scope(
            "versioned".to_string(),
            "deleted_at IS NULL".to_string(),
            ScopeTarget::Where,
        );
        manager.add_ignored_scope("versioned".to_string());

        let active_scopes = manager.get_active_scopes();
        assert_eq!(active_scopes.len(), 1);
        assert!(active_scopes.contains_key("tenantable"));
        assert!(!active_scopes.contains_key("versioned"));

        let ignored = manager.get_ignored_scope_names();
        assert!(ignored.contains(&"versioned".to_string()));
    }

    #[test]
    fn test_scopes_grouped_by_target() {
        let mut manager = ScopeManager::new();
        manager.add_scope(
            "tenantable".to_string(),
            "customer_id = $1".to_string(),
            ScopeTarget::Where,
        );
        manager.add_scope(
            "ownable".to_string(),
            "user_id = $1".to_string(),
            ScopeTarget::Where,
        );
        manager.add_scope(
            "mask_email".to_string(),
            "REPLACE(email, '@', '[at]') AS masked_email".to_string(),
            ScopeTarget::Select,
        );

        let grouped = manager.get_scopes_by_target();

        assert_eq!(grouped[&ScopeTarget::Where].len(), 2);
        assert_eq!(grouped[&ScopeTarget::Select].len(), 1);

        let where_scopes: Vec<&str> = grouped[&ScopeTarget::Where]
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(where_scopes.contains(&"tenantable"));
        assert!(where_scopes.contains(&"ownable"));
    }

    #[test]
    fn test_serialization() {
        let mut manager = ScopeManager::new();
        manager.add_scope(
            "tenantable".to_string(),
            "scope_tenantable".to_string(),
            ScopeTarget::Where,
        );
        manager.add_scope(
            "mask_email".to_string(),
            "scope_mask_email".to_string(),
            ScopeTarget::Select,
        );
        manager.add_ignored_scope("versioned".to_string());

        let serialized = manager.serialize_for_injection();
        assert!(serialized.contains("scopes:"));
        assert!(serialized.contains("tenantable=scope_tenantable:where"));
        assert!(serialized.contains("mask_email=scope_mask_email:select"));
        assert!(serialized.contains("ignore:versioned"));
    }

    #[test]
    fn test_deserialization() {
        let serialized = "scopes:tenantable=scope_tenantable:where;mask_email=scope_mask_email:select|ignore:versioned";
        let manager = ScopeManager::deserialize_from_injection(serialized).unwrap();

        let scope_names = manager.get_scope_names();
        assert!(scope_names.contains(&"tenantable".to_string()));
        assert!(scope_names.contains(&"mask_email".to_string()));

        let active_scopes = manager.get_active_scopes();
        assert_eq!(active_scopes["tenantable"].target, ScopeTarget::Where);
        assert_eq!(active_scopes["mask_email"].target, ScopeTarget::Select);

        let ignored = manager.get_ignored_scope_names();
        assert!(ignored.contains(&"versioned".to_string()));
    }

    #[test]
    fn test_apply_scopes_to_sql() {
        let mut scope_manager = ScopeManager::new();

        scope_manager.add_scope(
            "tenantable".to_string(),
            "customer_id = $1".to_string(),
            ScopeTarget::Where,
        );
        scope_manager.add_scope(
            "mask_email".to_string(),
            "REPLACE(email, '@', '[at]') AS masked_email".to_string(),
            ScopeTarget::Select,
        );

        let original_sql = "SELECT * FROM users WHERE age > 18";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Check that WHERE conditions are combined with AND
        assert!(result.contains("age > 18")); // Original condition
        assert!(result.contains("customer_id = $1")); // Scope condition
        assert!(result.contains("AND")); // Combined with AND

        // Check that SELECT scope is added
        assert!(result.contains("REPLACE(email, '@', '[at]') AS masked_email"));
    }

    #[test]
    fn test_substitute_scope_aliases() {
        let alias_manager = create_test_alias_manager();
        let mut scope_manager = ScopeManager::new();

        // Add scopes with aliases that need substitution
        scope_manager.add_scope(
            "table_filter".to_string(),
            "{{user_table}}.status = 'active'".to_string(),
            ScopeTarget::Where,
        );
        scope_manager.add_scope(
            "complex_filter".to_string(),
            "{{user_table}}.{{active_filter}} AND age > 18".to_string(),
            ScopeTarget::Where,
        );

        // Apply alias substitution
        let resolved_manager = scope_manager
            .substitute_scope_aliases(&alias_manager)
            .unwrap();

        let active_scopes = resolved_manager.get_active_scopes();

        // Check that aliases were substituted in scope SQL
        assert_eq!(active_scopes["table_filter"].sql, "users.status = 'active'");
        assert_eq!(
            active_scopes["complex_filter"].sql,
            "users.active = 1 AND age > 18"
        );

        // Verify targets are preserved
        assert_eq!(active_scopes["table_filter"].target, ScopeTarget::Where);
        assert_eq!(active_scopes["complex_filter"].target, ScopeTarget::Where);
    }

    #[test]
    fn test_apply_scopes_with_sql_parser() {
        let mut scope_manager = ScopeManager::new();

        // Add scopes with different targets
        scope_manager.add_scope(
            "age_filter".to_string(),
            "age > 18".to_string(),
            ScopeTarget::Where,
        );
        scope_manager.add_scope(
            "status_filter".to_string(),
            "status = 'active'".to_string(),
            ScopeTarget::Where,
        );
        scope_manager.add_scope(
            "name_order".to_string(),
            "name ASC".to_string(),
            ScopeTarget::OrderBy,
        );

        let original_sql = "SELECT id, name FROM users WHERE age > 0";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Check that WHERE conditions are combined with AND
        assert!(result.contains("age > 0")); // Original condition
        assert!(result.contains("age > 18")); // Scope condition
        assert!(result.contains("status = 'active'")); // Scope condition
        assert!(result.contains("AND")); // Combined with AND

        // Check that ORDER BY is added
        assert!(result.contains("ORDER BY"));
        assert!(result.contains("name ASC"));
    }

    #[test]
    fn test_apply_where_scopes_only() {
        let mut scope_manager = ScopeManager::new();

        // Add only WHERE scopes
        scope_manager.add_scope(
            "tenant_filter".to_string(),
            "tenant_id = 123".to_string(),
            ScopeTarget::Where,
        );
        scope_manager.add_scope(
            "active_filter".to_string(),
            "active = true".to_string(),
            ScopeTarget::Where,
        );

        let original_sql = "SELECT * FROM users";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Should add WHERE clause with scope conditions
        assert!(result.contains("WHERE"));
        assert!(result.contains("tenant_id = 123"));
        assert!(result.contains("active = true"));
        assert!(result.contains("AND")); // Multiple conditions combined
    }

    #[test]
    fn test_apply_order_by_scopes_only() {
        let mut scope_manager = ScopeManager::new();

        // Add only ORDER BY scope
        scope_manager.add_scope(
            "created_order".to_string(),
            "created_at DESC".to_string(),
            ScopeTarget::OrderBy,
        );

        let original_sql = "SELECT id, name FROM users WHERE id > 0";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Should add ORDER BY clause
        assert!(result.contains("ORDER BY"));
        assert!(result.contains("created_at DESC"));
        // Should preserve original WHERE
        assert!(result.contains("WHERE id > 0"));
    }

    #[test]
    fn test_apply_no_scopes() {
        let scope_manager = ScopeManager::new(); // Empty scope manager

        let original_sql = "SELECT id, name FROM users WHERE id > 0";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        // Should return original SQL unchanged
        assert_eq!(result, original_sql);
    }

    #[test]
    fn test_apply_group_by_scopes() {
        let mut scope_manager = ScopeManager::new();

        // Add GROUP BY scope
        scope_manager.add_scope(
            "group_department".to_string(),
            "department".to_string(),
            ScopeTarget::GroupBy,
        );
        scope_manager.add_scope(
            "group_status".to_string(),
            "status".to_string(),
            ScopeTarget::GroupBy,
        );

        let original_sql = "SELECT department, COUNT(*) FROM employees";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Should add GROUP BY clause
        assert!(result.contains("GROUP BY"));
        assert!(result.contains("department"));
        assert!(result.contains("status"));
    }

    #[test]
    fn test_apply_having_scopes() {
        let mut scope_manager = ScopeManager::new();

        // Add HAVING scope
        scope_manager.add_scope(
            "count_filter".to_string(),
            "COUNT(*) > 5".to_string(),
            ScopeTarget::Having,
        );
        scope_manager.add_scope(
            "avg_filter".to_string(),
            "AVG(salary) > 50000".to_string(),
            ScopeTarget::Having,
        );

        let original_sql = "SELECT department, COUNT(*) FROM employees GROUP BY department";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Should add HAVING clause
        assert!(result.contains("HAVING"));
        assert!(result.contains("COUNT(*) > 5"));
        assert!(result.contains("AVG(salary) > 50000"));
        assert!(result.contains("AND")); // Multiple conditions combined
    }

    #[test]
    fn test_apply_from_scopes() {
        let mut scope_manager = ScopeManager::new();

        // Add FROM scope
        scope_manager.add_scope(
            "audit_table".to_string(),
            "audit_logs al".to_string(),
            ScopeTarget::From,
        );

        let original_sql = "SELECT * FROM users";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Should add additional FROM table
        assert!(result.contains("FROM users"));
        assert!(result.contains("audit_logs"));
    }

    #[test]
    fn test_apply_join_scopes() {
        let mut scope_manager = ScopeManager::new();

        // Add JOIN scope
        scope_manager.add_scope(
            "profile_join".to_string(),
            "LEFT JOIN profiles p ON u.id = p.user_id".to_string(),
            ScopeTarget::Join,
        );
        scope_manager.add_scope(
            "department_join".to_string(),
            "INNER JOIN departments d ON u.dept_id = d.id".to_string(),
            ScopeTarget::Join,
        );

        let original_sql = "SELECT u.id, u.name FROM users u";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Should add JOIN clauses
        assert!(result.contains("LEFT JOIN profiles"));
        assert!(result.contains("ON u.id = p.user_id"));
        assert!(result.contains("INNER JOIN departments"));
        assert!(result.contains("ON u.dept_id = d.id"));
    }

    #[test]
    fn test_apply_complex_mixed_scopes() {
        let mut scope_manager = ScopeManager::new();

        // Add multiple different scope types
        scope_manager.add_scope(
            "tenant_filter".to_string(),
            "tenant_id = 123".to_string(),
            ScopeTarget::Where,
        );
        scope_manager.add_scope(
            "profile_join".to_string(),
            "LEFT JOIN profiles p ON u.id = p.user_id".to_string(),
            ScopeTarget::Join,
        );
        scope_manager.add_scope(
            "masked_email".to_string(),
            "REPLACE(u.email, '@', '[at]') AS masked_email".to_string(),
            ScopeTarget::Select,
        );
        scope_manager.add_scope(
            "group_dept".to_string(),
            "department".to_string(),
            ScopeTarget::GroupBy,
        );
        scope_manager.add_scope(
            "count_having".to_string(),
            "COUNT(*) > 1".to_string(),
            ScopeTarget::Having,
        );
        scope_manager.add_scope(
            "created_order".to_string(),
            "created_at DESC".to_string(),
            ScopeTarget::OrderBy,
        );

        let original_sql = "SELECT u.id, u.name FROM users u WHERE u.active = 1";
        let result = scope_manager.apply_scopes_to_sql(original_sql).unwrap();

        println!("Original SQL: {}", original_sql);
        println!("Modified SQL: {}", result);

        // Verify all scope types are applied
        assert!(result.contains("tenant_id = 123")); // WHERE
        assert!(result.contains("LEFT JOIN profiles")); // JOIN
        assert!(result.contains("REPLACE(u.email, '@', '[at]')")); // SELECT
        assert!(result.contains("GROUP BY")); // GROUP BY
        assert!(result.contains("department")); // GROUP BY content
        assert!(result.contains("HAVING")); // HAVING
        assert!(result.contains("COUNT(*) > 1")); // HAVING content
        assert!(result.contains("ORDER BY")); // ORDER BY
        assert!(result.contains("created_at DESC")); // ORDER BY content
        assert!(result.contains("u.active = 1")); // Original WHERE preserved
    }
}
