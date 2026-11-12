use std::io::Read;
use std::path::Path;

use crate::Metadata;

/// Entry names of a zip (skips undecodable names).
pub fn zip_names<R: std::io::Read + std::io::Seek>(zip: &zip::ZipArchive<R>) -> Vec<String> {
    zip.file_names().filter_map(|r| r.ok()).map(|c| c.into_owned()).collect()
}

/// Reads at most `limit` bytes from a reader.
pub fn read_limited<R: Read>(r: R, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    r.take(limit + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > limit {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "entry too large"));
    }
    Ok(buf)
}

pub fn is_image_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp"].iter().any(|e| lower.ends_with(e))
}

/// Checks the magic bytes of a supported image.
pub fn looks_like_image(b: &[u8]) -> bool {
    b.starts_with(&[0xFF, 0xD8, 0xFF])
        || b.starts_with(b"\x89PNG")
        || b.starts_with(b"GIF8")
        || (b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP")
        || b.starts_with(b"BM")
}

/// "Natural" order: `page2` < `page10`. Case-insensitive.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = ai.peek().copied().filter(|c| c.is_ascii_digit()) {
                    na.push(c);
                    ai.next();
                }
                let mut nb = String::new();
                while let Some(c) = bi.peek().copied().filter(|c| c.is_ascii_digit()) {
                    nb.push(c);
                    bi.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

/// Decodes `%20` and friends in resource paths.
pub fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolves `href` relative to `base_dir` inside a zip archive.
pub fn resolve_zip_path(base_dir: &str, href: &str) -> String {
    let href = href.split('#').next().unwrap_or("");
    let href = percent_decode(href);
    let mut parts: Vec<&str> = if href.starts_with('/') {
        Vec::new()
    } else {
        base_dir.split('/').filter(|p| !p.is_empty()).collect()
    };
    for seg in href.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Strips HTML tags and collapses whitespace (for descriptions).
pub fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    collapse_ws(&out)
}

pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clean_opt(v: &mut Option<String>) {
    if let Some(s) = v {
        let c = collapse_ws(s);
        *v = if c.is_empty() || c.eq_ignore_ascii_case("unknown") { None } else { Some(c) };
    }
}

pub fn normalize_metadata(m: &mut Metadata) {
    clean_opt(&mut m.title);
    clean_opt(&mut m.series);
    clean_opt(&mut m.publisher);
    clean_opt(&mut m.language);
    clean_opt(&mut m.isbn);
    clean_opt(&mut m.published);
    clean_opt(&mut m.subtitle);
    clean_opt(&mut m.rights);
    clean_opt(&mut m.source);
    clean_opt(&mut m.modified);
    if m.subtitle.is_some() && m.subtitle == m.title {
        m.subtitle = None;
    }
    for c in &mut m.contributors {
        c.name = collapse_ws(&c.name);
    }
    m.contributors.retain(|c| !c.name.is_empty() && !m.authors.iter().any(|a| a.eq_ignore_ascii_case(&c.name)));
    for i in &mut m.identifiers {
        i.value = collapse_ws(&i.value);
    }
    m.identifiers.retain(|i| !i.value.is_empty() && i.value.len() < 200);
    if let Some(d) = &m.description {
        let d = strip_html(d);
        m.description = if d.is_empty() { None } else { Some(d) };
    }
    let mut seen = std::collections::HashSet::new();
    m.authors = m
        .authors
        .iter()
        .flat_map(|a| split_authors(a))
        .filter(|a| !a.is_empty() && !a.eq_ignore_ascii_case("unknown"))
        .filter(|a| seen.insert(a.to_lowercase()))
        .collect();
    let mut seen = std::collections::HashSet::new();
    m.tags = m
        .tags
        .iter()
        .map(|t| collapse_ws(t))
        .filter(|t| !t.is_empty() && t.len() < 100)
        .filter(|t| seen.insert(t.to_lowercase()))
        .collect();
    if let Some(isbn) = &m.isbn {
        let digits: String = isbn.chars().filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x').collect();
        m.isbn = if digits.len() == 10 || digits.len() == 13 { Some(digits.to_uppercase()) } else { None };
    }
}

/// "A & B", "A; B", "A and B" → several authors. Does not split on commas
/// because "Last, First" is very common.
fn split_authors(s: &str) -> Vec<String> {
    let s = collapse_ws(s);
    s.split([';', '&'])
        .flat_map(|p| p.split(" and ").map(str::to_owned).collect::<Vec<_>>())
        .map(|p| p.trim().to_owned())
        .collect()
}

/// Fills in title/author from the path when missing: first the file name,
/// then the folder it sits in (`Author Name/Title.epub`).
pub fn fill_from_path(m: &mut Metadata, path: &Path) {
    fill_from_filename(m, path);
    if m.authors.is_empty() {
        if let Some(a) = author_from_folder(path) {
            m.authors.push(a);
        }
    }
}

/// Words that may start in lower case inside a person's name.
const NAME_PARTICLES: &[&str] = &[
    "de", "del", "la", "las", "los", "y", "e", "i", "van", "von", "der", "den", "da", "di", "do", "du", "le", "dos", "das", "al", "bin",
    "ibn", "ben", "zu", "ter",
];

/// Words of folder names that sort books by kind rather than by author
/// ("Ciencia Ficción", "Historical Fiction", "Varios Autores"…).
const NOT_NAMES: &[&str] = &[
    "adult", "adults", "adventure", "adventures", "art", "arte", "author", "authors", "autor", "autores", "aventura", "aventuras",
    "biblioteca", "bibliotecas", "biografia", "biografias", "biography", "book", "books", "calibre", "children", "ciencia", "ciencias",
    "classics", "clasicos", "cocina", "coleccion", "colecciones", "collection", "collections", "comic", "comics", "cooking", "crime",
    "cuentos", "descargas", "desconocido", "documents", "downloads", "drama", "ebook", "ebooks", "economia", "economics", "ensayo",
    "ensayos", "erotica", "fantasia", "fantasy", "fiction", "ficcion", "filosofia", "general", "historia", "historias", "historical",
    "history", "horror", "humor", "infantil", "juvenil", "leidos", "libro", "libros", "library", "literatura", "literature", "magazines",
    "manga", "manuales", "misterio", "mystery", "new", "nonfiction", "novela", "novelas", "novels", "otros", "pendientes", "philosophy",
    "poesia", "poetry", "policiaca", "politica", "politics", "psicologia", "psychology", "read", "relatos", "religion", "revistas",
    "romance", "romantica", "saga", "sagas", "science", "series", "short", "stories", "teatro", "terror", "thriller", "thrillers",
    "travel", "unknown", "unread", "various", "varios", "viajes", "western", "young",
];

/// Whether `s` reads like a person's name: two to six words, no digits,
/// every word capitalised except particles such as "de" or "von", and
/// none of them a genre or category word.
pub fn looks_like_name(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    if !(2..=6).contains(&words.len()) || s.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    if words.iter().any(|w| NOT_NAMES.contains(&fold_diacritics(&w.to_lowercase()).as_str())) {
        return false;
    }
    let capital = |w: &str| w.chars().next().is_some_and(char::is_uppercase);
    capital(words[0])
        && capital(words[words.len() - 1])
        && words.iter().all(|w| {
            (capital(w) || NAME_PARTICLES.contains(&w.to_lowercase().as_str()))
                && w.chars().all(|c| c.is_alphabetic() || matches!(c, '.' | '\'' | '’' | '-'))
        })
}

/// The author named by the folder a book is in, when that folder looks like
/// a person's name. Calibre's `Author/Title (12)/file` layout is understood.
pub fn author_from_folder(path: &Path) -> Option<String> {
    let mut dirs = path.ancestors().skip(1).filter_map(|d| d.file_name()?.to_str());
    let mut dir = dirs.next()?;
    let calibre = dir.strip_suffix(')').and_then(|d| d.rsplit_once(" (")).is_some_and(|(_, n)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    if calibre {
        dir = dirs.next()?;
    }
    if dir.starts_with(['_', '.']) {
        return None;
    }
    let name = collapse_ws(&dir.replace('_', " "));
    looks_like_name(&name).then_some(name)
}

/// Fills in title/author from the file name when missing.
/// Recognises "Author - Title", "Title (Author)" and "Title-Author_Name".
pub fn fill_from_filename(m: &mut Metadata, path: &Path) {
    let raw = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let stem = collapse_ws(&raw.replace('_', " "));
    let has_title = m.title.as_deref().is_some_and(|t| !t.trim().is_empty());
    if has_title && !m.authors.is_empty() {
        return;
    }
    let (author, title) = if let Some((a, t)) = stem.split_once(" - ") {
        (Some(a.trim().to_owned()), t.trim().to_owned())
    } else if let (Some(open), true) = (stem.rfind('('), stem.ends_with(')')) {
        (Some(stem[open + 1..stem.len() - 1].trim().to_owned()), stem[..open].trim().to_owned())
    } else if let Some((t, a)) = stem.rsplit_once('-').filter(|(t, a)| raw.contains('_') && !t.trim().is_empty() && looks_like_name(a)) {
        // "Habla_el_General-Agustin_Blanco_Munoz": words joined by "_", author after the last "-".
        (Some(a.trim().to_owned()), t.trim().to_owned())
    } else {
        (None, stem.clone())
    };
    if !has_title {
        m.title = Some(if title.is_empty() { stem } else { title });
    }
    if m.authors.is_empty() {
        if let Some(a) = author.filter(|a| !a.is_empty() && !a.chars().all(|c| c.is_ascii_digit())) {
            m.authors.push(a);
        }
    }
}

/// Sort key: leading articles removed, lowercase, no diacritics.
/// Width numbers are padded to, so "5" sorts before "11".
const NUMBER_WIDTH: usize = 10;

/// Key for sorting titles: lowercase, without accents or punctuation, without
/// a leading article, and with numbers padded so they sort naturally.
pub fn sort_key(s: &str) -> String {
    let folded = fold_diacritics(&s.to_lowercase());
    // Punctuation inside a word disappears: "(des)ventajas" → "desventajas".
    let words: Vec<String> = folded
        .split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|w| !w.is_empty())
        .map(|w| pad_numbers(&w))
        .collect();
    const ARTICLES: [&str; 14] = ["the", "a", "an", "el", "la", "los", "las", "un", "una", "le", "les", "der", "die", "das"];
    // "A & C" keeps its "a": an article is followed by a real word.
    let skip = usize::from(words.len() > 1 && ARTICLES.contains(&words[0].as_str()) && words[1].chars().count() > 1);
    words[skip..].join(" ")
}

fn pad_numbers(word: &str) -> String {
    let mut out = String::with_capacity(word.len() + NUMBER_WIDTH);
    let mut digits = String::new();
    for c in word.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        if !digits.is_empty() {
            let trimmed = digits.trim_start_matches('0');
            let n = if trimmed.is_empty() { "0" } else { trimmed };
            out.extend(std::iter::repeat_n('0', NUMBER_WIDTH.saturating_sub(n.len())));
            out.push_str(n);
            digits.clear();
        }
        if c != ' ' {
            out.push(c);
        }
    }
    out
}

