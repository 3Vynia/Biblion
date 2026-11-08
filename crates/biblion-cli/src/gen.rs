//! Realistic synthetic library generator for load testing.
//! About 0.5 % of files are corrupt and ~5 % have no embedded metadata.

use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const WORDS: &[&str] = &[
    "shadow", "light", "night", "wind", "sea", "city", "silence", "fire", "memory", "garden", "river", "time",
    "road", "mirror", "labyrinth", "dream", "winter", "summer", "last", "secret", "kingdom", "star", "storm",
    "island", "heart", "stone", "wolf", "raven", "dragon", "mountain", "border", "echo", "ash", "library", "war",
    "peace", "daughter", "king", "queen", "name", "autumn", "abyss", "horizon", "salt", "mist",
];
const FIRST: &[&str] = &[
    "Gabriel", "Isabel", "Jorge", "Ana", "Julio", "Elena", "Carlos", "Laura", "Mario", "Rosa", "Pablo", "Clara",
    "Arturo", "Lucía", "Miguel", "Carmen", "Ursula", "Isaac", "Terry", "Margaret",
];
const LAST: &[&str] = &[
    "García Márquez", "Allende", "Borges", "Cortázar", "Vargas Llosa", "Pérez-Reverte", "Matute", "Benedetti",
    "Le Guin", "Asimov", "Pratchett", "Atwood", "Zafón", "Bolaño", "Rulfo", "Mistral", "Fuentes", "Paz", "Neruda",
    "Sábato", "Onetti", "Delibes", "Laforet", "Galdós", "Machado",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[(self.next() % v.len() as u64) as usize]
    }
    fn chance(&mut self, per_thousand: u64) -> bool {
        self.next() % 1000 < per_thousand
    }
}

fn make_covers() -> Vec<Vec<u8>> {
    (0..32u32)
        .map(|i| {
            let img = image::RgbImage::from_fn(600, 900, |x, y| {
                let r = ((x * 255 / 600) + i * 37) % 256;
                let g = ((y * 255 / 900) + i * 91) % 256;
                let b = ((x ^ y) % 64 + i * 13) % 256;
                image::Rgb([r as u8, g as u8, b as u8])
            });
            let mut out = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
            out.into_inner()
        })
        .collect()
}

