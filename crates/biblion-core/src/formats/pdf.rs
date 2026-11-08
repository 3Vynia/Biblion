//! PDF: Info dictionary metadata and page count (lopdf, pure Rust).
//!
//! A PDF cover requires rasterising the first page. The core links no
//! rendering engine: the UI (pdf.js) renders it the first time the book is
//! shown and stores it with `Library::set_cover`.

use std::path::Path;

use super::Extracted;
use crate::{Error, Metadata, Result};

/// lopdf loads the whole file into memory; above this size only the file
/// name is used.
const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;

pub fn extract(path: &Path) -> Result<Extracted> {
    let len = std::fs::metadata(path).map_err(|e| Error::io(path, e))?.len();
    if len > MAX_PDF_BYTES {
        return Ok(Extracted::default());
    }
    let info = lopdf::Document::load_metadata(path).map_err(|e| Error::corrupt("pdf", e))?;
    let clean = |s: Option<String>| s.map(|v| v.trim_matches(char::from(0)).trim().to_owned()).filter(|v| !v.is_empty());
    let mut m = Metadata {
        title: clean(info.title).filter(|t| !looks_like_junk_title(t)),
        description: clean(info.subject),
        page_count: (info.page_count > 0).then_some(info.page_count),
        published: clean(info.creation_date).and_then(|d| pdf_date(&d)),
        ..Default::default()
    };
    if let Some(a) = clean(info.author) {
        m.authors.push(a);
    }
    if let Some(k) = clean(info.keywords) {
        m.tags.extend(k.split([',', ';']).map(|s| s.trim().to_owned()));
    }
    Ok(Extracted { metadata: m, cover: None })
}

/// Many PDFs carry junk titles such as "Microsoft Word - doc1.docx".
fn looks_like_junk_title(t: &str) -> bool {
    let l = t.to_ascii_lowercase();
    l.starts_with("microsoft word")
        || l.ends_with(".doc")
        || l.ends_with(".docx")
        || l.ends_with(".pdf")
        || l == "untitled"
        || l.starts_with("untitled-")
}

/// "D:20210315120000Z" → "2021-03-15"
fn pdf_date(d: &str) -> Option<String> {
    let d = d.trim_start_matches("D:");
    let digits: String = d.chars().take_while(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        n if n >= 8 => Some(format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8])),
        n if n >= 4 => Some(digits[0..4].to_owned()),
        _ => None,
    }
}
