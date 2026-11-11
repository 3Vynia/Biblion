//! Public facade. It is all the Tauri app needs: each method maps 1:1 to a
//! `#[tauri::command]`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::covers::CoverCache;
use crate::scanner::{self, ScanEvent, ScanOptions, ScanReport};
use crate::{
    db, formats, AuthorEntry, Book, BookId, BookStats, BookQuery, BookSummary, Error, Format, LibraryStats, Metadata, Page, Result,
    Shelf, TagEntry,
};

const MAX_IDLE_READERS: usize = 8;

pub struct Library {
    db_path: PathBuf,
    /// Write connection used by the scanner, which holds it for a whole scan.
    writer: Mutex<Connection>,
    /// Write connection for small user changes (progress, favourites…).
    /// SQLite still runs one write transaction at a time, but the scanner
    /// commits every few hundred books, so these wait milliseconds instead of
    /// the whole scan.
    user_writer: Mutex<Connection>,
    /// Reusable read connections (WAL allows reads while scanning).
    readers: Mutex<Vec<Connection>>,
    covers: CoverCache,
    scanning: AtomicBool,
    cancel: AtomicBool,
}

impl Library {
    /// Opens (or creates) a library in `data_dir`:
    /// `library.db` + `covers/thumbs/…`.
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref();
        std::fs::create_dir_all(data_dir).map_err(|e| Error::io(data_dir, e))?;
        let db_path = data_dir.join("library.db");
        let mut writer = db::open(&db_path)?;
        db::migrate(&mut writer)?;
        let user_writer = db::open(&db_path)?;
        Ok(Self {
            covers: CoverCache::new(data_dir.join("covers"))?,
            db_path,
            writer: Mutex::new(writer),
            user_writer: Mutex::new(user_writer),
            readers: Mutex::new(Vec::new()),
            scanning: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        })
    }

    fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.readers.lock().unwrap().pop();
        let conn = match conn {
            Some(c) => c,
            None => {
                let c = db::open(&self.db_path)?;
                c.execute_batch("PRAGMA query_only = ON;")?;
                c
            }
        };
        let out = f(&conn);
        let mut pool = self.readers.lock().unwrap();
        if pool.len() < MAX_IDLE_READERS {
            pool.push(conn);
        }
        out
    }

    fn write<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut conn = self.writer.lock().unwrap();
        f(&mut conn)
    }

    /// Writes made by the user; never blocked for the duration of a scan.
    fn user_write<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut conn = self.user_writer.lock().unwrap();
        f(&mut conn)
    }

    // ── Scanning ───────────────────────────────────────────────────────────

    /// Scans (blocking; in Tauri call it from `spawn_blocking`).
    /// Reads keep working meanwhile.
    pub fn scan(&self, opts: &ScanOptions, on_event: &(dyn Fn(ScanEvent) + Sync)) -> Result<ScanReport> {
        if self.scanning.swap(true, Ordering::SeqCst) {
            return Err(Error::corrupt("scanner", "a scan is already running"));
        }
        self.cancel.store(false, Ordering::SeqCst);
        let result = self.write(|conn| scanner::scan(conn, &self.covers, opts, &self.cancel, on_event));
        self.scanning.store(false, Ordering::SeqCst);
        result
    }

    pub fn cancel_scan(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::SeqCst)
    }

    // ── Queries ─────────────────────────────────────────────────────────

    pub fn list_books(&self, q: &BookQuery) -> Result<Page<BookSummary>> {
        self.read(|c| db::list_books(c, q))
    }

    pub fn get_book(&self, id: BookId) -> Result<Book> {
        self.read(|c| db::get_book(c, id))
    }

    pub fn list_authors(&self, prefix: Option<&str>, offset: u32, limit: u32) -> Result<Page<AuthorEntry>> {
        self.read(|c| db::list_authors(c, prefix, offset, limit))
    }

    pub fn list_series(&self, offset: u32, limit: u32) -> Result<Page<(String, u64)>> {
        self.read(|c| db::list_series(c, offset, limit))
    }

    pub fn stats(&self) -> Result<LibraryStats> {
        self.read(db::stats)
    }

    /// Groups of identical files (same fingerprint).
    pub fn duplicates(&self, limit: u32) -> Result<Vec<Vec<BookId>>> {
        self.read(|c| {
            let mut stmt = c.prepare(
                "SELECT group_concat(id) FROM books WHERE missing = 0 GROUP BY fingerprint HAVING COUNT(*) > 1 LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?.split(',').filter_map(|s| s.parse().ok()).collect());
            }
            Ok(out)
        })
    }

    // ── Reading and user state ──────────────────────────────────────

    pub fn set_progress(&self, id: BookId, locator: &str, percent: f64, finished: bool) -> Result<()> {
        self.user_write(|c| db::set_progress(c, id, locator, percent, finished))
    }

    pub fn set_favorite(&self, id: BookId, favorite: bool) -> Result<()> {
        self.user_write(|c| db::set_favorite(c, id, favorite))
    }

    pub fn set_rating(&self, id: BookId, rating: Option<u8>) -> Result<()> {
        self.user_write(|c| db::set_rating(c, id, rating))
    }

    pub fn update_metadata(&self, id: BookId, m: Metadata) -> Result<()> {
        let mut m = m;
        formats::util::normalize_metadata(&mut m);
        self.user_write(|c| db::update_metadata(c, id, &m))
    }

    pub fn create_shelf(&self, name: &str) -> Result<i64> {
        self.user_write(|c| db::create_shelf(c, name))
    }

    pub fn rename_shelf(&self, shelf: i64, name: &str) -> Result<()> {
        self.user_write(|c| db::rename_shelf(c, shelf, name))
    }

    /// Deletes a collection. The books in it are not touched.
    pub fn delete_shelf(&self, shelf: i64) -> Result<()> {
        self.user_write(|c| db::delete_shelf(c, shelf))
    }

    pub fn list_tags(&self, prefix: Option<&str>, offset: u32, limit: u32) -> Result<Page<TagEntry>> {
        self.read(|c| db::list_tags(c, prefix, offset, limit))
    }

    pub fn list_languages(&self) -> Result<Vec<(String, u64)>> {
        self.read(db::list_languages)
    }

    pub fn add_to_shelf(&self, shelf: i64, book: BookId) -> Result<()> {
        self.user_write(|c| db::add_to_shelf(c, shelf, book))
    }

    pub fn remove_from_shelf(&self, shelf: i64, book: BookId) -> Result<()> {
        self.user_write(|c| db::remove_from_shelf(c, shelf, book))
    }

    /// Ids of the shelves (collections) a book belongs to.
    pub fn book_shelves(&self, id: BookId) -> Result<Vec<i64>> {
        self.read(|c| db::book_shelves(c, id))
    }

    /// Takes several books out of a collection (they stay in the library).
    pub fn remove_many_from_shelf(&self, shelf: i64, books: &[BookId]) -> Result<usize> {
        self.user_write(|c| db::remove_many_from_shelf(c, shelf, books))
    }

    pub fn list_shelves(&self) -> Result<Vec<Shelf>> {
        self.read(db::list_shelves)
    }

    // ── Covers ──────────────────────────────────────────────────────────

    /// On-disk thumbnail path (the UI serves it via Tauri's asset protocol).
    pub fn thumbnail_path(&self, cover_key: &str) -> PathBuf {
        self.covers.thumb_path(cover_key)
    }

    pub fn covers_dir(&self) -> &Path {
        self.covers.root()
    }

    /// Full-resolution cover, extracted from the file on demand.
    pub fn full_cover(&self, id: BookId) -> Result<Option<Vec<u8>>> {
        let book = self.get_book(id)?;
        let path = Path::new(&book.path);
        if book.format.is_comic() {
            return Ok(formats::comic::read_page(path, 0).ok());
        }
        Ok(formats::extract(path, book.format)?.cover)
    }

    /// Sets the cover from image bytes (e.g. the first page of a PDF rendered
    /// by pdf.js in the UI, or an image chosen by the user).
    pub fn set_cover(&self, id: BookId, image_bytes: &[u8]) -> Result<String> {
        let book = self.get_book(id)?;
        let key = book.fingerprint.clone();
        self.covers.store(&key, image_bytes)?;
        self.user_write(|c| db::set_cover_key(c, id, &key))?;
        Ok(key)
    }

    /// Books of a format without a cover (so the UI can render PDF covers in the background).
    pub fn books_without_cover(&self, format: Format, limit: u32) -> Result<Vec<BookId>> {
        self.read(|c| {
            let mut stmt = c.prepare_cached(
                "SELECT id FROM books WHERE cover_key IS NULL AND missing = 0 AND format = ?1 ORDER BY id LIMIT ?2",
            )?;
            let v = stmt.query_map(rusqlite::params![format.as_str(), limit], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            Ok(v)
        })
    }

    // ── Analysis ──────────────────────────────────────────────────────────

    /// Stats computed earlier for the book's current file, if any.
    pub fn book_stats(&self, id: BookId) -> Result<Option<BookStats>> {
        self.read(|c| db::get_stats(c, id))
    }

    /// Counts words, characters, chapters and images (EPUB) or pages (comics).
    /// Results are cached until the file changes. Reads the whole book, so
    /// call it on demand, not for every book during a scan.
    pub fn analyze(&self, id: BookId) -> Result<BookStats> {
        if let Some(s) = self.book_stats(id)? {
            return Ok(s);
        }
        let book = self.get_book(id)?;
        let path = PathBuf::from(&book.path);
        let format = book.format;
        let stats = std::panic::catch_unwind(move || match format {
            Format::Epub => formats::epub::analyze(&path),
            Format::Cbz | Format::Cbr => {
                formats::comic::list_pages(&path).map(|p| BookStats::new(0, 0, 0, p.len() as u32))
            }
            _ => Err(Error::UnsupportedFormat(format!("content analysis is not available for {}", format.as_str()))),
        })
        .map_err(|_| Error::corrupt("analysis", "panic while reading the file"))??;
        self.user_write(|c| db::put_stats(c, id, &book.fingerprint, &stats))?;
        Ok(stats)
    }

    // ── Comics ────────────────────────────────────────────────────────────

    pub fn comic_pages(&self, id: BookId) -> Result<Vec<String>> {
        let book = self.get_book(id)?;
        formats::comic::list_pages(Path::new(&book.path))
    }

    pub fn comic_page(&self, id: BookId, index: usize) -> Result<Vec<u8>> {
        let book = self.get_book(id)?;
        formats::comic::read_page(Path::new(&book.path), index)
    }
}
