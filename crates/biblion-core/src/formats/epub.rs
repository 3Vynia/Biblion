//! EPUB 2 and 3: OPF metadata (including Calibre and EPUB3 series) and cover.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use roxmltree::{Document, Node};
use zip::ZipArchive;

use super::util::{zip_names, is_image_name, looks_like_image, read_limited, resolve_zip_path};
use super::{Extracted, MAX_COVER_BYTES, MAX_META_BYTES};
use crate::{Contributor, Error, Identifier, Metadata, Result};

type Zip = ZipArchive<BufReader<File>>;

pub fn open(path: &Path) -> Result<Zip> {
    let f = File::open(path).map_err(|e| Error::io(path, e))?;
    Ok(ZipArchive::new(BufReader::new(f))?)
}

pub fn read_entry(zip: &mut Zip, name: &str, limit: u64) -> Result<Vec<u8>> {
    // Some EPUBs have inconsistent casing between the OPF and the zip.
    let real = if zip.index_for_name(name).is_some() {
        name.to_owned()
    } else {
        zip_names(zip)
            .into_iter()
            .find(|n| n.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::corrupt("epub", format!("missing {name}")))?
    };
    let entry = zip.by_name(&real)?;
    read_limited(entry, limit).map_err(|e| Error::corrupt("epub", e))
}

fn local<'a>(n: &Node<'a, '_>) -> &'a str {
    n.tag_name().name()
}

fn text(n: &Node) -> Option<String> {
    let t: String = n.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect();
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// Path of the OPF inside the zip.
pub fn opf_path(zip: &mut Zip) -> Result<String> {
    let container = read_entry(zip, "META-INF/container.xml", MAX_META_BYTES)?;
    let container = String::from_utf8_lossy(&container);
    let doc = Document::parse(strip_bom(&container)).map_err(|e| Error::corrupt("epub", e))?;
    doc.descendants()
        .find(|n| local(n) == "rootfile")
        .and_then(|n| n.attribute("full-path"))
        .map(str::to_owned)
        .or_else(|| zip_names(zip).into_iter().find(|n| n.to_ascii_lowercase().ends_with(".opf")))
        .ok_or_else(|| Error::corrupt("epub", "no OPF found"))
}

fn strip_bom(s: &str) -> &str {
    s.trim_start_matches('\u{feff}')
}

pub fn extract(path: &Path) -> Result<Extracted> {
    let mut zip = open(path)?;
    let opf_name = opf_path(&mut zip)?;
    let opf_dir = opf_name.rsplit_once('/').map(|(d, _)| d).unwrap_or("").to_owned();
    let opf_bytes = read_entry(&mut zip, &opf_name, MAX_META_BYTES)?;
    let opf_str = String::from_utf8_lossy(&opf_bytes);
    let doc = Document::parse_with_options(
        strip_bom(&opf_str),
        roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() },
    )
    .map_err(|e| Error::corrupt("epub", e))?;

    let metadata = parse_opf_metadata(&doc);
    let cover = find_cover_href(&doc)
        .map(|href| resolve_zip_path(&opf_dir, &href))
        .and_then(|p| cover_from_entry(&mut zip, &p))
        .or_else(|| fallback_cover(&mut zip));

    Ok(Extracted { metadata, cover })
}

fn attr<'a>(n: &Node<'a, '_>, name: &str) -> Option<&'a str> {
    // Namespaced EPUB 2 attributes (opf:role, opf:scheme, opf:event) match by local name.
    n.attributes().find(|a| a.name() == name).map(|a| a.value())
}

