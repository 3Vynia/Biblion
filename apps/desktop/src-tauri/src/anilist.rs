//! AniList (anilist.co): manga, manhwa, manhua, light novels and one-shots.
//! See `online` for how sources are used. No account or key is needed.

use std::time::Duration;

use serde_json::{json, Value};

use crate::online::{strings, text, Candidate, Http, NetError, MAX_COVER_BYTES};

const API: &str = "https://graphql.anilist.co";
const COVER_HOST: &str = "https://s4.anilist.co/file/anilistcdn/";
const NAME: &str = "AniList";

const QUERY: &str = "query ($search: String) {
  Page(perPage: 10) {
    media(search: $search, type: MANGA, sort: SEARCH_MATCH) {
      id title { romaji english native } format countryOfOrigin
      startDate { year } description(asHtml: false) genres
      tags { name rank isMediaSpoiler }
      staff(perPage: 8) { edges { role node { name { full } } } }
      coverImage { extraLarge large } volumes siteUrl
    }
  }
}";

pub struct Client {
    http: Http,
}

impl Client {
    pub fn new() -> Self {
        // AniList allows 90 requests a minute.
        Self { http: Http::new(NAME, Duration::from_millis(700)) }
    }

    /// AniList searches titles only, in any of their languages.
    pub fn search(&self, title: &str, _author: &str) -> Result<Vec<Candidate>, NetError> {
        let title = title.trim();
        if title.is_empty() {
            return Ok(Vec::new());
        }
        let v = self.http.post_json(API, &json!({ "query": QUERY, "variables": { "search": title } }))?;
        if let Some(message) = v["errors"][0]["message"].as_str() {
            // "Not Found." is AniList's way of saying no matches.
            if message.to_lowercase().contains("not found") {
                return Ok(Vec::new());
            }
            return Err(NetError::Invalid(NAME, message.to_owned()));
        }
        Ok(parse_search(&v))
    }

    pub fn cover(&self, url: &str) -> Result<Vec<u8>, NetError> {
        if !valid_cover_url(url) {
            return Err(NetError::Invalid(NAME, "not an AniList image".into()));
        }
        let bytes = self.http.get(url, &[], MAX_COVER_BYTES)?;
        if bytes.len() < 1024 {
            return Err(NetError::Http(NAME, 404));
        }
        Ok(bytes)
    }
}

/// Covers are only fetched from AniList's own image server.
pub fn valid_cover_url(url: &str) -> bool {
    url.starts_with(COVER_HOST) && url.len() < 300 && !url.contains("..") && !url.contains(['?', '#', '@', '\\'])
}

fn kind(format: &str, country: &str) -> Option<String> {
    let k = match (format, country) {
        ("NOVEL", _) => "Light novel",
        ("ONE_SHOT", _) => "One-shot",
        ("MANGA", "KR") => "Manhwa",
        ("MANGA", "CN" | "TW" | "HK") => "Manhua",
        ("MANGA", _) => "Manga",
        _ => return None,
    };
    Some(k.to_owned())
}

/// Tags that describe the print, not the story.
const FORMAT_TAGS: &[&str] = &["Full Color", "Long Strip", "4-koma", "Webcomic", "Primarily Adult Cast", "Primarily Male Cast", "Primarily Female Cast"];

pub fn parse_search(v: &Value) -> Vec<Candidate> {
    let Some(media) = v["data"]["Page"]["media"].as_array() else { return Vec::new() };
    media
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_i64().filter(|&i| i > 0)?;
            let names = [text(&m["title"]["english"]), text(&m["title"]["romaji"]), text(&m["title"]["native"])];
            let title = names.iter().flatten().next()?.clone();
            let mut alt_titles: Vec<String> = names.into_iter().flatten().filter(|n| *n != title).collect();
            alt_titles.dedup();

            // Writers and artists, not translators or letterers.
            let mut authors: Vec<String> = Vec::new();
            for e in m["staff"]["edges"].as_array().into_iter().flatten() {
                let role = e["role"].as_str().unwrap_or("").to_lowercase();
                let creates = role.starts_with("story") || role.starts_with("art") || role.starts_with("original");
                if let (true, Some(name)) = (creates, text(&e["node"]["name"]["full"])) {
                    if !authors.contains(&name) && authors.len() < 4 {
                        authors.push(name);
                    }
                }
            }

            let mut subjects = strings(&m["genres"], 10);
            for t in m["tags"].as_array().into_iter().flatten() {
                let (Some(name), rank) = (text(&t["name"]), t["rank"].as_i64().unwrap_or(0)) else { continue };
                if rank >= 60 && !t["isMediaSpoiler"].as_bool().unwrap_or(true) && !FORMAT_TAGS.contains(&name.as_str()) && !subjects.contains(&name) {
                    subjects.push(name);
                }
            }
            subjects.truncate(10);

            let cover = text(&m["coverImage"]["extraLarge"]).or_else(|| text(&m["coverImage"]["large"])).filter(|u| valid_cover_url(u));
            Some(Candidate {
                source: "anilist".into(),
                key: id.to_string(),
                kind: kind(m["format"].as_str().unwrap_or(""), m["countryOfOrigin"].as_str().unwrap_or("")),
                title,
                subtitle: None,
                alt_titles,
                authors,
                year: m["startDate"]["year"].as_i64().and_then(|y| i32::try_from(y).ok()),
                publishers: Vec::new(),
                isbn: None,
                cover,
                languages: Vec::new(),
                pages: None,
                editions: 0,
                volumes: m["volumes"].as_u64().and_then(|n| u32::try_from(n).ok()).filter(|&n| n > 0),
                subjects,
                description: text(&m["description"]).map(|d| clean_description(&d)).filter(|d| !d.is_empty()),
                url: text(&m["siteUrl"]).filter(|u| u.starts_with("https://anilist.co/")),
            })
        })
        .collect()
}

