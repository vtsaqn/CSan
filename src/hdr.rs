//! Ghép HDR kiểu exposure fusion (port từ HdrFuse.java): căn chỉnh MTB -> trọng số lưới 1/8 -> trộn.
use image::RgbImage;

const G: usize = 8;

struct Lum {
    w: usize,
    h: usize,
    y: Vec<u8>,
}

struct Grid {
    w: usize,
    h: usize,
    wt: Vec<f32>,
}

fn dec(b: &[u8]) -> Result<RgbImage, String> {
    image::load_from_memory(b).map(|i| i.to_rgb8()).map_err(|e| format!("Giải mã JPEG lỗi: {}", e))
}

fn lum(img: &RgbImage, s: usize) -> Lum {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let lw = (w / s).max(1);
    let lh = (h / s).max(1);
    let raw = img.as_raw();
    let mut y = vec![0u8; lw * lh];
    for yy in 0..lh {
        for xx in 0..lw {
            let sx = (xx * s + s / 2).min(w - 1);
            let sy = (yy * s + s / 2).min(h - 1);
            let i = (sy * w + sx) * 3;
            let v = 299 * raw[i] as u32 + 587 * raw[i + 1] as u32 + 114 * raw[i + 2] as u32;
            y[yy * lw + xx] = (v / 1000) as u8;
        }
    }
    Lum { w: lw, h: lh, y }
}

struct Mtb {
    bit: Vec<u8>,
    mask: Vec<u8>,
}

fn mtb(l: &Lum) -> Mtb {
    let mut hist = [0usize; 256];
    for v in &l.y {
        hist[*v as usize] += 1;
    }
    let half = l.y.len() / 2;
    let mut acc = 0usize;
    let mut med = 0i32;
    for (i, hv) in hist.iter().enumerate() {
        acc += *hv;
        if acc >= half {
            med = i as i32;
            break;
        }
    }
    let mut bit = vec![0u8; l.y.len()];
    let mut mask = vec![0u8; l.y.len()];
    for (i, v) in l.y.iter().enumerate() {
        let v = *v as i32;
        bit[i] = if v > med { 1 } else { 0 };
        mask[i] = if (v - med).abs() > 4 { 1 } else { 0 };
    }
    Mtb { bit, mask }
}

fn search(a: &Lum, b: &Lum, cx: i32, cy: i32, rad: i32) -> (i32, i32) {
    let w = a.w.min(b.w) as i32;
    let h = a.h.min(b.h) as i32;
    let aw = a.w as i32;
    let bw = b.w as i32;
    let am = mtb(a);
    let bm = mtb(b);
    let mut best = f64::MAX;
    let (mut bx, mut by) = (cx, cy);
    for dy in (cy - rad)..=(cy + rad) {
        for dx in (cx - rad)..=(cx + rad) {
            let (mut e, mut n) = (0i64, 0i64);
            let ys = (-dy).max(0);
            let ye = h.min(h - dy);
            let xs = (-dx).max(0);
            let xe = w.min(w - dx);
            for y in ys..ye {
                let ia = y * aw;
                let ib = (y + dy) * bw + dx;
                for x in xs..xe {
                    let i1 = (ia + x) as usize;
                    let i2 = (ib + x) as usize;
                    if (am.mask[i1] & bm.mask[i2]) != 0 {
                        n += 1;
                        if am.bit[i1] != bm.bit[i2] {
                            e += 1;
                        }
                    }
                }
            }
            let mut sc = if n > (w as i64 * h as i64) / 8 { e as f64 / n as f64 } else { 1.0 };
            sc += 1e-4 * ((dx - cx).abs() + (dy - cy).abs()) as f64;
            if sc < best {
                best = sc;
                bx = dx;
                by = dy;
            }
        }
    }
    (bx, by)
}