/// "Gabriel García Márquez" → "marquez, gabriel garcia" (simple heuristic).
pub fn author_sort_key(name: &str) -> String {
    let name = fold_diacritics(&name.to_lowercase());
    if name.contains(',') {
        return name;
    }
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.len() {
        0 | 1 => name,
        _ => format!("{}, {}", parts[parts.len() - 1], parts[..parts.len() - 1].join(" ")),
    }
}

pub fn fold_diacritics(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'ø' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort() {
        let mut v = vec!["p10.jpg", "p2.jpg", "P1.jpg", "p02b.jpg"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["P1.jpg", "p2.jpg", "p02b.jpg", "p10.jpg"]);
    }

    #[test]
    fn resolve_paths() {
        assert_eq!(resolve_zip_path("OEBPS/text", "../images/c%20v.jpg#x"), "OEBPS/images/c v.jpg");
        assert_eq!(resolve_zip_path("", "cover.jpg"), "cover.jpg");
    }

    #[test]
    fn filename_parsing() {
        let mut m = Metadata::default();
        fill_from_filename(&mut m, Path::new("/x/Isaac Asimov - Fundación.epub"));
        assert_eq!(m.title.as_deref(), Some("Fundación"));
        assert_eq!(m.authors, vec!["Isaac Asimov"]);
    }

    #[test]
    fn author_from_the_path() {
        let parse = |p: &str| {
            let mut m = Metadata::default();
            fill_from_path(&mut m, Path::new(p));
            (m.title.unwrap_or_default(), m.authors.join("; "))
        };
        assert_eq!(parse("/b/Frank_Caudett/002_contra_Andromeda.epub"), ("002 contra Andromeda".into(), "Frank Caudett".into()));
        assert_eq!(parse("/b/Marta_Rivera_de_la_Cruz/Que_veinte.epub").1, "Marta Rivera de la Cruz");
        assert_eq!(parse("/b/Jordi_Sierra_i_Fabra/Algunos_dias.epub").1, "Jordi Sierra i Fabra");
        assert_eq!(parse("/b/_Otros/Habla_el_General-Agustin_Blanco_Munoz.epub"), ("Habla el General".into(), "Agustin Blanco Munoz".into()));
        assert_eq!(parse("/b/_Otros/Heroes_de_Tarsi-A_J_Ogayas.epub").1, "A J Ogayas");
        assert_eq!(parse("/c/Ursula K. Le Guin/A Wizard of Earthsea (42)/book.epub").1, "Ursula K. Le Guin");
        // Folders and hyphens that are not names are left alone.
        assert_eq!(parse("/b/_Otros/Sin_autor.epub").1, "");
        assert_eq!(parse("/b/Science Fiction 2024/x.epub").1, "");
        assert_eq!(parse("/b/epub/x.epub").1, "");
        assert_eq!(parse("/b/Varios_autores/x.epub").1, "");
        assert_eq!(parse("/b/Ciencia_Ficción/x.epub").1, "");
        assert_eq!(parse("/b/Historical Fiction/x.epub").1, "");
        assert_eq!(parse("/b/Varios Autores/x.epub").1, "");
        assert_eq!(parse("/b/Comics/The_Amazing_Spider-Man.cbz"), ("The Amazing Spider-Man".into(), "".into()));
        assert_eq!(parse("/b/Novelas/Spider-Man Returns.epub").1, "");
    }

    #[test]
    fn sorting_keys() {
        assert_eq!(sort_key("El Señor de los Anillos"), "senor de los anillos");
        // Punctuation never decides the order.
        assert_eq!(sort_key("El 'Problema Ruso' al final"), "problema ruso al final");
        assert_eq!(sort_key("Las (des)ventajas de ser virgen"), "desventajas de ser virgen");
        assert_eq!(sort_key("¡Hola!"), "hola");
        // A lone letter after "A" is not an article.
        assert_eq!(sort_key("A & C Una historia"), "a c una historia");
        // Numbers sort naturally.
        assert!(sort_key("Saga #5") < sort_key("Saga #11"));
        assert!(sort_key("Abyss 10466") < sort_key("Abyss 100313"));
        assert!(sort_key("002 contra Andromeda") < sort_key("Andromeda"));
        assert_eq!(author_sort_key("Gabriel García Márquez"), "marquez, gabriel garcia");
    }
}
