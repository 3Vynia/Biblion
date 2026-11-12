"""Builds a small PDF used by the UI tests: 5 text pages and an outline. All text is original."""
import sys

LINES = [
    "The tide came in twice a day, and twice a day the keeper",
    "walked down to the rocks to see what it had brought.",
    "Mostly it brought weed and driftwood, sometimes a bottle,",
    "once a single shoe that he kept on the windowsill.",
]

def page_stream(n):
    parts = ["BT /F1 22 Tf 72 740 Td (Chapter %d) Tj ET" % n]
    y = 690
    for i in range(18):
        line = LINES[i % len(LINES)]
        if n == 3 and i == 5:
            line = "The lighthouse logbook was kept on the windowsill."
        parts.append("BT /F1 12 Tf 72 %d Td (%s) Tj ET" % (y, line))
        y -= 28
    return "\n".join(parts).encode()

objects = {}
pages = 5
# 1 catalog, 2 pages, 3 font, 4 outlines, 5.. page/content pairs, then outline items
page_ids = [5 + 2 * i for i in range(pages)]
content_ids = [6 + 2 * i for i in range(pages)]
outline_ids = [5 + 2 * pages + i for i in range(3)]
objects[1] = b"<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R /PageMode /UseOutlines >>"
objects[2] = ("<< /Type /Pages /Count %d /Kids [%s] >>" % (pages, " ".join("%d 0 R" % p for p in page_ids))).encode()
objects[3] = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"
objects[4] = ("<< /Type /Outlines /First %d 0 R /Last %d 0 R /Count 3 >>" % (outline_ids[0], outline_ids[-1])).encode()
for i in range(pages):
    stream = page_stream(i + 1)
    objects[page_ids[i]] = ("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents %d 0 R >>" % content_ids[i]).encode()
    objects[content_ids[i]] = b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream"
titles = [("Chapter One", 0), ("Chapter Three", 2), ("Chapter Five", 4)]
for k, (title, page) in enumerate(titles):
    prev = (" /Prev %d 0 R" % outline_ids[k - 1]) if k else ""
    nxt = (" /Next %d 0 R" % outline_ids[k + 1]) if k + 1 < len(titles) else ""
    objects[outline_ids[k]] = ("<< /Title (%s) /Parent 4 0 R /Dest [%d 0 R /Fit]%s%s >>" % (title, page_ids[page], prev, nxt)).encode()

out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
offsets = {}
for num in sorted(objects):
    offsets[num] = len(out)
    out += b"%d 0 obj\n" % num + objects[num] + b"\nendobj\n"
xref = len(out)
count = max(objects) + 1
out += b"xref\n0 %d\n0000000000 65535 f \n" % count
for num in range(1, count):
    out += b"%010d 00000 n \n" % offsets[num]
out += b"trailer\n<< /Size %d /Root 1 0 R /Info << /Title (The Tide Logbook) /Author (Biblion Tests) >> >>\nstartxref\n%d\n%%%%EOF\n" % (count, xref)
open(sys.argv[1], "wb").write(out)
