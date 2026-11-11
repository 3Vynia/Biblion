//! Offline dictionaries in StarDict format (`.ifo` + `.idx[.gz]` +
//! `.dict[.dz]`), the format used by GoldenDict, KOReader and most free
//! dictionaries (e.g. FreeDict).
//!
//! The index is kept in memory (sorted, case-insensitive). Definitions are read
//! from disk on demand; for dictzip files (`.dict.dz`) only the compressed
//! chunks that hold the definition are inflated, so large dictionaries stay
//! cheap.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use flate2::{Decompress, FlushDecompress};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

const MAX_INDEX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRY_BYTES: u32 = 4 * 1024 * 1024;
const MAX_RESULTS_PER_DICTIONARY: usize = 5;

/// An installed dictionary, as shown in Settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DictionaryInfo {
    /// Path of the `.ifo` relative to the dictionaries folder.
    pub id: String,
    pub name: String,
    pub word_count: u64,
}

/// One definition found for a word.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    pub dictionary: String,
    pub word: String,
    pub body: String,
    /// The body is markup (HTML, XDXF, Pango) and should be shown as text.
    pub markup: bool,
}

struct IndexEntry {
    key: String,
    word: String,
    offset: u64,
    size: u32,
}

enum Data {
    Plain(PathBuf),
    DictZip(DictZip),
    Memory(Vec<u8>),
}

pub struct Dictionary {
    pub info: DictionaryInfo,
    index: Vec<IndexEntry>,
    data: Data,
    types: Option<String>,
}

fn bad(msg: impl ToString) -> Error {
    Error::corrupt("dictionary", msg)
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| Error::io(path, e))
}

fn gunzip(bytes: &[u8], limit: u64) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes).take(limit).read_to_end(&mut out).map_err(bad)?;
    Ok(out)
}

/// Finds `<stem>.<ext>` next to the `.ifo`, trying each extension in order.
fn sibling(ifo: &Path, exts: &[&str]) -> Option<PathBuf> {
    let stem = ifo.file_stem()?.to_str()?;
    exts.iter().map(|e| ifo.with_file_name(format!("{stem}.{e}"))).find(|p| p.is_file())
}

impl Dictionary {
    pub fn open(ifo: &Path, id: String) -> Result<Self> {
        let text = String::from_utf8_lossy(&read_file(ifo)?).into_owned();
        let mut lines = text.lines();
        if !lines.next().is_some_and(|l| l.trim_start_matches('\u{feff}').starts_with("StarDict's dict ifo file")) {
            return Err(bad("not a StarDict .ifo file"));
        }
        let field = |key: &str| {
            text.lines().find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_owned()))
        };
        let name = field("bookname").unwrap_or_else(|| id.clone());
        let offset_bits: u32 = field("idxoffsetbits").and_then(|v| v.parse().ok()).unwrap_or(32);
        let types = field("sametypesequence").filter(|s| !s.is_empty());

        let idx_path = sibling(ifo, &["idx", "idx.gz"]).ok_or_else(|| bad("missing .idx file"))?;
        let idx_size = fs::metadata(&idx_path).map_err(|e| Error::io(&idx_path, e))?.len();
        if idx_size > MAX_INDEX_BYTES {
            return Err(bad("index too large"));
        }
        let raw = read_file(&idx_path)?;
        let idx = if idx_path.extension().is_some_and(|e| e == "gz") { gunzip(&raw, MAX_INDEX_BYTES)? } else { raw };
        let mut index = parse_index(&idx, offset_bits == 64)?;
        index.sort_by(|a, b| a.key.cmp(&b.key).then_with(|| a.word.cmp(&b.word)));

        let dict_path = sibling(ifo, &["dict.dz", "dict", "dict.gz"]).ok_or_else(|| bad("missing .dict file"))?;
        let name_str = dict_path.to_string_lossy();
        let data = if name_str.ends_with(".dict") {
            Data::Plain(dict_path)
        } else {
            match DictZip::open(&dict_path) {
                Ok(z) => Data::DictZip(z),
                // A plain gzip file without random-access chunks: inflate it once.
                Err(_) => Data::Memory(gunzip(&read_file(&dict_path)?, 2 * MAX_INDEX_BYTES)?),
            }
        };

        let word_count = index.len() as u64;
        Ok(Self { info: DictionaryInfo { id, name, word_count }, index, data, types })
    }

    fn read(&self, offset: u64, size: u32) -> Result<Vec<u8>> {
        if size > MAX_ENTRY_BYTES {
            return Err(bad("entry too large"));
        }
        match &self.data {
            Data::Plain(path) => {
                let mut f = File::open(path).map_err(|e| Error::io(path, e))?;
                f.seek(SeekFrom::Start(offset)).map_err(|e| Error::io(path, e))?;
                let mut buf = vec![0; size as usize];
                f.read_exact(&mut buf).map_err(|e| Error::io(path, e))?;
                Ok(buf)
            }
            Data::DictZip(z) => z.read(offset, size),
            Data::Memory(bytes) => {
                let (start, end) = (offset as usize, offset as usize + size as usize);
                bytes.get(start..end).map(<[u8]>::to_vec).ok_or_else(|| bad("entry out of range"))
            }
        }
    }

    /// Definitions for the first spelling of `word` this dictionary knows.
    pub fn lookup(&self, word: &str) -> Vec<Definition> {
        for candidate in candidates(word) {
            let start = self.index.partition_point(|e| e.key < candidate);
            let hits: Vec<&IndexEntry> =
                self.index[start..].iter().take_while(|e| e.key == candidate).take(MAX_RESULTS_PER_DICTIONARY).collect();
            if hits.is_empty() {
                continue;
            }
            return hits
                .into_iter()
                .filter_map(|e| {
                    let bytes = self.read(e.offset, e.size).ok()?;
                    let (body, markup) = decode_entry(&bytes, self.types.as_deref())?;
                    Some(Definition { dictionary: self.info.name.clone(), word: e.word.clone(), body, markup })
                })
                .collect();
        }
        Vec::new()
    }
}

