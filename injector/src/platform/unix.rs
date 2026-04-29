use anyhow::{bail, Result};
use log::info;
use proc_maps::get_process_maps;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    os::unix::net::UnixStream,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

const AGENT_ADDR: &str = "127.0.0.1:7878";
const AGENT_LIB_NAME: &str = "libagent_loader";

pub fn inject(pid: u32, agent_lib: &PathBuf, client_lib: &PathBuf) -> Result<()> {
    let agent_abs = agent_lib.canonicalize()?;
    let client_abs = client_lib.canonicalize()?;

    if !is_lib_mapped(pid, AGENT_LIB_NAME)? {
        info!("Loading {} into PID {} via JVM Attach API", agent_abs.display(), pid);
        jvm_attach_load(pid, &agent_abs)?;
        thread::sleep(Duration::from_millis(600));
    } else {
        info!("agent_loader already present in PID {}", pid);
    }

    let cmd = format!("reload {}", client_abs.display());
    info!("Sending: {}", cmd);
    let response = tcp_command(AGENT_ADDR, &cmd)?;
    info!("Agent: {}", response.trim());

    if response.starts_with("ERR") {
        bail!("Agent rejected command: {}", response.trim());
    }

    Ok(())
}

/// Load a native library into the JVM at `pid` using the HotSpot Attach API.
///
/// Protocol (same as com.sun.tools.attach.VirtualMachine.loadAgentPath):
///   1. Drop a trigger file so the JVM starts its attach listener on SIGQUIT.
///   2. Send SIGQUIT.
///   3. Wait for `/tmp/.java_pid<pid>` Unix socket to appear.
///   4. Send:  "1\0load\0<abs_path>\0true\0\0"
///   5. Read:  integer return code (0 = success).
fn jvm_attach_load(pid: u32, lib: &PathBuf) -> Result<()> {
    let socket_path = format!("/tmp/.java_pid{}", pid);
    let lib_str = lib
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 library path"))?;

    // If the attach socket doesn't already exist, trigger its creation.
    if !PathBuf::from(&socket_path).exists() {
        let trigger = trigger_attach_listener(pid)?;
        let result = wait_for_socket(&socket_path, Duration::from_secs(10));
        // Clean up trigger file only after socket appears (or timeout).
        let _ = fs::remove_file(&trigger);
        result?;
    }

    // Connect to the JVM attach socket.
    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| anyhow::anyhow!("connect to attach socket {}: {}", socket_path, e))?;

    // Send: protocol_version\0 "load"\0 <abs_path>\0 "true"\0 ""\0
    let mut msg = Vec::new();
    for part in &["1", "load", lib_str, "true", ""] {
        msg.extend_from_slice(part.as_bytes());
        msg.push(0);
    }
    stream.write_all(&msg)?;
    stream.flush()?;

    // Response: "<return_code>\n[output]\n"
    let mut resp = String::new();
    stream.read_to_string(&mut resp)?;

    let code: i32 = resp
        .lines()
        .next()
        .and_then(|l| l.trim().parse().ok())
        .unwrap_or(-1);

    if code != 0 {
        bail!(
            "JVM attach load returned code {}: {}",
            code,
            resp.trim()
        );
    }

    info!("JVM attach load OK");
    Ok(())
}

/// Create the trigger file and send SIGQUIT so the JVM starts its attach listener.
/// Returns the trigger file path — caller must delete it AFTER the socket appears.
fn trigger_attach_listener(pid: u32) -> Result<PathBuf> {
    let trigger_name = format!(".attach_pid{}", pid);

    // Try the process's working directory first (HotSpot checks there).
    let trigger_path = if let Ok(cwd) = fs::read_link(format!("/proc/{}/cwd", pid)) {
        let p = cwd.join(&trigger_name);
        if fs::write(&p, "").is_ok() {
            p
        } else {
            let p = PathBuf::from(format!("/tmp/{}", trigger_name));
            fs::write(&p, "")?;
            p
        }
    } else {
        let p = PathBuf::from(format!("/tmp/{}", trigger_name));
        fs::write(&p, "")?;
        p
    };

    info!("Created attach trigger: {}", trigger_path.display());

    // SIGQUIT tells HotSpot's signal thread to check for the trigger file
    // and start the attach listener.
    let ret = unsafe { libc::kill(pid as libc::pid_t, libc::SIGQUIT) };
    if ret != 0 {
        bail!(
            "kill({}, SIGQUIT) failed: {}",
            pid,
            std::io::Error::last_os_error()
        );
    }

    // Do NOT remove the file here — the JVM reads it asynchronously.
    Ok(trigger_path)
}

fn wait_for_socket(path: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if PathBuf::from(path).exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!(
        "JVM attach socket {} did not appear within {:?}",
        path,
        timeout
    )
}

fn is_lib_mapped(pid: u32, name: &str) -> Result<bool> {
    let maps = get_process_maps(pid as i32)?;
    Ok(maps.iter().any(|m| {
        m.filename()
            .and_then(|p| p.to_str())
            .map(|s| s.contains(name))
            .unwrap_or(false)
    }))
}

fn tcp_command(addr: &str, cmd: &str) -> Result<String> {
    let mut stream = TcpStream::connect(addr)?;
    writeln!(stream, "{}", cmd)?;
    stream.flush()?;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}
