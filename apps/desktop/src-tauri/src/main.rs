// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backup;
mod commands;
mod protocols;
mod settings;
mod watcher;

use std::sync::{Arc, Mutex};

use biblion_core::Library;
use tauri::{Emitter, Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use commands::AppState;
use settings::Settings;

/// Registers an async URI scheme whose handler reads from disk on the
/// blocking pool, so serving covers never stalls the web view.
fn file_scheme<R: Runtime>(
    handler: fn(&Library, &str, Option<&str>) -> tauri::http::Response<Vec<u8>>,
) -> impl Fn(UriSchemeContext<'_, R>, tauri::http::Request<Vec<u8>>, UriSchemeResponder) + Send + Sync + 'static {
    move |ctx, request, responder| {
        let lib = ctx.app_handle().try_state::<AppState>().map(|s| s.lib.clone());
        let path = request.uri().path().to_owned();
        let range = request.headers().get(tauri::http::header::RANGE).and_then(|v| v.to_str().ok()).map(str::to_owned);
        tauri::async_runtime::spawn_blocking(move || {
            let response = match lib {
                Some(lib) => handler(&lib, &path, range.as_deref()),
                None => tauri::http::Response::builder().status(503).body(b"library not ready".to_vec()).unwrap(),
            };
            responder.respond(response);
        });
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,lopdf=error".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("thumb", file_scheme(protocols::thumbnail))
        .register_asynchronous_uri_scheme_protocol("book", file_scheme(protocols::book_file))
        .register_asynchronous_uri_scheme_protocol("comic", file_scheme(protocols::comic_page))
        .setup(|app| {
            // BIBLION_DATA_DIR lets development and testing use a separate library.
            let data_dir = match std::env::var_os("BIBLION_DATA_DIR") {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            tracing::info!("library data: {}", data_dir.display());
            let lib = Arc::new(Library::open(&data_dir)?);
            let settings_path = data_dir.join("settings.json");
            let settings = Settings::load(&settings_path);
            let handle = app.handle().clone();
            let watcher = watcher::FolderWatcher::start(lib.clone(), move |report| {
                let _ = handle.emit("library-changed", report);
            });
            watcher.set_roots(settings.watched_roots());
            if settings.auto_backup.unwrap_or(true) {
                // A daily copy of the user's data, off the startup path.
                let (lib, settings, dir) = (lib.clone(), settings.clone(), data_dir.clone());
                std::thread::spawn(move || match backup::auto_backup(&lib, &settings, &dir) {
                    Ok(Some(b)) => tracing::info!("automatic backup: {}", b.path),
                    Ok(None) => {}
                    Err(e) => tracing::warn!("automatic backup failed: {e}"),
                });
            }
            app.manage(AppState { lib, settings: Mutex::new(settings), settings_path, watcher });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_roots,
            commands::add_root,
            commands::remove_root,
            commands::get_reader_settings,
            commands::set_reader_settings,
            commands::get_ui_settings,
            commands::set_ui_settings,
            commands::get_ambience_settings,
            commands::get_watch_folders,
            commands::set_watch_folders,
            commands::set_ambience_settings,
            commands::data_dir,
            commands::analyze_book,
            commands::scan,
            commands::cancel_scan,
            commands::is_scanning,
            commands::list_books,
            commands::get_book,
            commands::stats,
            commands::list_authors,
            commands::list_series,
            commands::set_favorite,
            commands::list_shelves,
            commands::create_shelf,
            commands::rename_shelf,
            commands::delete_shelf,
            commands::list_tags,
            commands::list_languages,
            commands::set_in_shelf,
            commands::remove_from_shelf,
            commands::book_shelves,
            commands::set_progress,
            commands::comic_pages,
            commands::books_without_cover,
            commands::set_cover,
            commands::export_backup,
            commands::inspect_backup,
            commands::restore_backup,
            commands::list_auto_backups,
            commands::backups_dir,
            commands::get_auto_backup,
            commands::set_auto_backup,
            commands::save_text_file,
            commands::log_reading,
            commands::reading_stats,
            commands::duplicates,
            commands::health_counts,
            commands::problem_books,
            commands::forget_books,
            commands::trash_books,
            commands::reread_books,
            commands::forget_missing,
            commands::list_dictionaries,
            commands::define,
            commands::install_dictionary,
            commands::remove_dictionary,
            commands::add_annotation,
            commands::update_annotation,
            commands::delete_annotation,
            commands::book_annotations,
            commands::list_annotations,
            commands::annotation_counts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Biblion");
}
