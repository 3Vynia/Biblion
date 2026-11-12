//! Reading statistics: time and pages per day (logged by the reader), and
//! books finished.
//!
//! Days are local dates (`YYYY-MM-DD`) chosen by the UI, which knows the
//! reader's time zone; the core only adds them up.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::{row_to_summary, SUMMARY_COLS};
use crate::{BookId, BookSummary, Error, Result};

/// Most seconds accepted in one log call: the reader flushes every half
/// minute, so anything bigger is a clock jump, not reading.
const MAX_SECONDS_PER_CALL: u32 = 15 * 60;
const MAX_PAGES_PER_CALL: u32 = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DayReading {
    pub day: String,
    pub seconds: u64,
    pub pages: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookTime {
    pub book: BookSummary,
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorTime {
    pub name: String,
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinishedBook {
    pub book: BookSummary,
    /// Unix seconds.
    pub finished_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingStats {
    /// Days with any reading, in order.
    pub days: Vec<DayReading>,
    /// Books finished in the period, newest first.
    pub finished: Vec<FinishedBook>,
    /// Books read the longest in the period.
    pub top_books: Vec<BookTime>,
    pub top_authors: Vec<AuthorTime>,
    /// Different books opened in the period.
    pub books_read: u64,
}

/// Checks a `YYYY-MM-DD` day (so ranges compare as text).
fn valid_day(day: &str) -> bool {
    let b = day.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Adds reading time and page turns to a book's day.
pub fn log(c: &Connection, book: BookId, day: &str, seconds: u32, pages: u32) -> Result<()> {
    if !valid_day(day) {
        return Err(Error::corrupt("reading log", format!("invalid day {day:?}")));
    }
    let (seconds, pages) = (seconds.min(MAX_SECONDS_PER_CALL), pages.min(MAX_PAGES_PER_CALL));
    if seconds == 0 && pages == 0 {
        return Ok(());
    }
    c.execute(
        "INSERT INTO reading_log(book_id, day, seconds, pages) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(book_id, day) DO UPDATE SET seconds = seconds + excluded.seconds, pages = pages + excluded.pages",
        params![book, day, seconds, pages],
    )?;
    Ok(())
}

/// Statistics for the days `from..=to` and books finished between the unix
/// times `finished_from..finished_to`.
pub fn stats(c: &Connection, from: &str, to: &str, finished_from: i64, finished_to: i64, top: u32) -> Result<ReadingStats> {
    if !valid_day(from) || !valid_day(to) {
        return Err(Error::corrupt("reading stats", "invalid day range"));
    }
    let days = {
        let mut stmt = c.prepare(
            "SELECT day, SUM(seconds), SUM(pages) FROM reading_log WHERE day BETWEEN ?1 AND ?2 GROUP BY day ORDER BY day",
        )?;
        let rows = stmt.query_map(params![from, to], |r| {
            Ok(DayReading { day: r.get(0)?, seconds: r.get::<_, i64>(1)? as u64, pages: r.get::<_, i64>(2)? as u64 })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let finished = {
        let mut stmt = c.prepare(&format!(
            "SELECT {SUMMARY_COLS}, p.finished_at FROM progress p JOIN books b ON b.id = p.book_id
             WHERE p.finished = 1 AND p.finished_at >= ?1 AND p.finished_at < ?2 ORDER BY p.finished_at DESC"
        ))?;
        let rows = stmt.query_map(params![finished_from, finished_to], |r| {
            Ok(FinishedBook { book: row_to_summary(r)?, finished_at: r.get(9)? })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let top_books = {
        let mut stmt = c.prepare(&format!(
            "SELECT {SUMMARY_COLS}, t.s FROM (SELECT book_id, SUM(seconds) s FROM reading_log
               WHERE day BETWEEN ?1 AND ?2 GROUP BY book_id ORDER BY s DESC LIMIT ?3) t
             JOIN books b ON b.id = t.book_id LEFT JOIN progress p ON p.book_id = b.id ORDER BY t.s DESC"
        ))?;
        let rows = stmt.query_map(params![from, to, top], |r| {
            Ok(BookTime { book: row_to_summary(r)?, seconds: r.get::<_, i64>(9)? as u64 })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let top_authors = {
        let mut stmt = c.prepare(
            "SELECT a.name, SUM(l.seconds) s FROM reading_log l
             JOIN book_authors ba ON ba.book_id = l.book_id JOIN authors a ON a.id = ba.author_id
             WHERE l.day BETWEEN ?1 AND ?2 GROUP BY a.id ORDER BY s DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![from, to, top], |r| Ok(AuthorTime { name: r.get(0)?, seconds: r.get::<_, i64>(1)? as u64 }))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let books_read: i64 =
        c.query_row("SELECT COUNT(DISTINCT book_id) FROM reading_log WHERE day BETWEEN ?1 AND ?2", params![from, to], |r| r.get(0))?;
    Ok(ReadingStats { days, finished, top_books, top_authors, books_read: books_read as u64 })
}

#[cfg(test)]
mod tests {
    use super::valid_day;

    #[test]
    fn days_are_checked() {
        assert!(valid_day("2026-11-12"));
        assert!(!valid_day("2026-1-12"));
        assert!(!valid_day("2026/11/12"));
        assert!(!valid_day("2026-11-12' OR 1"));
    }
}
