//! SQLite schema, migrations and queries.
//!
//! Design choices for 200k+ books:
//! - WAL + `synchronous=NORMAL`: concurrent reads while scanning.
//! - Authors/tags are normalised (for browsing) **and** denormalised into
//!   `books` (to list a page without JOINs).
//! - FTS5 with `remove_diacritics 2`: "garcia" matches "García".
//! - One index per UI sort order, so pagination costs O(page).

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row, ToSql, Transaction};

use crate::formats::util::{author_sort_key, sort_key};
use crate::{
    AuthorEntry, Book, BookId, BookQuery, BookSummary, Format, LibraryStats, Metadata, Page, ReadingProgress, Result,
    Shelf, SortBy,
};

/// Separator for denormalised lists (a control character that never appears in names).
pub const SEP: char = '\u{1f}';

const SCHEMA_V1: &str = r#"
CREATE TABLE books (
    id            INTEGER PRIMARY KEY,
    path          TEXT    NOT NULL UNIQUE,
    format        TEXT    NOT NULL,
    size          INTEGER NOT NULL,
    mtime         INTEGER NOT NULL,
    fingerprint   TEXT    NOT NULL,
    title         TEXT    NOT NULL,
    sort_title    TEXT    NOT NULL,
    authors       TEXT    NOT NULL DEFAULT '',
    author_sort   TEXT    NOT NULL DEFAULT '',
    tags          TEXT    NOT NULL DEFAULT '',
    series        TEXT,
    series_index  REAL,
    publisher     TEXT,
    language      TEXT,
    isbn          TEXT,
    published     TEXT,
    description   TEXT,
    page_count    INTEGER,
    cover_key     TEXT,
    added_at      INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    missing       INTEGER NOT NULL DEFAULT 0,
    favorite      INTEGER NOT NULL DEFAULT 0,
    rating        INTEGER,
    user_edited   INTEGER NOT NULL DEFAULT 0,
    scan_error    TEXT
);
-- `missing` comes first in every sort index, so `missing = 0` and the
-- ORDER BY are served by the same index (no in-memory sort).
CREATE INDEX books_by_title   ON books(missing, sort_title, id);
CREATE INDEX books_by_author  ON books(missing, author_sort, sort_title, id);
CREATE INDEX books_by_added   ON books(missing, added_at DESC, id DESC);
CREATE INDEX books_by_series  ON books(missing, series IS NULL, series COLLATE NOCASE, series_index, sort_title);
CREATE INDEX books_by_format  ON books(missing, format, sort_title, id);
CREATE INDEX books_fingerprint ON books(fingerprint);
CREATE INDEX books_favorite   ON books(favorite, sort_title) WHERE favorite = 1;

CREATE TABLE authors (
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL UNIQUE COLLATE NOCASE,
    sort_name TEXT NOT NULL
);
CREATE INDEX authors_sort ON authors(sort_name);

