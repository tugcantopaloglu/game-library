pub mod discovery;
pub mod epic;
pub mod launcher;
pub mod models;
pub mod riot;
pub mod sources;
pub mod steam;
pub mod storage;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("{0}")]
    Invalid(String),
    #[error("Dosya işlemi başarısız: {0}")]
    Io(#[from] std::io::Error),
    #[error("Veritabanı işlemi başarısız: {0}")]
    Database(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, LibraryError>;
