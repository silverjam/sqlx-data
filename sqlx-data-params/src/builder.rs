//! High-level, type-safe SQL parameter builder with
//! support for filtering, sorting and pagination
//! across PostgreSQL, MySQL and SQLite.

use crate::filter::Filter;
use crate::sort::Sort;
use crate::{
    CursorDirection, CursorParams, FilterOperator, FilterParams, FilterValue,
    IntoParams, SearchParams, SortingParams, params::Params,
};
use std::marker::PhantomData;

//
// ========================
//   PARAMS ROOT BUILDER
// ========================
//
#[derive(Debug)]
pub struct ParamsBuilder {
    params: Params,
}

impl Default for ParamsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ParamsBuilder {
    pub fn new() -> Self {
        Self {
            params: Params::default(),
        }
    }

    //
    // -------------- SORT BUILDER --------------
    //
    pub fn sort(self) -> SortBuilder<Self, Safe> {
        SortBuilder::with_parent(self)
    }

    //
    // -------------- FILTER BUILDER --------------
    //
    pub fn filter(self) -> FilterBuilder<Self> {
        FilterBuilder::with_parent(self)
    }

    //
    // -------------- SEARCH BUILDER --------------
    //
    pub fn search(self) -> SearchBuilder<Self> {
        SearchBuilder::with_parent(self)
    }

    //
    // -------------- SERIAL BUILDER --------------
    //
    pub fn serial(self) -> SerialBuilder<Self> {
        SerialBuilder::with_parent(self)
    }

    //
    // -------------- SLICE BUILDER --------------
    //
    pub fn slice(self) -> SliceBuilder<Self> {
        SliceBuilder::with_parent(self)
    }

    //
    // -------------- CURSOR BUILDER --------------
    //
    pub fn cursor(self) -> CursorBuilder<Self, Initial> {
        CursorBuilder::with_parent(self)
    }

    //
    // -------------- LIMIT/OFFSET --------------
    //
    pub fn limit(mut self, limit: u32) -> Self {
        self.params.limit = Some(crate::pagination::LimitParam(limit));
        self
    }

    pub fn offset(mut self, offset: u32) -> Self {
        self.params.offset = Some(crate::pagination::OffsetParam(offset));
        self
    }

    //
    // -------------- FINAL BUILD --------------
    //
    pub fn build(self) -> Params {
        self.params
    }
}

//
// ========================
//       SORT BUILDER
// ========================
//
// Security states for typestate pattern
pub struct Safe;
pub struct Unsafe;

pub struct SortBuilder<P = (), S = Safe> {
    parent: Option<P>,
    sorts: SortingParams,
    allowed_columns: Option<&'static [&'static str]>,
    _security: PhantomData<S>,
}

impl Default for SortBuilder<(), Safe> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> std::fmt::Debug for SortBuilder<(), S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SortBuilder")
            .field("sorts", &self.sorts)
            .field("allowed_columns", &self.allowed_columns)
            .finish()
    }
}

impl SortBuilder<(), Safe> {
    /// Create as standalone (no parent) - Safe by default
    pub fn new() -> Self {
        Self {
            parent: None,
            sorts: SortingParams::new(),
            allowed_columns: None,
            _security: PhantomData,
        }
    }

    /// Finish and return only the sorting params
    pub fn build(self) -> Option<SortingParams> {
        if self.sorts.is_empty() {
            None
        } else {
            Some(self.sorts)
        }
    }
}

// Safe implementation - compile-time safety with &'static str
impl<P> SortBuilder<P, Safe> {
    fn with_parent(parent: P) -> Self {
        Self {
            parent: Some(parent),
            sorts: SortingParams::new(),
            allowed_columns: None,
            _security: PhantomData,
        }
    }

    /// Safe ASC ordering - only accepts static string literals (compile-time safe)
    pub fn asc(mut self, field: &'static str) -> Self {
        self.sorts = self.sorts.asc(field);
        self
    }

    /// Safe DESC ordering - only accepts static string literals (compile-time safe)
    pub fn desc(mut self, field: &'static str) -> Self {
        self.sorts = self.sorts.desc(field);
        self
    }

