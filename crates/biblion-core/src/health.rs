//! Library health: duplicate books, files that could not be read and files
//! that are gone from disk.
//!
//! "Identical" duplicates start from the quick fingerprint (size + first and
//! last 64 KiB) and are confirmed with a full BLAKE3 hash, so two files that
//! only share their edges are never called copies.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use rayon::prelude::*;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::db::{parse_format, split_list};
use crate::{BookId, Format, Page, Result};

/// Most groups returned for one kind of duplicate.
pub const MAX_GROUPS: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateKind {
    /// Byte-for-byte the same file.
    Identical,
    /// Same title and author, different files (another edition or format).
    SameBook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemKind {
    /// The file is there but could not be read fully.
    Unreadable,
    /// The file is no longer on disk.
    Missing,
}

/// A book as the health screen shows it: enough to decide which copy to keep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthBook {
    pub id: BookId,
    pub title: String,
    pub authors: Vec<String>,
    pub format: Format,
    pub cover_key: Option<String>,
    pub path: String,
    pub size: u64,
    pub added_at: i64,
    pub progress: Option<f64>,
    /// Highlights, notes and bookmarks.
    pub annotations: u32,
    pub favorite: bool,
    pub error: Option<String>,
    pub missing: bool,
    #[serde(skip)]
    mtime: i64,
    #[serde(skip)]
    fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub kind: DuplicateKind,
    pub books: Vec<HealthBook>,
    /// Bytes freed by keeping only one file of the group.
    pub wasted: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthCounts {
    pub unreadable: u64,
    pub missing: u64,
}

const COLS: &str = "b.id, b.title, b.authors, b.format, b.cover_key, b.path, b.size, b.added_at, p.percent,
    (SELECT COUNT(*) FROM bookmarks k WHERE k.book_id = b.id), b.favorite, b.scan_error, b.missing, b.mtime, b.fingerprint";

fn row(r: &Row) -> rusqlite::Result<HealthBook> {
    Ok(HealthBook {
        id: r.get(0)?,
        title: r.get(1)?,
        authors: split_list(r.get(2)?),
        format: parse_format(r.get(3)?),
        cover_key: r.get(4)?,
        path: r.get(5)?,
        size: r.get::<_, i64>(6)? as u64,
        added_at: r.get(7)?,
        progress: r.get(8)?,
        annotations: r.get(9)?,
        favorite: r.get(10)?,
        error: r.get(11)?,
        missing: r.get(12)?,
        mtime: r.get(13)?,
        fingerprint: r.get(14)?,
    })
}

pub fn counts(c: &Connection) -> Result<HealthCounts> {
    Ok(c.query_row(
        "SELECT COUNT(*) FILTER (WHERE scan_error IS NOT NULL AND missing = 0), COUNT(*) FILTER (WHERE missing = 1) FROM books",
        [],
        |r| Ok(HealthCounts { unreadable: r.get::<_, i64>(0)? as u64, missing: r.get::<_, i64>(1)? as u64 }),
    )?)
}

pub fn problems(c: &Connection, kind: ProblemKind, offset: u32, limit: u32) -> Result<Page<HealthBook>> {
    let filter = match kind {
        ProblemKind::Unreadable => "b.scan_error IS NOT NULL AND b.missing = 0",
        ProblemKind::Missing => "b.missing = 1",
    };
    let total: i64 = c.query_row(&format!("SELECT COUNT(*) FROM books b WHERE {filter}"), [], |r| r.get(0))?;
    let mut stmt = c.prepare(&format!(
        "SELECT {COLS} FROM books b LEFT JOIN progress p ON p.book_id = b.id WHERE {filter}
         ORDER BY b.path LIMIT ?1 OFFSET ?2"
    ))?;
    let items = stmt.query_map(params![limit, offset], row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Page { items, total: total as u64, offset })
}

/// Candidate groups: books sharing a fingerprint, or a title and author.
fn candidates(c: &Connection, kind: DuplicateKind) -> Result<Vec<Vec<HealthBook>>> {
    let key = match kind {
        DuplicateKind::Identical => "b.fingerprint",
        DuplicateKind::SameBook => "b.sort_title || char(31) || b.author_sort",
    };
    let extra = match kind {
        DuplicateKind::Identical => "",
        DuplicateKind::SameBook => "AND b.sort_title <> '' AND b.author_sort <> ''",
    };
    let mut stmt = c.prepare(&format!(
        "WITH g AS (SELECT {key} AS k FROM books b WHERE b.missing = 0 {extra} GROUP BY k HAVING COUNT(*) > 1 LIMIT ?1)
         SELECT {COLS}, {key} FROM books b JOIN g ON g.k = {key} LEFT JOIN progress p ON p.book_id = b.id
         WHERE b.missing = 0 ORDER BY {key}, b.added_at, b.id"
    ))?;
    let mut groups: Vec<Vec<HealthBook>> = Vec::new();
    let mut last: Option<String> = None;
    let mut rows = stmt.query([MAX_GROUPS * 4])?;
    while let Some(r) = rows.next()? {
        let k: String = r.get(15)?;
        let book = row(r)?;
        if last.as_ref() != Some(&k) {
            groups.push(Vec::new());
            last = Some(k);
        }
        groups.last_mut().unwrap().push(book);
    }
    Ok(groups)
}

/// Full-content hash, cached by (path, size, mtime) across calls.
pub type HashCache = std::sync::Mutex<HashMap<(String, u64, i64), String>>;

fn full_hash(path: &Path) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        match f.read(&mut buf)? {
            0 => break,
            n => {
                h.update(&buf[..n]);
            }
        }
    }
    Ok(h.finalize().to_hex().to_string())
}

