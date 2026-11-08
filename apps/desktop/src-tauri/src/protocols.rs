//! Custom URI schemes that stream files from disk to the web view without
//! going through IPC:
//!
//! - `thumb://localhost/<cover_key>`: cover thumbnails from the on-disk cache.
//! - `book://localhost/<book_id>`: the book file itself, for the readers.
//!
//! On Windows the web view sees them as `http://thumb.localhost/…`; the
//! request path is the same on every platform. Only database ids and
//! validated cover keys are accepted, so these schemes can never be used to
//! read arbitrary files.

use std::fs;

use biblion_core::{Format, Library};
use tauri::http::{header, Response, StatusCode};

type HttpResponse = Response<Vec<u8>>;

fn plain(status: StatusCode, msg: &str) -> HttpResponse {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(msg.as_bytes().to_vec())
        .unwrap()
}

fn first_segment(path: &str) -> &str {
    path.trim_start_matches('/').split(['/', '?']).next().unwrap_or("")
}

/// Cover keys are BLAKE3 hex fingerprints; accept only safe characters.
fn valid_cover_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 128 && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn thumbnail(lib: &Library, path: &str) -> HttpResponse {
    let key = first_segment(path);
    if !valid_cover_key(key) {
        return plain(StatusCode::BAD_REQUEST, "invalid cover key");
    }
    match fs::read(lib.thumbnail_path(key)) {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, "image/jpeg")
            // Keys are content fingerprints; a day is a safe cache window.
            .header(header::CACHE_CONTROL, "max-age=86400")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(bytes)
            .unwrap(),
        Err(_) => plain(StatusCode::NOT_FOUND, "thumbnail not found"),
    }
}

pub fn mime_type(format: Format) -> &'static str {
    match format {
        Format::Epub => "application/epub+zip",
        Format::Pdf => "application/pdf",
        Format::Mobi => "application/x-mobipocket-ebook",
        Format::Azw3 => "application/vnd.amazon.ebook",
        Format::Cbz => "application/vnd.comicbook+zip",
        Format::Cbr => "application/vnd.comicbook-rar",
    }
}

pub fn book_file(lib: &Library, path: &str) -> HttpResponse {
    let Ok(id) = first_segment(path).parse::<i64>() else {
        return plain(StatusCode::BAD_REQUEST, "invalid book id");
    };
    let book = match lib.get_book(id) {
        Ok(b) => b,
        Err(biblion_core::Error::NotFound(_)) => return plain(StatusCode::NOT_FOUND, "book not found"),
        Err(e) => return plain(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    match fs::read(&book.path) {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, mime_type(book.format))
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(bytes)
            .unwrap(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            plain(StatusCode::NOT_FOUND, "the book file is missing from disk")
        }
        Err(e) => plain(StatusCode::INTERNAL_SERVER_ERROR, &format!("cannot read the book file: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_cover_keys() {
        assert!(valid_cover_key("ab12cd"));
        assert!(!valid_cover_key(""));
        assert!(!valid_cover_key(".."));
        assert!(!valid_cover_key("a/b"));
        assert!(!valid_cover_key("%2e%2e"));
        assert_eq!(first_segment("/abc?x=1"), "abc");
        assert_eq!(first_segment("/../etc/passwd"), "..");
    }

    #[test]
    fn serves_only_known_books_and_thumbnails() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(tmp.path().join("data")).unwrap();
        assert_eq!(thumbnail(&lib, "/../../etc/passwd").status(), StatusCode::BAD_REQUEST);
        assert_eq!(thumbnail(&lib, "/deadbeef").status(), StatusCode::NOT_FOUND);
        assert_eq!(book_file(&lib, "/etc/passwd").status(), StatusCode::BAD_REQUEST);
        assert_eq!(book_file(&lib, "/42").status(), StatusCode::NOT_FOUND);
    }
}
