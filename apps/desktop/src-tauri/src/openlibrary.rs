//! Open Library (openlibrary.org): books. See `online` for how sources are
//! used. Parsing and cleaning are plain functions, tested without the network.

use std::time::Duration;

use serde_json::Value;

use crate::online::{strings, text, Candidate, Details, Http, NetError, MAX_COVER_BYTES};

const API: &str = "https://openlibrary.org";
const COVERS: &str = "https://covers.openlibrary.org";
const NAME: &str = "Open Library";

pub struct Client {
    http: Http,
}

impl Client {
    pub fn new() -> Self {
        // Gentle on a free service: at most a few requests a second.
        Self { http: Http::new(NAME, Duration::from_millis(350)) }
    }

    /// Searches by ISBN first (exact), then by title and author.
    pub fn search(&self, title: &str, author: &str, isbn: &str) -> Result<Vec<Candidate>, NetError> {
        let isbn: String = isbn.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if isbn.len() == 10 || isbn.len() == 13 {
            let found = parse_search(&self.http.get_json(&format!("{API}/search.json"), &[("isbn", &isbn), ("fields", FIELDS), ("limit", "5")])?);
            if !found.is_empty() {
                return Ok(found);
            }
        }
        let (title, author) = (title.trim(), author.trim());
        if title.is_empty() && author.is_empty() {
            return Ok(Vec::new());
        }
        let mut query = vec![("fields", FIELDS), ("limit", "10")];
        if !title.is_empty() {
            query.push(("title", title));
        }
        if !author.is_empty() {
            query.push(("author", author));
        }
        Ok(parse_search(&self.http.get_json(&format!("{API}/search.json"), &query)?))
    }

    pub fn work(&self, key: &str) -> Result<Details, NetError> {
        if !valid_work_key(key) {
            return Err(NetError::Invalid(NAME, format!("not a work key: {key}")));
        }
        Ok(parse_work(&self.http.get_json(&format!("{API}{key}.json"), &[])?))
    }

    /// A cover image; `large` for the one to keep, smaller ones for previews.
    pub fn cover(&self, id: i64, large: bool) -> Result<Vec<u8>, NetError> {
        if id <= 0 {
            return Err(NetError::Invalid(NAME, format!("not a cover id: {id}")));
        }
        let size = if large { "L" } else { "M" };
        // `default=false` answers 404 instead of a blank placeholder image.
        let bytes = self.http.get(&format!("{COVERS}/b/id/{id}-{size}.jpg"), &[("default", "false")], MAX_COVER_BYTES)?;
        if bytes.len() < 1024 {
            return Err(NetError::Http(NAME, 404));
        }
        Ok(bytes)
    }
}

const FIELDS: &str =
    "key,title,subtitle,author_name,first_publish_year,publisher,isbn,cover_i,language,number_of_pages_median,edition_count,subject";

pub fn valid_work_key(key: &str) -> bool {
    key.strip_prefix("/works/OL")
        .and_then(|k| k.strip_suffix('W'))
        .is_some_and(|n| !n.is_empty() && n.len() < 12 && n.bytes().all(|b| b.is_ascii_digit()))
}

pub fn parse_search(v: &Value) -> Vec<Candidate> {
    let Some(docs) = v["docs"].as_array() else { return Vec::new() };
    docs.iter()
        .filter_map(|d| {
            let key = text(&d["key"]).filter(|k| valid_work_key(k))?;
            let title = text(&d["title"])?;
            let isbns = strings(&d["isbn"], 200);
            let isbn = isbns.iter().find(|i| i.len() == 13).or_else(|| isbns.first()).cloned();
            let mut languages: Vec<String> = strings(&d["language"], 6).iter().map(|l| iso639_1(l)).collect();
            languages.dedup();
            Some(Candidate {
                source: "openlibrary".into(),
                url: Some(format!("{API}{key}")),
                key,
                kind: None,
                title,
                subtitle: text(&d["subtitle"]),
                alt_titles: Vec::new(),
                authors: strings(&d["author_name"], 6),
                year: d["first_publish_year"].as_i64().and_then(|y| i32::try_from(y).ok()),
                publishers: strings(&d["publisher"], 3),
                isbn,
                cover: d["cover_i"].as_i64().filter(|&c| c > 0).map(|c| format!("ol:{c}")),
                languages,
                pages: d["number_of_pages_median"].as_u64().and_then(|p| u32::try_from(p).ok()).filter(|&p| p > 0),
                editions: d["edition_count"].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32,
                volumes: None,
                subjects: clean_subjects(&strings(&d["subject"], 60)),
                description: None,
            })
        })
        .collect()
}

