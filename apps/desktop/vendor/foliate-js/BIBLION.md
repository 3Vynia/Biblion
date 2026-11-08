# Vendored foliate-js

- Upstream: https://github.com/johnfactotum/foliate-js
- Commit: 78914aef4466eb960965702401634c2cb348e9b1 (2026-05-01)
- License: MIT (see `LICENSE`)

Only the modules reachable from `view.js` are copied, unmodified, except:

- `paginator.js`: `expand()` returns early when the document is already gone
  (a resize observer can fire after the reader closes). Marked `// Biblion:`.

- `pdf.js` is a stub: Biblion opens PDFs with its own pdf.js reader, so the
  13 MB `vendor/pdfjs` folder is not included.

To update: copy the same files from a newer upstream commit and update the
commit hash above.
