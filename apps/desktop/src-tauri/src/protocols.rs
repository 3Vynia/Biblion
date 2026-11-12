//! Custom URI schemes that stream files from disk to the web view without
//! going through IPC:
//!
//! - `thumb://localhost/<cover_key>`: cover thumbnails from the on-disk cache.
//! - `book://localhost/<book_id>`: the book file itself, for the readers.
//!   Supports `Range` requests, so pdf.js can read large PDFs piece by piece.
//! - `comic://localhost/<book_id>/<page>`: one page image of a CBZ/CBR.
//!
//! On Windows the web view sees them as `http://thumb.localhost/…`; the
//! request path is the same on every platform. Only database ids and
//! validated cover keys are accepted, so these schemes can never be used to
//! read arbitrary files.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};

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

pub fn thumbnail(lib: &Library, path: &str, _range: Option<&str>) -> HttpResponse {
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

/// Parses a single `bytes=` range against a file of `len` bytes into an
/// inclusive `(start, end)`. `None` for anything unsatisfiable or unsupported.
pub fn parse_range(header: &str, len: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None; // multiple ranges are not needed by the readers
    }
    let (a, b) = spec.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    let (start, end) = if a.is_empty() {
        let suffix: u64 = b.parse().ok().filter(|&n| n > 0)?;
        (len.saturating_sub(suffix), len - 1)
    } else {
        let start: u64 = a.parse().ok()?;
        let end = if b.is_empty() { len - 1 } else { b.parse::<u64>().ok()?.min(len - 1) };
        (start, end)
    };
    (start <= end && start < len).then_some((start, end))
}

/// Largest piece served for one range request (pdf.js asks for 64 KiB chunks).
const MAX_RANGE: u64 = 16 * 1024 * 1024;

pub fn book_file(lib: &Library, path: &str, range: Option<&str>) -> HttpResponse {
    let Ok(id) = first_segment(path).parse::<i64>() else {
        return plain(StatusCode::BAD_REQUEST, "invalid book id");
    };
    let book = match lib.get_book(id) {
        Ok(b) => b,
        Err(biblion_core::Error::NotFound(_)) => return plain(StatusCode::NOT_FOUND, "book not found"),
        Err(e) => return plain(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let io_error = |e: std::io::Error| {
        if e.kind() == std::io::ErrorKind::NotFound {
            plain(StatusCode::NOT_FOUND, "the book file is missing from disk")
        } else {
            plain(StatusCode::INTERNAL_SERVER_ERROR, &format!("cannot read the book file: {e}"))
        }
    };
    let response = |status: StatusCode| {
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, mime_type(book.format))
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, "Content-Range, Accept-Ranges, Content-Length")
    };

    let Some(range) = range else {
        return match fs::read(&book.path) {
            Ok(bytes) => response(StatusCode::OK).body(bytes).unwrap(),
            Err(e) => io_error(e),
        };
    };
    let mut file = match File::open(&book.path) {
        Ok(f) => f,
        Err(e) => return io_error(e),
    };
    let len = match file.metadata() {
        Ok(m) => m.len(),
        Err(e) => return io_error(e),
    };
    let Some((start, end)) = parse_range(range, len) else {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{len}"))
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Vec::new())
            .unwrap();
    };
    let end = end.min(start + MAX_RANGE - 1);
    let mut buf = vec![0; (end - start + 1) as usize];
    if let Err(e) = file.seek(SeekFrom::Start(start)).and_then(|_| file.read_exact(&mut buf)) {
        return io_error(e);
    }
    response(StatusCode::PARTIAL_CONTENT).header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}")).body(buf).unwrap()
}

fn image_type(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xff, 0xd8, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'G', b'I', b'F', ..] => "image/gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'B', b'M', ..] => "image/bmp",
        _ => "application/octet-stream",
    }
}

/// `/<book_id>/<page index>` → the page image of a comic.
pub fn comic_page(lib: &Library, path: &str, _range: Option<&str>) -> HttpResponse {
    let mut parts = path.trim_start_matches('/').split('/');
    let (Some(Ok(id)), Some(Ok(index))) = (parts.next().map(str::parse::<i64>), parts.next().map(str::parse::<usize>))
    else {
        return plain(StatusCode::BAD_REQUEST, "expected /<book id>/<page>");
    };
    match lib.comic_page(id, index) {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, image_type(&bytes))
            .header(header::CACHE_CONTROL, "max-age=3600")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(bytes)
            .unwrap(),
        Err(biblion_core::Error::NotFound(_)) => plain(StatusCode::NOT_FOUND, "page not found"),
        Err(e) => plain(StatusCode::UNPROCESSABLE_ENTITY, &format!("cannot read this page: {e}")),
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
        assert_eq!(thumbnail(&lib, "/../../etc/passwd", None).status(), StatusCode::BAD_REQUEST);
        assert_eq!(thumbnail(&lib, "/deadbeef", None).status(), StatusCode::NOT_FOUND);
        assert_eq!(book_file(&lib, "/etc/passwd", None).status(), StatusCode::BAD_REQUEST);
        assert_eq!(book_file(&lib, "/42", None).status(), StatusCode::NOT_FOUND);
        assert_eq!(comic_page(&lib, "/42/x", None).status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn parses_byte_ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=900-", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=990-5000", 1000), Some((990, 999)), "end is clamped");
        assert_eq!(parse_range("bytes=-5000", 1000), Some((0, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None, "starts past the end");
        assert_eq!(parse_range("bytes=50-10", 1000), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
        assert_eq!(parse_range("bytes=0-1", 0), None);
    }

    #[test]
    fn serves_book_files_whole_and_in_ranges() {
        let tmp = tempfile::tempdir().unwrap();
        let books = tmp.path().join("books");
        fs::create_dir_all(&books).unwrap();
        // Not a valid PDF: still recorded by the scan (with an error), which is enough here.
        let content: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        fs::write(books.join("Some Author - Some Title.pdf"), &content).unwrap();
        let lib = Library::open(tmp.path().join("data")).unwrap();
        lib.scan(&biblion_core::scanner::ScanOptions { roots: vec![books], ..Default::default() }, &|_| {}).unwrap();
        let id = lib.list_books(&biblion_core::BookQuery { limit: 1, ..Default::default() }).unwrap().items[0].id;

        let whole = book_file(&lib, &format!("/{id}"), None);
        assert_eq!(whole.status(), StatusCode::OK);
        assert_eq!(whole.body(), &content);
        assert_eq!(whole.headers()[header::ACCEPT_RANGES], "bytes");

        let part = book_file(&lib, &format!("/{id}"), Some("bytes=100-199"));
        assert_eq!(part.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(part.body(), &content[100..200]);
        assert_eq!(part.headers()[header::CONTENT_RANGE], "bytes 100-199/5000");

        let tail = book_file(&lib, &format!("/{id}"), Some("bytes=-10"));
        assert_eq!(tail.body(), &content[4990..]);

        let bad = book_file(&lib, &format!("/{id}"), Some("bytes=6000-"));
        assert_eq!(bad.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(bad.headers()[header::CONTENT_RANGE], "bytes */5000");
    }
}
