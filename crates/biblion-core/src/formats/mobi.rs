//! MOBI / AZW3 (KF8): custom parser for PalmDB + MOBI header + EXTH records.
//! Only record 0 and the cover record are read; the text is never decompressed.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::util::looks_like_image;
use super::{Extracted, MAX_COVER_BYTES};
use crate::{Error, Metadata, Result};

fn be_u16(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
}
fn be_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn corrupt(msg: &str) -> Error {
    Error::corrupt("mobi", msg)
}

struct PalmDb {
    file: File,
    file_len: u64,
    offsets: Vec<u32>,
}

impl PalmDb {
    fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path).map_err(|e| Error::io(path, e))?;
        let file_len = file.metadata().map_err(|e| Error::io(path, e))?.len();
        let mut header = [0u8; 78];
        file.read_exact(&mut header).map_err(|_| corrupt("truncated PalmDB header"))?;
        let kind = &header[60..68];
        if kind != b"BOOKMOBI" && kind != b"TEXtREAd" {
            return Err(corrupt("not a MOBI file"));
        }
        let n = be_u16(&header, 76).unwrap() as usize;
        if n == 0 || n > 65_535 {
            return Err(corrupt("invalid record count"));
        }
        let mut list = vec![0u8; n * 8];
        file.read_exact(&mut list).map_err(|_| corrupt("truncated record list"))?;
        let offsets = (0..n).map(|i| be_u32(&list, i * 8).unwrap()).collect();
        Ok(Self { file, file_len, offsets })
    }

    fn record(&mut self, idx: usize, limit: u64) -> Result<Vec<u8>> {
        let start = *self.offsets.get(idx).ok_or_else(|| corrupt("record out of range"))? as u64;
        let end = self.offsets.get(idx + 1).map(|o| *o as u64).unwrap_or(self.file_len);
        if end < start || end > self.file_len || end - start > limit {
            return Err(corrupt("invalid record bounds"));
        }
        let mut buf = vec![0u8; (end - start) as usize];
        self.file.seek(SeekFrom::Start(start)).map_err(|_| corrupt("seek"))?;
        self.file.read_exact(&mut buf).map_err(|_| corrupt("truncated record"))?;
        Ok(buf)
    }
}

fn decode(bytes: &[u8], encoding: u32) -> String {
    if encoding == 65001 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    // CP1252
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
        '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    bytes
        .iter()
        .map(|&b| if (0x80..0xA0).contains(&b) { HIGH[(b - 0x80) as usize] } else { b as char })
        .collect()
}

pub fn extract(path: &Path) -> Result<Extracted> {
    let mut db = PalmDb::open(path)?;
    let rec0 = db.record(0, 1 << 20)?;
    let mut m = Metadata::default();

    if rec0.get(16..20) != Some(b"MOBI") {
        // Plain PalmDOC: no metadata.
        return Ok(Extracted::default());
    }
    let mobi_len = be_u32(&rec0, 20).ok_or_else(|| corrupt("MOBI header"))? as usize;
    let encoding = be_u32(&rec0, 28).unwrap_or(1252);
    let first_image = be_u32(&rec0, 108).unwrap_or(u32::MAX);
    let exth_flags = be_u32(&rec0, 128).unwrap_or(0);

    if let (Some(off), Some(len)) = (be_u32(&rec0, 84), be_u32(&rec0, 88)) {
        if let Some(b) = rec0.get(off as usize..(off as usize).saturating_add(len as usize)) {
            m.title = Some(decode(b, encoding));
        }
    }

    let mut cover_offset: Option<u32> = None;
    let mut thumb_offset: Option<u32> = None;
    if exth_flags & 0x40 != 0 {
        let base = 16 + mobi_len;
        if rec0.get(base..base + 4) == Some(b"EXTH") {
            let count = be_u32(&rec0, base + 8).unwrap_or(0).min(10_000);
            let mut p = base + 12;
            for _ in 0..count {
                let (Some(kind), Some(len)) = (be_u32(&rec0, p), be_u32(&rec0, p + 4)) else { break };
                let len = len as usize;
                if len < 8 {
                    break;
                }
                let Some(data) = rec0.get(p + 8..p + len) else { break };
                let s = || decode(data, encoding);
                match kind {
                    100 => m.authors.push(s()),
                    101 => m.publisher = Some(s()),
                    103 => m.description = Some(s()),
                    104 => m.isbn = Some(s()),
                    105 => m.tags.extend(s().split(';').map(|t| t.trim().to_owned())),
                    106 => m.published = Some(s()),
                    201 => cover_offset = be_u32(data, 0),
                    202 => thumb_offset = be_u32(data, 0),
                    503 => m.title = Some(s()),
                    524 => m.language = Some(s()),
                    _ => {}
                }
                p += len;
            }
        }
    }

    let mut cover = None;
    if first_image != u32::MAX {
        for off in [cover_offset, thumb_offset, Some(0)].into_iter().flatten() {
            if off == u32::MAX {
                continue;
            }
            let idx = first_image as usize + off as usize;
            if let Ok(b) = db.record(idx, MAX_COVER_BYTES) {
                if looks_like_image(&b) {
                    cover = Some(b);
                    break;
                }
            }
        }
    }
    Ok(Extracted { metadata: m, cover })
}
