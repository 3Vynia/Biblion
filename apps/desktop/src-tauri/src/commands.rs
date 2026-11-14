//! Thin `#[tauri::command]` wrappers around `biblion_core::Library`.
//!
//! Every command is `async` and runs the (blocking) SQLite work on Tauri's
//! blocking pool: synchronous commands would run on the main thread and
//! freeze the window.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use biblion_core::dictionary::{Definition, DictionaryInfo};
use biblion_core::backup::RestoreReport;
use biblion_core::stats::ReadingStats;
use biblion_core::health::{DuplicateGroup, DuplicateKind, HealthBook, HealthCounts, ProblemKind};
use biblion_core::scanner::{ScanEvent, ScanOptions, ScanReport};
use biblion_core::{
    Annotation, AnnotationEntry, AnnotationQuery, AuthorEntry, Book, BookId, BookQuery, BookStats, Format, BookSummary,
    Library, LibraryStats, Metadata, NewAnnotation, Page, Shelf, TagEntry,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use crate::settings::Settings;
use crate::backup::{self, AutoBackup, BackupSummary};
use crate::music::{self, MusicLibrary};
use crate::online::{Details, NetError, SourceResults, Sources};
use crate::watcher::FolderWatcher;

/// Upper bound for one listing page (the grid asks for 200).
const MAX_PAGE: u32 = 1000;
/// Minimum interval between progress events sent to the UI.
const EVENT_INTERVAL: Duration = Duration::from_millis(100);

pub struct AppState {
    pub lib: Arc<Library>,
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub watcher: FolderWatcher,
    /// Tracks of the music folder's last listing (served by `music://`).
    pub music: Mutex<Vec<PathBuf>>,
    pub online: Arc<Sources>,
}

/// Error sent to the UI: a stable `code` to branch on plus a readable message.
#[derive(Debug, Serialize)]
pub struct CmdError {
    pub code: &'static str,
    pub message: String,
}

impl CmdError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl From<biblion_core::Error> for CmdError {
    fn from(e: biblion_core::Error) -> Self {
        use biblion_core::Error as E;
        let code = match &e {
            E::Cancelled => "cancelled",
            E::NotFound(_) => "not_found",
            E::Db(_) => "database",
            E::Io { .. } => "io",
            E::Corrupt { .. } | E::Image(_) | E::UnsupportedFormat(_) => "invalid_file",
        };
        Self::new(code, e.to_string())
    }
}

type CmdResult<T> = Result<T, CmdError>;

async fn blocking<T, F>(f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> biblion_core::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CmdError::new("internal", format!("background task failed: {e}")))?
        .map_err(CmdError::from)
}

// ── Library folders ─────────────────────────────────────────────────────

#[tauri::command]
pub fn get_roots(state: State<'_, AppState>) -> Vec<PathBuf> {
    state.settings.lock().unwrap().roots.clone()
}

fn update_settings<T>(state: &AppState, f: impl FnOnce(&mut Settings) -> Result<T, String>) -> CmdResult<Vec<PathBuf>> {
    let mut settings = state.settings.lock().unwrap();
    let mut next = settings.clone();
    f(&mut next).map_err(|m| CmdError::new("invalid_folder", m))?;
    next.save(&state.settings_path).map_err(|e| CmdError::new("io", format!("cannot save settings: {e}")))?;
    *settings = next;
    state.watcher.set_roots(settings.watched_roots());
    Ok(settings.roots.clone())
}

#[tauri::command]
pub fn get_watch_folders(state: State<'_, AppState>) -> bool {
    state.settings.lock().unwrap().watching()
}

#[tauri::command]
pub fn set_watch_folders(state: State<'_, AppState>, on: bool) -> CmdResult<()> {
    update_settings(&state, |s| {
        s.watch_folders = Some(on);
        Ok(())
    })?;
    Ok(())
}

