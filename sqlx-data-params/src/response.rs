#[cfg(feature = "json")]
use serde::{Deserialize, Serialize};

///This is Result<T,sqlx::Error>
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
pub type Result<T, E = sqlx_data_integration::Error> = ::std::result::Result<T, E>;

///This is Result<T,Cursor::Error> - just to justificate any feature
#[cfg(not(any(feature = "sqlite", feature = "postgres", feature = "mysql")))]
pub type Result<T, E = crate::cursor::CursorError> = ::std::result::Result<T, E>;

/// Serial pagination response - classic page-based pagination with total count
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]
pub struct Serial<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub size: u32,
    pub total_items: i64,
    pub total_pages: u32,
}

/// Slice pagination response - efficient pagination without total count
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]
pub struct Slice<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub size: u32,
    pub has_next: bool,
    pub has_previous: bool,
    pub total_items: Option<i64>, // Optional total count
}

/// Cursor pagination response - cursor-based pagination for large datasets
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "json", derive(Serialize, Deserialize))]
pub struct Cursor<T> {
    pub data: Vec<T>,
    pub per_page: u32,
    pub has_next: bool,
    pub has_prev: bool,
    pub next_cursor: Option<String>,
    pub prev_cursor: Option<String>,
}

impl<T> Serial<T> {
    const DEFAULT_LIMIT: u32 = 20;

    pub fn new(data: Vec<T>, params: &crate::Params, total_elements: i64) -> Self {
        let Some(crate::pagination::Pagination::Serial(serial)) = &params.pagination else {
            let default_size = Self::page_size(params);
            // Will never reach here...
            return Self {
                data,
                page: 1,
                size: default_size,
                total_items: total_elements,
                total_pages: 1,
            };
        };

        let size = serial.limit().max(1);
        let total_pages = Self::calculate_total_pages(total_elements, size);

        Self {
            data,
            page: serial.page(),
            size,
            total_items: total_elements,
            total_pages,
        }
    }

    #[inline]
    fn calculate_total_pages(total_items: i64, size: u32) -> u32 {
        let size = size as i64;

        total_items
            .checked_add(size - 1)
            .and_then(|sum| sum.checked_div(size))
            .and_then(|pages| u32::try_from(pages.max(0)).ok())
            .unwrap_or(1)
    }

    #[inline]
    fn page_size(params: &crate::Params) -> u32 {
        params
            .limit
            .as_ref()
            .map(|l| l.0)
            .unwrap_or(Self::DEFAULT_LIMIT)
            .max(1)
    }
}

impl<T> Slice<T> {
    const DEFAULT_LIMIT: u32 = 20;

    pub fn new(mut data: Vec<T>, params: &crate::Params, total_elements: i64) -> Self {
        let size = Self::page_size(params);
        let has_next = Self::trim_to_page(&mut data, size);

        if let Some(crate::pagination::Pagination::Slice(slice)) = &params.pagination {
            let page = slice.page();

            return Self {
                data,
                page,
                size,
                has_next,
                has_previous: page > 1,
                total_items: (!slice.disable_total_count()).then_some(total_elements),
            };
        }

        // Fallback: offset/limit
        let page = Self::page_offset(params, size);

        Self {
            data,
            page,
            size,
            has_next,
            has_previous: page > 1,
            total_items: Some(total_elements),
        }
    }

    #[inline]
    fn page_size(params: &crate::Params) -> u32 {
        params
            .limit
            .as_ref()
            .map(|l| l.0)
            .unwrap_or(Self::DEFAULT_LIMIT)
            .max(1)
    }

    #[inline]
    fn page_offset(params: &crate::Params, size: u32) -> u32 {
        params
            .offset
            .as_ref()
            .map(|o| o.0 as i64)
            .and_then(|offset| offset.checked_div(size as i64))
            .and_then(|p| p.checked_add(1))
            .unwrap_or(1) as u32
    }

    #[inline]
    fn trim_to_page(data: &mut Vec<T>, size: u32) -> bool {
        let size = size as usize;

        if data.len() > size {
            data.truncate(size);
            return true;
        }
        false
    }
}

impl<T: crate::CursorSecureExtract> Cursor<T> {
    const DEFAULT_LIMIT: u32 = 20;

    pub fn new(mut data: Vec<T>, params: &crate::Params) -> Result<Self> {
        // Extract sorting params - cursor pagination requires ORDER BY
        let sort = params.sort_by.as_ref().ok_or_else(|| {
            crate::cursor::CursorError::decode_error(
                "Cursor pagination requires ORDER BY (sort_by)",
            )
        })?;

        let Some(crate::pagination::Pagination::Cursor(cursor)) = &params.pagination else {
            #[allow(clippy::useless_conversion)]
            return Err(crate::cursor::CursorError::decode_error("Cursor params is not present").into());
        };

        let is_backward = cursor.direction == Some(crate::cursor::CursorDirection::Before);
        let requested_limit = Self::page_size(params);
        
        let has_more = data.len() > requested_limit as usize;
        
        if has_more {
            data.truncate(requested_limit as usize);
        }
        
        let had_prev = !cursor.is_empty(); //had previous cursor

        let (next_cursor, prev_cursor) =
            Self::generate_cursors(cursor, &data, has_more, had_prev, sort)?;

        if is_backward {
            data.reverse();
        }

        Ok(Self {
            data,
            per_page: requested_limit,
            has_next: if is_backward { had_prev } else { has_more },
            has_prev: if is_backward { has_more } else { had_prev },
            next_cursor,
            prev_cursor,
        })
    }

    #[inline]
    fn page_size(params: &crate::Params) -> u32 {
        params
            .limit
            .as_ref()
            .map(|l| l.0)
            .unwrap_or(Self::DEFAULT_LIMIT)
            .max(1)
    }

    #[inline]
    fn generate_cursors(
        cursor: &crate::pagination::CursorParams,
        data: &[T],
        has_more_pages: bool,
        had_previous_cursor: bool,
        sorting_params: &crate::sort::SortingParams,
    ) -> Result<(Option<String>, Option<String>)> {
        if data.is_empty() {
            return Ok((None, None));
        }

        let next = if has_more_pages {
            cursor.generate_next_cursor(data, has_more_pages, sorting_params)?
        } else {
            None
        };

        let prev = if had_previous_cursor {
            cursor.generate_prev_cursor(data, had_previous_cursor, sorting_params)?
        } else {
            None
        };

        Ok((next, prev))
    }
}
