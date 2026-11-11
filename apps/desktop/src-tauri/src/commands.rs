//! Thin `#[tauri::command]` wrappers around `biblion_core::Library`.
//!
//! Every command is `async` and runs the (blocking) SQLite work on Tauri's
//! blocking pool: synchronous commands would run on the main thread and
//! freeze the window.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use biblion_core::scanner::{ScanEvent, ScanOptions, ScanReport};
use biblion_core::{Book, BookId, BookQuery, BookSummary, Library, LibraryStats, Page};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use crate::settings::Settings;

/// Upper bound for one listing page (the grid asks for 200).
const MAX_PAGE: u32 = 1000;
/// Minimum interval between progress events sent to the UI.
const EVENT_INTERVAL: Duration = Duration::from_millis(100);

pub struct AppState {
    pub lib: Arc<Library>,
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
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
    next.save(&state.settings_path)
        .map_err(|e| CmdError::new("io", format!("cannot save settings: {e}")))?;
    *settings = next;
    Ok(settings.roots.clone())
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
        let report = lib.scan(&ScanOptions { roots, force, threads: None }, &on_event)?;
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
