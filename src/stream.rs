//! Livestream: khung YUV từ camera → (tuỳ chọn) plugin ổn định → bộ mã hóa (H.264/H.265) → RTMP/RTMPS.
use crate::export::Jvm;
use crate::rtmp::{Meta, Rtmp};
use crate::video::*;
use std::ffi::c_int;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Khung NV12 đóng gói chặt (w*h*3/2 byte), đã xoay theo hướng stream.
pub struct StreamFrame {
    pub nv12: Vec<u8>,
    pub w: i32,
    pub h: i32,
    pub ts_us: i64,
}

#[derive(Clone)]
pub struct StabParams {
    pub port: u16,
    pub strength: u8,
    pub crop: u8,
    pub smooth: u16,
}

#[derive(Clone)]
pub struct StreamParams {
    pub url: String,
    pub key: String,
    pub hevc: bool,
    pub vbr_kbps: i32,
    pub fps: i32,
    pub abr_kbps: i32,
    pub audio: bool,
    pub stab: Option<StabParams>,
}

#[link(name = "mediandk")]
extern "C" {
    fn AMediaCodec_setParameters(c: *mut AMediaCodec, params: *const AMediaFormat) -> c_int;
}

/// Đổi bitrate mục tiêu của bộ mã hóa đang chạy (bps).
fn set_video_bitrate(codec: *mut AMediaCodec, bps: i32) {
    unsafe {
        let f = AMediaFormat_new();
        AMediaFormat_setInt32(f, cs("video-bitrate").as_ptr(), bps);
        AMediaCodec_setParameters(codec, f);
        AMediaFormat_delete(f);
    }
}

#[derive(Default)]
pub struct Stats {
    pub bytes: AtomicU64,
    /// tổng byte video đã gửi (để hiện kbps video và giữ dưới mức bitrate đã đặt)
    pub vbytes: AtomicU64,
    pub live: AtomicBool,
    pub stab_on: AtomicBool,
    pub state: Mutex<String>,
    pub failed: Mutex<Option<String>>,
}

pub fn codec_supported(hevc: bool) -> bool {
    unsafe {
        let c = AMediaCodec_createEncoderByType(cs(if hevc { "video/hevc" } else { "video/avc" }).as_ptr());
        if c.is_null() {
            false
        } else {
            AMediaCodec_delete(c);
            true
        }
    }
}

// ---------------- bộ mã hóa nhận bộ đệm NV12 ----------------
pub struct BufEncoder {
    pub codec: *mut AMediaCodec,
    pub w: i32,
    pub h: i32,
    stride: i32,
    sliceh: i32,
}

unsafe impl Send for BufEncoder {}

impl BufEncoder {
    pub fn new(w: i32, h: i32, fps: i32, bitrate_kbps: i32, hevc: bool, iframe_s: i32) -> Result<BufEncoder, String> {
        unsafe {
            let mime = if hevc { "video/hevc" } else { "video/avc" };
            let codec = AMediaCodec_createEncoderByType(cs(mime).as_ptr());
            if codec.is_null() {
                return Err(format!("Máy không có bộ mã hóa {}", if hevc { "H.265" } else { "H.264" }));
            }
            let mut ok = false;
            let bps = bitrate_kbps.max(100) * 1000;
            // bitrate-mode: 2 = CBR (cố định), 3 = CBR_FD (cố định, được phép bỏ khung), 1 = VBR (chỉ dùng khi máy không có CBR)
            // max-bitrate: giới hạn trên; máy không hiểu khoá này thì thử lại không có nó
            for (mode, with_max) in [(2, true), (3, true), (2, false), (3, false), (1, false)] {
                let f = AMediaFormat_new();
                AMediaFormat_setString(f, cs("mime").as_ptr(), cs(mime).as_ptr());
                AMediaFormat_setInt32(f, cs("width").as_ptr(), w);
                AMediaFormat_setInt32(f, cs("height").as_ptr(), h);
                AMediaFormat_setInt32(f, cs("color-format").as_ptr(), 21);
                AMediaFormat_setInt32(f, cs("bitrate").as_ptr(), bps);
                AMediaFormat_setInt32(f, cs("frame-rate").as_ptr(), fps);
                AMediaFormat_setInt32(f, cs("i-frame-interval").as_ptr(), iframe_s.max(1));
                AMediaFormat_setInt32(f, cs("max-bframes").as_ptr(), 0);
                AMediaFormat_setInt32(f, cs("bitrate-mode").as_ptr(), mode);
                if with_max {
                    AMediaFormat_setInt32(f, cs("max-bitrate").as_ptr(), bps);
                }
                let st = AMediaCodec_configure(codec, f, std::ptr::null_mut(), std::ptr::null_mut(), 1);
                AMediaFormat_delete(f);
                if st == 0 {
                    ok = true;
                    break;
                }
            }
            if !ok {
                AMediaCodec_delete(codec);
                return Err(format!("Cấu hình bộ mã hóa {}x{} lỗi", w, h));
            }
            if AMediaCodec_start(codec) != 0 {
                AMediaCodec_delete(codec);
                return Err("Không chạy được bộ mã hóa".to_string());
            }
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
            Ok(BufEncoder { codec, w, h, stride, sliceh })
        }
    }