pub fn parse_work(v: &Value) -> Details {
    // A description is either a string or {"type": "/type/text", "value": "…"}.
    let raw = text(&v["description"]).or_else(|| text(&v["description"]["value"]));
    Details { description: raw.map(|d| clean_description(&d)).filter(|d| !d.is_empty()), subjects: clean_subjects(&strings(&v["subjects"], 60)) }
}

/// Library-catalogue subjects are noisy ("nyt:combined-print=2008-06-01",
/// "Fiction, general"). Keeps short, plain ones, without duplicates.
pub fn clean_subjects(raw: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    raw.iter()
        .map(|s| s.trim().trim_end_matches('.').trim())
        .filter(|s| !s.is_empty() && s.chars().count() <= 40 && !s.contains([':', '=', '(', '/']) && !s.chars().any(|c| c.is_ascii_digit()))
        .filter(|s| !CATALOGUE_NOISE.contains(&s.to_lowercase().as_str()))
        .map(|s| {
            let mut c = s.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .filter(|s| seen.insert(s.to_lowercase()))
        .take(10)
        .collect()
}

/// Subjects about the copy, not the book.
const CATALOGUE_NOISE: &[&str] = &[
    "accessible book",
    "protected daisy",
    "in library",
    "lending library",
    "large type books",
    "open library staff picks",
    "overdrive",
    "long now manual for civilization",
    "reading level-grade",
];

/// Open Library descriptions are Markdown with reference links and, often, a
/// "----------" block listing other editions. Returns plain paragraphs.
pub(crate) fn clean_description(raw: &str) -> String {
    let raw = raw.replace("\r\n", "\n");
    let body = raw.split("\n----------").next().unwrap_or("");
    let body = body.split("\n**Also contained in").next().unwrap_or("");
    let kept: Vec<&str> = body
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            // Reference definitions: "[1]: https://…"
            !(t.starts_with('[') && t.split_once("]:").is_some_and(|(n, _)| n[1..].chars().all(|c| c.is_ascii_alphanumeric())))
        })
        .collect();
    let mut text = strip_links(&kept.join("\n"));
    for junk in ["([source])", "(source)", "([Source])"] {
        text = text.replace(junk, "");
    }
    let text = text.replace("**", "").replace("__", "");
    // Collapse runs of blank lines.
    let mut out = String::new();
    for para in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(para);
    }
    out
}

