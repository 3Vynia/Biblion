//! Biblion core: scanning, metadata extraction, covers, a database with
//! full-text search, and reading progress.
//!
//! The UI layer (Tauri) should only talk to [`Library`].

pub mod backup;
pub mod covers;
pub mod db;
pub mod dictionary;
pub mod error;
pub mod formats;
pub mod health;
pub mod library;
pub mod model;
pub mod scanner;
pub mod stats;

pub use error::{Error, Result};
pub use library::Library;
pub use model::*;
