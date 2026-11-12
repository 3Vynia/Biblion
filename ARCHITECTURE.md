# Architecture

```
┌──────────────────────── apps/desktop (Tauri v2) ───────────────────────────┐
│  Web UI (Svelte 5 + TypeScript)                                            │
│   • Virtualised cover grid    • Reader: foliate-js / pdf.js / comic viewer │
│                      ▲ invoke / Channel        ▲ thumb:// · book://        │
├─────────────────────────┼────────────────────────────┼─────────────────────┤
│  src-tauri: thin async #[tauri::command]s → biblion_core::Library          │
└─────────────────────────┼────────────────────────────┼─────────────────────┘
                          ▼                            │
┌──────────────────────── biblion-core ──────────────┼─────────────────────┐
│ library.rs   Facade: scan writer + user writer + reader pool (Send + Sync) │
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

**Writers that never block the UI.** SQLite in WAL mode. The scanner writes
batches of 500 per transaction from a single thread while the UI keeps reading
through the connection pool. User changes (progress, favourites, ratings,
metadata, shelves, covers) go through a second write connection: SQLite still
runs one write transaction at a time, but since the scanner commits every
batch, a progress save waits milliseconds instead of the whole scan. Both
behaviours are covered by tests.

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

## Desktop app (`apps/desktop`)

```
apps/desktop/
├─ src/                    Svelte 5 + TypeScript UI
│  ├─ lib/api.ts           typed wrappers around the commands
│  ├─ lib/types.ts         mirror of the serde types in biblion-core
│  ├─ lib/pageCache.ts     paged LRU cache behind the grid (unit-tested)
│  ├─ lib/gridLayout.ts    grid geometry (unit-tested)
│  ├─ components/          CoverGrid · BookDetail · ScanPanel
│  └─ reader/              EpubReader (foliate-js)
├─ vendor/foliate-js/      pinned copy, see BIBLION.md
└─ src-tauri/              crate `biblion-desktop` (workspace member)
   ├─ commands.rs          #[tauri::command]s → Library
   ├─ protocols.rs         thumb:// and book:// schemes
   └─ settings.rs          library folders (settings.json)
```

- **Commands are `async`** and run SQLite work on the blocking pool:
  synchronous commands would run on the main thread and freeze the window.
  Errors reach the UI as `{ code, message }` (`cancelled`, `not_found`,
  `invalid_folder`…).
- **Scan progress** streams through a Tauri `Channel`, throttled to ~10
  events/s. While a scan runs the grid refreshes every 2 s, keeping the old
  pages on screen until the new ones arrive (no flicker).
- **Thumbnails** are served from disk by the `thumb://localhost/<cover_key>`
  scheme: no bytes over IPC and no asset-protocol scope to manage. Keys are
  validated, so the scheme cannot read arbitrary files.
- **Book files** are served by `book://localhost/<book_id>`; only ids that
  exist in the database resolve. `Range` requests are supported (206 with
  `Content-Range`), so pdf.js reads a 500 MB PDF piece by piece through a
  `PDFDataRangeTransport` instead of loading it whole.
- **Comic pages** are served by `comic://localhost/<book_id>/<page>` straight
  into `<img>` tags; the comic reader shows spreads (cover alone, then pairs),
  manga order or a continuous strip.
- **Grid:** custom virtualisation (only visible rows plus overscan are in
  the DOM), pages of 200 books from `list_books(offset, limit)`, at most 12
  pages in memory, stale responses from a previous search are dropped.
- **Library folders** are canonicalised and never nested, so no file is
  walked twice. `settings.json` is written atomically; a corrupt file is set
  aside and the defaults are used.
- **Reader:** foliate-js renders EPUB, MOBI and AZW3. Book content runs in
  iframes, and the CSP (`script-src 'self'`) keeps scripts inside books from
  running. The position is saved as an EPUB CFI plus a percentage via
  `set_progress`, debounced to once per second and flushed on close.
- **Highlights, notes and bookmarks** live in the `bookmarks` table (v3
  migration). A highlight stores its CFI range, the passage and an optional
  note; foliate-js draws it through its overlayer and reports clicks on it.
  The Notes screen lists them across the library (newest first, filterable by
  kind, colour and text) and opens the book at the exact place.
- **Edits are always confirmed.** Renaming or deleting collections, taking
  books out of a collection, deleting highlights or bookmarks and changing an
  existing note go through a shared confirmation dialog.
- **UI tests:** `dev/mock.html` runs the real UI in a browser against an
  in-memory imitation of the commands (`dev/mockBackend.ts`), with a small
  original EPUB in `dev/fixtures`. `pnpm e2e` drives it with Playwright.
- **PDF reader:** pdf.js 6 with a virtualised column of pages (only pages
  near the screen keep a canvas), a text layer for selection, copy and the
  dictionary, outline, search with highlights, and night themes that invert
  the page. The reading position survives zoom and window changes.
- **PDF covers:** a background job walks `books_without_cover("pdf")`,
  renders page 1 and calls `set_cover`. It pauses while a book is open and
  never retries a PDF that failed in the same session.
- **Dictionaries:** StarDict files (`.ifo` + `.idx[.gz]` + `.dict[.dz]`) in
  the data folder's `dictionaries/`, installed from `.ifo`, `.zip`,
  `.tar.gz` or `.tar.xz` (FreeDict). dictzip files are read chunk by chunk.
  When nothing is found offline the UI can ask Wiktionary (only the word is
  sent; it can be turned off). Definitions are always shown as plain text.
- **Ambience** (`src/lib/ambience`): every sound is generated live with Web
  Audio, so there are no audio files to ship. The soundscape plays slow
  chords and sparse bell notes in a mode (read, focus, relax, sleep) that
  follows the time of day and answers page turns; textures (rain, fire, sea,
  noise…) are shaped noise. Everything goes through a gentle high shelf and
  a limiter. Scenes are saved in `settings.json`. On Linux, WebKitGTK needs
  the GStreamer base and good plugins to play sound.

- **Folder watching** (`src-tauri/src/watcher.rs`): `notify` follows the
  library folders. Events are gathered until things are quiet for 4 s (at
  most 60 s while a download keeps writing), reduced to the fewest folders
  that cover them, and those folders get a light incremental scan (no
  ANALYZE or FTS optimise). The UI hears `library-changed`. It can be turned
  off in Settings.
- **Authors from paths:** a book with no author gets one from
  `Author_Name/Title.epub` or `Title-Author_Name.epub` when it reads like a
  person's name (capitalised words, particles like "de", no genre words such
  as "Ciencia Ficción"). Migration v5 applies this to existing libraries.

## To do / ideas

- Cursor (keyset) pagination if jumps beyond 1M rows are ever needed.
- Duplicates screen (`Library::duplicates`) and broken-files screen (`scan_error`).
- Optional online metadata (Open Library).