#[tauri::command]
pub fn add_root(state: State<'_, AppState>, path: PathBuf) -> CmdResult<Vec<PathBuf>> {
    update_settings(&state, |s| s.add_root(&path))
}

#[tauri::command]
pub fn remove_root(state: State<'_, AppState>, path: PathBuf) -> CmdResult<Vec<PathBuf>> {
    update_settings(&state, |s| {
        s.remove_root(&path);
        Ok(())
    })
}

#[tauri::command]
pub fn get_reader_settings(state: State<'_, AppState>) -> Option<serde_json::Value> {
    state.settings.lock().unwrap().reader.clone()
}

#[tauri::command]
pub fn set_reader_settings(state: State<'_, AppState>, value: serde_json::Value) -> CmdResult<()> {
    if !value.is_object() {
        return Err(CmdError::new("invalid_argument", "reader settings must be an object"));
    }
    update_settings(&state, |s| {
        s.reader = Some(value);
        Ok(())
    })
    .map(|_| ())
}

#[tauri::command]
pub fn get_ui_settings(state: State<'_, AppState>) -> Option<serde_json::Value> {
    state.settings.lock().unwrap().ui.clone()
}

#[tauri::command]
pub fn set_ui_settings(state: State<'_, AppState>, value: serde_json::Value) -> CmdResult<()> {
    if !value.is_object() {
        return Err(CmdError::new("invalid_argument", "interface settings must be an object"));
    }
    update_settings(&state, |s| {
        s.ui = Some(value);
        Ok(())
    })
    .map(|_| ())
}

#[tauri::command]
pub fn get_ambience_settings(state: State<'_, AppState>) -> Option<serde_json::Value> {
    state.settings.lock().unwrap().ambience.clone()
}

#[tauri::command]
pub fn set_ambience_settings(state: State<'_, AppState>, value: serde_json::Value) -> CmdResult<()> {
    if !value.is_object() {
        return Err(CmdError::new("invalid_argument", "ambience settings must be an object"));
    }
    update_settings(&state, |s| {
        s.ambience = Some(value);
        Ok(())
    })
    .map(|_| ())
}

/// Where the library lives (shown in Settings).
#[tauri::command]
pub fn data_dir(state: State<'_, AppState>) -> PathBuf {
    state.settings_path.parent().map(PathBuf::from).unwrap_or_default()
}

// ── Scanning ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn scan(state: State<'_, AppState>, force: bool, on_event: Channel<ScanEvent>) -> CmdResult<ScanReport> {
    let roots = state.settings.lock().unwrap().roots.clone();
    if roots.is_empty() {
        return Err(CmdError::new("no_folders", "Add a library folder before scanning"));
    }
    let lib = state.lib.clone();
    blocking(move || {
        let last = Mutex::new(Instant::now() - EVENT_INTERVAL);
        let on_event = move |ev: ScanEvent| {
            let send = match ev {
                ScanEvent::Finished { .. } => true,
                _ => {
                    let mut last = last.lock().unwrap();
                    let due = last.elapsed() >= EVENT_INTERVAL;
                    if due {
                        *last = Instant::now();
                    }
                    due
                }
            };
            if send {
                let _ = on_event.send(ev);
            }
        };
        let report = lib.scan(&ScanOptions { roots, force, threads: None, light: false }, &on_event)?;
        tracing::info!(
            found = report.found,
            added = report.added,
            updated = report.updated,
            unchanged = report.unchanged,
            missing = report.missing,
            errors = report.errors,
            elapsed_ms = report.elapsed_ms as u64,
            "scan finished"
        );
        Ok(report)
    })
    .await
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) {
    state.lib.cancel_scan();
}

#[tauri::command]
pub fn is_scanning(state: State<'_, AppState>) -> bool {
    state.lib.is_scanning()
}

