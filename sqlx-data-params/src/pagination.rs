pub use crate::{CursorParams, SerialParams, slice::SliceParams};

#[derive(Clone, Debug)]
pub struct LimitParam(pub u32);

#[derive(Clone, Debug)]
pub struct OffsetParam(pub u32);

#[derive(Clone, Debug)]
pub enum Pagination {
    Serial(SerialParams),
    Slice(SliceParams),
    Cursor(CursorParams),
}

impl Default for Pagination {
    fn default() -> Self {
        Self::Serial(SerialParams::default())
    }
}
