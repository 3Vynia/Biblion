//! EPUB 2 and 3: OPF metadata (including Calibre and EPUB3 series) and cover.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use roxmltree::{Document, Node};
use zip::ZipArchive;

use super::util::{zip_names, is_image_name, looks_like_image, read_limited, resolve_zip_path};
use super::{Extracted, MAX_COVER_BYTES, MAX_META_BYTES};
use crate::{Error, Metadata, Result};

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

fn parse_opf_metadata(doc: &Document) -> Metadata {
    let mut m = Metadata::default();
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

    let mut creators: Vec<(i32, String)> = Vec::new();
    for n in meta.children().filter(|n| n.is_element()) {
        match local(&n) {
            "title" if m.title.is_none() => m.title = text(&n),
            "creator" => {
                let id = n.attribute("id");
                // Authors only (or no role given), not illustrators/editors.
                let role = n
                    .attributes()
                    .find(|a| a.name() == "role")
                    .map(|a| a.value().to_owned())
                    .or_else(|| refines(id, "role"));
                if role.as_deref().is_none_or(|r| r == "aut") {
                    if let Some(t) = text(&n) {
                        let order = refines(id, "display-seq").and_then(|s| s.parse().ok()).unwrap_or(1000);
                        creators.push((order, t));
                    }
                }
            }
            "publisher" => m.publisher = m.publisher.take().or_else(|| text(&n)),
            "language" => m.language = m.language.take().or_else(|| text(&n)),
            "date" => m.published = m.published.take().or_else(|| text(&n)),
            "description" => m.description = m.description.take().or_else(|| text(&n)),
            "subject" => m.tags.extend(text(&n)),
            "identifier" => {
                if m.isbn.is_none() {
                    let scheme = n.attributes().find(|a| a.name() == "scheme").map(|a| a.value().to_ascii_lowercase());
                    if let Some(t) = text(&n) {
                        let lower = t.to_ascii_lowercase();
                        if scheme.as_deref() == Some("isbn") || lower.starts_with("urn:isbn:") || lower.starts_with("isbn") {
                            m.isbn = Some(lower.trim_start_matches("urn:isbn:").trim_start_matches("isbn:").to_owned());
                        }
                    }
                }
            }
            "meta" => {
                let name = n.attribute("name");
                let content = n.attribute("content");
                match (name, content) {
                    (Some("calibre:series"), Some(c)) => m.series = Some(c.to_owned()),
                    (Some("calibre:series_index"), Some(c)) => m.series_index = c.trim().parse().ok(),
                    _ => {}
                }
                if n.attribute("property") == Some("belongs-to-collection") && m.series.is_none() {
                    let id = n.attribute("id");
                    let ctype = refines(id, "collection-type");
                    if ctype.as_deref().is_none_or(|t| t == "series") {
                        m.series = text(&n);
                        m.series_index = refines(id, "group-position").and_then(|s| s.trim().parse().ok());
                    }
                }
            }
            _ => {}
        }
    }
    creators.sort_by_key(|(o, _)| *o);
    m.authors = creators.into_iter().map(|(_, a)| a).collect();
    m
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
}
