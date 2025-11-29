use crate::filter::FilterParams;
use crate::pagination::LimitParam;
use crate::pagination::OffsetParam;
use crate::pagination::Pagination;
use crate::search::SearchParams;
use crate::sort::SortingParams;

#[derive(Clone, Debug, Default)]
pub struct Params {
    pub filters: Option<FilterParams>,
    pub search: Option<SearchParams>,
    pub sort_by: Option<SortingParams>,
    pub limit: Option<LimitParam>,
    pub offset: Option<OffsetParam>,
    pub pagination: Option<Pagination>,
}

impl Params {
    /// Checks if the params have no modifications (empty state)
    pub fn is_empty(&self) -> bool {
        self.filters.is_none()
            && self.search.is_none()
            && self.sort_by.is_none()
            && self.pagination.is_none()
            && self.limit.is_none()
            && self.offset.is_none()
    }

    /// Checks if the params have any modifications (opposite of is_empty)
    pub fn has_modifications(&self) -> bool {
        !self.is_empty()
    }

    pub fn is_disable_total_count(&self) -> bool {
        match &self.pagination {
            Some(Pagination::Slice(slice)) => slice.disable_total_count,
            _ => false, // Cursor and Serial does not have disable_total_count
        }
    }

    /// Returns 1 if pagination type requires LIMIT+1 for has_next detection, 0 otherwise
    pub fn limit_plus_one(&self) -> u32 {
        match &self.pagination {
            Some(Pagination::Slice(_)) => 1,  // Slice needs +1 for has_next
            Some(Pagination::Cursor(_)) => 1, // Cursor needs +1 for has_next
            Some(Pagination::Serial(_)) => 0, // Serial doesn't need +1 (uses total count)
            None => 0,                        // No pagination, no +1
        }
    }
}
