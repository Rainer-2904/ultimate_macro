//! NativeActivity bridge. Java owns camera permission and Activity lifecycle;
//! Rust owns product lookup and consumption. No camera frame crosses JNI.
use jni::{
    JavaVM,
    objects::{GlobalRef, JObject, JString, JValue},
};
use std::sync::OnceLock;

struct Bridge {
    vm: JavaVM,
    activity: GlobalRef,
}
static BRIDGE: OnceLock<Bridge> = OnceLock::new();

#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    let Some(data_dir) = app.internal_data_path() else {
        eprintln!("Android private storage is unavailable");
        return;
    };
    crate::storage::init(data_dir);
    // Reqwest's default TLS backend uses Android's certificate verifier. Its
    // Java companion is packaged by Gradle and must be initialized before HTTP.
    let tls_vm = unsafe { jni_platform::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let tls_init = tls_vm.attach_current_thread_for_scope(|env| {
        let context =
            unsafe { jni_platform::objects::JObject::from_raw(env, app.activity_as_ptr().cast()) };
        rustls_platform_verifier::android::init_with_env(env, context)
    });
    if let Err(error) = tls_init {
        eprintln!("Failed to initialize Android TLS: {error}");
        return;
    }
    // android-activity owns these pointers for the activity lifetime. Promote the
    // borrowed Activity to a global JNI reference before leaving this JNI scope.
    let bridge = (|| -> Result<Bridge, jni::errors::Error> {
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }?;
        let activity = {
            let env = vm.attach_current_thread()?;
            let borrowed = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
            env.new_global_ref(&borrowed)?
        };
        Ok(Bridge { vm, activity })
    })();
    match bridge {
        Ok(bridge) => {
            let _ = BRIDGE.set(bridge);
        }
        Err(error) => {
            eprintln!("Failed to initialize camera bridge: {error}");
            return;
        }
    }
    if let Err(error) = slint::android::init(app) {
        eprintln!("Failed to initialize Slint Android: {error}");
        return;
    }
    crate::run();
}

fn call(method: &str, signature: &str, args: &[JValue]) -> Result<Option<String>, String> {
    let bridge = BRIDGE.get().ok_or("Camera bridge is unavailable.")?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|e| e.to_string())?;
    // Polling creates Java strings; a local frame prevents JNI references from
    // accumulating on Slint's long-lived native thread.
    let result = env.with_local_frame(8, |env| -> Result<_, jni::errors::Error> {
        let value = env.call_method(bridge.activity.as_obj(), method, signature, args)?;
        if signature.ends_with('V') {
            return Ok(None);
        }
        let object = value.l()?;
        if object.is_null() {
            return Ok(None);
        }
        let string = JString::from(object);
        Ok(Some(env.get_string(&string)?.into()))
    });
    result.map_err(|error| {
        // A pending Java exception would poison subsequent JNI calls.
        let _ = env.exception_clear();
        log::error!("Android camera bridge failed: {error}");
        "Could not open camera. You can enter the barcode manually.".to_string()
    })
}

pub fn start_scan(generation: u64) -> Result<(), String> {
    call("startScan", "(J)V", &[JValue::Long(generation as i64)]).map(|_| ())
}

pub fn take_scan_result() -> Result<Option<String>, String> {
    call("takeScanResult", "()Ljava/lang/String;", &[])
}
