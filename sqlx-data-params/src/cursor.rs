use crate::FilterValue;
use crate::{IntoParams, Params};

#[cfg(feature = "json")]
use serde::{Deserialize, Serialize};

// ================================================================================================
// CORE TYPES
// ================================================================================================

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]
pub struct CursorEntry {
    pub value: CursorValue,
}

/// Client-facing cursor data - contains only the serializable data that goes to the client
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]
#[derive(Default)]
pub struct Cursor {
    #[cfg_attr(feature = "json", serde(skip_serializing_if = "Vec::is_empty"))]
    pub entries: Vec<CursorEntry>,
    #[cfg_attr(feature = "json", serde(skip_serializing_if = "Option::is_none"))]
    pub version: Option<u8>,
    #[cfg_attr(feature = "json", serde(skip_serializing_if = "Option::is_none"))]
    pub fingerprint: Option<u64>,
}

/// Internal cursor params with metadata - contains the cursor data plus internal processing metadata
#[derive(Clone, Debug, PartialEq)]
#[derive(Default)]
pub struct CursorParams {
    /// Internal direction metadata
    pub direction: Option<CursorDirection>,
    /// After, Before, and decoded cursor data - used when building queries
    pub values: Vec<FilterValue>,
    /// Optional error message if cursor processing failed
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CursorDirection {
    After,
    Before,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "json", serde(untagged))]
pub enum CursorValue {
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    String(String),
}

impl From<i64> for CursorValue {
    fn from(value: i64) -> Self {
        CursorValue::Int(value)
    }
}

impl From<u64> for CursorValue {
    fn from(value: u64) -> Self {
        CursorValue::UInt(value)
    }
}

impl From<f64> for CursorValue {
    fn from(value: f64) -> Self {
        CursorValue::Float(value)
    }
}

impl From<bool> for CursorValue {
    fn from(value: bool) -> Self {
        CursorValue::Bool(value)
    }
}

impl From<String> for CursorValue {
    fn from(value: String) -> Self {
        CursorValue::String(value)
    }
}

impl From<&str> for CursorValue {
    fn from(value: &str) -> Self {
        CursorValue::String(value.to_string())
    }
}

impl From<i32> for CursorValue {
    fn from(value: i32) -> Self {
        CursorValue::Int(value as i64)
    }
}

impl From<u32> for CursorValue {
    fn from(value: u32) -> Self {
        CursorValue::UInt(value as u64)
    }
}

impl From<f32> for CursorValue {
    fn from(value: f32) -> Self {
        CursorValue::Float(value as f64)
    }
}

pub type Result<T, E = CursorError> = ::std::result::Result<T, E>;

// ================================================================================================
// ERROR HANDLING
// ================================================================================================

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub type SqlxError = sqlx_data_integration::Error;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
enum CursorErrorKind {
    #[error(transparent)]
    #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
    Sqlx(#[from] SqlxError),

    #[error("Field '{0}' not allowed for cursor pagination")]
    InvalidField(String),

    #[error("Data is empty")]
    EmptyData,

    #[error("Encoding cursor failed: {0}")]
    EncodeError(String),

    #[error("Decoding cursor failed: {0}")]
    DecodeError(String),
}

#[derive(Debug)]
pub struct CursorError(CursorErrorKind);

impl CursorError {
    /// Create an InvalidField error with automatic type conversion based on features
    pub fn invalid_field(field: impl Into<String>) -> Self {
        Self(CursorErrorKind::InvalidField(field.into()))
    }

    pub fn empty_data() -> Self {
        Self(CursorErrorKind::EmptyData)
    }

    pub fn encode_error(msg: impl Into<String>) -> Self {
        Self(CursorErrorKind::EncodeError(msg.into()))
    }

