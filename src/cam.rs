//! Camera2 qua NDK (libcamera2ndk + libmediandk): xem trước YUV + chụp JPEG.
#![allow(non_camel_case_types, non_snake_case, dead_code)]
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use crate::video;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ---------- kiểu mờ (opaque) ----------
pub enum ACameraManager {}
pub enum ACameraDevice {}
pub enum ACameraMetadata {}
pub enum ACaptureRequest {}
pub enum ACaptureSessionOutputContainer {}
pub enum ACaptureSessionOutput {}
pub enum ACameraOutputTarget {}
pub enum ACameraCaptureSession {}
pub enum AImageReader {}
pub enum AImage {}
pub enum ANativeWindow {}

#[repr(C)]
struct IdList {
    num: c_int,
    ids: *const *const c_char,
}
#[repr(C)]
struct ConstEntry {
    tag: u32,
    typ: u8,
    count: u32,
    data: *const c_void,
}
#[repr(C)]
struct DevCb {
    context: *mut c_void,
    on_disconnected: Option<unsafe extern "C" fn(*mut c_void, *mut ACameraDevice)>,
    on_error: Option<unsafe extern "C" fn(*mut c_void, *mut ACameraDevice, c_int)>,
}
#[repr(C)]
struct SessCb {
    context: *mut c_void,
    on_closed: Option<unsafe extern "C" fn(*mut c_void, *mut ACameraCaptureSession)>,
    on_ready: Option<unsafe extern "C" fn(*mut c_void, *mut ACameraCaptureSession)>,
    on_active: Option<unsafe extern "C" fn(*mut c_void, *mut ACameraCaptureSession)>,
}
#[repr(C)]
struct ImgListener {
    context: *mut c_void,
    on_image_available: Option<unsafe extern "C" fn(*mut c_void, *mut AImageReader)>,
}

#[link(name = "camera2ndk")]
extern "C" {
    fn ACameraManager_create() -> *mut ACameraManager;
    fn ACameraManager_delete(m: *mut ACameraManager);
    fn ACameraManager_getCameraIdList(m: *mut ACameraManager, l: *mut *mut IdList) -> c_int;
    fn ACameraManager_deleteCameraIdList(l: *mut IdList);
    fn ACameraManager_getCameraCharacteristics(m: *mut ACameraManager, id: *const c_char, out: *mut *mut ACameraMetadata) -> c_int;
    fn ACameraMetadata_free(m: *mut ACameraMetadata);
    fn ACameraMetadata_getConstEntry(m: *const ACameraMetadata, tag: u32, e: *mut ConstEntry) -> c_int;
    fn ACameraManager_openCamera(m: *mut ACameraManager, id: *const c_char, cb: *mut DevCb, dev: *mut *mut ACameraDevice) -> c_int;
    fn ACameraDevice_close(d: *mut ACameraDevice) -> c_int;
    fn ACameraDevice_createCaptureRequest(d: *const ACameraDevice, template: c_int, out: *mut *mut ACaptureRequest) -> c_int;
    fn ACaptureRequest_free(r: *mut ACaptureRequest);
    fn ACaptureRequest_addTarget(r: *mut ACaptureRequest, t: *const ACameraOutputTarget) -> c_int;
    fn ACaptureRequest_setEntry_u8(r: *mut ACaptureRequest, tag: u32, count: u32, data: *const u8) -> c_int;
    fn ACaptureRequest_setEntry_i32(r: *mut ACaptureRequest, tag: u32, count: u32, data: *const i32) -> c_int;
    fn ACameraOutputTarget_create(w: *mut ANativeWindow, out: *mut *mut ACameraOutputTarget) -> c_int;
    fn ACameraOutputTarget_free(t: *mut ACameraOutputTarget);
    fn ACaptureSessionOutputContainer_create(out: *mut *mut ACaptureSessionOutputContainer) -> c_int;
    fn ACaptureSessionOutputContainer_add(c: *mut ACaptureSessionOutputContainer, o: *const ACaptureSessionOutput) -> c_int;
    fn ACaptureSessionOutputContainer_free(c: *mut ACaptureSessionOutputContainer);
    fn ACaptureSessionOutput_create(w: *mut ANativeWindow, out: *mut *mut ACaptureSessionOutput) -> c_int;
    fn ACaptureSessionOutput_free(o: *mut ACaptureSessionOutput);
    fn ACameraDevice_createCaptureSession(
        d: *mut ACameraDevice,
        c: *const ACaptureSessionOutputContainer,
        cb: *const SessCb,
        out: *mut *mut ACameraCaptureSession,
    ) -> c_int;
    fn ACameraCaptureSession_setRepeatingRequest(
        s: *mut ACameraCaptureSession,
        cb: *mut c_void,
        n: c_int,
        reqs: *mut *mut ACaptureRequest,
        seq: *mut c_int,
    ) -> c_int;
    fn ACameraCaptureSession_capture(
        s: *mut ACameraCaptureSession,
        cb: *mut c_void,
        n: c_int,
        reqs: *mut *mut ACaptureRequest,
        seq: *mut c_int,
    ) -> c_int;
    fn ACameraCaptureSession_stopRepeating(s: *mut ACameraCaptureSession) -> c_int;
    fn ACameraCaptureSession_close(s: *mut ACameraCaptureSession);
}

