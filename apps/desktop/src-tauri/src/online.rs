//! Book details from online sources, searched all at once: Open Library for
//! books, AniList for manga, manhwa, manhua and light novels. Used only when
//! the user turns it on and asks for one book; what each source answers is
//! turned into the same `Candidate`, so the UI shows one list.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{anilist, openlibrary};

/// Sources identify clients; no personal details are sent.
const USER_AGENT: &str = "Biblion/0.1 (local library manager)";
pub const MAX_JSON_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_COVER_BYTES: u64 = 12 * 1024 * 1024;

/// One match from any source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// "openlibrary" or "anilist".
    pub source: String,
    /// The source's own id ("/works/OL45804W", "105398").
    pub key: String,
    /// What it is, when the source says: "Manga", "Manhwa", "Light novel"…
    pub kind: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    /// Other names: romaji, native script, English.
    pub alt_titles: Vec<String>,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub publishers: Vec<String>,
    pub isbn: Option<String>,
    /// Opaque token for `Sources::cover` ("ol:123" or an image URL).
    pub cover: Option<String>,
    /// ISO 639-1 codes when known ("en").
    pub languages: Vec<String>,
    pub pages: Option<u32>,
    pub editions: u32,
    pub volumes: Option<u32>,
    pub subjects: Vec<String>,
    /// When the search already brings it (AniList); else see `details`.
    pub description: Option<String>,
    /// The book's page on the source, to open in a browser.
    pub url: Option<String>,
}

/// What only a second request brings: the long description and subjects.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Details {
    pub description: Option<String>,
    pub subjects: Vec<String>,
}

/// One source's answer: its matches, or why it had none.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceResults {
    pub source: String,
    pub label: String,
    pub candidates: Vec<Candidate>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub enum NetError {
    Offline(&'static str, String),
    Http(&'static str, u16),
    Invalid(&'static str, String),
}

impl NetError {
    pub fn not_found(&self) -> bool {
        matches!(self, NetError::Http(_, 404))
    }
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetError::Offline(s, e) => write!(f, "Could not reach {s}: {e}"),
            NetError::Http(s, 404) => write!(f, "{s} has nothing for this book"),
            NetError::Http(s, 429) => write!(f, "{s} asks to slow down. Try again in a minute."),
            NetError::Http(s, code) => write!(f, "{s} answered with an error ({code})"),
            NetError::Invalid(s, e) => write!(f, "{s} sent something unexpected: {e}"),
        }
    }
}

/// A polite HTTP client for one source: a gap between requests, timeouts and
/// size limits.
pub struct Http {
    name: &'static str,
    agent: ureq::Agent,
    gap: Duration,
    last: Mutex<Option<Instant>>,
}

impl Http {
    pub fn new(name: &'static str, gap: Duration) -> Self {
        let agent = ureq::Agent::config_builder().user_agent(USER_AGENT).timeout_global(Some(Duration::from_secs(20))).build().into();
        Self { name, agent, gap, last: Mutex::new(None) }
    }

    fn pace(&self) {
        let mut last = self.last.lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed() < self.gap {
                std::thread::sleep(self.gap - t.elapsed());
            }
        }
        *last = Some(Instant::now());
    }

    fn fail(&self, e: ureq::Error) -> NetError {
        match e {
            ureq::Error::StatusCode(code) => NetError::Http(self.name, code),
            e => NetError::Offline(self.name, e.to_string()),
        }
    }

    pub fn get(&self, url: &str, query: &[(&str, &str)], limit: u64) -> Result<Vec<u8>, NetError> {
        self.pace();
        let mut req = self.agent.get(url);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        let mut res = req.call().map_err(|e| self.fail(e))?;
        res.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| NetError::Offline(self.name, e.to_string()))
    }

    pub fn get_json(&self, url: &str, query: &[(&str, &str)]) -> Result<Value, NetError> {
        let bytes = self.get(url, query, MAX_JSON_BYTES)?;
        serde_json::from_slice(&bytes).map_err(|e| NetError::Invalid(self.name, e.to_string()))
    }

    pub fn post_json(&self, url: &str, body: &Value) -> Result<Value, NetError> {
        self.pace();
        let mut res = self
            .agent
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send(body.to_string())
            .map_err(|e| self.fail(e))?;
        let bytes = res.body_mut().with_config().limit(MAX_JSON_BYTES).read_to_vec().map_err(|e| NetError::Offline(self.name, e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| NetError::Invalid(self.name, e.to_string()))
    }
}

pub struct Sources {
    pub openlibrary: openlibrary::Client,
    pub anilist: anilist::Client,
}

impl Default for Sources {
    fn default() -> Self {
        Self { openlibrary: openlibrary::Client::new(), anilist: anilist::Client::new() }
    }
}

impl Sources {
    /// Asks every source at once. A source that fails does not hide the others.
    pub fn search(&self, title: &str, author: &str, isbn: &str) -> Vec<SourceResults> {
        let wrap = |source: &str, label: &str, r: Result<Vec<Candidate>, NetError>| match r {
            Ok(candidates) => SourceResults { source: source.into(), label: label.into(), candidates, error: None },
            Err(e) => SourceResults { source: source.into(), label: label.into(), candidates: Vec::new(), error: Some(e.to_string()) },
        };
        std::thread::scope(|s| {
            let ol = s.spawn(|| self.openlibrary.search(title, author, isbn));
            let al = s.spawn(|| self.anilist.search(title, author));
            let join = |h: std::thread::ScopedJoinHandle<'_, Result<Vec<Candidate>, NetError>>, name: &'static str| {
                h.join().unwrap_or_else(|_| Err(NetError::Invalid(name, "the search stopped".into())))
            };
            vec![wrap("openlibrary", "Open Library", join(ol, "Open Library")), wrap("anilist", "AniList", join(al, "AniList"))]
        })
    }

    /// The description and subjects, when the search did not bring them.
    pub fn details(&self, source: &str, key: &str) -> Result<Details, NetError> {
        match source {
            "openlibrary" => self.openlibrary.work(key),
            // AniList answers everything in the search.
            "anilist" => Ok(Details::default()),
            other => Err(NetError::Invalid("Biblion", format!("unknown source {other}"))),
        }
    }

    /// A cover image from a candidate's token. Only the sources' own image
    /// servers are ever contacted.
    pub fn cover(&self, token: &str, large: bool) -> Result<Vec<u8>, NetError> {
        if let Some(id) = token.strip_prefix("ol:") {
            let id = id.parse().map_err(|_| NetError::Invalid("Open Library", format!("not a cover id: {id}")))?;
            return self.openlibrary.cover(id, large);
        }
        if anilist::valid_cover_url(token) {
            return self.anilist.cover(token);
        }
        Err(NetError::Invalid("Biblion", "not a cover from a known source".into()))
    }
}

pub fn strings(v: &Value, take: usize) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).take(take).map(str::to_owned).collect())
        .unwrap_or_default()
}

pub fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
}
