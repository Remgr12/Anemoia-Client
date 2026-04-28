use anyhow::Result;
use ilhook::x64::{CallbackOption, HookFlags, HookType, Hooker, Registers};
use log::{error, warn};
use std::{
    ffi::CString,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{gui, jvm::Jvm, lua_engine};

// Stores the raw hook pointer so we can drop it on cleanup.
static HOOK_PTR: AtomicUsize = AtomicUsize::new(0);

pub fn install() -> Result<()> {
    let addr = find_glx_swap_buffers()?;
    log::info!("glXSwapBuffers @ 0x{:x}", addr);

    let hook = unsafe {
        Hooker::new(
            addr,
            HookType::JmpBack(on_swap_buffers),
            CallbackOption::None,
            0,
            HookFlags::empty(),
        )
        .hook()?
    };

    // Leak the hook — it must stay alive for the process lifetime.
    // We store the pointer so uninstall() can drop it.
    let raw = Box::into_raw(Box::new(hook)) as usize;
    HOOK_PTR.store(raw, Ordering::SeqCst);

    Ok(())
}

pub fn uninstall() {
    let raw = HOOK_PTR.swap(0, Ordering::SeqCst);
    if raw != 0 {
        // Safety: we allocated this in install() and it has not been freed.
        drop(unsafe { Box::from_raw(raw as *mut ilhook::x64::HookPoint) });
    }
}

/// Called by ilhook on every glXSwapBuffers invocation.
///
/// This fires on the LWJGL3 render thread, which is a native thread —
/// safe to attach/detach as a JNI daemon.
unsafe extern "win64" fn on_swap_buffers(_regs: *mut Registers, _user_data: usize) {
    tick();
    gui::frame();
}

fn tick() {
    let jvm = Jvm::get();
    let mut env = match jvm.attach() {
        Ok(e) => e,
        Err(e) => {
            warn!("tick: JNI attach failed: {}", e);
            return;
        }
    };

    if let Err(e) = lua_engine::on_tick(&mut env) {
        error!("tick: Lua error: {:#}", e);
    }
}

fn find_glx_swap_buffers() -> Result<usize> {
    let lib_name = CString::new("libGL.so.1").unwrap();
    let sym_name = CString::new("glXSwapBuffers").unwrap();

    unsafe {
        let handle = libc::dlopen(lib_name.as_ptr(), libc::RTLD_LAZY);
        anyhow::ensure!(!handle.is_null(), "libGL.so.1 not loaded in process");

        let addr = libc::dlsym(handle, sym_name.as_ptr()) as usize;
        anyhow::ensure!(addr != 0, "glXSwapBuffers symbol not found");

        Ok(addr)
    }
}
