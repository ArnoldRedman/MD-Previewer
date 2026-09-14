// macOS Finder 扩展：新建文件、打开终端、注册扩展
use std::fs;
use std::path::{Path, PathBuf};

// 只在注册 Finder 扩展的 macOS 分支里用到
#[cfg(target_os = "macos")]
use crate::paths::config_dir;
#[cfg(target_os = "macos")]
use crate::window::show_info_dialog;

pub(crate) fn create_finder_file(folder: &Path, kind: &str) -> std::io::Result<PathBuf> {
    if !folder.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Finder target folder does not exist",
        ));
    }
    let (extension, contents): (&str, &[u8]) = match kind {
        "txt" => ("txt", b""),
        "json" => ("json", b"{}\n"),
        "html" => (
            "html",
            b"<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n  <meta charset=\"utf-8\">\n  <title></title>\n</head>\n<body>\n</body>\n</html>\n",
        ),
        _ => ("md", b""),
    };
    let mut path = folder.join(format!("新建.{extension}"));
    let mut index = 2;
    while path.exists() {
        path = folder.join(format!("新建 {index}.{extension}"));
        index += 1;
    }
    fs::write(&path, contents)?;
    Ok(path)
}

pub(crate) fn normalize_new_markdown_path(mut path: PathBuf) -> PathBuf {
    let is_markdown = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkd"
            )
        })
        .unwrap_or(false);
    if !is_markdown {
        path.set_extension("md");
    }
    path
}

pub(crate) fn open_terminal(folder: &Path) -> bool {
    std::process::Command::new("open")
        .args(["-a", "Terminal"])
        .arg(folder)
        .spawn()
        .is_ok()
}

#[cfg(target_os = "macos")]
pub(crate) fn register_finder_extension() {
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(contents) = executable.parent().and_then(Path::parent) else {
        return;
    };
    let Some(bundle) = contents.parent() else {
        return;
    };
    let extension = bundle.join("Contents/PlugIns/MDPreviewerFinderExtension.appex");
    if bundle.extension().and_then(|value| value.to_str()) != Some("app") || !extension.exists() {
        return;
    }

    let marker = config_dir().join(".finder-extension-onboarded-v1");
    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    let _ = std::process::Command::new(lsregister)
        .args(["-f", "-R", "-trusted"])
        .arg(bundle)
        .status();
    let _ = std::process::Command::new("pluginkit")
        .arg("-a")
        .arg(bundle)
        .status();
    let _ = std::process::Command::new("pluginkit")
        .args([
            "-e",
            "use",
            "-i",
            "io.github.arnoldredman.mdpreviewer.finder-extension",
        ])
        .status();

    if marker.exists() {
        return;
    }
    let active = std::process::Command::new("pluginkit")
        .args(["-m", "-A", "-p", "com.apple.FinderSync"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .map(|output| {
            output.lines().any(|line| {
                line.trim_start().starts_with('+')
                    && line.contains("io.github.arnoldredman.mdpreviewer.finder-extension")
            })
        })
        .unwrap_or(false);
    if !active {
        show_info_dialog(
            "Enable the Finder Extension",
            "Open System Settings > General > Login Items & Extensions > Finder, then enable MD Previewer.",
        );
    }
    let _ = fs::create_dir_all(config_dir());
    let _ = fs::write(marker, "1");
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn register_finder_extension() {}
