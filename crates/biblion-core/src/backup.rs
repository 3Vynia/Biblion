//! Backup and restore of what the reader adds to a library: reading
//! progress, highlights, notes and bookmarks, collections, favourites and
//! ratings, the reading log, and details the user edited (with the covers
//! they chose).
//!
//! Books are referred to by fingerprint, path, and title + first author, so
//! a backup can be restored into a library on another computer, or after
//! the files moved. Restoring merges: nothing already there is deleted, the
//! newer progress wins, and restoring the same backup twice changes nothing.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::db::split_list;
use crate::{BookId, Metadata, Result};

pub const FORMAT_VERSION: u32 = 1;

/// How a backup names a book.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BookRef {
    pub fingerprint: String,
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEntry {
    pub book: usize,
    pub locator: String,
    pub percent: f64,
    pub finished: bool,
    pub updated_at: i64,
    #[serde(default)]
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationEntry {
    pub book: usize,
    pub kind: String,
    pub locator: String,
    pub highlight: Option<String>,
    pub note: Option<String>,
    pub color: Option<String>,
    pub chapter: Option<String>,
    pub percent: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShelfEntry {
    pub name: String,
    pub created_at: i64,
    /// (book, added_at)
    pub books: Vec<(usize, i64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub book: usize,
    pub day: String,
    pub seconds: i64,
    pub pages: i64,
}

/// Details the user edited by hand or picked online.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditEntry {
    pub book: usize,
    pub metadata: Metadata,
    pub updated_at: i64,
    /// A cover the user chose: its thumbnail, JPEG in base64.
    #[serde(default)]
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkEntry {
    pub book: usize,
    pub favorite: bool,
    pub rating: Option<i64>,
}

/// Everything a backup holds about the library. Entries point into `books`
/// by index, so each book is written once.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryBackup {
    pub version: u32,
    pub books: Vec<BookRef>,
    pub progress: Vec<ProgressEntry>,
    pub annotations: Vec<AnnotationEntry>,
    pub shelves: Vec<ShelfEntry>,
    pub marks: Vec<MarkEntry>,
    pub reading_log: Vec<LogEntry>,
    /// Absent in backups made before edits were kept.
    #[serde(default)]
    pub edits: Vec<EditEntry>,
}

/// What a restore did (or, for a preview, would do).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RestoreReport {
    pub books: usize,
    /// Books of the backup found in this library.
    pub matched: usize,
    pub progress: usize,
    pub annotations: usize,
    pub shelves_created: usize,
    pub shelf_books: usize,
    pub marks: usize,
    pub reading_days: usize,
    #[serde(default)]
    pub edits: usize,
    /// A few titles that could not be found, to show the user.
    pub missing_samples: Vec<String>,
}

/// The backup, and the library ids of its `edits` (in the same order), so
/// the caller can add the covers the user chose.
pub fn export(c: &Connection) -> Result<(LibraryBackup, Vec<BookId>)> {
    let mut out = LibraryBackup { version: FORMAT_VERSION, ..Default::default() };
    // Only books something refers to.
    let mut index: HashMap<BookId, usize> = HashMap::new();
    let mut stmt = c.prepare(
        "SELECT id, fingerprint, path, title, authors FROM books b WHERE
           EXISTS (SELECT 1 FROM progress p WHERE p.book_id = b.id)
        OR EXISTS (SELECT 1 FROM bookmarks k WHERE k.book_id = b.id)
        OR EXISTS (SELECT 1 FROM shelf_books s WHERE s.book_id = b.id)
        OR EXISTS (SELECT 1 FROM reading_log l WHERE l.book_id = b.id)
        OR b.favorite = 1 OR b.rating IS NOT NULL OR b.user_edited = 1
        ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, BookId>(0)?,
            BookRef {
                fingerprint: r.get(1)?,
                path: r.get(2)?,
                title: r.get(3)?,
                author: split_list(r.get(4)?).into_iter().next(),
            },
        ))
    })?;
    for row in rows {
        let (id, book) = row?;
        index.insert(id, out.books.len());
        out.books.push(book);
    }
    let at = |id: BookId| index.get(&id).copied();

    let mut stmt = c.prepare("SELECT book_id, locator, percent, finished, updated_at, finished_at FROM progress ORDER BY book_id")?;
    for r in stmt.query_map([], |r| {
        Ok((r.get::<_, BookId>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
    })? {
        let (id, locator, percent, finished, updated_at, finished_at) = r?;
        if let Some(book) = at(id) {
            out.progress.push(ProgressEntry { book, locator, percent, finished, updated_at, finished_at });
        }
    }

    let mut stmt = c.prepare(
        "SELECT book_id, kind, locator, highlight, note, color, chapter, percent, created_at, updated_at FROM bookmarks ORDER BY id",
    )?;
    for r in stmt.query_map([], |r| {
        Ok((
            r.get::<_, BookId>(0)?,
            AnnotationEntry {
                book: 0,
                kind: r.get(1)?,
                locator: r.get(2)?,
                highlight: r.get(3)?,
                note: r.get(4)?,
                color: r.get(5)?,
                chapter: r.get(6)?,
                percent: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
            },
        ))
    })? {
        let (id, mut a) = r?;
        if let Some(book) = at(id) {
            a.book = book;
            out.annotations.push(a);
        }
    }

    let mut shelves = c.prepare("SELECT id, name, created_at FROM shelves ORDER BY id")?;
    let mut members = c.prepare("SELECT book_id, added_at FROM shelf_books WHERE shelf_id = ?1 ORDER BY added_at")?;
    for s in shelves.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))? {
        let (id, name, created_at) = s?;
        let books = members
            .query_map([id], |r| Ok((r.get::<_, BookId>(0)?, r.get::<_, i64>(1)?)))?
            .filter_map(|m| m.ok())
            .filter_map(|(b, added)| at(b).map(|i| (i, added)))
            .collect();
        out.shelves.push(ShelfEntry { name, created_at, books });
    }

    let mut stmt = c.prepare("SELECT id, favorite, rating FROM books WHERE favorite = 1 OR rating IS NOT NULL")?;
    for r in stmt.query_map([], |r| Ok((r.get::<_, BookId>(0)?, r.get(1)?, r.get(2)?)))? {
        let (id, favorite, rating) = r?;
        if let Some(book) = at(id) {
            out.marks.push(MarkEntry { book, favorite, rating });
        }
    }

    let mut stmt = c.prepare("SELECT book_id, day, seconds, pages FROM reading_log ORDER BY day, book_id")?;
    for r in stmt.query_map([], |r| Ok((r.get::<_, BookId>(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))? {
        let (id, day, seconds, pages) = r?;
        if let Some(book) = at(id) {
            out.reading_log.push(LogEntry { book, day, seconds, pages });
        }
    }

    let mut edited = Vec::new();
    let mut stmt = c.prepare("SELECT id, updated_at FROM books WHERE user_edited = 1 ORDER BY id")?;
    for r in stmt.query_map([], |r| Ok((r.get::<_, BookId>(0)?, r.get::<_, i64>(1)?)))? {
        let (id, updated_at) = r?;
        if let Some(book) = at(id) {
            let metadata = crate::db::get_book(c, id)?.metadata;
            out.edits.push(EditEntry { book, metadata, updated_at, cover: None });
            edited.push(id);
        }
    }
    Ok((out, edited))
}

/// Finds the library's book for a reference: by fingerprint, then path, then
/// title and first author (when exactly one book matches).
fn find(tx: &Transaction, b: &BookRef) -> Result<Option<BookId>> {
    if let Some(id) = tx
        .query_row("SELECT id FROM books WHERE fingerprint = ?1 ORDER BY missing, id LIMIT 1", [&b.fingerprint], |r| r.get(0))
        .optional()?
    {
        return Ok(Some(id));
    }
    if let Some(id) = tx.query_row("SELECT id FROM books WHERE path = ?1", [&b.path], |r| r.get(0)).optional()? {
        return Ok(Some(id));
    }
    let sep = crate::db::SEP.to_string();
    let mut stmt = tx.prepare_cached(
        "SELECT id FROM books WHERE title = ?1 COLLATE NOCASE
           AND (?2 IS NULL OR authors = ?2 OR authors LIKE ?2 || ?3 || '%') LIMIT 2",
    )?;
    let ids: Vec<BookId> = stmt.query_map(params![b.title, b.author, sep], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(if ids.len() == 1 { Some(ids[0]) } else { None })
}

/// Merges a backup into the library. With `dry_run` nothing is kept: the
/// report says what would change. Also returns, for edits that were applied
/// and carry a cover, the book and the edit's index.
pub fn restore(c: &mut Connection, backup: &LibraryBackup, dry_run: bool) -> Result<(RestoreReport, Vec<(BookId, usize)>)> {
    let tx = c.transaction()?;
    let mut report = RestoreReport { books: backup.books.len(), ..Default::default() };
    let mut ids: Vec<Option<BookId>> = Vec::with_capacity(backup.books.len());
    for b in &backup.books {
        let id = find(&tx, b)?;
        if id.is_some() {
            report.matched += 1;
        } else if report.missing_samples.len() < 8 {
            report.missing_samples.push(match &b.author {
                Some(a) => format!("{} ({a})", b.title),
                None => b.title.clone(),
            });
        }
        ids.push(id);
    }
    let id_of = |i: usize| ids.get(i).copied().flatten();

    for p in &backup.progress {
        let Some(id) = id_of(p.book) else { continue };
        // Newer progress wins.
        let changed = tx.execute(
            "INSERT INTO progress(book_id, locator, percent, finished, updated_at, finished_at) VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(book_id) DO UPDATE SET locator = excluded.locator, percent = excluded.percent,
               finished = excluded.finished, updated_at = excluded.updated_at, finished_at = excluded.finished_at
             WHERE excluded.updated_at > progress.updated_at",
            params![id, p.locator, p.percent.clamp(0.0, 1.0), p.finished, p.updated_at, p.finished_at],
        )?;
        report.progress += changed;
    }

    {
        let mut exists = tx.prepare("SELECT 1 FROM bookmarks WHERE book_id = ?1 AND kind = ?2 AND locator = ?3")?;
        let mut insert = tx.prepare(
            "INSERT INTO bookmarks(book_id, kind, locator, highlight, note, color, chapter, percent, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        )?;
        for a in &backup.annotations {
            let Some(id) = id_of(a.book) else { continue };
            if exists.exists(params![id, a.kind, a.locator])? {
                continue;
            }
            insert.execute(params![
                id, a.kind, a.locator, a.highlight, a.note, a.color, a.chapter, a.percent, a.created_at, a.updated_at
            ])?;
            report.annotations += 1;
        }
    }

    for s in &backup.shelves {
        let name = s.name.trim();
        if name.is_empty() {
            continue;
        }
        let existing: Option<i64> =
            tx.query_row("SELECT id FROM shelves WHERE name = ?1 COLLATE NOCASE", [name], |r| r.get(0)).optional()?;
        let shelf = match existing {
            Some(id) => id,
            None => {
                tx.execute("INSERT INTO shelves(name, created_at) VALUES (?1, ?2)", params![name, s.created_at])?;
                report.shelves_created += 1;
                tx.last_insert_rowid()
            }
        };
        for &(book, added_at) in &s.books {
            let Some(id) = id_of(book) else { continue };
            report.shelf_books += tx.execute(
                "INSERT OR IGNORE INTO shelf_books(shelf_id, book_id, added_at) VALUES (?1, ?2, ?3)",
                params![shelf, id, added_at],
            )?;
        }
    }

    for m in &backup.marks {
        let Some(id) = id_of(m.book) else { continue };
        report.marks += tx.execute(
            "UPDATE books SET favorite = MAX(favorite, ?2), rating = COALESCE(rating, ?3)
             WHERE id = ?1 AND (favorite < ?2 OR (rating IS NULL AND ?3 IS NOT NULL))",
            params![id, m.favorite, m.rating],
        )?;
    }

    for l in &backup.reading_log {
        let Some(id) = id_of(l.book) else { continue };
        // The larger count wins, so restoring twice never doubles the time.
        report.reading_days += tx.execute(
            "INSERT INTO reading_log(book_id, day, seconds, pages) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(book_id, day) DO UPDATE SET seconds = MAX(seconds, excluded.seconds), pages = MAX(pages, excluded.pages)
             WHERE excluded.seconds > reading_log.seconds OR excluded.pages > reading_log.pages",
            params![id, l.day, l.seconds.max(0), l.pages.max(0)],
        )?;
    }

    let mut covers = Vec::new();
    for (i, e) in backup.edits.iter().enumerate() {
        let Some(id) = id_of(e.book) else { continue };
        // Edits made here since the backup are kept.
        let (edited, updated_at): (bool, i64) =
            tx.query_row("SELECT user_edited, updated_at FROM books WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if edited && updated_at >= e.updated_at {
            continue;
        }
        let mut m = e.metadata.clone();
        crate::formats::util::normalize_metadata(&mut m);
        crate::db::write_metadata(&tx, id, &m)?;
        report.edits += 1;
        if e.cover.is_some() {
            covers.push((id, i));
        }
    }

    if dry_run {
        tx.rollback()?;
    } else {
        tx.commit()?;
    }
    Ok((report, covers))
}
