use std::io::{Cursor, Write};
use std::path::Path;

use biblion_core::scanner::ScanOptions;
use biblion_core::{BookQuery, Library, Metadata, SortBy};
use zip::write::SimpleFileOptions;

fn cover_jpeg(seed: u8) -> Vec<u8> {
    let img = image::RgbImage::from_fn(400, 600, |x, y| image::Rgb([seed, (x % 255) as u8, (y % 255) as u8]));
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
    out.into_inner()
}

fn write_epub(path: &Path, title: &str, author: &str, seed: u8) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let o = SimpleFileOptions::default();
    zip.start_file("mimetype", o).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    zip.start_file("META-INF/container.xml", o).unwrap();
    zip.write_all(br#"<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>"#).unwrap();
    zip.start_file("content.opf", o).unwrap();
    write!(zip, r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{title}</dc:title><dc:creator>{author}</dc:creator></metadata><manifest><item id="c" href="img/c.jpg" media-type="image/jpeg" properties="cover-image"/></manifest></package>"#).unwrap();
    zip.start_file("img/c.jpg", o).unwrap();
    zip.write_all(&cover_jpeg(seed)).unwrap();
    zip.finish().unwrap();
}

fn scan(lib: &Library, root: &Path) -> biblion_core::scanner::ScanReport {
    lib.scan(&ScanOptions { roots: vec![root.to_path_buf()], ..Default::default() }, &|_| {}).unwrap()
}

fn find(lib: &Library, text: &str) -> Vec<biblion_core::BookSummary> {
    lib.list_books(&BookQuery { text: Some(text.into()), sort: SortBy::Relevance, include_missing: true, ..Default::default() })
        .unwrap()
        .items
}

#[test]
fn full_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    let lib = Library::open(tmp.path().join("data")).unwrap();

    write_epub(&books.join("a.epub"), "Cien años de soledad", "Gabriel García Márquez", 1);
    write_epub(&books.join("b.epub"), "Rayuela", "Julio Cortázar", 2);
    write_epub(&books.join("c.epub"), "Ficciones", "Jorge Luis Borges", 3);

    let r = scan(&lib, &books);
    assert_eq!((r.found, r.added, r.errors, r.covers), (3, 3, 0, 3));

    // Accent-insensitive prefix search
    let hits = find(&lib, "garcia sole");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Cien años de soledad");
    assert!(hits[0].has_cover);
    let a_id = hits[0].id;
    assert!(lib.thumbnail_path(hits[0].cover_key.as_ref().unwrap()).is_file());

    lib.set_progress(a_id, "epubcfi(/6/4!/4/2)", 0.42, false).unwrap();
    lib.set_favorite(a_id, true).unwrap();

    // Move + rename A, remove B
    std::fs::create_dir_all(books.join("moved")).unwrap();
    std::fs::rename(books.join("a.epub"), books.join("moved/García - Cien años.epub")).unwrap();
    std::fs::rename(books.join("b.epub"), tmp.path().join("b-fuera.epub")).unwrap();

    let r = scan(&lib, &books);
    assert_eq!(r.found, 2);
    assert_eq!(r.missing, 1, "{r:?}");
    let a = lib.get_book(a_id).unwrap();
    assert!(a.path.ends_with("García - Cien años.epub"), "a moved book keeps its id");
    assert_eq!(a.progress.unwrap().percent, 0.42, "and its progress");
    assert!(a.favorite);
    assert_eq!(lib.list_books(&BookQuery::default()).unwrap().total, 2, "missing books are not listed");

    // Manual edit: a forced rescan does not overwrite it
    lib.update_metadata(a_id, Metadata { title: Some("Cien años de soledad (ed. especial)".into()), authors: vec!["G. García Márquez".into()], ..Default::default() }).unwrap();
    lib.scan(&ScanOptions { roots: vec![books.clone()], force: true, ..Default::default() }, &|_| {}).unwrap();
    assert_eq!(lib.get_book(a_id).unwrap().metadata.title.as_deref(), Some("Cien años de soledad (ed. especial)"));
    assert_eq!(find(&lib, "especial").len(), 1, "FTS reflects the edit");

    // B comes back: revived with the same id
    let b_id = find(&lib, "rayuela")[0].id;
    std::fs::rename(tmp.path().join("b-fuera.epub"), books.join("b.epub")).unwrap();
    let r = scan(&lib, &books);
    assert_eq!(r.missing, 0);
    assert!(!lib.get_book(b_id).unwrap().missing);

    // Shelves
    let shelf = lib.create_shelf("Latin American Boom").unwrap();
    lib.add_to_shelf(shelf, a_id).unwrap();
    lib.add_to_shelf(shelf, b_id).unwrap();
    assert_eq!(lib.list_books(&BookQuery { shelf_id: Some(shelf), ..Default::default() }).unwrap().total, 2);
    assert_eq!(lib.list_books(&BookQuery { reading_only: true, ..Default::default() }).unwrap().total, 1);
}

