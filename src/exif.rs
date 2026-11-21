//! Ghi thông số chụp (EXIF) vào ảnh: máy chụp, ống kính (mm), ISO, khẩu độ f, EV, tốc độ màn trập, flash.
use crate::cam::Shared;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Meta {
    pub make: String,
    pub model: String,
    pub software: String,
    pub iso: Option<u32>,
    pub exp_s: Option<f64>,
    pub fnum: Option<f32>,
    pub ev: Option<f32>,
    pub focal: Option<f32>,
    pub focal35: Option<u32>,
    pub flash: Option<bool>,
}

/// Nguồn lấy thông số: kết quả chụp của camera (đến sau ảnh JPEG một chút) + tên máy.
pub struct MetaSrc {
    pub sh: Arc<Shared>,
    pub ev_step: f32,
    pub phys: [f32; 2],
    pub zoom: f32,
    pub make: String,
    pub model: String,
}

impl MetaSrc {
    /// Chờ tối đa ~0,7 giây cho đủ `n` kết quả chụp rồi lấy kết quả có EV gần 0 nhất.
    pub fn gather(&self, n: usize) -> Meta {
        let t0 = Instant::now();
        loop {
            let len = self.sh.metas.lock().map(|v| v.len()).unwrap_or(0);
            if len >= n || t0.elapsed() > Duration::from_millis(700) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let list = self.sh.metas.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default();
        let mut m = Meta { make: self.make.clone(), model: self.model.clone(), software: format!("CSar {}", env!("CSAN_VERSION")), ..Default::default() };
        let torch = self.sh.torch_shot.load(Ordering::SeqCst);
        if let Some(s) = list.iter().min_by_key(|s| s.ev_steps.abs()) {
            if s.iso > 0 {
                m.iso = Some(s.iso as u32);
            }
            if s.exp_ns > 0 {
                m.exp_s = Some(s.exp_ns as f64 / 1e9);
            }
            if s.aperture > 0.0 {
                m.fnum = Some(s.aperture);
            }
            if s.focal > 0.0 {
                m.focal = Some(s.focal);
                if self.phys[0] > 0.0 && self.phys[1] > 0.0 {
                    let diag = (self.phys[0] * self.phys[0] + self.phys[1] * self.phys[1]).sqrt();
                    m.focal35 = Some((s.focal * 43.267 / diag * self.zoom.max(1.0)).round() as u32);
                }
            }
            if self.ev_step > 0.0 {
                m.ev = Some(s.ev_steps as f32 * self.ev_step);
            }
            m.flash = Some(s.flash_state == 3 || torch);
        } else if torch {
            m.flash = Some(true);
        }
        m
    }
}

pub fn orient_of(rot: i32) -> u16 {
    match rot.rem_euclid(360) {
        90 => 6,
        180 => 3,
        270 => 8,
        _ => 1,
    }
}

pub fn rot_of(o: u16) -> i32 {
    match o {
        6 => 90,
        3 => 180,
        8 => 270,
        _ => 0,
    }
}

/// Hướng (EXIF Orientation) có sẵn trong JPEG do camera tạo.
pub fn read_orientation(j: &[u8]) -> Option<u16> {
    if j.len() < 4 || j[0] != 0xff || j[1] != 0xd8 {
        return None;
    }
    let mut i = 2usize;
    while i + 3 < j.len() {
        if j[i] != 0xff {
            break;
        }
        let m = j[i + 1];
        if m == 0xd9 || m == 0xda {
            break;
        }
        let sl = ((j[i + 2] as usize) << 8) | j[i + 3] as usize;
        if m == 0xe1 && i + 10 <= j.len() && &j[i + 4..i + 10] == b"Exif\0\0" {
            let t = &j[i + 10..(i + 2 + sl).min(j.len())];
            if t.len() < 8 {
                return None;
            }
            let le = &t[0..2] == b"II";
            let rd16 = |o: usize| -> Option<u16> {
                if o + 2 > t.len() {
                    None
                } else if le {
                    Some(u16::from_le_bytes([t[o], t[o + 1]]))
                } else {
                    Some(u16::from_be_bytes([t[o], t[o + 1]]))
                }
            };
            let rd32 = |o: usize| -> Option<u32> {
                if o + 4 > t.len() {
                    None
                } else if le {
                    Some(u32::from_le_bytes([t[o], t[o + 1], t[o + 2], t[o + 3]]))
                } else {
                    Some(u32::from_be_bytes([t[o], t[o + 1], t[o + 2], t[o + 3]]))
                }
            };
            let off = rd32(4)? as usize;
            let n = rd16(off)? as usize;
            for k in 0..n {
                let en = off + 2 + 12 * k;
                if rd16(en)? == 0x0112 {
                    return rd16(en + 8);
                }
            }
            return None;
        }
        i += 2 + sl;
    }
    None
}

/// Kích thước điểm ảnh (rộng, cao) lưu trong JPEG.
pub fn jpeg_dims(j: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2usize;
    while i + 9 < j.len() {
        if j[i] != 0xff {
            i += 1;
            continue;
        }
        let m = j[i + 1];
        if m == 0xff {
            i += 1;
            continue;
        }
        if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC {
            let h = ((j[i + 5] as u32) << 8) | j[i + 6] as u32;
            let w = ((j[i + 7] as u32) << 8) | j[i + 8] as u32;
            return Some((w, h));
        }
        if m == 0xd8 || m == 0x01 || (0xD0..=0xD7).contains(&m) {
            i += 2;
            continue;
        }
        if m == 0xd9 || m == 0xda {
            break;
        }
        let sl = ((j[i + 2] as usize) << 8) | j[i + 3] as usize;
        i += 2 + sl;
    }
    None
}

/// Hướng đúng cho ảnh camera: ưu tiên EXIF có sẵn (camera biết có xoay điểm ảnh hay chưa);
/// nếu không có mà ảnh đã là khổ dọc thì coi như đã xoay; còn lại suy từ góc cảm biến.
pub fn orient_for(jpeg: &[u8], rot: i32) -> u16 {
    if let Some(o) = read_orientation(jpeg) {
        return o;
    }
    if let Some((w, h)) = jpeg_dims(jpeg) {
        let r = rot.rem_euclid(360);
        if (r == 90 || r == 270) && h > w {
            return 1;
        }
    }
    orient_of(rot)
}

struct Ent {
    tag: u16,
    typ: u16,
    cnt: u32,
    data: Vec<u8>,
}

fn ascii(tag: u16, s: &str) -> Ent {
    let mut d = s.as_bytes().to_vec();
    d.push(0);
    Ent { tag, typ: 2, cnt: d.len() as u32, data: d }
}
fn short(tag: u16, v: u16) -> Ent {
    Ent { tag, typ: 3, cnt: 1, data: v.to_le_bytes().to_vec() }
}
fn long(tag: u16, v: u32) -> Ent {
    Ent { tag, typ: 4, cnt: 1, data: v.to_le_bytes().to_vec() }
}
fn rational(tag: u16, n: u32, d: u32) -> Ent {
    let mut b = n.to_le_bytes().to_vec();
    b.extend_from_slice(&d.to_le_bytes());
    Ent { tag, typ: 5, cnt: 1, data: b }
}
fn srational(tag: u16, n: i32, d: i32) -> Ent {
    let mut b = n.to_le_bytes().to_vec();
    b.extend_from_slice(&d.to_le_bytes());
    Ent { tag, typ: 10, cnt: 1, data: b }
}
fn undef(tag: u16, b: &[u8]) -> Ent {
    Ent { tag, typ: 7, cnt: b.len() as u32, data: b.to_vec() }
}

fn ifd(ents: &[Ent], base: u32) -> Vec<u8> {
    let n = ents.len();
    let data_start = base + 2 + 12 * n as u32 + 4;
    let mut head: Vec<u8> = Vec::new();
    head.extend_from_slice(&(n as u16).to_le_bytes());
    let mut data: Vec<u8> = Vec::new();
    for e in ents {
        head.extend_from_slice(&e.tag.to_le_bytes());
        head.extend_from_slice(&e.typ.to_le_bytes());
        head.extend_from_slice(&e.cnt.to_le_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            head.extend_from_slice(&v);
        } else {
            let off = data_start + data.len() as u32;
            head.extend_from_slice(&off.to_le_bytes());
            data.extend_from_slice(&e.data);
            if data.len() % 2 == 1 {
                data.push(0);
            }
        }
    }
    head.extend_from_slice(&0u32.to_le_bytes());
    head.extend_from_slice(&data);
    head
}

fn datetime_string() -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!("{:04}:{:02}:{:02} {:02}:{:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
}

fn rat_time(s: f64) -> (u32, u32) {
    if s >= 1.0 {
        (((s * 10.0).round() as u32).max(1), 10)
    } else {
        (1, ((1.0 / s).round() as u32).max(1))
    }
}

/// Khối TIFF (little-endian) chứa các thẻ EXIF.
pub fn tiff(m: &Meta, orient: u16) -> Vec<u8> {
    let dt = datetime_string();
    let mk_ifd0 = |exif_off: u32| {
        let mut v: Vec<Ent> = Vec::new();
        if !m.make.is_empty() {
            v.push(ascii(0x010F, &m.make));
        }
        if !m.model.is_empty() {
            v.push(ascii(0x0110, &m.model));
        }
        v.push(short(0x0112, orient));
        if !m.software.is_empty() {
            v.push(ascii(0x0131, &m.software));
        }
        v.push(ascii(0x0132, &dt));
        v.push(long(0x8769, exif_off));
        v
    };
    let mut ex: Vec<Ent> = Vec::new();
    if let Some(s) = m.exp_s {
        let (n, d) = rat_time(s);
        ex.push(rational(0x829A, n, d));
    }
    if let Some(f) = m.fnum {
        ex.push(rational(0x829D, (f * 10.0).round() as u32, 10));
    }
    if let Some(i) = m.iso {
        ex.push(short(0x8827, i.min(65535) as u16));
    }
    ex.push(undef(0x9000, b"0232"));
    ex.push(ascii(0x9003, &dt));
    ex.push(ascii(0x9004, &dt));
    if let Some(e) = m.ev {
        ex.push(srational(0x9204, (e * 100.0).round() as i32, 100));
    }
    if let Some(f) = m.flash {
        ex.push(short(0x9209, if f { 1 } else { 0 }));
    }
    if let Some(f) = m.focal {
        ex.push(rational(0x920A, (f * 10.0).round() as u32, 10));
    }
    if let Some(f) = m.focal35 {
        ex.push(short(0xA405, f.min(65535) as u16));
    }
    ex.sort_by_key(|e| e.tag);
    let ifd0_size = ifd(&mk_ifd0(0), 8).len() as u32;
    let exif_off = 8 + ifd0_size;
    let mut out = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
    out.extend_from_slice(&ifd(&mk_ifd0(exif_off), 8));
    out.extend_from_slice(&ifd(&ex, exif_off));
    out
}

pub fn app1(m: &Meta, orient: u16) -> Vec<u8> {
    let t = tiff(m, orient);
    let len = (t.len() + 6 + 2).min(65535) as u16;
    let mut o = vec![0xff, 0xe1, (len >> 8) as u8, (len & 0xff) as u8];
    o.extend_from_slice(b"Exif\0\0");
    o.extend_from_slice(&t);
    o
}

/// Thay (hoặc thêm) khối EXIF trong JPEG, giữ nguyên các khối còn lại.
pub fn set_jpeg(j: &[u8], app1: &[u8]) -> Vec<u8> {
    if j.len() < 4 || j[0] != 0xff || j[1] != 0xd8 {
        return j.to_vec();
    }
    let mut out = vec![0xff, 0xd8];
    out.extend_from_slice(app1);
    let mut i = 2usize;
    while i + 3 < j.len() {
        if j[i] != 0xff {
            break;
        }
        let m = j[i + 1];
        if m == 0xd9 || m == 0xda {
            break;
        }
        let sl = ((j[i + 2] as usize) << 8) | j[i + 3] as usize;
        let end = (i + 2 + sl).min(j.len());
        let is_exif = m == 0xe1 && i + 10 <= j.len() && &j[i + 4..i + 10] == b"Exif\0\0";
        if !is_exif {
            out.extend_from_slice(&j[i..end]);
        }
        i = end;
    }
    out.extend_from_slice(&j[i.min(j.len())..]);
    out
}

fn crc32(d: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in d {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// Thêm khối eXIf vào PNG (ngay sau IHDR).
pub fn set_png(png: &[u8], tiff: &[u8]) -> Vec<u8> {
    if png.len() < 33 || &png[0..8] != b"\x89PNG\r\n\x1a\n" {
        return png.to_vec();
    }
    let ihdr_len = u32::from_be_bytes([png[8], png[9], png[10], png[11]]) as usize;
    let end = 8 + 12 + ihdr_len;
    if end > png.len() {
        return png.to_vec();
    }
    let mut chunk = (tiff.len() as u32).to_be_bytes().to_vec();
    let mut body = b"eXIf".to_vec();
    body.extend_from_slice(tiff);
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    let mut out = png[..end].to_vec();
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[end..]);
    out
}
