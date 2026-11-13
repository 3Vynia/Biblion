//! The user's own music folder, mixed into the reading ambience.
//!
//! The folder is listed into tracks kept in memory; the web view plays them
//! through `music://localhost/<n>`, so it never names a path and only files
//! from that listing can be read.

use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

/// Most tracks listed (a music folder, not a whole disk).
const MAX_TRACKS: usize = 20_000;
const MAX_DEPTH: usize = 8;

const AUDIO: [(&str, &str); 8] = [
    ("mp3", "audio/mpeg"),
    ("ogg", "audio/ogg"),
    ("oga", "audio/ogg"),
    ("opus", "audio/ogg"),
    ("flac", "audio/flac"),
    ("m4a", "audio/mp4"),
    ("aac", "audio/aac"),
    ("wav", "audio/wav"),
];

pub fn audio_type(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    AUDIO.iter().find(|(e, _)| *e == ext).map(|(_, t)| *t)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Track {
    pub index: usize,
    pub title: String,
    /// The sub-folder it is in (an album, usually), relative to the music folder.
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MusicLibrary {
    pub folder: Option<String>,
    pub tracks: Vec<Track>,
}

/// Lists the audio files under `root`, sorted by folder and name. Hidden
/// folders are skipped.
pub fn list(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = WalkDir::new(root)
        .max_depth(MAX_DEPTH)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !e.file_name().to_str().is_some_and(|n| n.starts_with('.')))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && audio_type(e.path()).is_some())
        .map(|e| e.into_path())
        .take(MAX_TRACKS)
        .collect();
    out.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    out
}

pub fn describe(root: &Path, files: &[PathBuf]) -> MusicLibrary {
    let tracks = files
        .iter()
        .enumerate()
        .map(|(index, p)| Track {
            index,
            title: p.file_stem().map(|s| s.to_string_lossy().replace('_', " ")).unwrap_or_default(),
            folder: p
                .parent()
                .and_then(|d| d.strip_prefix(root).ok())
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_default(),
        })
        .collect();
    MusicLibrary { folder: Some(root.display().to_string()), tracks }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_audio_files_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("Album B")).unwrap();
        std::fs::create_dir_all(root.join(".cache")).unwrap();
        for f in ["Album B/02_Rain.flac", "Album B/01 Night.MP3", "cover.jpg", ".cache/x.mp3", "notes.txt", "Calm.opus"] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        let files = list(root);
        let lib = describe(root, &files);
        let titles: Vec<(&str, &str)> = lib.tracks.iter().map(|t| (t.folder.as_str(), t.title.as_str())).collect();
        assert_eq!(titles, [("Album B", "01 Night"), ("Album B", "02 Rain"), ("", "Calm")]);
        assert_eq!(audio_type(Path::new("a.M4A")), Some("audio/mp4"));
        assert_eq!(audio_type(Path::new("a.txt")), None);
    }
}