    /// Switch to unsafe mode with column whitelist for dynamic strings
    pub fn with_allowed_columns(self, allowed_columns: &'static [&'static str]) -> SortBuilder<P, Unsafe> {
        SortBuilder {
            parent: self.parent,
            sorts: self.sorts.with_allowed_columns(allowed_columns),
            allowed_columns: Some(allowed_columns),
            _security: PhantomData,
        }
    }
}

// Unsafe implementation - runtime validation against whitelist
impl SortBuilder<(), Unsafe> {
    /// Finish and return only the sorting params
    pub fn build(self) -> Option<SortingParams> {
        if self.sorts.is_empty() {
            None
        } else {
            Some(self.sorts)
        }
    }
}

impl<P> SortBuilder<P, Unsafe> {
    /// Unsafe ASC ordering - accepts dynamic strings (validation deferred to runtime)
    pub fn asc_unsafe(mut self, field: impl Into<String>) -> Self {
        let sort = Sort::asc_unsafe(field.into());
        self.sorts = self.sorts.push(sort);
        self
    }

    /// Unsafe DESC ordering - accepts dynamic strings (validation deferred to runtime)
    pub fn desc_unsafe(mut self, field: impl Into<String>) -> Self {
        let sort = Sort::desc_unsafe(field.into());
        self.sorts = self.sorts.push(sort);
        self
    }

}

// Generic methods available for both Safe and Unsafe modes
impl<P, S> SortBuilder<P, S> {
    /// Apply NULLS FIRST to the last added sort
    pub fn nulls_first(mut self) -> Self {
        self.sorts = self.sorts.apply_nulls_first();
        self
    }

    /// Apply NULLS LAST to the last added sort
    pub fn nulls_last(mut self) -> Self {
        self.sorts = self.sorts.apply_nulls_last();
        self
    }

    /// Apply default null ordering to the last added sort
    pub fn nulls_default(mut self) -> Self {
        self.sorts = self.sorts.apply_nulls_default();
        self
    }

    /// Finish and return to parent
    pub fn done(self) -> P
    where
        P: HasParams,
    {
        #[allow(clippy::expect_used)]
        let mut parent = self
            .parent
            .expect("SortBuilder::done called without a parent (programming error)");

        if !self.sorts.is_empty() {
            if let Some(existing_sorts) = parent.params_mut().sort_by.take() {
                parent.params_mut().sort_by = Some(existing_sorts.extend_with(self.sorts));
            } else {
                parent.params_mut().sort_by = Some(self.sorts);
            }
        }
        parent
    }
}

//
// ========================
//      FILTER BUILDER
// ========================
//

pub struct FilterBuilder<P = ()> {
    parent: Option<P>,
    filters: FilterParams,
}

impl Default for FilterBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FilterBuilder<()> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterBuilder")
            .field("filters", &self.filters)
            .finish()
    }
}

impl FilterBuilder<()> {
    /// Create as standalone (no parent)
    pub fn new() -> Self {
        Self {
            parent: None,
            filters: FilterParams::default(),
        }
    }

    /// Finish and return only the filters
    pub fn build(self) -> FilterParams {
        self.filters
    }
}

impl<P> FilterBuilder<P> {
    /// Create with parent
    pub fn with_parent(parent: P) -> Self {
        Self {
            parent: Some(parent),
            filters: FilterParams::default(),
        }
    }

    // --- PRIMITIVES ---

    fn push(
        mut self,
        field: impl Into<String>,
        op: FilterOperator,
        value: impl Into<FilterValue>,
    ) -> Self {
        self.filters.filters.push(Filter::new(field, op, value));
        self
    }

    pub fn eq(self, field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        self.push(field, FilterOperator::Eq, value)
    }

    pub fn ne(self, field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        self.push(field, FilterOperator::Ne, value)
    }

    pub fn gt(self, field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        self.push(field, FilterOperator::Gt, value)
    }

    pub fn lt(self, field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        self.push(field, FilterOperator::Lt, value)
    }

    pub fn gte(self, field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        self.push(field, FilterOperator::Gte, value)
    }