/// AniList descriptions carry a little HTML and a "(Source: …)" credit.
pub fn clean_description(raw: &str) -> String {
    let with_breaks = raw.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    // Drop the remaining tags.
    let mut plain = String::with_capacity(with_breaks.len());
    let mut in_tag = false;
    for c in with_breaks.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => plain.push(c),
            _ => {}
        }
    }
    let plain = plain.replace("&amp;", "&").replace("&quot;", "\"").replace("&#039;", "'").replace("&lt;", "<").replace("&gt;", ">");
    let paragraphs: Vec<&str> = plain
        .split('\n')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter(|p| !(p.starts_with("(Source:") || p.starts_with("(source:")) && !p.starts_with("Note:"))
        .collect();
    paragraphs.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manga_manhwa_and_novels_are_told_apart() {
        let v = json!({ "data": { "Page": { "media": [
            { "id": 105398, "title": { "romaji": "Na Honjaman Level Up", "english": "Solo Leveling", "native": "나 혼자만 레벨업" },
              "format": "MANGA", "countryOfOrigin": "KR", "startDate": { "year": 2018 },
              "description": "Hunters fight monsters.\n<br><br>\n(Source: Tappytoon)\n<br><br>\n<i>Note: Includes extras.</i>",
              "genres": ["Action", "Fantasy"],
              "tags": [ { "name": "Dungeon", "rank": 95, "isMediaSpoiler": false }, { "name": "War", "rank": 90, "isMediaSpoiler": true },
                        { "name": "Full Color", "rank": 89, "isMediaSpoiler": false }, { "name": "Gods", "rank": 40, "isMediaSpoiler": false } ],
              "staff": { "edges": [ { "role": "Story", "node": { "name": { "full": "Chugong" } } },
                                    { "role": "Art", "node": { "name": { "full": "Jang Sung-Rak" } } },
                                    { "role": "Translator (English)", "node": { "name": { "full": "Someone" } } } ] },
              "coverImage": { "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/manga/cover/large/bx105398.jpg" },
              "volumes": 14, "siteUrl": "https://anilist.co/manga/105398" },
            { "id": 2, "title": { "romaji": "Kono Light Novel" }, "format": "NOVEL", "countryOfOrigin": "JP" },
            { "id": 3, "title": {}, "format": "MANGA" },
            { "id": 4, "title": { "romaji": "Bad cover" }, "format": "MANGA", "countryOfOrigin": "JP",
              "coverImage": { "large": "https://evil.example/x.jpg" } }
        ] } } });
        let found = parse_search(&v);
        assert_eq!(found.len(), 3);
        let s = &found[0];
        assert_eq!((s.title.as_str(), s.kind.as_deref(), s.year), ("Solo Leveling", Some("Manhwa"), Some(2018)));
        assert_eq!(s.alt_titles, ["Na Honjaman Level Up", "나 혼자만 레벨업"]);
        assert_eq!(s.authors, ["Chugong", "Jang Sung-Rak"]);
        assert_eq!(s.subjects, ["Action", "Fantasy", "Dungeon"]);
        assert_eq!(s.description.as_deref(), Some("Hunters fight monsters."));
        assert_eq!(s.volumes, Some(14));
        assert!(s.cover.is_some());
        assert_eq!(found[1].kind.as_deref(), Some("Light novel"));
        assert_eq!(found[2].cover, None, "covers come only from AniList's server");
    }

    #[test]
    fn errors_and_odd_answers() {
        assert!(parse_search(&json!({})).is_empty());
        assert!(parse_search(&json!({ "data": { "Page": { "media": [null, 1, { "id": -1 }] } } })).is_empty());
        assert!(!valid_cover_url("https://s4.anilist.co/file/anilistcdn/../../etc"));
        assert!(!valid_cover_url("https://s4.anilist.co.evil.example/file/anilistcdn/x.jpg"));
        assert!(!valid_cover_url("http://s4.anilist.co/file/anilistcdn/x.jpg"));
    }

    #[test]
    #[ignore]
    fn live_search() {
        let found = Client::new().search("Solo Leveling", "").unwrap();
        println!("{:#?}", found.first());
        assert!(found.iter().any(|c| c.kind.as_deref() == Some("Manhwa")));
    }
}
