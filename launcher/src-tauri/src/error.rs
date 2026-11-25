use serde::{Serialize, Serializer};

/// One error type for every command. It serializes to a plain string so the
/// frontend can show it directly.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("Couldn't read archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl Error {
    pub fn msg(text: impl Into<String>) -> Self {
        Error::Msg(text.into())
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
