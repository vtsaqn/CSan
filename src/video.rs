//! Quay video: MediaCodec (H.264/HEVC qua surface) + AAudio/AAC + MediaMuxer (mp4).
#![allow(non_camel_case_types, non_snake_case, dead_code)]
use crate::cam::ANativeWindow;
use std::ffi::{c_char, c_int, c_void, CString};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub enum AMediaFormat {}
pub enum AMediaCodec {}
pub enum AMediaMuxer {}
pub enum AAudioStreamBuilder {}
pub enum AAudioStream {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BufInfo {
    pub offset: i32,
    pub size: i32,
    pub pts: i64,
    pub flags: u32,
}

pub enum AMediaExtractor {}

#[link(name = "mediandk")]
extern "C" {
    pub fn AMediaFormat_new() -> *mut AMediaFormat;
    pub fn AMediaFormat_delete(f: *mut AMediaFormat) -> c_int;
    pub fn AMediaFormat_setString(f: *mut AMediaFormat, name: *const c_char, v: *const c_char);
    pub fn AMediaFormat_setInt32(f: *mut AMediaFormat, name: *const c_char, v: i32);
    pub fn AMediaCodec_createEncoderByType(t: *const c_char) -> *mut AMediaCodec;
    pub fn AMediaCodec_configure(c: *mut AMediaCodec, f: *const AMediaFormat, surf: *mut ANativeWindow, crypto: *mut c_void, flags: u32) -> c_int;
    pub fn AMediaCodec_createInputSurface(c: *mut AMediaCodec, out: *mut *mut ANativeWindow) -> c_int;
    pub fn AMediaCodec_start(c: *mut AMediaCodec) -> c_int;
    pub fn AMediaCodec_stop(c: *mut AMediaCodec) -> c_int;
    pub fn AMediaCodec_delete(c: *mut AMediaCodec) -> c_int;
    pub fn AMediaCodec_dequeueInputBuffer(c: *mut AMediaCodec, timeout_us: i64) -> isize;
    pub fn AMediaCodec_getInputBuffer(c: *mut AMediaCodec, idx: usize, size: *mut usize) -> *mut u8;
    pub fn AMediaCodec_queueInputBuffer(c: *mut AMediaCodec, idx: usize, offset: i64, size: usize, time: u64, flags: u32) -> c_int;
    pub fn AMediaCodec_dequeueOutputBuffer(c: *mut AMediaCodec, info: *mut BufInfo, timeout_us: i64) -> isize;
    pub fn AMediaCodec_getOutputBuffer(c: *mut AMediaCodec, idx: usize, size: *mut usize) -> *mut u8;
    pub fn AMediaCodec_getOutputFormat(c: *mut AMediaCodec) -> *mut AMediaFormat;
    pub fn AMediaCodec_releaseOutputBuffer(c: *mut AMediaCodec, idx: usize, render: bool) -> c_int;
    pub fn AMediaCodec_signalEndOfInputStream(c: *mut AMediaCodec) -> c_int;
    pub fn AMediaMuxer_new(fd: c_int, format: c_int) -> *mut AMediaMuxer;
    pub fn AMediaMuxer_setOrientationHint(m: *mut AMediaMuxer, deg: c_int) -> c_int;
    pub fn AMediaMuxer_addTrack(m: *mut AMediaMuxer, f: *const AMediaFormat) -> isize;
    pub fn AMediaMuxer_start(m: *mut AMediaMuxer) -> c_int;
    pub fn AMediaMuxer_writeSampleData(m: *mut AMediaMuxer, track: usize, data: *const u8, info: *const BufInfo) -> c_int;
    pub fn AMediaMuxer_stop(m: *mut AMediaMuxer) -> c_int;
    pub fn AMediaMuxer_delete(m: *mut AMediaMuxer) -> c_int;
    pub fn AMediaFormat_getInt32(f: *const AMediaFormat, name: *const c_char, out: *mut i32) -> bool;
    pub fn AMediaFormat_getInt64(f: *const AMediaFormat, name: *const c_char, out: *mut i64) -> bool;
    pub fn AMediaFormat_getString(f: *const AMediaFormat, name: *const c_char, out: *mut *const c_char) -> bool;
    pub fn AMediaCodec_createDecoderByType(t: *const c_char) -> *mut AMediaCodec;
    pub fn AMediaCodec_getInputFormat(c: *mut AMediaCodec) -> *mut AMediaFormat;
    pub fn AMediaExtractor_new() -> *mut AMediaExtractor;
    pub fn AMediaExtractor_delete(e: *mut AMediaExtractor) -> c_int;
    pub fn AMediaExtractor_setDataSource(e: *mut AMediaExtractor, loc: *const c_char) -> c_int;
    pub fn AMediaExtractor_getTrackCount(e: *mut AMediaExtractor) -> usize;
    pub fn AMediaExtractor_getTrackFormat(e: *mut AMediaExtractor, idx: usize) -> *mut AMediaFormat;
    pub fn AMediaExtractor_selectTrack(e: *mut AMediaExtractor, idx: usize) -> c_int;
    pub fn AMediaExtractor_seekTo(e: *mut AMediaExtractor, us: i64, mode: c_int) -> c_int;
    pub fn AMediaExtractor_readSampleData(e: *mut AMediaExtractor, buf: *mut u8, cap: usize) -> isize;
    pub fn AMediaExtractor_getSampleTime(e: *mut AMediaExtractor) -> i64;
    pub fn AMediaExtractor_getSampleFlags(e: *mut AMediaExtractor) -> u32;
    pub fn AMediaExtractor_advance(e: *mut AMediaExtractor) -> bool;
}

#[link(name = "aaudio")]
extern "C" {
    pub fn AAudio_createStreamBuilder(b: *mut *mut AAudioStreamBuilder) -> c_int;
    pub fn AAudioStreamBuilder_setDirection(b: *mut AAudioStreamBuilder, v: c_int);
    pub fn AAudioStreamBuilder_setSampleRate(b: *mut AAudioStreamBuilder, v: i32);
    pub fn AAudioStreamBuilder_setChannelCount(b: *mut AAudioStreamBuilder, v: i32);
    pub fn AAudioStreamBuilder_setFormat(b: *mut AAudioStreamBuilder, v: c_int);
    pub fn AAudioStreamBuilder_setInputPreset(b: *mut AAudioStreamBuilder, v: c_int);
    pub fn AAudioStreamBuilder_openStream(b: *mut AAudioStreamBuilder, s: *mut *mut AAudioStream) -> c_int;
    pub fn AAudioStreamBuilder_delete(b: *mut AAudioStreamBuilder) -> c_int;
    pub fn AAudioStream_requestStart(s: *mut AAudioStream) -> c_int;
    pub fn AAudioStream_requestStop(s: *mut AAudioStream) -> c_int;
    pub fn AAudioStream_close(s: *mut AAudioStream) -> c_int;
    pub fn AAudioStream_read(s: *mut AAudioStream, buf: *mut c_void, frames: i32, timeout_ns: i64) -> c_int;
}

pub const COLOR_FORMAT_SURFACE: i32 = 0x7F000789;
pub const FLAG_CONFIG: u32 = 2;
pub const FLAG_EOS: u32 = 4;
const SAMPLE_RATE: i32 = 44100;

pub fn boottime_us() -> i64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe {
        libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts);
    }
    ts.tv_sec as i64 * 1_000_000 + ts.tv_nsec as i64 / 1000
}

