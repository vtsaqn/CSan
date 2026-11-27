mod cam;
mod cfg;
mod exif;
mod export;
mod hdr;
mod heic;
mod level;
mod media;
mod motion;
mod perm;
mod photo;
mod rtmp;
mod server;
mod stream;
mod tik;
mod video;

use android_activity::{AndroidApp, WindowManagerFlags};
use winit::platform::android::EventLoopBuilderExtAndroid;
use cfg::Cfg;
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CAM_PERM: &str = "android.permission.CAMERA";
const MIC_PERM: &str = "android.permission.RECORD_AUDIO";

#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("csan"));
    let app2 = app.clone();
    let options = eframe::NativeOptions {
        event_loop_builder: Some(Box::new(move |builder| {
            builder.with_android_app(app2);
        })),
        ..Default::default()
    };
    let r = eframe::run_native("CSar", options, Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(app)))
        }));
    if let Err(e) = r {
        log::error!("run_native: {}", e);
    }
}

/// Font DejaVu có đủ dấu tiếng Việt (font mặc định của egui thiếu nên hiện ô vuông).
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("vn".to_owned(), egui::FontData::from_static(include_bytes!("../assets/DejaVuSans.ttf")).into());
    if let Some(v) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        v.insert(0, "vn".to_owned());
    }
    if let Some(v) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        v.push("vn".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// Toàn bộ cài đặt (lưu trong config.txt, cùng tên khoá với app Java khi có).
#[derive(Clone)]
struct Set {
    url: String,
    tok: String,
    pden: i32,
    psharp: i32,
    send: bool,
    ifmt: i32,
    iq: i32,
    hdr: i32,
    night: i32,
    eis: i32,
    grid: bool,
    vc: i32,
    vres: i32,
    vbr: i32,
    audio: bool,
    ratio: i32,
    vlow: i32,
    stab: bool,
    stabscale: i32,
    stabext: bool,
    flash: i32,
    tm: i32,
    iqh: i32,
    level: bool,
    lc: i32,
    tikd: i32,
    tiks: i32,
    prot: i32,
    exif: bool,
    pres: i32,
    sturl: String,
    stkey: String,
    stc: i32,
    stbr: i32,
    stfps: i32,
    stres: i32,
    stki: i32,
    stabr: i32,
    strat: i32,
    stflip: bool,
    mode: i32,
    front: bool,
    ss_on: bool,
    ss_str: i32,
    ss_crop: i32,
    ss_smooth: i32,
    ss_port: String,
}

impl Set {
    fn load(c: &Cfg) -> Set {
        Set {
            url: c.str("stabsrv_url", ""),
            tok: c.str("stabsrv_tok", ""),
            pden: c.int("pden", 40) as i32,
            psharp: c.int("psharp", 25) as i32,
            send: c.boolean("send", false),
            ifmt: c.int("ifmt", 0) as i32,
            iq: c.int("iq", 95) as i32,
            hdr: c.int("hdr", 0) as i32,
            night: c.int("night", 0) as i32,
            eis: c.int("eis", 0) as i32,
            grid: c.boolean("grid", false),
            vc: c.int("vc", 0) as i32,
            vres: c.int("vres", 0) as i32,
            vbr: c.int("vbr", 16000) as i32,
            audio: c.boolean("audio", true),
            ratio: c.int("ratio", 0) as i32,
            vlow: c.int("vlow", 0) as i32,
            stab: c.boolean("stabsrv", false),
            stabscale: c.int("stabscale", 95) as i32,
            stabext: c.boolean("stabext", true),
            flash: c.int("flash", 0) as i32,
            tm: c.int("tm", 0) as i32,
            iqh: c.int("iqh", 70) as i32,
            level: c.boolean("level", false),
            lc: c.int("lc", 0) as i32,
            tikd: c.int("tikd", 1) as i32,
            tiks: c.int("tiks", 2) as i32,
            prot: c.int("prot", 0) as i32,
            exif: c.boolean("exif", true),
            pres: c.int("pres", 0) as i32,
            sturl: c.str("sturl", ""),
            stkey: c.str("stkey", ""),
            stc: c.int("stc", 0) as i32,
            stbr: c.int("stbr", 4000) as i32,
            stfps: c.int("stfps", 30) as i32,
            stres: c.int("stres", 1) as i32,
            stki: c.int("stki", 1) as i32,
            stabr: c.int("stabr", 128) as i32,
            // tỉ lệ stream: bản cũ lưu "stori" (0 dọc, 1 ngang, 2 ngang ngược) → đổi sang tỉ lệ + xoay 180°
            strat: if c.map.contains_key("strat") { (c.int("strat", 1) as i32).clamp(0, 6) } else if matches!(c.int("stori", 0), 1 | 2) { 0 } else { 1 },
            stflip: if c.map.contains_key("stflip") { c.boolean("stflip", false) } else { c.int("stori", 0) == 2 },
            mode: (c.int("mode", 0) as i32).clamp(0, 4),
            front: c.boolean("front", false),
            ss_on: c.boolean("ss_on", false),
            ss_str: c.int("ss_str", 50) as i32,
            ss_crop: c.int("ss_crop", 95) as i32,
            ss_smooth: c.int("ss_smooth", 20) as i32,
            ss_port: c.str("ss_port", "7788"),
        }
    }
    fn store(&self, c: &mut Cfg) {
        c.set("stabsrv_url", self.url.trim().to_string());
        c.set("stabsrv_tok", self.tok.trim().to_string());
        let b = |v: bool| (if v { "1" } else { "0" }).to_string();
        c.set("pden", self.pden.to_string());
        c.set("psharp", self.psharp.to_string());
        c.set("send", b(self.send));
        c.set("ifmt", self.ifmt.to_string());
        c.set("iq", self.iq.to_string());
        c.set("hdr", self.hdr.to_string());
        c.set("night", self.night.to_string());
        c.set("eis", self.eis.to_string());
        c.set("grid", b(self.grid));
        c.set("vc", self.vc.to_string());
        c.set("vres", self.vres.to_string());
        c.set("vbr", self.vbr.to_string());
        c.set("audio", b(self.audio));
        c.set("ratio", self.ratio.to_string());
        c.set("vlow", self.vlow.to_string());
        c.set("stabsrv", b(self.stab));
        c.set("stabscale", self.stabscale.to_string());
        c.set("stabext", b(self.stabext));
        c.set("flash", self.flash.to_string());
        c.set("tm", self.tm.to_string());
        c.set("iqh", self.iqh.to_string());
        c.set("level", b(self.level));
        c.set("lc", self.lc.to_string());
        c.set("tikd", self.tikd.to_string());
        c.set("tiks", self.tiks.to_string());
        c.set("prot", self.prot.to_string());
        c.set("exif", b(self.exif));
        c.set("pres", self.pres.to_string());
        c.set("sturl", self.sturl.trim().to_string());
        c.set("stkey", self.stkey.trim().to_string());
        c.set("stc", self.stc.to_string());
        c.set("stbr", self.stbr.to_string());
        c.set("stfps", self.stfps.to_string());
        c.set("stres", self.stres.to_string());
        c.set("stki", self.stki.to_string());
        c.set("stabr", self.stabr.to_string());
        c.set("strat", self.strat.to_string());
        c.set("stflip", b(self.stflip));
        c.set("mode", self.mode.to_string());
        c.set("front", b(self.front));
        c.set("ss_on", b(self.ss_on));
        c.set("ss_str", self.ss_str.to_string());
        c.set("ss_crop", self.ss_crop.to_string());
        c.set("ss_smooth", self.ss_smooth.to_string());
        c.set("ss_port", self.ss_port.trim().to_string());
    }
}

const SRATIOS: [(i32, i32); 7] = [(16, 9), (9, 16), (4, 3), (3, 4), (1, 1), (3, 2), (2, 3)];
const SRATIO_NAMES: [&str; 7] = ["16:9", "9:16", "4:3", "3:4", "1:1", "3:2", "2:3"];
const TIMERS: [u64; 3] = [0, 3, 10];
const TIKD: [i64; 3] = [15, 30, 60];
const SPD: [f32; 5] = [3.0, 2.0, 1.0, 0.5, 0.25];

fn base_name(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Đưa tệp vào DCIM/Csan và báo kết quả qua `set`.
fn publish_and_report(j: export::Jvm, p: &std::path::Path, label: &str, set: &dyn Fn(&str)) {
    match export::publish(j, p, true) {
        Ok(n) => set(&format!("Đã lưu {} → DCIM/Csan/{}", label, n)),
        Err(e) => set(&format!("Đã lưu {} trong thư mục app ({}); chưa vào thư viện ảnh: {}", label, base_name(p), e)),
    }
}

fn short(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    if c.len() <= n {
        s.to_string()
    } else {
        let mut o: String = c[..n].iter().collect();
        o.push('…');
        o
    }
}

fn chip(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response {
    let fill = if selected { egui::Color32::from_rgb(0, 110, 170) } else { egui::Color32::from_black_alpha(120) };
    ui.add(egui::Button::new(egui::RichText::new(text).color(egui::Color32::WHITE)).fill(fill).rounding(10.0))
}

fn fmt_clock(ms: i64) -> String {
    let s = (ms / 1000).max(0);
    format!("{}:{:02}", s / 60, s % 60)
}

struct LivePend {
    jpeg: Option<Vec<u8>>,
    clip: Option<Result<(Vec<u8>, f32, i64), String>>,
    t: Instant,
}

#[derive(Clone)]
struct TikJob {
    speed: f32,
    music: Option<String>,
    start_us: i64,
    seg_us: i64,
}

struct Trim {
    track: media::Track,
    start_ms: i64,
    seg_ms: i64,
    player: Option<media::Player>,
}

struct App {
    droid: AndroidApp,
    dir: PathBuf,
    cfg: Cfg,
    s: Set,
    cam: Option<cam::Camera>,
    front: bool,
    mode: u8,
    open_err: Option<String>,
    perm_asked: bool,
    last_check: Instant,
    last_sync: Instant,
    tex: Option<egui::TextureHandle>,
    last_seq: u64,
    status: Arc<Mutex<String>>,
    settings_open: bool,
    hdr_buf: Option<(Vec<Vec<u8>>, Instant)>,
    pending_shot: Option<Instant>,
    torch_off_at: Option<Instant>,
    user_torch: bool,
    timer_end: Option<Instant>,
    focus_ring: Option<(egui::Pos2, Instant)>,
    level: Option<level::Level>,
    compat: bool,
    live_pend: Option<LivePend>,
    tracks: Vec<media::Track>,
    zoom_t: f32,
    zoom_c: f32,
    device: (String, String),
    hevc_ok: bool,
    br_txt: String,
    st_last_bytes: u64,
    st_last_t: Instant,
    st_kbps: u64,
    top_y: f32,
    bottom_nat: f32,
    last_gap: f32,
    more: bool,
    last_status: String,
    status_at: Instant,
    music_open: bool,
    music_msg: String,
    filter: String,
    sel: Option<media::Track>,
    tik_start_ms: i64,
    tik_seg_ms: i64,
    player: Option<media::Player>,
    tik_job: Option<TikJob>,
    tik_limit: Option<Duration>,
    trim: Option<Trim>,
    /// đổi camera lúc đang quay VIDEO: lưu clip rồi tự quay tiếp bằng camera kia ngay khi mở xong
    resume_rec: bool,
    started: Instant,
    awake_set: bool,
}

impl App {
    fn new(droid: AndroidApp) -> App {
        let dir: PathBuf = droid
            .external_data_path()
            .map(|p| PathBuf::from(p.as_path()))
            .or_else(|| droid.internal_data_path().map(|p| PathBuf::from(p.as_path())))
            .unwrap_or_else(|| PathBuf::from("."));
        let cfg = Cfg::load(&dir);
        let s = Set::load(&cfg);
        let mode0 = s.mode.clamp(0, 4) as u8;
        let front0 = s.front;
        let device = media::device_names(&droid);
        App {
            droid,
            dir,
            cfg,
            br_txt: s.stbr.to_string(),
            s,
            cam: None,
            front: front0,
            mode: mode0,
            open_err: None,
            perm_asked: false,
            last_check: Instant::now() - Duration::from_secs(10),
            last_sync: Instant::now(),
            tex: None,
            last_seq: 0,
            status: Arc::new(Mutex::new(String::new())),
            settings_open: false,
            hdr_buf: None,
            pending_shot: None,
            torch_off_at: None,
            user_torch: false,
            timer_end: None,
            focus_ring: None,
            level: None,
            compat: false,
            live_pend: None,
            tracks: Vec::new(),
            zoom_t: 1.0,
            zoom_c: 1.0,
            device,
            hevc_ok: stream::codec_supported(true),
            st_last_bytes: 0,
            st_last_t: Instant::now(),
            st_kbps: 0,
            top_y: 60.0,
            bottom_nat: 330.0,
            last_gap: 0.0,
            more: false,
            last_status: String::new(),
            status_at: Instant::now(),
            music_open: false,
            music_msg: String::new(),
            filter: String::new(),
            sel: None,
            tik_start_ms: 0,
            tik_seg_ms: 15000,
            player: None,
            tik_job: None,
            tik_limit: None,
            trim: None,
            resume_rec: false,
            started: Instant::now(),
            awake_set: false,
        }
    }

    fn is_video(&self) -> bool {
        self.mode >= 2
    }

    fn set_status(&self, m: &str) {
        if let Ok(mut g) = self.status.lock() {
            *g = m.to_string();
        }
    }

    fn save_cfg(&mut self) {
        self.s.store(&mut self.cfg);
        self.cfg.save();
    }

    fn reopen(&mut self) {
        self.zoom_t = 1.0;
        self.zoom_c = 1.0;
        self.cam = None;
        self.open_err = None;
        self.tex = None;
        self.hdr_buf = None;
        self.pending_shot = None;
        self.timer_end = None;
    }

    fn open_opts(&self) -> cam::OpenOpts {
        cam::OpenOpts {
            front: self.front,
            dir: self.dir.clone(),
            video: self.mode == 2 || self.mode == 3,
            wide: self.s.ratio == 1,
            vres: self.s.vres.clamp(0, 2) as u8,
            vhevc: self.s.vc == 1,
            vbr_kbps: self.s.vbr,
            vfps: 30,
            jpeg_q: self.s.iq.clamp(50, 100) as u8,
            live: self.mode == 1,
            live_hevc: self.s.lc == 1,
            compat: self.compat,
            rot_extra: self.s.prot.clamp(0, 3) * 90,
            pres: self.s.pres.clamp(0, 3) as u8,
            stream: if self.mode == 4 {
                Some(cam::StreamOpts {
                    res: self.s.stres,
                    ratio: SRATIOS[self.s.strat.clamp(0, 6) as usize],
                    flip: self.s.stflip,
                    fps: self.s.stfps,
                    vbr_kbps: self.s.stbr,
                    hevc: self.s.stc == 1 && self.hevc_ok,
                    iframe_s: [1, 2, 4][self.s.stki.clamp(0, 2) as usize],
                })
            } else {
                None
            },
            live_audio: self.mode == 1 && self.s.audio && perm::has(&self.droid, MIC_PERM),
        }
    }

    /// Đẩy cài đặt (EIS, ban đêm, fps) vào camera đang mở.
    fn sync_ctl(&mut self) {
        let s = self.s.clone();
        if let Some(c) = self.cam.as_mut() {
            let luma = c.sh.luma.load(Ordering::Relaxed);
            let night = !c.video && (s.night == 2 || (s.night == 1 && luma < 70));
            let eis = if c.video { s.eis as u8 } else { 0 };
            let low = c.video && s.vlow == 1;
            if night != c.ctl.night || eis != c.ctl.eis || low != c.ctl.low_fps {
                c.ctl.night = night;
                c.ctl.eis = eis;
                c.ctl.low_fps = low;
                let _ = c.apply();
            }
        }
    }

    fn meta_src(&self) -> Option<exif::MetaSrc> {
        self.cam.as_ref().map(|c| exif::MetaSrc {
            sh: c.sh.clone(),
            ev_step: c.info.ev_step,
            phys: c.info.phys,
            zoom: c.ctl.zoom,
            make: self.device.0.clone(),
            model: self.device.1.clone(),
        })
    }

    fn spawn_save(&self, jpegs: Vec<Vec<u8>>, is_hdr: bool) {
        let src = self.meta_src();
        let nmeta = if is_hdr { 3 } else { 1 };
        let jv = export::Jvm::from_app(&self.droid);
        let cfg = self.cfg.clone();
        let dir = self.dir.clone();
        let st = self.status.clone();
        let rot = self.cam.as_ref().map(|c| c.info.sensor_ori).unwrap_or(90);
        std::thread::spawn(move || {
            let set = |m: &str| {
                if let Ok(mut g) = st.lock() {
                    *g = m.to_string();
                }
            };
            let mut prog = |m: &str, p: i32| {
                if p >= 0 {
                    set(&format!("{} {}%", m, p));
                } else {
                    set(m);
                }
            };
            let meta = src.map(|s| s.gather(nmeta)).unwrap_or_default();
            let r = if is_hdr {
                photo::save_hdr(&cfg, &dir, jpegs, rot, &meta, &mut prog)
            } else {
                photo::save_jpeg(&cfg, &dir, jpegs.into_iter().next().unwrap_or_default(), rot, &meta, &mut prog)
            };
            match r {
                Ok(p) => publish_and_report(jv, &p, "ảnh", &set),
                Err(e) => set(&format!("Lỗi lưu ảnh: {}", e)),
            }
        });
    }

    fn spawn_live(&self, jpeg: Vec<u8>, clip: Option<(Vec<u8>, f32, i64)>) {
        let src = self.meta_src();
        let rot = self.cam.as_ref().map(|c| c.info.sensor_ori).unwrap_or(90);
        let jv = export::Jvm::from_app(&self.droid);
        let cfg = self.cfg.clone();
        let dir = self.dir.clone();
        let st = self.status.clone();
        std::thread::spawn(move || {
            let set = |m: &str| {
                if let Ok(mut g) = st.lock() {
                    *g = m.to_string();
                }
            };
            let mut prog = |m: &str, p: i32| {
                if p >= 0 {
                    set(&format!("{} {}%", m, p));
                } else {
                    set(m);
                }
            };
            let meta = src.map(|s| s.gather(1)).unwrap_or_default();
            match photo::save_live(&cfg, &dir, jpeg, clip, rot, &meta, &mut prog) {
                Ok(p) => publish_and_report(jv, &p, "ảnh Live", &set),
                Err(e) => set(&format!("Lỗi lưu Live: {}", e)),
            }
        });
    }

    fn spawn_tik(&self, path: PathBuf, job: TikJob) {
        let jv = export::Jvm::from_app(&self.droid);
        let st = self.status.clone();
        let orient = self.cam.as_ref().map(|c| c.info.sensor_ori).unwrap_or(90);
        std::thread::spawn(move || {
            let set = |m: &str| {
                if let Ok(mut g) = st.lock() {
                    *g = m.to_string();
                }
            };
            set("Đang xử lý video TikTok…");
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let dst = path.with_file_name(format!("{}_TT.mp4", stem));
            let music = job.music.as_ref().map(|m| PathBuf::from(m));
            match tik::run(&path, &dst, job.speed, music.as_deref(), job.start_us, job.seg_us, orient) {
                Ok(_) => {
                    let _ = std::fs::remove_file(&path);
                    publish_and_report(jv, &dst, "video TikTok", &set);
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&dst);
                    let msg = format!("Lỗi xử lý TikTok: {}", e);
                    let _ = std::fs::write(path.with_file_name("csan_log.txt"), &msg);
                    publish_and_report(jv, &path, "video gốc", &set);
                    set(&format!("{} (đã lưu video gốc)", msg));
                }
            }
        });
    }

    fn tik_begin(&mut self, sp: f32) {
        let mut fin = TIKD[self.s.tikd.clamp(0, 2) as usize] * 1000;
        if self.sel.is_some() {
            fin = fin.min(self.tik_seg_ms);
        }
        self.tik_limit = Some(Duration::from_millis((fin as f32 * sp) as u64));
        self.tik_job = if self.sel.is_some() || (sp - 1.0).abs() > 0.001 {
            Some(TikJob {
                speed: sp,
                music: self.sel.as_ref().map(|t| t.path.clone()),
                start_us: self.tik_start_ms * 1000,
                seg_us: self.tik_seg_ms * 1000,
            })
        } else {
            None
        };
        if let Some(t) = self.sel.clone() {
            match media::Player::open(&self.droid, &t.path) {
                Some(p) => {
                    p.start_at(self.tik_start_ms, 1.0 / sp);
                    self.player = Some(p);
                }
                None => self.set_status("Không phát được nhạc"),
            }
        }
    }

    fn load_tracks(&mut self) {
        let perms = ["android.permission.READ_MEDIA_AUDIO", "android.permission.READ_EXTERNAL_STORAGE"];
        if !perm::has(&self.droid, perms[0]) && !perm::has(&self.droid, perms[1]) {
            perm::request(&self.droid, &perms);
            self.set_status("Hãy cấp quyền đọc nhạc rồi bấm Làm mới");
        }
        let (t, msg) = media::list_tracks(&self.droid);
        self.tracks = t;
        if self.tracks.is_empty() {
            self.music_msg = format!("Không thấy bài nhạc nào. {}", msg);
            self.set_status(&self.music_msg.clone());
        } else {
            self.music_msg = format!("Tìm thấy {} bài", self.tracks.len());
        }
    }

    fn spawn_publish(&self, path: PathBuf) {
        let jv = export::Jvm::from_app(&self.droid);
        let st = self.status.clone();
        std::thread::spawn(move || {
            let set = |m: &str| {
                if let Ok(mut g) = st.lock() {
                    *g = m.to_string();
                }
            };
            publish_and_report(jv, &path, "video", &set);
        });
    }

    fn spawn_stab(&self, path: PathBuf) {
        let jv = export::Jvm::from_app(&self.droid);
        let cfg = self.cfg.clone();
        let st = self.status.clone();
        std::thread::spawn(move || {
            let set = |m: &str| {
                if let Ok(mut g) = st.lock() {
                    *g = m.to_string();
                }
            };
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    set(&format!("Đọc video lỗi: {}", e));
                    return;
                }
            };
            let mut prog = |m: &str, p: i32| {
                if p >= 0 {
                    set(&format!("{} {}%", m, p));
                } else {
                    set(m);
                }
            };
            match server::stab_process(&cfg, &bytes, 30.0, &mut prog) {
                Ok(b) => {
                    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let o = path.with_file_name(format!("{}_stab.mp4", stem));
                    match std::fs::write(&o, b) {
                        Ok(_) => {
                            let _ = std::fs::remove_file(&path);
                            publish_and_report(jv, &o, "video ổn định", &set);
                        }
                        Err(e) => {
                            set(&format!("Ghi video lỗi: {}", e));
                            publish_and_report(jv, &path, "video gốc", &set);
                        }
                    }
                }
                Err(e) => {
                    set(&format!("Server ổn định lỗi, giữ video gốc: {}", e));
                    publish_and_report(jv, &path, "video gốc", &set);
                }
            }
        });
    }

    fn tick(&mut self) {
        // giữ màn hình luôn sáng: đặt một lần, sau khi app đã chạy ổn định vài giây (không đặt lúc đang khởi tạo cửa sổ)
        if !self.awake_set && self.cam.is_some() && self.started.elapsed() > Duration::from_secs(3) {
            self.awake_set = true;
            self.droid.set_window_flags(WindowManagerFlags::KEEP_SCREEN_ON, WindowManagerFlags::empty());
        }
        // mở camera khi đủ quyền
        if self.cam.is_none() && self.open_err.is_none() && self.last_check.elapsed() > Duration::from_millis(500) {
            self.last_check = Instant::now();
            if perm::has(&self.droid, CAM_PERM) {
                match cam::Camera::open(self.open_opts()) {
                    Ok(c) => {
                        self.cam = Some(c);
                        self.sync_ctl();
                        if self.resume_rec {
                            self.resume_rec = false;
                            self.toggle_record();
                        }
                    }
                    Err(e) => {
                        if !self.compat {
                            self.compat = true;
                            self.set_status(&format!("Camera lỗi ({}) → thử chế độ tương thích", e));
                        } else {
                            self.resume_rec = false;
                            self.open_err = Some(e);
                        }
                    }
                }
            } else if !self.perm_asked {
                self.perm_asked = true;
                perm::request(&self.droid, &[CAM_PERM, MIC_PERM]);
            }
        }
        if self.cam.as_ref().map(|c| c.is_lost()).unwrap_or(false) {
            self.cam = None;
            self.open_err = Some("Camera bị ngắt".to_string());
        }
        if self.last_sync.elapsed() > Duration::from_millis(600) {
            self.last_sync = Instant::now();
            self.sync_ctl();
        }
        let now = Instant::now();

        // đèn flash trước khi chụp
        if let Some(t) = self.pending_shot {
            if now >= t {
                self.pending_shot = None;
                if let Some(c) = &self.cam {
                    if let Err(e) = c.shoot() {
                        self.set_status(&e);
                    }
                }
                self.torch_off_at = Some(now + Duration::from_millis(1500));
            }
        }
        if let Some(t) = self.torch_off_at {
            if now >= t {
                self.torch_off_at = None;
                if !self.user_torch {
                    if let Some(c) = self.cam.as_mut() {
                        c.ctl.torch = false;
                        let _ = c.apply();
                    }
                }
            }
        }

        // hẹn giờ
        if let Some(t) = self.timer_end {
            if now >= t {
                self.timer_end = None;
                self.capture();
            }
        }

        self.poll_stream();

        // thước thăng bằng (gia tốc kế)
        if self.s.level && self.level.is_none() {
            self.level = Some(level::Level::start("zanq.fun.csar"));
        } else if !self.s.level && self.level.is_some() {
            self.level = None;
        }
        // TikTok: tự dừng khi đủ thời lượng
        let el = self.cam.as_ref().and_then(|c| c.rec_elapsed());
        if let (Some(lim), Some(el)) = (self.tik_limit, el) {
            if el >= lim {
                self.toggle_record();
            }
        }

        // ảnh về từ camera
        let shots = self.cam.as_ref().map(|c| c.take_shots()).unwrap_or_default();
        for sh in shots {
            if self.hdr_buf.is_some() {
                let mut done: Option<Vec<Vec<u8>>> = None;
                if let Some((v, _)) = self.hdr_buf.as_mut() {
                    v.push(sh);
                    if v.len() >= 3 {
                        done = Some(std::mem::take(v));
                    }
                }
                if let Some(v) = done {
                    self.hdr_buf = None;
                    self.set_status("HDR: đang ghép ảnh…");
                    self.spawn_save(v, true);
                }
            } else if let Some(lp) = self.live_pend.as_mut() {
                if lp.jpeg.is_none() {
                    lp.jpeg = Some(sh);
                }
            } else {
                self.spawn_save(vec![sh], false);
            }
        }
        // Live: clip về từ bộ đệm vòng
        let clips = self.cam.as_ref().map(|c| c.take_clips()).unwrap_or_default();
        for r in clips {
            if let Some(lp) = self.live_pend.as_mut() {
                lp.clip = Some(r);
            }
        }
        let ready = match &self.live_pend {
            Some(lp) => (lp.jpeg.is_some() && lp.clip.is_some()) || (lp.jpeg.is_some() && lp.t.elapsed() > Duration::from_secs(8)),
            None => false,
        };
        if ready {
            if let Some(lp) = self.live_pend.take() {
                let clip = match lp.clip {
                    Some(Ok(c)) => Some(c),
                    Some(Err(e)) => {
                        self.set_status(&format!("Live: {}", e));
                        None
                    }
                    None => None,
                };
                self.spawn_live(lp.jpeg.unwrap_or_default(), clip);
            }
        }
        if self.live_pend.as_ref().map(|l| l.t.elapsed() > Duration::from_secs(15)).unwrap_or(false) {
            self.live_pend = None;
        }
        let expired = self.hdr_buf.as_ref().map(|(_, t)| t.elapsed() > Duration::from_secs(9)).unwrap_or(false);
        if expired {
            if let Some((v, _)) = self.hdr_buf.take() {
                if v.is_empty() {
                    self.set_status("HDR: không nhận được ảnh");
                } else {
                    self.set_status("HDR: thiếu ảnh, lưu ảnh thường");
                    let one = v[if v.len() > 1 { 1 } else { 0 }].clone();
                    self.spawn_save(vec![one], false);
                }
            }
        }
    }

    fn scene_hdr(&self) -> bool {
        match &self.cam {
            Some(c) => {
                let d = c.sh.dark_pm.load(Ordering::Relaxed) as f32 / 1000.0;
                let b = c.sh.bright_pm.load(Ordering::Relaxed) as f32 / 1000.0;
                (d >= 0.12 && b >= 0.05) || (b >= 0.20 && d >= 0.03) || (d >= 0.35 && b >= 0.02)
            }
            None => false,
        }
    }

    fn capture(&mut self) {
        let (hdr_ok, has_flash, luma) = match &self.cam {
            Some(c) => (c.hdr_ok(), c.info.has_flash, c.sh.luma.load(Ordering::Relaxed)),
            None => {
                self.set_status("Camera chưa mở");
                return;
            }
        };
        if self.mode == 1 {
            let r = match &self.cam {
                Some(c) => {
                    if c.live_mark() {
                        Some(c.shoot())
                    } else {
                        None
                    }
                }
                None => None,
            };
            match r {
                Some(Ok(_)) => {
                    self.live_pend = Some(LivePend { jpeg: None, clip: None, t: Instant::now() });
                    self.set_status("Đang chụp Live… giữ yên máy 1,5 giây");
                }
                Some(Err(e)) => self.set_status(&e),
                None => self.set_status("Live chưa sẵn sàng"),
            }
            return;
        }
        let hm = self.s.hdr;
        if hm == 2 && !hdr_ok {
            self.set_status("Máy không cho chỉnh phơi sáng nên không chụp HDR được");
        }
        if hdr_ok && (hm == 2 || (hm == 1 && self.scene_hdr())) {
            let r = self.cam.as_ref().map(|c| c.shoot_hdr());
            match r {
                Some(Ok(_)) => {
                    self.hdr_buf = Some((Vec::new(), Instant::now()));
                    self.set_status("HDR: đang chụp 3 ảnh, giữ vững máy…");
                }
                Some(Err(e)) => self.set_status(&e),
                None => {}
            }
            return;
        }
        let need_flash = has_flash && (self.s.flash == 2 || (self.s.flash == 1 && luma < 70));
        if need_flash && !self.user_torch {
            if let Some(c) = self.cam.as_mut() {
                c.ctl.torch = true;
                let _ = c.apply();
            }
            self.pending_shot = Some(Instant::now() + Duration::from_millis(350));
            return;
        }
        let r = self.cam.as_ref().map(|c| c.shoot());
        match r {
            Some(Ok(_)) => self.set_status("Đang chụp..."),
            Some(Err(e)) => self.set_status(&e),
            None => {}
        }
    }

    fn on_shutter(&mut self) {
        if self.mode == 4 {
            self.toggle_stream();
        } else if self.is_video() {
            self.toggle_record();
        } else if self.timer_end.is_some() {
            self.timer_end = None;
            self.set_status("Đã huỷ hẹn giờ");
        } else {
            let secs = TIMERS[(self.s.tm.clamp(0, 2)) as usize];
            if secs > 0 {
                self.timer_end = Some(Instant::now() + Duration::from_secs(secs));
            } else {
                self.capture();
            }
        }
    }

    /// Cập nhật tốc độ gửi, phát hiện lỗi từ luồng phát.
    fn poll_stream(&mut self) {
        let stats = self.cam.as_ref().and_then(|c| c.stream_stats());
        let st = match stats {
            Some(s) => s,
            None => return,
        };
        let el = self.st_last_t.elapsed();
        if el >= Duration::from_secs(1) {
            let b = st.vbytes.load(Ordering::Relaxed);
            self.st_kbps = b.saturating_sub(self.st_last_bytes) * 8 / 1000 / el.as_secs().max(1);
            self.st_last_bytes = b;
            self.st_last_t = Instant::now();
        }
        let failed = st.failed.lock().ok().and_then(|g| g.clone());
        if let Some(e) = failed {
            if let Some(c) = self.cam.as_mut() {
                c.stop_stream();
            }
            self.set_status(&format!("Lỗi livestream: {}", e));
            self.reopen();
        }
    }

    fn toggle_stream(&mut self) {
        let streaming = self.cam.as_ref().map(|c| c.is_streaming()).unwrap_or(false);
        if streaming {
            if let Some(c) = self.cam.as_mut() {
                c.stop_stream();
            }
            self.set_status("Đã dừng livestream");
            self.reopen();
            return;
        }
        if self.s.sturl.trim().is_empty() {
            self.set_status("Chưa nhập URL livestream (Cài đặt → Livestream)");
            return;
        }
        let mut audio = self.s.audio;
        if audio && !perm::has(&self.droid, MIC_PERM) {
            perm::request(&self.droid, &[MIC_PERM]);
            audio = false;
            self.set_status("Chưa có quyền micro: phát không tiếng (cấp quyền rồi phát lại)");
        }
        let stab = if self.s.ss_on {
            Some(stream::StabParams {
                port: self.s.ss_port.trim().parse().unwrap_or(7788),
                strength: self.s.ss_str.clamp(0, 100) as u8,
                crop: self.s.ss_crop.clamp(50, 100) as u8,
                smooth: self.s.ss_smooth.clamp(1, 120) as u16,
            })
        } else {
            None
        };
        let p = stream::StreamParams {
            url: self.s.sturl.clone(),
            key: self.s.stkey.clone(),
            hevc: self.s.stc == 1 && self.hevc_ok,
            vbr_kbps: self.s.stbr,
            fps: self.s.stfps,
            abr_kbps: self.s.stabr,
            audio,
            stab,
        };
        let jv = export::Jvm::from_app(&self.droid);
        let r = self.cam.as_mut().map(|c| c.start_stream(p, jv));
        match r {
            Some(Ok(_)) => {
                self.st_last_bytes = 0;
                self.st_last_t = Instant::now();
                self.st_kbps = 0;
                self.set_status("Đang kết nối máy chủ…");
            }
            Some(Err(e)) => self.set_status(&format!("Không phát được: {}", e)),
            None => self.set_status("Camera chưa mở"),
        }
    }

    fn toggle_record(&mut self) {
        let recording = self.cam.as_ref().map(|c| c.rec.is_some() || c.is_streaming()).unwrap_or(false);
        if recording {
            let r = self.cam.as_mut().map(|c| c.stop_rec());
            match r {
                Some(Ok(p)) => {
                    self.set_status(&format!("Đã lưu {}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                    self.player = None;
                    self.tik_limit = None;
                    if let Some(job) = self.tik_job.take() {
                        self.spawn_tik(p, job);
                    } else if self.s.stab && !self.s.url.trim().is_empty() {
                        self.spawn_stab(p);
                    } else {
                        self.spawn_publish(p);
                    }
                }
                Some(Err(e)) => self.set_status(&format!("Lỗi quay: {}", e)),
                None => {}
            }
            self.reopen(); // bộ mã hóa đã dùng xong → mở lại camera
            return;
        }
        let mut audio = self.s.audio;
        if audio && !perm::has(&self.droid, MIC_PERM) {
            perm::request(&self.droid, &[MIC_PERM]);
            audio = false;
            self.set_status("Chưa có quyền micro: quay không tiếng (cấp quyền rồi quay lại)");
        }
        let tik = self.mode == 3;
        let no_mic = tik && self.sel.is_some();
        let sp = SPD[self.s.tiks.clamp(0, 4) as usize];
        let r = self.cam.as_mut().map(|c| c.start_rec(audio && !no_mic));
        match r {
            Some(Ok(_)) => {
                if tik {
                    self.tik_begin(sp);
                }
            }
            Some(Err(e)) => self.set_status(&format!("Không quay được: {}", e)),
            None => self.set_status("Camera chưa mở"),
        }
    }

    fn tap_focus(&mut self, u: f32, v: f32, pos: egui::Pos2) {
        let (rot, front) = match &self.cam {
            Some(c) => (c.info.sensor_ori, self.front),
            None => return,
        };
        let u = if front { 1.0 - u } else { u };
        let (sx, sy) = match rot {
            90 => (v, 1.0 - u),
            270 => (1.0 - v, u),
            180 => (1.0 - u, 1.0 - v),
            _ => (u, v),
        };
        if let Some(c) = self.cam.as_mut() {
            c.focus_at(sx, sy);
        }
        self.focus_ring = Some((pos, Instant::now()));
    }

    fn upload_frame(&mut self, ctx: &egui::Context) {
        if let Some(c) = &self.cam {
            let seq = c.sh.seq.load(Ordering::SeqCst);
            if seq != self.last_seq {
                // lấy bộ đệm điểm ảnh ra khỏi khoá thật nhanh (không chép) để luồng camera không bị chặn
                let mut taken: Option<egui::ColorImage> = None;
                if let Ok(mut f) = c.sh.frame.lock() {
                    if f.w > 0 && f.h > 0 && f.pixels.len() == f.w * f.h {
                        let size = [f.w, f.h];
                        taken = Some(egui::ColorImage { size, pixels: std::mem::take(&mut f.pixels) });
                    }
                }
                if let Some(img) = taken {
                    match &mut self.tex {
                        Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                        None => self.tex = Some(ctx.load_texture("cam", img, egui::TextureOptions::LINEAR)),
                    }
                    self.last_seq = seq;
                }
            }
        }
    }

    /// Đổi camera trước/sau.
    /// - Đang livestream: giữ nguyên bộ mã hóa + kết nối RTMP, chỉ đóng/mở lại camera (hình đứng ~0,5–1 giây, không ngắt luồng).
    /// - Đang quay VIDEO: lưu clip hiện tại rồi tự quay tiếp bằng camera còn lại (ra 2 tệp).
    fn flip_camera(&mut self) {
        let streaming = self.cam.as_ref().map(|c| c.is_streaming()).unwrap_or(false);
        let rec_video = self.mode == 2 && self.cam.as_ref().map(|c| c.rec.is_some()).unwrap_or(false);
        self.front = !self.front;
        self.s.front = self.front;
        self.save_cfg();
        self.compat = false;
        if streaming {
            let mut keep = self.cam.as_mut().and_then(|c| c.take_stream_keep());
            self.cam = None;
            self.last_seq = 0;
            self.zoom_t = 1.0;
            self.zoom_c = 1.0;
            let mut res: Result<cam::Camera, String> = Err("chưa mở".to_string());
            for _ in 0..4 {
                res = cam::Camera::open_keep(self.open_opts(), &mut keep);
                if res.is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            match res {
                Ok(c) => {
                    self.cam = Some(c);
                    self.sync_ctl();
                    self.set_status("Đã đổi camera, livestream vẫn chạy");
                }
                Err(e) => {
                    // không mở được camera kia → quay lại camera cũ, vẫn giữ luồng
                    self.front = !self.front;
                    self.s.front = self.front;
                    self.save_cfg();
                    let mut back: Result<cam::Camera, String> = Err("chưa mở".to_string());
                    for _ in 0..4 {
                        back = cam::Camera::open_keep(self.open_opts(), &mut keep);
                        if back.is_ok() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(250));
                    }
                    match back {
                        Ok(c) => {
                            self.cam = Some(c);
                            self.sync_ctl();
                            self.set_status(&format!("Không đổi được camera ({}) – giữ camera cũ", e));
                        }
                        Err(e2) => {
                            if let Some(k) = keep.take() {
                                k.streamer.stop();
                            }
                            self.open_err = Some(e2.clone());
                            self.set_status(&format!("Livestream dừng: {}", e2));
                        }
                    }
                }
            }
            return;
        }
        if rec_video {
            self.set_status("Đã lưu clip, tiếp tục quay bằng camera kia…");
            self.toggle_record();
            self.resume_rec = true;
            return;
        }
        self.reopen();
    }
}

fn combo(ui: &mut egui::Ui, label: &str, val: &mut i32, items: &[&str]) -> bool {
    let mut ch = false;
    let cur = items[(*val).clamp(0, items.len() as i32 - 1) as usize];
    egui::ComboBox::from_label(label).selected_text(cur).show_ui(ui, |ui| {
        for (i, n) in items.iter().enumerate() {
            if ui.selectable_value(val, i as i32, *n).changed() {
                ch = true;
            }
        }
    });
    ch
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        self.tick();
        self.upload_frame(ctx);

        // ---- nền: xem trước phủ kín chiều ngang, đặt ngay dưới thanh trên; phần dư bên dưới do thanh điều khiển lấp ----
        let screen = ctx.screen_rect();
        let mut prev_rect = egui::Rect::NOTHING;
        {
            let bg = ctx.layer_painter(egui::LayerId::background());
            bg.rect_filled(screen, 0.0, egui::Color32::BLACK);
            if let Some(t) = &self.tex {
                let sz = t.size_vec2();
                let h = screen.width() * sz.y / sz.x.max(1.0);
                prev_rect = egui::Rect::from_min_size(egui::pos2(screen.min.x, self.top_y), egui::vec2(screen.width(), h));
                bg.image(t.id(), prev_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            }
        }
        // phần dư dưới ảnh xem trước; khi ảnh tạm mất (đang mở lại camera) thì giữ giá trị cũ để bố cục không nhảy
        let gap = if prev_rect.width() > 1.0 {
            let g = (screen.max.y - prev_rect.max.y).max(0.0);
            self.last_gap = g;
            g
        } else {
            self.last_gap
        };

        // zoom mượt: chụm 2 ngón / bấm nút chỉ đổi mục tiêu, giá trị thật trượt dần theo từng khung hình
        let zd = ctx.input(|i| i.zoom_delta());
        let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.1);
        if let Some(c) = self.cam.as_mut() {
            let maxz = c.info.max_zoom.max(1.0);
            if (zd - 1.0).abs() > 0.002 {
                self.zoom_t = self.zoom_t * zd;
            }
            self.zoom_t = self.zoom_t.clamp(1.0, maxz);
            let k = 1.0 - (-dt * 10.0).exp();
            let mut nz = self.zoom_c + (self.zoom_t - self.zoom_c) * k;
            if (self.zoom_t - nz).abs() < 0.004 {
                nz = self.zoom_t;
            }
            if (nz - self.zoom_c).abs() > 0.0005 {
                self.zoom_c = nz;
                c.set_zoom(nz);
            }
        } else {
            self.zoom_t = 1.0;
            self.zoom_c = 1.0;
        }

        let recording = self.cam.as_ref().map(|c| c.rec.is_some()).unwrap_or(false);
        let streaming = self.cam.as_ref().map(|c| c.is_streaming()).unwrap_or(false);
        // đang quay hoặc phát: không cho đổi chế độ/cài đặt làm mở lại camera (sẽ ngắt)
        let busy = recording || streaming;
        let mut s_changed = false;
        let mut reopen = false;

        let top_r = egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(22.0);
            ui.style_mut().text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(16.8));
            ui.spacing_mut().button_padding = egui::vec2(7.0, 4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("CSar {}", env!("CSAN_VERSION"))).strong().size(16.8));
                if ui.button(if self.s.grid { "Lưới ✔" } else { "Lưới" }).clicked() {
                    self.s.grid = !self.s.grid;
                    s_changed = true;
                }
                if ui.button("⚙").clicked() {
                    self.settings_open = !self.settings_open;
                }
            });
            ui.add_space(4.0);
        });
        self.top_y = top_r.response.rect.max.y;

        // ảnh dài (16:9) lấn xuống thanh điều khiển → nền mờ hơn, ẩn hàng tuỳ chọn (mở bằng nút "⋯")
        let overlay = gap < 190.0 && (prev_rect.width() > 1.0 || self.last_gap > 0.0);
        let alpha: u8 = if overlay { 45 } else { 175 };
        let status_now = self.status.lock().map(|g| g.clone()).unwrap_or_default();
        if status_now != self.last_status {
            self.last_status = status_now;
            self.status_at = Instant::now();
        }
        // chiều cao thanh điều khiển luôn cố định → các nút ở cùng vị trí trong mọi chế độ
        let panel_h = gap.max(190.0);
        let bot_r = egui::TopBottomPanel::bottom("bottom")
            .exact_height(panel_h)
            .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha(alpha)).inner_margin(egui::Margin::symmetric(6.0, 4.0)))
            .show(ctx, |ui| {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(18.0);
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 92.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.columns(3, |cols| {
                    cols[0].vertical_centered(|ui| {
                        ui.add_space(24.0);
                        let has = self.cam.as_ref().map(|c| c.info.has_flash).unwrap_or(false);
                        if has && chip(ui, "Đèn pin", self.user_torch).clicked() {
                            self.user_torch = !self.user_torch;
                            let t = self.user_torch;
                            if let Some(c) = self.cam.as_mut() {
                                c.ctl.torch = t;
                                let _ = c.apply();
                            }
                        }
                    });
                    cols[1].vertical_centered(|ui| {
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(84.0, 84.0), egui::Sense::click());
                        let c = rect.center();
                        let painter = ui.painter();
                        if self.is_video() {
                            painter.circle_stroke(c, 38.0, egui::Stroke::new(3.0_f32, egui::Color32::WHITE));
                            if recording {
                                painter.rect_filled(egui::Rect::from_center_size(c, egui::vec2(34.0, 34.0)), 6.0, egui::Color32::RED);
                            } else {
                                painter.circle_filled(c, 30.0, egui::Color32::RED);
                            }
                        } else {
                            painter.circle_stroke(c, 38.0, egui::Stroke::new(3.0_f32, egui::Color32::WHITE));
                            let r = if resp.is_pointer_button_down_on() { 28.0 } else { 32.0 };
                            painter.circle_filled(c, r, egui::Color32::WHITE);
                        }
                        if resp.clicked() {
                            self.on_shutter();
                        }
                    });
                    cols[2].vertical_centered(|ui| {
                        ui.add_space(24.0);
                        let can_flip = !recording || self.mode == 2;
                        if can_flip && ui.button("🔄 Đổi").clicked() {
                            self.flip_camera();
                        }
                    });
                });
            });
            // chế độ Ảnh / Live / Video / TikTok
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                if !busy {
                    for (m, name) in [(0u8, "ẢNH"), (1, "LIVE"), (2, "VIDEO"), (3, "TIKTOK"), (4, "STREAM")] {
                        if chip(ui, name, self.mode == m).clicked() && self.mode != m {
                            self.mode = m;
                            self.s.mode = m as i32;
                            s_changed = true;
                            self.compat = false;
                            self.reopen();
                        }
                    }
                }
            });
            // thanh zoom
            let mut zt_set: Option<f32> = None;
            if let Some(c) = self.cam.as_mut() {
                let maxz = c.info.max_zoom;
                let cur = c.ctl.zoom;
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    for z in [1.0f32, 2.0, 3.0, 5.0] {
                        if z > maxz + 0.01 {
                            continue;
                        }
                        let sel = (cur - z).abs() < 0.15;
                        if chip(ui, &format!("{}x", z), sel).clicked() {
                            zt_set = Some(z);
                        }
                    }
                    ui.label(egui::RichText::new(format!("{:.1}x", cur)).color(egui::Color32::WHITE));
                    if overlay && !busy {
                        if chip(ui, if self.more { "▾ Ẩn" } else { "⋯ Tùy chọn" }, self.more).clicked() {
                            self.more = !self.more;
                        }
                    }
                });
            }
            if let Some(z) = zt_set {
                self.zoom_t = z;
            }
            // hàng tuỳ chọn theo chế độ (luôn nằm ngay trên hàng zoom)
            if !busy && (!overlay || self.more) {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    match self.mode {
                        0 | 1 => {
                            let f = ["Tắt", "Tự động", "Bật"];
                            if chip(ui, &format!("⚡ {}", f[self.s.flash.clamp(0, 2) as usize]), false).clicked() {
                                self.s.flash = (self.s.flash + 1) % 3;
                                s_changed = true;
                            }
                            if self.mode == 0 && chip(ui, &format!("HDR {}", f[self.s.hdr.clamp(0, 2) as usize]), false).clicked() {
                                self.s.hdr = (self.s.hdr + 1) % 3;
                                s_changed = true;
                            }
                            if chip(ui, &format!("⏱ {}s", TIMERS[self.s.tm.clamp(0, 2) as usize]), false).clicked() {
                                self.s.tm = (self.s.tm + 1) % 3;
                                s_changed = true;
                            }
                            if chip(ui, ["12MP", "8MP", "4MP", "2MP"][self.s.pres.clamp(0, 3) as usize], false).clicked() {
                                self.s.pres = (self.s.pres + 1) % 4;
                                s_changed = true;
                                reopen = true;
                            }
                        }
                        2 => {
                            let e = ["Chống rung: Tắt", "EIS", "EIS+", "OIS"];
                            if chip(ui, e[self.s.eis.clamp(0, 3) as usize], false).clicked() {
                                self.s.eis = (self.s.eis + 1) % 4;
                                s_changed = true;
                            }
                            if chip(ui, ["1080p", "720p", "4K"][self.s.vres.clamp(0, 2) as usize], false).clicked() {
                                self.s.vres = (self.s.vres + 1) % 3;
                                s_changed = true;
                                reopen = true;
                            }
                        }
                        3 => {
                            let label = match &self.sel {
                                Some(t) => format!("♪ {}", short(&t.title, 12)),
                                None => "♪ Chọn nhạc".to_string(),
                            };
                            if chip(ui, &label, self.sel.is_some()).clicked() {
                                self.music_open = true;
                                self.load_tracks();
                            }
                            if chip(ui, &format!("Tốc độ {}", ["3x", "2x", "1x", "0,5x", "0,25x"][self.s.tiks.clamp(0, 4) as usize]), false).clicked() {
                                self.s.tiks = (self.s.tiks + 1) % 5;
                                s_changed = true;
                            }
                            if chip(ui, &format!("{} giây", TIKD[self.s.tikd.clamp(0, 2) as usize]), false).clicked() {
                                self.s.tikd = (self.s.tikd + 1) % 3;
                                s_changed = true;
                            }
                        }
                        _ => {
                            if chip(ui, if self.s.ss_on { "Ổn định: Bật" } else { "Ổn định: Tắt" }, self.s.ss_on).clicked() {
                                self.s.ss_on = !self.s.ss_on;
                                s_changed = true;
                            }
                            if chip(ui, if self.s.stc == 1 { "H.265" } else { "H.264" }, false).clicked() {
                                if self.hevc_ok {
                                    self.s.stc = (self.s.stc + 1) % 2;
                                    s_changed = true;
                                    reopen = true;
                                } else {
                                    self.s.stc = 0;
                                    self.set_status("Máy này không có bộ mã hóa H.265");
                                }
                            }
                            // bitrate chỉ nhập tay trong Cài đặt → bấm nút này để mở Cài đặt
                            if chip(ui, &format!("{}k", self.s.stbr), false).clicked() {
                                self.settings_open = true;
                            }
                            if chip(ui, ["480p", "720p", "1080p"][self.s.stres.clamp(0, 2) as usize], false).clicked() {
                                self.s.stres = (self.s.stres + 1) % 3;
                                s_changed = true;
                                reopen = true;
                            }
                            if chip(ui, SRATIO_NAMES[self.s.strat.clamp(0, 6) as usize], false).clicked() {
                                self.s.strat = (self.s.strat + 1) % 7;
                                s_changed = true;
                                reopen = true;
                            }
                        }
                    }
                    if self.mode != 4 && chip(ui, ["4:3", "16:9"][self.s.ratio.clamp(0, 1) as usize], false).clicked() {
                        self.s.ratio = (self.s.ratio + 1) % 2;
                        s_changed = true;
                        reopen = true;
                    }
                });
            }
            ui.add_space(6.0);
            });
        });
        let _ = bot_r;

        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            if prev_rect.width() > 1.0 {
                let rect = prev_rect;
                let vis = rect.intersect(ui.max_rect());
                let mut tap: Option<(f32, f32, egui::Pos2)> = None;
                if vis.width() > 1.0 && vis.height() > 1.0 {
                    let r = ui.interact(vis, egui::Id::new("preview"), egui::Sense::click());
                    if r.clicked() {
                        if let Some(p) = r.interact_pointer_pos() {
                            tap = Some(((p.x - rect.min.x) / rect.width(), (p.y - rect.min.y) / rect.height(), p));
                        }
                    }
                }
                if let Some((u, v, p)) = tap {
                    self.tap_focus(u, v, p);
                }
                let painter = ui.painter();
                if self.s.grid && rect.width() > 1.0 {
                    let st = egui::Stroke::new(1.0_f32, egui::Color32::from_white_alpha(110));
                    for i in 1..3 {
                        let fx = rect.min.x + rect.width() * i as f32 / 3.0;
                        let fy = rect.min.y + rect.height() * i as f32 / 3.0;
                        painter.line_segment([egui::pos2(fx, rect.min.y), egui::pos2(fx, rect.max.y)], st);
                        painter.line_segment([egui::pos2(rect.min.x, fy), egui::pos2(rect.max.x, fy)], st);
                    }
                }
                if self.s.level {
                    if let Some(l) = &self.level {
                        let (roll, _pitch, flat, x, y) = l.read();
                        let c = rect.center();
                        if flat {
                            let mut off = egui::vec2(-x, y) * 300.0;
                            if off.length() > 60.0 {
                                off = off.normalized() * 60.0;
                            }
                            painter.circle_stroke(c, 60.0, egui::Stroke::new(1.5_f32, egui::Color32::WHITE));
                            let ok = off.length() < 4.0;
                            painter.circle_filled(c + off, 10.0, if ok { egui::Color32::GREEN } else { egui::Color32::YELLOW });
                        } else {
                            let ang = roll.to_radians();
                            let d = egui::vec2(ang.cos(), ang.sin()) * (rect.width() * 0.22);
                            let ok = roll.abs() < 1.0;
                            let col = if ok { egui::Color32::GREEN } else { egui::Color32::WHITE };
                            painter.line_segment([c - d, c + d], egui::Stroke::new(2.5_f32, col));
                            painter.text(c + egui::vec2(0.0, 18.0), egui::Align2::CENTER_TOP, format!("{:.0}°", roll), egui::FontId::proportional(14.0), col);
                        }
                    }
                }
                if let Some((p, t0)) = self.focus_ring {
                    if t0.elapsed() < Duration::from_millis(1000) {
                        painter.circle_stroke(p, 34.0, egui::Stroke::new(2.0_f32, egui::Color32::YELLOW));
                    } else {
                        self.focus_ring = None;
                    }
                }
                // thông báo + thời gian quay: vẽ đè lên ảnh, không chiếm chỗ bố cục
                let mut msg: Option<(String, egui::Color32)> = None;
                if let Some(t0) = self.cam.as_ref().and_then(|c| c.stream_t0()) {
                    let sec = t0.elapsed().as_secs();
                    let stats = self.cam.as_ref().and_then(|c| c.stream_stats());
                    let stab = stats.as_ref().map(|s| s.stab_on.load(Ordering::Relaxed)).unwrap_or(false);
                    let state = stats.as_ref().and_then(|s| s.state.lock().ok().map(|g| g.clone())).unwrap_or_default();
                    let mut t = format!("● LIVE {:02}:{:02} · {} kbps{}", sec / 60, sec % 60, self.st_kbps, if stab { " · ổn định" } else { "" });
                    if state.contains('⚠') || state.starts_with("Đang kết nối") {
                        t.push('\n');
                        t.push_str(&state);
                    }
                    msg = Some((t, egui::Color32::LIGHT_RED));
                } else if let Some(d) = self.cam.as_ref().and_then(|c| c.rec_elapsed()) {
                    let sec = d.as_secs();
                    msg = Some((format!("● {:02}:{:02}", sec / 60, sec % 60), egui::Color32::LIGHT_RED));
                } else if !self.last_status.is_empty() {
                    let life = if self.last_status.chars().count() > 50 { 12 } else { 6 };
                    if self.status_at.elapsed() < Duration::from_secs(life) {
                        msg = Some((self.last_status.clone(), egui::Color32::WHITE));
                    }
                }
                if let Some((text, col)) = msg {
                    let galley = painter.layout(text, egui::FontId::proportional(15.0), col, rect.width() * 0.9);
                    let sz = galley.size();
                    let pos = egui::pos2(rect.center().x - sz.x / 2.0, rect.min.y + 12.0);
                    painter.rect_filled(egui::Rect::from_min_size(pos, sz).expand(6.0), 8.0, egui::Color32::from_black_alpha(150));
                    painter.galley(pos, galley, col);
                }
                if let Some(te) = self.timer_end {
                    let left = te.saturating_duration_since(Instant::now()).as_secs() + 1;
                    painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}", left), egui::FontId::proportional(96.0), egui::Color32::WHITE);
                }
            } else if self.cam.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    match self.open_err.clone() {
                        Some(e) => {
                            ui.colored_label(egui::Color32::LIGHT_RED, e);
                            if ui.button("Mở lại camera").clicked() {
                                self.open_err = None;
                            }
                        }
                        None => {
                            ui.label("Đang chờ quyền camera...");
                        }
                    }
                });
            }
        });

        // ---- chọn nhạc ----
        let mut pick: Option<media::Track> = None;
        let mut refresh = false;
        let mut mo = self.music_open;
        egui::Window::new("Chọn nhạc").open(&mut mo).show(ctx, |ui| {
            if !self.music_msg.is_empty() {
                ui.label(self.music_msg.clone());
            }
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Tìm bài hát…"));
                if r.gained_focus() {
                    self.droid.show_soft_input(true);
                }
                if ui.button("Làm mới").clicked() {
                    refresh = true;
                }
            });
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                let f = self.filter.to_lowercase();
                for t in self.tracks.iter() {
                    if !f.is_empty() && !t.title.to_lowercase().contains(&f) && !t.artist.to_lowercase().contains(&f) {
                        continue;
                    }
                    if ui.selectable_label(false, format!("{} – {}  ({})", t.title, t.artist, fmt_clock(t.dur_ms))).clicked() {
                        pick = Some(t.clone());
                    }
                }
            });
        });
        self.music_open = mo;
        if refresh {
            self.load_tracks();
        }
        if let Some(t) = pick {
            let seg = (TIKD[self.s.tikd.clamp(0, 2) as usize] * 1000).min(t.dur_ms.max(1000));
            self.trim = Some(Trim { start_ms: 0, seg_ms: seg, track: t, player: None });
            self.music_open = false;
        }

        // ---- cắt nhạc ----
        if let Some(mut tr) = self.trim.take() {
            let mut open = true;
            let mut done = false;
            let maxd = TIKD[self.s.tikd.clamp(0, 2) as usize] * 1000;
            egui::Window::new("Cắt nhạc").open(&mut open).show(ctx, |ui| {
                ui.label(format!("{} – {}", tr.track.title, tr.track.artist));
                let dur = tr.track.dur_ms.max(2000);
                ui.add(egui::Slider::new(&mut tr.start_ms, 0..=(dur - 1000)).text("Bắt đầu (ms)"));
                let maxseg = maxd.min(dur - tr.start_ms).max(1000);
                tr.seg_ms = tr.seg_ms.clamp(1000, maxseg);
                ui.add(egui::Slider::new(&mut tr.seg_ms, 1000..=maxseg).text("Độ dài (ms)"));
                ui.label(format!("{} → {}", fmt_clock(tr.start_ms), fmt_clock(tr.start_ms + tr.seg_ms)));
                ui.horizontal(|ui| {
                    if ui.button("▶ Nghe thử").clicked() {
                        tr.player = media::Player::open(&self.droid, &tr.track.path);
                        if let Some(p) = &tr.player {
                            p.start_at(tr.start_ms, 1.0);
                        }
                    }
                    if ui.button("■ Dừng").clicked() {
                        tr.player = None;
                    }
                    if ui.button("Dùng đoạn này").clicked() {
                        done = true;
                    }
                });
            });
            if done {
                self.tik_start_ms = tr.start_ms;
                self.tik_seg_ms = tr.seg_ms;
                self.sel = Some(tr.track.clone());
            } else if open {
                self.trim = Some(tr);
            }
        }

        let mut open = self.settings_open;
        egui::Window::new("Cài đặt").open(&mut open).vscroll(true).show(ctx, |ui| {
            ui.label("Server (IP:PORT)");
            let r = ui.add(egui::TextEdit::singleline(&mut self.s.url).hint_text("192.168.1.10:8000"));
            if r.gained_focus() {
                self.droid.show_soft_input(true);
            }
            s_changed |= r.changed();
            ui.label("Token (X-Token)");
            let r = ui.add(egui::TextEdit::singleline(&mut self.s.tok));
            if r.gained_focus() {
                self.droid.show_soft_input(true);
            }
            s_changed |= r.changed();
            ui.separator();
            ui.strong("Ảnh");
            reopen |= combo(ui, "Định dạng", &mut self.s.ifmt, &["JPEG", "PNG", "WebP (lossless)", "HEIC"]);
            reopen |= ui.add(egui::Slider::new(&mut self.s.iq, 50..=100).text("Chất lượng JPEG")).changed();
            s_changed |= ui.add(egui::Slider::new(&mut self.s.iqh, 30..=100).text("Chất lượng HEIC")).changed();
            reopen |= combo(ui, "Mã hóa Live", &mut self.s.lc, &["H.264", "HEVC"]);
            s_changed |= ui.checkbox(&mut self.s.level, "Thước thăng bằng").changed();
            s_changed |= ui.checkbox(&mut self.s.exif, "Ghi thông số chụp vào ảnh (EXIF: máy, mm, ISO, f, EV, tốc độ, flash)").changed();
            reopen |= combo(ui, "Tỉ lệ", &mut self.s.ratio, &["4:3", "16:9"]);
            reopen |= combo(ui, "Độ phân giải ảnh", &mut self.s.pres, &["12 MP (tối đa)", "8 MP", "4 MP", "2 MP"]);
            s_changed |= combo(ui, "HDR", &mut self.s.hdr, &["Tắt", "Tự động", "Bật"]);
            s_changed |= combo(ui, "Chụp đêm", &mut self.s.night, &["Tắt", "Tự động", "Bật"]);
            s_changed |= combo(ui, "Flash", &mut self.s.flash, &["Tắt", "Tự động", "Bật"]);
            s_changed |= ui.checkbox(&mut self.s.grid, "Hiện lưới").changed();
            reopen |= combo(ui, "Xoay xem trước/ảnh thêm", &mut self.s.prot, &["0°", "90°", "180°", "270°"]);
            s_changed |= ui.add(egui::Slider::new(&mut self.s.pden, 0..=100).text("Khử nhiễu (server)")).changed();
            s_changed |= ui.add(egui::Slider::new(&mut self.s.psharp, 0..=100).text("Làm nét (server)")).changed();
            s_changed |= ui.checkbox(&mut self.s.send, "Gửi ảnh lên server khử nhiễu").changed();
            ui.separator();
            ui.strong("Video");
            reopen |= combo(ui, "Độ phân giải", &mut self.s.vres, &["1080p", "720p", "4K"]);
            reopen |= combo(ui, "Mã hóa", &mut self.s.vc, &["H.264", "HEVC (H.265)"]);
            reopen |= ui.add(egui::Slider::new(&mut self.s.vbr, 2000..=80000).text("Bitrate kbps")).changed();
            s_changed |= combo(ui, "Chống rung", &mut self.s.eis, &["Tắt", "EIS", "EIS nâng cao", "OIS"]);
            s_changed |= combo(ui, "Thiếu sáng", &mut self.s.vlow, &["Giữ 30 fps", "Cho giảm fps"]);
            s_changed |= ui.checkbox(&mut self.s.audio, "Ghi âm thanh (mic chính của máy)").changed();
            s_changed |= ui.checkbox(&mut self.s.stab, "Ổn định video qua server").changed();
            s_changed |= ui.add(egui::Slider::new(&mut self.s.stabscale, 70..=100).text("Tỉ lệ khung ổn định %")).changed();
            s_changed |= ui.checkbox(&mut self.s.stabext, "Mở rộng viền").changed();
            ui.separator();
            ui.strong("Livestream (RTMP / RTMPS)");
            ui.label("URL máy chủ (rtmp://host/app hoặc rtmps://host:443/app)");
            let r = ui.add(egui::TextEdit::singleline(&mut self.s.sturl).hint_text("rtmps://a.rtmps.youtube.com/live2"));
            if r.gained_focus() {
                self.droid.show_soft_input(true);
            }
            s_changed |= r.changed();
            ui.label("Khoá luồng (stream key)");
            let r = ui.add(egui::TextEdit::singleline(&mut self.s.stkey).password(true));
            if r.gained_focus() {
                self.droid.show_soft_input(true);
            }
            s_changed |= r.changed();
            reopen |= combo(ui, "Codec video", &mut self.s.stc, &["H.264 (AVC)", "H.265 (HEVC)"]);
            if !self.hevc_ok {
                ui.small("Máy này không có bộ mã hóa H.265 → sẽ dùng H.264.");
            }
            ui.horizontal(|ui| {
                ui.label("Bitrate video tối đa (kbps, nhập tay 100–100000):");
                let r = ui.add(egui::TextEdit::singleline(&mut self.br_txt).desired_width(80.0));
                if r.gained_focus() {
                    self.droid.show_soft_input(true);
                }
                if r.lost_focus() {
                    if let Ok(v) = self.br_txt.trim().parse::<i32>() {
                        let v = v.clamp(100, 100_000);
                        if v != self.s.stbr {
                            self.s.stbr = v;
                            s_changed = true;
                            reopen = true;
                        }
                    }
                    self.br_txt = self.s.stbr.to_string();
                }
            });
            ui.small("Bitrate cố định (CBR) và là mức trần: bộ mã hóa được giữ không vượt mức này. Áp dụng từ lần phát sau.");
            reopen |= combo(ui, "Độ phân giải stream", &mut self.s.stres, &["480p", "720p", "1080p"]);
            let mut fi = match self.s.stfps {
                24 => 0,
                60 => 2,
                _ => 1,
            };
            if combo(ui, "FPS", &mut fi, &["24", "30", "60"]) {
                self.s.stfps = [24, 30, 60][fi.clamp(0, 2) as usize];
                reopen = true;
            }
            reopen |= combo(ui, "Khung chủ đạo", &mut self.s.stki, &["1 giây", "2 giây", "4 giây"]);
            reopen |= combo(ui, "Tỉ lệ khung stream", &mut self.s.strat, &SRATIO_NAMES);
            reopen |= ui.checkbox(&mut self.s.stflip, "Xoay 180° (cầm máy ngược)").changed();
            s_changed |= ui.add(egui::Slider::new(&mut self.s.stabr, 32..=256).text("Bitrate âm thanh (kbps)")).changed();
            ui.separator();
            ui.strong("Ổn định video cho livestream (plugin OpenCV)");
            s_changed |= ui.checkbox(&mut self.s.ss_on, "Bật ổn định (cần app plugin đang chạy)").changed();
            s_changed |= ui.add(egui::Slider::new(&mut self.s.ss_str, 0..=100).text("Mức ổn định")).changed();
            s_changed |= ui.add(egui::Slider::new(&mut self.s.ss_crop, 80..=100).text("Tỉ lệ khung giữ lại (%)")).changed();
            s_changed |= ui.add(egui::Slider::new(&mut self.s.ss_smooth, 3..=60).text("Độ làm mượt (khung)")).changed();
            if ui.button("Mở app plugin CSar Stab").clicked() {
                if let Err(e) = media::open_plugin(&self.droid) {
                    self.set_status(&format!("Không mở được plugin (đã cài CSar Stab chưa?): {}", e));
                }
            }
            ui.label("Cổng plugin (localhost)");
            let r = ui.add(egui::TextEdit::singleline(&mut self.s.ss_port).hint_text("7788"));
            if r.gained_focus() {
                self.droid.show_soft_input(true);
            }
            s_changed |= r.changed();
            ui.separator();
            ui.small(format!("Thư mục ảnh/video + config: {}", self.dir.display()));
        });
        self.settings_open = open;

        if s_changed || reopen {
            self.save_cfg();
            if reopen && !busy {
                self.reopen();
            } else {
                self.sync_ctl();
            }
        }
    }
}
