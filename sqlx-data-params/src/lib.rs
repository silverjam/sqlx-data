mod adapter;
mod builder;
mod cursor;
mod filter;
mod pagination;
mod params;
mod response;
mod search;
mod serial;
mod slice;
mod sort;

pub use builder::{
    CursorBuilder, FilterBuilder, ParamsBuilder, SearchBuilder, SerialBuilder, SliceBuilder,
    SortBuilder,
};
pub use cursor::{CursorDirection, CursorError, CursorParams, CursorSecureExtract, CursorValue, Cursor as CursorData};

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub use cursor::SqlxError;

pub use filter::{Filter, FilterOperator, FilterParams, FilterValue};
pub use pagination::{LimitParam, OffsetParam, Pagination};
pub use params::Params;
pub use response::{Cursor, Serial, Slice};
pub use search::SearchParams;
pub use serial::SerialParams;
pub use slice::SliceParams;
pub use sort::{NullOrdering, Sort, SortDirection, SortingParams};

/// Trait for types that can be converted to Params
pub trait IntoParams: std::fmt::Debug {
    fn into_params(self) -> Params;
}

impl IntoParams for Params {
    fn into_params(self) -> Params {
        self
    }
}

impl IntoParams for &Params {
    fn into_params(self) -> Params {
        self.clone()
    }
}
