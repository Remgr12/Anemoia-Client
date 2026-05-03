pub mod config;
mod glfw;
mod gui;
mod hook;
mod hotkeys;
mod jvm;
mod lua_api;
mod lua_engine;
pub mod mc;
mod packet_capture;
pub mod zulip;

use log::info;
use simplelog::{Config, LevelFilter, WriteLogger};
use std::{
    fs::File,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

pub(crate) static RUNNING: AtomicBool = AtomicBool::new(false);

/// Called by agent_loader after dlopen.  `raw_jvm` is the `JavaVM*` received
/// from `Agent_OnAttach` — passed directly so we never need dlsym for it.
#[no_mangle]
pub extern "C" fn initialize_client(raw_jvm: *mut jni::sys::JavaVM) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let _ = WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("/tmp/anemoia_client.log").unwrap(),
    );
    info!("anemoia_client: initializing");

    // Safety: pointer comes from the JVM's own Agent_OnAttach; valid for
    // the process lifetime.
    let raw_jvm = raw_jvm as usize; // move as usize to satisfy Send
    thread::spawn(move || {
        if let Err(e) = init(raw_jvm as *mut jni::sys::JavaVM) {
            log::error!("init failed: {:#}", e);
            RUNNING.store(false, Ordering::SeqCst);
        }
    });
}

#[no_mangle]
pub extern "C" fn cleanup_client() {
    if !RUNNING.swap(false, Ordering::SeqCst) {
        return;
    }
    info!("anemoia_client: cleanup");
    hotkeys::stop();
    gui::cleanup();
    hook::uninstall();
    lua_engine::teardown();
}

fn init(raw_jvm: *mut jni::sys::JavaVM) -> anyhow::Result<()> {
    jvm::Jvm::init(raw_jvm)?;
    info!("JVM acquired");

    zulip::init();
    lua_engine::init()?;
    info!("Lua engine ready");

    hook::install()?;

    // GUI + hotkey thread initialize lazily on the first render frame so the
    // OpenGL context is guaranteed to be current.

    Ok(())
}