fn parse_index(bytes: &[u8], offsets_64: bool) -> Result<Vec<IndexEntry>> {
    let width = if offsets_64 { 8 } else { 4 };
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let end = bytes[i..].iter().position(|&b| b == 0).map(|p| i + p).ok_or_else(|| bad("truncated index"))?;
        let word = String::from_utf8_lossy(&bytes[i..end]).into_owned();
        let p = end + 1;
        let fields = bytes.get(p..p + width + 4).ok_or_else(|| bad("truncated index"))?;
        let offset = if offsets_64 {
            u64::from_be_bytes(fields[..8].try_into().unwrap())
        } else {
            u32::from_be_bytes(fields[..4].try_into().unwrap()) as u64
        };
        let size = u32::from_be_bytes(fields[width..width + 4].try_into().unwrap());
        out.push(IndexEntry { key: word.to_lowercase(), word, offset, size });
        i = p + width + 4;
    }
    Ok(out)
}

/// Takes one field of type `t` off the front of `rest`. Lower-case types are
/// text (NUL-terminated except the last), upper-case ones binary (u32 size
/// first, except the last).
fn take_field<'a>(t: char, last: bool, rest: &mut &'a [u8]) -> Option<&'a [u8]> {
    let r: &'a [u8] = rest;
    let (field, next) = if last {
        (r, &r[r.len()..])
    } else if t.is_ascii_lowercase() {
        let end = r.iter().position(|&b| b == 0)?;
        (&r[..end], &r[end + 1..])
    } else {
        let size = u32::from_be_bytes(r.get(..4)?.try_into().ok()?) as usize;
        (r.get(4..4 + size)?, &r[(4 + size).min(r.len())..])
    };
    *rest = next;
    Some(field)
}

/// Text of an entry, joining its text fields.
fn decode_entry(bytes: &[u8], types: Option<&str>) -> Option<(String, bool)> {
    let mut parts: Vec<(char, &[u8])> = Vec::new();
    let mut rest = bytes;
    let take = take_field;
    match types {
        Some(seq) => {
            let n = seq.chars().count();
            for (i, t) in seq.chars().enumerate() {
                let field = take(t, i + 1 == n, &mut rest)?;
                parts.push((t, field));
            }
        }
        None => {
            while let Some((&t, tail)) = rest.split_first() {
                rest = tail;
                let t = t as char;
                let field = take(t, false, &mut rest).or_else(|| {
                    let all = rest;
                    rest = &rest[rest.len()..];
                    Some(all)
                })?;
                parts.push((t, field));
            }
        }
    }
    let texts: Vec<(String, bool)> = parts
        .into_iter()
        .filter_map(|(t, f)| match t {
            'm' | 't' | 'y' | 'l' | 'w' => Some((String::from_utf8_lossy(f).trim().to_owned(), false)),
            'h' | 'g' | 'x' | 'k' => Some((String::from_utf8_lossy(f).trim().to_owned(), true)),
            _ => None,
        })
        .filter(|(s, _)| !s.is_empty())
        .collect();
    if texts.is_empty() {
        return None;
    }
    let markup = texts.iter().any(|(_, m)| *m);
    Some((texts.into_iter().map(|(s, _)| s).collect::<Vec<_>>().join("\n\n"), markup))
}