    pub fn lte(self, field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        self.push(field, FilterOperator::Lte, value)
    }

    /// Safe LIKE operator - automatically escapes special characters (% and _).
    /// Use this for user input to prevent wildcard injection.
    pub fn like(self, field: impl Into<String>, pat: impl Into<String>) -> Self {
        self.push(field, FilterOperator::Like, pat.into())
    }

    /// Case-insensitive LIKE operator - automatically escapes special characters.
    /// Safe for user input.
    pub fn ilike(self, field: impl Into<String>, pat: impl Into<String>) -> Self {
        self.push(field, FilterOperator::ILike, pat.into())
    }

    /// Unsafe LIKE operator - allows intentional wildcards (% and _).
    /// WARNING: Only use with controlled input, never with direct user input.
    /// Use this when you intentionally want wildcard behavior.
    pub fn like_pattern(self, field: impl Into<String>, pat: impl Into<String>) -> Self {
        self.push(field, FilterOperator::UnsafeLike, pat.into())
    }

    pub fn r#in(
        self,
        field: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<FilterValue>>,
    ) -> Self {
        let array = FilterValue::Array(values.into_iter().map(Into::into).collect());
        self.push(field, FilterOperator::In, array)
    }

    /// Alias for r#in
    pub fn in_values(
        self,
        field: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<FilterValue>>,
    ) -> Self {
        self.r#in(field, values)
    }

    pub fn not_in(
        self,
        field: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<FilterValue>>,
    ) -> Self {
        self.push(
            field,
            FilterOperator::NotIn,
            FilterValue::Array(values.into_iter().map(Into::into).collect()),
        )
    }

    pub fn between(
        self,
        field: impl Into<String>,
        min: impl Into<FilterValue>,
        max: impl Into<FilterValue>,
    ) -> Self {
        self.push(
            field,
            FilterOperator::Between,
            FilterValue::Array(vec![min.into(), max.into()]),
        )
    }

    pub fn is_null(self, field: impl Into<String>) -> Self {
        self.push(field, FilterOperator::IsNull, FilterValue::Null)
    }

    pub fn is_not_null(self, field: impl Into<String>) -> Self {
        self.push(field, FilterOperator::IsNotNull, FilterValue::Null)
    }

    pub fn contains(self, field: impl Into<String>, value: impl Into<FilterValue>) -> Self {
        self.push(field, FilterOperator::Contains, value)
    }

    /// Negates the last applied filter operation.
    /// Supports: Like, LikePattern, ILike, In, Between.
    /// Usage: .like("name", "pattern").not() // Creates NOT LIKE
    #[allow(clippy::should_implement_trait)]
    pub fn not(mut self) -> Self {
        if let Some(last_filter) = self.filters.filters.last_mut() {
            last_filter.not = true;
        }
        self
    }

    /// Finish and return to parent
    pub fn done(self) -> P
    where
        P: HasParams,
    {
        #[allow(clippy::expect_used)]
        let mut parent = self
            .parent
            .expect("FilterBuilder::done called without a parent (programming error)");
        if let Some(ref mut existing_filters) = parent.params_mut().filters {
            existing_filters.filters.extend(self.filters.filters);
        } else {
            parent.params_mut().filters = Some(self.filters);
        }
        parent
    }
}

//
// ========================
//      SEARCH BUILDER
// ========================
//

pub struct SearchBuilder<P = ()> {
    parent: Option<P>,
    search: SearchParams,
}

impl Default for SearchBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SearchBuilder<()> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchBuilder")
            .field("search", &self.search)
            .finish()
    }
}

impl SearchBuilder<()> {
    pub fn new() -> Self {
        Self {
            parent: None,
            search: SearchParams::new("", vec![]),
        }
    }

    pub fn build(self) -> SearchParams {
        self.search
    }
}

impl<P> SearchBuilder<P> {
    pub fn with_parent(parent: P) -> Self {
        Self {
            parent: Some(parent),
            search: SearchParams::new("", vec![]),
        }
    }

    pub fn query(mut self, q: impl Into<String>) -> Self {
        self.search.query = q.into();
        self
    }