CREATE TABLE book_authors (
    book_id   INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    author_id INTEGER NOT NULL REFERENCES authors(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL,
    PRIMARY KEY (book_id, author_id)
) WITHOUT ROWID;
CREATE INDEX book_authors_author ON book_authors(author_id, book_id);

CREATE TABLE tags (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE book_tags (
    book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (book_id, tag_id)
) WITHOUT ROWID;
CREATE INDEX book_tags_tag ON book_tags(tag_id, book_id);

CREATE TABLE progress (
    book_id    INTEGER PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    locator    TEXT    NOT NULL,
    percent    REAL    NOT NULL,
    finished   INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);
CREATE INDEX progress_recent ON progress(updated_at DESC);

CREATE TABLE bookmarks (
    id         INTEGER PRIMARY KEY,
    book_id    INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    locator    TEXT    NOT NULL,
    note       TEXT,
    highlight  TEXT,
    color      TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX bookmarks_book ON bookmarks(book_id, created_at);

CREATE TABLE shelves (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at INTEGER NOT NULL
);
CREATE TABLE shelf_books (
    shelf_id INTEGER NOT NULL REFERENCES shelves(id) ON DELETE CASCADE,
    book_id  INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (shelf_id, book_id)
) WITHOUT ROWID;
CREATE INDEX shelf_books_book ON shelf_books(book_id);

CREATE VIRTUAL TABLE books_fts USING fts5(
    title, authors, series, tags, description,
    tokenize = 'unicode61 remove_diacritics 2',
    prefix = '2 3'
);
"#;

const MIGRATIONS: &[&str] = &[SCHEMA_V1];

pub fn open(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

pub fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;
         PRAGMA cache_size = -65536;
         PRAGMA mmap_size = 268435456;",
    )?;
    Ok(())
}

pub fn migrate(conn: &mut Connection) -> Result<()> {
    let version = conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? as usize;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

// ───────────────────────────── Writes (scanner) ─────────────────────────────

/// Scanner output for one file.
#[derive(Debug, Clone)]
pub struct ScannedBook {
    pub path: String,
    pub format: Format,
    pub size: u64,
    pub mtime: i64,
    pub fingerprint: String,
    pub metadata: Metadata,
    pub cover_key: Option<String>,
    pub error: Option<String>,
    /// When the book counts as added; `None` means now. The first import of a
    /// library uses file dates, so "recently added" is meaningful from day one.
    pub added_at: Option<i64>,
}

/// Minimal state of a known book, for incremental scans.
#[derive(Debug, Clone)]
pub struct KnownFile {
    pub id: BookId,
    pub size: u64,
    pub mtime: i64,
    pub missing: bool,
}

pub fn known_files(conn: &Connection) -> Result<std::collections::HashMap<String, KnownFile>> {
    let mut stmt = conn.prepare("SELECT path, id, size, mtime, missing FROM books")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, KnownFile { id: r.get(1)?, size: r.get::<_, i64>(2)? as u64, mtime: r.get(3)?, missing: r.get(4)? }))
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Inserts or updates by path. If a new file matches a missing book by
/// fingerprint it is treated as a move/rename and keeps the id (and hence
/// progress, shelves, favourites…).
pub fn upsert_scanned(tx: &Transaction, b: &ScannedBook) -> Result<(BookId, bool)> {
    let now = now();
    let existing: Option<(BookId, bool)> = tx
        .query_row("SELECT id, user_edited FROM books WHERE path = ?1", [&b.path], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let existing = match existing {
        Some(e) => Some(e),
        None => tx
            .query_row(
                "SELECT id, user_edited FROM books WHERE fingerprint = ?1 AND missing = 1 LIMIT 1",
                [&b.fingerprint],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?,
    };
    let m = &b.metadata;
    let title = m.title.clone().unwrap_or_else(|| "Untitled".into());
    let authors_joined = m.authors.join(&SEP.to_string());
    let author_sort = m.authors.first().map(|a| author_sort_key(a)).unwrap_or_default();
    let tags_joined = m.tags.join(&SEP.to_string());

    let (id, is_new) = match existing {
        Some((id, true)) => {
            // User-edited metadata: only update file data.
            tx.execute(
                "UPDATE books SET path=?2, format=?3, size=?4, mtime=?5, fingerprint=?6, missing=0,
                 cover_key=COALESCE(cover_key, ?7), updated_at=?8, scan_error=?9 WHERE id=?1",
                params![id, b.path, b.format.as_str(), b.size as i64, b.mtime, b.fingerprint, b.cover_key, now, b.error],
            )?;
            return Ok((id, false));
        }
        Some((id, false)) => {
            tx.execute(
                "UPDATE books SET path=?2, format=?3, size=?4, mtime=?5, fingerprint=?6, title=?7, sort_title=?8,
                   authors=?9, author_sort=?10, tags=?11, series=?12, series_index=?13, publisher=?14, language=?15,
                   isbn=?16, published=?17, description=?18, page_count=?19, cover_key=?20, updated_at=?21,
                   missing=0, scan_error=?22
                 WHERE id=?1",
                params![
                    id, b.path, b.format.as_str(), b.size as i64, b.mtime, b.fingerprint, title, sort_key(&title),
                    authors_joined, author_sort, tags_joined, m.series, m.series_index, m.publisher, m.language,
                    m.isbn, m.published, m.description, m.page_count, b.cover_key, now, b.error
                ],
            )?;
            (id, false)
        }
        None => {
            tx.execute(
                "INSERT INTO books (path, format, size, mtime, fingerprint, title, sort_title, authors, author_sort,
                   tags, series, series_index, publisher, language, isbn, published, description, page_count,
                   cover_key, added_at, updated_at, scan_error)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
                params![
                    b.path, b.format.as_str(), b.size as i64, b.mtime, b.fingerprint, title, sort_key(&title),
                    authors_joined, author_sort, tags_joined, m.series, m.series_index, m.publisher, m.language,
                    m.isbn, m.published, m.description, m.page_count, b.cover_key, b.added_at.unwrap_or(now), now,
                    b.error
                ],
            )?;
            (tx.last_insert_rowid(), true)
        }
    };
    set_relations(tx, id, m, is_new)?;
    Ok((id, is_new))
}

/// Rewrites a book's authors, tags and FTS entry.
fn set_relations(tx: &Transaction, id: BookId, m: &Metadata, is_new: bool) -> Result<()> {
    if !is_new {
        tx.execute("DELETE FROM book_authors WHERE book_id=?1", [id])?;
        tx.execute("DELETE FROM book_tags WHERE book_id=?1", [id])?;
        tx.execute("DELETE FROM books_fts WHERE rowid=?1", [id])?;
    }
    {
        let mut ins_author = tx.prepare_cached("INSERT INTO authors(name, sort_name) VALUES (?1, ?2) ON CONFLICT(name) DO NOTHING")?;
        let mut get_author = tx.prepare_cached("SELECT id FROM authors WHERE name = ?1")?;
        let mut link = tx.prepare_cached("INSERT OR IGNORE INTO book_authors(book_id, author_id, position) VALUES (?1,?2,?3)")?;
        for (pos, a) in m.authors.iter().enumerate() {
            ins_author.execute(params![a, author_sort_key(a)])?;
            let aid: i64 = get_author.query_row([a], |r| r.get(0))?;
            link.execute(params![id, aid, pos as i64])?;
        }
    }
    {
        let mut ins_tag = tx.prepare_cached("INSERT INTO tags(name) VALUES (?1) ON CONFLICT(name) DO NOTHING")?;
        let mut get_tag = tx.prepare_cached("SELECT id FROM tags WHERE name = ?1")?;
        let mut link = tx.prepare_cached("INSERT OR IGNORE INTO book_tags(book_id, tag_id) VALUES (?1,?2)")?;
        for t in &m.tags {
            ins_tag.execute([t])?;
            let tid: i64 = get_tag.query_row([t], |r| r.get(0))?;
            link.execute(params![id, tid])?;
        }
    }
    tx.prepare_cached("INSERT INTO books_fts(rowid, title, authors, series, tags, description) VALUES (?1,?2,?3,?4,?5,?6)")?
        .execute(params![
            id,
            m.title.as_deref().unwrap_or(""),
            m.authors.join(" "),
            m.series.as_deref().unwrap_or(""),
            m.tags.join(" "),
            m.description.as_deref().unwrap_or("")
        ])?;
    Ok(())
}

pub fn mark_missing(tx: &Transaction, ids: &[BookId]) -> Result<()> {
    let mut stmt = tx.prepare_cached("UPDATE books SET missing = 1 WHERE id = ?1")?;
    for id in ids {
        stmt.execute([id])?;
    }
    Ok(())
}

/// Deletes authors and tags left without books.
pub fn gc_orphans(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DELETE FROM authors WHERE id NOT IN (SELECT author_id FROM book_authors);
         DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM book_tags);",
    )?;
    Ok(())
}

/// Manual metadata edit: sets `user_edited` so a rescan never overwrites it.
pub fn update_metadata(conn: &mut Connection, id: BookId, m: &Metadata) -> Result<()> {
    let tx = conn.transaction()?;
    let title = m.title.clone().unwrap_or_else(|| "Untitled".into());
    let n = tx.execute(
        "UPDATE books SET title=?2, sort_title=?3, authors=?4, author_sort=?5, tags=?6, series=?7, series_index=?8,
           publisher=?9, language=?10, isbn=?11, published=?12, description=?13, user_edited=1, updated_at=?14
         WHERE id=?1",
        params![
            id, title, sort_key(&title), m.authors.join(&SEP.to_string()),
            m.authors.first().map(|a| author_sort_key(a)).unwrap_or_default(), m.tags.join(&SEP.to_string()),
            m.series, m.series_index, m.publisher, m.language, m.isbn, m.published, m.description, now()
        ],
    )?;
    if n == 0 {
        return Err(crate::Error::NotFound(format!("book {id}")));
    }
    set_relations(&tx, id, m, false)?;
    tx.commit()?;
    Ok(())
}

// ───────────────────────────── Reads (UI) ─────────────────────────────

fn split_list(s: String) -> Vec<String> {
    if s.is_empty() { Vec::new() } else { s.split(SEP).map(str::to_owned).collect() }
}

fn parse_format(s: String) -> Format {
    Format::parse(&s).unwrap_or(Format::Epub)
}

const SUMMARY_COLS: &str =
    "b.id, b.title, b.authors, b.series, b.series_index, b.format, b.cover_key, p.percent, b.favorite";

fn row_to_summary(r: &Row) -> rusqlite::Result<BookSummary> {
    let cover_key: Option<String> = r.get(6)?;
    Ok(BookSummary {
        id: r.get(0)?,
        title: r.get(1)?,
        authors: split_list(r.get(2)?),
        series: r.get(3)?,
        series_index: r.get(4)?,
        format: parse_format(r.get(5)?),
        has_cover: cover_key.is_some(),
        cover_key,
        progress: r.get(7)?,
        favorite: r.get(8)?,
    })
}

/// Turns free user text into a safe FTS5 query: every word quoted and
/// prefix-matched (`"tolk"*`), all of them required.
pub fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(16)
        .map(|t| format!("\"{}\"*", t.replace('"', "")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" AND "))
}

