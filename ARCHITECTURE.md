# Architecture

```
┌──────────────────────── Tauri app (next phase) ────────────────────────────┐
│  Web UI (Svelte/React)                                                     │
│   • Virtualised cover grid    • Reader: foliate-js / pdf.js / comic viewer │
│                         ▲ invoke / events            ▲ asset://thumbs      │
├─────────────────────────┼────────────────────────────┼─────────────────────┤
│  src-tauri: thin #[tauri::command]s → biblion_core::Library                │
└─────────────────────────┼────────────────────────────┼─────────────────────┘
                          ▼                            │
┌──────────────────────── biblion-core ──────────────┼─────────────────────┐
│ library.rs   Facade: 1 writer + pool of readers (Send + Sync)              │
│ scanner.rs   walk → diff → parallel extraction (rayon) → batched writer    │
│ formats/     epub · pdf · mobi/azw3 · comic (cbz/cbr) · util               │
│ covers.rs    360×540 JPEG thumbnails on disk (DCT scaling + SIMD resize)   │
│ db.rs        SQLite WAL + FTS5, migrations, paginated queries              │
└────────────────────────────────────────────────────────────────────────────┘
```

## Key decisions for 200k+ books

**Incremental scanning.** The walk only calls `stat`. A book is reprocessed
only when its size or `mtime` changes, so rescanning 200k unchanged books takes
~1.5 s.

**Moves without losing anything.** Every file gets a BLAKE3 fingerprint of
`size + first 64 KiB + last 64 KiB`. Files that disappear are marked `missing`
instead of being deleted. When a new file shows up with the same fingerprint it
is treated as a move: it keeps its id, progress, favourites and shelves.

**One writer, many readers.** SQLite in WAL mode. The scanner writes batches of
500 per transaction from a single thread while the UI keeps reading through the
connection pool. This is covered by a test.

**Indexes shaped by the UI.** Every grid sort order (title, author, series,
added, format) has an index that starts with `missing`, so the filter and the
`ORDER BY` are served by the same index with no in-memory sort. Authors and
tags are normalised (for browsing) and also denormalised into `books` (to list
a page without JOINs).

**Search.** FTS5 with the `unicode61 remove_diacritics 2` tokenizer and 2- and
3-character prefix indexes. User text is turned into quoted prefix terms
(`"garc"*`), so it can never inject FTS syntax.

**Resilience to broken files.** Every extractor:
- enforces size limits (against zip bombs);
- checks magic bytes;
- runs inside `catch_unwind`.

A corrupt file is recorded with its error and with whatever metadata the file
name provides (`Author - Title.ext`). It never stops the scan.

**Manual edits.** When the user edits a book's metadata it is flagged
`user_edited`, and rescans only update file-level data.

## Formats

| Format | Metadata | Cover | Suggested reader (UI) |
|---|---|---|---|
| EPUB 2/3 | OPF: title, authors (with roles), Calibre and EPUB3 series, ISBN, subjects | `cover-image` manifest item, `meta cover`, heuristics | foliate-js |
| MOBI / AZW3 | Custom PalmDB + MOBI + EXTH parser | EXTH record 201/202 | foliate-js |
| CBZ / CBR | ComicInfo.xml (series, number, writer, genre) | first page (natural order) | custom viewer via `comic_page` |
| PDF | Info dictionary (lopdf), page count, junk titles filtered | rendered by the UI with pdf.js → `Library::set_cover` | pdf.js |

`.cbr` files that are really zips, and vice versa, are detected from their
magic bytes.

## Tauri layer (next phase)

```rust
// src-tauri/src/main.rs (sketch)
struct AppState(Arc<Library>);

#[tauri::command]
fn list_books(state: State<AppState>, query: BookQuery) -> Result<Page<BookSummary>, String> {
    state.0.list_books(&query).map_err(|e| e.to_string())
}

#[tauri::command]
async fn scan(app: AppHandle, state: State<'_, AppState>, roots: Vec<PathBuf>) -> Result<ScanReport, String> {
    let lib = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        lib.scan(&ScanOptions { roots, ..Default::default() }, &|ev| { let _ = app.emit("scan", ev); })
    }).await.unwrap().map_err(|e| e.to_string())
}
// … get_book, set_progress, set_favorite, update_metadata, shelves, comic_page, set_cover…
```

- **Thumbnails** are served straight from disk through the asset protocol
  (`convertFileSrc(thumbnailPath)`), with no bytes going over IPC.
- **Grid:** virtualised (e.g. TanStack Virtual), fetching pages of 200 items
  with `list_books(offset, limit)`.
- **PDF covers:** a background UI task walks `books_without_cover("pdf")`,
  renders page 1 with pdf.js and calls `set_cover`.
- **Progress:** the reader stores an opaque `locator` (CFI for EPUB, page for
  PDF and comics) plus a percentage via `set_progress`.

## To do / ideas

- `notify` to watch folders and rescan on changes.
- Cursor (keyset) pagination if jumps beyond 1M rows are ever needed.
- Duplicates screen (`Library::duplicates`) and broken-files screen (`scan_error`).
- Optional online metadata (Open Library).
