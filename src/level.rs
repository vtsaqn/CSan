//! Thước thăng bằng: đọc gia tốc kế qua NDK (ASensorManager).
#![allow(non_camel_case_types, dead_code)]
use std::ffi::{c_char, c_int, c_void, CString};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub enum ALooper {}
pub enum ASensorManager {}
pub enum ASensor {}
pub enum ASensorEventQueue {}

#[repr(C)]
struct SensorEvent {
    version: i32,
    sensor: i32,
    typ: i32,
    reserved0: i32,
    timestamp: i64,
    data: [f32; 16],
    flags: u32,
    reserved1: [i32; 3],
}

#[link(name = "android")]
extern "C" {
    fn ALooper_prepare(opts: c_int) -> *mut ALooper;
    fn ALooper_pollOnce(timeout: c_int, out_fd: *mut c_int, out_events: *mut c_int, out_data: *mut *mut c_void) -> c_int;
    fn ASensorManager_getInstanceForPackage(pkg: *const c_char) -> *mut ASensorManager;
    fn ASensorManager_getDefaultSensor(m: *mut ASensorManager, t: c_int) -> *const ASensor;
    fn ASensorManager_createEventQueue(m: *mut ASensorManager, l: *mut ALooper, ident: c_int, cb: *mut c_void, data: *mut c_void) -> *mut ASensorEventQueue;
    fn ASensorManager_destroyEventQueue(m: *mut ASensorManager, q: *mut ASensorEventQueue) -> c_int;
    fn ASensorEventQueue_enableSensor(q: *mut ASensorEventQueue, s: *const ASensor) -> c_int;
    fn ASensorEventQueue_disableSensor(q: *mut ASensorEventQueue, s: *const ASensor) -> c_int;
    fn ASensorEventQueue_setEventRate(q: *mut ASensorEventQueue, s: *const ASensor, usec: i32) -> c_int;
    fn ASensorEventQueue_getEvents(q: *mut ASensorEventQueue, ev: *mut SensorEvent, count: usize) -> isize;
}

pub struct Level {
    acc: Arc<[AtomicU32; 3]>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

fn run(acc: Arc<[AtomicU32; 3]>, stop: Arc<AtomicBool>, pkg: String) {
    unsafe {
        let looper = ALooper_prepare(1);
        let pk = CString::new(pkg).unwrap_or_default();
        let mgr = ASensorManager_getInstanceForPackage(pk.as_ptr());
        if mgr.is_null() || looper.is_null() {
            return;
        }
        let sensor = ASensorManager_getDefaultSensor(mgr, 1); // ASENSOR_TYPE_ACCELEROMETER
        if sensor.is_null() {
            return;
        }
        let q = ASensorManager_createEventQueue(mgr, looper, 3, null_mut(), null_mut());
        if q.is_null() {
            return;
        }
        ASensorEventQueue_enableSensor(q, sensor);
        ASensorEventQueue_setEventRate(q, sensor, 40_000);
        let mut ev: [SensorEvent; 8] = std::mem::zeroed();
        while !stop.load(Ordering::SeqCst) {
            let mut fd: c_int = 0;
            let mut events: c_int = 0;
            let mut data: *mut c_void = null_mut();
            ALooper_pollOnce(100, &mut fd, &mut events, &mut data);
            loop {
                let n = ASensorEventQueue_getEvents(q, ev.as_mut_ptr(), 8);
                if n <= 0 {
                    break;
                }
                let e = &ev[(n - 1) as usize];
                for k in 0..3 {
                    acc[k].store(e.data[k].to_bits(), Ordering::Relaxed);
                }
            }
        }
        ASensorEventQueue_disableSensor(q, sensor);
        ASensorManager_destroyEventQueue(mgr, q);
    }
}

impl Level {
    pub fn start(pkg: &str) -> Level {
        let acc = Arc::new([AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)]);
        let stop = Arc::new(AtomicBool::new(false));
        let (a2, s2, p) = (acc.clone(), stop.clone(), pkg.to_string());
        let handle = std::thread::Builder::new().name("csan-level".to_string()).spawn(move || run(a2, s2, p)).ok();
        Level { acc, stop, handle }
    }

    fn v(&self, i: usize) -> f32 {
        f32::from_bits(self.acc[i].load(Ordering::Relaxed))
    }

    /// (roll độ, pitch độ, nằm phẳng?, x, y) - roll = nghiêng trái/phải khi cầm dọc.
    pub fn read(&self) -> (f32, f32, bool, f32, f32) {
        let (x, y, z) = (self.v(0), self.v(1), self.v(2));
        let g = (x * x + y * y + z * z).sqrt().max(0.001);
        let flat = z.abs() > 0.9 * g;
        let roll = x.atan2(y).to_degrees();
        let pitch = (z / g).clamp(-1.0, 1.0).asin().to_degrees();
        (roll, pitch, flat, x / g, y / g)
    }
}

impl Drop for Level {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