#[link(name = "mediandk")]
extern "C" {
    fn AImageReader_new(w: i32, h: i32, format: i32, max_images: i32, out: *mut *mut AImageReader) -> c_int;
    fn AImageReader_delete(r: *mut AImageReader);
    fn AImageReader_getWindow(r: *mut AImageReader, out: *mut *mut ANativeWindow) -> c_int;
    fn AImageReader_setImageListener(r: *mut AImageReader, l: *mut ImgListener) -> c_int;
    fn AImageReader_acquireLatestImage(r: *mut AImageReader, out: *mut *mut AImage) -> c_int;
    fn AImage_delete(i: *mut AImage);
    fn AImage_getPlaneData(i: *const AImage, idx: c_int, data: *mut *mut u8, len: *mut c_int) -> c_int;
    fn AImage_getPlaneRowStride(i: *const AImage, idx: c_int, out: *mut c_int) -> c_int;
    fn AImage_getPlanePixelStride(i: *const AImage, idx: c_int, out: *mut c_int) -> c_int;
}

const TEMPLATE_PREVIEW: c_int = 1;
const TEMPLATE_STILL: c_int = 2;
const TEMPLATE_RECORD: c_int = 3;

const T_AE_COMP: u32 = 65537;
const T_AE_LOCK: u32 = 65538;
const T_AE_MODE: u32 = 65539;
const T_AE_REGIONS: u32 = 65540;
const T_AE_FPS: u32 = 65541;
const T_AF_MODE: u32 = 65543;
const T_AF_REGIONS: u32 = 65544;
const T_AF_TRIGGER: u32 = 65545;
const T_AWB_LOCK: u32 = 65546;
const T_VSTAB: u32 = 65553;
const T_AE_COMP_RANGE: u32 = 65557;
const T_AE_COMP_STEP: u32 = 65558;
const T_FLASH_MODE: u32 = 262146;
const T_FLASH_AVAIL: u32 = 327680;
const T_JPEG_ORIENTATION: u32 = 458755;
const T_JPEG_QUALITY: u32 = 458756;
const T_LENS_OIS: u32 = 524292;
const T_LENS_FACING: u32 = 524293;
const T_NR_MODE: u32 = 655360;
const T_CROP: u32 = 851968;
const T_MAX_ZOOM: u32 = 851972;
const T_STREAM_CFG: u32 = 851978;
const T_SENSOR_ORI: u32 = 917517;
const T_ACTIVE_ARRAY: u32 = 983040;

const FMT_YUV: i32 = 0x23;
const FMT_JPEG: i32 = 0x100;
const FMT_PRIVATE: i32 = 0x22;

// ---------- dữ liệu dùng chung với luồng callback ----------
#[derive(Default)]
pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

pub struct Shared {
    pub frame: Mutex<Frame>,
    pub seq: AtomicU64,
    pub shots: Mutex<Vec<Vec<u8>>>,
    pub lost: AtomicBool,
    pub closing: AtomicBool,
    /// thống kê cảnh (từ khung xem trước): độ sáng TB 0..255, tỉ lệ tối/sáng theo ‰
    pub luma: AtomicU32,
    pub dark_pm: AtomicU32,
    pub bright_pm: AtomicU32,
    rot: i32,
    mirror: bool,
    pw: i32,
    ph: i32,
}

unsafe extern "C" fn on_disc(ctx: *mut c_void, _d: *mut ACameraDevice) {
    let sh = &*(ctx as *const Shared);
    sh.lost.store(true, Ordering::SeqCst);
}
unsafe extern "C" fn on_err(ctx: *mut c_void, _d: *mut ACameraDevice, _e: c_int) {
    let sh = &*(ctx as *const Shared);
    sh.lost.store(true, Ordering::SeqCst);
}
unsafe extern "C" fn on_sess(_c: *mut c_void, _s: *mut ACameraCaptureSession) {}

unsafe fn plane(img: *mut AImage, i: c_int) -> Option<(&'static [u8], usize, usize)> {
    let mut p: *mut u8 = null_mut();
    let mut len: c_int = 0;
    let mut rs: c_int = 0;
    let mut ps: c_int = 0;
    if AImage_getPlaneData(img, i, &mut p, &mut len) != 0 || p.is_null() || len <= 0 {
        return None;
    }
    AImage_getPlaneRowStride(img, i, &mut rs);
    AImage_getPlanePixelStride(img, i, &mut ps);
    Some((std::slice::from_raw_parts(p, len as usize), rs.max(0) as usize, ps.max(1) as usize))
}

unsafe extern "C" fn on_preview(ctx: *mut c_void, reader: *mut AImageReader) {
    let sh = &*(ctx as *const Shared);
    if sh.closing.load(Ordering::SeqCst) {
        return;
    }
    let mut img: *mut AImage = null_mut();
    if AImageReader_acquireLatestImage(reader, &mut img) != 0 || img.is_null() {
        return;
    }
    convert(sh, img);
    AImage_delete(img);
}

