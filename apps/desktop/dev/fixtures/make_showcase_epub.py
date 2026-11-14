"""Builds the EPUB shown in the README's reader screenshot: the opening of
Mary Shelley's Frankenstein (1818), which is in the public domain."""
import sys, zipfile

HEADER = "<p class=\"place\">St. Petersburgh, Dec. 11th, 17—</p><p class=\"place\">To Mrs. Saville, England</p>"
PARAS = [
    "You will rejoice to hear that no disaster has accompanied the commencement of an enterprise which you have regarded "
    "with such evil forebodings. I arrived here yesterday, and my first task is to assure my dear sister of my welfare and "
    "increasing confidence in the success of my undertaking.",
    "I am already far north of London, and as I walk in the streets of Petersburgh, I feel a cold northern breeze play upon "
    "my cheeks, which braces my nerves and fills me with delight. Do you understand this feeling? This breeze, which has "
    "travelled from the regions towards which I am advancing, gives me a foretaste of those icy climes. Inspirited by this "
    "wind of promise, my daydreams become more fervent and vivid. I try in vain to be persuaded that the pole is the seat of "
    "frost and desolation; it ever presents itself to my imagination as the region of beauty and delight. There, Margaret, "
    "the sun is for ever visible, its broad disk just skirting the horizon and diffusing a perpetual splendour.",
    "There—for with your leave, my sister, I will put some trust in preceding navigators—there snow and frost are banished; "
    "and, sailing over a calm sea, we may be wafted to a land surpassing in wonders and in beauty every region hitherto "
    "discovered on the habitable globe. What may not be expected in a country of eternal light? I may there discover the "
    "wondrous power which attracts the needle and may regulate a thousand celestial observations that require only this "
    "voyage to render their seeming eccentricities consistent for ever.",
    "These reflections have dispelled the agitation with which I began my letter, and I feel my heart glow with an "
    "enthusiasm which elevates me to heaven, for nothing contributes so much to tranquillise the mind as a steady "
    "purpose—a point on which the soul may fix its intellectual eye. This expedition has been the favourite dream of my "
    "early years. I have read with ardour the accounts of the various voyages which have been made in the prospect of "
    "arriving at the North Pacific Ocean through the seas which surround the pole.",
    "You may remember that a history of all the voyages made for purposes of discovery composed the whole of our good "
    "Uncle Thomas’ library. My education was neglected, yet I was passionately fond of reading. These volumes were my "
    "study day and night, and my familiarity with them increased that regret which I had felt, as a child, on learning "
    "that my father’s dying injunction had forbidden my uncle to allow me to embark in a seafaring life.",
]

CSS = "h1{font-size:1.6em;margin:0 0 .4em}p.place{font-style:italic;text-align:right;margin:0}p.place+p:not(.place){margin-top:1.4em}"

def chapter(title):
    body = HEADER + "".join(f"<p>{p}</p>" for p in PARAS)
    return f"""<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" lang="en">
<head><title>{title}</title><style>{CSS}</style></head>
<body><h1>{title}</h1>{body}</body></html>"""

out = sys.argv[1]
with zipfile.ZipFile(out, "w") as z:
    z.writestr(zipfile.ZipInfo("mimetype"), "application/epub+zip", compress_type=zipfile.ZIP_STORED)
    z.writestr("META-INF/container.xml", '<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>')
    z.writestr("OEBPS/content.opf", """<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:identifier id="uid">urn:uuid:5d1b8c4e-2f6a-4c3b-8e7d-9a0b1c2d3e4f</dc:identifier>
  <dc:title>Frankenstein</dc:title><dc:creator>Mary Shelley</dc:creator><dc:language>en</dc:language>
  <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
 </metadata>
 <manifest>
  <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
  <item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>
 </manifest>
 <spine><itemref idref="c1"/></spine>
</package>""")
    z.writestr("OEBPS/nav.xhtml", """<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head>
<body><nav epub:type="toc"><ol><li><a href="c1.xhtml">Letter 1</a></li></ol></nav></body></html>""")
    z.writestr("OEBPS/c1.xhtml", chapter("Letter 1"))
print("wrote", out)