#[test]
fn corrupt_and_odd_files_do_not_break_scan() {
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    std::fs::create_dir_all(&books).unwrap();
    std::fs::write(books.join("Author X - Empty.epub"), b"").unwrap();
    std::fs::write(books.join("junk.pdf"), b"%PDF-1.4 this is not a pdf").unwrap();
    std::fs::write(books.join("fake.cbr"), b"neither rar nor zip").unwrap();
    std::fs::write(books.join("broken.mobi"), vec![0u8; 100]).unwrap();
    std::fs::write(books.join("ignored.txt"), b"hello").unwrap();
    // a .cbr that is really a zip (very common)
    {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(books.join("Series 01.cbr")).unwrap());
        zip.start_file("p1.jpg", SimpleFileOptions::default()).unwrap();
        zip.write_all(&cover_jpeg(9)).unwrap();
        zip.finish().unwrap();
    }
    let lib = Library::open(tmp.path().join("data")).unwrap();
    let r = scan(&lib, &books);
    assert_eq!(r.found, 5);
    assert_eq!(r.errors, 4);
    let empty = find(&lib, "empty");
    assert_eq!(empty[0].authors, vec!["Author X"], "metadata from the file name");
    let comic = &find(&lib, "series")[0];
    assert!(comic.has_cover, "detects a zip inside a .cbr");
    assert_eq!(lib.comic_pages(comic.id).unwrap(), vec!["p1.jpg"]);
}

#[test]
fn reads_work_while_scanning() {
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    for i in 0..300 {
        write_epub(&books.join(format!("{i}.epub")), &format!("Book {i}"), "Some Author", (i % 200) as u8);
    }
    let lib = std::sync::Arc::new(Library::open(tmp.path().join("data")).unwrap());
    let reader = {
        let lib = lib.clone();
        std::thread::spawn(move || {
            let mut reads = 0;
            while lib.is_scanning() || reads == 0 {
                lib.list_books(&BookQuery::default()).unwrap();
                lib.stats().unwrap();
                reads += 1;
            }
            reads
        })
    };
    let r = scan(&lib, &books);
    assert_eq!(r.added, 300);
    assert!(reader.join().unwrap() > 0);
    assert!(lib.scan(&ScanOptions { roots: vec![books.clone()], ..Default::default() }, &|_| {}).is_ok());
}