// ── Queries ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_books(state: State<'_, AppState>, mut query: BookQuery) -> CmdResult<Page<BookSummary>> {
    query.limit = query.limit.clamp(1, MAX_PAGE);
    let lib = state.lib.clone();
    blocking(move || lib.list_books(&query)).await
}

#[tauri::command]
pub async fn get_book(state: State<'_, AppState>, id: BookId) -> CmdResult<Book> {
    let lib = state.lib.clone();
    blocking(move || lib.get_book(id)).await
}

#[tauri::command]
pub async fn stats(state: State<'_, AppState>) -> CmdResult<LibraryStats> {
    let lib = state.lib.clone();
    blocking(move || lib.stats()).await
}

#[tauri::command]
pub async fn list_authors(
    state: State<'_, AppState>,
    prefix: Option<String>,
    offset: u32,
    limit: u32,
) -> CmdResult<Page<AuthorEntry>> {
    let lib = state.lib.clone();
    let limit = limit.clamp(1, MAX_PAGE);
    blocking(move || lib.list_authors(prefix.as_deref().filter(|p| !p.trim().is_empty()), offset, limit)).await
}

#[tauri::command]
pub async fn list_series(state: State<'_, AppState>, offset: u32, limit: u32) -> CmdResult<Page<(String, u64)>> {
    let lib = state.lib.clone();
    let limit = limit.clamp(1, MAX_PAGE);
    blocking(move || lib.list_series(offset, limit)).await
}

#[tauri::command]
pub async fn analyze_book(state: State<'_, AppState>, id: BookId) -> CmdResult<BookStats> {
    let lib = state.lib.clone();
    blocking(move || lib.analyze(id)).await
}

// ── Collections ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn set_favorite(state: State<'_, AppState>, id: BookId, favorite: bool) -> CmdResult<()> {
    let lib = state.lib.clone();
    blocking(move || lib.set_favorite(id, favorite)).await
}

#[tauri::command]
pub async fn list_shelves(state: State<'_, AppState>) -> CmdResult<Vec<Shelf>> {
    let lib = state.lib.clone();
    blocking(move || lib.list_shelves()).await
}

fn collection_name(name: &str) -> CmdResult<String> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(CmdError::new("invalid_argument", "Collection names must have 1 to 80 characters"));
    }
    Ok(name)
}

fn unique_name_error(e: CmdError) -> CmdError {
    if e.message.contains("UNIQUE") {
        CmdError::new("already_exists", "A collection with that name already exists")
    } else {
        e
    }
}

#[tauri::command]
pub async fn create_shelf(state: State<'_, AppState>, name: String) -> CmdResult<i64> {
    let name = collection_name(&name)?;
    let lib = state.lib.clone();
    blocking(move || lib.create_shelf(&name)).await.map_err(unique_name_error)
}

#[tauri::command]
pub async fn rename_shelf(state: State<'_, AppState>, id: i64, name: String) -> CmdResult<()> {
    let name = collection_name(&name)?;
    let lib = state.lib.clone();
    blocking(move || lib.rename_shelf(id, &name)).await.map_err(unique_name_error)
}

#[tauri::command]
pub async fn delete_shelf(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let lib = state.lib.clone();
    blocking(move || lib.delete_shelf(id)).await
}

#[tauri::command]
pub async fn list_tags(
    state: State<'_, AppState>,
    prefix: Option<String>,
    offset: u32,
    limit: u32,
) -> CmdResult<Page<TagEntry>> {
    let lib = state.lib.clone();
    let limit = limit.clamp(1, MAX_PAGE);
    blocking(move || lib.list_tags(prefix.as_deref().filter(|p| !p.trim().is_empty()), offset, limit)).await
}

#[tauri::command]
pub async fn list_languages(state: State<'_, AppState>) -> CmdResult<Vec<(String, u64)>> {
    let lib = state.lib.clone();
    blocking(move || lib.list_languages()).await
}

