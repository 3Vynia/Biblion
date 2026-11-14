//! `biblion`: command-line tool on top of biblion-core.
//! Manages a library without a UI and runs performance tests.

mod gen;

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use biblion_core::scanner::{ScanEvent, ScanOptions};
use biblion_core::{BookQuery, Format, Library, SortBy};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "biblion", about = "Local digital library manager")]
struct Cli {
    /// Data folder (DB + cover cache).
    #[arg(long, global = true, env = "BIBLION_DATA", default_value = "./biblion-data")]
    data: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Sort {
    Title,
    Author,
    Series,
    Added,
    Recent,
    Relevance,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scan one or more folders (incremental).
    Scan {
        roots: Vec<PathBuf>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        threads: Option<usize>,
    },
    /// Full-text search.
    Search {
        text: String,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// List books, paginated.
    List {
        #[arg(long, value_enum, default_value = "title")]
        sort: Sort,
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long, default_value_t = 20)]
        limit: u32,
        #[arg(long)]
        format: Option<String>,
    },
    /// Full record of a book (JSON).
    Show { id: i64 },
    /// Authors (optionally by prefix).
    Authors {
        prefix: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Library statistics.
    Stats,
    /// Save reading progress.
    Progress { id: i64, percent: f64, #[arg(long, default_value = "")] locator: String },
    /// Generate a synthetic test library (EPUB, CBZ, MOBI, AZW3, PDF).
    GenTest {
        dir: PathBuf,
        #[arg(long, default_value_t = 1000)]
        count: usize,
    },
    /// Benchmark typical UI queries.
    Bench,
    /// Sets details of many books from a JSON file: a list of objects with a
    /// "path" and the fields to change ("title", "authors", "series",
    /// "series_index", "publisher", "published", "language", "tags",
    /// "description"…). Fields left out keep their value; the books count as
    /// edited by hand, so rescans keep the changes.
    ApplyMetadata {
        file: PathBuf,
        /// Only report what would change.
        #[arg(long)]
        dry_run: bool,
    },
}

fn apply_metadata(lib: &Library, file: &std::path::Path, dry_run: bool) -> Result<()> {
    let entries: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_slice(&std::fs::read(file)?)?;
    let (mut changed, mut same, mut missing) = (0, 0, Vec::new());
    for entry in entries {
        let Some(path) = entry.get("path").and_then(|p| p.as_str()) else { anyhow::bail!("an entry has no \"path\"") };
        let Some(id) = lib.book_at(std::path::Path::new(path))? else {
            missing.push(path.to_owned());
            continue;
        };
        let current = lib.get_book(id)?.metadata;
        let mut value = serde_json::to_value(&current)?;
        for (k, v) in &entry {
            if k != "path" {
                if value.get(k).is_none() {
                    anyhow::bail!("unknown field \"{k}\" for {path}");
                }
                value[k] = v.clone();
            }
        }
        let next: biblion_core::Metadata = serde_json::from_value(value.clone())?;
        // Compared after the round trip, so 16 and 16.0 are the same number.
        if serde_json::to_value(&next)? == serde_json::to_value(&current)? {
            same += 1;
            continue;
        }
        changed += 1;
        if !dry_run {
            lib.update_metadata(id, next)?;
        }
    }
    println!("{} {changed} books, {same} already up to date, {} not in the library", if dry_run { "Would change" } else { "Changed" }, missing.len());
    for m in missing.iter().take(10) {
        println!("  not found: {m}");
    }
    Ok(())
}

fn print_page(page: &biblion_core::Page<biblion_core::BookSummary>) {
    println!("{} results (from {}):", page.total, page.offset);
    for b in &page.items {
        let series = b.series.as_ref().map(|s| format!(" [{s} #{}]", b.series_index.unwrap_or(0.0))).unwrap_or_default();
        println!(
            "  #{:<7} {:<5} {}{} — {}{}",
            b.id,
            b.format.as_str(),
            b.title,
            series,
            b.authors.join(", "),
            if b.has_cover { "" } else { "  (no cover)" }
        );
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let cli = Cli::parse();

    if let Cmd::GenTest { dir, count } = &cli.cmd {
        let t = Instant::now();
        gen::generate(dir, *count)?;
        println!("Generated {count} books in {:?}", t.elapsed());
        return Ok(());
    }

    let lib = Library::open(&cli.data)?;
    match cli.cmd {
        Cmd::Scan { roots, force, threads } => {
            let roots = roots.into_iter().map(|r| r.canonicalize()).collect::<std::io::Result<Vec<_>>>()?;
            let opts = ScanOptions { roots, threads, force, light: false };
            let report = lib.scan(&opts, &|ev| match ev {
                ScanEvent::Walking { found } => eprint!("\rWalking… {found} files   "),
                ScanEvent::Processing { done, total } => eprint!("\rProcessing {done}/{total}   "),
                ScanEvent::Finished { .. } => eprintln!(),
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.elapsed_ms > 0 {
                let processed = report.added + report.updated;
                println!("≈ {:.0} books/s processed", processed as f64 / (report.elapsed_ms as f64 / 1000.0));
            }
        }
        Cmd::Search { text, limit } => {
            let t = Instant::now();
            let page = lib.list_books(&BookQuery { text: Some(text), sort: SortBy::Relevance, limit, ..Default::default() })?;
            print_page(&page);
            println!("({:?})", t.elapsed());
        }
        Cmd::List { sort, offset, limit, format } => {
            let sort = match sort {
                Sort::Title => SortBy::Title,
                Sort::Author => SortBy::Author,
                Sort::Series => SortBy::Series,
                Sort::Added => SortBy::AddedDesc,
                Sort::Recent => SortBy::RecentlyRead,
                Sort::Relevance => SortBy::Relevance,
            };
            let format = format.as_deref().and_then(Format::parse);
            print_page(&lib.list_books(&BookQuery { sort, offset, limit, format, ..Default::default() })?);
        }
        Cmd::Show { id } => println!("{}", serde_json::to_string_pretty(&lib.get_book(id)?)?),
        Cmd::Authors { prefix, limit } => {
            let page = lib.list_authors(prefix.as_deref(), 0, limit)?;
            println!("{} authors", page.total);
            for a in page.items {
                println!("  #{:<6} {} ({} books)", a.id, a.name, a.book_count);
            }
        }
        Cmd::Stats => println!("{}", serde_json::to_string_pretty(&lib.stats()?)?),
        Cmd::Progress { id, percent, locator } => {
            lib.set_progress(id, &locator, percent, percent >= 1.0)?;
            println!("ok");
        }
        Cmd::Bench => bench(&lib)?,
        Cmd::ApplyMetadata { file, dry_run } => apply_metadata(&lib, &file, dry_run)?,
        Cmd::GenTest { .. } => unreachable!(),
    }
    Ok(())
}

fn bench(lib: &Library) -> Result<()> {
    let run = |name: &str, q: BookQuery| -> Result<()> {
        // The first run warms the cache; we report the mean of 20.
        lib.list_books(&q)?;
        let t = Instant::now();
        let mut total = 0;
        for _ in 0..20 {
            total = lib.list_books(&q)?.total;
        }
        println!("  {:<42} {:>8.2} ms   ({} results)", name, t.elapsed().as_secs_f64() * 1000.0 / 20.0, total);
        Ok(())
    };
    let stats = lib.stats()?;
    println!("Library: {} books, {} authors", stats.books, stats.authors);
    run("first page by title", BookQuery { limit: 100, ..Default::default() })?;
    run("page 1000 by title (offset 100k)", BookQuery { offset: 100_000, limit: 100, ..Default::default() })?;
    run("first page by author", BookQuery { sort: SortBy::Author, limit: 100, ..Default::default() })?;
    run("recently added", BookQuery { sort: SortBy::AddedDesc, limit: 100, ..Default::default() })?;
    run("by series", BookQuery { sort: SortBy::Series, limit: 100, ..Default::default() })?;
    run("search 'shadow' (relevance)", BookQuery { text: Some("shadow".into()), sort: SortBy::Relevance, limit: 100, ..Default::default() })?;
    run("search 'gar' prefix (relevance)", BookQuery { text: Some("gar".into()), sort: SortBy::Relevance, limit: 100, ..Default::default() })?;
    run("search 'marquez night' without accents", BookQuery { text: Some("marquez night".into()), sort: SortBy::Relevance, limit: 100, ..Default::default() })?;
    run("filter format cbz", BookQuery { format: Some(Format::Cbz), limit: 100, ..Default::default() })?;
    run("search + epub filter, sorted by title", BookQuery { text: Some("light".into()), format: Some(Format::Epub), limit: 100, ..Default::default() })?;
    Ok(())
}