/// "[text][1]" and "[text](url)" → "text".
fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let label = &after[..close];
        let tail = &after[close + 1..];
        let skip = match tail.chars().next() {
            Some('(') => tail.find(')').map(|e| e + 1),
            Some('[') => tail.find(']').map(|e| e + 1),
            _ => Some(0),
        };
        match skip {
            Some(n) => {
                out.push_str(label);
                rest = &tail[n..];
            }
            None => {
                out.push_str(&rest[open..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Open Library uses MARC (ISO 639-2/B) codes; the library uses ISO 639-1.
fn iso639_1(code: &str) -> String {
    let short = match code {
        "eng" => "en",
        "spa" => "es",
        "fre" | "fra" => "fr",
        "ger" | "deu" => "de",
        "ita" => "it",
        "por" => "pt",
        "rus" => "ru",
        "jpn" => "ja",
        "chi" | "zho" => "zh",
        "kor" => "ko",
        "cat" => "ca",
        "dut" | "nld" => "nl",
        "pol" => "pl",
        "swe" => "sv",
        "dan" => "da",
        "nor" => "no",
        "fin" => "fi",
        "gre" | "ell" => "el",
        "tur" => "tr",
        "ara" => "ar",
        "heb" => "he",
        "lat" => "la",
        "glg" => "gl",
        "baq" | "eus" => "eu",
        other => other,
    };
    short.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_results_are_parsed_and_cleaned() {
        let v = json!({ "numFound": 2, "docs": [
            { "key": "/works/OL27448W", "title": "The Lord of the Rings", "author_name": ["J.R.R. Tolkien"],
              "first_publish_year": 1954, "publisher": ["Allen & Unwin", "Houghton Mifflin", "Ballantine", "Other"],
              "isbn": ["0618640150", "9780618640157"], "cover_i": 9255566, "language": ["eng", "spa", "tlh"],
              "number_of_pages_median": 1193, "edition_count": 250,
              "subject": ["Fiction", "fantasy fiction", "Fantasy fiction", "nyt:combined-print=2008", "Middle Earth (Imaginary place)", "Hobbits."] },
            { "key": "/authors/OL1A", "title": "Not a work" },
            { "key": "/works/OL1W" }
        ]});
        let found = parse_search(&v);
        assert_eq!(found.len(), 1);
        let c = &found[0];
        assert_eq!(c.key, "/works/OL27448W");
        assert_eq!(c.year, Some(1954));
        assert_eq!(c.publishers.len(), 3);
        assert_eq!(c.isbn.as_deref(), Some("9780618640157"));
        assert_eq!(c.languages, ["en", "es", "tlh"]);
        assert_eq!(c.subjects, ["Fiction", "Fantasy fiction", "Hobbits"]);
        assert_eq!(c.cover.as_deref(), Some("ol:9255566"));
    }

    #[test]
    fn broken_search_answers_give_nothing() {
        assert!(parse_search(&json!({})).is_empty());
        assert!(parse_search(&json!({ "docs": "nope" })).is_empty());
        assert!(parse_search(&json!({ "docs": [null, 3, { "key": 5 }] })).is_empty());
    }

    #[test]
    fn work_descriptions_lose_markdown_and_edition_lists() {
        let v = json!({
            "description": { "type": "/type/text", "value":
                "A hobbit goes on a [journey][1].\r\n\r\n\r\nIt is **long** ([source][2])\n\n[1]: https://example.org\n[2]: https://example.org/2\n----------\nAlso contained in:\n- Something" },
            "subjects": ["Dragons", "Dragons", "Ring", "Accessible book", "Protected DAISY", "Places: Middle-earth"]
        });
        let w = parse_work(&v);
        assert_eq!(w.description.as_deref(), Some("A hobbit goes on a journey.\n\nIt is long"));
        assert_eq!(w.subjects, ["Dragons", "Ring"]);
        assert_eq!(parse_work(&json!({ "description": "Plain [link](http://x.y) text" })).description.as_deref(), Some("Plain link text"));
        assert_eq!(parse_work(&json!({ "description": "  " })).description, None);
        assert_eq!(parse_work(&json!(null)), Details::default());
    }

    #[test]
    fn only_work_keys_are_fetched() {
        assert!(valid_work_key("/works/OL27448W"));
        for bad in ["/works/OLW", "/works/OL12A", "/authors/OL1W", "/works/OL1W/../x", "https://x/works/OL1W", "/works/OL1234567890123W"] {
            assert!(!valid_work_key(bad), "{bad}");
        }
    }

    /// Talks to the real service: `cargo test -- --ignored live`.
    #[test]
    #[ignore]
    fn live_search_work_and_cover() {
        let c = Client::new();
        let found = c.search("The Hobbit", "Tolkien", "").unwrap();
        let hobbit = found.iter().find(|b| b.title == "The Hobbit").expect("found The Hobbit");
        println!("{hobbit:#?}");
        let work = c.work(&hobbit.key).unwrap();
        println!("{work:#?}");
        assert!(work.description.is_some());
        let id = hobbit.cover.as_deref().unwrap().strip_prefix("ol:").unwrap().parse().unwrap();
        let cover = c.cover(id, false).unwrap();
        assert!(cover.starts_with(&[0xff, 0xd8]), "a JPEG");
        let by_isbn = c.search("", "", "978-0-261-10221-7").unwrap();
        println!("by isbn: {:?}", by_isbn.iter().map(|b| &b.title).collect::<Vec<_>>());
    }

    #[test]
    fn unclosed_brackets_survive() {
        assert_eq!(strip_links("a [b c"), "a [b c");
        assert_eq!(strip_links("[x](unclosed"), "[x](unclosed");
        assert_eq!(strip_links("[x] y"), "x y");
    }
}
