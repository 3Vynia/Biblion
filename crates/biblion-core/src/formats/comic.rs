//! CBZ (zip) and CBR (rar) comics. The real container is detected from the
//! magic bytes, because .cbr files that are really zips (and vice versa) are common.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use zip::ZipArchive;

use super::util::{zip_names, is_image_name, looks_like_image, natural_cmp, read_limited};
use super::{Extracted, MAX_COVER_BYTES, MAX_META_BYTES};
use crate::{Error, Metadata, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Zip,
    Rar,
}

fn sniff(path: &Path) -> Result<Container> {
    let mut f = File::open(path).map_err(|e| Error::io(path, e))?;
    let mut magic = [0u8; 8];
    let n = f.read(&mut magic).map_err(|e| Error::io(path, e))?;
    let magic = &magic[..n];
    if magic.starts_with(b"PK") {
        Ok(Container::Zip)
    } else if magic.starts_with(b"Rar!") {
        Ok(Container::Rar)
    } else {
        Err(Error::corrupt("comic", "unknown container (neither zip nor rar)"))
    }
}

fn is_page(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    is_image_name(name)
        && !lower.starts_with("__macosx/")
        && !lower.rsplit('/').next().is_some_and(|f| f.starts_with('.'))
}

/// Page list (entry names) in reading order.
pub fn list_pages(path: &Path) -> Result<Vec<String>> {
    let mut names = match sniff(path)? {
        Container::Zip => {
            let zip = open_zip(path)?;
            zip_names(&zip).into_iter().filter(|n| is_page(n)).collect::<Vec<_>>()
        }
        Container::Rar => rar::list(path)?.into_iter().filter(|n| is_page(n)).collect(),
    };
    names.sort_by(|a, b| natural_cmp(a, b));
    Ok(names)
}

/// Bytes of page `index` (0 = cover).
pub fn read_page(path: &Path, index: usize) -> Result<Vec<u8>> {
    let pages = list_pages(path)?;
    let name = pages.get(index).ok_or_else(|| Error::NotFound(format!("page {index}")))?;
    read_entry(path, name, MAX_COVER_BYTES)?.ok_or_else(|| Error::NotFound(name.clone()))
}

fn open_zip(path: &Path) -> Result<ZipArchive<BufReader<File>>> {
    let f = File::open(path).map_err(|e| Error::io(path, e))?;
    Ok(ZipArchive::new(BufReader::new(f))?)
}

fn read_entry(path: &Path, name: &str, limit: u64) -> Result<Option<Vec<u8>>> {
    match sniff(path)? {
        Container::Zip => {
            let mut zip = open_zip(path)?;
            let Ok(entry) = zip.by_name(name) else { return Ok(None) };
            Ok(Some(read_limited(entry, limit).map_err(|e| Error::corrupt("cbz", e))?))
        }
        Container::Rar => rar::read(path, name, limit),
    }
}

pub fn extract(path: &Path) -> Result<Extracted> {
    let container = sniff(path)?;
    let mut names: Vec<String> = match container {
        Container::Zip => zip_names(&open_zip(path)?),
        Container::Rar => rar::list(path)?,
    };
    let comic_info = names.iter().find(|n| n.eq_ignore_ascii_case("comicinfo.xml") || n.to_ascii_lowercase().ends_with("/comicinfo.xml")).cloned();
    names.retain(|n| is_page(n));
    names.sort_by(|a, b| natural_cmp(a, b));

    let mut metadata = Metadata { page_count: Some(names.len() as u32), ..Default::default() };
    if let Some(ci) = comic_info {
        if let Some(bytes) = read_entry(path, &ci, MAX_META_BYTES)? {
            parse_comic_info(&String::from_utf8_lossy(&bytes), &mut metadata);
        }
    }
    // Cover: the first page that really is an image.
    let mut cover = None;
    for name in names.iter().take(3) {
        if let Some(b) = read_entry(path, name, MAX_COVER_BYTES)? {
            if looks_like_image(&b) {
                cover = Some(b);
                break;
            }
        }
    }
    Ok(Extracted { metadata, cover })
}