pub fn list_books(conn: &Connection, q: &BookQuery) -> Result<Page<BookSummary>> {
    let mut joins = String::from(" LEFT JOIN progress p ON p.book_id = b.id");
    let mut wheres: Vec<String> = Vec::new();
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();
    let mut relevance = false;

    if let Some(fts) = q.text.as_deref().and_then(fts_query) {
        joins.push_str(" JOIN books_fts f ON f.rowid = b.id");
        wheres.push("books_fts MATCH ?".into());
        args.push(Box::new(fts));
        relevance = true;
    }
    if let Some(a) = q.author_id {
        wheres.push("b.id IN (SELECT book_id FROM book_authors WHERE author_id = ?)".into());
        args.push(Box::new(a));
    }
    if let Some(s) = &q.series {
        wheres.push("b.series = ? COLLATE NOCASE".into());
        args.push(Box::new(s.clone()));
    }
    if let Some(t) = &q.tag {
        wheres.push("b.id IN (SELECT bt.book_id FROM book_tags bt JOIN tags t ON t.id = bt.tag_id WHERE t.name = ?)".into());
        args.push(Box::new(t.clone()));
    }
    if let Some(s) = q.shelf_id {
        wheres.push("b.id IN (SELECT book_id FROM shelf_books WHERE shelf_id = ?)".into());
        args.push(Box::new(s));
    }
    if let Some(f) = q.format {
        wheres.push("b.format = ?".into());
        args.push(Box::new(f.as_str()));
    }
    if q.favorites_only {
        wheres.push("b.favorite = 1".into());
    }
    if q.reading_only {
        wheres.push("p.book_id IS NOT NULL AND p.finished = 0".into());
    }
    if q.finished_only {
        wheres.push("p.finished = 1".into());
    }
    if !q.include_missing {
        wheres.push("b.missing = 0".into());
    }
    let where_sql = if wheres.is_empty() { String::new() } else { format!(" WHERE {}", wheres.join(" AND ")) };

    let order = match q.sort {
        SortBy::Relevance if relevance => "bm25(books_fts, 10.0, 5.0, 3.0, 2.0, 1.0), b.sort_title",
        SortBy::Title | SortBy::Relevance => "b.sort_title, b.id",
        SortBy::Author => "b.author_sort, b.sort_title, b.id",
        SortBy::Series => "b.series IS NULL, b.series COLLATE NOCASE, b.series_index, b.sort_title",
        SortBy::AddedDesc => "b.added_at DESC, b.id DESC",
        SortBy::RecentlyRead => "p.updated_at IS NULL, p.updated_at DESC, b.sort_title",
    };
    let limit = if q.limit == 0 { 100 } else { q.limit.min(1000) };

    // The JOIN with progress only affects the count when filtering by "reading".
    let count_joins = if q.reading_only || q.finished_only { joins.clone() } else { joins.replace(" LEFT JOIN progress p ON p.book_id = b.id", "") };
    let count_sql = format!("SELECT COUNT(*) FROM books b{count_joins}{where_sql}");
    let total: i64 = conn.prepare_cached(&count_sql)?.query_row(params_from_iter(args.iter()), |r| r.get(0))?;

    let sql = format!("SELECT {SUMMARY_COLS} FROM books b{joins}{where_sql} ORDER BY {order} LIMIT {limit} OFFSET {}", q.offset);
    let mut stmt = conn.prepare_cached(&sql)?;
    let items = stmt.query_map(params_from_iter(args.iter()), row_to_summary)?.collect::<std::result::Result<_, _>>()?;
    Ok(Page { items, total: total as u64, offset: q.offset })
}

