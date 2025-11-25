/// Source of SQL content
#[derive(Clone, Debug)]
pub enum SqlSource {
    Inline(String), // SQL string directly in the attribute
    File(String),   // File path to SQL file
}

impl SqlSource {
    /// Get the SQL content as string (reads file if needed)
    /// This always returns the actual SQL text, never file paths
    pub fn resolve_content(self) -> syn::Result<String> {
        match self {
            SqlSource::Inline(sql) => Ok(sql),
            SqlSource::File(file_path) => Self::read_file_content(&file_path),
        }
    }

    /// Centralized file reading logic with security validation
    fn read_file_content(file_path: &str) -> syn::Result<String> {
        // Security validation: prevent path traversal attacks
        Self::validate_file_path(file_path)?;

        let content = if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let full_path = std::path::Path::new(&manifest_dir).join(file_path);
            std::fs::read_to_string(full_path).or_else(|_| std::fs::read_to_string(file_path)) // Fallback to relative path
        } else {
            std::fs::read_to_string(file_path) // No manifest dir available
        }
        .map_err(|e| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("Failed to read SQL file '{}': {}", file_path, e),
            )
        })?;

        Ok(content)
    }

    /// Validate file path for security - prevent path traversal attacks
    fn validate_file_path(file_path: &str) -> syn::Result<()> {
        // Check for path traversal attempts
        if file_path.contains("..") {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "File path cannot contain '..' (path traversal prevented for security)",
            ));
        }

        // Check for absolute paths
        if file_path.starts_with('/') || file_path.starts_with('\\') {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "File path must be relative (absolute paths not allowed for security)",
            ));
        }

        // Windows drive letters
        if file_path.len() >= 2 && file_path.chars().nth(1) == Some(':') {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "File path cannot contain drive letters (absolute paths not allowed for security)",
            ));
        }

        // Restrict to .sql files only
        if !file_path.ends_with(".sql") {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Only .sql files are allowed for security reasons",
            ));
        }

        // Check for null bytes (security measure)
        if file_path.contains('\0') {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "File path cannot contain null bytes",
            ));
        }

        Ok(())
    }
}

/// Represents the different kinds of Rust types we can work with
#[derive(Debug, Clone, PartialEq)]
pub enum ReturnType {
    /// Scalar types like i32, String, bool
    Scalar { name: syn::Ident },

    /// Struct types that can be mapped from database rows
    Struct { name: syn::Ident },

    /// Tuple types like (i32, String, bool)
    Tuple { elements: Vec<ReturnType> },

    /// Vec<T> types for multiple results
    Vec { element_type: Box<ReturnType> },

    /// Option<T> types for nullable results
    Option { inner_type: Box<ReturnType> },

    /// Result<T, E> types (standard return type)
    Result {
        ok_type: Box<ReturnType>,
        err_type: Box<ReturnType>,
    },

    /// Stream<Item = T> types for streaming results
    Stream { item_type: Box<ReturnType> },

    /// Unit type ()
    Unit,

    /// Unknown or unsupported types
    Unknown { name: String },
}

/// Represents the SQLx query strategy to use
#[derive(Debug, Clone, PartialEq)]
pub enum QueryType {
    /// Use query_as! for struct mapping
    QueryAs,
    /// Use query_scalar! for scalar values
    QueryScalar,
    /// Use raw query! for complex cases
    Query,
}

/// How to fetch results from the database
#[derive(Debug, Clone, PartialEq)]
pub enum FetchMethod {
    Execute,       // execute() - no results expected
    FetchOne,      // fetch_one() - expects exactly one result
    FetchAll,      // fetch_all() - returns Vec of results
    FetchOptional, // fetch_optional() - returns Option, no error if empty
    Fetch,         // fetch() - returns Stream for streaming results
}

impl ReturnType {}