/// YUV_420_888 -> RGBA (giảm 1/2 độ phân giải, xoay theo màn hình) + thống kê cảnh.
unsafe fn convert(sh: &Shared, img: *mut AImage) {
    let (y, ys, _) = match plane(img, 0) {
        Some(v) => v,
        None => return,
    };
    let (u, us, ups) = match plane(img, 1) {
        Some(v) => v,
        None => return,
    };
    let (v, vs, vps) = match plane(img, 2) {
        Some(v) => v,
        None => return,
    };
    let sw = (sh.pw / 2) as usize;
    let shh = (sh.ph / 2) as usize;
    if sw == 0 || shh == 0 {
        return;
    }
    let (dw, dh) = if sh.rot == 90 || sh.rot == 270 { (shh, sw) } else { (sw, shh) };
    let mut out = vec![255u8; dw * dh * 4];
    let (mut sum, mut dark, mut bright) = (0u64, 0u32, 0u32);
    for dy in 0..dh {
        for dx in 0..dw {
            let mx = if sh.mirror { dw - 1 - dx } else { dx };
            let (sx, sy) = match sh.rot {
                90 => (dy, shh - 1 - mx),
                270 => (sw - 1 - dy, mx),
                180 => (sw - 1 - mx, shh - 1 - dy),
                _ => (mx, dy),
            };
            let px = sx * 2;
            let py = sy * 2;
            let yi = py * ys + px;
            let iu = (py / 2) * us + (px / 2) * ups;
            let iv = (py / 2) * vs + (px / 2) * vps;
            if yi >= y.len() || iu >= u.len() || iv >= v.len() {
                continue;
            }
            let yv = y[yi] as i32;
            sum += yv as u64;
            if yv < 25 {
                dark += 1;
            } else if yv > 235 {
                bright += 1;
            }
            let uv = u[iu] as i32 - 128;
            let vv = v[iv] as i32 - 128;
            let r = yv + ((1436 * vv) >> 10);
            let g = yv - ((352 * uv + 731 * vv) >> 10);
            let b = yv + ((1814 * uv) >> 10);
            let o = (dy * dw + dx) * 4;
            out[o] = r.clamp(0, 255) as u8;
            out[o + 1] = g.clamp(0, 255) as u8;
            out[o + 2] = b.clamp(0, 255) as u8;
        }
    }
    let n = (dw * dh) as u64;
    sh.luma.store((sum / n.max(1)) as u32, Ordering::Relaxed);
    sh.dark_pm.store(dark * 1000 / n.max(1) as u32, Ordering::Relaxed);
    sh.bright_pm.store(bright * 1000 / n.max(1) as u32, Ordering::Relaxed);
    if let Ok(mut f) = sh.frame.lock() {
        *f = Frame { w: dw, h: dh, rgba: out };
        sh.seq.fetch_add(1, Ordering::SeqCst);
    }
}

unsafe extern "C" fn on_jpeg(ctx: *mut c_void, reader: *mut AImageReader) {
    let sh = &*(ctx as *const Shared);
    if sh.closing.load(Ordering::SeqCst) {
        return;
    }
    let mut img: *mut AImage = null_mut();
    if AImageReader_acquireLatestImage(reader, &mut img) != 0 || img.is_null() {
        return;
    }
    if let Some((data, _, _)) = plane(img, 0) {
        if let Ok(mut v) = sh.shots.lock() {
            v.push(data.to_vec());
        }
    }
    AImage_delete(img);
}

// ---------- thông tin / điều khiển ----------
#[derive(Clone, Default)]
pub struct Info {
    pub act: [i32; 4], // left, top, width, height
    pub max_zoom: f32,
    pub has_flash: bool,
    pub ev_min: i32,
    pub ev_max: i32,
    pub ev_step: f32,
    pub sensor_ori: i32,
}

#[derive(Clone)]
pub struct Ctl {
    pub zoom: f32,
    pub torch: bool,
    pub ae_lock: bool,
    pub awb_lock: bool,
    pub af: Option<(f32, f32)>,
    pub night: bool,
    pub eis: u8,
    pub low_fps: bool,
    pub recording: bool,
}

impl Default for Ctl {
    fn default() -> Ctl {
        Ctl { zoom: 1.0, torch: false, ae_lock: false, awb_lock: false, af: None, night: false, eis: 0, low_fps: false, recording: false }
    }
}

pub struct OpenOpts {
    pub front: bool,
    pub dir: PathBuf,
    pub video: bool,
    pub wide: bool,
    pub vres: u8, // 0=1080p 1=720p 2=4K
    pub vhevc: bool,
    pub vbr_kbps: i32,
    pub vfps: i32,
    pub jpeg_q: u8,
    pub live: bool,
    pub live_hevc: bool,
    pub compat: bool,
}

