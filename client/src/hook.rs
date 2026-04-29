use anyhow::Result;
use ilhook::x64::{CallbackOption, HookFlags, HookType, Hooker, Registers};
use log::{error, info, warn};
use std::{
    ffi::CString,
    sync::{Mutex, OnceLock},
};

use crate::{gui, jvm::Jvm, lua_engine};

static HOOK_PTRS: OnceLock<Mutex<Vec<usize>>> = OnceLock::new();

pub fn install() -> Result<()> {
    let addrs = find_all_swap_buffers();
    if addrs.is_empty() {
        anyhow::bail!("neither eglSwapBuffers nor glXSwapBuffers found");
    }

    let mut hooks = Vec::new();
    for (addr, name) in addrs {
        info!("{} @ 0x{:x}", name, addr);

        let hook = unsafe {
            Hooker::new(
                addr,
                HookType::JmpBack(on_swap_buffers),
                CallbackOption::None,
                0,
                HookFlags::empty(),
            )
            .hook()
        };

        match hook {
            Ok(h) => {
                let raw = Box::into_raw(Box::new(h)) as usize;
                hooks.push(raw);
                info!("{} hooked", name);
            }
            Err(e) => {
                warn!("failed to hook {}: {}", name, e);
            }
        }
    }

    HOOK_PTRS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .extend(hooks);

    Ok(())
}

pub fn uninstall() {
    if let Some(mutex) = HOOK_PTRS.get() {
        let mut hooks = mutex.lock().unwrap();
        for raw in hooks.drain(..) {
            if raw != 0 {
                drop(unsafe { Box::from_raw(raw as *mut ilhook::x64::HookPoint) });
            }
        }
    }
}

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

/// Find eglSwapBuffers (Wayland/EGL) and glXSwapBuffers (X11).
/// Returns a list of all found functions to be hooked.
fn find_all_swap_buffers() -> Vec<(usize, &'static str)> {
    let candidates: &[(&str, &str)] = &[
        ("glXSwapBuffers", "libGL"),
        ("eglSwapBuffers", "libEGL"),
    ];

    candidates
        .iter()
        .filter_map(|&(sym_name, lib_substr)| {
            find_sym(sym_name, lib_substr).map(|addr| (addr, sym_name))
        })
        .collect()
}

/// Look up `sym_name` via RTLD_DEFAULT, then by opening the first mapped
/// library whose path contains `lib_substr`.
pub(crate) fn find_sym(sym_name: &str, lib_substr: &str) -> Option<usize> {
    unsafe {
        let cname = CString::new(sym_name).ok()?;

        // Fast path: symbol already in global namespace.
        let addr = libc::dlsym(libc::RTLD_DEFAULT, cname.as_ptr()) as usize;
        if addr != 0 {
            return Some(addr);
        }

        // Slow path: library loaded RTLD_LOCAL — find it in maps and open directly.
        let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
        for line in maps.lines() {
            if !line.contains(lib_substr) {
                continue;
            }
            let path = line.split_whitespace().last()?;
            if !path.starts_with('/') {
                continue;
            }
            let cpath = CString::new(path).ok()?;
            let handle = libc::dlopen(cpath.as_ptr(), libc::RTLD_LAZY | libc::RTLD_NOLOAD);
            if handle.is_null() {
                // NOLOAD failed — try loading fresh (safe: already mapped, just gets a handle).
                let handle = libc::dlopen(cpath.as_ptr(), libc::RTLD_LAZY);
                if handle.is_null() {
                    continue;
                }
                let addr = libc::dlsym(handle, cname.as_ptr()) as usize;
                if addr != 0 {
                    return Some(addr);
                }
            } else {
                let addr = libc::dlsym(handle, cname.as_ptr()) as usize;
                if addr != 0 {
                    return Some(addr);
                }
            }
        }

        None
    }
}
