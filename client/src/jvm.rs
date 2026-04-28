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
    pub fn init() -> Result<()> {
        if INSTANCE.get().is_some() {
            return Ok(());
        }
        let raw = unsafe { find_jvm()? };
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
    pub fn attach(&self) -> Result<jni::JNIEnv<'_>> {
        Ok(self.inner.attach_current_thread_as_daemon()?)
    }
}

type GetCreatedJVMsFn =
    unsafe extern "C" fn(*mut *mut RawJavaVM, jsize, *mut jsize) -> jint;

unsafe fn find_jvm() -> Result<*mut RawJavaVM> {
    let sym = libc::dlsym(
        libc::RTLD_DEFAULT,
        b"JNI_GetCreatedJavaVMs\0".as_ptr() as *const libc::c_char,
    );

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
