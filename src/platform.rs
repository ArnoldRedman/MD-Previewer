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
    // 换过标记名：v2 要跑一次历史遗留清理（旧版本按 exe 文件名注册过好几份「打开方式」入口）
    let marker = marker_dir.join(".md-previewer-shell-registered-v2");
    if marker.exists() {
        return;
    }
    // 去掉 verbatim 前缀再写进注册表，比对时大小写不敏感由 paths_equal 负责
    let exe_str = exe.to_string_lossy().to_string();
    let exe_str = exe_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&exe_str)
        .to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    // 清理跟接管判断无关，先做：多个同名入口、指向已删除 exe 的入口先收掉
    prune_legacy_associations(&hkcu);
    // 调试构建没有壳 DLL，不能覆盖安装包已经写好的菜单；
    // 已经指向另一个还活着的 md-previewer.exe 时也不接管（装了正式版的人随手跑免安装版，
    // 不应该把关联和右键菜单抢到临时目录去）
    let leave_existing = shell_dll_path(&exe).is_none() && com_handler_present(&hkcu);
    if !leave_existing && should_take_over(&hkcu, &exe_str) {
        register_open_with(&hkcu, &exe_str);
        // 只有 COM 菜单写成功才记标记；经典菜单回退下次还能升级
        if register_context_menu(&hkcu, &exe, &exe_str, shell_menu_label(lang)) {
            let _ = fs::create_dir_all(&marker_dir);
            let _ = fs::write(&marker, "");
            let _ = fs::remove_file(marker_dir.join(".md-previewer-shell-registered"));
            notify_shell_associations_changed();
        }
    }
}

#[cfg(target_os = "windows")]
const SHELL_CLSID: &str = "{7E2A9C14-5B6D-4E83-9F10-A1C3D5E7B902}";

/// 「打开方式」里只允许出现一份 MD Previewer：Applications 的 key 名固定用这个，
/// 不跟着当前运行的 exe 文件名走（否则安装版、免安装版、dev 构建各算一个应用）
#[cfg(target_os = "windows")]
const APP_EXE_NAME: &str = "md-previewer.exe";

#[cfg(target_os = "windows")]
const APP_FRIENDLY_NAME: &str = "MD Previewer";