pub fn cs(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

/// Bộ mã hóa video lấy hình từ surface của camera.
pub struct Encoder {
    pub codec: *mut AMediaCodec,
    pub window: *mut ANativeWindow,
}

unsafe impl Send for Encoder {}

impl Encoder {
    pub fn new(w: i32, h: i32, fps: i32, bitrate: i32, hevc: bool) -> Result<Encoder, String> {
        unsafe {
            let mime = if hevc { "video/hevc" } else { "video/avc" };
            let codec = AMediaCodec_createEncoderByType(cs(mime).as_ptr());
            if codec.is_null() {
                return Err(format!("Máy không có bộ mã hóa {}", mime));
            }
            let f = AMediaFormat_new();
            AMediaFormat_setString(f, cs("mime").as_ptr(), cs(mime).as_ptr());
            AMediaFormat_setInt32(f, cs("width").as_ptr(), w);
            AMediaFormat_setInt32(f, cs("height").as_ptr(), h);
            AMediaFormat_setInt32(f, cs("bitrate").as_ptr(), bitrate);
            AMediaFormat_setInt32(f, cs("frame-rate").as_ptr(), fps);
            AMediaFormat_setInt32(f, cs("i-frame-interval").as_ptr(), 1);
            AMediaFormat_setInt32(f, cs("color-format").as_ptr(), COLOR_FORMAT_SURFACE);
            let st = AMediaCodec_configure(codec, f, null_mut(), null_mut(), 1);
            AMediaFormat_delete(f);
            if st != 0 {
                AMediaCodec_delete(codec);
                return Err(format!("Cấu hình bộ mã hóa lỗi ({}x{} {}fps, mã {})", w, h, fps, st));
            }
            let mut win: *mut ANativeWindow = null_mut();
            if AMediaCodec_createInputSurface(codec, &mut win) != 0 || win.is_null() {
                AMediaCodec_delete(codec);
                return Err("Không tạo được surface cho bộ mã hóa".to_string());
            }
            Ok(Encoder { codec, window: win })
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            if !self.codec.is_null() {
                AMediaCodec_delete(self.codec);
            }
        }
    }
}

struct Audio {
    codec: *mut AMediaCodec,
    stream: *mut AAudioStream,
    frames: i64,
    base_us: i64,
    eos_sent: bool,
}

unsafe fn audio_new() -> Option<Audio> {
    let mut b: *mut AAudioStreamBuilder = null_mut();
    if AAudio_createStreamBuilder(&mut b) != 0 || b.is_null() {
        return None;
    }
    AAudioStreamBuilder_setDirection(b, 1);
    AAudioStreamBuilder_setSampleRate(b, SAMPLE_RATE);
    AAudioStreamBuilder_setChannelCount(b, 1);
    AAudioStreamBuilder_setFormat(b, 1);
    AAudioStreamBuilder_setInputPreset(b, 5);
    let mut s: *mut AAudioStream = null_mut();
    let st = AAudioStreamBuilder_openStream(b, &mut s);
    AAudioStreamBuilder_delete(b);
    if st != 0 || s.is_null() {
        return None;
    }
    let codec = AMediaCodec_createEncoderByType(cs("audio/mp4a-latm").as_ptr());
    if codec.is_null() {
        AAudioStream_close(s);
        return None;
    }
    let f = AMediaFormat_new();
    AMediaFormat_setString(f, cs("mime").as_ptr(), cs("audio/mp4a-latm").as_ptr());
    AMediaFormat_setInt32(f, cs("sample-rate").as_ptr(), SAMPLE_RATE);
    AMediaFormat_setInt32(f, cs("channel-count").as_ptr(), 1);
    AMediaFormat_setInt32(f, cs("bitrate").as_ptr(), 128_000);
    AMediaFormat_setInt32(f, cs("aac-profile").as_ptr(), 2);
    AMediaFormat_setInt32(f, cs("max-input-size").as_ptr(), 16384);
    let st = AMediaCodec_configure(codec, f, null_mut(), null_mut(), 1);
    AMediaFormat_delete(f);
    if st != 0 || AMediaCodec_start(codec) != 0 {
        AMediaCodec_delete(codec);
        AAudioStream_close(s);
        return None;
    }
    let base_us = boottime_us();
    if AAudioStream_requestStart(s) != 0 {
        AMediaCodec_stop(codec);
        AMediaCodec_delete(codec);
        AAudioStream_close(s);
        return None;
    }
    Some(Audio { codec, stream: s, frames: 0, base_us, eos_sent: false })
}

unsafe fn audio_pump(a: &mut Audio, stopping: bool) {
    if a.eos_sent {
        return;
    }
    if stopping {
        let idx = AMediaCodec_dequeueInputBuffer(a.codec, 100_000);
        if idx >= 0 {
            let pts = a.base_us + a.frames * 1_000_000 / SAMPLE_RATE as i64;
            AMediaCodec_queueInputBuffer(a.codec, idx as usize, 0, 0, pts as u64, FLAG_EOS);
            a.eos_sent = true;
        }
        return;
    }
    let mut pcm = [0i16; 1024];
    let n = AAudioStream_read(a.stream, pcm.as_mut_ptr() as *mut c_void, 1024, 0);
    if n <= 0 {
        return;
    }
    let idx = AMediaCodec_dequeueInputBuffer(a.codec, 5000);
    if idx < 0 {
        return;
    }
    let mut cap = 0usize;
    let p = AMediaCodec_getInputBuffer(a.codec, idx as usize, &mut cap);
    let bytes = n as usize * 2;
    if p.is_null() || cap < bytes {
        AMediaCodec_queueInputBuffer(a.codec, idx as usize, 0, 0, 0, 0);
        return;
    }
    std::ptr::copy_nonoverlapping(pcm.as_ptr() as *const u8, p, bytes);
    let pts = a.base_us + a.frames * 1_000_000 / SAMPLE_RATE as i64;
    AMediaCodec_queueInputBuffer(a.codec, idx as usize, 0, bytes, pts as u64, 0);
    a.frames += n as i64;
}

unsafe fn audio_free(a: &mut Audio) {
    AAudioStream_requestStop(a.stream);
    AAudioStream_close(a.stream);
    AMediaCodec_stop(a.codec);
    AMediaCodec_delete(a.codec);
}

/// Lấy dữ liệu đã mã hóa ra ghi vào muxer. Trả về true nếu đã nhận EOS.
unsafe fn drain(codec: *mut AMediaCodec, muxer: *mut AMediaMuxer, track: &mut isize, started: bool, base: &mut Option<i64>, last: &mut i64) -> bool {
    if *track >= 0 && !started {
        return false;
    }
    let mut eos = false;
    for _ in 0..16 {
        let mut info = BufInfo { offset: 0, size: 0, pts: 0, flags: 0 };
        let idx = AMediaCodec_dequeueOutputBuffer(codec, &mut info, 0);
        if idx == -1 {
            break;
        }
        if idx == -2 {
            let f = AMediaCodec_getOutputFormat(codec);
            if !f.is_null() {
                *track = AMediaMuxer_addTrack(muxer, f);
                AMediaFormat_delete(f);
            }
            break;
        }
        if idx < 0 {
            continue;
        }
        if started && *track >= 0 && info.size > 0 && (info.flags & FLAG_CONFIG) == 0 {
            let mut sz = 0usize;
            let p = AMediaCodec_getOutputBuffer(codec, idx as usize, &mut sz);
            if !p.is_null() {
                // chuẩn hoá mốc thời gian về 0 (camera trả mốc tuyệt đối theo giờ máy → trình phát hiện sai thời lượng)
                if base.is_none() {
                    *base = Some(info.pts);
                }
                let mut inf = info;
                inf.pts = (info.pts - base.unwrap_or(0)).max(0);
                if inf.pts <= *last {
                    inf.pts = *last + 1;
                }
                *last = inf.pts;
                AMediaMuxer_writeSampleData(muxer, *track as usize, p, &inf);
            }
        }
        if (info.flags & FLAG_EOS) != 0 {
            eos = true;
        }
        AMediaCodec_releaseOutputBuffer(codec, idx as usize, false);
        if eos {
            break;
        }
    }
    eos
}

pub struct Recorder {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<(), String>>>,
    pub path: PathBuf,
    pub t0: Instant,
}

fn run(enc: Encoder, path: PathBuf, audio: bool, orient: i32, stop: Arc<AtomicBool>) -> Result<(), String> {
    unsafe {
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path).map_err(|e| e.to_string())?;
        let muxer = AMediaMuxer_new(file.as_raw_fd(), 0);
        if muxer.is_null() {
            return Err("Không tạo được muxer".to_string());
        }
        AMediaMuxer_setOrientationHint(muxer, orient);
        if AMediaCodec_start(enc.codec) != 0 {
            AMediaMuxer_delete(muxer);
            return Err("Không chạy được bộ mã hóa video".to_string());
        }
        let mut aud = if audio { audio_new() } else { None };
        let mut vbase: Option<i64> = None;
        let mut abase: Option<i64> = aud.as_ref().map(|a| a.base_us);
        let (mut vlast, mut alast) = (-1i64, -1i64);
        let mut vtrack: isize = -1;
        let mut atrack: isize = -1;
        let mut started = false;
        let mut v_done = false;
        let mut a_done = aud.is_none();
        let t0 = Instant::now();
        let mut stop_at: Option<Instant> = None;
        loop {
            let stopping = stop.load(Ordering::SeqCst);
            if stopping && stop_at.is_none() {
                stop_at = Some(Instant::now());
                AMediaCodec_signalEndOfInputStream(enc.codec);
            }
            if let Some(a) = aud.as_mut() {
                audio_pump(a, stopping);
            }
            if !v_done {
                v_done = drain(enc.codec, muxer, &mut vtrack, started, &mut vbase, &mut vlast);
            }
            if let Some(a) = aud.as_ref() {
                if !a_done {
                    a_done = drain(a.codec, muxer, &mut atrack, started, &mut abase, &mut alast);
                }
            }
            if !started && vtrack >= 0 {
                if aud.is_none() || atrack >= 0 {
                    started = AMediaMuxer_start(muxer) == 0;
                } else if t0.elapsed() > Duration::from_secs(2) {
                    // âm thanh không ra được định dạng → bỏ tiếng, vẫn ghi hình
                    if let Some(mut a) = aud.take() {
                        audio_free(&mut a);
                    }
                    a_done = true;
                    started = AMediaMuxer_start(muxer) == 0;
                }
            }
            if v_done && a_done {
                break;
            }
            if let Some(s) = stop_at {
                if s.elapsed() > Duration::from_secs(4) {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        if started {
            AMediaMuxer_stop(muxer);
        }
        AMediaMuxer_delete(muxer);
        if let Some(mut a) = aud.take() {
            audio_free(&mut a);
        }
        AMediaCodec_stop(enc.codec);
        drop(enc);
        drop(file);
        if !started {
            return Err("Video rỗng (bộ mã hóa không trả dữ liệu)".to_string());
        }
        Ok(())
    }
}

impl Recorder {
    pub fn start(enc: Encoder, path: PathBuf, audio: bool, orient: i32) -> Result<Recorder, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        let p = path.clone();
        let handle = std::thread::Builder::new()
            .name("csan-rec".to_string())
            .spawn(move || run(enc, p, audio, orient, st))
            .map_err(|e| e.to_string())?;
        Ok(Recorder { stop, handle: Some(handle), path, t0: Instant::now() })
    }

    pub fn stop(mut self) -> Result<PathBuf, String> {
        self.stop.store(true, Ordering::SeqCst);
        let r = match self.handle.take() {
            Some(h) => h.join().unwrap_or_else(|_| Err("Luồng quay bị lỗi".to_string())),
            None => Ok(()),
        };
        r.map(|_| self.path.clone())
    }
}

// ====================================================================
// Live: bộ mã hóa chạy liên tục, giữ ~6 giây hình + tiếng gần nhất trong RAM (ring buffer)
// ====================================================================
struct LFrame {
    pts: i64,
    flags: u32,
    data: Vec<u8>,
}

pub type ClipResult = Result<(Vec<u8>, f32), String>;

pub struct Live {
    stop: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<()>,
    handle: Option<JoinHandle<()>>,
    clips: Arc<std::sync::Mutex<Vec<ClipResult>>>,
}

#[allow(clippy::too_many_arguments)]
unsafe fn build_clip(
    ring: &[LFrame],
    fmt: *mut AMediaFormat,
    aring: &[LFrame],
    afmt: *mut AMediaFormat,
    delta: i64,
    t: i64,
    dir: &PathBuf,
    orient: i32,
) -> ClipResult {
    if fmt.is_null() || ring.is_empty() {
        return Err("Không có dữ liệu Live".to_string());
    }
    let lo = t - 1_500_000;
    let hi = t + 1_500_000;
    let mut start: Option<usize> = None;
    for (i, f) in ring.iter().enumerate() {
        if (f.flags & 1) != 0 && f.pts <= lo {
            start = Some(i);
        }
    }
    if start.is_none() {
        start = ring.iter().position(|f| (f.flags & 1) != 0);
    }
    let start = start.ok_or_else(|| "Không có keyframe".to_string())?;
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join(format!("clip_{}.mp4", boottime_us()));
    let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path).map_err(|e| e.to_string())?;
    let m = AMediaMuxer_new(file.as_raw_fd(), 0);
    if m.is_null() {
        let _ = std::fs::remove_file(&path);
        return Err("Không tạo được muxer".to_string());
    }
    AMediaMuxer_setOrientationHint(m, orient);
    let tr = AMediaMuxer_addTrack(m, fmt);
    let atr: isize = if !afmt.is_null() && !aring.is_empty() { AMediaMuxer_addTrack(m, afmt) } else { -1 };
    if tr < 0 || AMediaMuxer_start(m) != 0 {
        AMediaMuxer_delete(m);
        let _ = std::fs::remove_file(&path);
        return Err("Muxer không chạy được".to_string());
    }
    let base = ring[start].pts;
    let mut cnt = 0u32;
    let mut last = base;
    for f in &ring[start..] {
        if f.pts > hi {
            break;
        }
        let info = BufInfo { offset: 0, size: f.data.len() as i32, pts: f.pts - base, flags: f.flags & 1 };
        AMediaMuxer_writeSampleData(m, tr as usize, f.data.as_ptr(), &info);
        cnt += 1;
        last = f.pts;
    }
    if atr >= 0 {
        // quy mốc thời gian âm thanh sang mốc của hình bằng độ lệch đo được lúc bấm chụp
        let mut alast = -1i64;
        for f in aring {
            let p = f.pts + delta - base;
            if p < 0 {
                continue;
            }
            if p > hi - base {
                break;
            }
            let pts = if p <= alast { alast + 1 } else { p };
            alast = pts;
            let info = BufInfo { offset: 0, size: f.data.len() as i32, pts, flags: 0 };
            AMediaMuxer_writeSampleData(m, atr as usize, f.data.as_ptr(), &info);
        }
    }
    let fps = if cnt > 1 && last > base { (cnt - 1) as f32 * 1_000_000.0 / (last - base) as f32 } else { 0.0 };
    AMediaMuxer_stop(m);
    AMediaMuxer_delete(m);
    drop(file);
    let bytes = std::fs::read(&path).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&path);
    bytes.map(|b| (b, fps))
}