    /// Convenience method - sets query and fields in one call
    pub fn search<I, S>(mut self, query: impl Into<String>, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.search.query = query.into();
        self.search.fields = fields.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.search.fields = fields.into_iter().map(Into::into).collect();
        self
    }

    pub fn exact(mut self, yes: bool) -> Self {
        self.search = self.search.with_exact_match(yes);
        self
    }

    pub fn case_sensitive(mut self, yes: bool) -> Self {
        self.search = self.search.with_case_sensitive(yes);
        self
    }

    pub fn done(self) -> P
    where
        P: HasParams,
    {
        #[allow(clippy::expect_used)]
        let mut parent = self
            .parent
            .expect("SearchBuilder::done called without a parent (programming error)");

        if !self.search.query.is_empty() {
            parent.params_mut().search = Some(self.search);
        }

        parent
    }
}

//
// ========================
//     SERIAL BUILDER
// ========================
//

pub struct SerialBuilder<P = ()> {
    parent: Option<P>,
    serial: crate::serial::SerialParams,
}

impl Default for SerialBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SerialBuilder<()> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialBuilder")
            .field("serial", &self.serial)
            .finish()
    }
}

impl SerialBuilder<()> {
    pub fn new() -> Self {
        Self {
            parent: None,
            serial: crate::serial::SerialParams::default(),
        }
    }

    pub fn with_page(page: u32, per_page: u32) -> Self {
        Self {
            parent: None,
            serial: crate::serial::SerialParams::new(page, per_page),
        }
    }

    pub fn build(self) -> crate::serial::SerialParams {
        self.serial
    }
}

impl<P> SerialBuilder<P> {
    pub fn with_parent(parent: P) -> Self {
        Self {
            parent: Some(parent),
            serial: crate::serial::SerialParams::default(),
        }
    }

    pub fn page(mut self, page: u32, per_page: u32) -> Self {
        self.serial = crate::serial::SerialParams::new(page, per_page);
        self
    }

    pub fn done(self) -> P
    where
        P: HasParams,
    {
        #[allow(clippy::expect_used)]
        let mut parent = self
            .parent
            .expect("SerialBuilder::done called without a parent (programming error)");
        let params = parent.params_mut();
        let limit = self.serial.limit();
        let offset = self.serial.offset();
        params.pagination = Some(crate::pagination::Pagination::Serial(self.serial));
        params.limit = Some(crate::pagination::LimitParam(limit));
        params.offset = Some(crate::pagination::OffsetParam(offset));
        parent
    }
}

//
// ========================
//     SLICE BUILDER
// ========================
//

pub struct SliceBuilder<P = ()> {
    parent: Option<P>,
    slice: crate::slice::SliceParams,
}

impl Default for SliceBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SliceBuilder<()> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SliceBuilder")
            .field("slice", &self.slice)
            .finish()
    }
}

impl SliceBuilder<()> {
    pub fn new() -> Self {
        Self {
            parent: None,
            slice: crate::slice::SliceParams::default(),
        }
    }

    pub fn with_page(page: u32, per_page: u32) -> Self {
        Self {
            parent: None,
            slice: crate::slice::SliceParams::new(page, per_page),
        }
    }

    pub fn build(self) -> crate::slice::SliceParams {
        self.slice
    }
}

impl<P> SliceBuilder<P> {
    pub fn with_parent(parent: P) -> Self {
        Self {
            parent: Some(parent),
            slice: crate::slice::SliceParams::default(),
        }
    }

    pub fn page(mut self, page: u32, per_page: u32) -> Self {
        self.slice = crate::slice::SliceParams::new(page, per_page);
        self
    }

    pub fn enable_total_count(mut self) -> Self {
        self.slice = self.slice.with_disable_total_count(false);
        self
    }

    pub fn done(self) -> P
    where
        P: HasParams,
    {
        #[allow(clippy::expect_used)]
        let mut parent = self
            .parent
            .expect("SliceBuilder::done called without a parent (programming error)");
        let params = parent.params_mut();
        let limit = self.slice.limit();
        let offset = self.slice.offset();
        params.pagination = Some(crate::pagination::Pagination::Slice(self.slice));
        params.limit = Some(crate::pagination::LimitParam(limit));
        params.offset = Some(crate::pagination::OffsetParam(offset));
        parent
    }
}

