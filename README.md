# Biblion

A local digital library manager written in Rust, built for collections of
**200,000+ books**: EPUB, PDF, MOBI, AZW3 and CBZ/CBR comics.

This repository contains the **core** (`biblion-core`) and a **command-line
tool** (`biblion`). The desktop app (Tauri + web UI) builds on
`biblion_core::Library`; see [ARCHITECTURE.md](ARCHITECTURE.md).

## Measured performance (200,000 synthetic books, 9.7 GB, 4 cores)

| Operation | Time |
|---|---|
| First full scan (metadata + 146k thumbnails) | 2 min 55 s (~1,150 books/s) |
| Rescan with no changes | 1.5 s |
| Page of 100 books, any sort order | 3–5 ms |
| Page at offset 100,000 | 8 ms |
| Full-text search (relevance) | 4–17 ms |
| Peak RAM while scanning | 220 MB |
| Database / thumbnail cache | 191 MB / 3.3 GB |

## Quick start

```bash
cargo build --release
B=./target/release/biblion

$B --data ~/biblion-data scan /path/to/my/books   # incremental; rescanning is cheap
$B --data ~/biblion-data search "garcia marquez"  # accent-insensitive, prefix match
$B --data ~/biblion-data list --sort author --limit 50
$B --data ~/biblion-data show 123                 # full record as JSON
$B --data ~/biblion-data stats

# Load testing
$B gen-test /tmp/books --count 200000
$B --data /tmp/db scan /tmp/books && $B --data /tmp/db bench
```

## Desktop app

Requirements: Rust, Node.js, pnpm and the
[Tauri v2 system dependencies](https://v2.tauri.app/start/prerequisites/)
(`webkit2gtk-4.1`, `librsvg`… on Linux).

```bash
cd apps/desktop
pnpm install
pnpm tauri dev      # development
pnpm tauri build    # installable bundles
pnpm test           # UI unit tests
pnpm e2e            # UI end-to-end tests (mock backend, see dev/)
```

Set `BIBLION_DATA_DIR=/some/folder` to use a separate library while testing.

## Tests

```bash
cargo test
cargo run --release -p biblion-core --example thumb_bench
```

## Status

Work in progress. The core and the CLI work. The desktop app has a home
screen with "continue reading", a virtualised cover grid with filters,
authors, series, subjects and collections, full book details, an
EPUB/MOBI/AZW3 reader with themes, highlights, notes, bookmarks and a
dictionary, a PDF reader (zoom, contents, search, night mode), a comic reader
(spreads, manga, strip) and a Notes screen for the whole library.

## License

No license yet: © Crystal, all rights reserved.
