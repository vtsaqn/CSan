//! CSar Stab: plugin ổn định video cho CSar. Mở cổng TCP localhost, nhận khung NV12, trả khung đã ổn định.
//! Giao thức CSAR-STB v1: xem README.md ở thư mục gốc.
mod stab;
#[cfg(feature = "opencv")]
mod cv;

use android_activity::AndroidApp;
use eframe::egui;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use winit::platform::android::EventLoopBuilderExtAndroid;

#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("csarstab"));
    let app2 = app.clone();
    let options = eframe::NativeOptions {
        event_loop_builder: Some(Box::new(move |builder| {
            builder.with_android_app(app2);
        })),
        ..Default::default()
    };
    let r = eframe::run_native(
        "CSar Stab",
        options,
        Box::new(move |cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert("vn".to_owned(), egui::FontData::from_static(include_bytes!("../../assets/DejaVuSans.ttf")).into());
            if let Some(v) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                v.insert(0, "vn".to_owned());
            }
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(Ui::new(app)))
        }),
    );
    if let Err(e) = r {
        log::error!("run_native: {}", e);
    }
}

fn wake_lock(app: &AndroidApp) {
    use jni::objects::{JObject, JValue};
    let vm = match unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) } {
        Ok(v) => v,
        Err(_) => return,
    };
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(_) => return,
    };
    let act = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    let r: jni::errors::Result<()> = (|| {
        let name = env.new_string("power")?;
        let pm = env.call_method(&act, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[JValue::from(&name)])?.l()?;
        let tag = env.new_string("csar:stab")?;
        let wl = env.call_method(&pm, "newWakeLock", "(ILjava/lang/String;)Landroid/os/PowerManager$WakeLock;", &[JValue::Int(1), JValue::from(&tag)])?.l()?;
        env.call_method(&wl, "acquire", "()V", &[])?;
        Ok(())
    })();
    if r.is_err() {
        let _ = env.exception_clear();
    }
}

struct Shared {
    status: Mutex<String>,
    frames: AtomicU64,
    stop: AtomicBool,
}

enum Backend {
    Native(stab::Stab),
    #[cfg(feature = "opencv")]
    Cv(cv::CvStab),
}

impl Backend {
    fn process(&mut self, f: &mut [u8]) {
        match self {
            Backend::Native(s) => s.process(f),
            #[cfg(feature = "opencv")]
            Backend::Cv(s) => s.process(f),
        }
    }
}

fn be16(b: &[u8], i: usize) -> usize {
    ((b[i] as usize) << 8) | b[i + 1] as usize
}

fn handle(mut s: TcpStream, sh: &Shared) -> Result<(), String> {
    let _ = s.set_nodelay(true);
    s.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let mut hello = [0u8; 17];
    s.read_exact(&mut hello).map_err(|e| e.to_string())?;
    if &hello[..8] != b"CSARSTB1" {
        return Err("Sai giao thức".to_string());
    }
    let (w, h) = (be16(&hello, 8), be16(&hello, 10));
    let (_fps, strength, crop, smooth) = (hello[12], hello[13], hello[14], be16(&hello, 15));
    s.write_all(b"CSAROK1").map_err(|e| e.to_string())?;
    let p = stab::Params { w, h, strength: strength as f32 / 100.0, crop: crop as f32 / 100.0, smooth: smooth.max(1) as f32 };
    #[cfg(feature = "opencv")]
    let mut be = Backend::Cv(cv::CvStab::new(w, h, p.strength, p.crop, p.smooth));
    #[cfg(not(feature = "opencv"))]
    let mut be = Backend::Native(stab::Stab::new(p));
    if let Ok(mut st) = sh.status.lock() {
        *st = format!("Đang ổn định {}x{} (mức {}%, crop {}%, làm mượt {})", w, h, strength, crop, smooth);
    }
    let want = w * h * 3 / 2;
    let mut buf = vec![0u8; want];
    s.set_read_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;
    while !sh.stop.load(Ordering::SeqCst) {
        let mut l = [0u8; 4];
        s.read_exact(&mut l).map_err(|e| e.to_string())?;
        let n = u32::from_be_bytes(l) as usize;
        if n != want {
            return Err(format!("Kích thước khung {} khác {}", n, want));
        }
        s.read_exact(&mut buf).map_err(|e| e.to_string())?;
        be.process(&mut buf);
        s.write_all(&(want as u32).to_be_bytes()).map_err(|e| e.to_string())?;
        s.write_all(&buf).map_err(|e| e.to_string())?;
        sh.frames.fetch_add(1, Ordering::Relaxed);
    }
    Ok(())
}