unsafe fn read_i32s(md: *mut ACameraMetadata, tag: u32) -> Vec<i32> {
    let mut e = ConstEntry { tag: 0, typ: 0, count: 0, data: std::ptr::null() };
    if ACameraMetadata_getConstEntry(md, tag, &mut e) != 0 || e.data.is_null() {
        return Vec::new();
    }
    std::slice::from_raw_parts(e.data as *const i32, e.count as usize).to_vec()
}
unsafe fn read_u8(md: *mut ACameraMetadata, tag: u32) -> Option<u8> {
    let mut e = ConstEntry { tag: 0, typ: 0, count: 0, data: std::ptr::null() };
    if ACameraMetadata_getConstEntry(md, tag, &mut e) != 0 || e.data.is_null() || e.count == 0 {
        return None;
    }
    Some(*(e.data as *const u8))
}
unsafe fn read_f32(md: *mut ACameraMetadata, tag: u32) -> Option<f32> {
    let mut e = ConstEntry { tag: 0, typ: 0, count: 0, data: std::ptr::null() };
    if ACameraMetadata_getConstEntry(md, tag, &mut e) != 0 || e.data.is_null() || e.count == 0 {
        return None;
    }
    Some(*(e.data as *const f32))
}

fn pick_aspect(list: &[(i32, i32)], r: (i32, i32), max_area: i64) -> Option<(i32, i32)> {
    let ok = |s: &&(i32, i32)| (s.0 as i64 * r.1 as i64 - s.1 as i64 * r.0 as i64).abs() <= (r.0.max(r.1) as i64) && s.0 as i64 * s.1 as i64 <= max_area;
    list.iter().filter(ok).max_by_key(|s| s.0 as i64 * s.1 as i64).cloned()
}

fn largest_under(list: &[(i32, i32)], max_area: i64) -> Option<(i32, i32)> {
    list.iter().filter(|s| s.0 as i64 * s.1 as i64 <= max_area).max_by_key(|s| s.0 as i64 * s.1 as i64).cloned()
}

fn rtime() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

// ---------- Camera ----------
pub struct Camera {
    pub sh: Arc<Shared>,
    pub info: Info,
    pub ctl: Ctl,
    pub video: bool,
    pub vsize: (i32, i32),
    pub jsize: (i32, i32),
    pub rec: Option<video::Recorder>,
    enc: Option<video::Encoder>,
    live: Option<video::Live>,
    jpeg_q: u8,
    vfps: i32,
    dir: PathBuf,
    ctx: *const Shared,
    mgr: *mut ACameraManager,
    dev: *mut ACameraDevice,
    rd_p: *mut AImageReader,
    rd_j: *mut AImageReader,
    cont: *mut ACaptureSessionOutputContainer,
    out_p: *mut ACaptureSessionOutput,
    out_j: *mut ACaptureSessionOutput,
    out_v: *mut ACaptureSessionOutput,
    tgt_p: *mut ACameraOutputTarget,
    tgt_j: *mut ACameraOutputTarget,
    tgt_v: *mut ACameraOutputTarget,
    sess: *mut ACameraCaptureSession,
    req_p: *mut ACaptureRequest,
    dev_cb: Box<DevCb>,
    sess_cb: Box<SessCb>,
    lis_p: Box<ImgListener>,
    lis_j: Box<ImgListener>,
}

