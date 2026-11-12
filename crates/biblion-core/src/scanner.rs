//! Incremental, parallel scanner.
//!
//! 1. Walk the folders (only `stat`, files are not opened).
//! 2. Diff against the DB by (path, size, mtime): unchanged files are skipped.
//! 3. Mark vanished files as missing (never deleted, so progress is kept).
//! 4. Extract metadata + cover of new/changed files in parallel (rayon).
//! 5. A single writer thread inserts in batched transactions.
//!
//! Moves/renames are detected by fingerprint (BLAKE3 of size + first and
//! last 64 KiB), so a book keeps its id and history.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use rayon::prelude::*;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::covers::CoverCache;
use crate::db::{self, ScannedBook};
use crate::formats;
use crate::{Error, Format, Result};

const BATCH: usize = 500;
const SAMPLE: u64 = 64 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanOptions {
    pub roots: Vec<PathBuf>,
    /// Extraction threads (default: number of CPUs).
    pub threads: Option<usize>,
    /// Reprocess everything even if unchanged.
    pub force: bool,
    /// Skip the index upkeep (ANALYZE, FTS optimise) after the scan. For the
    /// small, frequent scans of watched folders.
    #[serde(default)]
    pub light: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScanEvent {
    Walking { found: usize },
    Processing { done: usize, total: usize },
    Finished { report: ScanReport },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanReport {
    pub found: usize,
    pub unchanged: usize,
    pub added: usize,
    pub updated: usize,
    pub missing: usize,
    pub errors: usize,
    pub covers: usize,
    pub elapsed_ms: u128,
    /// First errors (path, message) to show in the UI.
    pub error_samples: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct FileEntry {
    path: PathBuf,
    format: Format,
    size: u64,
    mtime: i64,
}

pub fn fingerprint(path: &Path, size: u64) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    h.update(&size.to_le_bytes());
    let mut buf = vec![0u8; SAMPLE as usize];
    let n = read_full(&mut f, &mut buf)?;
    h.update(&buf[..n]);
    if size > SAMPLE * 2 {
        f.seek(SeekFrom::Start(size - SAMPLE))?;
        let n = read_full(&mut f, &mut buf)?;
        h.update(&buf[..n]);
    }
    Ok(h.finalize().to_hex()[..32].to_owned())
}

fn read_full(f: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

fn walk(roots: &[PathBuf], cancel: &AtomicBool, on_event: &(dyn Fn(ScanEvent) + Sync)) -> Result<Vec<FileEntry>> {
    let mut out = Vec::new();
    for root in roots {
        let walker = WalkDir::new(root).follow_links(true).into_iter().filter_entry(|e| {
            // Skip hidden folders and macOS junk.
            e.depth() == 0 || !e.file_name().to_str().is_some_and(|n| n.starts_with('.') || n == "__MACOSX")
        });
        for entry in walker {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            let Some(format) = Format::from_path(entry.path()) else { continue };
            let Ok(meta) = entry.metadata() else { continue };
            let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            out.push(FileEntry { path: entry.into_path(), format, size: meta.len(), mtime });
            if out.len() % 5000 == 0 {
                on_event(ScanEvent::Walking { found: out.len() });
            }
        }
    }
    on_event(ScanEvent::Walking { found: out.len() });
    Ok(out)
}

fn path_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Processes one file: fingerprint, metadata and thumbnail. Never fails:
/// errors are recorded in `ScannedBook::error`.
fn process(entry: &FileEntry, covers: &CoverCache) -> (ScannedBook, bool) {
    let path_str = path_string(&entry.path);
    let fingerprint = match fingerprint(&entry.path, entry.size) {
        Ok(f) => f,
        Err(e) => {
            // Without a fingerprint we cannot detect moves; hash the path instead.
            let fallback = blake3::hash(path_str.as_bytes()).to_hex()[..32].to_owned();
            let ex = formats::fallback(&entry.path);
            return (
                ScannedBook { path: path_str, format: entry.format, size: entry.size, mtime: entry.mtime, fingerprint: fallback, metadata: ex.metadata, cover_key: None, error: Some(e.to_string()), added_at: None },
                false,
            );
        }
    };

    let result = catch_unwind(AssertUnwindSafe(|| formats::extract(&entry.path, entry.format)));
    let (extracted, error) = match result {
        Ok(Ok(ex)) => (ex, None),
        Ok(Err(e)) => (formats::fallback(&entry.path), Some(e.to_string())),
        Err(_) => (formats::fallback(&entry.path), Some("panic while reading the file".to_owned())),
    };

    let mut cover_key = None;
    let mut new_cover = false;
    if covers.has_thumb(&fingerprint) {
        cover_key = Some(fingerprint.clone());
    } else if let Some(bytes) = &extracted.cover {
        match catch_unwind(AssertUnwindSafe(|| covers.store(&fingerprint, bytes))) {
            Ok(Ok(_)) => {
                cover_key = Some(fingerprint.clone());
                new_cover = true;
            }
            Ok(Err(e)) => tracing::debug!("invalid cover in {path_str}: {e}"),
            Err(_) => tracing::warn!("panic while decoding cover of {path_str}"),
        }
    }

    (
        ScannedBook { path: path_str, format: entry.format, size: entry.size, mtime: entry.mtime, fingerprint, metadata: extracted.metadata, cover_key, error, added_at: None },
        new_cover,
    )
}

pub fn scan(
    conn: &mut Connection,
    covers: &CoverCache,
    opts: &ScanOptions,
    cancel: &AtomicBool,
    on_event: &(dyn Fn(ScanEvent) + Sync),
) -> Result<ScanReport> {
    let started = Instant::now();
    let mut report = ScanReport::default();

    // 1. Walk
    let files = walk(&opts.roots, cancel, on_event)?;
    report.found = files.len();

    // 2. Diff against the DB
    let known = db::known_files(conn)?;
    let first_import = known.is_empty();
    let mut seen: HashSet<&str> = HashSet::with_capacity(files.len());
    let mut todo: Vec<&FileEntry> = Vec::new();
    let mut revived: Vec<i64> = Vec::new();
    let path_strings: Vec<String> = files.iter().map(|f| path_string(&f.path)).collect();
    for (f, p) in files.iter().zip(&path_strings) {
        seen.insert(p.as_str());
        match known.get(p) {
            Some(k) if !opts.force && k.size == f.size && k.mtime == f.mtime => {
                report.unchanged += 1;
                if k.missing {
                    revived.push(k.id);
                }
            }
            _ => todo.push(f),
        }
    }

    // 3. Missing files (only inside the scanned roots)
    let roots: Vec<String> = opts.roots.iter().map(|r| path_string(r)).collect();
    let gone: Vec<i64> = known
        .iter()
        .filter(|(p, k)| !k.missing && !seen.contains(p.as_str()) && roots.iter().any(|r| Path::new(p.as_str()).starts_with(r)))
        .map(|(_, k)| k.id)
        .collect();
    report.missing = gone.len();
    {
        let tx = conn.transaction()?;
        db::mark_missing(&tx, &gone)?;
        let mut stmt = tx.prepare("UPDATE books SET missing = 0 WHERE id = ?1")?;
        for id in &revived {
            stmt.execute([id])?;
        }
        drop(stmt);
        tx.commit()?;
    }

    // 4–5. Parallel extraction + single writer
    let total = todo.len();
    on_event(ScanEvent::Processing { done: 0, total });
    let threads = opts.threads.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("biblion-scan-{i}"))
        .build()
        .map_err(|e| Error::corrupt("scanner", e))?;
    let (tx_chan, rx_chan) = crossbeam_channel::bounded::<(ScannedBook, bool)>(BATCH * 2);

    let write_result: Result<()> = std::thread::scope(|s| {
        s.spawn(|| {
            pool.install(|| {
                todo.par_iter().for_each_with(tx_chan, |chan, entry| {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let item = process(entry, covers);
                    let _ = chan.send(item);
                });
            });
        });

        let mut done = 0usize;
        let mut last_event = Instant::now();
        let mut batch: Vec<(ScannedBook, bool)> = Vec::with_capacity(BATCH);
        let flush = |conn: &mut Connection, batch: &mut Vec<(ScannedBook, bool)>, report: &mut ScanReport| -> Result<()> {
            let tx = conn.transaction()?;
            for (book, new_cover) in batch.drain(..) {
                if let Some(e) = &book.error {
                    report.errors += 1;
                    if report.error_samples.len() < 50 {
                        report.error_samples.push((book.path.clone(), e.clone()));
                    }
                }
                report.covers += new_cover as usize;
                let (_, is_new) = db::upsert_scanned(&tx, &book)?;
                if is_new { report.added += 1 } else { report.updated += 1 }
            }
            tx.commit()?;
            Ok(())
        };
        let now = db::now();
        for mut item in rx_chan.iter() {
            if first_import {
                item.0.added_at = Some(item.0.mtime.clamp(0, now));
            }
            batch.push(item);
            done += 1;
            if batch.len() >= BATCH {
                flush(conn, &mut batch, &mut report)?;
            }
            if last_event.elapsed() > Duration::from_millis(200) {
                on_event(ScanEvent::Processing { done, total });
                last_event = Instant::now();
            }
        }
        flush(conn, &mut batch, &mut report)?;
        on_event(ScanEvent::Processing { done, total });
        Ok(())
    });
    write_result?;

    // A "moved" book was updated from its missing record, so recount.
    report.missing = conn.query_row("SELECT COUNT(*) FROM books WHERE missing = 1", [], |r| r.get::<_, i64>(0))? as usize;
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    db::gc_orphans(conn)?;
    if report.added + report.updated > 0 && !opts.light {
        // Planner statistics + compact the FTS index after changes.
        conn.execute_batch("ANALYZE; INSERT INTO books_fts(books_fts) VALUES('optimize');")?;
    }
    report.elapsed_ms = started.elapsed().as_millis();
    on_event(ScanEvent::Finished { report: report.clone() });
    Ok(report)
}