//
// ========================
//      CURSOR BUILDER
// ========================
// Using typestate builder
//

/// Represents the initial state where no direction has been defined.
/// At this stage, it can transition to `After`, `Before`, `FirstPage`, or decoded cursor.
pub struct Initial;

/// Represents the state for first page with empty cursor.
/// This is explicitly chosen by the developer to indicate initial pagination state.
pub struct FirstPage;

/// Represents the state where the `After` direction (Next Page) has been explicitly defined.
pub struct After;

/// Represents the state where the `Before` direction (Previous Page) has been explicitly defined.
pub struct Before;


pub struct CursorBuilder<P = (), S = Initial> {
    parent: Option<P>,
    cursor: CursorParams,
    _state: PhantomData<S>,
}

impl Default for CursorBuilder<(), Initial> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> std::fmt::Debug for CursorBuilder<(), S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CursorBuilder")
            .field("cursor", &self.cursor)
            .finish()
    }
}

impl CursorBuilder<(), Initial> {
    pub fn new() -> Self {
        Self {
            parent: None,
            cursor: CursorParams::default(),
            _state: PhantomData,
        }
    }

    pub fn build(self) -> CursorParams {
        self.cursor
    }
}

// -----------------------------
// Estado INITIAL - Initial state with no direction defined
impl<P> CursorBuilder<P, Initial> {
    pub fn with_parent(parent: P) -> Self {
        Self {
            parent: Some(parent),
            cursor: CursorParams::default(),
            _state: PhantomData,
        }
    }

    // Transition: Initial -> FirstPage (explicit first page)
    pub fn first_page(self) -> CursorBuilder<P, FirstPage> {
        CursorBuilder {
            parent: self.parent,
            cursor: CursorParams::default(), // Empty cursor for first page
            _state: PhantomData,
        }
    }

    // Transition: Initial -> After
    pub fn after(
        self,
        value: impl Into<FilterValue>
    ) -> CursorBuilder<P, After> {
        let mut cursor = CursorParams::default();
        cursor = cursor.and_field(value.into());
        cursor.direction = Some(CursorDirection::After);

        CursorBuilder {
            parent: self.parent,
            cursor,
            _state: PhantomData,
        }
    }

    // Transition: Initial -> Before
    pub fn before(
        self,
        value: impl Into<FilterValue>
    ) -> CursorBuilder<P, Before> {
        let mut cursor = CursorParams::default();
        cursor = cursor.and_field(value.into());
        cursor.direction = Some(CursorDirection::Before);

        CursorBuilder {
            parent: self.parent,
            cursor,
            _state: PhantomData,
        }
    }

    // Transition: Initial -> After (with encoded token - user must decode externally)
    pub fn next_cursor<T: crate::CursorSecureExtract>(self, token: impl Into<String>) -> CursorBuilder<P, After> {
        self.decoded::<T, After>(token, CursorDirection::After)
    }

    // Transition: Initial -> Before (with encoded token - user must decode externally)
    pub fn prev_cursor<T: crate::CursorSecureExtract>(self, token: impl Into<String>) -> CursorBuilder<P, Before> {
        self.decoded::<T, Before>(token, CursorDirection::Before)
    }

    // Helper method to decode token with direction
    fn decoded<T: crate::CursorSecureExtract, S>(
        self,
        token: impl Into<String>,
        direction: CursorDirection,
    ) -> CursorBuilder<P, S> {
        let token = token.into();

        let cursor = match T::decode(&token) {
            Ok(decoded_values) => {
                CursorParams::from_values(decoded_values, direction)
            }
            Err(e) => {
                CursorParams::with_error(direction, e.to_string())
            }
        };

        CursorBuilder {
            parent: self.parent,
            cursor,
            _state: PhantomData,
        }
    }
}

// -----------------------------
// Estado FIRSTPAGE - Explicit first page with empty cursor
impl<P> CursorBuilder<P, FirstPage> {