/// 跟安装脚本里 `$progidDescription` 保持同一份文案
#[cfg(target_os = "windows")]
const PROGID_DESCRIPTION: &str = "MD Previewer Markdown Document";

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
        let _ = key.set_value("", &PROGID_DESCRIPTION.to_string());
        let _ = key.set_value("FriendlyTypeName", &PROGID_DESCRIPTION.to_string());
    }
    if let Ok((key, _)) = hkcu.create_subkey(format!(r"{progid_root}\DefaultIcon")) {
        let _ = key.set_value("", &format!("\"{exe}\",0"));
    }
    if let Ok((key, _)) = hkcu.create_subkey(format!(r"{progid_root}\shell\open\command")) {
        let _ = key.set_value("", &format!("\"{exe}\" \"%1\""));
    }

    let app_root = format!(r"Software\Classes\Applications\{APP_EXE_NAME}");
    if let Ok((key, _)) = hkcu.create_subkey(&app_root) {
        let _ = key.set_value("FriendlyAppName", &APP_FRIENDLY_NAME.to_string());
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

/// 是否由当前进程接管注册：已经指向另一个「还活着」的 exe 时就不动，
/// 指向已删除的路径（绿色版换位置、临时目录跑过）则接管，自己会修好
#[cfg(target_os = "windows")]
fn should_take_over(hkcu: &winreg::RegKey, exe: &str) -> bool {
    match registered_command_target(hkcu) {
        Some(target) => paths_equal(&target, exe) || !target.is_file(),
        None => true,
    }
}

#[cfg(target_os = "windows")]
fn registered_command_target(hkcu: &winreg::RegKey) -> Option<std::path::PathBuf> {
    let command = hkcu
        .open_subkey(format!(
            r"Software\Classes\Applications\{APP_EXE_NAME}\shell\open\command"
        ))
        .and_then(|key| key.get_value::<String, _>(""))
        .ok()?;
    parse_command_target(&command)
}

/// 从 `"C:\path\app.exe" "%1"` 里取出 exe 路径
#[cfg(target_os = "windows")]
fn parse_command_target(command: &str) -> Option<std::path::PathBuf> {
    let trimmed = command.trim();
    if let Some(rest) = trimmed.strip_prefix('"') {
        return rest
            .find('"')
            .map(|end| std::path::PathBuf::from(&rest[..end]));
    }
    trimmed
        .split_whitespace()
        .next()
        .map(std::path::PathBuf::from)
}

/// Windows 路径大小写不敏感；`current_exe()` 有时会带回 verbatim 前缀（\\?\），
/// 直接比字符串会误判成「已经是别的 exe 在管」，于是本次不接管也修不了死链
#[cfg(target_os = "windows")]
fn paths_equal(a: &Path, b: &str) -> bool {
    normalize_exe_path(&a.to_string_lossy()) == normalize_exe_path(b)
}

#[cfg(target_os = "windows")]
fn normalize_exe_path(value: &str) -> String {
    value.strip_prefix(r"\\?\").unwrap_or(value).to_lowercase()
}

/// 把历史遗留的「打开方式」入口收干净：
/// 旧版本按 exe 文件名写过 `Applications\<名字>`，Windows 又把每个用过的名字记进
/// `FileExts\<ext>\OpenWithList`，两边都不清理，于是右键属性里的「更改默认打开方式」
/// 会堆出好几行同名项（卸载也带不走）。只认自己人：FriendlyAppName 是 MD Previewer 才动
#[cfg(target_os = "windows")]
fn prune_legacy_associations(hkcu: &winreg::RegKey) {
    use winreg::enums::{KEY_READ, KEY_WRITE};

    let mut stale_exe_names: Vec<String> = Vec::new();
    if let Ok(apps) = hkcu.open_subkey(r"Software\Classes\Applications") {
        for name in apps.enum_keys().flatten() {
            if name.eq_ignore_ascii_case(APP_EXE_NAME) {
                continue;
            }
            let Ok(key) = apps.open_subkey(&name) else {
                continue;
            };
            let friendly = key
                .get_value::<String, _>("FriendlyAppName")
                .unwrap_or_default();
            if friendly == APP_FRIENDLY_NAME {
                stale_exe_names.push(name);
            }
        }
    }
    if !stale_exe_names.is_empty() {
        for name in &stale_exe_names {
            if let Ok(apps) =
                hkcu.open_subkey_with_flags(r"Software\Classes\Applications", KEY_READ | KEY_WRITE)
            {
                let _ = apps.delete_subkey_all(name);
            }
        }
        prune_open_with_list(hkcu, &stale_exe_names);
    }
    prune_legacy_progids(hkcu);
    prune_dead_shell_entries(hkcu);
}

/// 从每个扩展的 `OpenWithList` 里删掉这些 exe 名（对话框的候选就来自这里）
#[cfg(target_os = "windows")]
fn prune_open_with_list(hkcu: &winreg::RegKey, stale: &[String]) {
    use winreg::enums::{KEY_READ, KEY_WRITE};

    for ext in OPEN_WITH_EXTENSIONS {
        let path = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\{ext}\OpenWithList"
        );
        let Ok(key) = hkcu.open_subkey_with_flags(&path, KEY_READ | KEY_WRITE) else {
            continue;
        };
        let mru = key.get_value::<String, _>("MRUList").unwrap_or_default();
        let mut letters: Vec<String> = Vec::new();
        let mut entries: Vec<(String, String)> = Vec::new();
        for value_name in key.enum_values().flatten().map(|(name, _)| name) {
            if value_name == "MRUList" || value_name.len() != 1 {
                continue;
            }
            letters.push(value_name.clone());
            if let Ok(value) = key.get_value::<String, _>(&value_name) {
                entries.push((value_name, value));
            }
        }
        let (new_mru, kept) = renumber_open_with_list(&mru, &entries, stale);
        if kept.len() == entries.len() {
            continue;
        }
        for letter in &letters {
            let _ = key.delete_value(letter);
        }
        if new_mru.is_empty() {
            drop(key);
            let _ = hkcu.delete_subkey_all(&path);
            continue;
        }
        for (letter, name) in &kept {
            let _ = key.set_value::<String, _>(letter, name);
        }
        let _ = key.set_value("MRUList", &new_mru);
    }
}

/// 按原先后顺序剔掉要删的名字，剩下的重新编成 a、b、c…
/// 返回 (新 MRUList, 保留的 (字母, exe 名))；纯函数，单测直接管逻辑
#[cfg(target_os = "windows")]
fn renumber_open_with_list(
    mru: &str,
    entries: &[(String, String)],
    stale: &[String],
) -> (String, Vec<(String, String)>) {
    let kept: Vec<String> = mru
        .chars()
        .filter_map(|letter| {
            let letter = letter.to_string();
            let (_, name) = entries.iter().find(|(entry, _)| *entry == letter)?;
            if stale
                .iter()
                .any(|dropped| dropped.eq_ignore_ascii_case(name))
            {
                return None;
            }
            Some(name.clone())
        })
        .collect();
    let mut new_mru = String::new();
    let mut renumbered = Vec::new();
    for (index, name) in kept.into_iter().enumerate() {
        let letter = (b'a' + index as u8) as char;
        new_mru.push(letter);
        renumbered.push((letter.to_string(), name));
    }
    (new_mru, renumbered)
}

/// 指向已删除文件的注册也要收：绿色版换过目录、临时目录里跑过一次，都会留下
/// 「以 MD Previewer 编辑」菜单和 ProgID 命令指向一个不存在的 exe，收掉后本次就能重新注册
#[cfg(target_os = "windows")]
fn prune_dead_shell_entries(hkcu: &winreg::RegKey) {
    use winreg::enums::{KEY_READ, KEY_WRITE};

    // 右键菜单：COM 版看 CLSID 里的 DLL，经典版看 verb 自己的命令
    let verb_path = r"Software\Classes\*\shell\MDPreviewer";
    if let Ok(verb) = hkcu.open_subkey_with_flags(verb_path, KEY_READ | KEY_WRITE) {
        let handler = verb
            .get_value::<String, _>("ExplorerCommandHandler")
            .unwrap_or_default();
        if !handler.is_empty() {
            let clsid_path = format!(r"Software\Classes\CLSID\{handler}");
            let dll = hkcu
                .open_subkey(format!(r"{clsid_path}\InProcServer32"))
                .and_then(|key| key.get_value::<String, _>(""))
                .ok();
            if let Some(dll) = dll {
                let dll = dll.trim_matches('"').to_string();
                if !Path::new(&dll).is_file() {
                    let _ = verb.delete_value("ExplorerCommandHandler");
                    if let Ok(classes) =
                        hkcu.open_subkey_with_flags(r"Software\Classes\CLSID", KEY_READ | KEY_WRITE)
                    {
                        let _ = classes.delete_subkey_all(&handler);
                    }
                }
            }
        } else if let Some(target) = verb
            .open_subkey("command")
            .and_then(|key| key.get_value::<String, _>(""))
            .ok()
            .as_deref()
            .and_then(parse_command_target)
        {
            if !target.is_file() {
                if let Ok(classes) =
                    hkcu.open_subkey_with_flags(r"Software\Classes", KEY_READ | KEY_WRITE)
                {
                    let _ = classes.delete_subkey_all(r"*\shell\MDPreviewer");
                }
            }
        }
    }

    // ProgID 命令指向已删除的 exe：连它在各扩展候选列表里的挂名一起收掉
    let progid = "MDPreviewer.md";
    let target = hkcu
        .open_subkey(format!(r"Software\Classes\{progid}\shell\open\command"))
        .and_then(|key| key.get_value::<String, _>(""))
        .ok()
        .as_deref()
        .and_then(parse_command_target);
    if !matches!(target, Some(path) if path.is_file()) {
        if let Ok(classes) = hkcu.open_subkey_with_flags(r"Software\Classes", KEY_READ | KEY_WRITE)
        {
            let _ = classes.delete_subkey_all(progid);
        }
        for ext in OPEN_WITH_EXTENSIONS {
            for path in [
                format!(r"Software\Classes\{ext}\OpenWithProgids"),
                format!(
                    r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\{ext}\OpenWithProgids"
                ),
            ] {
                if let Ok(key) = hkcu.open_subkey_with_flags(&path, KEY_READ | KEY_WRITE) {
                    let _ = key.delete_value(progid);
                }
            }
        }
    }
}

/// 旧版本用过的 ProgID（MDPreviewer.txt 之类）也会在 OpenWithProgids 里留一行同名候选
#[cfg(target_os = "windows")]
fn prune_legacy_progids(hkcu: &winreg::RegKey) {
    use winreg::enums::{KEY_READ, KEY_WRITE};

    let Ok(classes) = hkcu.open_subkey(r"Software\Classes") else {
        return;
    };
    let stale: Vec<String> = classes
        .enum_keys()
        .flatten()
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("mdpreviewer.") && lower != "mdpreviewer.md"
        })
        .collect();
    if stale.is_empty() {
        return;
    }
    for ext in OPEN_WITH_EXTENSIONS {
        for path in [
            format!(r"Software\Classes\{ext}\OpenWithProgids"),
            format!(
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\{ext}\OpenWithProgids"
            ),
        ] {
            let Ok(key) = hkcu.open_subkey_with_flags(&path, KEY_READ | KEY_WRITE) else {
                continue;
            };
            for name in &stale {
                let _ = key.delete_value(name);
            }
        }
    }
    if let Ok(classes) = hkcu.open_subkey_with_flags(r"Software\Classes", KEY_READ | KEY_WRITE) {
        for name in &stale {
            let _ = classes.delete_subkey_all(name);
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
        let _ = fs::create_dir_all(config_dir());
        // Explorer 可能正把旧那份当右键菜单模块加载着，覆盖写会失败；
        // 这时就用现成的（老一点的菜单 DLL 也能跑），否则会连带跳过整个注册，
        // 菜单和关联就永远停在旧路径上修不好
        if fs::write(&dest, SHELL_DLL).is_err() {
            return dest.is_file().then_some(dest);
        }
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
    use super::{parse_command_target, paths_equal, renumber_open_with_list, OPEN_WITH_EXTENSIONS};
    use std::path::Path;

    #[test]
    fn open_with_lists_markdown_and_env() {
        assert!(OPEN_WITH_EXTENSIONS.contains(&".md"));
        assert!(OPEN_WITH_EXTENSIONS.contains(&".env"));
        assert!(OPEN_WITH_EXTENSIONS.contains(&".txt"));
    }

    #[test]
    fn open_with_list_drops_legacy_exe_name_and_renumbers() {
        // 真实现场：b = md-previewer.exe、c = MD-Previewer-windows-x64.exe
        let entries = vec![
            ("b".to_string(), "md-previewer.exe".to_string()),
            ("c".to_string(), "MD-Previewer-windows-x64.exe".to_string()),
        ];
        let (mru, kept) = renumber_open_with_list(
            "bc",
            &entries,
            &["MD-Previewer-windows-x64.exe".to_string()],
        );
        assert_eq!(mru, "a");
        assert_eq!(
            kept,
            vec![("a".to_string(), "md-previewer.exe".to_string())]
        );
    }

    #[test]
    fn open_with_list_keeps_other_apps_and_order() {
        let entries = vec![
            ("a".to_string(), "rider64.exe".to_string()),
            ("b".to_string(), "md-previewer.exe".to_string()),
            ("c".to_string(), "MD-Previewer-windows-x64.exe".to_string()),
        ];
        let (mru, kept) = renumber_open_with_list(
            "bca",
            &entries,
            &["MD-Previewer-windows-x64.exe".to_string()],
        );
        // 顺序照旧（b、a），字母重新编号
        assert_eq!(mru, "ab");
        assert_eq!(kept[0].1, "md-previewer.exe");
        assert_eq!(kept[1].1, "rider64.exe");
    }

    #[test]
    fn open_with_list_drops_everything_when_nothing_left() {
        let entries = vec![("a".to_string(), "md-previewer.exe".to_string())];
        let (mru, kept) = renumber_open_with_list("a", &entries, &["md-previewer.exe".to_string()]);
        assert!(mru.is_empty() && kept.is_empty());
    }

    #[test]
    fn exe_path_comparison_ignores_verbatim_prefix_and_case() {
        assert!(paths_equal(
            Path::new(r"\\?\D:\tools\MD-Previewer.exe"),
            r"d:\tools\md-previewer.exe"
        ));
        assert!(!paths_equal(
            Path::new(r"D:\tools\md-previewer.exe"),
            r"D:\tools\other.exe"
        ));
    }

    #[test]
    fn command_target_handles_quoted_and_bare_paths() {
        assert_eq!(
            parse_command_target("\"C:\\app dir\\md-previewer.exe\" \"%1\"")
                .unwrap()
                .to_string_lossy(),
            "C:\\app dir\\md-previewer.exe"
        );
        assert_eq!(
            parse_command_target("C:\\app\\md-previewer.exe %1")
                .unwrap()
                .to_string_lossy(),
            "C:\\app\\md-previewer.exe"
        );
        assert!(parse_command_target("   ").is_none());
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
