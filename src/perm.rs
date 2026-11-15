use android_activity::AndroidApp;
use jni::objects::{JObject, JValue};
use jni::JNIEnv;
use jni::JavaVM;

fn with_env<R>(app: &AndroidApp, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Option<R> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let act = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    f(&mut *env, &act).ok()
}

pub fn has(app: &AndroidApp, perm: &str) -> bool {
    with_env(app, |env, act| {
        let s = env.new_string(perm)?;
        let r = env
            .call_method(act, "checkSelfPermission", "(Ljava/lang/String;)I", &[JValue::from(&s)])?
            .i()?;
        Ok(r == 0)
    })
    .unwrap_or(false)
}

pub fn request(app: &AndroidApp, perms: &[&str]) {
    let _ = with_env(app, |env, act| {
        let arr = env.new_object_array(perms.len() as i32, "java/lang/String", JObject::null())?;
        for (i, p) in perms.iter().enumerate() {
            let s = env.new_string(*p)?;
            env.set_object_array_element(&arr, i as i32, &s)?;
        }
        env.call_method(
            act,
            "requestPermissions",
            "([Ljava/lang/String;I)V",
            &[JValue::from(&arr), JValue::Int(1001)],
        )?;
        Ok(())
    });
}