/// Spellings to try for a selected word, most specific first.
pub fn candidates(word: &str) -> Vec<String> {
    let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
    let lower = trimmed.to_lowercase();
    let mut out = vec![lower.clone()];
    let mut push = |s: String| {
        if s.chars().count() > 1 && !out.contains(&s) {
            out.push(s)
        }
    };
    // English and Spanish possessives and plurals; good enough for a fallback.
    if let Some(base) = lower.strip_suffix("'s").or_else(|| lower.strip_suffix("’s")) {
        push(base.to_owned());
    }
    if let Some(base) = lower.strip_suffix("ies") {
        push(format!("{base}y"));
    }
    if let Some(base) = lower.strip_suffix("ces") {
        push(format!("{base}z"));
    }
    if let Some(base) = lower.strip_suffix("es") {
        push(base.to_owned());
    }
    if let Some(base) = lower.strip_suffix('s') {
        push(base.to_owned());
    }
    out
}

/// Random access to a dictzip file: a gzip file whose deflate stream is cut in
/// fixed-size chunks listed in the header's "RA" extra field.
struct DictZip {
    path: PathBuf,
    chunk_len: u64,
    /// File offset and compressed size of each chunk.
    chunks: Vec<(u64, u32)>,
}

impl DictZip {
    fn open(path: &Path) -> Result<Self> {
        let mut f = File::open(path).map_err(|e| Error::io(path, e))?;
        let mut head = vec![0u8; 64 * 1024];
        let n = f.read(&mut head).map_err(|e| Error::io(path, e))?;
        let h = &head[..n];
        if h.len() < 12 || h[0] != 0x1f || h[1] != 0x8b || h[2] != 8 {
            return Err(bad("not a gzip file"));
        }
        let flags = h[3];
        if flags & 4 == 0 {
            return Err(bad("no dictzip header"));
        }
        let xlen = u16::from_le_bytes([h[10], h[11]]) as usize;
        let extra = h.get(12..12 + xlen).ok_or_else(|| bad("truncated header"))?;
        let mut pos = 12 + xlen;
        let (mut chunk_len, mut sizes) = (0u64, Vec::new());
        let mut e = 0;
        while e + 4 <= extra.len() {
            let len = u16::from_le_bytes([extra[e + 2], extra[e + 3]]) as usize;
            let data = extra.get(e + 4..e + 4 + len).ok_or_else(|| bad("truncated header"))?;
            if &extra[e..e + 2] == b"RA" && data.len() >= 6 {
                chunk_len = u16::from_le_bytes([data[2], data[3]]) as u64;
                let count = u16::from_le_bytes([data[4], data[5]]) as usize;
                sizes = (0..count)
                    .map(|i| data.get(6 + i * 2..8 + i * 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as u32))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| bad("truncated chunk table"))?;
            }
            e += 4 + len;
        }
        if chunk_len == 0 || sizes.is_empty() {
            return Err(bad("no dictzip chunk table"));
        }
        // Skip the optional file name, comment and header CRC.
        for flag in [8u8, 16] {
            if flags & flag != 0 {
                pos += h.get(pos..).and_then(|r| r.iter().position(|&b| b == 0)).ok_or_else(|| bad("truncated header"))? + 1;
            }
        }
        if flags & 2 != 0 {
            pos += 2;
        }
        let mut chunks = Vec::with_capacity(sizes.len());
        let mut offset = pos as u64;
        for size in sizes {
            chunks.push((offset, size));
            offset += size as u64;
        }
        Ok(Self { path: path.to_owned(), chunk_len, chunks })
    }

    fn read(&self, offset: u64, size: u32) -> Result<Vec<u8>> {
        let first = (offset / self.chunk_len) as usize;
        let last = ((offset + size as u64).saturating_sub(1) / self.chunk_len) as usize;
        if last >= self.chunks.len() {
            return Err(bad("entry out of range"));
        }
        let mut f = File::open(&self.path).map_err(|e| Error::io(&self.path, e))?;
        let mut out = Vec::with_capacity(((last - first + 1) as u64 * self.chunk_len) as usize);
        for &(start, csize) in &self.chunks[first..=last] {
            let mut compressed = vec![0; csize as usize];
            f.seek(SeekFrom::Start(start)).map_err(|e| Error::io(&self.path, e))?;
            f.read_exact(&mut compressed).map_err(|e| Error::io(&self.path, e))?;
            // Each chunk ends with a full flush, so it inflates on its own.
            let mut inflater = Decompress::new(false);
            let mut chunk = Vec::with_capacity(self.chunk_len as usize);
            inflater.decompress_vec(&compressed, &mut chunk, FlushDecompress::Sync).map_err(bad)?;
            out.extend_from_slice(&chunk);
        }
        let skip = (offset - first as u64 * self.chunk_len) as usize;
        out.get(skip..skip + size as usize).map(<[u8]>::to_vec).ok_or_else(|| bad("entry out of range"))
    }
}

