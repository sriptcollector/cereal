//! Local engine servers (whisper-server, llama-server) bound to 127.0.0.1 only.
//! On Windows they sit in a kill-on-close job, so they never outlive the app.

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct Server {
    child: Child,
    pub port: u16,
    /// What this server was started with (engine id + model path), to know when to restart.
    pub key: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).unwrap_or(18_775)
}

#[cfg(windows)]
mod job {
    use once_cell::sync::OnceCell;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::*;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

    struct Job(HANDLE);
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}
    static JOB: OnceCell<Option<Job>> = OnceCell::new();

    pub fn assign(pid: u32) {
        let job = JOB.get_or_init(|| unsafe {
            let h = CreateJobObjectW(None, None).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(h, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32).ok()?;
            Some(Job(h))
        });
        if let Some(Job(h)) = job {
            unsafe {
                if let Ok(p) = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid) {
                    let _ = AssignProcessToJobObject(*h, p);
                }
            }
        }
    }
}

/// A command that never opens a console window.
pub fn quiet_command(exe: &Path) -> Command {
    let mut cmd = Command::new(exe);
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Ties a child process's life to the app's (Windows job object).
pub fn adopt(child: &Child) {
    #[cfg(windows)]
    job::assign(child.id());
    let _ = child;
}

pub fn spawn_server(exe: &Path, args: &[String], key: String, log_path: &Path) -> Result<Server> {
    let port = free_port();
    let mut cmd = quiet_command(exe);
    cmd.args(args).arg("--host").arg("127.0.0.1").arg("--port").arg(port.to_string());
    let log = std::fs::File::create(log_path)?;
    cmd.stdout(Stdio::from(log.try_clone()?)).stderr(Stdio::from(log)).stdin(Stdio::null());
    let child = cmd.spawn().with_context(|| format!("start {}", exe.display()))?;
    adopt(&child);
    Ok(Server { child, port, key })
}

pub async fn wait_ready(base: &str, path: &str, timeout: Duration) -> Result<()> {
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(r) = client.get(format!("{base}{path}")).timeout(Duration::from_secs(2)).send().await {
            if r.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(anyhow!("engine did not become ready"))
}

pub fn threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8)
}
