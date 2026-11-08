//! cargo run --release -p biblion-core --example thumb_bench
use std::io::Cursor;
use std::time::Instant;

fn main() {
    for (w, h) in [(600u32, 900u32), (1600, 2400), (3000, 4500)] {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x ^ y) % 256) as u8]));
        let mut jpg = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut jpg, image::ImageFormat::Jpeg).unwrap();
        let jpg = jpg.into_inner();
        let n = 30;
        let t = Instant::now();
        let mut size = 0;
        for _ in 0..n {
            size = biblion_core::covers::make_thumbnail(&jpg).unwrap().len();
        }
        println!("{w}x{h}: {:.1} ms/miniatura ({} KB)", t.elapsed().as_secs_f64() * 1000.0 / n as f64, size / 1024);
    }
}
