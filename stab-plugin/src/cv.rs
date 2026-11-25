//! Bộ ổn định dùng OpenCV (bật bằng feature `opencv`): đặc trưng Shi-Tomasi + LK optical flow +
//! estimateAffinePartial2D (RANSAC), lọc thông cao có rò, rồi dịch/xoay/phóng khung.
//! Khung vào/ra là NV12 đóng gói chặt.
use opencv::{calib3d, core, imgproc, prelude::*, video};

pub struct CvStab {
    w: i32,
    h: i32,
    strength: f64,
    crop: f64,
    alpha: f64,
    prev: Option<Vec<u8>>, // luma 1/2 độ phân giải
    ox: f64,
    oy: f64,
    oa: f64,
}

impl CvStab {
    pub fn new(w: usize, h: usize, strength: f32, crop: f32, smooth: f32) -> CvStab {
        let n = smooth.max(1.0) as f64;
        CvStab { w: w as i32, h: h as i32, strength: strength as f64, crop: (crop as f64).clamp(0.5, 1.0), alpha: ((n - 1.0) / n).clamp(0.0, 0.995), prev: None, ox: 0.0, oy: 0.0, oa: 0.0 }
    }

    fn small(&self, frame: &[u8]) -> Vec<u8> {
        let (w, h) = (self.w as usize, self.h as usize);
        let (nw, nh) = (w / 2, h / 2);
        let mut o = vec![0u8; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                o[y * nw + x] = frame[(2 * y) * w + 2 * x];
            }
        }
        o
    }

    fn motion(&self, prev: &[u8], cur: &[u8]) -> opencv::Result<Option<(f64, f64, f64)>> {
        let (sw, sh) = (self.w / 2, self.h / 2);
        let p = core::Mat::new_rows_cols_with_data(sh, sw, prev)?;
        let c = core::Mat::new_rows_cols_with_data(sh, sw, cur)?;
        let mut pts0 = core::Vector::<core::Point2f>::new();
        imgproc::good_features_to_track(&p, &mut pts0, 200, 0.01, 12.0, &core::no_array(), 3, false, 0.04)?;
        if pts0.len() < 12 {
            return Ok(None);
        }
        let mut pts1 = core::Vector::<core::Point2f>::new();
        let mut status = core::Vector::<u8>::new();
        let mut err = core::Vector::<f32>::new();
        let crit = core::TermCriteria::new(1 | 2, 20, 0.03)?;
        video::calc_optical_flow_pyr_lk(&p, &c, &pts0, &mut pts1, &mut status, &mut err, core::Size::new(21, 21), 3, crit, 0, 1e-4)?;
        let mut g0 = core::Vector::<core::Point2f>::new();
        let mut g1 = core::Vector::<core::Point2f>::new();
        for i in 0..pts0.len() {
            if status.get(i)? == 1 {
                g0.push(pts0.get(i)?);
                g1.push(pts1.get(i)?);
            }
        }
        if g0.len() < 8 {
            return Ok(None);
        }
        let mut inliers = core::Mat::default();
        let m = calib3d::estimate_affine_partial_2d(&g0, &g1, &mut inliers, calib3d::RANSAC, 3.0, 2000, 0.99, 10)?;
        if m.empty() {
            return Ok(None);
        }
        let dx = *m.at_2d::<f64>(0, 2)? * 2.0;
        let dy = *m.at_2d::<f64>(1, 2)? * 2.0;
        let da = (*m.at_2d::<f64>(1, 0)?).atan2(*m.at_2d::<f64>(0, 0)?);
        Ok(Some((dx, dy, da)))
    }

    pub fn process(&mut self, frame: &mut [u8]) {
        let (w, h) = (self.w as usize, self.h as usize);
        if frame.len() < w * h * 3 / 2 {
            return;
        }
        let cur = self.small(frame);
        if let Some(prev) = self.prev.take() {
            if let Ok(Some((mx, my, ma))) = self.motion(&prev, &cur) {
                self.ox = self.alpha * (self.ox - mx);
                self.oy = self.alpha * (self.oy - my);
                self.oa = self.alpha * (self.oa - ma);
            }
        }
        self.prev = Some(cur);
        let s = self.crop;
        let mx = ((1.0 - s) * w as f64 / 2.0).max(0.02 * w as f64);
        let my = ((1.0 - s) * h as f64 / 2.0).max(0.02 * h as f64);
        let (tx, ty) = ((self.ox * self.strength).clamp(-mx, mx), (self.oy * self.strength).clamp(-my, my));
        let ta = -(self.oa * self.strength).clamp(-0.1, 0.1);
        let _ = self.warp(frame, tx, ty, ta, s);
    }

    /// Y: warpAffine của OpenCV (có xoay); UV: dịch + thu phóng như bản thuần Rust.
    fn warp(&self, frame: &mut [u8], tx: f64, ty: f64, ta: f64, s: f64) -> opencv::Result<()> {
        let (w, h) = (self.w, self.h);
        let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
        let (a, b) = (ta.cos() * s, ta.sin() * s);
        let m = core::Mat::from_slice_2d(&[
            [a as f32, (-b) as f32, (cx - a * cx + b * cy - tx) as f32],
            [b as f32, a as f32, (cy - b * cx - a * cy - ty) as f32],
        ])?;
        let ylen = (w * h) as usize;
        let src = core::Mat::new_rows_cols_with_data(h, w, &frame[..ylen])?.try_clone()?;
        let mut dst = core::Mat::default();
        imgproc::warp_affine(&src, &mut dst, &m, core::Size::new(w, h), imgproc::INTER_LINEAR | imgproc::WARP_INVERSE_MAP, core::BORDER_REPLICATE, core::Scalar::default())?;
        frame[..ylen].copy_from_slice(dst.data_bytes()?);
        let uv_src = frame[ylen..].to_vec();
        let (cw, ch) = (w as usize / 2, h as usize / 2);
        crate::stab::warp(&uv_src, &mut frame[ylen..], cw, ch, 2, 0, s as f32, (tx / 2.0) as f32, (ty / 2.0) as f32);
        crate::stab::warp(&uv_src, &mut frame[ylen..], cw, ch, 2, 1, s as f32, (tx / 2.0) as f32, (ty / 2.0) as f32);
        Ok(())
    }
}