impl Camera {
    pub fn open(o: OpenOpts) -> Result<Camera, String> {
        unsafe {
            let mgr = ACameraManager_create();
            if mgr.is_null() {
                return Err("Không tạo được ACameraManager".into());
            }
            let mut list: *mut IdList = null_mut();
            if ACameraManager_getCameraIdList(mgr, &mut list) != 0 || list.is_null() {
                ACameraManager_delete(mgr);
                return Err("Không lấy được danh sách camera".into());
            }
            let n = (*list).num.max(0) as usize;
            let mut chosen: Option<CString> = None;
            let mut first: Option<CString> = None;
            for i in 0..n {
                let id = *(*list).ids.add(i);
                if first.is_none() {
                    first = Some(CStr::from_ptr(id).to_owned());
                }
                let mut md: *mut ACameraMetadata = null_mut();
                if ACameraManager_getCameraCharacteristics(mgr, id, &mut md) == 0 && !md.is_null() {
                    if let Some(f) = read_u8(md, T_LENS_FACING) {
                        if chosen.is_none() && ((o.front && f == 0) || (!o.front && f == 1)) {
                            chosen = Some(CStr::from_ptr(id).to_owned());
                        }
                    }
                    ACameraMetadata_free(md);
                }
            }
            ACameraManager_deleteCameraIdList(list);
            let id = match chosen.or(first) {
                Some(i) => i,
                None => {
                    ACameraManager_delete(mgr);
                    return Err("Máy không có camera".into());
                }
            };

            // đặc tính camera
            let mut info = Info { act: [0, 0, 4000, 3000], max_zoom: 1.0, has_flash: false, ev_min: 0, ev_max: 0, ev_step: 0.0, sensor_ori: if o.front { 270 } else { 90 } };
            let mut yuv: Vec<(i32, i32)> = Vec::new();
            let mut jpg: Vec<(i32, i32)> = Vec::new();
            let mut prv: Vec<(i32, i32)> = Vec::new();
            let mut md: *mut ACameraMetadata = null_mut();
            if ACameraManager_getCameraCharacteristics(mgr, id.as_ptr(), &mut md) == 0 && !md.is_null() {
                let a = read_i32s(md, T_ACTIVE_ARRAY);
                if a.len() >= 4 {
                    info.act = [a[0], a[1], a[2], a[3]];
                }
                info.max_zoom = read_f32(md, T_MAX_ZOOM).unwrap_or(1.0).max(1.0);
                info.has_flash = read_u8(md, T_FLASH_AVAIL).unwrap_or(0) == 1;
                let r = read_i32s(md, T_AE_COMP_RANGE);
                if r.len() >= 2 {
                    info.ev_min = r[0];
                    info.ev_max = r[1];
                }
                let st = read_i32s(md, T_AE_COMP_STEP);
                if st.len() >= 2 && st[1] != 0 {
                    info.ev_step = st[0] as f32 / st[1] as f32;
                }
                let so = read_i32s(md, T_SENSOR_ORI);
                if let Some(v) = so.first() {
                    info.sensor_ori = *v;
                } else if let Some(v) = read_u8(md, T_SENSOR_ORI) {
                    info.sensor_ori = v as i32;
                }
                let cfg = read_i32s(md, T_STREAM_CFG);
                for c in cfg.chunks(4) {
                    if c.len() < 4 || c[3] != 0 {
                        continue;
                    }
                    match c[0] {
                        FMT_JPEG => jpg.push((c[1], c[2])),
                        FMT_YUV => yuv.push((c[1], c[2])),
                        FMT_PRIVATE => prv.push((c[1], c[2])),
                        _ => {}
                    }
                }
                ACameraMetadata_free(md);
            }

            // chọn kích thước
            let aspect = if o.wide { (16, 9) } else { (4, 3) };
            let (jmax, pmax): (i64, i64) = if o.compat { (3_300_000, 400_000) } else { (12_600_000, 1_300_000) };
            let jsize = pick_aspect(&jpg, aspect, jmax).or_else(|| largest_under(&jpg, jmax)).unwrap_or(if o.compat { (1280, 720) } else { (1920, 1080) });
            let target_h = match o.vres {
                1 => 720,
                2 => 2160,
                _ => 1080,
            };
            let vsize = {
                let cands: Vec<(i32, i32)> = prv.iter().cloned().filter(|s| s.1 <= target_h && (s.0 as i64 * 9 - s.1 as i64 * 16).abs() <= 16).collect();
                cands.iter().max_by_key(|s| s.0 as i64 * s.1 as i64).cloned().unwrap_or((1920, 1080))
            };
            let refsz = if o.video { vsize } else { jsize };
            let psize = pick_aspect(&yuv, refsz, pmax).or_else(|| largest_under(&yuv, pmax)).unwrap_or((640, 480));

            let sh = Arc::new(Shared {
                frame: Mutex::new(Frame::default()),
                seq: AtomicU64::new(0),
                shots: Mutex::new(Vec::new()),
                lost: AtomicBool::new(false),
                closing: AtomicBool::new(false),
                luma: AtomicU32::new(128),
                dark_pm: AtomicU32::new(0),
                bright_pm: AtomicU32::new(0),
                rot: info.sensor_ori,
                mirror: o.front,
                pw: psize.0,
                ph: psize.1,
            });
            let ctx = Arc::into_raw(sh.clone());
            let ctxp = ctx as *mut c_void;
            let mut c = Camera {
                sh,
                info,
                ctl: Ctl::default(),
                video: o.video,
                vsize,
                jsize,
                rec: None,
                enc: None,
                live: None,
                jpeg_q: o.jpeg_q,
                vfps: o.vfps,
                dir: o.dir.clone(),
                ctx,
                mgr,
                dev: null_mut(),
                rd_p: null_mut(),
                rd_j: null_mut(),
                cont: null_mut(),
                out_p: null_mut(),
                out_j: null_mut(),
                out_v: null_mut(),
                tgt_p: null_mut(),
                tgt_j: null_mut(),
                tgt_v: null_mut(),
                sess: null_mut(),
                req_p: null_mut(),
                dev_cb: Box::new(DevCb { context: ctxp, on_disconnected: Some(on_disc), on_error: Some(on_err) }),
                sess_cb: Box::new(SessCb { context: ctxp, on_closed: Some(on_sess), on_ready: Some(on_sess), on_active: Some(on_sess) }),
                lis_p: Box::new(ImgListener { context: ctxp, on_image_available: Some(on_preview) }),
                lis_j: Box::new(ImgListener { context: ctxp, on_image_available: Some(on_jpeg) }),
            };

            if AImageReader_new(psize.0, psize.1, FMT_YUV, 3, &mut c.rd_p) != 0 || c.rd_p.is_null() {
                return Err(format!("Không tạo được reader xem trước {}x{}", psize.0, psize.1));
            }
            AImageReader_setImageListener(c.rd_p, &mut *c.lis_p);
            let mut win_p: *mut ANativeWindow = null_mut();
            AImageReader_getWindow(c.rd_p, &mut win_p);
            if win_p.is_null() {
                return Err("Không lấy được surface xem trước".into());
            }
            let mut win_x: *mut ANativeWindow = null_mut();
            let mut win_l: *mut ANativeWindow = null_mut();
            if o.video {
                let bitrate = o.vbr_kbps.max(1000) * 1000;
                let enc = match video::Encoder::new(vsize.0, vsize.1, o.vfps, bitrate, o.vhevc) {
                    Ok(e) => e,
                    Err(e1) => {
                        if o.vhevc {
                            video::Encoder::new(vsize.0, vsize.1, o.vfps, bitrate, false).map_err(|e2| format!("{} / {}", e1, e2))?
                        } else {
                            return Err(e1);
                        }
                    }
                };
                win_x = enc.window;
                c.enc = Some(enc);
            } else {
                if AImageReader_new(jsize.0, jsize.1, FMT_JPEG, 4, &mut c.rd_j) != 0 || c.rd_j.is_null() {
                    return Err(format!("Không tạo được reader JPEG {}x{}", jsize.0, jsize.1));
                }
                AImageReader_setImageListener(c.rd_j, &mut *c.lis_j);
                AImageReader_getWindow(c.rd_j, &mut win_x);
                if win_x.is_null() {
                    return Err("Không lấy được surface JPEG".into());
                }
                if o.live {
                    let lsz = pick_aspect(&prv, jsize, 2_100_000).or_else(|| largest_under(&prv, 2_100_000)).unwrap_or((1280, 720));
                    let enc = match video::Encoder::new(lsz.0, lsz.1, 30, 10_000_000, o.live_hevc) {
                        Ok(e) => e,
                        Err(e1) => {
                            if o.live_hevc {
                                video::Encoder::new(lsz.0, lsz.1, 30, 10_000_000, false).map_err(|e2| format!("Live: {} / {}", e1, e2))?
                            } else {
                                return Err(format!("Live: {}", e1));
                            }
                        }
                    };
                    win_l = enc.window;
                    c.enc = Some(enc);
                }
            }

            let st = ACameraManager_openCamera(c.mgr, id.as_ptr(), &mut *c.dev_cb, &mut c.dev);
            if st != 0 || c.dev.is_null() {
                return Err(format!("Mở camera lỗi (mã {})", st));
            }
            if ACaptureSessionOutputContainer_create(&mut c.cont) != 0 {
                return Err("Tạo output container lỗi".into());
            }
            ACaptureSessionOutput_create(win_p, &mut c.out_p);
            ACaptureSessionOutputContainer_add(c.cont, c.out_p);
            ACameraOutputTarget_create(win_p, &mut c.tgt_p);
            if o.video {
                ACaptureSessionOutput_create(win_x, &mut c.out_v);
                ACaptureSessionOutputContainer_add(c.cont, c.out_v);
                ACameraOutputTarget_create(win_x, &mut c.tgt_v);
            } else {
                ACaptureSessionOutput_create(win_x, &mut c.out_j);
                ACaptureSessionOutputContainer_add(c.cont, c.out_j);
                ACameraOutputTarget_create(win_x, &mut c.tgt_j);
            }
            if o.live && !win_l.is_null() {
                ACaptureSessionOutput_create(win_l, &mut c.out_v);
                ACaptureSessionOutputContainer_add(c.cont, c.out_v);
                ACameraOutputTarget_create(win_l, &mut c.tgt_v);
            }
            let st = ACameraDevice_createCaptureSession(c.dev, c.cont, &*c.sess_cb, &mut c.sess);
            if st != 0 || c.sess.is_null() {
                return Err(format!("Tạo session lỗi (mã {})", st));
            }
            if o.live {
                if let Some(enc) = c.enc.take() {
                    c.live = Some(video::Live::start(enc, o.dir.clone(), c.info.sensor_ori));
                    c.ctl.recording = true;
                }
            }
            c.apply()?;
            Ok(c)
        }
    }