#[tauri::command]
pub async fn set_in_shelf(state: State<'_, AppState>, shelf: i64, book: BookId, member: bool) -> CmdResult<()> {
    let lib = state.lib.clone();
    blocking(move || if member { lib.add_to_shelf(shelf, book) } else { lib.remove_from_shelf(shelf, book) }).await
}

/// Number of books per call; more than this is a bug in the UI.
const MAX_BATCH: usize = 100_000;

#[tauri::command]
pub async fn remove_from_shelf(state: State<'_, AppState>, shelf: i64, books: Vec<BookId>) -> CmdResult<usize> {
    if books.len() > MAX_BATCH {
        return Err(CmdError::new("invalid_argument", "too many books in one request"));
    }
    let lib = state.lib.clone();
    blocking(move || lib.remove_many_from_shelf(shelf, &books)).await
}

#[tauri::command]
pub async fn book_shelves(state: State<'_, AppState>, id: BookId) -> CmdResult<Vec<i64>> {
    let lib = state.lib.clone();
    blocking(move || lib.book_shelves(id)).await
}

// ── Highlights, notes and bookmarks ──────────────────────────────────────

#[tauri::command]
pub async fn add_annotation(state: State<'_, AppState>, annotation: NewAnnotation) -> CmdResult<i64> {
    let lib = state.lib.clone();
    blocking(move || lib.add_annotation(&annotation)).await
}

#[tauri::command]
pub async fn update_annotation(
    state: State<'_, AppState>,
    id: i64,
    note: Option<String>,
    color: Option<String>,
) -> CmdResult<()> {
    let lib = state.lib.clone();
    blocking(move || lib.update_annotation(id, note.as_deref(), color.as_deref())).await
}

#[tauri::command]
pub async fn delete_annotation(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let lib = state.lib.clone();
    blocking(move || lib.delete_annotation(id)).await
}

#[tauri::command]
pub async fn book_annotations(state: State<'_, AppState>, id: BookId) -> CmdResult<Vec<Annotation>> {
    let lib = state.lib.clone();
    blocking(move || lib.book_annotations(id)).await
}

#[tauri::command]
pub async fn list_annotations(state: State<'_, AppState>, query: AnnotationQuery) -> CmdResult<Page<AnnotationEntry>> {
    let lib = state.lib.clone();
    blocking(move || lib.list_annotations(&query)).await
}

#[tauri::command]
pub async fn annotation_counts(state: State<'_, AppState>) -> CmdResult<(u64, u64)> {
    let lib = state.lib.clone();
    blocking(move || lib.annotation_counts()).await
}

// ── Comics ───────────────────────────────────────────────────────────────

/// Page names of a CBZ/CBR in reading order (images load via `comic://`).
#[tauri::command]
pub async fn comic_pages(state: State<'_, AppState>, id: BookId) -> CmdResult<Vec<String>> {
    let lib = state.lib.clone();
    blocking(move || lib.comic_pages(id)).await
}

// ── Covers ───────────────────────────────────────────────────────────────

/// Books of a format that still have no cover (for background generation).
#[tauri::command]
pub async fn books_without_cover(state: State<'_, AppState>, format: Format, limit: u32) -> CmdResult<Vec<BookId>> {
    let lib = state.lib.clone();
    blocking(move || lib.books_without_cover(format, limit.clamp(1, 500))).await
}

/// Stores a cover rendered by the UI (e.g. page 1 of a PDF). Returns its key.
#[tauri::command]
pub async fn set_cover(state: State<'_, AppState>, id: BookId, image: Vec<u8>) -> CmdResult<String> {
    if image.is_empty() || image.len() > 16 * 1024 * 1024 {
        return Err(CmdError::new("invalid_argument", "the cover image is empty or too large"));
    }
    let lib = state.lib.clone();
    blocking(move || lib.set_cover(id, &image)).await
}