    /// Đưa 1 khung NV12 đóng gói chặt vào bộ mã hóa; false nếu bộ mã hóa đang bận (bỏ khung).
    pub fn feed(&self, nv12: &[u8], pts_us: i64) -> bool {
        let (w, h, s, sl) = (self.w as usize, self.h as usize, self.stride as usize, self.sliceh as usize);
        if nv12.len() < w * h * 3 / 2 {
            return false;
        }
        unsafe {
            let idx = AMediaCodec_dequeueInputBuffer(self.codec, 0);
            if idx < 0 {
                return false;
            }
            let mut cap = 0usize;
            let p = AMediaCodec_getInputBuffer(self.codec, idx as usize, &mut cap);
            let total = s * sl * 3 / 2;
            if p.is_null() || cap < total {
                AMediaCodec_queueInputBuffer(self.codec, idx as usize, 0, 0, 0, 0);
                return false;
            }
            let dst = std::slice::from_raw_parts_mut(p, cap);
            for y in 0..h {
                dst[y * s..y * s + w].copy_from_slice(&nv12[y * w..y * w + w]);
            }
            let uv0 = s * sl;
            for y in 0..h / 2 {
                dst[uv0 + y * s..uv0 + y * s + w].copy_from_slice(&nv12[w * h + y * w..w * h + y * w + w]);
            }
            AMediaCodec_queueInputBuffer(self.codec, idx as usize, 0, total, pts_us.max(0) as u64, 0);
            true
        }
    }
}

impl Drop for BufEncoder {
    fn drop(&mut self) {
        unsafe {
            if !self.codec.is_null() {
                AMediaCodec_stop(self.codec);
                AMediaCodec_delete(self.codec);
            }
        }
    }
}

unsafe fn drain_all(codec: *mut AMediaCodec, out: &mut Vec<(Vec<u8>, BufInfo)>) {
    for _ in 0..24 {
        let mut info = BufInfo { offset: 0, size: 0, pts: 0, flags: 0 };
        let idx = AMediaCodec_dequeueOutputBuffer(codec, &mut info, 0);
        if idx == -1 {
            break;
        }
        if idx < 0 {
            continue;
        }
        if info.size > 0 {
            let mut sz = 0usize;
            let p = AMediaCodec_getOutputBuffer(codec, idx as usize, &mut sz);
            if !p.is_null() {
                out.push((std::slice::from_raw_parts(p.add(info.offset as usize), info.size as usize).to_vec(), info));
            }
        }
        AMediaCodec_releaseOutputBuffer(codec, idx as usize, false);
    }
}

