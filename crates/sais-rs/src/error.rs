use thiserror::Error;

#[derive(Debug, Error)]
pub enum SaisError {
    #[error("input length {0} exceeds SA index range")]
    InputTooLarge(usize),

    #[error("output buffer length {got} != input length {expected}")]
    BufferLen { expected: usize, got: usize },

    #[error("invalid primary index {primary} for BWT length {len}")]
    InvalidPrimaryIndex { primary: i64, len: usize },
}