/// Saves metadata edited by the user (or picked from Open Library). The book
/// is marked as edited, so rescans keep these values.
#[tauri::command]
pub async fn update_metadata(state: State<'_, AppState>, id: BookId, metadata: Metadata) -> CmdResult<()> {
    let lib = state.lib.clone();
    blocking(move || lib.update_metadata(id, metadata)).await
}

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp"];
const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

/// Reads an image file the user picked, refusing anything that is not one.
fn read_image_file(path: &std::path::Path) -> CmdResult<Vec<u8>> {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(CmdError::new("invalid_argument", "Choose a JPEG, PNG, WebP, GIF or BMP image"));
    }
    let meta = std::fs::metadata(path).map_err(|e| CmdError::new("io", format!("Cannot open {}: {e}", path.display())))?;
    if meta.len() > MAX_IMAGE_BYTES {
        return Err(CmdError::new("invalid_argument", "This image is too large (over 32 MB)"));
    }
    std::fs::read(path).map_err(|e| CmdError::new("io", format!("Cannot read {}: {e}", path.display())))
}

/// An image file's bytes, to preview it before it becomes a cover.
#[tauri::command]
pub async fn read_image(path: PathBuf) -> CmdResult<tauri::ipc::Response> {
    let bytes = tauri::async_runtime::spawn_blocking(move || read_image_file(&path))
        .await
        .map_err(|e| CmdError::new("internal", format!("background task failed: {e}")))??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Makes an image file the book's cover (kept by rescans and backups).
#[tauri::command]
pub async fn cover_from_file(state: State<'_, AppState>, id: BookId, path: PathBuf) -> CmdResult<String> {
    let lib = state.lib.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = read_image_file(&path)?;
        lib.replace_cover(id, &bytes).map_err(CmdError::from)
    })
    .await
    .map_err(|e| CmdError::new("internal", format!("background task failed: {e}")))?
}

// ── Open Library ─────────────────────────────────────────────────────────

impl From<NetError> for CmdError {
    fn from(e: NetError) -> Self {
        let code = match &e {
            NetError::Offline(..) => "offline",
            e if e.not_found() => "not_found",
            NetError::Http(..) => "online_error",
            NetError::Invalid(..) => "invalid_argument",
        };
        Self::new(code, e.to_string())
    }
}

#[tauri::command]
pub fn get_online_metadata(state: State<'_, AppState>) -> bool {
    state.settings.lock().unwrap().online_metadata.unwrap_or(false)
}

#[tauri::command]
pub fn set_online_metadata(state: State<'_, AppState>, on: bool) -> CmdResult<()> {
    update_settings(&state, |s| {
        s.online_metadata = Some(on);
        Ok(())
    })?;
    Ok(())
}

/// Runs a request on the blocking pool, only when the user allowed it.
async fn online<T, F>(state: &AppState, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Sources) -> Result<T, CmdError> + Send + 'static,
{
    if !state.settings.lock().unwrap().online_metadata.unwrap_or(false) {
        return Err(CmdError::new("disabled", "Looking up books online is turned off in Settings"));
    }
    let sources = state.online.clone();
    tauri::async_runtime::spawn_blocking(move || f(&sources))
        .await
        .map_err(|e| CmdError::new("internal", format!("background task failed: {e}")))?
}

/// Searches every source at once (Open Library, AniList).
#[tauri::command]
pub async fn md_search(state: State<'_, AppState>, title: String, author: String, isbn: String) -> CmdResult<Vec<SourceResults>> {
    online(&state, move |s| Ok(s.search(&title, &author, &isbn))).await
}

#[tauri::command]
pub async fn md_details(state: State<'_, AppState>, source: String, key: String) -> CmdResult<Details> {
    online(&state, move |s| Ok(s.details(&source, &key)?)).await
}