// ───────────────────────────── Installed set ─────────────────────────────

/// All dictionaries in a folder (searched recursively for `.ifo` files).
pub fn open_all(dir: &Path) -> Vec<Dictionary> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(dir).max_depth(3).into_iter().flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ifo")) {
            let id = p.strip_prefix(dir).unwrap_or(p).to_string_lossy().replace('\\', "/");
            match Dictionary::open(p, id) {
                Ok(d) => out.push(d),
                Err(e) => tracing::warn!("skipping dictionary {}: {e}", p.display()),
            }
        }
    }
    out.sort_by(|a, b| a.info.name.to_lowercase().cmp(&b.info.name.to_lowercase()));
    out
}

const DICT_FILES: [&str; 6] = [".ifo", ".idx", ".idx.gz", ".dict", ".dict.dz", ".syn"];
const MAX_INSTALL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn is_dict_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    DICT_FILES.iter().any(|ext| lower.ends_with(ext)) || lower.ends_with(".dict.gz")
}

/// Plain file name only, so archive entries can't write outside the folder.
fn safe_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?.trim();
    (!base.is_empty() && base != "." && base != "..").then(|| base.to_owned())
}

/// Installs a dictionary from an `.ifo` (copying its sibling files), a `.zip`
/// or a `.tar.gz`/`.tgz`/`.tar` into a new subfolder of `dir`. Returns the
/// subfolder; fails (and leaves nothing behind) if it holds no dictionary.
pub fn install(dir: &Path, source: &Path) -> Result<PathBuf> {
    let file_name = source.file_name().and_then(|n| n.to_str()).ok_or_else(|| bad("invalid file name"))?;
    let lower = file_name.to_ascii_lowercase();
    let stem = ["tar.gz", "tgz", "tar", "zip", "ifo"]
        .iter()
        .find_map(|ext| lower.strip_suffix(&format!(".{ext}")).map(|s| file_name[..s.len()].to_owned()))
        .ok_or_else(|| bad("choose a StarDict .ifo file, or a .zip / .tar.gz with one"))?;
    let mut target = dir.join(&stem);
    let mut n = 2;
    while target.exists() {
        target = dir.join(format!("{stem} ({n})"));
        n += 1;
    }
    fs::create_dir_all(&target).map_err(|e| Error::io(&target, e))?;
    let result = (|| -> Result<()> {
        let mut written = 0u64;
        let mut save = |name: &str, reader: &mut dyn Read| -> Result<()> {
            let Some(name) = safe_name(name).filter(|n| is_dict_file(n)) else { return Ok(()) };
            let path = target.join(name);
            let mut out = File::create(&path).map_err(|e| Error::io(&path, e))?;
            written += std::io::copy(&mut reader.take(MAX_INSTALL_BYTES - written), &mut out).map_err(|e| Error::io(&path, e))?;
            if written >= MAX_INSTALL_BYTES {
                return Err(bad("dictionary too large"));
            }
            Ok(())
        };
        if lower.ends_with(".ifo") {
            let ifo_stem = &file_name[..file_name.len() - 4];
            for ext in DICT_FILES.iter().chain(&[".dict.gz"]) {
                let p = source.with_file_name(format!("{ifo_stem}{ext}"));
                if p.is_file() {
                    let mut f = File::open(&p).map_err(|e| Error::io(&p, e))?;
                    save(&format!("{ifo_stem}{ext}"), &mut f)?;
                }
            }
        } else if lower.ends_with(".zip") {
            let f = File::open(source).map_err(|e| Error::io(source, e))?;
            let mut zip = zip::ZipArchive::new(f)?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                if entry.is_file() {
                    let name = entry.name()?.into_owned();
                    save(&name, &mut entry)?;
                }
            }
        } else {
            let f = File::open(source).map_err(|e| Error::io(source, e))?;
            let reader: Box<dyn Read> = if lower.ends_with(".tar") { Box::new(f) } else { Box::new(flate2::read::GzDecoder::new(f)) };
            let mut archive = tar::Archive::new(reader);
            for entry in archive.entries().map_err(bad)? {
                let mut entry = entry.map_err(bad)?;
                if entry.header().entry_type().is_file() {
                    let name = entry.path().map_err(bad)?.to_string_lossy().into_owned();
                    save(&name, &mut entry)?;
                }
            }
        }
        if open_all(&target).is_empty() {
            return Err(bad("no StarDict dictionary found (it needs .ifo, .idx and .dict files)"));
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&target);
        return Err(e);
    }
    Ok(target)
}