fn align(refs: &[Lum; 3], img: &RgbImage) -> (i32, i32) {
    let s = [8usize, 4, 2];
    let (mut dx, mut dy) = (0i32, 0i32);
    for k in 0..3 {
        let b = lum(img, s[k]);
        if k > 0 {
            dx *= 2;
            dy *= 2;
        }
        let r = search(&refs[k], &b, dx, dy, if k == 0 { 8 } else { 2 });
        dx = r.0;
        dy = r.1;
    }
    (dx * 2, dy * 2)
}

fn box_h(a: &mut [f32], w: usize, h: usize, r: i32) {
    let mut row = vec![0f32; w];
    for y in 0..h {
        let o = y * w;
        for x in 0..w {
            let (mut s, mut c) = (0f32, 0f32);
            for k in -r..=r {
                let xx = x as i32 + k;
                if xx >= 0 && (xx as usize) < w {
                    s += a[o + xx as usize];
                    c += 1.0;
                }
            }
            row[x] = s / c;
        }
        a[o..o + w].copy_from_slice(&row);
    }
}

fn box_v(a: &mut [f32], w: usize, h: usize, r: i32) {
    let mut col = vec![0f32; h];
    for x in 0..w {
        for y in 0..h {
            let (mut s, mut c) = (0f32, 0f32);
            for k in -r..=r {
                let yy = y as i32 + k;
                if yy >= 0 && (yy as usize) < h {
                    s += a[yy as usize * w + x];
                    c += 1.0;
                }
            }
            col[y] = s / c;
        }
        for y in 0..h {
            a[y * w + x] = col[y];
        }
    }
}

fn grid(img: &RgbImage) -> Grid {
    let w = img.width() as usize;
    let h = img.height() as usize;
    let gw = ((w + G - 1) / G).max(1);
    let gh = ((h + G - 1) / G).max(1);
    let raw = img.as_raw();
    let n = gw * gh;
    let mut px = vec![[0f32; 3]; n];
    let mut y = vec![0f32; n];
    for gy in 0..gh {
        for gx in 0..gw {
            let sx = (gx * G + G / 2).min(w - 1);
            let sy = (gy * G + G / 2).min(h - 1);
            let i = (sy * w + sx) * 3;
            let c = [raw[i] as f32 / 255.0, raw[i + 1] as f32 / 255.0, raw[i + 2] as f32 / 255.0];
            px[gy * gw + gx] = c;
            y[gy * gw + gx] = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
        }
    }
    let mut wt = vec![0f32; n];
    for yy in 0..gh {
        for xx in 0..gw {
            let i = yy * gw + xx;
            let c0 = y[i];
            let l = y[yy * gw + xx.saturating_sub(1)];
            let r = y[yy * gw + (xx + 1).min(gw - 1)];
            let u = y[yy.saturating_sub(1) * gw + xx];
            let d = y[(yy + 1).min(gh - 1) * gw + xx];
            let contrast = (l + r + u + d - 4.0 * c0).abs();
            let [rr, gg, bb] = px[i];
            let mean = (rr + gg + bb) / 3.0;
            let sat = (((rr - mean).powi(2) + (gg - mean).powi(2) + (bb - mean).powi(2)) / 3.0).sqrt();
            let we = (-((c0 - 0.5) * (c0 - 0.5)) / (2.0 * 0.2 * 0.2)).exp();
            wt[i] = (contrast + 0.01) * (sat + 0.05) * (we + 0.001);
        }
    }
    for _ in 0..3 {
        box_h(&mut wt, gw, gh, 2);
        box_v(&mut wt, gw, gh, 2);
    }
    Grid { w: gw, h: gh, wt }
}

