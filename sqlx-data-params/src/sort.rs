use crate::{IntoParams, Params};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum NullOrdering {
    First,
    Last,
    #[default]
    Default,
}

/// Internal enum to track whether a sort field was added safely or unsafely
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SortSafety {
    #[default]
    Safe, // Added via asc() or desc() with compile-time validation
    Unsafe, // Added via asc_unsafe() or desc_unsafe() with runtime validation needed
}

impl SortDirection {
    /// Returns true if this is ASC direction
    pub fn is_asc(self) -> bool {
        matches!(self, Self::Asc)
    }

    /// Returns true if this is DESC direction
    pub fn is_desc(self) -> bool {
        matches!(self, Self::Desc)
    }

    pub fn flip(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}


impl NullOrdering {
    /// Returns Some(true) for First, Some(false) for Last, None for Default
    pub fn as_bool_option(self) -> Option<bool> {
        match self {
            Self::First => Some(true),
            Self::Last => Some(false),
            Self::Default => None,
        }
    }

    /// Returns true if nulls should come first
    pub fn is_first(self) -> bool {
        matches!(self, Self::First)
    }

    /// Returns true if nulls should come last
    pub fn is_last(self) -> bool {
        matches!(self, Self::Last)
    }

    /// Returns true if using default null ordering
    pub fn is_default(self) -> bool {
        matches!(self, Self::Default)
    }
}


#[derive(Debug, Clone)]
pub struct Sort {
    pub field: String,
    pub direction: SortDirection,
    pub nulls: NullOrdering,
    safety_type: SortSafety,
}

impl Sort {
    pub fn new(field: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            field: field.into(),
            direction,
            nulls: NullOrdering::default(),
            safety_type: SortSafety::Safe,
        }
    }

    pub fn new_unsafe(field: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            field: field.into(),
            direction,
            nulls: NullOrdering::default(),
            safety_type: SortSafety::Unsafe,
        }
    }

    /// Returns true if this sort was added via unsafe methods
    pub fn is_unsafe(&self) -> bool {
        matches!(self.safety_type, SortSafety::Unsafe)
    }

    pub fn asc(field: impl Into<String>) -> Self {
        Self::new(field, SortDirection::Asc)
    }

    pub fn desc(field: impl Into<String>) -> Self {
        Self::new(field, SortDirection::Desc)
    }

    pub fn asc_unsafe(field: impl Into<String>) -> Self {
        Self::new_unsafe(field, SortDirection::Asc)
    }

    pub fn desc_unsafe(field: impl Into<String>) -> Self {
        Self::new_unsafe(field, SortDirection::Desc)
    }

    pub fn nulls_first(mut self) -> Self {
        self.nulls = NullOrdering::First;
        self
    }

    pub fn nulls_last(mut self) -> Self {
        self.nulls = NullOrdering::Last;
        self
    }

    pub fn nulls_default(mut self) -> Self {
        self.nulls = NullOrdering::Default;
        self
    }

    /// Returns true if this is ASC direction
    pub fn is_asc(&self) -> bool {
        self.direction.is_asc()
    }

    /// Returns true if this is DESC direction
    pub fn is_desc(&self) -> bool {
        self.direction.is_desc()
    }

    /// Returns nulls ordering as Option<bool>
    pub fn nulls_as_bool(&self) -> Option<bool> {
        self.nulls.as_bool_option()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SortingParams {
    sorts: Vec<Sort>,
    unsafe_fields: Vec<String>,
    allowed_columns: Option<&'static [&'static str]>,
}

impl SortingParams {
    pub fn new() -> Self {
        Self {
            sorts: Vec::new(),
            unsafe_fields: Vec::new(),
            allowed_columns: None,
        }
    }

    /// Get readonly access to sorts
    pub fn sorts(&self) -> &[Sort] {
        &self.sorts
    }

    /// Check if there are any unsafe fields that need validation
    pub fn has_unsafe_fields(&self) -> bool {
        !self.unsafe_fields.is_empty()
    }

    /// Set allowed columns for unsafe field validation
    pub fn with_allowed_columns(mut self, columns: &'static [&'static str]) -> Self {
        self.allowed_columns = Some(columns);
        self
    }

    /// Validate all unsafe fields against the whitelist
    pub fn validate_fields(&self) -> Result<(), String> {
        if !self.has_unsafe_fields() {
            return Ok(());
        }

        let allowed = match self.allowed_columns {
            Some(cols) => cols,
            None => {
                return Err("Unsafe fields present but no allowed_columns specified".to_string());
            }
        };

        // SECURITY NOTE:
        // This uses slice::contains, which performs an exact, case-sensitive match.
        // It does NOT perform substring matching.
        // Do NOT replace with str::contains, starts_with, regex, etc. (SQL injection risk)
        for field in &self.unsafe_fields {
            if !allowed.contains(&field.as_str()) {
                return Err(format!("Field '{}' is not in allowed columns list", field));
            }
        }

        Ok(())
    }

    pub fn push(mut self, sort: Sort) -> Self {
        // Track unsafe fields
        if sort.is_unsafe() {
            self.unsafe_fields.push(sort.field.clone());
        }
        self.sorts.push(sort);
        self
    }

    pub fn sort_by(mut self, field: impl Into<String>, direction: SortDirection) -> Self {
        self.sorts.push(Sort::new(field, direction));
        self
    }

    pub fn asc(self, field: impl Into<String>) -> Self {
        self.sort_by(field, SortDirection::Asc)
    }

    pub fn desc(self, field: impl Into<String>) -> Self {
        self.sort_by(field, SortDirection::Desc)
    }

    pub fn is_empty(&self) -> bool {
        self.sorts.is_empty()
    }

    /// Apply NULLS FIRST to the last added sort
    pub fn apply_nulls_first(mut self) -> Self {
        if let Some(last_sort) = self.sorts.last_mut() {
            last_sort.nulls = NullOrdering::First;
        }
        self
    }

    /// Apply NULLS LAST to the last added sort
    pub fn apply_nulls_last(mut self) -> Self {
        if let Some(last_sort) = self.sorts.last_mut() {
            last_sort.nulls = NullOrdering::Last;
        }
        self
    }

    /// Apply default null ordering to the last added sort
    pub fn apply_nulls_default(mut self) -> Self {
        if let Some(last_sort) = self.sorts.last_mut() {
            last_sort.nulls = NullOrdering::Default;
        }
        self
    }

    /// Combine with another SortingParams, extending the sorts list
    pub fn extend_with(mut self, other: SortingParams) -> Self {
        self.sorts.extend(other.sorts);
        self.unsafe_fields.extend(other.unsafe_fields);
        // Preserve allowed_columns from self if present, otherwise use other's
        if self.allowed_columns.is_none() {
            self.allowed_columns = other.allowed_columns;
        }
        self
    }
}

impl IntoParams for SortingParams {
    fn into_params(self) -> Params {
        Params {
            filters: None,
            search: None,
            sort_by: Some(self),
            pagination: None,
            limit: None,
            offset: None,
        }
    }
}
