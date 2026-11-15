//! Lưu ảnh: JPEG / PNG / WebP / HEIC, tuỳ chọn gửi server khử nhiễu, ghép HDR, ảnh động (Live).
use crate::cfg::Cfg;
use crate::{hdr, heic, motion, server};
use image::{ImageFormat, RgbImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn stamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn rotate(img: RgbImage, rot: i32) -> RgbImage {
    match rot {
        90 => image::imageops::rotate90(&img),
        180 => image::imageops::rotate180(&img),
        270 => image::imageops::rotate270(&img),
        _ => img,
    }
}

fn encode_jpeg(img: &RgbImage, q: u8) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let mut buf: Vec<u8> = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

fn encode_other(img: &RgbImage, fmt: ImageFormat) -> Result<Vec<u8>, String> {
    let mut cur = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img.clone()).write_to(&mut cur, fmt).map_err(|e| e.to_string())?;
    Ok(cur.into_inner())
}

/// HEIC chỉ dùng khi nhỏ hơn JPEG 95 rõ rệt (như app gốc): thử tối đa 3 lần, mỗi lần giảm chất lượng.
fn heic_small(img: &RgbImage, q: i32) -> Result<Vec<u8>, String> {
    let reference = encode_jpeg(img, 95)?.len();
    let mut q = q.clamp(30, 100);
    let mut best: Option<Vec<u8>> = None;
    for _ in 0..3 {
        let h = heic::encode(img, q)?;
        if best.as_ref().map(|b| h.len() < b.len()).unwrap_or(true) {
            best = Some(h);
        }
        if best.as_ref().map(|b| (b.len() as f64) <= reference as f64 * 0.6).unwrap_or(false) {
            break;
        }
        q = (q - 20).max(30);
    }
    let b = best.ok_or_else(|| "HEIC rỗng".to_string())?;
    if b.len() as f64 >= reference as f64 * 0.95 {
        return Err(format!("HEIC {} KB không nhỏ hơn JPEG {} KB", b.len() / 1024, reference / 1024));
    }
    Ok(b)
}

/// ifmt: 0 = JPEG, 1 = PNG, 2 = WebP (không mất dữ liệu), 3 = HEIC
fn write_rgb(cfg: &Cfg, dir: &Path, img: RgbImage, rot: i32, prog: &mut dyn FnMut(&str, i32)) -> Result<PathBuf, String> {
    let img = rotate(img, rot);
    let q = cfg.int("iq", 95).clamp(50, 100) as u8;
    let mut fmt = cfg.int("ifmt", 0);
    let mut heic_bytes: Option<Vec<u8>> = None;
    if fmt == 3 {
        match heic_small(&img, cfg.int("iqh", 70) as i32) {
            Ok(b) => heic_bytes = Some(b),
            Err(e) => {
                prog(&format!("HEIC lỗi → lưu JPEG: {}", e), -1);
                fmt = 0;
            }
        }
    }
    let (bytes, ext) = match (heic_bytes, fmt) {
        (Some(b), _) => (b, "heic"),
        (None, 1) => (encode_other(&img, ImageFormat::Png)?, "png"),
        (None, 2) => (encode_other(&img, ImageFormat::WebP)?, "webp"),
        _ => (encode_jpeg(&img, q)?, "jpg"),
    };
    let _ = std::fs::create_dir_all(dir);
    let p = dir.join(format!("IMG_{}.{}", stamp(), ext));
    std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
    Ok(p)
}

fn maybe_server(cfg: &Cfg, jpeg: Vec<u8>, prog: &mut dyn FnMut(&str, i32)) -> Vec<u8> {
    if cfg.boolean("send", false) && !cfg.str("stabsrv_url", "").trim().is_empty() {
        match server::photo_process(cfg, &jpeg, prog) {
            Ok(b) => return b,
            Err(e) => prog(&format!("Server lỗi, dùng ảnh gốc: {}", e), -1),
        }
    }
    jpeg
}

/// Lưu 1 ảnh JPEG từ camera. Trả về đường dẫn tệp.
pub fn save_jpeg(cfg: &Cfg, dir: &Path, jpeg: Vec<u8>, rot: i32, prog: &mut dyn FnMut(&str, i32)) -> Result<PathBuf, String> {
    let jpeg = maybe_server(cfg, jpeg, prog);
    if cfg.int("ifmt", 0) == 0 {
        let _ = std::fs::create_dir_all(dir);
        let p = dir.join(format!("IMG_{}.jpg", stamp()));
        std::fs::write(&p, jpeg).map_err(|e| e.to_string())?;
        return Ok(p);
    }
    let img = image::load_from_memory(&jpeg).map_err(|e| e.to_string())?.to_rgb8();
    write_rgb(cfg, dir, img, rot, prog)
}

/// Ghép 3 ảnh HDR rồi lưu; lỗi thì lưu ảnh vừa sáng.
pub fn save_hdr(cfg: &Cfg, dir: &Path, jpegs: Vec<Vec<u8>>, rot: i32, prog: &mut dyn FnMut(&str, i32)) -> Result<PathBuf, String> {
    if jpegs.len() < 3 {
        return Err("Thiếu ảnh HDR".to_string());
    }
    prog("HDR: đang ghép ảnh…", -1);
    match hdr::fuse(&jpegs[0], &jpegs[1], &jpegs[2]).and_then(|img| write_rgb(cfg, dir, img, rot, &mut |_m: &str, _p: i32| {})) {
        Ok(p) => Ok(p),
        Err(e) => {
            prog(&format!("HDR lỗi ({}) → lưu ảnh vừa sáng", e), -1);
            save_jpeg(cfg, dir, jpegs[1].clone(), rot, prog)
        }
    }
}

/// Live: JPEG + clip MP4 -> Motion Photo (.jpg). Lỗi ghép thì lưu ảnh thường.
pub fn save_live(cfg: &Cfg, dir: &Path, jpeg: Vec<u8>, clip: Option<(Vec<u8>, f32)>, prog: &mut dyn FnMut(&str, i32)) -> Result<PathBuf, String> {
    let mut jpeg = maybe_server(cfg, jpeg, prog);
    let _ = std::fs::create_dir_all(dir);
    let p = dir.join(format!("LIVE_{}.jpg", stamp()));
    if let Some((mut vid, fps)) = clip {
        if cfg.boolean("stabsrv", false) && !cfg.str("stabsrv_url", "").trim().is_empty() {
            match server::stab_process(cfg, &vid, fps, prog) {
                Ok(b) => vid = b,
                Err(e) => prog(&format!("Ổn định Live lỗi: {}", e), -1),
            }
        }
        match motion::build_motion_photo(&jpeg, &vid) {
            Ok(m) => jpeg = m,
            Err(e) => prog(&format!("Ghép Live lỗi → lưu ảnh thường: {}", e), -1),
        }
    }
    std::fs::write(&p, jpeg).map_err(|e| e.to_string())?;
    Ok(p)
}
