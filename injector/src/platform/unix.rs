use anyhow::Result;
use log::info;
use proc_maps::get_process_maps;
use ptrace_inject::{Injector, Process};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

const AGENT_ADDR: &str = "127.0.0.1:7878";
const AGENT_LIB_NAME: &str = "libagent_loader";

pub fn inject(pid: u32, agent_lib: &PathBuf, client_lib: &PathBuf) -> Result<()> {
    let agent_abs = agent_lib.canonicalize()?;
    let client_abs = client_lib.canonicalize()?;

    if !is_lib_mapped(pid, AGENT_LIB_NAME)? {
        info!("Injecting {} into PID {}", agent_abs.display(), pid);
        // ptrace-inject uses eyre — map to anyhow.
        let proc = Process::get(pid).map_err(|e| anyhow::anyhow!("{}", e))?;
        let mut injector = Injector::attach(proc).map_err(|e| anyhow::anyhow!("{}", e))?;
        injector.inject(&agent_abs).map_err(|e| anyhow::anyhow!("{}", e))?;
        // Give the ctor time to bind the TCP server before we connect.
        thread::sleep(Duration::from_millis(600));
    } else {
        info!("agent_loader already present in PID {}", pid);
    }

    let cmd = format!("reload {}", client_abs.display());
    info!("Sending: {}", cmd);
    let response = tcp_command(AGENT_ADDR, &cmd)?;
    info!("Agent: {}", response.trim());

    if response.starts_with("ERR") {
        anyhow::bail!("Agent rejected command: {}", response.trim());
    }

    Ok(())
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