fn serve(port: u16, sh: Arc<Shared>) {
    let l = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            if let Ok(mut st) = sh.status.lock() {
                *st = format!("Không mở được cổng {}: {}", port, e);
            }
            return;
        }
    };
    let _ = l.set_nonblocking(true);
    if let Ok(mut st) = sh.status.lock() {
        *st = format!("Đang chờ CSar kết nối ở cổng {}", port);
    }
    while !sh.stop.load(Ordering::SeqCst) {
        match l.accept() {
            Ok((s, _)) => {
                let _ = s.set_nonblocking(false);
                if let Ok(mut st) = sh.status.lock() {
                    *st = "CSar đã kết nối".to_string();
                }
                let r = handle(s, &sh);
                if let Ok(mut st) = sh.status.lock() {
                    *st = match r {
                        Ok(_) => format!("Đang chờ CSar kết nối ở cổng {}", port),
                        Err(e) => format!("Ngắt kết nối ({}). Đang chờ ở cổng {}", e, port),
                    };
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

struct Ui {
    shared: Arc<Shared>,
    port: String,
    running: bool,
    app: AndroidApp,
}

impl Ui {
    fn new(app: AndroidApp) -> Ui {
        Ui { shared: Arc::new(Shared { status: Mutex::new("Chưa chạy".to_string()), frames: AtomicU64::new(0), stop: AtomicBool::new(true) }), port: "7788".to_string(), running: false, app }
    }

    fn start(&mut self) {
        let port: u16 = self.port.trim().parse().unwrap_or(7788);
        wake_lock(&self.app);
        self.shared = Arc::new(Shared { status: Mutex::new("Đang khởi động…".to_string()), frames: AtomicU64::new(0), stop: AtomicBool::new(false) });
        let sh = self.shared.clone();
        std::thread::spawn(move || serve(port, sh));
        self.running = true;
    }

    fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.running = false;
        if let Ok(mut st) = self.shared.status.lock() {
            *st = "Đã dừng".to_string();
        }
    }
}

impl eframe::App for Ui {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(300));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(40.0);
            ui.heading("CSar Stab");
            ui.label("Plugin ổn định video cho livestream của CSar.");
            #[cfg(feature = "opencv")]
            ui.label("Bộ ổn định: OpenCV (LK optical flow + affine)");
            #[cfg(not(feature = "opencv"))]
            ui.label("Bộ ổn định: thuần Rust (khối SAD + lọc thông cao)");
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Cổng:");
                let r = ui.add_enabled(!self.running, egui::TextEdit::singleline(&mut self.port).desired_width(80.0));
                if r.gained_focus() {
                    self.app.show_soft_input(true);
                }
            });
            if self.running {
                if ui.button("■ Dừng").clicked() {
                    self.stop();
                }
            } else if ui.button("▶ Bắt đầu").clicked() {
                self.start();
            }
            ui.separator();
            let st = self.shared.status.lock().map(|g| g.clone()).unwrap_or_default();
            ui.label(st);
            ui.label(format!("Số khung đã xử lý: {}", self.shared.frames.load(Ordering::Relaxed)));
            ui.separator();
            ui.small("Hãy để app này chạy (có thể mở cửa sổ nổi / chia đôi màn hình) và tắt tối ưu pin cho nó, rồi bật \"Ổn định\" trong CSar → STREAM.");
        });
    }
}