    pub fn is_lost(&self) -> bool {
        self.sh.lost.load(Ordering::SeqCst)
    }

    pub fn take_shots(&self) -> Vec<Vec<u8>> {
        match self.sh.shots.lock() {
            Ok(mut v) => std::mem::take(&mut *v),
            Err(_) => Vec::new(),
        }
    }

    fn crop(&self) -> [i32; 4] {
        let a = self.info.act;
        let z = self.ctl.zoom.clamp(1.0, self.info.max_zoom.max(1.0));
        let cw = ((a[2] as f32 / z) as i32).max(16) & !1;
        let ch = ((a[3] as f32 / z) as i32).max(16) & !1;
        [a[0] + (a[2] - cw) / 2, a[1] + (a[3] - ch) / 2, cw, ch]
    }

    fn region(&self, nx: f32, ny: f32) -> [i32; 5] {
        let a = self.info.act;
        let c = self.crop();
        let cx = c[0] + (nx.clamp(0.0, 1.0) * c[2] as f32) as i32;
        let cy = c[1] + (ny.clamp(0.0, 1.0) * c[3] as f32) as i32;
        let half = (c[2].min(c[3]) as f32 * 0.1) as i32;
        let x0 = (cx - half).max(a[0]);
        let y0 = (cy - half).max(a[1]);
        let x1 = (cx + half).min(a[0] + a[2] - 1);
        let y1 = (cy + half).min(a[1] + a[3] - 1);
        [x0, y0, x1, y1, 1000]
    }