fn live_run(
    enc: Encoder,
    rx: std::sync::mpsc::Receiver<()>,
    out: Arc<std::sync::Mutex<Vec<ClipResult>>>,
    stop: Arc<AtomicBool>,
    dir: PathBuf,
    orient: i32,
    audio: bool,
) {
    unsafe {
        if AMediaCodec_start(enc.codec) != 0 {
            if let Ok(mut o) = out.lock() {
                o.push(Err("Không chạy được bộ mã hóa Live".to_string()));
            }
            return;
        }
        let mut aud = if audio { audio_new() } else { None };
        let mut ring: Vec<LFrame> = Vec::new();
        let mut aring: Vec<LFrame> = Vec::new();
        let mut fmt: *mut AMediaFormat = null_mut();
        let mut afmt: *mut AMediaFormat = null_mut();
        // (pts hình mới nhất, pts tiếng mới nhất, lúc bấm)
        let mut pend: Vec<(i64, i64, Instant)> = Vec::new();
        while !stop.load(Ordering::SeqCst) {
            for _ in 0..32 {
                let mut info = BufInfo { offset: 0, size: 0, pts: 0, flags: 0 };
                let idx = AMediaCodec_dequeueOutputBuffer(enc.codec, &mut info, 0);
                if idx == -1 {
                    break;
                }
                if idx == -2 {
                    if !fmt.is_null() {
                        AMediaFormat_delete(fmt);
                    }
                    fmt = AMediaCodec_getOutputFormat(enc.codec);
                    continue;
                }
                if idx < 0 {
                    continue;
                }
                if info.size > 0 && (info.flags & FLAG_CONFIG) == 0 {
                    let mut sz = 0usize;
                    let p = AMediaCodec_getOutputBuffer(enc.codec, idx as usize, &mut sz);
                    if !p.is_null() {
                        let data = std::slice::from_raw_parts(p.add(info.offset as usize), info.size as usize).to_vec();
                        ring.push(LFrame { pts: info.pts, flags: info.flags, data });
                    }
                }
                AMediaCodec_releaseOutputBuffer(enc.codec, idx as usize, false);
            }
            if let Some(a) = aud.as_mut() {
                audio_pump(a, false);
                for _ in 0..32 {
                    let mut info = BufInfo { offset: 0, size: 0, pts: 0, flags: 0 };
                    let idx = AMediaCodec_dequeueOutputBuffer(a.codec, &mut info, 0);
                    if idx == -1 {
                        break;
                    }
                    if idx == -2 {
                        if !afmt.is_null() {
                            AMediaFormat_delete(afmt);
                        }
                        afmt = AMediaCodec_getOutputFormat(a.codec);
                        continue;
                    }
                    if idx < 0 {
                        continue;
                    }
                    if info.size > 0 && (info.flags & FLAG_CONFIG) == 0 {
                        let mut sz = 0usize;
                        let p = AMediaCodec_getOutputBuffer(a.codec, idx as usize, &mut sz);
                        if !p.is_null() {
                            let data = std::slice::from_raw_parts(p.add(info.offset as usize), info.size as usize).to_vec();
                            aring.push(LFrame { pts: info.pts, flags: 0, data });
                        }
                    }
                    AMediaCodec_releaseOutputBuffer(a.codec, idx as usize, false);
                }
            }
            let newest = ring.last().map(|f| f.pts).unwrap_or(0);
            while ring.len() > 2 && newest - ring[0].pts > 6_000_000 {
                ring.remove(0);
            }
            let anewest = aring.last().map(|f| f.pts).unwrap_or(0);
            while aring.len() > 2 && anewest - aring[0].pts > 7_000_000 {
                aring.remove(0);
            }
            while rx.try_recv().is_ok() {
                pend.push((newest, anewest, Instant::now()));
            }
            let mut i = 0;
            while i < pend.len() {
                let (t, at, w) = pend[i];
                if newest >= t + 1_500_000 || w.elapsed() > Duration::from_millis(3000) {
                    let delta = if at != 0 && t != 0 { t - at } else { 0 };
                    let r = build_clip(&ring, fmt, &aring, afmt, delta, t, &dir, orient);
                    if let Ok(mut o) = out.lock() {
                        o.push(r);
                    }
                    pend.remove(i);
                } else {
                    i += 1;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if !fmt.is_null() {
            AMediaFormat_delete(fmt);
        }
        if !afmt.is_null() {
            AMediaFormat_delete(afmt);
        }
        if let Some(mut a) = aud.take() {
            audio_free(&mut a);
        }
        AMediaCodec_stop(enc.codec);
    }
}

impl Live {
    pub fn start(enc: Encoder, dir: PathBuf, orient: i32, audio: bool) -> Live {
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let clips = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (s2, c2) = (stop.clone(), clips.clone());
        let handle = std::thread::Builder::new().name("csan-live".to_string()).spawn(move || live_run(enc, rx, c2, s2, dir, orient, audio)).ok();
        Live { stop, tx, handle, clips }
    }
    pub fn mark(&self) {
        let _ = self.tx.send(());
    }
    pub fn take(&self) -> Vec<ClipResult> {
        match self.clips.lock() {
            Ok(mut v) => std::mem::take(&mut *v),
            Err(_) => Vec::new(),
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
