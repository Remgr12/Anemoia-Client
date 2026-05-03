use anyhow::Result;
use jni::{
    sys::{jint, jsize, JavaVM as RawJavaVM, JNI_OK},
    JavaVM,
};
use std::sync::OnceLock;

pub struct Jvm {
    inner: JavaVM,
}

// JavaVM is Send + Sync in jni 0.21.
unsafe impl Send for Jvm {}
unsafe impl Sync for Jvm {}

static INSTANCE: OnceLock<Jvm> = OnceLock::new();

impl Jvm {
    /// Initialise from the `JavaVM*` passed by the JVM to `Agent_OnAttach`.
    /// Falls back to `JNI_GetCreatedJavaVMs` if the pointer is null.
    pub fn init(raw_jvm: *mut RawJavaVM) -> Result<()> {
        if INSTANCE.get().is_some() {
            return Ok(());
        }
        let raw = if !raw_jvm.is_null() {
            raw_jvm
        } else {
            unsafe { find_jvm()? }
        };
        let inner = unsafe { JavaVM::from_raw(raw)? };
        INSTANCE
            .set(Jvm { inner })
            .map_err(|_| anyhow::anyhow!("JVM already initialized"))?;
        Ok(())
    }

    pub fn get() -> &'static Jvm {
        INSTANCE.get().expect("Jvm::init() not called")
    }

    /// Attach the calling thread as a daemon and return a JNI env.
    ///
    /// Safe for LWJGL3's render thread: the glXSwapBuffers callsite is native,
    /// so the calling thread is a non-Java thread and daemon attachment is safe.
    ///
    /// Also clears any pending Java exception left by a previous JNI call on this
    /// thread. The JNI spec forbids calling most JNI functions while an exception
    /// is pending; clearing here gives every caller a clean env unconditionally.
    pub fn attach(&self) -> Result<jni::JNIEnv<'_>> {
        let env = self.inner.attach_current_thread_as_daemon()?;
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        Ok(env)
    }

    /// Finds a class. If `env.find_class` fails (e.g. because we are in a native hook
    /// and the system class loader is used), this falls back to using the current thread's
    /// context class loader.
    pub fn find_class<'a>(env: &mut jni::JNIEnv<'a>, name: &str) -> Result<jni::objects::JClass<'a>> {
        match env.find_class(name) {
            Ok(cls) => Ok(cls),
            Err(_) => {
                if env.exception_check()? {
                    env.exception_clear()?;
                }
                
                let thread_cls = env.find_class("java/lang/Thread")?;
                let current_thread = env
                    .call_static_method(thread_cls, "currentThread", "()Ljava/lang/Thread;", &[])?
                    .l()?;

                let class_loader = env
                    .call_method(current_thread, "getContextClassLoader", "()Ljava/lang/ClassLoader;", &[])?
                    .l()?;

                let dotted_name = name.replace('/', ".");
                let jname = env.new_string(dotted_name)?;

                let cls_obj = env
                    .call_method(
                        class_loader,
                        "loadClass",
                        "(Ljava/lang/String;)Ljava/lang/Class;",
                        &[jni::objects::JValue::from(&jname)],
                    )?
                    .l()?;

                Ok(jni::objects::JClass::from(cls_obj))
            }
        }
    }
}

type GetCreatedJVMsFn =
    unsafe extern "C" fn(*mut *mut RawJavaVM, jsize, *mut jsize) -> jint;

unsafe fn find_jvm() -> Result<*mut RawJavaVM> {
    let sym = find_jni_symbol();
    anyhow::ensure!(!sym.is_null(), "JNI_GetCreatedJavaVMs not found in process");

    let get_vms: GetCreatedJVMsFn = std::mem::transmute(sym);
    let mut raw: *mut RawJavaVM = std::ptr::null_mut();
    let mut count: jsize = 0;

    anyhow::ensure!(
        get_vms(&mut raw, 1, &mut count) == JNI_OK && count > 0,
        "No running JVM found"
    );

    Ok(raw)
}

/// Look up `JNI_GetCreatedJavaVMs` via RTLD_DEFAULT first, then by opening
/// libjvm.so directly. The JVM may load libjvm.so with RTLD_LOCAL (not
/// RTLD_GLOBAL), hiding it from RTLD_DEFAULT.
unsafe fn find_jni_symbol() -> *mut libc::c_void {
    let name = b"JNI_GetCreatedJavaVMs\0".as_ptr() as *const libc::c_char;

    let sym = libc::dlsym(libc::RTLD_DEFAULT, name);
    if !sym.is_null() {
        return sym;
    }

    // Scan /proc/self/maps for libjvm.so and open it directly.
    let maps = match std::fs::read_to_string("/proc/self/maps") {
        Ok(m) => m,
        Err(_) => return std::ptr::null_mut(),
    };

    for line in maps.lines() {
        if !line.contains("libjvm.so") {
            continue;
        }
        if let Some(path) = line.split_whitespace().last() {
            if let Ok(cpath) = std::ffi::CString::new(path) {
                let handle = libc::dlopen(cpath.as_ptr(), libc::RTLD_LAZY | libc::RTLD_NOLOAD);
                if !handle.is_null() {
                    let sym = libc::dlsym(handle, name);
                    if !sym.is_null() {
                        return sym;
                    }
                }
            }
        }
    }

    std::ptr::null_mut()
}