    pub fn done(self) -> P
    where
        P: HasParams,
    {
        #[allow(clippy::expect_used)]
        let mut parent = self
            .parent
            .expect("CursorBuilder::done called without a parent (programming error)");
        let params = parent.params_mut();
        // Set cursor pagination with empty cursor - this enables next_cursor/prev_cursor generation
        params.pagination = Some(crate::pagination::Pagination::Cursor(self.cursor));
        parent
    }
}

// -----------------------------
// Estado AFTER - After direction (next page) defined
impl<P> CursorBuilder<P, After> {

    /// Add more fields to create composite cursor
    pub fn and_field(mut self, value: impl Into<FilterValue>) -> Self {
        self.cursor = self.cursor.and_field(value.into());
        self
    }

    pub fn done(self) -> P
    where
        P: HasParams,
    {
        #[allow(clippy::expect_used)]
        let mut parent = self
            .parent
            .expect("CursorBuilder::done called without a parent (programming error)");
        let params = parent.params_mut();
        params.pagination = Some(crate::pagination::Pagination::Cursor(self.cursor));
        parent
    }
}

// -----------------------------
// Estado BEFORE - Before direction (previous page) defined
impl<P> CursorBuilder<P, Before> {

    /// Add more fields to create composite cursor
    pub fn and_field(mut self, value: impl Into<FilterValue>) -> Self {
        self.cursor = self.cursor.and_field(value.into());
        self
    }

    pub fn done(self) -> P
    where
        P: HasParams,
    {
        #[allow(clippy::expect_used)]
        let mut parent = self
            .parent
            .expect("CursorBuilder::done called without a parent (programming error)");
        let params = parent.params_mut();
        params.pagination = Some(crate::pagination::Pagination::Cursor(self.cursor));
        parent
    }
}

// -----------------------------

/// Trait for types that have params
pub trait HasParams {
    fn params_mut(&mut self) -> &mut Params;
}

impl HasParams for ParamsBuilder {
    fn params_mut(&mut self) -> &mut Params {
        &mut self.params
    }
}

impl IntoParams for ParamsBuilder {
    fn into_params(self) -> Params {
        self.params
    }
}

impl IntoParams for FilterBuilder<()> {
    fn into_params(self) -> Params {
        Params {
            filters: if self.filters.filters.is_empty() {
                None
            } else {
                Some(self.filters)
            },
            ..Default::default()
        }
    }
}

impl IntoParams for SearchBuilder<()> {
    fn into_params(self) -> Params {
        Params {
            search: if self.search.query.is_empty() {
                None
            } else {
                Some(self.search)
            },
            ..Default::default()
        }
    }
}

impl IntoParams for SerialBuilder<()> {
    fn into_params(self) -> Params {
        let limit = self.serial.limit();
        let offset = self.serial.offset();
        Params {
            pagination: Some(crate::pagination::Pagination::Serial(self.serial)),
            limit: Some(crate::pagination::LimitParam(limit)),
            offset: Some(crate::pagination::OffsetParam(offset)),
            ..Default::default()
        }
    }
}

impl IntoParams for SliceBuilder<()> {
    fn into_params(self) -> Params {
        let limit = self.slice.limit();
        let offset = self.slice.offset();
        Params {
            pagination: Some(crate::pagination::Pagination::Slice(self.slice)),
            limit: Some(crate::pagination::LimitParam(limit)),
            offset: Some(crate::pagination::OffsetParam(offset)),
            ..Default::default()
        }
    }
}

// IntoParams implementations for different cursor states
impl IntoParams for CursorBuilder<(), Initial> {
    fn into_params(self) -> Params {
        Params {
            // Initial state without cursor - no pagination set
            ..Default::default()
        }
    }
}

impl IntoParams for CursorBuilder<(), FirstPage> {
    fn into_params(self) -> Params {
        Params {
            pagination: Some(crate::pagination::Pagination::Cursor(self.cursor)),
            ..Default::default()
        }
    }
}