/// Removes an installed dictionary (its files, and its folder when empty).
pub fn remove(dir: &Path, id: &str) -> Result<()> {
    let ifo = dir.join(id);
    // `id` comes from `open_all`; refuse anything that escapes the folder.
    if id.split(['/', '\\']).any(|p| p == "..") || !ifo.starts_with(dir) || !ifo.is_file() {
        return Err(Error::NotFound(format!("dictionary {id}")));
    }
    let stem = ifo.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_owned();
    for ext in DICT_FILES.iter().chain(&[".dict.gz"]) {
        let p = ifo.with_file_name(format!("{stem}{ext}"));
        if p.is_file() {
            fs::remove_file(&p).map_err(|e| Error::io(&p, e))?;
        }
    }
    if let Some(parent) = ifo.parent().filter(|p| *p != dir) {
        if fs::read_dir(parent).map(|mut d| d.next().is_none()).unwrap_or(false) {
            let _ = fs::remove_dir(parent);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Writes a StarDict dictionary; `dictzip` stores `.dict.dz` with tiny chunks.
    pub(crate) fn write_dict(dir: &Path, stem: &str, name: &str, entries: &[(&str, &str)], dictzip: bool) {
        fs::create_dir_all(dir).unwrap();
        let mut sorted: Vec<_> = entries.to_vec();
        sorted.sort_by_key(|(w, _)| w.to_lowercase());
        let (mut idx, mut data) = (Vec::new(), Vec::new());
        for (word, def) in &sorted {
            idx.extend_from_slice(word.as_bytes());
            idx.push(0);
            idx.extend_from_slice(&(data.len() as u32).to_be_bytes());
            idx.extend_from_slice(&(def.len() as u32).to_be_bytes());
            data.extend_from_slice(def.as_bytes());
        }
        fs::write(
            dir.join(format!("{stem}.ifo")),
            format!("StarDict's dict ifo file\nversion=2.4.2\nbookname={name}\nwordcount={}\nidxfilesize={}\nsametypesequence=m\n", sorted.len(), idx.len()),
        )
        .unwrap();
        fs::write(dir.join(format!("{stem}.idx")), &idx).unwrap();
        if dictzip {
            fs::write(dir.join(format!("{stem}.dict.dz")), make_dictzip(&data, 16)).unwrap();
        } else {
            fs::write(dir.join(format!("{stem}.dict")), &data).unwrap();
        }
    }

    fn make_dictzip(data: &[u8], chunk: usize) -> Vec<u8> {
        let mut deflater = flate2::Compress::new(flate2::Compression::default(), false);
        let (mut body, mut sizes) = (Vec::new(), Vec::new());
        for part in data.chunks(chunk) {
            let mut out = Vec::with_capacity(part.len() + 64);
            deflater.compress_vec(part, &mut out, flate2::FlushCompress::Full).unwrap();
            sizes.push(out.len() as u16);
            body.extend_from_slice(&out);
        }
        let mut ra = Vec::new();
        ra.extend_from_slice(&1u16.to_le_bytes());
        ra.extend_from_slice(&(chunk as u16).to_le_bytes());
        ra.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
        sizes.iter().for_each(|s| ra.extend_from_slice(&s.to_le_bytes()));
        let mut extra = b"RA".to_vec();
        extra.extend_from_slice(&(ra.len() as u16).to_le_bytes());
        extra.extend_from_slice(&ra);
        let mut out = vec![0x1f, 0x8b, 8, 4, 0, 0, 0, 0, 0, 3];
        out.extend_from_slice(&(extra.len() as u16).to_le_bytes());
        out.extend_from_slice(&extra);
        out.extend_from_slice(&body);
        out
    }

    const ENTRIES: [(&str, &str); 5] = [
        ("Casa", "Edificio para habitar."),
        ("house", "A building for people to live in."),
        ("library", "A collection of books."),
        ("lighthouse", "A tower with a bright light that guides ships at sea, standing on a rock."),
        ("run", "To move quickly on foot."),
    ];

    #[test]
    fn looks_up_plain_and_dictzip_dictionaries() {
        for dictzip in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            write_dict(tmp.path(), "test", "Test Dictionary", &ENTRIES, dictzip);
            let d = Dictionary::open(&tmp.path().join("test.ifo"), "test.ifo".into()).unwrap();
            assert_eq!(d.info.name, "Test Dictionary");
            assert_eq!(d.info.word_count, 5);
            // Case-insensitive, punctuation and plurals stripped; spans dictzip chunks.
            let lighthouse = d.lookup("Lighthouses,");
            assert_eq!(lighthouse.len(), 1, "dictzip={dictzip}");
            assert_eq!(lighthouse[0].body, ENTRIES[3].1);
            assert_eq!(d.lookup("casa")[0].word, "Casa");
            assert!(d.lookup("libraries")[0].body.contains("books"));
            assert!(d.lookup("unknownword").is_empty());
        }
    }

    #[test]
    fn decodes_entries_with_and_without_a_type_sequence() {
        assert_eq!(decode_entry(b"plain text", Some("m")), Some(("plain text".into(), false)));
        assert_eq!(decode_entry(b"<b>bold</b>", Some("h")), Some(("<b>bold</b>".into(), true)));
        // Two fields: phonetics then meaning.
        assert_eq!(decode_entry(b"ha\x00meaning", Some("tm")), Some(("ha\n\nmeaning".into(), false)));
        // No sequence: each field starts with its type.
        assert_eq!(decode_entry(b"mfirst\x00hsecond", None), Some(("first\n\nsecond".into(), true)));
        assert_eq!(decode_entry(b"", Some("m")), None);
    }

    #[test]
    fn install_from_folder_zip_and_tarball_then_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        write_dict(&src, "es-en", "Spanish-English", &ENTRIES, true);
        let dicts = tmp.path().join("dictionaries");
        fs::create_dir_all(&dicts).unwrap();

        // From an .ifo next to its files.
        install(&dicts, &src.join("es-en.ifo")).unwrap();
        // From a zip with folders inside (flattened).
        let zip_path = tmp.path().join("bundle.zip");
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        for f in ["es-en.ifo", "es-en.idx", "es-en.dict.dz"] {
            zip.start_file(format!("nested/dir/{f}"), zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(&fs::read(src.join(f)).unwrap()).unwrap();
        }
        zip.finish().unwrap();
        install(&dicts, &zip_path).unwrap();
        // From a .tar.gz.
        let tgz = tmp.path().join("pack.tar.gz");
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(File::create(&tgz).unwrap(), flate2::Compression::fast()));
        for f in ["es-en.ifo", "es-en.idx", "es-en.dict.dz"] {
            builder.append_path_with_name(src.join(f), format!("pack/{f}")).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
        install(&dicts, &tgz).unwrap();

        let all = open_all(&dicts);
        assert_eq!(all.len(), 3);
        assert!(all.iter().all(|d| d.lookup("house").len() == 1));

        // An archive without a dictionary leaves nothing behind.
        let empty = tmp.path().join("empty.zip");
        let mut zip = zip::ZipWriter::new(File::create(&empty).unwrap());
        zip.start_file("readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
        zip.finish().unwrap();
        assert!(install(&dicts, &empty).is_err());
        assert!(!dicts.join("empty").exists());

        // Removing deletes the files and the emptied folder; ids can't escape.
        let id = all[0].info.id.clone();
        remove(&dicts, &id).unwrap();
        assert_eq!(open_all(&dicts).len(), 2);
        assert!(remove(&dicts, "../src/es-en.ifo").is_err());
        assert!(src.join("es-en.ifo").exists());
    }
}
