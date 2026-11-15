//! HEIC: mã hóa 1 khung HEVC bằng MediaCodec (NDK) rồi tự đóng gói HEIF (ftyp/meta/mdat).
use crate::video::*;
use image::RgbImage;
use std::ptr::null_mut;

const MAX_SIDES: [u32; 3] = [4096, 3072, 2048];

fn rgb_to_nv12(img: &RgbImage, stride: usize, sliceh: usize) -> Vec<u8> {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let raw = img.as_raw();
    let mut out = vec![0u8; stride * sliceh * 3 / 2];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            let (r, g, b) = (raw[i] as i32, raw[i + 1] as i32, raw[i + 2] as i32);
            out[y * stride + x] = (16 + ((66 * r + 129 * g + 25 * b + 128) >> 8)).clamp(16, 235) as u8;
        }
    }
    let uv0 = stride * sliceh;
    for y in 0..h / 2 {
        for x in 0..w / 2 {
            let (mut r, mut g, mut b) = (0i32, 0i32, 0i32);
            for dy in 0..2 {
                for dx in 0..2 {
                    let i = ((y * 2 + dy) * w + x * 2 + dx) * 3;
                    r += raw[i] as i32;
                    g += raw[i + 1] as i32;
                    b += raw[i + 2] as i32;
                }
            }
            let (r, g, b) = (r / 4, g / 4, b / 4);
            let u = 128 + ((-38 * r - 74 * g + 112 * b + 128) >> 8);
            let v = 128 + ((112 * r - 94 * g - 18 * b + 128) >> 8);
            let o = uv0 + y * stride + x * 2;
            out[o] = u.clamp(16, 240) as u8;
            out[o + 1] = v.clamp(16, 240) as u8;
        }
    }
    out
}