    pub fn decode_error(msg: impl Into<String>) -> Self {
        Self(CursorErrorKind::DecodeError(msg.into()))
    }
}

// Convert CursorError to sqlx_data_integration::Error when database features are enabled
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
impl From<SqlxError> for CursorError {
    fn from(e: SqlxError) -> Self {
        Self(CursorErrorKind::Sqlx(e))
    }
}

impl std::fmt::Display for CursorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for CursorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
impl From<CursorError> for sqlx_data_integration::Error {
    fn from(err: CursorError) -> Self {
        match err.0 {
            CursorErrorKind::Sqlx(e) => e.into(),
            other => sqlx_data_integration::Error::Decode(other.into()),
        }
    }
}

// ================================================================================================
// IMPLEMENTATIONS
// ================================================================================================



impl Cursor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_multi(entries: Vec<CursorEntry>) -> Self {
        Self { entries, version: None, fingerprint: None }
    }

    pub fn and_field(mut self, value: impl Into<CursorValue>) -> Self {
        self.entries.push(CursorEntry {
            value: value.into(),
        });
        self
    }
}

impl CursorParams {
    pub fn new(value: FilterValue, direction: CursorDirection) -> Self {
        Self {
            values: vec![value],
            direction: Some(direction),
            error: None,
        }
    }

    pub fn from_values(values: Vec<FilterValue>, direction: CursorDirection) -> Self {
        Self {
            values,
            direction: Some(direction),
            error: None,
        }
    }

    pub fn with_error(direction: CursorDirection, error: impl Into<String>) -> Self {
        Self {
            values: vec![],
            direction: Some(direction),
            error: Some(error.into()),
        }
    }

    pub fn and_field(mut self, value: FilterValue) -> Self {
        self.values.push(value);
        self
    }

    /// Access to the cursor values
    pub fn values(&self) -> &[FilterValue] {
        &self.values
    }

    /// Check if cursor has values
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get the number of values
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if this cursor has an error
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// Get the error message if any
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Generate cursor from a specific item in the data
    fn generate_cursor<T: CursorSecureExtract>(
        data: &[T],
        has_more: bool,
        sorting_params: &crate::sort::SortingParams,
        get_item: impl FnOnce(&[T]) -> Option<&T>,
    ) -> Result<Option<Cursor>> {
        if !has_more || data.is_empty() {
            return Ok(None);
        }

        // Extract field names from sorting parameters
        let fields: Vec<String> = sorting_params
            .sorts()
            .iter()
            .map(|s| s.field.clone())
            .collect();

        if fields.is_empty() {
            return Err(CursorError::invalid_field(
                "Cursor pagination requires ORDER BY fields",
            ));
        }

        let item = get_item(data).ok_or(CursorError::empty_data())?;

        let values = item.extract_whitelisted_fields(&fields)?;

        if values.len() != fields.len() {
            return Err(CursorError::invalid_field(
                "Cursor fields mismatch with sorting params",
            ));
        }

        let entries: Vec<CursorEntry> = values
            .into_iter()
            .map(|value| CursorEntry { value })
            .collect();

        Ok(Some(Cursor::new_multi(entries)))
    }

    /// Generate next cursor from the last item in data
    pub fn generate_next_cursor<T: CursorSecureExtract>(
        &self,
        data: &[T],
        has_next: bool,
        sorting_params: &crate::sort::SortingParams,
    ) -> Result<Option<String>> {
        let cursor = Self::generate_cursor(data, has_next, sorting_params, |data| data.last())?;
        match cursor {
            Some(c) => Ok(Some(T::encode(&c)?)),
            None => Ok(None),
        }
    }

    /// Generate prev cursor from the first item in data
    pub fn generate_prev_cursor<T: CursorSecureExtract>(
        &self,
        data: &[T],
        has_prev: bool,
        sorting_params: &crate::sort::SortingParams,
    ) -> Result<Option<String>> {
        let cursor = Self::generate_cursor(data, has_prev, sorting_params, |data| data.first())?;
        match cursor {
            Some(c) => Ok(Some(T::encode(&c)?)),
            None => Ok(None),
        }
    }

    
}

// ================================================================================================
// SECURITY TRAITS
// ================================================================================================

