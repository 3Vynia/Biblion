//! Watches the library folders and rescans only what changed.
//!
//! File events are gathered until the folders have been quiet for a few
//! seconds (or for at most a minute while something keeps writing, like a
//! download), then the folders that changed are scanned with a light,
//! incremental scan. The UI is told through the `library-changed` event.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use biblion_core::scanner::{ScanOptions, ScanReport};
use biblion_core::{Format, Library};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

/// Quiet time before changes are scanned.
const QUIET: Duration = Duration::from_secs(4);
/// Longest wait while events keep coming.
const MAX_WAIT: Duration = Duration::from_secs(60);
/// Past this many changed folders, their library folders are scanned whole.
const MAX_DIRS: usize = 64;

enum Msg {
    Roots(Vec<PathBuf>),
    Changed(Vec<PathBuf>),
}

/// Handle to the watcher thread. Dropping it stops the thread.
pub struct FolderWatcher {
    tx: Sender<Msg>,
}

impl FolderWatcher {
    /// Starts the thread; `notify` is called after each scan that changed
    /// something.
    pub fn start(lib: Arc<Library>, notify: impl Fn(ScanReport) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<Msg>();
        let events = tx.clone();
        std::thread::Builder::new()
            .name("biblion-watch".into())
            .spawn(move || run(lib, rx, events, notify))
            .expect("cannot start the folder watcher");
        Self { tx }
    }

    /// Watches these folders instead (empty to stop watching).
    pub fn set_roots(&self, roots: Vec<PathBuf>) {
        let _ = self.tx.send(Msg::Roots(roots));
    }
}

fn run(lib: Arc<Library>, rx: mpsc::Receiver<Msg>, events: Sender<Msg>, notify: impl Fn(ScanReport)) {
    // Holds the watches; replacing it drops the old ones.
    let mut _watcher: Option<RecommendedWatcher> = None;
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut dirty: BTreeSet<PathBuf> = BTreeSet::new();
    let mut first: Option<Instant> = None;
    let mut last = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Msg::Roots(next)) => {
                _watcher = None; // drop the old watches first
                dirty.clear();
                first = None;
                roots = next;
                if !roots.is_empty() {
                    _watcher = watch(&roots, events.clone());
                }
            }
            Ok(Msg::Changed(paths)) => {
                let paths: Vec<PathBuf> = paths.into_iter().filter(|p| relevant(p)).collect();
                if !paths.is_empty() {
                    dirty.extend(paths);
                    first.get_or_insert_with(Instant::now);
                    last = Instant::now();
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let Some(since) = first else { continue };
        if (last.elapsed() < QUIET && since.elapsed() < MAX_WAIT) || lib.is_scanning() {
            continue;
        }
        let dirs = folders_to_scan(&dirty, &roots);
        if dirs.is_empty() {
            dirty.clear();
            first = None;
            continue;
        }
        tracing::info!("watched folders changed, scanning {} folder(s)", dirs.len());
        let opts = ScanOptions { roots: dirs, light: true, ..Default::default() };
        match lib.scan(&opts, &|_| {}) {
            Ok(report) => {
                dirty.clear();
                first = None;
                tracing::info!(added = report.added, updated = report.updated, "watch scan finished");
                notify(report);
            }
            // Another scan started in between: try again on the next tick.
            Err(e) => tracing::debug!("watch scan postponed: {e}"),
        }
    }
}

fn watch(roots: &[PathBuf], events: Sender<Msg>) -> Option<RecommendedWatcher> {
    let handler = move |res: notify::Result<notify::Event>| match res {
        Ok(ev) if !matches!(ev.kind, EventKind::Access(_)) => {
            let _ = events.send(Msg::Changed(ev.paths));
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("folder watch error: {e}"),
    };
    let mut watcher = match notify::recommended_watcher(handler) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("cannot watch the library folders: {e}");
            return None;
        }
    };
    let started = Instant::now();
    for root in roots {
        if let Err(e) = watcher.watch(root, RecursiveMode::Recursive) {
            tracing::warn!("cannot watch {}: {e}", root.display());
        }
    }
    tracing::info!("watching {} library folder(s) ({} ms)", roots.len(), started.elapsed().as_millis());
    Some(watcher)
}

