// 系统集成：默认应用注册、文件管理器定位、Linux WebKit 兼容
/// 在系统文件管理器中定位并选中文件
use crate::i18n::Lang;
use std::path::Path;

// 默认应用注册只在 macOS / Windows 上实现，其他平台走不到这两个依赖
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::paths::config_dir;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::fs;

pub(crate) fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            let _ = open::that(parent);
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn register_as_default(_lang: Lang) {
    use std::process::Command;
    let marker = config_dir().join(".md-previewer-registered");
    if marker.exists() {
        return;
    }
    let _ = Command::new("swift")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(b"import Foundation\nimport CoreServices\nlet _ = LSSetDefaultRoleHandlerForContentType(\"net.daringfireball.markdown\" as NSString, .viewer, \"io.github.arnoldredman.mdpreviewer\" as NSString)\n");
            }
            child.wait()
        });
    let _ = fs::create_dir_all(marker.parent().unwrap());
    let _ = fs::write(&marker, "");
}

/// 给当前用户写右键「以 MD Previewer 编辑」和「打开方式」
/// Win8+ 不允许程序偷偷改默认应用，双击默认仍要用户在「打开方式」里勾选
#[cfg(target_os = "windows")]
pub(crate) fn register_as_default(lang: Lang) {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let marker_dir = config_dir();
    // 换过文件名：旧标记只注册过 Markdown 打开方式，不能挡住这次的右键菜单
    let marker = marker_dir.join(".md-previewer-shell-registered");
    if !marker.exists() {
        let exe_str = exe.to_string_lossy().to_string();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        // 调试构建没有壳 DLL，不能覆盖安装包已经写好的菜单
        let leave_existing = shell_dll_path(&exe).is_none() && com_handler_present(&hkcu);
        if !leave_existing {
            register_open_with(&hkcu, &exe_str);
            // 只有 COM 菜单写成功才记标记；经典菜单回退下次还能升级
            if register_context_menu(&hkcu, &exe, &exe_str, shell_menu_label(lang)) {
                let _ = fs::create_dir_all(&marker_dir);
                let _ = fs::write(&marker, "");
                notify_shell_associations_changed();
            }
        }
    }
}

#[cfg(target_os = "windows")]
const SHELL_CLSID: &str = "{7E2A9C14-5B6D-4E83-9F10-A1C3D5E7B902}";

#[cfg(target_os = "windows")]
const OPEN_WITH_EXTENSIONS: &[&str] = &[
    ".md",
    ".markdown",
    ".mdown",
    ".mkd",
    ".txt",
    ".json",
    ".toml",
    ".yaml",
    ".yml",
    ".log",
    ".env",
];

#[cfg(target_os = "windows")]
const SHELL_DLL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/md-previewer-shell.dll"));

#[cfg(target_os = "windows")]
fn shell_menu_label(lang: Lang) -> &'static str {
    match lang {
        Lang::Zh => "以 MD Previewer 编辑",
        Lang::En => "Edit with MD Previewer",
    }
}

#[cfg(target_os = "windows")]
fn register_open_with(hkcu: &winreg::RegKey, exe: &str) {
    let progid = "MDPreviewer.md";
    for ext in OPEN_WITH_EXTENSIONS {
        for path in [
            format!(r"Software\Classes\{ext}\OpenWithProgids"),
            format!(
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\{ext}\OpenWithProgids"
            ),
        ] {
            if let Ok((key, _)) = hkcu.create_subkey(&path) {
                let _ = key.set_value::<String, _>(progid, &String::new());
            }
        }
    }

    let progid_root = format!(r"Software\Classes\{progid}");
    if let Ok((key, _)) = hkcu.create_subkey(&progid_root) {
        let _ = key.set_value("", &"Markdown Document".to_string());
        let _ = key.set_value("FriendlyTypeName", &"Markdown Document".to_string());
    }
    if let Ok((key, _)) = hkcu.create_subkey(format!(r"{progid_root}\DefaultIcon")) {
        let _ = key.set_value("", &format!("\"{exe}\",0"));
    }
    if let Ok((key, _)) = hkcu.create_subkey(format!(r"{progid_root}\shell\open\command")) {
        let _ = key.set_value("", &format!("\"{exe}\" \"%1\""));
    }

    let Some(exe_name) = std::path::Path::new(exe)
        .file_name()
        .and_then(|name| name.to_str())
    else {
        return;
    };
    let app_root = format!(r"Software\Classes\Applications\{exe_name}");
    if let Ok((key, _)) = hkcu.create_subkey(&app_root) {
        let _ = key.set_value("FriendlyAppName", &"MD Previewer".to_string());
    }
    if let Ok((key, _)) = hkcu.create_subkey(format!(r"{app_root}\shell\open\command")) {
        let _ = key.set_value("", &format!("\"{exe}\" \"%1\""));
    }
    if let Ok((key, _)) = hkcu.create_subkey(format!(r"{app_root}\SupportedTypes")) {
        for ext in OPEN_WITH_EXTENSIONS {
            let _ = key.set_value::<String, _>(ext, &String::new());
        }
    }
}

