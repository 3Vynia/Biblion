use serde::{Deserialize, Serialize};

pub type BookId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Epub,
    Pdf,
    Cbz,
    Cbr,
    Mobi,
    Azw3,
}

impl Format {
    pub const ALL: [Format; 6] =
        [Format::Epub, Format::Pdf, Format::Cbz, Format::Cbr, Format::Mobi, Format::Azw3];

    pub fn from_extension(ext: &str) -> Option<Self> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "epub" => Format::Epub,
            "pdf" => Format::Pdf,
            "cbz" => Format::Cbz,
            "cbr" => Format::Cbr,
            "mobi" | "prc" => Format::Mobi,
            "azw3" | "azw" | "kf8" => Format::Azw3,
            _ => return None,
        })
    }

    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        path.extension().and_then(|e| e.to_str()).and_then(Self::from_extension)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Format::Epub => "epub",
            Format::Pdf => "pdf",
            Format::Cbz => "cbz",
            Format::Cbr => "cbr",
            Format::Mobi => "mobi",
            Format::Azw3 => "azw3",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::from_extension(s)
    }

    pub fn is_comic(self) -> bool {
        matches!(self, Format::Cbz | Format::Cbr)
    }
}

/// Metadata extracted from a file (before it is stored in the DB).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub isbn: Option<String>,
    pub published: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub page_count: Option<u32>,
}

/// A book as the UI sees it in listings (lightweight).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSummary {
    pub id: BookId,
    pub title: String,
    pub authors: Vec<String>,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub format: Format,
    pub has_cover: bool,
    /// Thumbnail key in the cover cache.
    pub cover_key: Option<String>,
    pub progress: Option<f64>,
    pub favorite: bool,
}

/// Full record of a book.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub id: BookId,
    pub path: String,
    pub format: Format,
    pub size: u64,
    pub fingerprint: String,
    pub metadata: Metadata,
    pub has_cover: bool,
    pub cover_key: Option<String>,
    pub added_at: i64,
    pub updated_at: i64,
    pub missing: bool,
    pub favorite: bool,
    pub rating: Option<u8>,
    pub progress: Option<ReadingProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadingProgress {
    /// Opaque position for the reader (EPUB CFI, page number for PDF/comics…).
    pub locator: String,
    /// 0.0 – 1.0
    pub percent: f64,
    pub updated_at: i64,
    pub finished: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    #[default]
    Title,
    Author,
    Series,
    AddedDesc,
    RecentlyRead,
    Relevance,
}

/// Listing query: combinable filters + pagination.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BookQuery {
    /// Free text (FTS5): title, authors, series, tags, description.
    pub text: Option<String>,
    pub author_id: Option<i64>,
    pub series: Option<String>,
    pub tag: Option<String>,
    pub shelf_id: Option<i64>,
    pub format: Option<Format>,
    pub favorites_only: bool,
    pub reading_only: bool,
    /// Only books marked as finished.
    pub finished_only: bool,
    pub include_missing: bool,
    pub sort: SortBy,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorEntry {
    pub id: i64,
    pub name: String,
    pub book_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shelf {
    pub id: i64,
    pub name: String,
    pub book_count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryStats {
    pub books: u64,
    pub missing: u64,
    pub authors: u64,
    pub series: u64,
    pub with_cover: u64,
    pub total_bytes: u64,
    pub by_format: Vec<(Format, u64)>,
}