/// Books, and folders (which may hold books). Partial downloads (`.part`,
/// `.!qB`…) are not books, so they are skipped.
fn relevant(path: &Path) -> bool {
    Format::from_path(path).is_some() || path.extension().is_none() || path.is_dir()
}

/// Whether the path is inside a hidden file or folder below `root` (the
/// scanner skips those too).
fn hidden_below(path: &Path, root: &Path) -> bool {
    path.strip_prefix(root)
        .is_ok_and(|rel| rel.components().any(|c| c.as_os_str().to_str().is_some_and(|n| n.starts_with('.'))))
}

/// The smallest set of existing folders covering every changed path, inside
/// the library folders. Too many folders fall back to their library folders.
fn folders_to_scan(changed: &BTreeSet<PathBuf>, roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for path in changed {
        let Some(root) = roots.iter().find(|r| path.starts_with(r)) else { continue };
        if hidden_below(path, root) {
            continue;
        }
        // A file's folder, or the nearest folder that still exists (it may
        // have been deleted or renamed).
        let mut dir = if path.is_dir() { path.clone() } else { path.parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone()) };
        while !dir.is_dir() && dir != *root {
            if !dir.pop() {
                break;
            }
        }
        if dir.starts_with(root) {
            dirs.insert(dir);
        }
    }
    // Drop folders inside another changed folder (sorted order puts parents first).
    let mut out: Vec<PathBuf> = Vec::new();
    for d in dirs {
        if !out.iter().any(|p| d.starts_with(p)) {
            out.push(d);
        }
    }
    if out.len() > MAX_DIRS {
        out = roots.iter().filter(|r| out.iter().any(|d| d.starts_with(r))).cloned().collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_paths_become_few_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("lib");
        std::fs::create_dir_all(root.join("A/B")).unwrap();
        std::fs::create_dir_all(root.join("C")).unwrap();
        let changed: BTreeSet<PathBuf> = [
            root.join("A/x.epub"),
            root.join("A/B/y.epub"),          // inside A, already covered
            root.join("C/z.pdf"),
            root.join("Gone/Deeper/w.epub"),  // deleted folder: its nearest existing parent
            tmp.path().join("elsewhere/v.epub"), // outside the library
        ]
        .into();
        assert_eq!(folders_to_scan(&changed, &[root.clone()]), [root.clone()]);

        let changed: BTreeSet<PathBuf> = [root.join("A/x.epub"), root.join("A/B/y.epub"), root.join("C/z.pdf")].into();
        assert_eq!(folders_to_scan(&changed, &[root.clone()]), [root.join("A"), root.join("C")]);
    }

    #[test]
    fn only_books_and_folders_matter() {
        assert!(relevant(Path::new("/l/a/book.epub")));
        assert!(relevant(Path::new("/l/a/Some Folder")));
        assert!(!relevant(Path::new("/l/a/book.epub.part")));
        assert!(!relevant(Path::new("/l/a/book.epub.!qB")));
        assert!(hidden_below(Path::new("/l/.cache/book.epub"), Path::new("/l")));
        assert!(!hidden_below(Path::new("/.tmp/l/a/book.epub"), Path::new("/.tmp/l")));
        assert!(!relevant(Path::new("/l/a/notes.txt")));
    }

    #[test]
    fn new_books_are_picked_up() {
        let tmp = tempfile::tempdir().unwrap();
        let books = tmp.path().join("books");
        std::fs::create_dir_all(&books).unwrap();
        let lib = Arc::new(Library::open(&tmp.path().join("data")).unwrap());
        let (tx, rx) = mpsc::channel();
        let w = FolderWatcher::start(lib.clone(), move |r| {
            let _ = tx.send(r);
        });
        w.set_roots(vec![books.canonicalize().unwrap()]);
        std::thread::sleep(Duration::from_millis(500));
        std::fs::create_dir_all(books.join("Frank_Caudett")).unwrap();
        std::fs::write(books.join("Frank_Caudett/Contra_Andromeda.epub"), vec![0u8; 1024]).unwrap();
        let report = rx.recv_timeout(Duration::from_secs(20)).expect("a scan after the change");
        assert_eq!(report.added, 1);
        assert_eq!(lib.stats().unwrap().books, 1);
    }
}
