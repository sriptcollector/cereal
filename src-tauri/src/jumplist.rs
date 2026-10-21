//! Windows taskbar jump list (right-click the taskbar icon): New note, Settings,
//! Restart, Quit. Each runs Cereal with `--task=<name>`; the running copy handles it.

use windows::core::{Interface, HSTRING, PCWSTR};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};
use windows::Win32::UI::Shell::{DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, ShellLink};
use windows::core::PROPVARIANT;
use windows::Win32::System::Com::StructuredStorage::InitPropVariantFromStringVector;

// PKEY_Title
const PKEY_TITLE: PROPERTYKEY = PROPERTYKEY { fmtid: windows::core::GUID::from_u128(0xF29F85E0_4FF9_1068_AB91_08002B27B3D9), pid: 2 };

pub fn set() {
    std::thread::spawn(|| unsafe {
        if let Err(e) = build() {
            log::warn!("jump list: {e}");
        }
    });
}

unsafe fn build() -> windows::core::Result<()> {
    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let exe = std::env::current_exe().unwrap_or_default();
    let list: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
    let mut min = 0u32;
    let _removed: IObjectArray = list.BeginList(&mut min)?;
    let tasks: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
    for (task, title) in [("new-note", "New note"), ("settings", "Settings"), ("restart", "Restart Cereal"), ("quit", "Quit Cereal")] {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(exe.as_os_str()))?;
        link.SetArguments(&HSTRING::from(format!("--task={task}")))?;
        link.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0)?;
        let props: IPropertyStore = link.cast()?;
        let t = HSTRING::from(title);
        let pv: PROPVARIANT = InitPropVariantFromStringVector(Some(&[PCWSTR(t.as_ptr())]))?;
        props.SetValue(&PKEY_TITLE, &pv)?;
        props.Commit()?;
        tasks.AddObject(&link)?;
    }
    let arr: IObjectArray = tasks.cast()?;
    list.AddUserTasks(&arr)?;
    list.CommitList()?;
    Ok(())
}
