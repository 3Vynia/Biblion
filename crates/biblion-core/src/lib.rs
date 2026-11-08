//! Biblion core: scanning, metadata extraction, covers, a database with
//! full-text search, and reading progress.
//!
//! The UI layer (Tauri) should only talk to [`Library`].

pub mod covers;
pub mod db;
pub mod error;
pub mod formats;
pub mod library;
pub mod model;
pub mod scanner;

pub use error::{Error, Result};
pub use library::Library;
pub use model::*;
