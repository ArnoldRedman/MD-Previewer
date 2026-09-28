// Windows 11 的新右键不显示普通 shell\command，只认 IExplorerCommand
// 资源管理器会把这个 DLL 载进自己的进程，这里不能 panic

use std::ffi::c_void;

use windows::core::{w, GUID, HRESULT, PCWSTR, PWSTR};
use windows::Win32::{
    Foundation::{
        CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, ERROR_SUCCESS, E_NOTIMPL, E_OUTOFMEMORY,
        E_POINTER, S_FALSE,
    },
    System::{
        Com::{CoTaskMemAlloc, CoTaskMemFree, IBindCtx, IClassFactory, IClassFactory_Impl},
        Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ},
        SystemServices::SFGAO_FOLDER,
    },
    UI::{
        Shell::{
            IEnumExplorerCommand, IExplorerCommand, IExplorerCommand_Impl, IShellItem,
            IShellItemArray, ShellExecuteW, ECF_DEFAULT, ECS_ENABLED, ECS_HIDDEN,
            SIGDN_FILESYSPATH,
        },
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};
use windows_core::{implement, Interface};

// 与 src/platform.rs、安装脚本里的 CLSID 保持一致
const CLSID_COMMAND: GUID = GUID::from_u128(0x7E2A9C14_5B6D_4E83_9F10_A1C3D5E7B902);
const CLSID_KEY: &str = "Software\\Classes\\CLSID\\{7E2A9C14-5B6D-4E83-9F10-A1C3D5E7B902}";
// ponytail: 一次最多开 32 个，避免误选一大片文件时把进程打爆
const MAX_OPEN: u32 = 32;

#[implement(IExplorerCommand)]
struct Command;

impl IExplorerCommand_Impl for Command_Impl {
    fn GetTitle(&self, _: windows::core::Ref<IShellItemArray>) -> windows::core::Result<PWSTR> {
        let title = reg_sz(CLSID_KEY, "Title").unwrap_or_else(|| "Edit with MD Previewer".into());
        alloc_pwstr(&title)
    }

    fn GetIcon(&self, _: windows::core::Ref<IShellItemArray>) -> windows::core::Result<PWSTR> {
        let Some(exe) = reg_sz(CLSID_KEY, "Exe") else {
            return alloc_pwstr("");
        };
        alloc_pwstr(&format!("{exe},0"))
    }

    fn GetToolTip(
        &self,
        items: windows::core::Ref<IShellItemArray>,
    ) -> windows::core::Result<PWSTR> {
        self.GetTitle(items)
    }

    fn GetCanonicalName(&self) -> windows::core::Result<GUID> {
        Ok(CLSID_COMMAND)
    }

    fn GetState(
        &self,
        items: windows::core::Ref<IShellItemArray>,
        _: windows::core::BOOL,
    ) -> windows::core::Result<u32> {
        if folders_only(items) {
            Ok(ECS_HIDDEN.0 as u32)
        } else {
            Ok(ECS_ENABLED.0 as u32)
        }
    }

    fn Invoke(
        &self,
        items: windows::core::Ref<IShellItemArray>,
        _: windows::core::Ref<IBindCtx>,
    ) -> windows::core::Result<()> {
        let Some(exe) = reg_sz(CLSID_KEY, "Exe") else {
            return Ok(());
        };
        let Some(items) = items.as_ref() else {
            return Ok(());
        };
        let count = unsafe { items.GetCount() }.unwrap_or(0).min(MAX_OPEN);
        let exe_wide = wide(&exe);
        for index in 0..count {
            let Ok(item) = (unsafe { items.GetItemAt(index) }) else {
                continue;
            };
            if is_folder(&item) {
                continue;
            }
            let Ok(name) = (unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }) else {
                continue;
            };
            let Some(path) = owned_string(name) else {
                continue;
            };
            let params = wide(&format!("\"{path}\""));
            unsafe {
                ShellExecuteW(
                    None,
                    w!("open"),
                    PCWSTR(exe_wide.as_ptr()),
                    PCWSTR(params.as_ptr()),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
            }
        }
        Ok(())
    }

    fn GetFlags(&self) -> windows::core::Result<u32> {
        Ok(ECF_DEFAULT.0 as u32)
    }

    fn EnumSubCommands(&self) -> windows::core::Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: windows::core::Ref<windows::core::IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        if iid.is_null() || object.is_null() {
            return Err(E_POINTER.into());
        }
        let command: IExplorerCommand = Command.into();
        unsafe { command.query(iid, object).ok() }
    }

    fn LockServer(&self, _: windows::core::BOOL) -> windows::core::Result<()> {
        Ok(())
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    unsafe {
        if *rclsid != CLSID_COMMAND {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        *ppv = std::ptr::null_mut();
        let factory: IClassFactory = Factory.into();
        factory.query(riid, ppv)
    }
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    // 返回 S_FALSE，避免资源管理器在菜单还挂着时卸掉 DLL
    S_FALSE
}

fn folders_only(items: windows::core::Ref<IShellItemArray>) -> bool {
    let Some(items) = items.as_ref() else {
        return false;
    };
    let Ok(count) = (unsafe { items.GetCount() }) else {
        return false;
    };
    if count == 0 || count > MAX_OPEN {
        return false;
    }
    for index in 0..count {
        let Ok(item) = (unsafe { items.GetItemAt(index) }) else {
            return false;
        };
        if !is_folder(&item) {
            return false;
        }
    }
    true
}

fn is_folder(item: &IShellItem) -> bool {
    unsafe { item.GetAttributes(SFGAO_FOLDER) }
        .map(|flags| flags.contains(SFGAO_FOLDER))
        .unwrap_or(false)
}

fn owned_string(value: PWSTR) -> Option<String> {
    if value.0.is_null() {
        return None;
    }
    let text = unsafe { value.to_string().ok() };
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    text
}

fn alloc_pwstr(value: &str) -> windows::core::Result<PWSTR> {
    let data = wide(value);
    let bytes = data.len() * 2;
    let mem = unsafe { CoTaskMemAlloc(bytes) } as *mut u16;
    if mem.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), mem, data.len());
    }
    Ok(PWSTR(mem))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn reg_sz(subkey: &str, name: &str) -> Option<String> {
    let sub = wide(subkey);
    let value = wide(name);
    let mut size = 0u32;
    unsafe {
        if RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(sub.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        ) != ERROR_SUCCESS
            || size < 2
        {
            return None;
        }
        let mut buf = vec![0u16; (size as usize) / 2];
        if RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(sub.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let end = buf.iter().position(|unit| *unit == 0).unwrap_or(buf.len());
        String::from_utf16(&buf[..end]).ok()
    }
}
