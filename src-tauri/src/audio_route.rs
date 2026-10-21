//! Which speakers/headset the call is playing to (Windows). Granola follows the output
//! device Teams renders to; Cereal does this for every call app, so the other side is
//! recorded even when the call plays on a headset that isn't the default device.

use windows::core::{Interface, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::{
    eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};

fn exe_of(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// The friendly name of the output device a call app is actively playing to, if any.
pub fn call_output_device(is_call_app: impl Fn(&str) -> bool) -> Option<String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let devs = en.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).ok()?;
        for i in 0..devs.GetCount().ok()? {
            let Ok(dev) = devs.Item(i) else { continue };
            let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { continue };
            let Ok(sessions) = mgr.GetSessionEnumerator() else { continue };
            for k in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(ctl) = sessions.GetSession(k) else { continue };
                if ctl.GetState().map(|s| s != AudioSessionStateActive).unwrap_or(true) {
                    continue;
                }
                let Ok(c2) = ctl.cast::<IAudioSessionControl2>() else { continue };
                let Ok(pid) = c2.GetProcessId() else { continue };
                let Some(exe) = exe_of(pid) else { continue };
                if is_call_app(&exe) {
                    let store = dev.OpenPropertyStore(STGM_READ).ok()?;
                    let v = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
                    let name = v.to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
        None
    }
}