// ---------------- đóng gói FLV ----------------
fn split_nals(d: &[u8]) -> Vec<Vec<u8>> {
    let mut starts: Vec<(usize, usize)> = Vec::new();
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

fn avc_seq(sps: &[u8], pps: &[u8]) -> Option<Vec<u8>> {
    if sps.len() < 4 {
        return None;
    }
    let mut p = vec![0x17, 0, 0, 0, 0, 1, sps[1], sps[2], sps[3], 0xFF, 0xE1];
    p.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    p.extend_from_slice(sps);
    p.push(1);
    p.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    p.extend_from_slice(pps);
    Some(p)
}

fn hevc_seq(vps: &[u8], sps: &[u8], pps: &[u8]) -> Option<Vec<u8>> {
    let r = unescape(sps);
    if r.len() < 15 {
        return None;
    }
    let mut p = vec![0x90, b'h', b'v', b'c', b'1', 1, r[3]];
    p.extend_from_slice(&r[4..8]);
    p.extend_from_slice(&r[8..14]);
    p.push(r[14]);
    p.extend_from_slice(&[0xF0, 0x00, 0xFC, 0xFD, 0xF8, 0xF8, 0x00, 0x00, 0x0F, 3]);
    for (t, n) in [(32u8, vps), (33u8, sps), (34u8, pps)] {
        p.push(0x80 | t);
        p.extend_from_slice(&1u16.to_be_bytes());
        p.extend_from_slice(&(n.len() as u16).to_be_bytes());
        p.extend_from_slice(n);
    }
    Some(p)
}

// ---------------- plugin ổn định (OpenCV) qua localhost ----------------
// Giao thức CSAR-STB v1 (xem README):
//   client → "CSARSTB1" + w(u16 BE) + h(u16 BE) + fps(u8) + mức ổn định(u8 0..100) + tỉ lệ cắt(u8 %) + cửa sổ làm mượt(u16 BE)
//   plugin → "CSAROK1"
//   mỗi khung: client → len(u32 BE) + NV12 đóng gói chặt; plugin → len(u32 BE) + NV12 cùng kích thước (len = 0: dùng nguyên khung gốc)
pub struct StabPlugin {
    s: TcpStream,
}

impl StabPlugin {
    pub fn connect(p: &StabParams, w: i32, h: i32, fps: i32) -> Result<StabPlugin, String> {
        let addr: SocketAddr = format!("127.0.0.1:{}", p.port).parse().map_err(|e: std::net::AddrParseError| e.to_string())?;
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(1500)).map_err(|e| format!("Không kết nối được plugin ổn định ({})", e))?;
        let _ = s.set_nodelay(true);
        s.set_read_timeout(Some(Duration::from_millis(2500))).map_err(|e| e.to_string())?;
        s.set_write_timeout(Some(Duration::from_millis(2500))).map_err(|e| e.to_string())?;
        let mut hello = b"CSARSTB1".to_vec();
        hello.extend_from_slice(&(w as u16).to_be_bytes());
        hello.extend_from_slice(&(h as u16).to_be_bytes());
        hello.push(fps.clamp(1, 120) as u8);
        hello.push(p.strength.min(100));
        hello.push(p.crop.clamp(50, 100));
        hello.extend_from_slice(&p.smooth.to_be_bytes());
        s.write_all(&hello).map_err(|e| e.to_string())?;
        let mut ok = [0u8; 7];
        s.read_exact(&mut ok).map_err(|e| format!("Plugin không trả lời ({})", e))?;
        if &ok != b"CSAROK1" {
            return Err("Plugin trả về phiên bản giao thức lạ".to_string());
        }
        s.set_read_timeout(Some(Duration::from_millis(300))).map_err(|e| e.to_string())?;
        Ok(StabPlugin { s })
    }

    pub fn process(&mut self, nv12: &mut Vec<u8>) -> Result<(), String> {
        self.s.write_all(&(nv12.len() as u32).to_be_bytes()).map_err(|e| e.to_string())?;
        self.s.write_all(nv12).map_err(|e| e.to_string())?;
        let mut l = [0u8; 4];
        self.s.read_exact(&mut l).map_err(|e| e.to_string())?;
        let n = u32::from_be_bytes(l) as usize;
        if n == 0 {
            return Ok(());
        }
        if n != nv12.len() {
            return Err("Plugin trả khung sai kích thước".to_string());
        }
        self.s.read_exact(nv12).map_err(|e| e.to_string())
    }
}