/// Tách Annex-B thành các NAL (không gồm start code).
fn split_nals(d: &[u8]) -> Vec<Vec<u8>> {
    let mut starts: Vec<(usize, usize)> = Vec::new(); // (vị trí start code, độ dài start code)
    let mut i = 0;
    while i + 3 <= d.len() {
        if d[i] == 0 && d[i + 1] == 0 && d[i + 2] == 1 {
            starts.push((i, 3));
            i += 3;
        } else if i + 4 <= d.len() && d[i] == 0 && d[i + 1] == 0 && d[i + 2] == 0 && d[i + 3] == 1 {
            starts.push((i, 4));
            i += 4;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for k in 0..starts.len() {
        let b = starts[k].0 + starts[k].1;
        let e = if k + 1 < starts.len() { starts[k + 1].0 } else { d.len() };
        if e > b {
            out.push(d[b..e].to_vec());
        }
    }
    out
}

struct Hevc {
    vps: Vec<u8>,
    sps: Vec<u8>,
    pps: Vec<u8>,
    slices: Vec<Vec<u8>>,
}

fn encode_hevc(img: &RgbImage, quality: i32) -> Result<Hevc, String> {
    let (w, h) = (img.width() as i32, img.height() as i32);
    unsafe {
        let codec = AMediaCodec_createEncoderByType(cs("video/hevc").as_ptr());
        if codec.is_null() {
            return Err("Máy không có bộ mã hóa HEVC".to_string());
        }
        let mut configured = false;
        for cq in [true, false] {
            let f = AMediaFormat_new();
            AMediaFormat_setString(f, cs("mime").as_ptr(), cs("video/hevc").as_ptr());
            AMediaFormat_setInt32(f, cs("width").as_ptr(), w);
            AMediaFormat_setInt32(f, cs("height").as_ptr(), h);
            AMediaFormat_setInt32(f, cs("color-format").as_ptr(), 21);
            AMediaFormat_setInt32(f, cs("frame-rate").as_ptr(), 1);
            AMediaFormat_setInt32(f, cs("i-frame-interval").as_ptr(), 0);
            let br = ((w as i64 * h as i64) as f64 * (quality.clamp(10, 100) as f64 / 100.0) * 1.6) as i32;
            AMediaFormat_setInt32(f, cs("bitrate").as_ptr(), br.max(500_000));
            if cq {
                AMediaFormat_setInt32(f, cs("bitrate-mode").as_ptr(), 0);
                AMediaFormat_setInt32(f, cs("quality").as_ptr(), quality.clamp(1, 100));
            }
            let st = AMediaCodec_configure(codec, f, null_mut(), null_mut(), 1);
            AMediaFormat_delete(f);
            if st == 0 {
                configured = true;
                break;
            }
        }
        if !configured {
            AMediaCodec_delete(codec);
            return Err(format!("Cấu hình HEVC {}x{} lỗi", w, h));
        }
        if AMediaCodec_start(codec) != 0 {
            AMediaCodec_delete(codec);
            return Err("Không chạy được bộ mã hóa HEVC".to_string());
        }
        // stride / slice-height mà bộ mã hóa yêu cầu
        let (mut stride, mut sliceh) = (w, h);
        let inf = AMediaCodec_getInputFormat(codec);
        if !inf.is_null() {
            let (mut s, mut sh) = (0i32, 0i32);
            if AMediaFormat_getInt32(inf, cs("stride").as_ptr(), &mut s) && s >= w {
                stride = s;
            }
            if AMediaFormat_getInt32(inf, cs("slice-height").as_ptr(), &mut sh) && sh >= h {
                sliceh = sh;
            }
            AMediaFormat_delete(inf);
        }
        let nv = rgb_to_nv12(img, stride as usize, sliceh as usize);
        let idx = AMediaCodec_dequeueInputBuffer(codec, 2_000_000);
        if idx < 0 {
            AMediaCodec_stop(codec);
            AMediaCodec_delete(codec);
            return Err("Bộ mã hóa HEVC không nhận dữ liệu".to_string());
        }
        let mut cap = 0usize;
        let p = AMediaCodec_getInputBuffer(codec, idx as usize, &mut cap);
        if p.is_null() || cap < nv.len() {
            AMediaCodec_stop(codec);
            AMediaCodec_delete(codec);
            return Err(format!("Bộ đệm HEVC nhỏ ({} < {})", cap, nv.len()));
        }
        std::ptr::copy_nonoverlapping(nv.as_ptr(), p, nv.len());
        AMediaCodec_queueInputBuffer(codec, idx as usize, 0, nv.len(), 0, FLAG_EOS);

        let mut all: Vec<u8> = Vec::new();
        let t0 = std::time::Instant::now();
        let mut eos = false;
        while !eos && t0.elapsed() < std::time::Duration::from_secs(20) {
            let mut info = BufInfo { offset: 0, size: 0, pts: 0, flags: 0 };
            let o = AMediaCodec_dequeueOutputBuffer(codec, &mut info, 200_000);
            if o >= 0 {
                let mut sz = 0usize;
                let q = AMediaCodec_getOutputBuffer(codec, o as usize, &mut sz);
                if !q.is_null() && info.size > 0 {
                    all.extend_from_slice(std::slice::from_raw_parts(q.add(info.offset as usize), info.size as usize));
                }
                if (info.flags & FLAG_EOS) != 0 {
                    eos = true;
                }
                AMediaCodec_releaseOutputBuffer(codec, o as usize, false);
            }
        }
        AMediaCodec_stop(codec);
        AMediaCodec_delete(codec);

        let (mut vps, mut sps, mut pps) = (Vec::new(), Vec::new(), Vec::new());
        let mut slices = Vec::new();
        for n in split_nals(&all) {
            if n.len() < 3 {
                continue;
            }
            match (n[0] >> 1) & 0x3f {
                32 => vps = n,
                33 => sps = n,
                34 => pps = n,
                16..=21 => slices.push(n),
                _ => {}
            }
        }
        if vps.is_empty() || sps.is_empty() || pps.is_empty() || slices.is_empty() {
            return Err("HEVC không trả đủ VPS/SPS/PPS/khung ảnh".to_string());
        }
        Ok(Hevc { vps, sps, pps, slices })
    }
}

fn unescape(n: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(n.len());
    let mut z = 0;
    for &b in n {
        if z >= 2 && b == 3 {
            z = 0;
            continue;
        }
        if b == 0 {
            z += 1;
        } else {
            z = 0;
        }
        o.push(b);
    }
    o
}

fn bx(t: &[u8; 4], p: &[u8]) -> Vec<u8> {
    let mut o = ((p.len() + 8) as u32).to_be_bytes().to_vec();
    o.extend_from_slice(t);
    o.extend_from_slice(p);
    o
}

fn fbx(t: &[u8; 4], p: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; 4];
    v.extend_from_slice(p);
    bx(t, &v)
}

fn hvcc(h: &Hevc) -> Result<Vec<u8>, String> {
    let r = unescape(&h.sps);
    if r.len() < 15 {
        return Err("SPS quá ngắn".to_string());
    }
    let mut c = vec![1u8, r[3]];
    c.extend_from_slice(&r[4..8]); // cờ tương thích
    c.extend_from_slice(&r[8..14]); // cờ ràng buộc
    c.push(r[14]); // level
    c.extend_from_slice(&[0xF0, 0x00, 0xFC, 0xFD, 0xF8, 0xF8, 0x00, 0x00, 0x0F, 3]);
    for (t, n) in [(32u8, &h.vps), (33u8, &h.sps), (34u8, &h.pps)] {
        c.push(0x80 | t);
        c.extend_from_slice(&1u16.to_be_bytes());
        c.extend_from_slice(&(n.len() as u16).to_be_bytes());
        c.extend_from_slice(n);
    }
    Ok(bx(b"hvcC", &c))
}

fn heif(h: &Hevc, w: u32, hh: u32) -> Result<Vec<u8>, String> {
    let mut data: Vec<u8> = Vec::new();
    for s in &h.slices {
        data.extend_from_slice(&(s.len() as u32).to_be_bytes());
        data.extend_from_slice(s);
    }
    let ftyp = bx(b"ftyp", b"heic\0\0\0\0mif1heic");
    let hdlr = {
        let mut p = vec![0u8; 4];
        p.extend_from_slice(b"pict");
        p.extend_from_slice(&[0u8; 12]);
        p.push(0);
        fbx(b"hdlr", &p)
    };
    let pitm = fbx(b"pitm", &1u16.to_be_bytes());
    let infe = {
        let mut p = vec![];
        p.extend_from_slice(&1u16.to_be_bytes());
        p.extend_from_slice(&0u16.to_be_bytes());
        p.extend_from_slice(b"hvc1");
        p.push(0);
        let mut v = vec![2u8, 0, 0, 0]; // version 2
        v.extend_from_slice(&p);
        bx(b"infe", &v)
    };
    let iinf = {
        let mut p = 1u16.to_be_bytes().to_vec();
        p.extend_from_slice(&infe);
        fbx(b"iinf", &p)
    };
    let ispe = {
        let mut p = w.to_be_bytes().to_vec();
        p.extend_from_slice(&hh.to_be_bytes());
        fbx(b"ispe", &p)
    };
    let ipco = {
        let mut p = ispe.clone();
        p.extend_from_slice(&hvcc(h)?);
        bx(b"ipco", &p)
    };
    let ipma = {
        let mut p = 1u32.to_be_bytes().to_vec();
        p.extend_from_slice(&1u16.to_be_bytes());
        p.extend_from_slice(&[2, 0x01, 0x82]);
        fbx(b"ipma", &p)
    };
    let iprp = {
        let mut p = ipco.clone();
        p.extend_from_slice(&ipma);
        bx(b"iprp", &p)
    };
    let build_iloc = |off: u32| {
        let mut p = vec![0x44u8, 0x00];
        p.extend_from_slice(&1u16.to_be_bytes()); // item_count
        p.extend_from_slice(&1u16.to_be_bytes()); // item_ID
        p.extend_from_slice(&0u16.to_be_bytes()); // data_reference_index
        p.extend_from_slice(&1u16.to_be_bytes()); // extent_count
        p.extend_from_slice(&off.to_be_bytes());
        p.extend_from_slice(&(data.len() as u32).to_be_bytes());
        fbx(b"iloc", &p)
    };
    let build_meta = |off: u32| {
        let mut p = hdlr.clone();
        p.extend_from_slice(&pitm);
        p.extend_from_slice(&build_iloc(off));
        p.extend_from_slice(&iinf);
        p.extend_from_slice(&iprp);
        fbx(b"meta", &p)
    };
    let meta_len = build_meta(0).len();
    let off = (ftyp.len() + meta_len + 8) as u32;
    let meta = build_meta(off);
    let mut out = ftyp;
    out.extend_from_slice(&meta);
    out.extend_from_slice(&bx(b"mdat", &data));
    Ok(out)
}

/// quality 1..100. Ảnh đã xoay đúng; HEIC không ghi rotation.
pub fn encode(src: &RgbImage, quality: i32) -> Result<Vec<u8>, String> {
    let mut last = String::from("HEIC lỗi");
    for ms in MAX_SIDES {
        let mut img = src.clone();
        let longest = img.width().max(img.height());
        if longest > ms {
            let k = ms as f32 / longest as f32;
            let nw = ((img.width() as f32 * k) as u32).max(2);
            let nh = ((img.height() as f32 * k) as u32).max(2);
            img = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle);
        }
        let (w, h) = (img.width() & !1, img.height() & !1);
        if w != img.width() || h != img.height() {
            img = image::imageops::crop_imm(&img, 0, 0, w, h).to_image();
        }
        match encode_hevc(&img, quality).and_then(|hv| heif(&hv, w, h)) {
            Ok(b) => return Ok(b),
            Err(e) => last = e,
        }
    }
    Err(last)
}
