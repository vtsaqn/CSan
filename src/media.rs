//! Chọn nhạc (MediaStore) và phát thử nhạc (MediaPlayer) qua JNI.
use android_activity::AndroidApp;
use jni::objects::{GlobalRef, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};

#[derive(Clone)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub dur_ms: i64,
    pub path: String,
}

fn with_env<R>(app: &AndroidApp, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Option<R> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let act = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    f(&mut *env, &act).ok()
}

fn jstr(env: &mut JNIEnv, o: JObject) -> String {
    if o.is_null() {
        return String::new();
    }
    let s = JString::from(o);
    env.get_string(&s).map(|j| j.into()).unwrap_or_default()
}

/// Liệt kê nhạc trên máy (cần quyền đọc âm thanh).
pub fn list_tracks(app: &AndroidApp) -> Vec<Track> {
    with_env(app, |env, act| {
        let resolver = env.call_method(act, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()?;
        let s = env.new_string("content://media/external/audio/media")?;
        let uri = env
            .call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::from(&s)])?
            .l()?;
        let cols = ["title", "artist", "duration", "_data"];
        let arr = env.new_object_array(cols.len() as i32, "java/lang/String", JObject::null())?;
        for (i, c) in cols.iter().enumerate() {
            let cs = env.new_string(*c)?;
            env.set_object_array_element(&arr, i as i32, &cs)?;
        }
        let sel = env.new_string("is_music != 0 AND duration > 10000")?;
        let order = env.new_string("title COLLATE NOCASE ASC")?;
        let null = JObject::null();
        let cur = env
            .call_method(
                &resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[JValue::from(&uri), JValue::from(&arr), JValue::from(&sel), JValue::Object(&null), JValue::from(&order)],
            )?
            .l()?;
        let mut out: Vec<Track> = Vec::new();
        if cur.is_null() {
            return Ok(out);
        }
        while out.len() < 500 && env.call_method(&cur, "moveToNext", "()Z", &[])?.z()? {
            let t: jni::errors::Result<Track> = env.with_local_frame(16, |env| {
                let title = env.call_method(&cur, "getString", "(I)Ljava/lang/String;", &[JValue::Int(0)])?.l()?;
                let artist = env.call_method(&cur, "getString", "(I)Ljava/lang/String;", &[JValue::Int(1)])?.l()?;
                let dur = env.call_method(&cur, "getLong", "(I)J", &[JValue::Int(2)])?.j()?;
                let data = env.call_method(&cur, "getString", "(I)Ljava/lang/String;", &[JValue::Int(3)])?.l()?;
                Ok(Track { title: jstr(env, title), artist: jstr(env, artist), dur_ms: dur, path: jstr(env, data) })
            });
            if let Ok(t) = t {
                if !t.path.is_empty() {
                    out.push(t);
                }
            }
        }
        let _ = env.call_method(&cur, "close", "()V", &[]);
        Ok(out)
    })
    .unwrap_or_default()
}

/// Trình phát nhạc (android.media.MediaPlayer).
pub struct Player {
    app: AndroidApp,
    obj: GlobalRef,
}

impl Player {
    pub fn open(app: &AndroidApp, path: &str) -> Option<Player> {
        let obj = with_env(app, |env, _act| {
            let mp = env.new_object("android/media/MediaPlayer", "()V", &[])?;
            let p = env.new_string(path)?;
            env.call_method(&mp, "setDataSource", "(Ljava/lang/String;)V", &[JValue::from(&p)])?;
            env.call_method(&mp, "prepare", "()V", &[])?;
            env.new_global_ref(&mp)
        })?;
        Some(Player { app: app.clone(), obj })
    }

    /// Phát từ ms với tốc độ speed (1.0 = thường).
    pub fn start_at(&self, ms: i64, speed: f32) {
        let o = self.obj.clone();
        let _ = with_env(&self.app, move |env, _| {
            if (speed - 1.0).abs() > 0.001 {
                let pp = env.new_object("android/media/PlaybackParams", "()V", &[])?;
                env.call_method(&pp, "setSpeed", "(F)Landroid/media/PlaybackParams;", &[JValue::Float(speed)])?;
                env.call_method(&pp, "setPitch", "(F)Landroid/media/PlaybackParams;", &[JValue::Float(1.0)])?;
                let _ = env.call_method(&o, "setPlaybackParams", "(Landroid/media/PlaybackParams;)V", &[JValue::from(&pp)]);
                let _ = env.exception_clear();
            }
            env.call_method(&o, "seekTo", "(I)V", &[JValue::Int(ms as i32)])?;
            env.call_method(&o, "start", "()V", &[])?;
            Ok(())
        });
    }

    pub fn pos_ms(&self) -> i64 {
        let o = self.obj.clone();
        with_env(&self.app, move |env, _| Ok(env.call_method(&o, "getCurrentPosition", "()I", &[])?.i()? as i64)).unwrap_or(0)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let o = self.obj.clone();
        let _ = with_env(&self.app, move |env, _| {
            let _ = env.call_method(&o, "stop", "()V", &[]);
            let _ = env.exception_clear();
            env.call_method(&o, "release", "()V", &[])?;
            Ok(())
        });
    }
}
