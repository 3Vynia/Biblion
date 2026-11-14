//! Per-format metadata and cover extraction.
//!
//! Every function is independent of the DB: it takes a path and returns
//! [`Extracted`]. They run in parallel from the scanner.

pub mod comic;
pub mod epub;
pub mod mobi;
pub mod pdf;
pub mod util;

use std::path::Path;

use crate::{Format, Metadata, Result};

/// Read limit for a cover image (protection against zip bombs).
pub const MAX_COVER_BYTES: u64 = 32 * 1024 * 1024;
/// Limit for metadata text files (OPF, ComicInfo.xml…).
pub const MAX_META_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct Extracted {
    pub metadata: Metadata,
    /// Raw bytes of the cover image, if one was found.
    pub cover: Option<Vec<u8>>,
}

/// Extracts metadata and cover. Corrupt files yield `Err`, and the scanner
/// then records the book with metadata taken from its file name.
pub fn extract(path: &Path, format: Format) -> Result<Extracted> {
    let mut out = match format {
        Format::Epub => epub::extract(path)?,
        Format::Cbz | Format::Cbr => comic::extract(path)?,
        Format::Mobi | Format::Azw3 => mobi::extract(path)?,
        Format::Pdf => pdf::extract(path)?,
    };
    if format.is_comic() {
        util::fill_comic_from_path(&mut out.metadata, path);
    } else {
        util::fill_from_path(&mut out.metadata, path);
    }
    util::normalize_metadata(&mut out.metadata);
    Ok(out)
}

/// Minimal metadata when the file cannot be read.
pub fn fallback(path: &Path) -> Extracted {
    let mut out = Extracted::default();
    if Format::from_path(path).is_some_and(Format::is_comic) {
        util::fill_comic_from_path(&mut out.metadata, path);
    } else {
        util::fill_from_path(&mut out.metadata, path);
    }
    util::normalize_metadata(&mut out.metadata);
    out
}
