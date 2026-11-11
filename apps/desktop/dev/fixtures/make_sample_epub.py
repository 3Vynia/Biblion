"""Builds a small EPUB 3 used by the UI tests. All text is original."""
import sys, zipfile

PARAS = [
    "The lighthouse keeper counted the waves the way other people counted sheep, and still he could not sleep.",
    "Every evening the lamp turned once for each ship that had ever been lost on the reef, or so the village said.",
    "Mara arrived on the last ferry of autumn, carrying a suitcase of letters that had never been posted.",
    "She asked for a room facing the sea, and the keeper gave her the only one he had: his own.",
    "In the morning the fog sat on the water like a cat on a warm stove, unwilling to move for anyone.",
    "They spoke little at first. The sea did most of the talking, and neither of them minded.",
]

def chapter(n, title, extra=""):
    body = "".join(f"<p>{p}</p>" for p in PARAS * 6)
    return f"""<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">
<head><title>{title}</title></head>
<body><h1>{title}</h1>{extra}{body}</body></html>"""

note = ('<p>The keeper kept a logbook<a epub:type="noteref" href="#n1">1</a> on the windowsill.</p>'
        '<aside epub:type="footnote" id="n1"><p>Its pages smelled of salt and lamp oil.</p></aside>')

out = sys.argv[1]
with zipfile.ZipFile(out, "w") as z:
    z.writestr(zipfile.ZipInfo("mimetype"), "application/epub+zip", compress_type=zipfile.ZIP_STORED)
    z.writestr("META-INF/container.xml", '<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>')
    z.writestr("OEBPS/content.opf", """<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:identifier id="uid">urn:uuid:0b6f3c1e-8f5a-4d7e-9a51-1b2c3d4e5f60</dc:identifier>
  <dc:title>The Keeper of the Reef</dc:title><dc:creator>Biblion Tests</dc:creator><dc:language>en</dc:language>
  <meta property="dcterms:modified">2026-11-11T00:00:00Z</meta>
 </metadata>
 <manifest>
  <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
  <item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>
  <item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/>
  <item id="c3" href="c3.xhtml" media-type="application/xhtml+xml"/>
 </manifest>
 <spine><itemref idref="c1"/><itemref idref="c2"/><itemref idref="c3"/></spine>
</package>""")
    z.writestr("OEBPS/nav.xhtml", """<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head>
<body><nav epub:type="toc"><ol><li><a href="c1.xhtml">The Lamp</a></li><li><a href="c2.xhtml">The Ferry</a></li><li><a href="c3.xhtml">The Fog</a></li></ol></nav></body></html>""")
    z.writestr("OEBPS/c1.xhtml", chapter(1, "The Lamp", note))
    z.writestr("OEBPS/c2.xhtml", chapter(2, "The Ferry"))
    z.writestr("OEBPS/c3.xhtml", chapter(3, "The Fog"))
