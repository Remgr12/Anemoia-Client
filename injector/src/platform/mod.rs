use anyhow::Result;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod unix;

pub fn inject(pid: u32, agent_lib: &PathBuf, client_lib: &PathBuf) -> Result<()> {
    #[cfg(target_os = "linux")]
    return unix::inject(pid, agent_lib, client_lib);

    #[cfg(not(target_os = "linux"))]
    anyhow::bail!("Unsupported platform — only Linux is implemented");
}