pub fn get_book(conn: &Connection, id: BookId) -> Result<Book> {
    conn.query_row(
        "SELECT b.id, b.path, b.format, b.size, b.fingerprint, b.title, b.authors, b.tags, b.series, b.series_index,
                b.publisher, b.language, b.isbn, b.published, b.description, b.page_count, b.cover_key, b.added_at,
                b.updated_at, b.missing, b.favorite, b.rating, p.locator, p.percent, p.updated_at, p.finished
         FROM books b LEFT JOIN progress p ON p.book_id = b.id WHERE b.id = ?1",
        [id],
        |r| {
            let cover_key: Option<String> = r.get(16)?;
            let locator: Option<String> = r.get(22)?;
            Ok(Book {
                id: r.get(0)?,
                path: r.get(1)?,
                format: parse_format(r.get(2)?),
                size: r.get::<_, i64>(3)? as u64,
                fingerprint: r.get(4)?,
                metadata: Metadata {
                    title: r.get(5)?,
                    authors: split_list(r.get(6)?),
                    tags: split_list(r.get(7)?),
                    series: r.get(8)?,
                    series_index: r.get(9)?,
                    publisher: r.get(10)?,
                    language: r.get(11)?,
                    isbn: r.get(12)?,
                    published: r.get(13)?,
                    description: r.get(14)?,
                    page_count: r.get(15)?,
                },
                has_cover: cover_key.is_some(),
                cover_key,
                added_at: r.get(17)?,
                updated_at: r.get(18)?,
                missing: r.get(19)?,
                favorite: r.get(20)?,
                rating: r.get(21)?,
                progress: match locator {
                    Some(locator) => Some(ReadingProgress { locator, percent: r.get(23)?, updated_at: r.get(24)?, finished: r.get(25)? }),
                    None => None,
                },
            })
        },
    )
    .optional()?
    .ok_or_else(|| crate::Error::NotFound(format!("book {id}")))
}