#[cfg(target_os = "windows")]
fn register_context_menu(hkcu: &winreg::RegKey, exe: &Path, exe_str: &str, label: &str) -> bool {
    let Ok((verb, _)) = hkcu.create_subkey(r"Software\Classes\*\shell\MDPreviewer") else {
        return false;
    };

    // 有 DLL 就挂经典右键；没有就退回 command 项，且不拆掉已经写好的注册
    if let Some(dll) = shell_dll_path(exe) {
        let _ = verb.set_value("", &label.to_string());
        let _ = verb.set_value("NeverDefault", &String::new());
        let _ = verb.set_value("Icon", &format!("\"{exe_str}\",0"));
        let clsid_root = format!(r"Software\Classes\CLSID\{SHELL_CLSID}");
        if hkcu.create_subkey(&clsid_root).is_err() {
            write_static_verb(&verb, exe_str);
            return false;
        }
        if let Ok((key, _)) = hkcu.create_subkey(&clsid_root) {
            let _ = key.set_value("", &"MD Previewer".to_string());
            let _ = key.set_value("Exe", &exe_str.to_string());
            let _ = key.set_value("Title", &label.to_string());
        }
        if let Ok((key, _)) = hkcu.create_subkey(format!(r"{clsid_root}\InProcServer32")) {
            let _ = key.set_value("", &dll.to_string_lossy().to_string());
            let _ = key.set_value("ThreadingModel", &"Apartment".to_string());
        }
        let _ = verb.set_value("ExplorerCommandHandler", &SHELL_CLSID.to_string());
        let _ = verb.delete_subkey_all("command");
        return true;
    }

    if verb
        .get_value::<String, _>("ExplorerCommandHandler")
        .is_ok()
    {
        return true;
    }
    let _ = verb.set_value("", &label.to_string());
    let _ = verb.set_value("NeverDefault", &String::new());
    let _ = verb.set_value("Icon", &format!("\"{exe_str}\",0"));
    write_static_verb(&verb, exe_str);
    false
}

#[cfg(target_os = "windows")]
fn com_handler_present(hkcu: &winreg::RegKey) -> bool {
    hkcu.open_subkey(r"Software\Classes\*\shell\MDPreviewer")
        .and_then(|key| key.get_value::<String, _>("ExplorerCommandHandler"))
        .is_ok()
}

#[cfg(target_os = "windows")]
fn write_static_verb(verb: &winreg::RegKey, exe: &str) {
    if let Ok((key, _)) = verb.create_subkey(r"command") {
        let _ = key.set_value("", &format!("\"{exe}\" \"%1\""));
    }
}

#[cfg(target_os = "windows")]
fn shell_dll_path(exe: &Path) -> Option<std::path::PathBuf> {
    if let Some(dir) = exe.parent() {
        let beside = dir.join("md-previewer-shell.dll");
        if beside.is_file() {
            return Some(beside);
        }
    }
    if SHELL_DLL.is_empty() {
        return None;
    }
    let dest = config_dir().join("md-previewer-shell.dll");
    if fs::read(&dest).ok().as_deref() != Some(SHELL_DLL) {
        fs::create_dir_all(config_dir()).ok()?;
        fs::write(&dest, SHELL_DLL).ok()?;
    }
    Some(dest)
}

#[cfg(target_os = "windows")]
fn notify_shell_associations_changed() {
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    #[link(name = "shell32")]
    extern "system" {
        fn SHChangeNotify(
            event_id: i32,
            flags: u32,
            item1: *const std::ffi::c_void,
            item2: *const std::ffi::c_void,
        );
    }
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, 0, std::ptr::null(), std::ptr::null());
    }
}

#[cfg(all(test, target_os = "windows"))]
mod shell_registration_tests {
    use super::OPEN_WITH_EXTENSIONS;

    #[test]
    fn open_with_lists_markdown_and_env() {
        assert!(OPEN_WITH_EXTENSIONS.contains(&".md"));
        assert!(OPEN_WITH_EXTENSIONS.contains(&".env"));
        assert!(OPEN_WITH_EXTENSIONS.contains(&".txt"));
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn register_as_default(_lang: Lang) {}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn linux_webkit_compat_env(
    disable_dmabuf: Option<&str>,
    disable_compositing: Option<&str>,
    nvidia_driver_present: bool,
) -> Option<(&'static str, &'static str)> {
    if disable_dmabuf.is_some() || disable_compositing.is_some() || !nvidia_driver_present {
        return None;
    }

    Some(("WEBKIT_DISABLE_DMABUF_RENDERER", "1"))
}

#[cfg(target_os = "linux")]
pub(crate) fn apply_linux_webkit_compat_env() {
    let nvidia_driver_present = Path::new("/proc/driver/nvidia/version").exists();
    if let Some((key, value)) = linux_webkit_compat_env(
        std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER")
            .ok()
            .as_deref(),
        std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE")
            .ok()
            .as_deref(),
        nvidia_driver_present,
    ) {
        std::env::set_var(key, value);
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn apply_linux_webkit_compat_env() {}
