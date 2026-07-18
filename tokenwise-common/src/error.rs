use thiserror::Error;

/// All errors that Tokenwise can produce.
#[derive(Debug, Error)]
pub enum TokenwiseError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Port in use: {0}")]
    PortInUse(u16),

    #[error("Missing prerequisite: {0}")]
    MissingPrerequisite(String),

    #[error("Invalid invocation: {0}")]
    InvalidInvocation(String),
}