pub fn set_progress(conn: &Connection, id: BookId, locator: &str, percent: f64, finished: bool) -> Result<()> {
    conn.execute(
        "INSERT INTO progress(book_id, locator, percent, finished, updated_at) VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(book_id) DO UPDATE SET locator=excluded.locator, percent=excluded.percent,
           finished=excluded.finished, updated_at=excluded.updated_at",
        params![id, locator, percent.clamp(0.0, 1.0), finished, now()],
    )?;
    Ok(())
}

pub fn list_authors(conn: &Connection, prefix: Option<&str>, offset: u32, limit: u32) -> Result<Page<AuthorEntry>> {
    let pattern = format!("{}%", prefix.unwrap_or("").replace(['%', '_'], ""));
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM authors WHERE name LIKE ?1", [&pattern], |r| r.get(0))?;
    let mut stmt = conn.prepare_cached(
        "SELECT a.id, a.name, (SELECT COUNT(*) FROM book_authors ba WHERE ba.author_id = a.id)
         FROM authors a WHERE a.name LIKE ?1 ORDER BY a.sort_name LIMIT ?2 OFFSET ?3",
    )?;
    let items = stmt
        .query_map(params![pattern, limit.clamp(1, 1000), offset], |r| {
            Ok(AuthorEntry { id: r.get(0)?, name: r.get(1)?, book_count: r.get::<_, i64>(2)? as u64 })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(Page { items, total: total as u64, offset })
}

pub fn list_series(conn: &Connection, offset: u32, limit: u32) -> Result<Page<(String, u64)>> {
    let total: i64 = conn.query_row("SELECT COUNT(DISTINCT series COLLATE NOCASE) FROM books WHERE series IS NOT NULL AND missing = 0", [], |r| r.get(0))?;
    let mut stmt = conn.prepare_cached(
        "SELECT series, COUNT(*) FROM books WHERE series IS NOT NULL AND missing = 0
         GROUP BY series COLLATE NOCASE ORDER BY series COLLATE NOCASE LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit.clamp(1, 1000), offset], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(Page { items, total: total as u64, offset })
}

pub fn book_shelves(conn: &Connection, book: BookId) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare_cached("SELECT shelf_id FROM shelf_books WHERE book_id = ?1")?;
    let ids = stmt.query_map([book], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    Ok(ids)
}

pub fn create_shelf(conn: &Connection, name: &str) -> Result<i64> {
    conn.execute("INSERT INTO shelves(name, created_at) VALUES (?1, ?2)", params![name.trim(), now()])?;
    Ok(conn.last_insert_rowid())
}

pub fn add_to_shelf(conn: &Connection, shelf: i64, book: BookId) -> Result<()> {
    conn.execute("INSERT OR IGNORE INTO shelf_books(shelf_id, book_id, added_at) VALUES (?1,?2,?3)", params![shelf, book, now()])?;
    Ok(())
}

pub fn remove_from_shelf(conn: &Connection, shelf: i64, book: BookId) -> Result<()> {
    conn.execute("DELETE FROM shelf_books WHERE shelf_id=?1 AND book_id=?2", params![shelf, book])?;
    Ok(())
}

pub fn list_shelves(conn: &Connection) -> Result<Vec<Shelf>> {
    let mut stmt = conn.prepare_cached(
        "SELECT s.id, s.name, (SELECT COUNT(*) FROM shelf_books sb WHERE sb.shelf_id = s.id) FROM shelves s ORDER BY s.name",
    )?;
    let v = stmt
        .query_map([], |r| Ok(Shelf { id: r.get(0)?, name: r.get(1)?, book_count: r.get::<_, i64>(2)? as u64 }))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(v)
}

pub fn set_favorite(conn: &Connection, id: BookId, fav: bool) -> Result<()> {
    conn.execute("UPDATE books SET favorite=?2 WHERE id=?1", params![id, fav])?;
    Ok(())
}

pub fn set_rating(conn: &Connection, id: BookId, rating: Option<u8>) -> Result<()> {
    conn.execute("UPDATE books SET rating=?2 WHERE id=?1", params![id, rating.map(|r| r.min(5))])?;
    Ok(())
}

pub fn set_cover_key(conn: &Connection, id: BookId, key: &str) -> Result<()> {
    conn.execute("UPDATE books SET cover_key=?2 WHERE id=?1", params![id, key])?;
    Ok(())
}

pub fn stats(conn: &Connection) -> Result<LibraryStats> {
    let mut s = conn.query_row(
        "SELECT COUNT(*) FILTER (WHERE missing=0), COUNT(*) FILTER (WHERE missing=1),
                COUNT(cover_key) FILTER (WHERE missing=0), COALESCE(SUM(size) FILTER (WHERE missing=0), 0),
                COUNT(DISTINCT series COLLATE NOCASE)
         FROM books",
        [],
        |r| {
            Ok(LibraryStats {
                books: r.get::<_, i64>(0)? as u64,
                missing: r.get::<_, i64>(1)? as u64,
                with_cover: r.get::<_, i64>(2)? as u64,
                total_bytes: r.get::<_, i64>(3)? as u64,
                series: r.get::<_, i64>(4)? as u64,
                ..Default::default()
            })
        },
    )?;
    s.authors = conn.query_row("SELECT COUNT(*) FROM authors", [], |r| r.get::<_, i64>(0))? as u64;
    let mut stmt = conn.prepare("SELECT format, COUNT(*) FROM books WHERE missing=0 GROUP BY format ORDER BY 2 DESC")?;
    s.by_format = stmt
        .query_map([], |r| Ok((parse_format(r.get(0)?), r.get::<_, i64>(1)? as u64)))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(s)
}
