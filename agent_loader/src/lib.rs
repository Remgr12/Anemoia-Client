use ctor::{ctor, dtor};
use jni::{
    sys::{jint, jsize, JavaVM as RawJavaVM, JNI_OK},
    JavaVM,
};
use libloading::{Library, Symbol};
use log::{error, info, warn};
use simplelog::{Config, LevelFilter, WriteLogger};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

static RUNNING: AtomicBool = AtomicBool::new(false);
static LOADED_LIB: OnceLock<Mutex<Option<Library>>> = OnceLock::new();
static SAVED_JVM: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[no_mangle]
#[ctor]
fn agent_onload() {
    let _ = WriteLogger::init(
        LevelFilter::Info,
        Config::default(),
        File::create("/tmp/anemoia_agent.log").unwrap(),
    );

    LOADED_LIB.get_or_init(|| Mutex::new(None));
    RUNNING.store(true, Ordering::SeqCst);
    info!("agent_loader: loaded");

    unsafe {
        // Cast through *const () to satisfy the "no direct fn→integer cast" lint.
        libc::signal(libc::SIGTERM, handle_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, handle_signal as *const () as libc::sighandler_t);
    }

    thread::spawn(jvm_monitor);
    thread::spawn(command_server);
}

#[no_mangle]
#[dtor]
fn agent_onunload() {
    RUNNING.store(false, Ordering::SeqCst);
    unload_client();
    info!("agent_loader: unloaded");
}

extern "C" fn handle_signal(_: libc::c_int) {
    RUNNING.store(false, Ordering::SeqCst);
}

/// Called by the JVM after loading the library via the Attach API.
/// Saves the `JavaVM*` so we can pass it to the client on load.
#[no_mangle]
pub extern "C" fn Agent_OnAttach(
    vm: *mut libc::c_void,
    _options: *mut libc::c_char,
    _reserved: *mut libc::c_void,
) -> i32 {
    SAVED_JVM.store(vm as usize, std::sync::atomic::Ordering::SeqCst);
    0 // JNI_OK
}

// ── JVM health monitor ───────────────────────────────────────────────────────

fn jvm_monitor() {
    let mut failures = 0u32;
    while RUNNING.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_secs(2));
        // Attach check only — avoids JClass lifetime tying env to local jvm.
        let ok = if let Some(jvm) = get_jvm() {
            jvm.attach_current_thread_as_daemon().is_ok()
        } else {
            false
        };

        if ok {
            failures = 0;
        } else {
            failures += 1;
            warn!("JVM health check failed ({}/3)", failures);
        }

        if failures >= 3 {
            warn!("JVM appears dead — unloading client");
            unload_client();
            break;
        }
    }
}

// ── TCP command server ───────────────────────────────────────────────────────

fn command_server() {
    let listener = match TcpListener::bind("127.0.0.1:7878") {
        Ok(l) => l,
        Err(e) => {
            error!("TCP bind failed: {}", e);
            return;
        }
    };
    info!("Command server on 127.0.0.1:7878");

    for stream in listener.incoming() {
        if !RUNNING.load(Ordering::Relaxed) {
            break;
        }
        match stream {
            Ok(mut sock) => {
                let mut reader = BufReader::new(sock.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let line = line.trim().to_owned();
                info!("CMD: {}", line);

                let reply = dispatch_command(&line);
                let _ = writeln!(sock, "{}", reply);
            }
            Err(e) => error!("accept: {}", e),
        }
    }
}

fn dispatch_command(line: &str) -> String {
    if let Some(path) = line.strip_prefix("reload ") {
        match reload_client(path) {
            Ok(_) => "OK".into(),
            Err(e) => format!("ERR: {:#}", e),
        }
    } else if line == "unload" {
        unload_client();
        "OK".into()
    } else {
        "ERR: unknown command".into()
    }
}

// ── Library loading ──────────────────────────────────────────────────────────

fn reload_client(path: &str) -> anyhow::Result<()> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let tmp = format!("/tmp/anemoia_client_{}.so", ts);

    std::fs::copy(path, &tmp)?;
    info!("Copied client -> {}", tmp);

    // Expose repo root so the client can locate its scripts dir at runtime.
    // path = <repo>/target/{profile}/lib*.so  →  3 parents = repo root
    if let Ok(abs) = std::fs::canonicalize(path) {
        if let Some(root) = abs.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
            #[allow(deprecated)]
            std::env::set_var("ANEMOIA_ROOT", root);
        }
    }

    unload_client();
    load_client(&tmp)?;

    // Clean up temp file after symbols are pinned in memory.
    let cleanup = tmp.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));
        let _ = std::fs::remove_file(&cleanup);
    });

    Ok(())
}

fn load_client(path: &str) -> anyhow::Result<()> {
    let guard = LOADED_LIB.get().unwrap();
    let mut lock = guard.lock().unwrap();

    if lock.is_some() {
        return Ok(());
    }

    let lib = unsafe { Library::new(path) }?;

    unsafe {
        type InitFn = extern "C" fn(*mut libc::c_void);
        if let Ok(init) = lib.get::<Symbol<InitFn>>(b"initialize_client\0") {
            let jvm = SAVED_JVM.load(std::sync::atomic::Ordering::SeqCst) as *mut libc::c_void;
            init(jvm);
        }
    }

    *lock = Some(lib);
    info!("Client library loaded");
    Ok(())
}

fn unload_client() {
    if let Some(guard) = LOADED_LIB.get() {
        let mut lock = guard.lock().unwrap();
        if let Some(lib) = lock.take() {
            unsafe {
                if let Ok(cleanup) = lib.get::<Symbol<extern "C" fn()>>(b"cleanup_client\0") {
                    cleanup();
                }
            }
            drop(lib);
            info!("Client library unloaded");
        }
    }
}

// ── JVM acquisition via dlsym (no link-time libjvm dependency) ───────────────

fn get_jvm() -> Option<JavaVM> {
    type GetCreatedJVMs =
        unsafe extern "C" fn(*mut *mut RawJavaVM, jsize, *mut jsize) -> jint;

    unsafe {
        let sym = find_jni_symbol();
        if sym.is_null() {
            warn!("JNI_GetCreatedJavaVMs not found (RTLD_DEFAULT and libjvm.so both failed)");
            return None;
        }

        let get_vms: GetCreatedJVMs = std::mem::transmute(sym);
        let mut raw: *mut RawJavaVM = std::ptr::null_mut();
        let mut count: jsize = 0;

        if get_vms(&mut raw, 1, &mut count) != JNI_OK || count == 0 {
            return None;
        }

        JavaVM::from_raw(raw).ok()
    }
}

unsafe fn find_jni_symbol() -> *mut libc::c_void {
    let name = b"JNI_GetCreatedJavaVMs\0".as_ptr() as *const libc::c_char;

    let sym = libc::dlsym(libc::RTLD_DEFAULT, name);
    if !sym.is_null() {
        return sym;
    }

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
