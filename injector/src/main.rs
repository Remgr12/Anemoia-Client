mod platform;

use anyhow::{bail, Result};
use log::{error, info};
use simplelog::{ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode};
use std::path::PathBuf;
use sysinfo::System;

fn main() -> Result<()> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])?;

    let args: Vec<String> = std::env::args().collect();
    let pid = parse_pid(&args)?;

    let agent_lib = resolve_sibling_lib("libagent_loader.so")?;
    let client_lib = resolve_sibling_lib("libanemoia_client.so")?;

    info!("Target PID: {}", pid);

    if let Err(e) = platform::inject(pid, &agent_lib, &client_lib) {
        error!("Injection failed: {:#}", e);
        std::process::exit(1);
    }

    info!("Injection complete");
    Ok(())
}

fn parse_pid(args: &[String]) -> Result<u32> {
    // Accept: anemoia-inject <pid>  or  anemoia-inject --pid <pid>
    if let Some(pos) = args.iter().position(|a| a == "--pid") {
        let raw = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("--pid requires a value"))?;
        return Ok(raw.parse()?);
    }

    if args.len() == 2 {
        if let Ok(pid) = args[1].parse::<u32>() {
            return Ok(pid);
        }
    }

    // No PID given — auto-detect.
    find_minecraft_pid().ok_or_else(|| anyhow::anyhow!("No Minecraft/Java process found"))
}

fn resolve_sibling_lib(name: &str) -> Result<PathBuf> {
    let dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot resolve exe directory"))?
        .to_path_buf();

    let path = dir.join(name);
    if !path.exists() {
        bail!("Library not found: {}\n  Expected at: {}", name, path.display());
    }
    Ok(path)
}

fn find_minecraft_pid() -> Option<u32> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let processes: Vec<_> = sys
        .processes()
        .iter()
        .filter_map(|(pid, proc)| {
            let name = proc.name().to_string_lossy().to_lowercase();
            let cmd = proc
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");

            if name.contains("java") || cmd.contains("minecraft") {
                Some((pid.as_u32(), cmd.contains("minecraft")))
            } else {
                None
            }
        })
        .collect();

    processes
        .iter()
        .find(|(_, is_mc)| *is_mc)
        .or_else(|| processes.first())
        .map(|(pid, _)| *pid)
}