/// A cover preview, sent as raw bytes.
#[tauri::command]
pub async fn md_cover(state: State<'_, AppState>, token: String) -> CmdResult<tauri::ipc::Response> {
    let bytes = online(&state, move |s| Ok(s.cover(&token, false)?)).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Downloads the large cover and makes it the book's cover.
#[tauri::command]
pub async fn md_apply_cover(state: State<'_, AppState>, id: BookId, token: String) -> CmdResult<String> {
    let lib = state.lib.clone();
    online(&state, move |s| {
        let bytes = s.cover(&token, true)?;
        Ok(lib.replace_cover(id, &bytes)?)
    })
    .await
}

// ── Music ────────────────────────────────────────────────────────────────

async fn list_music(state: &AppState, folder: Option<PathBuf>) -> CmdResult<MusicLibrary> {
    let Some(folder) = folder else {
        state.music.lock().unwrap().clear();
        return Ok(MusicLibrary::default());
    };
    let (files, lib) = tauri::async_runtime::spawn_blocking(move || {
        let files = music::list(&folder);
        let lib = music::describe(&folder, &files);
        (files, lib)
    })
    .await
    .map_err(|e| CmdError::new("internal", e.to_string()))?;
    *state.music.lock().unwrap() = files;
    Ok(lib)
}

/// Lists the music folder (again: files may have changed since).
#[tauri::command]
pub async fn get_music(state: State<'_, AppState>) -> CmdResult<MusicLibrary> {
    let folder = state.settings.lock().unwrap().music_folder.clone();
    list_music(state.inner(), folder).await
}

/// Sets (or, with no path, forgets) the music folder and lists it.
#[tauri::command]
pub async fn set_music_folder(state: State<'_, AppState>, path: Option<PathBuf>) -> CmdResult<MusicLibrary> {
    let folder = match path {
        Some(p) => {
            let p = p.canonicalize().map_err(|e| CmdError::new("invalid_folder", format!("Cannot open {}: {e}", p.display())))?;
            if !p.is_dir() {
                return Err(CmdError::new("invalid_folder", format!("{} is not a folder", p.display())));
            }
            Some(p)
        }
        None => None,
    };
    let chosen = folder.clone();
    update_settings(&state, move |s| {
        s.music_folder = chosen;
        Ok(())
    })?;
    list_music(state.inner(), folder).await
}

// ── Backups ──────────────────────────────────────────────────────────────

fn data_folder(state: &AppState) -> PathBuf {
    state.settings_path.parent().map(PathBuf::from).unwrap_or_default()
}

/// Saves a backup of progress, notes, collections, reading time and
/// preferences to `path`.
#[tauri::command]
pub async fn export_backup(state: State<'_, AppState>, path: PathBuf) -> CmdResult<BackupSummary> {
    let lib = state.lib.clone();
    let settings = state.settings.lock().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let file = backup::build(&lib, &settings)?;
        backup::write(&file, &path).map_err(|e| CmdError::new("io", format!("Cannot save the backup: {e}")))
    })
    .await
    .map_err(|e| CmdError::new("internal", e.to_string()))?
}

#[derive(Debug, Serialize)]
pub struct BackupPreview {
    pub created_at: i64,
    pub app_version: String,
    pub has_settings: bool,
    /// What restoring would change (nothing is changed yet).
    pub report: RestoreReport,
}

#[tauri::command]
pub async fn inspect_backup(state: State<'_, AppState>, path: PathBuf) -> CmdResult<BackupPreview> {
    let lib = state.lib.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let file = backup::read(&path).map_err(|m| CmdError::new("invalid_backup", m))?;
        let report = lib.restore_backup(&file.library, true)?;
        let s = &file.settings;
        Ok(BackupPreview {
            created_at: file.created_at,
            app_version: file.app_version,
            has_settings: s.reader.is_some() || s.ui.is_some() || s.ambience.is_some(),
            report,
        })
    })
    .await
    .map_err(|e| CmdError::new("internal", e.to_string()))?
}