    unsafe fn make_req(&self, template: c_int, still: bool) -> Result<*mut ACaptureRequest, String> {
        let mut r: *mut ACaptureRequest = null_mut();
        if ACameraDevice_createCaptureRequest(self.dev, template, &mut r) != 0 || r.is_null() {
            return Err("Tạo request lỗi".into());
        }
        let set8 = |tag: u32, v: u8| {
            ACaptureRequest_setEntry_u8(r, tag, 1, &v);
        };
        let set32 = |tag: u32, v: &[i32]| {
            ACaptureRequest_setEntry_i32(r, tag, v.len() as u32, v.as_ptr());
        };
        set8(T_AE_MODE, 1);
        set8(T_AF_MODE, if self.video && !still { 3 } else { 4 });
        set8(T_AE_LOCK, self.ctl.ae_lock as u8);
        set8(T_AWB_LOCK, self.ctl.awb_lock as u8);
        set8(T_FLASH_MODE, if self.ctl.torch && self.info.has_flash { 2 } else { 0 });
        set32(T_CROP, &self.crop());
        if let Some((x, y)) = self.ctl.af {
            let reg = self.region(x, y);
            set32(T_AF_REGIONS, &reg);
            set32(T_AE_REGIONS, &reg);
        }
        if self.video {
            let f = self.vfps.max(15);
            set32(T_AE_FPS, &[if self.ctl.low_fps { 15 } else { f }, f]);
            match self.ctl.eis {
                1 | 2 => set8(T_VSTAB, 1),
                3 => set8(T_LENS_OIS, 1),
                _ => {}
            }
        } else if self.ctl.night && self.info.ev_step > 0.0 {
            let ec = ((1.5 / self.info.ev_step).round() as i32).clamp(self.info.ev_min, self.info.ev_max);
            if ec != 0 {
                set32(T_AE_COMP, &[ec]);
            }
            set8(T_NR_MODE, 2);
        }
        Ok(r)
    }

    /// Dựng lại request lặp (xem trước/quay) theo trạng thái điều khiển hiện tại.
    pub fn apply(&mut self) -> Result<(), String> {
        unsafe {
            if self.sess.is_null() || self.dev.is_null() {
                return Err("Camera chưa sẵn sàng".into());
            }
            let tpl = if self.video { TEMPLATE_RECORD } else { TEMPLATE_PREVIEW };
            let r = self.make_req(tpl, false)?;
            ACaptureRequest_addTarget(r, self.tgt_p);
            if self.ctl.recording && !self.tgt_v.is_null() {
                ACaptureRequest_addTarget(r, self.tgt_v);
            }
            let mut rr = r;
            let st = ACameraCaptureSession_setRepeatingRequest(self.sess, null_mut(), 1, &mut rr, null_mut());
            if !self.req_p.is_null() {
                ACaptureRequest_free(self.req_p);
            }
            self.req_p = r;
            if st != 0 {
                return Err(format!("Bật xem trước lỗi (mã {})", st));
            }
            Ok(())
        }
    }

    pub fn set_zoom(&mut self, z: f32) {
        self.ctl.zoom = z.clamp(1.0, self.info.max_zoom.max(1.0));
        let _ = self.apply();
    }

    /// Chạm lấy nét: (nx,ny) là toạ độ chuẩn hoá 0..1 trong khung cảm biến (đã tính vùng crop).
    pub fn focus_at(&mut self, nx: f32, ny: f32) {
        self.ctl.af = Some((nx, ny));
        let _ = self.apply();
        unsafe {
            let tpl = if self.video { TEMPLATE_RECORD } else { TEMPLATE_PREVIEW };
            if let Ok(mut r) = self.make_req(tpl, false) {
                ACaptureRequest_addTarget(r, self.tgt_p);
                let t: u8 = 1;
                ACaptureRequest_setEntry_u8(r, T_AF_TRIGGER, 1, &t);
                ACameraCaptureSession_capture(self.sess, null_mut(), 1, &mut r, null_mut());
                ACaptureRequest_free(r);
            }
        }
    }

    /// Chụp 1 ảnh JPEG; ảnh về qua take_shots().
    pub fn shoot(&self) -> Result<(), String> {
        if self.video || self.tgt_j.is_null() {
            return Err("Đang ở chế độ video".into());
        }
        unsafe {
            let mut req = self.make_req(TEMPLATE_STILL, true)?;
            ACaptureRequest_addTarget(req, self.tgt_j);
            ACaptureRequest_setEntry_u8(req, T_JPEG_QUALITY, 1, &self.jpeg_q);
            let o: i32 = self.info.sensor_ori;
            ACaptureRequest_setEntry_i32(req, T_JPEG_ORIENTATION, 1, &o);
            let st = ACameraCaptureSession_capture(self.sess, null_mut(), 1, &mut req, null_mut());
            ACaptureRequest_free(req);
            if st != 0 {
                return Err(format!("Chụp lỗi (mã {})", st));
            }
            Ok(())
        }
    }