pub fn duplicates(c: &Connection, kind: DuplicateKind, cache: &HashCache) -> Result<Vec<DuplicateGroup>> {
    let groups = candidates(c, kind)?;
    let mut out: Vec<DuplicateGroup> = match kind {
        DuplicateKind::Identical => {
            // Hash every candidate once (in parallel), then split the groups.
            let todo: Vec<(String, u64, i64)> = {
                let cached = cache.lock().unwrap();
                groups
                    .iter()
                    .flatten()
                    .map(|b| (b.path.clone(), b.size, b.mtime))
                    .filter(|k| !cached.contains_key(k))
                    .collect()
            };
            let hashed: Vec<((String, u64, i64), String)> =
                todo.into_par_iter().filter_map(|k| full_hash(Path::new(&k.0)).ok().map(|h| (k, h))).collect();
            let cached = {
                let mut cached = cache.lock().unwrap();
                cached.extend(hashed);
                cached.clone()
            };
            let mut out = Vec::new();
            for group in groups {
                let mut by_hash: HashMap<String, Vec<HealthBook>> = HashMap::new();
                for b in group {
                    if let Some(h) = cached.get(&(b.path.clone(), b.size, b.mtime)) {
                        by_hash.entry(h.clone()).or_default().push(b);
                    }
                }
                out.extend(by_hash.into_values().filter(|g| g.len() > 1).map(|books| group_of(DuplicateKind::Identical, books)));
            }
            out
        }
        DuplicateKind::SameBook => groups
            .into_iter()
            // Groups made only of identical files are listed as identical.
            .filter(|g| g.iter().any(|b| b.fingerprint != g[0].fingerprint))
            .map(|books| group_of(DuplicateKind::SameBook, books))
            .collect(),
    };
    match kind {
        DuplicateKind::Identical => out.sort_by(|a, b| b.wasted.cmp(&a.wasted).then(a.books[0].title.cmp(&b.books[0].title))),
        DuplicateKind::SameBook => out.sort_by(|a, b| a.books[0].title.to_lowercase().cmp(&b.books[0].title.to_lowercase())),
    }
    out.truncate(MAX_GROUPS as usize);
    Ok(out)
}

fn group_of(kind: DuplicateKind, mut books: Vec<HealthBook>) -> DuplicateGroup {
    // The copy worth keeping first: read the most, then annotated, then oldest.
    books.sort_by(|a, b| {
        b.progress
            .unwrap_or(0.0)
            .total_cmp(&a.progress.unwrap_or(0.0))
            .then(b.annotations.cmp(&a.annotations))
            .then(b.favorite.cmp(&a.favorite))
            .then(a.added_at.cmp(&b.added_at))
    });
    let total: u64 = books.iter().map(|b| b.size).sum();
    let largest = books.iter().map(|b| b.size).max().unwrap_or(0);
    let wasted = match kind {
        DuplicateKind::Identical => total - books[0].size,
        DuplicateKind::SameBook => total - largest,
    };
    DuplicateGroup { kind, books, wasted }
}

/// Forgets books: their records, progress, notes and collection entries.
/// Files on disk are not touched.
pub fn remove(c: &mut Connection, ids: &[BookId]) -> Result<usize> {
    let tx = c.transaction()?;
    let mut n = 0;
    {
        let mut fts = tx.prepare("DELETE FROM books_fts WHERE rowid = ?1")?;
        let mut del = tx.prepare("DELETE FROM books WHERE id = ?1")?;
        for id in ids {
            fts.execute([id])?;
            n += del.execute([id])?;
        }
    }
    tx.commit()?;
    crate::db::gc_orphans(c)?;
    Ok(n)
}