/// Merges a backup into the library; with `settings`, its preferences
/// replace the current ones too.
#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, path: PathBuf, settings: bool) -> CmdResult<RestoreReport> {
    let lib = state.lib.clone();
    let file = tauri::async_runtime::spawn_blocking(move || backup::read(&path))
        .await
        .map_err(|e| CmdError::new("internal", e.to_string()))?
        .map_err(|m| CmdError::new("invalid_backup", m))?;
    let library = file.library.clone();
    let report = blocking(move || lib.restore_backup(&library, false)).await?;
    if settings {
        update_settings(&state, |s| {
            let b = &file.settings;
            if b.reader.is_some() {
                s.reader = b.reader.clone();
            }
            if b.ui.is_some() {
                s.ui = b.ui.clone();
            }
            if b.ambience.is_some() {
                s.ambience = b.ambience.clone();
            }
            Ok(())
        })?;
    }
    tracing::info!(annotations = report.annotations, progress = report.progress, "backup restored");
    Ok(report)
}

#[tauri::command]
pub fn list_auto_backups(state: State<'_, AppState>) -> Vec<AutoBackup> {
    backup::list_auto(&data_folder(&state))
}

#[tauri::command]
pub fn backups_dir(state: State<'_, AppState>) -> PathBuf {
    backup::auto_dir(&data_folder(&state))
}

#[tauri::command]
pub fn get_auto_backup(state: State<'_, AppState>) -> bool {
    state.settings.lock().unwrap().auto_backup.unwrap_or(true)
}

#[tauri::command]
pub fn set_auto_backup(state: State<'_, AppState>, on: bool) -> CmdResult<()> {
    update_settings(&state, |s| {
        s.auto_backup = Some(on);
        Ok(())
    })?;
    Ok(())
}

/// Saves text the UI made (notes as Markdown) to a file the user chose.
#[tauri::command]
pub async fn save_text_file(path: PathBuf, contents: String) -> CmdResult<()> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !matches!(ext.as_str(), "md" | "txt" | "csv") {
        return Err(CmdError::new("invalid_argument", "Only .md, .txt and .csv files can be written"));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let tmp = path.with_extension(format!("{ext}.tmp"));
        std::fs::write(&tmp, contents).and_then(|_| std::fs::rename(&tmp, &path))
    })
    .await
    .map_err(|e| CmdError::new("internal", e.to_string()))?
    .map_err(|e| CmdError::new("io", format!("Cannot save the file: {e}")))
}

// ── Reading statistics ───────────────────────────────────────────────────

/// Adds reading time and page turns to a book's local day (`YYYY-MM-DD`).
#[tauri::command]
pub async fn log_reading(state: State<'_, AppState>, id: BookId, day: String, seconds: u32, pages: u32) -> CmdResult<()> {
    let lib = state.lib.clone();
    blocking(move || lib.log_reading(id, &day, seconds, pages)).await
}

#[tauri::command]
pub async fn reading_stats(
    state: State<'_, AppState>,
    from: String,
    to: String,
    finished_from: i64,
    finished_to: i64,
) -> CmdResult<ReadingStats> {
    let lib = state.lib.clone();
    blocking(move || lib.reading_stats(&from, &to, finished_from, finished_to)).await
}

// ── Library health ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn duplicates(state: State<'_, AppState>, kind: DuplicateKind) -> CmdResult<Vec<DuplicateGroup>> {
    let lib = state.lib.clone();
    blocking(move || lib.duplicates(kind)).await
}

#[tauri::command]
pub async fn health_counts(state: State<'_, AppState>) -> CmdResult<HealthCounts> {
    let lib = state.lib.clone();
    blocking(move || lib.health_counts()).await
}

#[tauri::command]
pub async fn problem_books(state: State<'_, AppState>, kind: ProblemKind, offset: u32, limit: u32) -> CmdResult<Page<HealthBook>> {
    let lib = state.lib.clone();
    blocking(move || lib.problem_books(kind, offset, limit.clamp(1, MAX_PAGE))).await
}

