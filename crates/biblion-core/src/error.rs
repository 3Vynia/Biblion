use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("corrupt or invalid file ({format}): {msg}")]
    Corrupt { format: &'static str, msg: String },
    #[error("invalid image: {0}")]
    Image(#[from] image::ImageError),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("scan cancelled")]
    Cancelled,
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }
    pub fn corrupt(format: &'static str, msg: impl ToString) -> Self {
        Error::Corrupt { format, msg: msg.to_string() }
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Self {
        Error::corrupt("zip", e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