impl IntoParams for CursorBuilder<(), After> {
    fn into_params(self) -> Params {
        Params {
            pagination: Some(crate::pagination::Pagination::Cursor(self.cursor)),
            ..Default::default()
        }
    }
}

impl IntoParams for CursorBuilder<(), Before> {
    fn into_params(self) -> Params {
        Params {
            pagination: Some(crate::pagination::Pagination::Cursor(self.cursor)),
            ..Default::default()
        }
    }
}


impl<S> IntoParams for SortBuilder<(), S> {
    fn into_params(self) -> Params {
        Params {
            sort_by: if self.sorts.is_empty() {
                None
            } else {
                Some(self.sorts)
            },
            ..Default::default()
        }
    }
}
#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn test_safe_sort_builder_compile_time_safety() {
        // This should compile - using static string literals
        let params = ParamsBuilder::new()
            .sort()
            .asc("id")           // &'static str - safe
            .desc("created_at")  // &'static str - safe
            .done()
            .build();

        assert!(params.sort_by.is_some());
        let sort_by = params.sort_by.unwrap();
        assert_eq!(sort_by.sorts().len(), 2);
        assert_eq!(sort_by.sorts()[0].field, "id");
        assert!(sort_by.sorts()[0].is_asc());
        assert_eq!(sort_by.sorts()[1].field, "created_at");
        assert!(!sort_by.sorts()[1].is_asc());
    }

    #[test]
    fn test_unsafe_sort_builder_with_runtime_validation() {
        // Test using dynamic strings - validation happens at runtime
        let dynamic_field = "name".to_string();
        let invalid_field = "malicious_field".to_string();

        let params = ParamsBuilder::new()
            .sort()
            .with_allowed_columns(&["id", "name", "created_at", "age"])
            .asc_unsafe(dynamic_field)  // Valid field - preserved
            .desc_unsafe(invalid_field) // Invalid field - also preserved
            .done()
            .build();

        assert!(params.sort_by.is_some());
        let sort_by = params.sort_by.unwrap();
        assert_eq!(sort_by.sorts().len(), 2);

        // Fields are preserved as-is during build
        assert_eq!(sort_by.sorts()[0].field, "name");
        assert!(sort_by.sorts()[0].is_asc());
        assert_eq!(sort_by.sorts()[1].field, "malicious_field");
        assert!(!sort_by.sorts()[1].is_asc());

        // Should have unsafe fields
        assert!(sort_by.has_unsafe_fields());

        // Runtime validation should fail due to "malicious_field"
        assert!(sort_by.validate_fields().is_err());
    }

    #[test]
    fn test_standalone_sort_builder() {
        // Test standalone usage (without ParamsBuilder)
        let safe_sort = SortBuilder::new()
            .asc("id")
            .desc("created_at")
            .build();

        assert!(safe_sort.is_some());
        let sort_params = safe_sort.unwrap();
        assert_eq!(sort_params.sorts().len(), 2);

        // Test unsafe standalone - no validation in builder, field is preserved
        let unsafe_sort = SortBuilder::new()
            .with_allowed_columns(&["id", "name"])
            .asc_unsafe("invalid_field".to_string()) // Field is preserved, validation deferred
            .build();

        assert!(unsafe_sort.is_some());
        let sort_params = unsafe_sort.unwrap();
        assert_eq!(sort_params.sorts().len(), 1);
        assert_eq!(sort_params.sorts()[0].field, "invalid_field"); // Field preserved
        assert!(sort_params.has_unsafe_fields()); // Should have unsafe fields

        // Validation should fail at runtime
        assert!(sort_params.validate_fields().is_err());
    }

    #[test]
    fn test_runtime_validation_success() {
        // Test successful runtime validation
        let params = ParamsBuilder::new()
            .sort()
            .with_allowed_columns(&["id", "name", "email"])
            .asc_unsafe("name".to_string())  // Valid field
            .desc_unsafe("id".to_string())   // Valid field
            .done()
            .build();

        let sort_by = params.sort_by.unwrap();
        assert!(sort_by.has_unsafe_fields());

        // Runtime validation should succeed
        assert!(sort_by.validate_fields().is_ok());
    }
}