/// dark/mid/bright: 3 JPEG chụp -2/0/+2 EV. Kết quả là ảnh RGB theo hướng cảm biến (chưa xoay).
pub fn fuse(dark: &[u8], mid: &[u8], bright: &[u8]) -> Result<RgbImage, String> {
    let imgs = [dec(dark)?, dec(mid)?, dec(bright)?];
    let (w, h) = imgs[1].dimensions();
    if imgs[0].dimensions() != (w, h) || imgs[2].dimensions() != (w, h) {
        return Err("3 ảnh khác kích thước".to_string());
    }
    let (w, h) = (w as usize, h as usize);
    let refs = [lum(&imgs[1], 8), lum(&imgs[1], 4), lum(&imgs[1], 2)];
    let sh = [align(&refs, &imgs[0]), (0, 0), align(&refs, &imgs[2])];
    let grids = [grid(&imgs[0]), grid(&imgs[1]), grid(&imgs[2])];
    let raws = [imgs[0].as_raw(), imgs[1].as_raw(), imgs[2].as_raw()];

    // chỉ số ô lưới theo cột
    let mut gx0 = vec![vec![0usize; w]; 3];
    let mut gx1 = vec![vec![0usize; w]; 3];
    let mut fx = vec![vec![0f32; w]; 3];
    let mut sxs = vec![vec![0usize; w]; 3];
    for i in 0..3 {
        for x in 0..w {
            let sx = (x as i32 + sh[i].0).clamp(0, w as i32 - 1) as usize;
            sxs[i][x] = sx;
            let gxf = ((sx as f32 + 0.5) / G as f32 - 0.5).clamp(0.0, (grids[i].w - 1) as f32);
            let a = gxf as usize;
            gx0[i][x] = a;
            gx1[i][x] = (a + 1).min(grids[i].w - 1);
            fx[i][x] = gxf - a as f32;
        }
    }

    let mut out = vec![0u8; w * h * 3];
    for y in 0..h {
        let mut ro = [0usize; 3];
        let mut gy0 = [0usize; 3];
        let mut gy1 = [0usize; 3];
        let mut fy = [0f32; 3];
        for i in 0..3 {
            let sy = (y as i32 + sh[i].1).clamp(0, h as i32 - 1) as usize;
            ro[i] = sy * w;
            let gyf = ((sy as f32 + 0.5) / G as f32 - 0.5).clamp(0.0, (grids[i].h - 1) as f32);
            let a = gyf as usize;
            gy0[i] = a;
            gy1[i] = (a + 1).min(grids[i].h - 1);
            fy[i] = gyf - a as f32;
        }
        for x in 0..w {
            let (mut sw, mut sr, mut sg, mut sb) = (0f32, 0f32, 0f32, 0f32);
            for i in 0..3 {
                let g = &grids[i];
                let r0 = gy0[i] * g.w;
                let r1 = gy1[i] * g.w;
                let f = fx[i][x];
                let top = g.wt[r0 + gx0[i][x]] * (1.0 - f) + g.wt[r0 + gx1[i][x]] * f;
                let bot = g.wt[r1 + gx0[i][x]] * (1.0 - f) + g.wt[r1 + gx1[i][x]] * f;
                let wv = top * (1.0 - fy[i]) + bot * fy[i];
                let p = (ro[i] + sxs[i][x]) * 3;
                sw += wv;
                sr += wv * raws[i][p] as f32;
                sg += wv * raws[i][p + 1] as f32;
                sb += wv * raws[i][p + 2] as f32;
            }
            let o = (y * w + x) * 3;
            if sw < 1e-12 {
                let p = (ro[1] + sxs[1][x]) * 3;
                out[o] = raws[1][p];
                out[o + 1] = raws[1][p + 1];
                out[o + 2] = raws[1][p + 2];
            } else {
                out[o] = (sr / sw + 0.5).clamp(0.0, 255.0) as u8;
                out[o + 1] = (sg / sw + 0.5).clamp(0.0, 255.0) as u8;
                out[o + 2] = (sb / sw + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }
    RgbImage::from_raw(w as u32, h as u32, out).ok_or_else(|| "Tạo ảnh HDR lỗi".to_string())
}
