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

#[test]
fn full_epub_metadata_is_stored() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("books/rich.epub");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let o = SimpleFileOptions::default();
    zip.start_file("mimetype", o).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    zip.start_file("META-INF/container.xml", o).unwrap();
    zip.write_all(br#"<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>"#).unwrap();
    zip.start_file("content.opf", o).unwrap();
    zip.write_all(br##"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
      <dc:title id="t">Rayuela</dc:title><dc:title id="s">Una novela</dc:title><meta refines="#s" property="title-type">subtitle</meta>
      <dc:creator>Julio Cortazar</dc:creator><dc:contributor opf:role="trl">Gregory Rabassa</dc:contributor>
      <dc:identifier>urn:uuid:11111111-2222-3333-4444-555555555555</dc:identifier><dc:identifier opf:scheme="ISBN">9788437604572</dc:identifier>
      <dc:rights>Public domain</dc:rights><meta property="dcterms:modified">2021-06-01T00:00:00Z</meta>
    </metadata></package>"##).unwrap();
    zip.finish().unwrap();

    let lib = Library::open(tmp.path().join("data")).unwrap();
    scan(&lib, &tmp.path().join("books"));
    let id = find(&lib, "rayuela")[0].id;
    let m = lib.get_book(id).unwrap().metadata;
    assert_eq!(m.subtitle.as_deref(), Some("Una novela"));
    assert_eq!(m.contributors, vec![biblion_core::Contributor { name: "Gregory Rabassa".into(), role: "trl".into() }]);
    assert_eq!(m.identifiers.iter().map(|i| i.scheme.as_str()).collect::<Vec<_>>(), ["uuid", "isbn"]);
    assert_eq!(m.isbn.as_deref(), Some("9788437604572"));
    assert_eq!(m.rights.as_deref(), Some("Public domain"));
    assert_eq!(m.modified.as_deref(), Some("2021-06-01T00:00:00Z"));
    assert_eq!(m.format_version.as_deref(), Some("EPUB 3.0"));
}

fn write_epub_with_text(path: &Path, title: &str, chapters: &[&str]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let o = SimpleFileOptions::default();
    zip.start_file("mimetype", o).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    zip.start_file("META-INF/container.xml", o).unwrap();
    zip.write_all(br#"<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#).unwrap();
    let items: String = (0..chapters.len())
        .map(|i| format!(r#"<item id="c{i}" href="text/c{i}.xhtml" media-type="application/xhtml+xml"/>"#))
        .collect();
    let spine: String = (0..chapters.len()).map(|i| format!(r#"<itemref idref="c{i}"/>"#)).collect();
    let nav: String = (0..chapters.len()).map(|i| format!(r#"<li><a href="text/c{i}.xhtml">Chapter {i}</a></li>"#)).collect();
    zip.start_file("OEBPS/content.opf", o).unwrap();
    write!(zip, r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{title}</dc:title></metadata>
      <manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>{items}
      <item id="i1" href="img/a.jpg" media-type="image/jpeg"/><item id="i2" href="img/b.png" media-type="image/png"/></manifest>
      <spine>{spine}</spine></package>"#).unwrap();
    zip.start_file("OEBPS/nav.xhtml", o).unwrap();
    write!(zip, r#"<html xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol>{nav}</ol></nav></body></html>"#).unwrap();
    for (i, text) in chapters.iter().enumerate() {
        zip.start_file(format!("OEBPS/text/c{i}.xhtml"), o).unwrap();
        write!(zip, "<html><head><title>Ignored title</title><style>p {{ color: red }}</style></head><body><h1>Chapter</h1><p>{text}</p><script>var notWords = 1;</script></body></html>").unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn analyze_counts_words_chapters_and_images() {
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    let path = books.join("counted.epub");
    // 1 + 9 words, 1 + 5 words (the <h1> counts, <title>/<style>/<script> don't).
    write_epub_with_text(&path, "Counted", &["One two three four five six seven eight nine", "Ten &amp; eleven, twelve"]);
    let lib = Library::open(tmp.path().join("data")).unwrap();
    scan(&lib, &books);
    let id = find(&lib, "counted")[0].id;

    let stats = lib.analyze(id).unwrap();
    assert_eq!(stats.words, 1 + 9 + 1 + 3);
    assert_eq!(stats.chapters, 2);
    assert_eq!(stats.images, 2);
    assert!(stats.characters > 50);
    assert_eq!(stats.reading_minutes, 1);
    // Cached until the file changes.
    assert_eq!(lib.book_stats(id).unwrap(), Some(stats.clone()));

    std::thread::sleep(std::time::Duration::from_millis(1100));
    write_epub_with_text(&path, "Counted", &["Only four words here"]);
    scan(&lib, &books);
    assert_eq!(lib.book_stats(id).unwrap(), None, "stats of the old file must not be reused");
    assert_eq!(lib.analyze(id).unwrap().words, 5);
}

fn write_tagged_epub(path: &Path, title: &str, language: &str, subjects: &[&str]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let o = SimpleFileOptions::default();
    zip.start_file("mimetype", o).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    zip.start_file("META-INF/container.xml", o).unwrap();
    zip.write_all(br#"<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>"#).unwrap();
    let subjects: String = subjects.iter().map(|s| format!("<dc:subject>{s}</dc:subject>")).collect();
    zip.start_file("content.opf", o).unwrap();
    write!(zip, r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{title}</dc:title><dc:language>{language}</dc:language>{subjects}</metadata></package>"#).unwrap();
    zip.finish().unwrap();
}

#[test]
fn subjects_languages_and_collection_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    write_tagged_epub(&books.join("a.epub"), "Alpha", "es", &["Science Fiction", "Space"]);
    write_tagged_epub(&books.join("b.epub"), "Beta", "es", &["Science Fiction"]);
    write_tagged_epub(&books.join("c.epub"), "Gamma", "en", &["History"]);
    let lib = Library::open(tmp.path().join("data")).unwrap();
    scan(&lib, &books);

    // Subjects with counts, alphabetical, with an optional prefix.
    let tags = lib.list_tags(None, 0, 10).unwrap();
    let names: Vec<(&str, u64)> = tags.items.iter().map(|t| (t.name.as_str(), t.book_count)).collect();
    assert_eq!(names, [("History", 1), ("Science Fiction", 2), ("Space", 1)]);
    assert_eq!(lib.list_tags(Some("sc"), 0, 10).unwrap().total, 1);
    let sf = lib.list_books(&BookQuery { tag: Some("science fiction".into()), limit: 10, ..Default::default() }).unwrap();
    assert_eq!(sf.total, 2);

    // Languages with counts, and filtering by language.
    assert_eq!(lib.list_languages().unwrap(), vec![("es".to_string(), 2), ("en".to_string(), 1)]);
    let en = lib.list_books(&BookQuery { language: Some("EN".into()), limit: 10, ..Default::default() }).unwrap();
    assert_eq!((en.total, en.items[0].title.as_str()), (1, "Gamma"));

    // Collections can be renamed and deleted; books stay in the library.
    let shelf = lib.create_shelf("Sci-fi").unwrap();
    let other = lib.create_shelf("Others").unwrap();
    lib.add_to_shelf(shelf, sf.items[0].id).unwrap();
    lib.rename_shelf(shelf, "Science fiction").unwrap();
    assert!(lib.rename_shelf(shelf, "others").is_err(), "names are unique, ignoring case");
    assert!(lib.list_shelves().unwrap().iter().any(|s| s.name == "Science fiction" && s.book_count == 1));
    lib.delete_shelf(shelf).unwrap();
    let left: Vec<i64> = lib.list_shelves().unwrap().iter().map(|s| s.id).collect();
    assert_eq!(left, vec![other]);
    assert!(lib.book_shelves(sf.items[0].id).unwrap().is_empty());
    assert_eq!(lib.stats().unwrap().books, 3);
    assert!(matches!(lib.delete_shelf(shelf), Err(biblion_core::Error::NotFound(_))));
}

#[test]
fn remove_many_books_from_a_collection() {
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    for t in ["One", "Two", "Three"] {
        write_epub(&books.join(format!("{t}.epub")), t, "Some Author", 1);
    }
    let lib = Library::open(tmp.path().join("data")).unwrap();
    scan(&lib, &books);
    let ids: Vec<i64> = ["one", "two", "three"].iter().map(|t| find(&lib, t)[0].id).collect();
    let shelf = lib.create_shelf("Mine").unwrap();
    for id in &ids {
        lib.add_to_shelf(shelf, *id).unwrap();
    }

    assert_eq!(lib.remove_many_from_shelf(shelf, &ids[..2]).unwrap(), 2);
    let left = lib.list_books(&BookQuery { shelf_id: Some(shelf), limit: 10, ..Default::default() }).unwrap();
    assert_eq!(left.items.iter().map(|b| b.title.as_str()).collect::<Vec<_>>(), ["Three"]);
    // Books not in the collection are ignored; the library keeps all books.
    assert_eq!(lib.remove_many_from_shelf(shelf, &ids[..2]).unwrap(), 0);
    assert_eq!(lib.stats().unwrap().books, 3);
}

#[test]
fn highlights_notes_and_bookmarks() {
    use biblion_core::{AnnotationKind, AnnotationQuery, NewAnnotation};
    let tmp = tempfile::tempdir().unwrap();
    let books = tmp.path().join("books");
    write_epub(&books.join("a.epub"), "Alpha", "Ann Author", 1);
    write_epub(&books.join("b.epub"), "Beta", "Ben Author", 2);
    let lib = Library::open(tmp.path().join("data")).unwrap();
    scan(&lib, &books);
    let (a, b) = (find(&lib, "alpha")[0].id, find(&lib, "beta")[0].id);

    let new = |book, kind, locator: &str, text: &str, color: Option<&str>, percent| NewAnnotation {
        book_id: book,
        kind,
        locator: locator.into(),
        text: text.into(),
        note: None,
        color: color.map(Into::into),
        chapter: Some("Chapter 1".into()),
        percent,
    };
    let h1 = lib.add_annotation(&new(a, AnnotationKind::Highlight, "epubcfi(/6/4!/4/2,/1:0,/1:10)", "It was a dark night", Some("yellow"), 0.5)).unwrap();
    let h2 = lib.add_annotation(&new(a, AnnotationKind::Highlight, "epubcfi(/6/2!/4/2,/1:0,/1:5)", "First words", Some("blue"), 0.1)).unwrap();
    let bm = lib.add_annotation(&new(a, AnnotationKind::Bookmark, "epubcfi(/6/8!/4/2/1:0)", "Page start", None, 0.9)).unwrap();
    lib.add_annotation(&new(b, AnnotationKind::Highlight, "epubcfi(/6/2!/4/2,/1:0,/1:3)", "Beta quote", Some("yellow"), 0.3)).unwrap();

    // Per book, in reading order.
    let in_a: Vec<i64> = lib.book_annotations(a).unwrap().iter().map(|x| x.id).collect();
    assert_eq!(in_a, [h2, h1, bm]);

    // Notes can be added and changed; colour too.
    lib.update_annotation(h1, Some("Great opening"), Some("pink")).unwrap();
    let h = lib.book_annotations(a).unwrap().into_iter().find(|x| x.id == h1).unwrap();
    assert_eq!((h.note.as_deref(), h.color.as_deref()), (Some("Great opening"), Some("pink")));
    lib.update_annotation(h1, Some("  "), None).unwrap();
    let h = lib.book_annotations(a).unwrap().into_iter().find(|x| x.id == h1).unwrap();
    assert_eq!((h.note, h.color.as_deref()), (None, Some("pink")), "blank notes are removed, colour kept");

    // Library-wide listing with filters, newest first, with book details.
    let q = |q: AnnotationQuery| lib.list_annotations(&q).unwrap();
    let all = q(AnnotationQuery { limit: 10, ..Default::default() });
    assert_eq!(all.total, 4);
    assert_eq!(all.items[0].book_title, "Beta");
    let yellow = q(AnnotationQuery { color: Some("yellow".into()), limit: 10, ..Default::default() });
    assert_eq!(yellow.total, 1);
    let marks = q(AnnotationQuery { kind: Some(AnnotationKind::Bookmark), limit: 10, ..Default::default() });
    assert_eq!(marks.items.iter().map(|x| x.annotation.id).collect::<Vec<_>>(), [bm]);
    let found = q(AnnotationQuery { text: Some("dark".into()), limit: 10, ..Default::default() });
    assert_eq!(found.total, 1);
    let wildcard = q(AnnotationQuery { text: Some("%".into()), limit: 10, ..Default::default() });
    assert_eq!(wildcard.total, 0, "% and _ are matched literally");
    let of_b = q(AnnotationQuery { book_id: Some(b), limit: 10, ..Default::default() });
    assert_eq!(of_b.total, 1);
    assert_eq!(lib.annotation_counts().unwrap(), (3, 1), "highlights, bookmarks");

    lib.delete_annotation(h2).unwrap();
    assert!(matches!(lib.delete_annotation(h2), Err(biblion_core::Error::NotFound(_))));
    assert_eq!(lib.book_annotations(a).unwrap().len(), 2);
}

#[test]
fn dictionaries_install_define_and_remove() {
    let tmp = tempfile::tempdir().unwrap();
    // A tiny StarDict dictionary in a download folder.
    let src = tmp.path().join("download");
    std::fs::create_dir_all(&src).unwrap();
    let entries = [("faro", "Torre alta en las costas, con luz para guiar a los navegantes."), ("niebla", "Nube en contacto con la tierra.")];
    let (mut idx, mut data) = (Vec::new(), Vec::new());
    for (w, d) in entries {
        idx.extend_from_slice(w.as_bytes());
        idx.push(0);
        idx.extend_from_slice(&(data.len() as u32).to_be_bytes());
        idx.extend_from_slice(&(d.len() as u32).to_be_bytes());
        data.extend_from_slice(d.as_bytes());
    }
    std::fs::write(src.join("es.ifo"), "StarDict's dict ifo file\nversion=2.4.2\nbookname=Diccionario de prueba\nwordcount=2\nsametypesequence=m\n").unwrap();
    std::fs::write(src.join("es.idx"), &idx).unwrap();
    std::fs::write(src.join("es.dict"), &data).unwrap();

    let lib = Library::open(tmp.path().join("data")).unwrap();
    assert!(lib.list_dictionaries().is_empty());
    assert!(lib.define("faro").is_empty());

    let installed = lib.install_dictionary(&src.join("es.ifo")).unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].name, "Diccionario de prueba");
    let defs = lib.define("Faros.");
    assert_eq!(defs.len(), 1);
    assert!(defs[0].body.starts_with("Torre alta"));
    assert!(tmp.path().join("data/dictionaries").is_dir());

    assert!(lib.remove_dictionary(&installed[0].id).unwrap().is_empty());
    assert!(lib.define("faro").is_empty());
    assert!(src.join("es.ifo").exists(), "the original files are left alone");
}
