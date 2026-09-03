use thiserror::Error;

#[derive(Debug, Error)]
pub enum DocumentErrors {
    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}
