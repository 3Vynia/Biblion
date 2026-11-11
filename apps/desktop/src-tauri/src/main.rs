// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod protocols;
mod settings;

use std::sync::{Arc, Mutex};

use biblion_core::Library;
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use commands::AppState;
use settings::Settings;

/// Registers an async URI scheme whose handler reads from disk on the
/// blocking pool, so serving covers never stalls the web view.
fn file_scheme<R: Runtime>(
    handler: fn(&Library, &str) -> tauri::http::Response<Vec<u8>>,
) -> impl Fn(UriSchemeContext<'_, R>, tauri::http::Request<Vec<u8>>, UriSchemeResponder) + Send + Sync + 'static {
    move |ctx, request, responder| {
        let lib = ctx.app_handle().try_state::<AppState>().map(|s| s.lib.clone());
        let path = request.uri().path().to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            let response = match lib {
                Some(lib) => handler(&lib, &path),
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
        .setup(|app| {
            // BIBLION_DATA_DIR lets development and testing use a separate library.
            let data_dir = match std::env::var_os("BIBLION_DATA_DIR") {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            tracing::info!("library data: {}", data_dir.display());
            let lib = Library::open(&data_dir)?;
            let settings_path = data_dir.join("settings.json");
            let settings = Settings::load(&settings_path);
            app.manage(AppState { lib: Arc::new(lib), settings: Mutex::new(settings), settings_path });
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Biblion");
}
