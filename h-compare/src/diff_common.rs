//! Shared coordinates for a replaced/deleted/inserted block of rows.

#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockSpan {
    pub old_index: usize,
    pub old_len: usize,
    pub new_index: usize,
    pub new_len: usize,
}
