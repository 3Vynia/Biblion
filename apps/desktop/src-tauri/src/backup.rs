//! Backup files: the library's user data (see `biblion_core::backup`) plus
//! the app's own preferences, as one JSON file. Also the automatic daily
//! backups kept in the data folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use biblion_core::backup::LibraryBackup;
use biblion_core::Library;
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

pub const FORMAT: &str = "biblion-backup";
/// Backups bigger than this are not read (a real one is a few MB).
const MAX_FILE: u64 = 512 * 1024 * 1024;
/// Automatic backups kept.
pub const KEEP: usize = 7;
const DAY: i64 = 86_400;

/// Preferences that travel with a backup (the library folders do not: they
/// belong to the computer).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackupSettings {
    pub reader: Option<serde_json::Value>,
    pub ui: Option<serde_json::Value>,
    pub ambience: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub format: String,
    pub created_at: i64,
    pub app_version: String,
    pub library: LibraryBackup,
    #[serde(default)]
    pub settings: BackupSettings,
}

/// Counts shown after saving a backup.
#[derive(Debug, Clone, Serialize)]
pub struct BackupSummary {
    pub path: String,
    pub created_at: i64,
    pub size: u64,
    pub books: usize,
    pub progress: usize,
    pub annotations: usize,
    pub shelves: usize,
    pub reading_days: usize,
}

pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// UTC date of a unix time, as YYYY-MM-DD (civil-from-days, no time zone needed).
pub fn date(unix: i64) -> String {
    let z = unix.div_euclid(DAY) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn build(lib: &Library, settings: &Settings) -> biblion_core::Result<BackupFile> {
    Ok(BackupFile {
        format: FORMAT.to_owned(),
        created_at: unix_now(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        library: lib.export_backup()?,
        settings: BackupSettings {
            reader: settings.reader.clone(),
            ui: settings.ui.clone(),
            ambience: settings.ambience.clone(),
        },
    })
}

/// Writes atomically (temp file + rename), so a crash never leaves half a backup.
pub fn write(file: &BackupFile, path: &Path) -> io::Result<BackupSummary> {
    let json = serde_json::to_vec_pretty(file).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, path)?;
    let l = &file.library;
    Ok(BackupSummary {
        path: path.display().to_string(),
        created_at: file.created_at,
        size: json.len() as u64,
        books: l.books.len(),
        progress: l.progress.len(),
        annotations: l.annotations.len(),
        shelves: l.shelves.len(),
        reading_days: l.reading_log.len(),
    })
}

pub fn read(path: &Path) -> Result<BackupFile, String> {
    let size = fs::metadata(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?.len();
    if size > MAX_FILE {
        return Err("This file is too big to be a Biblion backup.".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let file: BackupFile = serde_json::from_slice(&bytes).map_err(|_| "This is not a Biblion backup.".to_owned())?;
    if file.format != FORMAT {
        return Err("This is not a Biblion backup.".into());
    }
    if file.library.version > biblion_core::backup::FORMAT_VERSION {
        return Err("This backup comes from a newer Biblion. Update the app to restore it.".into());
    }
    Ok(file)
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoBackup {
    pub path: String,
    pub created_at: i64,
    pub size: u64,
}

pub fn auto_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

/// Automatic backups, newest first.
pub fn list_auto(data_dir: &Path) -> Vec<AutoBackup> {
    let Ok(entries) = fs::read_dir(auto_dir(data_dir)) else { return Vec::new() };
    let mut out: Vec<AutoBackup> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with("biblion-") && n.ends_with(".json")))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let created = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
            Some(AutoBackup { path: e.path().display().to_string(), created_at: created, size: meta.len() })
        })
        .collect();
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

/// Writes today's automatic backup unless one was made in the last day, then
/// keeps only the newest `KEEP`. Returns the new backup, if one was written.
pub fn auto_backup(lib: &Library, settings: &Settings, data_dir: &Path) -> io::Result<Option<BackupSummary>> {
    let now = unix_now();
    if list_auto(data_dir).first().is_some_and(|b| now - b.created_at < DAY) {
        return Ok(None);
    }
    let file = build(lib, settings).map_err(io::Error::other)?;
    if file.library.books.is_empty() {
        return Ok(None); // nothing worth keeping yet
    }
    let dir = auto_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let summary = write(&file, &dir.join(format!("biblion-{}.json", date(now))))?;
    for old in list_auto(data_dir).into_iter().skip(KEEP) {
        let _ = fs::remove_file(old.path);
    }
    Ok(Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(1_794_542_000), "2026-11-13");
        assert_eq!(date(951_782_400), "2000-02-29");
    }

    #[test]
    fn files_round_trip_and_reject_others() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(tmp.path().join("data")).unwrap();
        let file = build(&lib, &Settings::default()).unwrap();
        let path = tmp.path().join("b.json");
        let summary = write(&file, &path).unwrap();
        assert!(summary.size > 0);
        assert_eq!(read(&path).unwrap().format, FORMAT);

        fs::write(tmp.path().join("other.json"), br#"{"hello": 1}"#).unwrap();
        assert!(read(&tmp.path().join("other.json")).unwrap_err().contains("not a Biblion backup"));
        let mut newer = file.clone();
        newer.library.version = 99;
        write(&newer, &path).unwrap();
        assert!(read(&path).unwrap_err().contains("newer Biblion"));
    }

    #[test]
    fn automatic_backups_are_listed_newest_first() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let lib = Library::open(&data).unwrap();
        // An empty library has nothing to back up.
        assert!(auto_backup(&lib, &Settings::default(), &data).unwrap().is_none());

        let dir = auto_dir(&data);
        fs::create_dir_all(&dir).unwrap();
        for i in 0..10 {
            let p = dir.join(format!("biblion-2020-01-{:02}.json", i + 1));
            fs::write(&p, b"{}").unwrap();
            let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800 + i * 86_400);
            fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
        }
        assert_eq!(list_auto(&data).len(), 10);
        assert!(list_auto(&data)[0].path.ends_with("biblion-2020-01-10.json"));
    }
}