fn parse_opf_metadata(doc: &Document) -> Metadata {
    let mut m = Metadata::default();
    if let Some(v) = doc.root_element().attribute("version") {
        m.format_version = Some(format!("EPUB {}", v.trim()));
    }
    let Some(meta) = doc.descendants().find(|n| local(n) == "metadata") else {
        return m;
    };
    // EPUB3: <meta refines="#id" property="..."> attached to elements.
    let refines = |id: Option<&str>, prop: &str| -> Option<String> {
        let id = id?;
        meta.children()
            .filter(|n| local(n) == "meta")
            .find(|n| {
                n.attribute("refines").map(|r| r.trim_start_matches('#')) == Some(id)
                    && n.attribute("property") == Some(prop)
            })
            .and_then(|n| text(&n))
    };

    let mut titles: Vec<(Option<String>, String)> = Vec::new();
    let mut creators: Vec<(i32, String)> = Vec::new();
    let mut modified_epub3: Option<String> = None;
    for n in meta.children().filter(|n| n.is_element()) {
        let id = n.attribute("id");
        match local(&n) {
            "title" => {
                if let Some(t) = text(&n) {
                    titles.push((refines(id, "title-type"), t));
                }
            }
            tag @ ("creator" | "contributor") => {
                let Some(name) = text(&n) else { continue };
                let role = attr(&n, "role")
                    .map(str::to_owned)
                    .or_else(|| refines(id, "role"))
                    .map(|r| r.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| if tag == "creator" { "aut" } else { "ctb" }.to_owned());
                if role == "aut" {
                    let order = refines(id, "display-seq").and_then(|s| s.parse().ok()).unwrap_or(1000);
                    creators.push((order, name));
                } else if !m.contributors.iter().any(|c| c.name == name && c.role == role) {
                    m.contributors.push(Contributor { name, role });
                }
            }
            "publisher" => m.publisher = m.publisher.take().or_else(|| text(&n)),
            "language" => m.language = m.language.take().or_else(|| text(&n)),
            "description" => m.description = m.description.take().or_else(|| text(&n)),
            "rights" => m.rights = m.rights.take().or_else(|| text(&n)),
            "source" => m.source = m.source.take().or_else(|| text(&n)),
            "subject" => m.tags.extend(text(&n)),
            "date" => match attr(&n, "event").map(str::to_ascii_lowercase).as_deref() {
                Some("modification") => m.modified = m.modified.take().or_else(|| text(&n)),
                Some("creation") | Some("original-publication") | Some("publication") | None => {
                    m.published = m.published.take().or_else(|| text(&n))
                }
                Some(_) => {}
            },
            "identifier" => {
                if let Some(ident) = parse_identifier(&n, refines(id, "identifier-type")) {
                    if !m.identifiers.contains(&ident) {
                        m.identifiers.push(ident);
                    }
                }
            }
            "meta" => {
                match (n.attribute("name"), n.attribute("content")) {
                    (Some("calibre:series"), Some(c)) => m.series = Some(c.to_owned()),
                    (Some("calibre:series_index"), Some(c)) => m.series_index = c.trim().parse().ok(),
                    _ => {}
                }
                match n.attribute("property") {
                    Some("dcterms:modified") => modified_epub3 = text(&n),
                    Some("belongs-to-collection") if m.series.is_none() => {
                        let ctype = refines(id, "collection-type");
                        if ctype.as_deref().is_none_or(|t| t == "series") {
                            m.series = text(&n);
                            m.series_index = refines(id, "group-position").and_then(|s| s.trim().parse().ok());
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // Main title: the one marked "main", else the first. Subtitle only when marked.
    let main = titles.iter().position(|(t, _)| t.as_deref() == Some("main")).unwrap_or(0);
    m.title = titles.get(main).map(|(_, t)| t.clone());
    m.subtitle = titles
        .iter()
        .enumerate()
        .find(|(i, (t, _))| *i != main && t.as_deref() == Some("subtitle"))
        .map(|(_, (_, t))| t.clone());
    m.modified = modified_epub3.or(m.modified);
    m.isbn = m.identifiers.iter().find(|i| i.scheme == "isbn").map(|i| i.value.clone());
    creators.sort_by_key(|(o, _)| *o);
    m.authors = creators.into_iter().map(|(_, a)| a).collect();
    m
}

/// Works out the scheme of a `<dc:identifier>` from its attributes, EPUB3
/// refinements (ONIX code list 5) and well-known prefixes.
fn parse_identifier(n: &Node, refined_type: Option<String>) -> Option<Identifier> {
    let raw = text(n)?;
    let lower = raw.to_ascii_lowercase();
    let declared = attr(n, "scheme").map(str::to_ascii_lowercase);
    let onix = refined_type.as_deref().map(|t| match t.trim() {
        "02" | "15" => "isbn",
        "06" => "doi",
        _ => "other",
    });

    const PREFIXES: [(&str, &str); 10] = [
        ("urn:isbn:", "isbn"),
        ("isbn:", "isbn"),
        ("urn:uuid:", "uuid"),
        ("uuid:", "uuid"),
        ("urn:doi:", "doi"),
        ("doi:", "doi"),
        ("urn:asin:", "asin"),
        ("asin:", "asin"),
        ("calibre:", "calibre"),
        ("google:", "google"),
    ];
    let (prefix_scheme, value) = PREFIXES
        .iter()
        .find(|(p, _)| lower.starts_with(p))
        .map(|(p, sch)| (Some(*sch), raw[p.len()..].trim().to_owned()))
        .unwrap_or((None, raw.clone()));

    let digits: String = value.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let looks_isbn = (digits.len() == 10 || digits.len() == 13)
        && digits[..digits.len() - 1].chars().all(|c| c.is_ascii_digit())
        && (digits.starts_with("978") || digits.starts_with("979") || digits.len() == 10)
        && value.chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' ' || c == 'X' || c == 'x');
    let looks_uuid = value.len() == 36 && value.chars().filter(|&c| c == '-').count() == 4;

    let scheme = match (prefix_scheme, declared.as_deref(), onix) {
        (Some(s), _, _) => s.to_owned(),
        (_, Some("isbn"), _) | (_, _, Some("isbn")) => "isbn".to_owned(),
        (_, Some("amazon" | "asin" | "mobi-asin"), _) => "asin".to_owned(),
        (_, Some("doi"), _) | (_, _, Some("doi")) => "doi".to_owned(),
        (_, Some("uuid"), _) => "uuid".to_owned(),
        _ if looks_isbn => "isbn".to_owned(),
        _ if looks_uuid => "uuid".to_owned(),
        (_, Some(other), _) => other.to_owned(),
        _ => "other".to_owned(),
    };
    Some(Identifier { scheme, value })
}

/// Upper bounds for analysis, against huge or malicious files.
const MAX_SECTION_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 256 * 1024 * 1024;

/// Counts words, characters, chapters and images of an EPUB.
pub fn analyze(path: &Path) -> Result<crate::BookStats> {
    let mut zip = open(path)?;
    let opf_name = opf_path(&mut zip)?;
    let opf_dir = opf_name.rsplit_once('/').map(|(d, _)| d).unwrap_or("").to_owned();
    let opf_bytes = read_entry(&mut zip, &opf_name, MAX_META_BYTES)?;
    let opf_str = String::from_utf8_lossy(&opf_bytes);
    let doc = Document::parse_with_options(strip_bom(&opf_str), roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
        .map_err(|e| Error::corrupt("epub", e))?;

    let manifest: Vec<Node> = doc.descendants().filter(|n| local(n) == "item").collect();
    let href_of = |id: &str| manifest.iter().find(|n| n.attribute("id") == Some(id)).and_then(|n| n.attribute("href"));
    let images = manifest
        .iter()
        .filter(|n| n.attribute("media-type").is_some_and(|t| t.starts_with("image/")))
        .count() as u32;
    let spine_node = doc.descendants().find(|n| local(n) == "spine");
    let spine: Vec<String> = doc
        .descendants()
        .filter(|n| local(n) == "itemref")
        .filter_map(|n| n.attribute("idref"))
        .filter_map(href_of)
        .map(|h| resolve_zip_path(&opf_dir, h))
        .collect();

    let (mut words, mut characters, mut total) = (0u64, 0u64, 0u64);
    for section in &spine {
        let Ok(bytes) = read_entry(&mut zip, section, MAX_SECTION_BYTES) else { continue };
        total += bytes.len() as u64;
        if total > MAX_TEXT_BYTES {
            break;
        }
        let (w, c) = count_text(&String::from_utf8_lossy(&bytes));
        words += w;
        characters += c;
    }

    // Chapters: EPUB3 nav, else the EPUB2 NCX, else the number of sections.
    let nav = manifest
        .iter()
        .find(|n| n.attribute("properties").is_some_and(|p| p.split_whitespace().any(|x| x == "nav")))
        .and_then(|n| n.attribute("href"))
        .and_then(|h| read_entry(&mut zip, &resolve_zip_path(&opf_dir, h), MAX_META_BYTES).ok())
        .map(|b| count_toc_entries(&String::from_utf8_lossy(&b), "a"));
    let mut ncx = || {
        let id = spine_node?.attribute("toc")?;
        let href = href_of(id)?;
        let bytes = read_entry(&mut zip, &resolve_zip_path(&opf_dir, href), MAX_META_BYTES).ok()?;
        Some(count_toc_entries(&String::from_utf8_lossy(&bytes), "navPoint"))
    };
    let chapters = nav.filter(|&n| n > 0).or_else(|| ncx().filter(|&n| n > 0)).unwrap_or(spine.len() as u32);

    Ok(crate::BookStats::new(words, characters, chapters, images))
}

/// Entries of a table of contents: `<a>` inside the toc `<nav>`, or NCX navPoints.
fn count_toc_entries(xml: &str, entry: &str) -> u32 {
    let Ok(doc) = Document::parse_with_options(strip_bom(xml), roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
    else {
        return 0;
    };
    let scope = if entry == "a" {
        doc.descendants().find(|n| {
            local(n) == "nav" && n.attributes().any(|a| a.name() == "type" && a.value().split_whitespace().any(|t| t == "toc"))
        })
    } else {
        Some(doc.root_element())
    };
    scope.map(|s| s.descendants().filter(|n| local(n) == entry).count() as u32).unwrap_or(0)
}

/// Words and non-whitespace characters of an (X)HTML document's visible text.
/// Tolerant of broken markup: it scans the text instead of parsing XML.
pub fn count_text(html: &str) -> (u64, u64) {
    const SKIP: [&str; 3] = ["head", "script", "style"];
    const INLINE: [&str; 14] = ["a", "b", "i", "em", "strong", "span", "sup", "sub", "small", "u", "s", "abbr", "cite", "q"];
    let (mut words, mut chars) = (0u64, 0u64);
    let mut in_word = false;
    let mut word_has_alnum = false;
    let mut end_word = |in_word: &mut bool, has: &mut bool| {
        if *in_word && *has {
            words += 1;
        }
        *in_word = false;
        *has = false;
    };
    let mut i = 0;
    while i < html.len() {
        let c = html[i..].chars().next().unwrap();
        if c == '<' {
            let end = html[i..].find('>').map(|e| i + e + 1).unwrap_or(html.len());
            let tag = &html[i + 1..end.saturating_sub(1)];
            let closing = tag.starts_with('/');
            let name: String = tag
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_lowercase();
            if !closing && SKIP.contains(&name.as_str()) && !tag.ends_with('/') {
                let close = format!("</{name}");
                let lower_rest = html[end..].to_ascii_lowercase();
                i = lower_rest.find(&close).map(|p| end + p).unwrap_or(html.len());
                end_word(&mut in_word, &mut word_has_alnum);
                continue;
            }
            if !INLINE.contains(&name.as_str()) {
                end_word(&mut in_word, &mut word_has_alnum);
            }
            i = end;
            continue;
        }
        let (ch, len) = if c == '&' {
            let semi = html[i..].char_indices().take(12).find(|(_, c)| *c == ';').map(|(p, _)| p);
            match semi {
                Some(p) => {
                    let entity = &html[i + 1..i + p];
                    let ch = match entity {
                        "nbsp" | "#160" => ' ',
                        "amp" | "lt" | "gt" | "quot" | "apos" => '&',
                        e if e.starts_with('#') => 'x',
                        _ => 'x', // named letters such as &eacute;
                    };
                    (ch, p + 1)
                }
                None => ('&', 1),
            }
        } else {
            (c, c.len_utf8())
        };
        if ch.is_whitespace() {
            end_word(&mut in_word, &mut word_has_alnum);
        } else {
            chars += 1;
            in_word = true;
            word_has_alnum |= ch.is_alphanumeric();
        }
        i += len;
    }
    end_word(&mut in_word, &mut word_has_alnum);
    (words, chars)
}

/// Finds the cover href in the manifest (EPUB3, EPUB2 and heuristics).
fn find_cover_href(doc: &Document) -> Option<String> {
    let manifest = doc.descendants().find(|n| local(n) == "manifest")?;
    let items: Vec<Node> = manifest.children().filter(|n| local(n) == "item").collect();
    let is_img = |n: &Node| {
        n.attribute("media-type").is_some_and(|t| t.starts_with("image/"))
            || n.attribute("href").is_some_and(is_image_name)
    };

    // EPUB3
    if let Some(i) = items
        .iter()
        .find(|n| n.attribute("properties").is_some_and(|p| p.split_whitespace().any(|x| x == "cover-image")))
    {
        return i.attribute("href").map(str::to_owned);
    }
    // EPUB2: <meta name="cover" content="item-id"/>
    if let Some(id) = doc
        .descendants()
        .find(|n| local(n) == "meta" && n.attribute("name") == Some("cover"))
        .and_then(|n| n.attribute("content"))
    {
        if let Some(i) = items.iter().find(|n| n.attribute("id") == Some(id) && is_img(n)) {
            return i.attribute("href").map(str::to_owned);
        }
        // Sometimes `content` points straight at the href.
        if is_image_name(id) {
            return Some(id.to_owned());
        }
    }
    // Heuristic: image item whose id or href contains "cover".
    items
        .iter()
        .filter(|n| is_img(n))
        .find(|n| {
            n.attribute("id").is_some_and(|v| v.to_ascii_lowercase().contains("cover"))
                || n.attribute("href").is_some_and(|v| v.to_ascii_lowercase().contains("cover"))
        })
        .and_then(|n| n.attribute("href"))
        .map(str::to_owned)
}

fn cover_from_entry(zip: &mut Zip, name: &str) -> Option<Vec<u8>> {
    let data = read_entry(zip, name, MAX_COVER_BYTES).ok()?;
    looks_like_image(&data).then_some(data)
}

/// Last resort: the largest image whose name contains "cover", or the first image.
fn fallback_cover(zip: &mut Zip) -> Option<Vec<u8>> {
    let mut images: Vec<(String, u64)> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().and_then(|e| Some((e.name().ok()?.into_owned(), e.size()))))
        .filter(|(n, _)| is_image_name(n))
        .collect();
    if images.is_empty() {
        return None;
    }
    let pick = images
        .iter()
        .filter(|(n, _)| n.to_ascii_lowercase().contains("cover"))
        .max_by_key(|(_, s)| *s)
        .map(|(n, _)| n.clone())
        .unwrap_or_else(|| {
            images.sort_by(|a, b| super::util::natural_cmp(&a.0, &b.0));
            images[0].0.clone()
        });
    cover_from_entry(zip, &pick)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_epub2_and_epub3_series() {
        let opf = r##"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
  <dc:title>Fundación</dc:title>
  <dc:creator id="c1">Isaac Asimov</dc:creator>
  <meta refines="#c1" property="role">aut</meta>
  <dc:creator id="c2">Ilustrador X</dc:creator>
  <meta refines="#c2" property="role">ill</meta>
  <dc:identifier opf:scheme="ISBN">978-84-9759-920-9</dc:identifier>
  <dc:subject>Ciencia ficción</dc:subject>
  <meta property="belongs-to-collection" id="s">Trilogía de la Fundación</meta>
  <meta refines="#s" property="group-position">1</meta>
  <meta name="cover" content="img1"/>
 </metadata>
 <manifest><item id="img1" href="images/portada.jpg" media-type="image/jpeg"/></manifest>
</package>"##;
        let doc = Document::parse(opf).unwrap();
        let m = parse_opf_metadata(&doc);
        assert_eq!(m.title.as_deref(), Some("Fundación"));
        assert_eq!(m.authors, vec!["Isaac Asimov"]);
        assert_eq!(m.series.as_deref(), Some("Trilogía de la Fundación"));
        assert_eq!(m.series_index, Some(1.0));
        assert_eq!(m.isbn.as_deref(), Some("978-84-9759-920-9"));
        assert_eq!(find_cover_href(&doc).as_deref(), Some("images/portada.jpg"));
    }

    #[test]
    fn reads_every_useful_field() {
        let opf = r##"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
  <dc:title id="t1">The Name of the Wind</dc:title>
  <meta refines="#t1" property="title-type">main</meta>
  <dc:title id="t2">The Kingkiller Chronicle: Day One</dc:title>
  <meta refines="#t2" property="title-type">subtitle</meta>
  <dc:creator id="a1">Patrick Rothfuss</dc:creator>
  <meta refines="#a1" property="role" scheme="marc:relators">aut</meta>
  <dc:creator id="a2">Gabriel Dols</dc:creator>
  <meta refines="#a2" property="role" scheme="marc:relators">trl</meta>
  <dc:contributor opf:role="ill">Jane Illustrator</dc:contributor>
  <dc:contributor>Someone Without Role</dc:contributor>
  <dc:identifier id="uid">urn:uuid:7c9e6679-7425-40de-944b-e07fc1f90ae7</dc:identifier>
  <dc:identifier opf:scheme="ISBN">978-84-01-35230-9</dc:identifier>
  <dc:identifier opf:scheme="AMAZON">B004JHYRG0</dc:identifier>
  <dc:identifier id="d">10.1000/xyz123</dc:identifier>
  <meta refines="#d" property="identifier-type" scheme="onix:codelist5">06</meta>
  <dc:date opf:event="publication">2007-03-27</dc:date>
  <dc:date opf:event="modification">2011-05-02</dc:date>
  <meta property="dcterms:modified">2020-01-15T10:00:00Z</meta>
  <dc:rights>All rights reserved</dc:rights>
  <dc:source>urn:isbn:9780756404079</dc:source>
  <dc:publisher>Plaza &amp; Janés</dc:publisher>
  <dc:language>es</dc:language>
 </metadata>
</package>"##;
        let doc = Document::parse(opf).unwrap();
        let m = parse_opf_metadata(&doc);
        assert_eq!(m.title.as_deref(), Some("The Name of the Wind"));
        assert_eq!(m.subtitle.as_deref(), Some("The Kingkiller Chronicle: Day One"));
        assert_eq!(m.authors, vec!["Patrick Rothfuss"]);
        let roles: Vec<(&str, &str)> = m.contributors.iter().map(|c| (c.name.as_str(), c.role.as_str())).collect();
        assert_eq!(roles, [("Gabriel Dols", "trl"), ("Jane Illustrator", "ill"), ("Someone Without Role", "ctb")]);
        let ids: Vec<(&str, &str)> = m.identifiers.iter().map(|i| (i.scheme.as_str(), i.value.as_str())).collect();
        assert_eq!(
            ids,
            [
                ("uuid", "7c9e6679-7425-40de-944b-e07fc1f90ae7"),
                ("isbn", "978-84-01-35230-9"),
                ("asin", "B004JHYRG0"),
                ("doi", "10.1000/xyz123"),
            ]
        );
        assert_eq!(m.isbn.as_deref(), Some("978-84-01-35230-9"));
        assert_eq!(m.published.as_deref(), Some("2007-03-27"));
        assert_eq!(m.modified.as_deref(), Some("2020-01-15T10:00:00Z"));
        assert_eq!(m.rights.as_deref(), Some("All rights reserved"));
        assert_eq!(m.source.as_deref(), Some("urn:isbn:9780756404079"));
        assert_eq!(m.format_version.as_deref(), Some("EPUB 3.0"));
    }

    #[test]
    fn epub2_dates_and_creator_roles() {
        let opf = r##"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
 <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
  <dc:title>Ficciones</dc:title>
  <dc:creator opf:role="aut" opf:file-as="Borges, Jorge Luis">Jorge Luis Borges</dc:creator>
  <dc:creator opf:role="edt">An Editor</dc:creator>
  <dc:date opf:event="modification">2015-01-01</dc:date>
  <dc:date>1944</dc:date>
  <dc:identifier>9788420633121</dc:identifier>
 </metadata>
</package>"##;
        let m = parse_opf_metadata(&Document::parse(opf).unwrap());
        assert_eq!(m.authors, vec!["Jorge Luis Borges"]);
        assert_eq!(m.contributors.len(), 1);
        assert_eq!((m.contributors[0].name.as_str(), m.contributors[0].role.as_str()), ("An Editor", "edt"));
        assert_eq!(m.published.as_deref(), Some("1944"));
        assert_eq!(m.modified.as_deref(), Some("2015-01-01"));
        assert_eq!(m.isbn.as_deref(), Some("9788420633121"));
        assert_eq!(m.identifiers[0].scheme, "isbn");
        assert_eq!(m.format_version.as_deref(), Some("EPUB 2.0"));
    }
}
