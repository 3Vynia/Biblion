//! On-disk cover cache.
//!
//! Each book gets a JPEG thumbnail (≤ 360×540) at
//! `<cache>/thumbs/ab/abcdef….jpg`, keyed by the file fingerprint.
//! For 200k books that is a few GB at most, and the grid never touches the books.
//! The full-size cover is extracted from the file itself on demand.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use image::{DynamicImage, ImageReader, Limits};

use crate::{Error, Result};

pub const THUMB_W: u32 = 360;
pub const THUMB_H: u32 = 540;
const JPEG_QUALITY: u8 = 82;

#[derive(Debug, Clone)]
pub struct CoverCache {
    root: PathBuf,
}

impl CoverCache {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("thumbs")).map_err(|e| Error::io(&root, e))?;
        Ok(Self { root })
    }

    pub fn thumb_path(&self, key: &str) -> PathBuf {
        let shard = key.get(0..2).unwrap_or("00");
        self.root.join("thumbs").join(shard).join(format!("{key}.jpg"))
    }

    pub fn has_thumb(&self, key: &str) -> bool {
        self.thumb_path(key).is_file()
    }

    /// Generates and stores the thumbnail; atomic write (tmp + rename).
    pub fn store(&self, key: &str, image_bytes: &[u8]) -> Result<PathBuf> {
        let jpeg = make_thumbnail(image_bytes)?;
        let dest = self.thumb_path(key);
        let dir = dest.parent().expect("thumb path has parent");
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let tmp = dest.with_extension(format!("tmp{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        fs::write(&tmp, &jpeg).map_err(|e| Error::io(&tmp, e))?;
        fs::rename(&tmp, &dest).map_err(|e| Error::io(&dest, e))?;
        Ok(dest)
    }

    pub fn remove(&self, key: &str) {
        let _ = fs::remove_file(self.thumb_path(key));
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Decodes (with memory limits), resizes with SIMD and encodes as JPEG.
pub fn make_thumbnail(bytes: &[u8]) -> Result<Vec<u8>> {
    let img = match decode_jpeg_scaled(bytes) {
        Some(img) => img,
        None => decode_generic(bytes)?,
    };
    let rgb = img.into_rgb8();
    let (w, h) = rgb.dimensions();
    let (tw, th) = fit(w, h, THUMB_W, THUMB_H);
    let (pixels, w, h) = if (tw, th) != (w, h) {
        use fast_image_resize::{images::Image, FilterType as F, PixelType, ResizeAlg, ResizeOptions, Resizer};
        let src = Image::from_vec_u8(w, h, rgb.into_raw(), PixelType::U8x3).map_err(|e| Error::corrupt("cover", e))?;
        let mut dst = Image::new(tw, th, PixelType::U8x3);
        Resizer::new()
            .resize(&src, &mut dst, &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(F::Bilinear)))
            .map_err(|e| Error::corrupt("cover", e))?;
        (dst.into_vec(), tw, th)
    } else {
        (rgb.into_raw(), w, h)
    };
    let mut out = Vec::with_capacity(48 * 1024);
    jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY)
        .encode(&pixels, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| Error::corrupt("cover", e))?;
    Ok(out)
}

/// Size that fits in `max_w × max_h` keeping the aspect ratio (never upscales).
fn fit(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if w <= max_w && h <= max_h {
        return (w.max(1), h.max(1));
    }
    let scale = f64::min(max_w as f64 / w as f64, max_h as f64 / h as f64);
    (((w as f64 * scale).round() as u32).max(1), ((h as f64 * scale).round() as u32).max(1))
}

fn decode_generic(bytes: &[u8]) -> Result<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| Error::io("<cover>", e))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}

/// JPEG with DCT-domain scaling (1/2, 1/4, 1/8): a 3000×4500 cover decodes
/// straight to ~375×563 without materialising 40 MB of pixels.
fn decode_jpeg_scaled(bytes: &[u8]) -> Option<DynamicImage> {
    if !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return None;
    }
    let mut dec = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    dec.read_info().ok()?;
    let info = dec.info()?;
    if info.width as u32 > 20_000 || info.height as u32 > 20_000 {
        return None;
    }
    // Only worth it when the JPEG is at least twice the target; otherwise zune (via `image`) is faster.
    if (info.width as u32) < THUMB_W * 2 || (info.height as u32) < THUMB_H * 2 {
        return None;
    }
    let (w, h) = dec.scale(THUMB_W as u16, THUMB_H as u16).ok()?;
    let px = dec.decode().ok()?;
    let (w, h) = (w as u32, h as u32);
    use jpeg_decoder::PixelFormat::*;
    let img = match info.pixel_format {
        RGB24 => DynamicImage::ImageRgb8(image::RgbImage::from_raw(w, h, px)?),
        L8 => DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, px)?),
        L16 => return None,
        CMYK32 => {
            // Adobe CMYK JPEG (inverted).
            let rgb: Vec<u8> = px
                .chunks_exact(4)
                .flat_map(|p| {
                    let k = p[3] as u16;
                    [(p[0] as u16 * k / 255) as u8, (p[1] as u16 * k / 255) as u8, (p[2] as u16 * k / 255) as u8]
                })
                .collect();
            DynamicImage::ImageRgb8(image::RgbImage::from_raw(w, h, rgb)?)
        }
    };
    Some(img)
}