/// **Security-First Cursor Field Whitelist Trait**
///
/// This trait enforces a whitelist-based security model for cursor pagination fields.
/// Implementors MUST explicitly whitelist each allowed field to prevent field injection attacks.
pub trait CursorSecureExtract {
    /// **SECURITY CRITICAL**: Extract values ONLY for explicitly whitelisted cursor fields.
    ///
    /// **THIS IS A SECURITY WHITELIST** - Only return values for fields you explicitly allow.
    /// **ALWAYS** return `Err` for any field not in your whitelist to prevent field injection.
    ///
    /// # Security Model
    ///
    /// This method acts as the primary defense against field injection attacks via cursor pagination.
    /// Even if malicious field names are injected through `from_encoded()` or other vectors,
    /// this whitelist ensures only safe, predefined fields can be accessed.
    ///
    /// # Implementation Requirements
    ///
    /// - **MUST** use explicit `match field.as_str()` with hardcoded field names
    /// - **MUST** return `Err` for the default case (`_`)
    /// - **NEVER** use dynamic field resolution or reflection
    /// - **ONLY** allow fields that are safe for cursor-based ordering
    ///
    /// # Example
    /// ```rust, ignore
    /// # use sqlx_data_params::{CursorSecureExtract, CursorValue, CursorError, SqlxError};
    /// type Result<T> = ::std::result::Result<T, SqlxError>;
    /// struct User {
    ///     id: i64,
    ///     name: String,
    ///     email: String,
    ///     password_hash: String, // ← NEVER include sensitive fields!
    /// }
    ///
    /// impl CursorSecureExtract for User {
    ///     #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
    ///     fn extract_whitelisted_fields(&self, fields: &[String]) -> Result<Vec<CursorValue>> {
    ///         let mut values = Vec::with_capacity(fields.len());
    ///         for field in fields {
    ///             // 🛡️ SECURITY WHITELIST: Only these fields are allowed
    ///             match field.as_str() {
    ///                 "id" => values.push(self.id.into()),           // ✅ Safe: Primary key
    ///                 "name" => values.push(self.name.clone().into()), // ✅ Safe: Public field
    ///                 "email" => values.push(self.email.clone().into()), // ✅ Safe: Public field
    ///                 // password_hash is NOT in whitelist - cannot be accessed via cursor
    ///                 _ => return Err(CursorError::invalid_field(field.clone()).into()), // 🚫 REJECT: All non-whitelisted fields
    ///             }
    ///         }
    ///         Ok(values)
    ///     }
    ///
    ///     #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
    ///     fn extract_whitelisted_fields(&self, fields: &[String]) -> Result<Vec<CursorValue>> {
    ///         let mut values = Vec::with_capacity(fields.len());
    ///         for field in fields {
    ///             // 🛡️ SECURITY WHITELIST: Only these fields are allowed
    ///             match field.as_str() {
    ///                 "id" => values.push(self.id.into()),           // ✅ Safe: Primary key
    ///                 "name" => values.push(self.name.clone().into()), // ✅ Safe: Public field
    ///                 "email" => values.push(self.email.clone().into()), // ✅ Safe: Public field
    ///                 // password_hash is NOT in whitelist - cannot be accessed via cursor
    ///                 _ => return Err(CursorError::invalid_field(field.clone())), // 🚫 REJECT: All non-whitelisted fields
    ///             }
    ///         }
    ///         Ok(values)
    ///     }
    /// }
    /// ```
    ///
    /// # Security Benefits
    ///
    /// - **Field Injection Prevention**: Malicious fields from `from_encoded()` are rejected
    /// - **Data Exposure Control**: Sensitive fields cannot be accessed via cursor pagination
    /// - **Explicit Security Model**: Developers must consciously choose which fields to expose
    /// - **Defense in Depth**: Multiple layers protect against various attack vectors
    #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
    fn extract_whitelisted_fields(
        &self,
        fields: &[String],
    ) -> Result<Vec<CursorValue>, sqlx_data_integration::Error>;

    #[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
    fn extract_whitelisted_fields(&self, fields: &[String]) -> Result<Vec<CursorValue>>;

