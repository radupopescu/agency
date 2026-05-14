use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("provider error: {0}")]
    Provider(String),

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("config: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, Error>;
