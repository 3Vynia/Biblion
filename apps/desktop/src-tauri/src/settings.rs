//! Small persistent app settings (library folders), stored as JSON next to
//! the database. Writes are atomic (temp file + rename) so a crash can never
//! leave a half-written file behind.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Library folders, canonicalised and never nested inside each other.
    pub roots: Vec<PathBuf>,
    /// Reader preferences (theme, font…). Owned and validated by the UI;
    /// stored as-is so new options don't need a backend change.
    pub reader: Option<serde_json::Value>,
    /// Interface preferences (theme, sidebar…), also owned by the UI.
    pub ui: Option<serde_json::Value>,
}

impl Settings {
    /// Loads settings; a missing or unreadable file yields the defaults
    /// (and the broken file is kept aside for inspection).
    pub fn load(path: &Path) -> Settings {
        let Ok(text) = fs::read_to_string(path) else { return Settings::default() };
        match serde_json::from_str(&text) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("ignoring corrupt settings file {}: {e}", path.display());
                let _ = fs::rename(path, path.with_extension("json.corrupt"));
                Settings::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(io::Error::other)?)?;
        fs::rename(tmp, path)
    }

    /// Adds a library folder. Returns an error message for the UI when the
    /// folder is invalid or already covered by another root. When the new
    /// folder contains existing roots, they are replaced by it, so no file is
    /// ever walked twice.
    pub fn add_root(&mut self, path: &Path) -> Result<PathBuf, String> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("Cannot open folder {}: {e}", path.display()))?;
        if !path.is_dir() {
            return Err(format!("{} is not a folder", path.display()));
        }
        if let Some(parent) = self.roots.iter().find(|r| path.starts_with(r)) {
            return Err(format!("{} is already part of the library ({})", path.display(), parent.display()));
        }
        self.roots.retain(|r| !r.starts_with(&path));
        self.roots.push(path.clone());
        self.roots.sort();
        Ok(path)
    }

    pub fn remove_root(&mut self, path: &Path) -> bool {
        let before = self.roots.len();
        self.roots.retain(|r| r != path);
        self.roots.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_are_canonical_and_never_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let a_sub = a.join("sub");
        let b = tmp.path().join("b");
        fs::create_dir_all(&a_sub).unwrap();
        fs::create_dir_all(&b).unwrap();

        let mut s = Settings::default();
        s.add_root(&a_sub).unwrap();
        s.add_root(&b).unwrap();
        // The parent replaces the nested root.
        s.add_root(&a.join("sub/..")).unwrap();
        assert_eq!(s.roots, vec![a.canonicalize().unwrap(), b.canonicalize().unwrap()]);
        // A folder inside an existing root is rejected.
        assert!(s.add_root(&a_sub).unwrap_err().contains("already part"));
        // A file or a missing path is rejected.
        fs::write(tmp.path().join("f.epub"), b"x").unwrap();
        assert!(s.add_root(&tmp.path().join("f.epub")).is_err());
        assert!(s.add_root(&tmp.path().join("nope")).is_err());

        assert!(s.remove_root(&b.canonicalize().unwrap()));
        assert!(!s.remove_root(&b));
        assert_eq!(s.roots.len(), 1);
    }

    #[test]
    fn save_load_roundtrip_and_corrupt_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());

        let s = Settings { roots: vec![PathBuf::from("/books")], reader: Some(serde_json::json!({ "theme": "sepia" })), ui: None };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);

        fs::write(&path, b"{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        assert!(tmp.path().join("settings.json.corrupt").exists());
    }
}
