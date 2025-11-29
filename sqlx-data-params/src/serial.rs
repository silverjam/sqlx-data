use crate::{IntoParams, Params};

#[derive(Clone, Debug)]
pub struct SerialParams {
    page: u32,
    page_size: u32,
}

impl Default for SerialParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
        }
    }
}

impl SerialParams {
    pub fn new(page: u32, per_page: u32) -> Self {
        Self {
            page: page.max(1),
            page_size: per_page.max(1),
        }
    }

    #[inline]
    pub fn page(&self) -> u32 {
        self.page
    }

    #[inline]
    pub fn per_page(&self) -> u32 {
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
}

impl IntoParams for SerialParams {
    fn into_params(self) -> Params {
        let page_size = self.page_size;
        let offset = self.offset();
        Params {
            filters: None,
            search: None,
            sort_by: None,
            pagination: Some(crate::pagination::Pagination::Serial(self)),
            limit: Some(crate::pagination::LimitParam(page_size)),
            offset: Some(crate::pagination::OffsetParam(offset)),
        }
    }
}