/// Takes books out of the library; their files stay where they are.
#[tauri::command]
pub async fn forget_books(state: State<'_, AppState>, ids: Vec<BookId>) -> CmdResult<usize> {
    let lib = state.lib.clone();
    blocking(move || lib.remove_books(&ids)).await
}

#[derive(Debug, Serialize)]
pub struct TrashReport {
    pub removed: usize,
    /// (path, reason) of files that could not be moved.
    pub failed: Vec<(String, String)>,
}

/// Moves the books' files to the system trash (so they can be restored)
/// and takes the books out of the library.
#[tauri::command]
pub async fn trash_books(state: State<'_, AppState>, ids: Vec<BookId>) -> CmdResult<TrashReport> {
    let lib = state.lib.clone();
    blocking(move || {
        let mut gone = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            let book = lib.get_book(id)?;
            let path = PathBuf::from(&book.path);
            if !path.exists() {
                gone.push(id);
                continue;
            }
            match trash::delete(&path) {
                Ok(()) => gone.push(id),
                Err(e) => failed.push((book.path, e.to_string())),
            }
        }
        let removed = lib.remove_books(&gone)?;
        tracing::info!(removed, failed = failed.len(), "moved books to the trash");
        Ok(TrashReport { removed, failed })
    })
    .await
}

/// Forgets every book whose file is gone.
#[tauri::command]
pub async fn forget_missing(state: State<'_, AppState>) -> CmdResult<usize> {
    let lib = state.lib.clone();
    blocking(move || lib.remove_books(&lib.problem_ids(ProblemKind::Missing)?)).await
}

/// Reads these books' files again (after a download finished, say). With
/// no ids, every book that could not be read.
#[tauri::command]
pub async fn reread_books(state: State<'_, AppState>, ids: Option<Vec<BookId>>) -> CmdResult<ScanReport> {
    let lib = state.lib.clone();
    blocking(move || {
        let ids = match ids {
            Some(ids) => ids,
            None => lib.problem_ids(ProblemKind::Unreadable)?,
        };
        let roots = ids.iter().map(|&id| lib.get_book(id).map(|b| PathBuf::from(b.path))).collect::<biblion_core::Result<Vec<_>>>()?;
        lib.scan(&ScanOptions { roots, force: true, light: true, threads: None }, &|_| {})
    })
    .await
}

// ── Dictionaries ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_dictionaries(state: State<'_, AppState>) -> CmdResult<Vec<DictionaryInfo>> {
    let lib = state.lib.clone();
    blocking(move || Ok(lib.list_dictionaries())).await
}

#[tauri::command]
pub async fn define(state: State<'_, AppState>, word: String) -> CmdResult<Vec<Definition>> {
    let word: String = word.trim().chars().take(100).collect();
    if word.is_empty() {
        return Ok(Vec::new());
    }
    let lib = state.lib.clone();
    blocking(move || Ok(lib.define(&word))).await
}

#[tauri::command]
pub async fn install_dictionary(state: State<'_, AppState>, path: PathBuf) -> CmdResult<Vec<DictionaryInfo>> {
    let lib = state.lib.clone();
    blocking(move || lib.install_dictionary(&path)).await
}

#[tauri::command]
pub async fn remove_dictionary(state: State<'_, AppState>, id: String) -> CmdResult<Vec<DictionaryInfo>> {
    let lib = state.lib.clone();
    blocking(move || lib.remove_dictionary(&id)).await
}

// ── Reading ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn set_progress(
    state: State<'_, AppState>,
    id: BookId,
    locator: String,
    percent: f64,
    finished: bool,
) -> CmdResult<()> {
    if !percent.is_finite() {
        return Err(CmdError::new("invalid_argument", "progress must be a number"));
    }
    let lib = state.lib.clone();
    blocking(move || lib.set_progress(id, &locator, percent.clamp(0.0, 1.0), finished)).await
}