struct Spec {
    title: String,
    author: String,
    series: Option<(String, u32)>,
    tags: Vec<String>,
    description: String,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn epub(spec: &Spec, cover: &[u8], with_meta: bool) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflate = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    zip.start_file("META-INF/container.xml", deflate).unwrap();
    zip.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
    let meta = if with_meta {
        let series = spec.series.as_ref().map(|(s, i)| format!(r#"<meta name="calibre:series" content="{}"/><meta name="calibre:series_index" content="{i}"/>"#, xml_escape(s))).unwrap_or_default();
        let tags: String = spec.tags.iter().map(|t| format!("<dc:subject>{}</dc:subject>", xml_escape(t))).collect();
        format!(
            "<dc:title>{}</dc:title><dc:creator opf:role=\"aut\">{}</dc:creator><dc:language>en</dc:language><dc:description>&lt;p&gt;{}&lt;/p&gt;</dc:description>{tags}{series}",
            xml_escape(&spec.title), xml_escape(&spec.author), xml_escape(&spec.description)
        )
    } else {
        String::new()
    };
    zip.start_file("OEBPS/content.opf", deflate).unwrap();
    write!(zip, r#"<?xml version="1.0" encoding="UTF-8"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">{meta}<meta name="cover" content="cover-img"/></metadata><manifest><item id="cover-img" href="images/cover.jpg" media-type="image/jpeg"/><item id="c1" href="text/c1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#).unwrap();
    zip.start_file("OEBPS/images/cover.jpg", stored).unwrap();
    zip.write_all(cover).unwrap();
    zip.start_file("OEBPS/text/c1.xhtml", deflate).unwrap();
    write!(zip, "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><h1>{}</h1><p>{}</p></body></html>", xml_escape(&spec.title), xml_escape(&spec.description).repeat(20)).unwrap();
    zip.finish().unwrap().into_inner()
}

fn cbz(spec: &Spec, cover: &[u8], pages: &[Vec<u8>]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    if let Some((s, i)) = &spec.series {
        zip.start_file("ComicInfo.xml", SimpleFileOptions::default()).unwrap();
        write!(zip, "<?xml version=\"1.0\"?><ComicInfo><Series>{}</Series><Number>{i}</Number><Writer>{}</Writer><Genre>{}</Genre></ComicInfo>", xml_escape(s), xml_escape(&spec.author), spec.tags.join(", ")).unwrap();
    }
    // Deliberately unordered names to exercise natural sorting.
    for (n, page) in [(10, &pages[0]), (2, &pages[1]), (1, &cover.to_vec())] {
        zip.start_file(format!("page{n}.jpg"), stored).unwrap();
        zip.write_all(page).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn mobi(spec: &Spec, cover: &[u8], kf8: bool) -> Vec<u8> {
    let title = spec.title.as_bytes();
    let mut exth = Vec::new();
    let mut n_rec = 0u32;
    let mut add = |kind: u32, data: &[u8]| {
        exth.extend_from_slice(&kind.to_be_bytes());
        exth.extend_from_slice(&(data.len() as u32 + 8).to_be_bytes());
        exth.extend_from_slice(data);
        n_rec += 1;
    };
    add(100, spec.author.as_bytes());
    add(503, title);
    add(524, b"en");
    add(201, &0u32.to_be_bytes());
    let mut exth_block = b"EXTH".to_vec();
    exth_block.extend_from_slice(&(exth.len() as u32 + 12).to_be_bytes());
    exth_block.extend_from_slice(&n_rec.to_be_bytes());
    exth_block.extend_from_slice(&exth);
    while !exth_block.len().is_multiple_of(4) {
        exth_block.push(0);
    }

    let mobi_len = 232u32;
    let mut rec0 = vec![0u8; 16 + mobi_len as usize];
    rec0[0..2].copy_from_slice(&1u16.to_be_bytes()); // no compression
    rec0[16..20].copy_from_slice(b"MOBI");
    rec0[20..24].copy_from_slice(&mobi_len.to_be_bytes());
    rec0[24..28].copy_from_slice(&2u32.to_be_bytes());
    rec0[28..32].copy_from_slice(&65001u32.to_be_bytes());
    rec0[36..40].copy_from_slice(&(if kf8 { 8u32 } else { 6u32 }).to_be_bytes());
    let name_off = (rec0.len() + exth_block.len()) as u32;
    rec0[84..88].copy_from_slice(&name_off.to_be_bytes());
    rec0[88..92].copy_from_slice(&(title.len() as u32).to_be_bytes());
    rec0[108..112].copy_from_slice(&2u32.to_be_bytes()); // first image = record 2
    rec0[128..132].copy_from_slice(&0x40u32.to_be_bytes());
    rec0.extend_from_slice(&exth_block);
    rec0.extend_from_slice(title);
    rec0.extend_from_slice(&[0, 0]);

    let text = format!("<html><body>{}</body></html>", spec.description).into_bytes();
    let records: [&[u8]; 3] = [&rec0, &text, cover];
    let mut out = vec![0u8; 78];
    let name = spec.title.bytes().filter(u8::is_ascii_alphanumeric).take(31).collect::<Vec<_>>();
    out[..name.len()].copy_from_slice(&name);
    out[60..68].copy_from_slice(b"BOOKMOBI");
    out[76..78].copy_from_slice(&(records.len() as u16).to_be_bytes());
    let mut offset = 78 + records.len() * 8 + 2;
    for (i, r) in records.iter().enumerate() {
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&((i as u32) * 2).to_be_bytes());
        offset += r.len();
    }
    out.extend_from_slice(&[0, 0]);
    for r in records {
        out.extend_from_slice(r);
    }
    out
}

fn pdf_text(s: &str) -> String {
    let mut hex = String::from("<FEFF");
    for u in s.encode_utf16() {
        hex.push_str(&format!("{u:04X}"));
    }
    hex.push('>');
    hex
}

fn pdf(spec: &Spec) -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_owned(),
        format!("<< /Title {} /Author {} /Keywords {} /CreationDate (D:20190412120000Z) >>", pdf_text(&spec.title), pdf_text(&spec.author), pdf_text(&spec.tags.join(", "))),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info 4 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect()
}

pub fn generate(dir: &Path, count: usize) -> Result<()> {
    let covers = make_covers();
    let pages: Vec<Vec<u8>> = covers.iter().take(2).cloned().collect();
    (0..count).into_par_iter().try_for_each(|i| -> Result<()> {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (i as u64).wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
        for _ in 0..4 {
            rng.next();
        }
        let n_words = 1 + rng.next() % 4;
        let mut title: Vec<String> = (0..n_words).map(|_| rng.pick(WORDS).to_owned()).collect();
        title[0] = {
            let mut c = title[0].chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        let title = format!("{} {}", ["The", "A", "Our", "Their", ""][(rng.next() % 5) as usize], title.join(" of the ")).trim().to_owned();
        let title = format!("{title} {i}");
        let author = format!("{} {}", rng.pick(FIRST), rng.pick(LAST));
        let series = rng.chance(250).then(|| (format!("The {} Saga", rng.pick(WORDS)), (1 + rng.next() % 12) as u32));
        let tags = vec![rng.pick(&["Novel", "Fantasy", "Science fiction", "Essay", "Poetry", "History", "Comic", "Horror"]).to_owned()];
        let description = format!("A story about {} and {}.", rng.pick(WORDS), rng.pick(WORDS));
        let spec = Spec { title, author, series, tags, description };
        let cover = &covers[(rng.next() % covers.len() as u64) as usize];

        let roll = rng.next() % 100;
        let (ext, bytes) = match roll {
            0..=49 => ("epub", epub(&spec, cover, !rng.chance(50))),
            50..=64 => ("pdf", pdf(&spec)),
            65..=79 => ("cbz", cbz(&spec, cover, &pages)),
            80..=89 => ("mobi", mobi(&spec, cover, false)),
            _ => ("azw3", mobi(&spec, cover, true)),
        };
        let bytes = if rng.chance(5) { bytes[..bytes.len() / 3].to_vec() } else { bytes };

        let shard = format!("{:03}", i % 400);
        let folder = dir.join(shard).join(sanitize(&spec.author));
        fs::create_dir_all(&folder)?;
        fs::write(folder.join(format!("{} - {}.{ext}", sanitize(&spec.author), sanitize(&spec.title))), bytes)?;
        Ok(())
    })
}