#[test]
fn user_writes_do_not_wait_for_a_scan() {
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    write_epub(&books.join("first.epub"), "First", "Some Author", 1);
    let lib = std::sync::Arc::new(Library::open(tmp.path().join("data")).unwrap());
    scan(&lib, &books);
    let id = find(&lib, "first")[0].id;
    for i in 0..50 {
        write_epub(&books.join(format!("{i}.epub")), &format!("Book {i}"), "Some Author", i as u8);
    }

    // While the scan is running, save progress from another thread (as the
    // reader does) and require it to finish long before the scan would.
    let saved = std::sync::Mutex::new(None);
    let on_event = |ev: biblion_core::scanner::ScanEvent| {
        if !matches!(ev, biblion_core::scanner::ScanEvent::Processing { done: 0, .. }) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let lib = lib.clone();
        std::thread::spawn(move || {
            let _ = tx.send(lib.set_progress(id, "epubcfi(/6/2)", 0.25, false).is_ok()
                && lib.set_favorite(id, true).is_ok());
        });
        *saved.lock().unwrap() = Some(rx.recv_timeout(std::time::Duration::from_secs(5)));
    };
    lib.scan(&ScanOptions { roots: vec![books.clone()], ..Default::default() }, &on_event).unwrap();

    assert_eq!(*saved.lock().unwrap(), Some(Ok(true)), "user writes blocked by the scan");
    let book = lib.get_book(id).unwrap();
    assert_eq!(book.progress.unwrap().locator, "epubcfi(/6/2)");
    assert!(book.favorite);
}

#[test]
fn finished_filter_and_book_shelves() {
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    for (i, t) in ["Alpha", "Beta", "Gamma"].iter().enumerate() {
        write_epub(&books.join(format!("{t}.epub")), t, "Some Author", i as u8);
    }
    let lib = Library::open(tmp.path().join("data")).unwrap();
    scan(&lib, &books);
    let id = |t: &str| find(&lib, t)[0].id;

    lib.set_progress(id("alpha"), "a", 0.4, false).unwrap();
    lib.set_progress(id("beta"), "b", 1.0, true).unwrap();
    let list = |q: BookQuery| lib.list_books(&q).unwrap();
    let reading = list(BookQuery { reading_only: true, limit: 10, ..Default::default() });
    let finished = list(BookQuery { finished_only: true, limit: 10, ..Default::default() });
    assert_eq!((reading.total, reading.items[0].title.as_str()), (1, "Alpha"));
    assert_eq!((finished.total, finished.items[0].title.as_str()), (1, "Beta"));

    let to_read = lib.create_shelf("To read").unwrap();
    let other = lib.create_shelf("Classics").unwrap();
    lib.add_to_shelf(to_read, id("gamma")).unwrap();
    lib.add_to_shelf(other, id("gamma")).unwrap();
    let mut shelves = lib.book_shelves(id("gamma")).unwrap();
    shelves.sort();
    assert_eq!(shelves, vec![to_read, other]);
    assert!(lib.book_shelves(id("alpha")).unwrap().is_empty());
}

#[test]
fn recently_added_follows_file_dates_on_first_import() {
    use std::time::{Duration, SystemTime};
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    let day = Duration::from_secs(86_400);
    let ago = |days: u32| SystemTime::now() - day * days;
    for (name, days) in [("Old", 300), ("Middle", 30), ("Recent", 2)] {
        let path = books.join(format!("{name}.epub"));
        write_epub(&path, name, "Some Author", 1);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(ago(days)).unwrap();
    }
    let lib = Library::open(tmp.path().join("data")).unwrap();
    scan(&lib, &books);
    let recent = |lib: &Library| -> Vec<String> {
        lib.list_books(&BookQuery { sort: SortBy::AddedDesc, limit: 10, ..Default::default() })
            .unwrap()
            .items
            .into_iter()
            .map(|b| b.title)
            .collect()
    };
    assert_eq!(recent(&lib), ["Recent", "Middle", "Old"]);

    // Later additions count from when they show up, whatever their file date.
    std::thread::sleep(Duration::from_millis(1100));
    let late = books.join("Late.epub");
    write_epub(&late, "Late", "Some Author", 2);
    std::fs::File::options().write(true).open(&late).unwrap().set_modified(ago(1000)).unwrap();
    scan(&lib, &books);
    assert_eq!(recent(&lib), ["Late", "Recent", "Middle", "Old"]);
}