    pub fn hdr_ok(&self) -> bool {
        !self.video && self.info.ev_max > self.info.ev_min && self.info.ev_step > 0.0
    }

    /// Chụp 3 ảnh -2/0/+2 EV (khoá cân bằng trắng) để ghép HDR.
    pub fn shoot_hdr(&self) -> Result<(), String> {
        if !self.hdr_ok() || self.tgt_j.is_null() {
            return Err("Máy không chụp HDR được".into());
        }
        unsafe {
            let mut reqs: [*mut ACaptureRequest; 3] = [null_mut(); 3];
            let evs = [-2.0f32, 0.0, 2.0];
            for (i, ev) in evs.iter().enumerate() {
                let r = self.make_req(TEMPLATE_STILL, true)?;
                ACaptureRequest_addTarget(r, self.tgt_j);
                ACaptureRequest_setEntry_u8(r, T_JPEG_QUALITY, 1, &self.jpeg_q);
                let o: i32 = self.info.sensor_ori;
                ACaptureRequest_setEntry_i32(r, T_JPEG_ORIENTATION, 1, &o);
                let one: u8 = 1;
                ACaptureRequest_setEntry_u8(r, T_AWB_LOCK, 1, &one);
                let idx = ((*ev / self.info.ev_step).round() as i32).clamp(self.info.ev_min, self.info.ev_max);
                ACaptureRequest_setEntry_i32(r, T_AE_COMP, 1, &idx);
                reqs[i] = r;
            }
            let st = ACameraCaptureSession_capture(self.sess, null_mut(), 3, reqs.as_mut_ptr(), null_mut());
            for r in reqs.iter() {
                if !r.is_null() {
                    ACaptureRequest_free(*r);
                }
            }
            if st != 0 {
                return Err(format!("Chụp HDR lỗi (mã {})", st));
            }
            Ok(())
        }
    }

    pub fn start_rec(&mut self, audio: bool) -> Result<(), String> {
        let enc = self.enc.take().ok_or_else(|| "Chưa có bộ mã hóa (mở lại camera)".to_string())?;
        let _ = std::fs::create_dir_all(&self.dir);
        let path = self.dir.join(format!("VID_{}.mp4", rtime()));
        let r = video::Recorder::start(enc, path, audio, self.info.sensor_ori)?;
        self.rec = Some(r);
        self.ctl.recording = true;
        self.apply()
    }

    /// Dừng quay. Sau khi dừng phải mở lại camera (bộ mã hóa đã dùng xong).
    pub fn stop_rec(&mut self) -> Result<PathBuf, String> {
        self.ctl.recording = false;
        let _ = self.apply();
        match self.rec.take() {
            Some(r) => r.stop(),
            None => Err("Không đang quay".into()),
        }
    }

    /// Live: đánh dấu thời điểm bấm chụp để cắt clip (trước/sau ~1,5 giây).
    pub fn live_mark(&self) -> bool {
        match &self.live {
            Some(l) => {
                l.mark();
                true
            }
            None => false,
        }
    }

    pub fn take_clips(&self) -> Vec<video::ClipResult> {
        match &self.live {
            Some(l) => l.take(),
            None => Vec::new(),
        }
    }

    pub fn has_live(&self) -> bool {
        self.live.is_some()
    }

    pub fn rec_elapsed(&self) -> Option<Duration> {
        self.rec.as_ref().map(|r| r.t0.elapsed())
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        unsafe {
            self.sh.closing.store(true, Ordering::SeqCst);
            if let Some(r) = self.rec.take() {
                let _ = r.stop();
            }
            if !self.sess.is_null() {
                ACameraCaptureSession_stopRepeating(self.sess);
                ACameraCaptureSession_close(self.sess);
            }
            if !self.dev.is_null() {
                ACameraDevice_close(self.dev);
            }
            self.live = None;
            if !self.req_p.is_null() {
                ACaptureRequest_free(self.req_p);
            }
            for t in [self.tgt_p, self.tgt_j, self.tgt_v] {
                if !t.is_null() {
                    ACameraOutputTarget_free(t);
                }
            }
            for o in [self.out_p, self.out_j, self.out_v] {
                if !o.is_null() {
                    ACaptureSessionOutput_free(o);
                }
            }
            if !self.cont.is_null() {
                ACaptureSessionOutputContainer_free(self.cont);
            }
            if !self.rd_p.is_null() {
                AImageReader_delete(self.rd_p);
            }
            if !self.rd_j.is_null() {
                AImageReader_delete(self.rd_j);
            }
            self.enc = None;
            if !self.mgr.is_null() {
                ACameraManager_delete(self.mgr);
            }
            if !self.ctx.is_null() {
                drop(Arc::from_raw(self.ctx));
            }
        }
    }
}
