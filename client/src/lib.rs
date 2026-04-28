mod glfw;
mod gui;
mod hook;
mod hotkeys;
mod jvm;
mod lua_api;
mod lua_engine;
pub mod mc;

use log::info;
use simplelog::{Config, LevelFilter, WriteLogger};
use std::{
    fs::File,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

pub(crate) static RUNNING: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn initialize_client() {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let _ = WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("/tmp/anemoia_client.log").unwrap(),
    );
    info!("anemoia_client: initializing");

    thread::spawn(|| {
        if let Err(e) = init() {
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
    hook::uninstall();
    lua_engine::teardown();
}

fn init() -> anyhow::Result<()> {
    jvm::Jvm::init()?;
    info!("JVM acquired");

    lua_engine::init()?;
    info!("Lua engine ready");

    hook::install()?;
    info!("glXSwapBuffers hooked");

    // GUI + hotkey thread initialize lazily on the first render frame so the
    // OpenGL context is guaranteed to be current.

    Ok(())
}
