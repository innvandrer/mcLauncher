use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, Error>;

/// Unified error type for the whole backend. Implements `Serialize` so it can
/// be returned directly from `#[tauri::command]` functions and surfaced to the
/// frontend as a readable string.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("data error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("task error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("checksum mismatch for {file} (expected {expected}, got {actual})")]
    Checksum {
        file: String,
        expected: String,
        actual: String,
    },

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}

/// Convenience for building an `Error::Other` with formatting.
#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::error::Error::Other(format!($($arg)*)))
    };
}
