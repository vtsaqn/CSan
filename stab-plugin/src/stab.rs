//! Bộ ổn định video thuần Rust (không cần OpenCV): ước lượng dịch chuyển toàn cục theo kim tự tháp (SAD),
//! lọc thông cao có rò (cho phép lia máy chủ ý), rồi dịch + phóng nhẹ khung (crop) bằng nội suy song tuyến.
//! Khung vào/ra là NV12 đóng gói chặt (w*h*3/2 byte).

pub struct Params {
    pub w: usize,
    pub h: usize,
    pub strength: f32, // 0..1
    pub crop: f32,     // 0.5..1  (phần khung giữ lại)
    pub smooth: f32,   // số khung của hằng số thời gian
}

pub struct Stab {
    p: Params,
    prev_y: Vec<u8>,
    prev_l2: Vec<u8>,
    prev_l4: Vec<u8>,
    ox: f32,
    oy: f32,
    has_prev: bool,
}

fn down2(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let (nw, nh) = (w / 2, h / 2);
    let mut o = vec![0u8; nw * nh];
    for y in 0..nh {
        for x in 0..nw {
            let i = (2 * y) * w + 2 * x;
            let s = src[i] as u32 + src[i + 1] as u32 + src[i + w] as u32 + src[i + w + 1] as u32;
            o[y * nw + x] = ((s + 2) / 4) as u8;
        }
    }
    o
}

/// Tìm (dx,dy) sao cho cur(x+dx,y+dy) ≈ prev(x,y) quanh tâm (cx,cy), bán kính rad.
fn search(prev: &[u8], cur: &[u8], w: usize, h: usize, cx: i32, cy: i32, rad: i32, step: usize) -> (i32, i32) {
    let margin = ((w.min(h) / 8) as i32).max(cx.abs().max(cy.abs()) + rad + 2) as usize;
    if margin * 2 + 8 >= w.min(h) {
        return (0, 0);
    }
    let (x0, x1, y0, y1) = (margin, w - margin, margin, h - margin);
    let mut best = u64::MAX;
    let mut bd = (cx, cy);
    for dy in (cy - rad)..=(cy + rad) {
        for dx in (cx - rad)..=(cx + rad) {
            let mut sad = 0u64;
            let mut n = 0u64;
            let mut y = y0;
            while y < y1 {
                let ry = (y as i32 + dy) as usize;
                let mut x = x0;
                while x < x1 {
                    let rx = (x as i32 + dx) as usize;
                    sad += (prev[y * w + x] as i32 - cur[ry * w + rx] as i32).unsigned_abs() as u64;
                    n += 1;
                    x += step;
                }
                y += step;
            }
            let score = sad * 1024 / n.max(1) + ((dx - cx).abs() + (dy - cy).abs()) as u64;
            if score < best {
                best = score;
                bd = (dx, dy);
            }
        }
    }
    bd
}

fn estimate(pp: &[u8], pl2: &[u8], pl4: &[u8], cur_y: &[u8], w: usize, h: usize) -> (i32, i32) {
    let cur_l2 = down2(cur_y, w, h);
    let cur_l4 = down2(&cur_l2, w / 2, h / 2);
    let (m4x, m4y) = search(pl4, &cur_l4, w / 4, h / 4, 0, 0, 6, 1);
    let (m2x, m2y) = search(pl2, &cur_l2, w / 2, h / 2, m4x * 2, m4y * 2, 2, 1);
    search(pp, cur_y, w, h, m2x * 2, m2y * 2, 2, 2)
}

/// Dịch + thu phóng một kênh (Y hoặc U/V xen kẽ). `pstep`: số byte mỗi điểm ảnh; `off`: vị trí kênh.
pub(crate) fn warp(src: &[u8], dst: &mut [u8], pw: usize, ph: usize, pstep: usize, off: usize, s: f32, shx: f32, shy: f32) {
    let (cx, cy) = (pw as f32 / 2.0, ph as f32 / 2.0);
    let (maxx, maxy) = ((pw - 1) as f32, (ph - 1) as f32);
    for oy in 0..ph {
        let sy = (cy + (oy as f32 + 0.5 - cy) * s - shy - 0.5).clamp(0.0, maxy);
        let y0 = sy as usize;
        let y1 = (y0 + 1).min(ph - 1);
        let fy = sy - y0 as f32;
        for ox in 0..pw {
            let sx = (cx + (ox as f32 + 0.5 - cx) * s - shx - 0.5).clamp(0.0, maxx);
            let x0 = sx as usize;
            let x1 = (x0 + 1).min(pw - 1);
            let fx = sx - x0 as f32;
            let a = src[(y0 * pw + x0) * pstep + off] as f32;
            let b = src[(y0 * pw + x1) * pstep + off] as f32;
            let c = src[(y1 * pw + x0) * pstep + off] as f32;
            let d = src[(y1 * pw + x1) * pstep + off] as f32;
            let v = (a * (1.0 - fx) + b * fx) * (1.0 - fy) + (c * (1.0 - fx) + d * fx) * fy;
            dst[(oy * pw + ox) * pstep + off] = (v + 0.5) as u8;
        }
    }
}

impl Stab {
    pub fn new(p: Params) -> Stab {
        Stab { p, prev_y: Vec::new(), prev_l2: Vec::new(), prev_l4: Vec::new(), ox: 0.0, oy: 0.0, has_prev: false }
    }

    pub fn process(&mut self, frame: &mut [u8]) {
        let (w, h) = (self.p.w, self.p.h);
        if frame.len() < w * h * 3 / 2 || w < 64 || h < 64 || w % 4 != 0 || h % 4 != 0 {
            return;
        }
        let cur_y = &frame[..w * h];
        if self.has_prev {
            let (mx, my) = estimate(&self.prev_y, &self.prev_l2, &self.prev_l4, cur_y, w, h);
            let alpha = ((self.p.smooth - 1.0).max(0.0) / self.p.smooth.max(1.0)).clamp(0.0, 0.995);
            if (mx.abs() as usize) > w / 4 || (my.abs() as usize) > h / 4 {
                self.ox = 0.0;
                self.oy = 0.0;
            } else {
                self.ox = alpha * (self.ox - mx as f32);
                self.oy = alpha * (self.oy - my as f32);
            }
        }
        let s = self.p.crop.clamp(0.5, 1.0);
        let max_x = ((1.0 - s) * w as f32 / 2.0).max(0.02 * w as f32);
        let max_y = ((1.0 - s) * h as f32 / 2.0).max(0.02 * h as f32);
        self.ox = self.ox.clamp(-max_x, max_x);
        self.oy = self.oy.clamp(-max_y, max_y);
        let shx = self.ox * self.p.strength;
        let shy = self.oy * self.p.strength;

        // lưu khung gốc làm tham chiếu cho lần sau
        self.prev_y = cur_y.to_vec();
        self.prev_l2 = down2(&self.prev_y, w, h);
        self.prev_l4 = down2(&self.prev_l2, w / 2, h / 2);
        self.has_prev = true;

        if shx.abs() < 0.05 && shy.abs() < 0.05 && s >= 0.999 {
            return;
        }
        let src = frame.to_vec();
        warp(&src[..w * h], &mut frame[..w * h], w, h, 1, 0, s, shx, shy);
        let uv = w * h;
        let (cw, ch) = (w / 2, h / 2);
        warp(&src[uv..], &mut frame[uv..], cw, ch, 2, 0, s, shx / 2.0, shy / 2.0);
        warp(&src[uv..], &mut frame[uv..], cw, ch, 2, 1, s, shx / 2.0, shy / 2.0);
    }
}
