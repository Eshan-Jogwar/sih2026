use std::fmt;

#[derive(Debug)]
pub enum DocumentErrors {
    StorageError(String),
    ValidationError(String),
    NetworkError(String),
}

impl fmt::Display for DocumentErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentErrors::StorageError(msg) => write!(f, "Storage error: {}", msg),
            DocumentErrors::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            DocumentErrors::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for DocumentErrors {}

impl From<reqwest::Error> for DocumentErrors {
    fn from(err: reqwest::Error) -> Self {
        DocumentErrors::NetworkError(err.to_string())
    }
}
