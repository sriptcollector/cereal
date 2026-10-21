//! What this computer has: GPU vendor and memory, RAM, CPU threads.

use serde::Serialize;

#[derive(Clone, Debug, Serialize, Default)]
pub struct Hardware {
    pub gpu_vendor: String, // "nvidia" | "amd" | "intel" | "apple" | ""
    pub gpu_name: String,
    pub vram_mb: u64,
    pub ram_mb: u64,
    pub threads: usize,
}

impl Hardware {
    pub fn fingerprint(&self) -> String {
        format!("{}|{}|{}|{}", self.gpu_vendor, self.vram_mb / 1024, self.ram_mb / 1024, self.threads)
    }
}

pub fn detect() -> Hardware {
    let mut h = Hardware { threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4), ..Default::default() };
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Graphics::Dxgi::*;
        use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        if GlobalMemoryStatusEx(&mut m).is_ok() {
            h.ram_mb = m.ullTotalPhys / (1024 * 1024);
        }
        if let Ok(f) = CreateDXGIFactory1::<IDXGIFactory1>() {
            let mut i = 0;
            while let Ok(a) = f.EnumAdapters1(i) {
                i += 1;
                let Ok(d) = a.GetDesc1() else { continue };
                if (d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                    continue;
                }
                let vram = (d.DedicatedVideoMemory / (1024 * 1024)) as u64;
                let vendor = match d.VendorId {
                    0x10DE => "nvidia",
                    0x1002 => "amd",
                    0x8086 => "intel",
                    _ => "",
                };
                // Keep the adapter with the most dedicated memory.
                if vram > h.vram_mb {
                    h.vram_mb = vram;
                    h.gpu_vendor = vendor.into();
                    let n = d.Description.iter().position(|c| *c == 0).unwrap_or(d.Description.len());
                    h.gpu_name = String::from_utf16_lossy(&d.Description[..n]);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            h.ram_mb = String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().unwrap_or(0) / (1024 * 1024);
        }
        if cfg!(target_arch = "aarch64") {
            h.gpu_vendor = "apple".into();
            h.gpu_name = "Apple silicon".into();
            h.vram_mb = h.ram_mb; // unified memory
        }
    }
    h
}
