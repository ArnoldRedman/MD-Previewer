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

/// Windows: write HKCU registry so .md shows up in the "Open with" list, then
/// prompt the user once to finish wiring the default app (Win8+ blocks silent
/// default-handler changes — only the Settings app can confirm it).
#[cfg(target_os = "windows")]
pub(crate) fn register_as_default(_lang: Lang) {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let marker_dir = config_dir();
    let marker = marker_dir.join(".md-previewer-registered");
    if marker.exists() {
        return;
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let exe_str = exe.to_string_lossy().to_string();
    let progid = "MDPreviewer.md";
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    // Advertise MD Previewer as a choice for these extensions.
    for ext in [".md", ".markdown", ".mdown", ".mkd"] {
        let path = format!(r"Software\Classes\{ext}\OpenWithProgids");
        if let Ok((key, _)) = hkcu.create_subkey(&path) {
            let _ = key.set_value::<String, _>(progid, &String::new());
        }
    }

    // ProgID definition: description, icon, open command.
    let progid_root = format!(r"Software\Classes\{progid}");
    if let Ok((k, _)) = hkcu.create_subkey(&progid_root) {
        let _ = k.set_value("", &"Markdown Document".to_string());
        let _ = k.set_value("FriendlyTypeName", &"Markdown Document".to_string());
    }
    if let Ok((k, _)) = hkcu.create_subkey(format!(r"{progid_root}\DefaultIcon")) {
        let _ = k.set_value("", &format!("\"{exe_str}\",0"));
    }
    if let Ok((k, _)) = hkcu.create_subkey(format!(r"{progid_root}\shell\open\command")) {
        let _ = k.set_value("", &format!("\"{exe_str}\" \"%1\""));
    }

    // Applications\<exe-name> entry gives us a friendly label in the "Open with" menu.
    if let Some(exe_name) = exe.file_name().map(|n| n.to_string_lossy().to_string()) {
        let app_root = format!(r"Software\Classes\Applications\{exe_name}");
        if let Ok((k, _)) = hkcu.create_subkey(&app_root) {
            let _ = k.set_value("FriendlyAppName", &"MD Previewer".to_string());
        }
        if let Ok((k, _)) = hkcu.create_subkey(format!(r"{app_root}\shell\open\command")) {
            let _ = k.set_value("", &format!("\"{exe_str}\" \"%1\""));
        }
        if let Ok((k, _)) = hkcu.create_subkey(format!(r"{app_root}\SupportedTypes")) {
            for ext in [".md", ".markdown", ".mdown", ".mkd"] {
                let _ = k.set_value::<String, _>(ext, &String::new());
            }
        }
    }

    let _ = fs::create_dir_all(&marker_dir);
    let _ = fs::write(&marker, "");
    // Intentionally no dialog: users can pick MD Previewer via "Open with"
    // whenever they want, and Win10+ blocks silent default-handler changes
    // anyway — asking them to click through Settings on first launch is noise.
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
