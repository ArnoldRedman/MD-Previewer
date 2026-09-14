// 窗口外观：几何保存与恢复、侧栏宽度、图标
use crate::paths::config_dir;
use crate::settings::{Settings, ThemeChoice};
use crate::UserEvent;
use std::fs;
use std::path::PathBuf;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::EventLoop;
use tao::window::Window;

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.ico");
const DEFAULT_W: f64 = 900.0;
const DEFAULT_H: f64 = 700.0;
#[derive(Copy, Clone)]
pub(crate) struct WindowGeom {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
}

fn geom_path() -> PathBuf {
    config_dir().join("window.geom")
}

pub(crate) fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// 旧版本把 macOS 菜单选的主题单独存在 theme.txt，现在并入 settings.json；
/// 首次启动把旧值搬过去并删掉旧文件，之后只剩一个来源
pub(crate) fn migrate_theme_file(settings: &mut Settings) {
    let legacy = config_dir().join("theme.txt");
    let Ok(raw) = fs::read_to_string(&legacy) else {
        return;
    };
    settings.theme = ThemeChoice::from_str(&raw);
    if let Err(error) = settings.save(&settings_path()) {
        eprintln!("Could not migrate theme setting: {error}");
        return;
    }
    let _ = fs::remove_file(legacy);
}

#[cfg(target_os = "macos")]
pub(crate) fn show_info_dialog(title: &str, description: &str) {
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title(title)
        .set_description(description)
        .show();
}

pub(crate) fn show_warning_dialog(title: &str, description: &str) {
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(title)
        .set_description(description)
        .show();
}

pub(crate) fn load_window_geom() -> Option<WindowGeom> {
    let txt = fs::read_to_string(geom_path()).ok()?;
    let parts: Vec<&str> = txt.trim().split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    Some(WindowGeom {
        x: parts[0].parse().ok()?,
        y: parts[1].parse().ok()?,
        w: parts[2].parse().ok()?,
        h: parts[3].parse().ok()?,
    })
}

/// 侧栏宽度。页面 CSS 里的 `.sidebar` 宽度和正文左边距都由它插值生成，只此一处
pub(crate) const SIDEBAR_WIDTH: f64 = 260.0;
/// 收窄后至少保留的窗口宽度，避免把窗口挤到没法用
const MIN_WINDOW_WIDTH: f64 = 360.0;

/// 侧栏开合时整体加宽/收窄窗口，让正文可视宽度保持不变。
/// 优先往左扩：正文和右上角工具栏在屏幕上原地不动，只是左边多出一条侧栏。
/// 顶到显示器左边就退化为只改宽度；最大化时不动窗口，此时只能挤占正文
pub(crate) fn resize_for_sidebar(window: &Window, opening: bool) {
    if window.is_maximized() {
        return;
    }
    let scale = window.scale_factor();
    let size = window.inner_size().to_logical::<f64>(scale);
    let delta = if opening {
        SIDEBAR_WIDTH
    } else {
        -SIDEBAR_WIDTH
    };
    if let Ok(position) = window.outer_position() {
        let position = position.to_logical::<f64>(scale);
        let left_limit = window
            .current_monitor()
            .map(|monitor| monitor.position().to_logical::<f64>(scale).x)
            .unwrap_or(0.0);
        window.set_outer_position(LogicalPosition::new(
            (position.x - delta).max(left_limit),
            position.y,
        ));
    }
    window.set_inner_size(LogicalSize::new(
        (size.width + delta).max(MIN_WINDOW_WIDTH),
        size.height,
    ));
}

pub(crate) fn save_window_geom(window: &Window) {
    let Ok(pos) = window.outer_position() else {
        return;
    };
    let scale = window.scale_factor();
    let size = window.inner_size();
    let geom = WindowGeom {
        x: pos.x as f64 / scale,
        y: pos.y as f64 / scale,
        w: size.width as f64 / scale,
        h: size.height as f64 / scale,
    };
    if geom.w < 200.0 || geom.h < 150.0 {
        return;
    }
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(
        dir.join("window.geom"),
        format!("{},{},{},{}", geom.x, geom.y, geom.w, geom.h),
    );
}

/// Return the saved geometry only when its center still falls inside
/// some connected monitor — prevents the window from landing off-screen
/// after a display swap.
pub(crate) fn geom_visible(geom: &WindowGeom, event_loop: &EventLoop<UserEvent>) -> bool {
    let cx = geom.x + geom.w / 2.0;
    let cy = geom.y + geom.h / 2.0;
    event_loop.available_monitors().any(|mon| {
        let scale = mon.scale_factor();
        let mp = mon.position();
        let ms = mon.size();
        let mx = mp.x as f64 / scale;
        let my = mp.y as f64 / scale;
        let mw = ms.width as f64 / scale;
        let mh = ms.height as f64 / scale;
        cx >= mx && cx <= mx + mw && cy >= my && cy <= my + mh
    })
}

pub(crate) fn centered_geom(event_loop: &EventLoop<UserEvent>) -> WindowGeom {
    if let Some(mon) = event_loop.primary_monitor() {
        let scale = mon.scale_factor();
        let mp = mon.position();
        let ms = mon.size();
        let mx = mp.x as f64 / scale;
        let my = mp.y as f64 / scale;
        let mw = ms.width as f64 / scale;
        let mh = ms.height as f64 / scale;
        WindowGeom {
            x: mx + (mw - DEFAULT_W) / 2.0,
            y: my + (mh - DEFAULT_H) / 2.0,
            w: DEFAULT_W,
            h: DEFAULT_H,
        }
    } else {
        WindowGeom {
            x: 100.0,
            y: 100.0,
            w: DEFAULT_W,
            h: DEFAULT_H,
        }
    }
}
/// Decode embedded icon.ico to an RGBA tao Icon for the window chrome.
pub(crate) fn load_window_icon() -> Option<tao::window::Icon> {
    let img = image::load_from_memory_with_format(ICON_BYTES, image::ImageFormat::Ico).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    tao::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
}