// ---------------- luồng phát ----------------
pub struct Streamer {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    pub stats: Arc<Stats>,
    pub t0: Instant,
}

fn fail(stats: &Stats, msg: &str) {
    if let Ok(mut f) = stats.failed.lock() {
        *f = Some(msg.to_string());
    }
    stats.live.store(false, Ordering::SeqCst);
}

#[allow(clippy::too_many_arguments)]
fn run(enc: BufEncoder, rx: Receiver<StreamFrame>, active: Arc<AtomicBool>, p: StreamParams, jvm: Jvm, stats: Arc<Stats>, stop: Arc<AtomicBool>) {
    let set = |m: &str| {
        if let Ok(mut g) = stats.state.lock() {
            *g = m.to_string();
        }
    };
    set("Đang kết nối máy chủ…");
    let meta = Meta { w: enc.w, h: enc.h, fps: p.fps, vkbps: p.vbr_kbps, akbps: p.abr_kbps, audio: p.audio, hevc: p.hevc };
    let mut rtmp = match Rtmp::connect(&p.url, &p.key, jvm, &meta) {
        Ok(r) => r,
        Err(e) => {
            fail(&stats, &e);
            return;
        }
    };
    let mut aud = if p.audio { unsafe { audio_new_br(p.abr_kbps.max(32) * 1000) } } else { None };
    let mut plugin: Option<StabPlugin> = None;
    if let Some(sp) = &p.stab {
        match StabPlugin::connect(sp, enc.w, enc.h, p.fps) {
            Ok(pl) => {
                plugin = Some(pl);
                stats.stab_on.store(true, Ordering::SeqCst);
            }
            Err(e) => set(&format!("⚠ {} → phát không ổn định", e)),
        }
    }
    stats.live.store(true, Ordering::SeqCst);
    if plugin.is_some() {
        set("Đang phát (ổn định: bật)");
    } else {
        set("Đang phát");
    }
    active.store(true, Ordering::SeqCst);

    let (mut vps, mut sps, mut pps): (Vec<u8>, Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new(), Vec::new());
    let mut vcfg = false;
    let mut acfg = false;
    let mut err: Option<String> = None;
    // bitrate video đã đặt là mức TỐI ĐA: đo mỗi giây, vượt thì hạ mục tiêu của bộ mã hóa, dưới thì nâng lại (không quá mức đã đặt)
    let cap_kbps = p.vbr_kbps.max(100) as f64;
    let mut cur_kbps = cap_kbps;
    let mut win_t = Instant::now();
    let mut win_bytes: u64 = 0;
    let mut vtotal: u64 = 0;

    'main: while !stop.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(mut f) => {
                if let Some(pl) = plugin.as_mut() {
                    if let Err(e) = pl.process(&mut f.nv12) {
                        plugin = None;
                        stats.stab_on.store(false, Ordering::SeqCst);
                        set(&format!("⚠ Plugin ổn định lỗi ({}) → phát không ổn định", e));
                    }
                }
                enc.feed(&f.nv12, f.ts_us);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        // video
        let mut outs: Vec<(Vec<u8>, BufInfo)> = Vec::new();
        unsafe { drain_all(enc.codec, &mut outs) };
        for (data, info) in outs {
            let nals = split_nals(&data);
            for n in &nals {
                if n.is_empty() {
                    continue;
                }
                if p.hevc {
                    match (n[0] >> 1) & 0x3f {
                        32 => vps = n.clone(),
                        33 => sps = n.clone(),
                        34 => pps = n.clone(),
                        _ => {}
                    }
                } else {
                    match n[0] & 0x1f {
                        7 => sps = n.clone(),
                        8 => pps = n.clone(),
                        _ => {}
                    }
                }
            }
            if !vcfg {
                let rec = if p.hevc {
                    if vps.is_empty() || sps.is_empty() || pps.is_empty() { None } else { hevc_seq(&vps, &sps, &pps) }
                } else if sps.is_empty() || pps.is_empty() {
                    None
                } else {
                    avc_seq(&sps, &pps)
                };
                if let Some(r) = rec {
                    if let Err(e) = rtmp.video(0, &r) {
                        err = Some(e);
                        break 'main;
                    }
                    vcfg = true;
                }
            }
            if (info.flags & FLAG_CONFIG) != 0 || !vcfg {
                continue;
            }
            let slices: Vec<&Vec<u8>> = nals
                .iter()
                .filter(|n| !n.is_empty() && if p.hevc { (n[0] >> 1) & 0x3f <= 21 } else { matches!(n[0] & 0x1f, 1 | 5) })
                .collect();
            if slices.is_empty() {
                continue;
            }
            let key = (info.flags & 1) != 0;
            let mut pl: Vec<u8> = if p.hevc {
                vec![if key { 0x93 } else { 0xA3 }, b'h', b'v', b'c', b'1']
            } else {
                vec![if key { 0x17 } else { 0x27 }, 1, 0, 0, 0]
            };
            for s in slices {
                pl.extend_from_slice(&(s.len() as u32).to_be_bytes());
                pl.extend_from_slice(s);
            }
            let plen = pl.len() as u64;
            if let Err(e) = rtmp.video((info.pts.max(0) / 1000) as u32, &pl) {
                err = Some(e);
                break 'main;
            }
            win_bytes += plen;
            vtotal += plen;
            stats.vbytes.store(vtotal, Ordering::Relaxed);
        }
        let we = win_t.elapsed();
        if we >= Duration::from_millis(1000) {
            let kbps = win_bytes as f64 * 8.0 / 1000.0 / we.as_secs_f64();
            let mut want = cur_kbps;
            if kbps > cap_kbps * 0.97 {
                want = (cur_kbps * cap_kbps / kbps * 0.92).max(cap_kbps * 0.35);
            } else if kbps < cap_kbps * 0.85 && cur_kbps < cap_kbps {
                want = (cur_kbps * 1.1).min(cap_kbps);
            }
            if (want - cur_kbps).abs() > cap_kbps * 0.02 {
                cur_kbps = want;
                set_video_bitrate(enc.codec, (cur_kbps * 1000.0) as i32);
            }
            win_t = Instant::now();
            win_bytes = 0;
        }
        // âm thanh
        if let Some(a) = aud.as_mut() {
            let mut aouts: Vec<(Vec<u8>, BufInfo)> = Vec::new();
            unsafe {
                audio_pump(a, false);
                drain_all(a.codec, &mut aouts);
            }
            for (data, info) in aouts {
                if (info.flags & FLAG_CONFIG) != 0 {
                    let mut pl = vec![0xAF, 0];
                    pl.extend_from_slice(&data);
                    if let Err(e) = rtmp.audio(0, &pl) {
                        err = Some(e);
                        break 'main;
                    }
                    acfg = true;
                    continue;
                }
                if !acfg {
                    continue;
                }
                let mut pl = vec![0xAF, 1];
                pl.extend_from_slice(&data);
                let ts = ((info.pts - a.base_us).max(0) / 1000) as u32;
                if let Err(e) = rtmp.audio(ts, &pl) {
                    err = Some(e);
                    break 'main;
                }
            }
        }
        stats.bytes.store(rtmp.sent, Ordering::Relaxed);
    }
    active.store(false, Ordering::SeqCst);
    if let Some(mut a) = aud.take() {
        unsafe { audio_free(&mut a) };
    }
    rtmp.close();
    stats.live.store(false, Ordering::SeqCst);
    if let Some(e) = err {
        fail(&stats, &e);
    }
    drop(enc);
}

impl Streamer {
    pub fn start(enc: BufEncoder, rx: Receiver<StreamFrame>, active: Arc<AtomicBool>, p: StreamParams, jvm: Jvm) -> Streamer {
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Stats::default());
        let (s2, st2) = (stop.clone(), stats.clone());
        let handle = std::thread::Builder::new().name("csar-stream".to_string()).spawn(move || run(enc, rx, active, p, jvm, st2, s2)).ok();
        Streamer { stop, handle, stats, t0: Instant::now() }
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
