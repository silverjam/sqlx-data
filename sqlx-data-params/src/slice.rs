use crate::{IntoParams, Params};

#[derive(Clone, Debug)]
pub struct SliceParams {
    page: u32,
    page_size: u32,
    pub disable_total_count: bool,
}

impl Default for SliceParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
            disable_total_count: true,
        }
    }
}

impl SliceParams {
    pub fn new(page: u32, per_page: u32) -> Self {
        Self {
            page: page.max(1),
            page_size: per_page.max(1),
            disable_total_count: true,
        }
    }

    #[inline]
    pub fn page(&self) -> u32 {
        self.page
    }

    #[inline]
    pub fn page_size(&self) -> u32 {
        self.page_size
    }

    #[inline]
    pub fn offset(&self) -> u32 {
        (self.page.saturating_sub(1)).saturating_mul(self.page_size)
    }

    #[inline]
    pub fn limit(&self) -> u32 {
        self.page_size
    }

    #[inline]
    pub fn disable_total_count(&self) -> bool {
        self.disable_total_count
    }

    pub fn with_disable_total_count(mut self, disable: bool) -> Self {
        self.disable_total_count = disable;
        self
    }
}

impl IntoParams for SliceParams {
    fn into_params(self) -> Params {
        let page_size = self.page_size;
        let offset = self.offset();
        Params {
            filters: None,
            search: None,
            sort_by: None,
            pagination: Some(crate::pagination::Pagination::Slice(self)),
            limit: Some(crate::pagination::LimitParam(page_size)),
            offset: Some(crate::pagination::OffsetParam(offset)),
        }
    }
}
