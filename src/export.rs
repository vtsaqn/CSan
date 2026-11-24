//! Đưa ảnh/video vào thư viện máy (MediaStore) tại DCIM/Csan. Không cần thêm quyền (Android 10+).
use android_activity::AndroidApp;
use jni::objects::{JObject, JValue};
use jni::{JNIEnv, JavaVM};
use std::path::Path;

/// Con trỏ JVM + Activity (dạng số) để dùng được từ luồng phụ.
#[derive(Clone, Copy)]
pub struct Jvm {
    vm: usize,
    act: usize,
}

impl Jvm {
    pub fn from_app(app: &AndroidApp) -> Jvm {
        Jvm { vm: app.vm_as_ptr() as usize, act: app.activity_as_ptr() as usize }
    }
}

pub fn run<R>(j: Jvm, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Result<R, String> {
    let vm = unsafe { JavaVM::from_raw(j.vm as *mut jni::sys::JavaVM) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let act = unsafe { JObject::from_raw(j.act as jni::sys::jobject) };
    match f(&mut *env, &act) {
        Ok(v) => Ok(v),
        Err(e) => Err(crate::media::describe(&mut *env, &e)),
    }
}

fn put_str(env: &mut JNIEnv, cv: &JObject, k: &str, v: &str) -> jni::errors::Result<()> {
    let ks = env.new_string(k)?;
    let vs = env.new_string(v)?;
    env.call_method(cv, "put", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::from(&ks), JValue::from(&vs)])?;
    Ok(())
}

fn put_int(env: &mut JNIEnv, cv: &JObject, k: &str, v: i32) -> jni::errors::Result<()> {
    let ks = env.new_string(k)?;
    let io = env.new_object("java/lang/Integer", "(I)V", &[JValue::Int(v)])?;
    env.call_method(cv, "put", "(Ljava/lang/String;Ljava/lang/Integer;)V", &[JValue::from(&ks), JValue::from(&io)])?;
    Ok(())
}

fn mime_of(ext: &str) -> (&'static str, bool) {
    match ext {
        "jpg" | "jpeg" => ("image/jpeg", false),
        "png" => ("image/png", false),
        "webp" => ("image/webp", false),
        "heic" => ("image/heic", false),
        "mp4" => ("video/mp4", true),
        _ => ("application/octet-stream", false),
    }
}

/// Chép tệp vào DCIM/Csan qua MediaStore. Thành công thì xoá tệp tạm; trả về tên tệp.
pub fn publish(j: Jvm, path: &Path, delete_src: bool) -> Result<String, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".to_string());
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let (mime, video) = mime_of(&ext);
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    run(j, |env, act| {
        let resolver = env.call_method(act, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()?;
        let cv = env.new_object("android/content/ContentValues", "()V", &[])?;
        put_str(env, &cv, "_display_name", &name)?;
        put_str(env, &cv, "mime_type", mime)?;
        put_str(env, &cv, "relative_path", "DCIM/Csan")?;
        put_int(env, &cv, "is_pending", 1)?;
        let us = env.new_string(if video { "content://media/external/video/media" } else { "content://media/external/images/media" })?;
        let base = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::from(&us)])?.l()?;
        let item = env
            .call_method(&resolver, "insert", "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;", &[JValue::from(&base), JValue::from(&cv)])?
            .l()?;
        if item.is_null() {
            return Err(jni::errors::Error::NullPtr("MediaStore.insert trả về null"));
        }
        let os = env.call_method(&resolver, "openOutputStream", "(Landroid/net/Uri;)Ljava/io/OutputStream;", &[JValue::from(&item)])?.l()?;
        if os.is_null() {
            return Err(jni::errors::Error::NullPtr("openOutputStream trả về null"));
        }
        for chunk in data.chunks(1 << 20) {
            env.with_local_frame(8, |env| -> jni::errors::Result<()> {
                let arr = env.byte_array_from_slice(chunk)?;
                env.call_method(&os, "write", "([BII)V", &[JValue::from(&arr), JValue::Int(0), JValue::Int(chunk.len() as i32)])?;
                Ok(())
            })?;
        }
        env.call_method(&os, "close", "()V", &[])?;
        let cv2 = env.new_object("android/content/ContentValues", "()V", &[])?;
        put_int(env, &cv2, "is_pending", 0)?;
        let null = JObject::null();
        env.call_method(
            &resolver,
            "update",
            "(Landroid/net/Uri;Landroid/content/ContentValues;Ljava/lang/String;[Ljava/lang/String;)I",
            &[JValue::from(&item), JValue::from(&cv2), JValue::Object(&null), JValue::Object(&null)],
        )?;
        Ok(())
    })?;
    if delete_src {
        let _ = std::fs::remove_file(path);
    }
    Ok(name)
}