/// ComicInfo.xml (ComicRack format).
fn parse_comic_info(xml: &str, m: &mut Metadata) {
    let Ok(doc) = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}')) else { return };
    let get = |tag: &str| {
        doc.descendants()
            .find(|n| n.tag_name().name() == tag)
            .and_then(|n| n.text())
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    m.series = get("Series");
    m.series_index = get("Number").and_then(|n| n.parse().ok());
    m.title = get("Title").or_else(|| match (&m.series, get("Number")) {
        (Some(s), Some(n)) => Some(format!("{s} #{n}")),
        _ => None,
    });
    for role in ["Writer", "Penciller"] {
        if let Some(v) = get(role) {
            m.authors.extend(v.split(',').map(|s| s.trim().to_owned()));
        }
    }
    m.publisher = get("Publisher");
    m.description = get("Summary");
    m.language = get("LanguageISO");
    m.published = get("Year").map(|y| match get("Month") {
        Some(mo) => format!("{y}-{mo:0>2}"),
        None => y,
    });
    for key in ["Genre", "Tags"] {
        if let Some(v) = get(key) {
            m.tags.extend(v.split(',').map(|s| s.trim().to_owned()));
        }
    }
    if let Some(p) = get("PageCount").and_then(|p| p.parse().ok()) {
        m.page_count = Some(p);
    }
}

#[cfg(feature = "cbr")]
mod rar {
    use super::*;

    pub fn list(path: &Path) -> Result<Vec<String>> {
        let archive = unrar::Archive::new(path).open_for_listing().map_err(|e| Error::corrupt("cbr", e))?;
        let mut out = Vec::new();
        for entry in archive {
            let entry = entry.map_err(|e| Error::corrupt("cbr", e))?;
            if entry.is_file() {
                out.push(entry.filename.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(out)
    }

    pub fn read(path: &Path, name: &str, limit: u64) -> Result<Option<Vec<u8>>> {
        let mut archive = unrar::Archive::new(path).open_for_processing().map_err(|e| Error::corrupt("cbr", e))?;
        while let Some(header) = archive.read_header().map_err(|e| Error::corrupt("cbr", e))? {
            let entry_name = header.entry().filename.to_string_lossy().replace('\\', "/");
            if entry_name == name {
                if header.entry().unpacked_size > limit {
                    return Err(Error::corrupt("cbr", "entry too large"));
                }
                let (data, _) = header.read().map_err(|e| Error::corrupt("cbr", e))?;
                return Ok(Some(data));
            }
            archive = header.skip().map_err(|e| Error::corrupt("cbr", e))?;
        }
        Ok(None)
    }
}

#[cfg(not(feature = "cbr"))]
mod rar {
    use super::*;
    pub fn list(_: &Path) -> Result<Vec<String>> {
        Err(Error::UnsupportedFormat("cbr (built without the `cbr` feature)".into()))
    }
    pub fn read(_: &Path, _: &str, _: u64) -> Result<Option<Vec<u8>>> {
        Err(Error::UnsupportedFormat("cbr".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comic_info() {
        let xml = r#"<ComicInfo><Series>Mortadelo</Series><Number>12</Number><Writer>Ibáñez</Writer><Year>1985</Year><Month>3</Month><Genre>Humor, Aventura</Genre></ComicInfo>"#;
        let mut m = Metadata::default();
        parse_comic_info(xml, &mut m);
        assert_eq!(m.title.as_deref(), Some("Mortadelo #12"));
        assert_eq!(m.series_index, Some(12.0));
        assert_eq!(m.published.as_deref(), Some("1985-03"));
        assert_eq!(m.tags, vec!["Humor", "Aventura"]);
    }
}