    /// Encode cursor to string token
    ///
    /// Example implementation:
    /// ```rust,ignore
    /// fn encode(cursor: &Cursor) -> Result<String, sqlx_data_integration::Error> {
    ///     use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as BASE64};
    ///     let json_bytes = serde_json::to_vec(&cursor)
    ///         .map_err(|e| CursorError::encode_error(format!("JSON serialization failed: {}", e)))?;
    ///     Ok(BASE64.encode(json_bytes))
    /// }
    /// ```
    #[cfg(feature = "json")]
    fn encode(cursor: &Cursor) -> Result<String, sqlx_data_integration::Error>;

    /// Encode cursor to string token (JSON feature disabled)
    #[cfg(not(feature = "json"))]
    fn encode(_cursor: &Cursor) -> Result<String>;

    /// Decode string token to FilterValue vector
    ///
    /// Example implementation:
    /// ```rust,ignore
    /// fn decode(encoded: &str) -> Result<Vec<FilterValue>> {
    ///     use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as BASE64};
    ///     let bytes = BASE64
    ///         .decode(encoded)
    ///         .map_err(|e| CursorError::decode_error(format!("Base64 decode failed: {}", e)))?;
    ///
    ///     let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|e| {
    ///         CursorError::decode_error(format!("JSON deserialization failed: {}", e))
    ///     })?;
    ///
    ///     // Convert CursorValue to FilterValue
    ///     let filter_values: Vec<FilterValue> = cursor.entries.into_iter().map(|entry| {
    ///         match entry.value {
    ///             CursorValue::Int(v) => FilterValue::Int(v),
    ///             CursorValue::UInt(v) => FilterValue::UInt(v),
    ///             CursorValue::Float(v) => FilterValue::Float(v),
    ///             CursorValue::Bool(v) => FilterValue::Bool(v),
    ///             CursorValue::String(v) => v.into(), // Or Whatever conversion is appropriate
    ///         }
    ///     }).collect();
    ///
    ///     Ok(filter_values)
    /// }
    /// ```
    #[cfg(feature = "json")]
    fn decode(encoded: &str) -> Result<Vec<FilterValue>, sqlx_data_integration::Error>;

    /// Decode string token to FilterValue vector (JSON feature disabled)
    #[cfg(not(feature = "json"))]
    fn decode(_encoded: &str) -> Result<Vec<FilterValue>>;
}

// ================================================================================================
// PARAMS INTEGRATION
// ================================================================================================

impl IntoParams for CursorParams {
    fn into_params(self) -> Params {
        let per_page = 20; // Default value
        let pagination = crate::pagination::Pagination::Cursor(self);
        Params {
            filters: None,
            search: None,
            sort_by: None,
            pagination: Some(pagination),
            limit: Some(crate::pagination::LimitParam(per_page)),
            offset: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_builder_pattern() {
        let cursor =
            CursorParams::new(FilterValue::String("alice".into()), CursorDirection::Before)
                .and_field(FilterValue::Int(25))
                .and_field(FilterValue::Float(99.5));

        assert_eq!(cursor.len(), 3);
        assert_eq!(cursor.direction.unwrap(), CursorDirection::Before);
    }

    #[test]
    fn test_cursor_state_detection() {
        let cursor_with_data = CursorParams::new(FilterValue::Int(123), CursorDirection::After);
        assert!(!cursor_with_data.is_empty());
        assert!(!cursor_with_data.has_error());

        let cursor_with_error = CursorParams::with_error(CursorDirection::After, "decode failed");
        assert!(cursor_with_error.is_empty());
        assert!(cursor_with_error.has_error());
    }

    #[test]
    fn test_cursor_values() {
        let cursor = CursorParams::new(FilterValue::Int(123), CursorDirection::After)
            .and_field(FilterValue::String("test".into()));

        assert_eq!(cursor.len(), 2);
        assert_eq!(cursor.values().len(), 2);
        assert_eq!(cursor.direction, Some(CursorDirection::After));
    }

    #[test]
    fn test_error_workflow() {
        let cursor_ok = CursorParams::new(FilterValue::Int(123), CursorDirection::After);
        assert!(!cursor_ok.has_error());
        assert_eq!(cursor_ok.error(), None);

        let cursor_err = CursorParams::with_error(CursorDirection::Before, "Invalid token");
        assert!(cursor_err.has_error());
        assert_eq!(cursor_err.error(), Some("Invalid token"));
    }
}
