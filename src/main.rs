#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod session;
mod settings;
mod single_instance;

use notify::{Event, RecursiveMode, Watcher};
use pulldown_cmark::{html, CowStr, Event as MdEvent, Options, Parser, Tag, TagEnd};
use session::DocumentSession;
use settings::{OpenMode, Settings, TabMode};
use std::collections::HashMap;
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event as TaoEvent, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::{Theme, Window, WindowBuilder};
use wry::{WebView, WebViewBuilder};

const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.ico");
const DEFAULT_W: f64 = 900.0;
const DEFAULT_H: f64 = 700.0;
static APP_DIRTY: AtomicBool = AtomicBool::new(false);
/// 当前设置是否需要跨启动的 `session.json`。用静态量是为了让 `persist_session`
/// 的十来个调用点不必都拿到设置，语义见 `Settings::keeps_session`
static SESSION_ENABLED: AtomicBool = AtomicBool::new(true);

#[derive(Debug)]
struct SelfWriteRecord {
    at: Instant,
    path: PathBuf,
    content: String,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug)]
enum UserEvent {
    NewFile,
    OpenFile,
    OpenPaths(Vec<PathBuf>, bool),
    ActivateTab(u64),
    CloseTab(u64),
    CloseOthers(u64),
    CloseActiveTab,
    LocateTab(u64),
    RevealTab(u64),
    RenderPreview(String),
    FileChanged(PathBuf), // external change: refresh preview AND textarea
    ExternalChangeResolved(bool),
    FileSaved(PathBuf), // our own save: refresh preview only, leave textarea cursor alone
    SaveFailed(String),
    DirtyChanged(bool),
    ToggleEdit,
    ShowFind,
    Print, // route print through wry's native API (WKWebView ignores window.print())
    SetTheme(ThemeChoice),
    SettingsChanged,
    SetEncoding(String),
    OpenUrl(&'static str),
    Quit,
    RecentChanged,
    Ready, // first paint landed: inject hljs now; if bench mode, also exit
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    fn as_str(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "light" => ThemeChoice::Light,
            "dark" => ThemeChoice::Dark,
            _ => ThemeChoice::System,
        }
    }

    fn tao_theme(self) -> Option<Theme> {
        match self {
            ThemeChoice::System => None,
            ThemeChoice::Light => Some(Theme::Light),
            ThemeChoice::Dark => Some(Theme::Dark),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct EnhanceFlags {
    math: bool,
    mermaid: bool,
}

impl EnhanceFlags {
    fn any(self) -> bool {
        self.math || self.mermaid
    }
}

fn is_help_arg(arg: &str) -> bool {
    arg == "-h" || arg == "--help"
}

fn print_help() {
    println!(
        "MD Previewer {}\n\nUsage:\n  md-previewer [file.md|file.txt]\n\nOptions:\n  -h, --help    Show this help message",
        env!("CARGO_PKG_VERSION")
    );
}

#[derive(Copy, Clone)]
enum Lang {
    Zh,
    En,
}

fn detect_lang() -> Lang {
    sys_locale::get_locale()
        .map(|l| {
            if l.to_lowercase().starts_with("zh") {
                Lang::Zh
            } else {
                Lang::En
            }
        })
        .unwrap_or(Lang::En)
}

struct Strings {
    drop_hint: &'static str,
    cannot_read: &'static str,
    open_file: &'static str,
    recent_title: &'static str,
    missing_title: &'static str,
    missing_body: &'static str,
    locate_file: &'static str,
    close_tab: &'static str,
    btn_edit: &'static str,
    btn_preview: &'static str,
    btn_new: &'static str,
    new_filename: &'static str,
    btn_open: &'static str,
    btn_search: &'static str,
    btn_print: &'static str,
    btn_zoom: &'static str,
    btn_zoom_out: &'static str,
    btn_zoom_reset: &'static str,
    btn_zoom_in: &'static str,
    search_placeholder: &'static str,
    stat_words: &'static str,
    stat_chars: &'static str,
    btn_settings: &'static str,
    set_open_mode: &'static str,
    set_open_tab: &'static str,
    set_open_window: &'static str,
    set_tab_mode: &'static str,
    set_tab_keep: &'static str,
    set_tab_single: &'static str,
    btn_sidebar: &'static str,
    sidebar_folder: &'static str,
    sidebar_recent: &'static str,
    sidebar_empty: &'static str,
    set_author_mode: &'static str,
    set_author_off: &'static str,
    set_author_on: &'static str,
    author_help: &'static str,
    copy_title_line: &'static str,
    copy_title: &'static str,
    copy_body: &'static str,
    copied: &'static str,
    btn_split: &'static str,
    sidebar_outline: &'static str,
    sidebar_outline_empty: &'static str,
    set_word_wrap: &'static str,
    set_wrap_on: &'static str,
    set_wrap_off: &'static str,
    code_copy: &'static str,
    code_copied: &'static str,
    tab_menu_close: &'static str,
    tab_menu_close_others: &'static str,
    tab_menu_copy_path: &'static str,
    tab_menu_reveal: &'static str,
    encoding_title: &'static str,
}

impl Strings {
    fn for_lang(lang: Lang) -> Self {
        match lang {
            Lang::Zh => Strings {
                drop_hint: "Drop a .md or .txt file here or press Cmd/Ctrl+O to open",
                cannot_read: "无法读取文件",
                open_file: "Open File",
                recent_title: "Recent",
                missing_title: "文件已移动或删除",
                missing_body: "这个标签会继续保留。你可以重新定位文件，或关闭标签。",
                locate_file: "重新定位",
                close_tab: "关闭标签",
                btn_edit: "编辑 (Cmd/Ctrl+E)",
                btn_preview: "预览 (Cmd/Ctrl+E)",
                btn_new: "新建 Markdown (Cmd/Ctrl+N)",
                new_filename: "新建.md",
                btn_open: "Open File (Cmd/Ctrl+O)",
                btn_search: "搜索 (Cmd/Ctrl+F)",
                btn_print: "打印 (Cmd/Ctrl+P)",
                btn_zoom: "正文缩放",
                btn_zoom_out: "缩小正文 (Cmd/Ctrl+-)",
                btn_zoom_reset: "重置正文缩放 (Cmd/Ctrl+0)",
                btn_zoom_in: "放大正文 (Cmd/Ctrl++)",
                search_placeholder: "搜索",
                stat_words: "字",
                stat_chars: "字符",
                btn_settings: "设置",
                set_open_mode: "打开 Markdown 文件时",
                set_open_tab: "沿用当前窗口",
                set_open_window: "开新窗口",
                set_tab_mode: "标签栏",
                set_tab_keep: "累计标签",
                set_tab_single: "只留当前",
                btn_sidebar: "侧栏",
                sidebar_folder: "当前文件夹",
                sidebar_recent: "最近打开",
                sidebar_empty: "没有可显示的文件",
                set_author_mode: "作者模式",
                set_author_off: "关",
                set_author_on: "开",
                author_help: "开启后，章节标题右侧出现「复制标题行」和「复制标题」两个按钮，正文右上角出现「复制正文」按钮，直接取用去发布，不用手动拖选。「复制标题行」原样复制整行；「复制标题」会去掉「第 N 章」这类序号。",
                copy_title_line: "复制标题行",
                copy_title: "复制标题",
                copy_body: "复制正文",
                copied: "已复制",
                btn_split: "分栏模式 (Cmd/Ctrl+\\)",
                sidebar_outline: "大纲",
                sidebar_outline_empty: "当前文档无标题大纲",
                set_word_wrap: "自动换行",
                set_wrap_on: "开",
                set_wrap_off: "关",
                code_copy: "复制",
                code_copied: "已复制",
                tab_menu_close: "关闭标签",
                tab_menu_close_others: "关闭其他标签",
                tab_menu_copy_path: "复制路径",
                tab_menu_reveal: "在文件管理器中显示",
                encoding_title: "编码格式",
            },
            Lang::En => Strings {
                drop_hint: "Drop a .md or .txt file here or press Cmd/Ctrl+O to open",
                cannot_read: "Cannot read file",
                open_file: "Open File",
                recent_title: "Recent",
                missing_title: "File Moved or Deleted",
                missing_body: "This tab is kept. Locate the file again or close the tab.",
                locate_file: "Locate File",
                close_tab: "Close Tab",
                btn_edit: "Edit (Cmd/Ctrl+E)",
                btn_preview: "Preview (Cmd/Ctrl+E)",
                btn_new: "New Markdown (Cmd/Ctrl+N)",
                new_filename: "Untitled.md",
                btn_open: "Open File (Cmd/Ctrl+O)",
                btn_search: "Find (Cmd/Ctrl+F)",
                btn_print: "Print (Cmd/Ctrl+P)",
                btn_zoom: "Content zoom",
                btn_zoom_out: "Zoom out (Cmd/Ctrl+-)",
                btn_zoom_reset: "Reset zoom (Cmd/Ctrl+0)",
                btn_zoom_in: "Zoom in (Cmd/Ctrl++)",
                search_placeholder: "Find",
                stat_words: "non-space",
                stat_chars: "chars",
                btn_settings: "Settings",
                set_open_mode: "When opening a Markdown file",
                set_open_tab: "Reuse window",
                set_open_window: "New window",
                set_tab_mode: "Tab bar",
                set_tab_keep: "Keep tabs",
                set_tab_single: "Current only",
                btn_sidebar: "Sidebar",
                sidebar_folder: "This folder",
                sidebar_recent: "Recent",
                sidebar_empty: "Nothing to show",
                set_author_mode: "Author mode",
                set_author_off: "Off",
                set_author_on: "On",
                author_help: "Adds copy buttons for publishing: two beside the chapter heading and one above the body. Copy heading line takes the whole line as written; Copy title drops the chapter number.",
                copy_title_line: "Copy heading line",
                copy_title: "Copy title",
                copy_body: "Copy body",
                copied: "Copied",
                btn_split: "Split View (Cmd/Ctrl+\\)",
                sidebar_outline: "Outline",
                sidebar_outline_empty: "No headings in document",
                set_word_wrap: "Word wrap",
                set_wrap_on: "On",
                set_wrap_off: "Off",
                code_copy: "Copy",
                code_copied: "Copied",
                tab_menu_close: "Close Tab",
                tab_menu_close_others: "Close Others",
                tab_menu_copy_path: "Copy Path",
                tab_menu_reveal: "Reveal in File Manager",
                encoding_title: "Encoding",
            },
        }
    }
}

fn config_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("MD_PREVIEWER_CONFIG_DIR") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("md-previewer")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".config/md-previewer")
    }
}

#[derive(Copy, Clone)]
struct WindowGeom {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn geom_path() -> PathBuf {
    config_dir().join("window.geom")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

fn theme_path() -> PathBuf {
    config_dir().join("theme.txt")
}

fn load_theme_choice() -> ThemeChoice {
    fs::read_to_string(theme_path())
        .map(|raw| ThemeChoice::from_str(&raw))
        .unwrap_or_default()
}

fn save_theme_choice(choice: ThemeChoice) {
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("theme.txt"), choice.as_str());
}

#[cfg(target_os = "macos")]
fn show_info_dialog(title: &str, description: &str) {
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title(title)
        .set_description(description)
        .show();
}

fn show_warning_dialog(title: &str, description: &str) {
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(title)
        .set_description(description)
        .show();
}

fn load_window_geom() -> Option<WindowGeom> {
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
const SIDEBAR_WIDTH: f64 = 260.0;
/// 收窄后至少保留的窗口宽度，避免把窗口挤到没法用
const MIN_WINDOW_WIDTH: f64 = 360.0;

/// 侧栏开合时整体加宽/收窄窗口，让正文可视宽度保持不变。
/// 优先往左扩：正文和右上角工具栏在屏幕上原地不动，只是左边多出一条侧栏。
/// 顶到显示器左边就退化为只改宽度；最大化时不动窗口，此时只能挤占正文
fn resize_for_sidebar(window: &Window, opening: bool) {
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

fn save_window_geom(window: &Window) {
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
fn geom_visible(geom: &WindowGeom, event_loop: &EventLoop<UserEvent>) -> bool {
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

fn centered_geom(event_loop: &EventLoop<UserEvent>) -> WindowGeom {
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

#[cfg(test)]
fn md_to_html(md: &str) -> String {
    md_to_html_with_base(md, None)
}

fn md_to_html_with_base(md: &str, base_dir: Option<&Path>) -> String {
    let (front_matter, markdown) = split_yaml_front_matter(md)
        .map(|(metadata, body)| (Some(metadata), body))
        .unwrap_or((None, md));
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM;
    let parser = Parser::new_ext(markdown, opts);
    let events = embed_local_images(
        add_mark_highlights(add_heading_ids(parser.collect())),
        base_dir,
    );
    let mut html_out = String::new();
    if let Some(metadata) = front_matter {
        html_out.push_str(r#"<aside class="front-matter"><pre>"#);
        html_out.push_str(&html_escape_text(metadata.trim_end_matches(['\r', '\n'])));
        html_out.push_str("</pre></aside>\n");
    }
    html::push_html(&mut html_out, events.into_iter());
    html_out
}

fn split_yaml_front_matter(md: &str) -> Option<(&str, &str)> {
    let mut lines = md.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return None;
    }

    let metadata_start = first.len();
    let mut offset = metadata_start;
    for line in lines {
        let value = line.trim_end_matches(['\r', '\n']);
        if value == "---" || value == "..." {
            let metadata = &md[metadata_start..offset];
            let body = &md[offset + line.len()..];
            return Some((metadata, body));
        }
        offset += line.len();
    }
    None
}

fn embed_local_images<'a>(events: Vec<MdEvent<'a>>, base_dir: Option<&Path>) -> Vec<MdEvent<'a>> {
    let Some(base_dir) = base_dir else {
        return events;
    };

    events
        .into_iter()
        .map(|event| match event {
            MdEvent::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let embedded = local_image_data_url(base_dir, dest_url.as_ref());
                MdEvent::Start(Tag::Image {
                    link_type,
                    dest_url: embedded.map(CowStr::from).unwrap_or(dest_url),
                    title,
                    id,
                })
            }
            _ => event,
        })
        .collect()
}

fn local_image_data_url(base_dir: &Path, url: &str) -> Option<String> {
    let image_path = resolve_local_relative_image_path(base_dir, url)?;
    let mime = image_mime_type(&image_path)?;
    let bytes = fs::read(image_path).ok()?;
    Some(format!("data:{mime};base64,{}", base64_encode(&bytes)))
}

fn resolve_local_relative_image_path(base_dir: &Path, url: &str) -> Option<PathBuf> {
    let path_part = url.split(['#', '?']).next()?.trim();
    if !is_local_relative_url(path_part) {
        return None;
    }

    let mut candidate = base_dir.to_path_buf();
    for segment in path_part.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        let decoded = percent_decode_path_segment(segment)?;
        let segment_path = Path::new(&decoded);
        if segment_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return None;
        }
        candidate.push(segment_path);
    }

    if candidate == base_dir {
        return None;
    }
    Some(candidate)
}

fn is_local_relative_url(url: &str) -> bool {
    !url.is_empty()
        && !url.starts_with('#')
        && !url.starts_with('/')
        && !url.starts_with('\\')
        && !url.starts_with("//")
        && !url.contains(':')
}

fn percent_decode_path_segment(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = bytes.get(i + 1).and_then(|b| hex_value(*b))?;
            let lo = bytes.get(i + 2).and_then(|b| hex_value(*b))?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn image_mime_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()?
        .to_string_lossy()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(((bytes.len() + 2) / 3) * 4);
    let mut chunks = bytes.chunks_exact(3);
    for chunk in &mut chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | chunk[2] as u32;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push(TABLE[(n & 0x3f) as usize] as char);
    }

    match chunks.remainder() {
        [a] => {
            let n = (*a as u32) << 16;
            out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
            out.push('=');
            out.push('=');
        }
        [a, b] => {
            let n = ((*a as u32) << 16) | ((*b as u32) << 8);
            out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
            out.push('=');
        }
        _ => {}
    }

    out
}

fn add_mark_highlights<'a>(events: Vec<MdEvent<'a>>) -> Vec<MdEvent<'a>> {
    let mut out = Vec::with_capacity(events.len());

    for event in events {
        match event {
            MdEvent::Text(text) => {
                if text.contains("==") {
                    push_mark_highlight_events(text.as_ref(), &mut out);
                } else {
                    out.push(MdEvent::Text(text));
                }
            }
            _ => out.push(event),
        }
    }

    out
}

fn push_mark_highlight_events<'a>(text: &str, out: &mut Vec<MdEvent<'a>>) {
    let mut rest = text;

    while let Some(open) = rest.find("==") {
        let after_open = open + 2;
        let Some(close_rel) = rest[after_open..].find("==") else {
            break;
        };
        let close = after_open + close_rel;
        let body = &rest[after_open..close];
        if body.trim().is_empty() {
            break;
        }

        if open > 0 {
            out.push(MdEvent::Text(CowStr::Boxed(
                rest[..open].to_string().into_boxed_str(),
            )));
        }
        out.push(MdEvent::Html(CowStr::Borrowed(
            r#"<mark class="mdp-mark">"#,
        )));
        out.push(MdEvent::Text(CowStr::Boxed(
            body.to_string().into_boxed_str(),
        )));
        out.push(MdEvent::Html(CowStr::Borrowed("</mark>")));
        rest = &rest[close + 2..];
    }

    if !rest.is_empty() {
        out.push(MdEvent::Text(CowStr::Boxed(
            rest.to_string().into_boxed_str(),
        )));
    }
}

fn add_heading_ids<'a>(mut events: Vec<MdEvent<'a>>) -> Vec<MdEvent<'a>> {
    let mut seen: HashMap<String, usize> = HashMap::new();

    for i in 0..events.len() {
        let generate_id = match &events[i] {
            MdEvent::Start(Tag::Heading { id: Some(id), .. }) => {
                register_heading_id(id.as_ref(), &mut seen);
                false
            }
            MdEvent::Start(Tag::Heading { id: None, .. }) => true,
            _ => false,
        };

        if !generate_id {
            continue;
        }

        let text = collect_heading_text(&events, i);
        let base = heading_slug(&text);
        let id_value = unique_heading_id(base, &mut seen);
        if let MdEvent::Start(Tag::Heading { id, .. }) = &mut events[i] {
            *id = Some(CowStr::Boxed(id_value.into_boxed_str()));
        }
    }

    events
}

fn collect_heading_text(events: &[MdEvent<'_>], start: usize) -> String {
    let mut text = String::new();

    for event in events.iter().skip(start + 1) {
        match event {
            MdEvent::End(TagEnd::Heading(_)) => break,
            MdEvent::Text(value)
            | MdEvent::Code(value)
            | MdEvent::InlineMath(value)
            | MdEvent::DisplayMath(value) => text.push_str(value.as_ref()),
            MdEvent::SoftBreak | MdEvent::HardBreak => text.push(' '),
            _ => {}
        }
    }

    text
}

fn heading_slug(text: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;

    for c in text.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            slug.push(c);
            last_dash = false;
        } else if c.is_whitespace() && !slug.is_empty() && !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

fn register_heading_id(id: &str, seen: &mut HashMap<String, usize>) {
    if !id.is_empty() {
        *seen.entry(id.to_string()).or_insert(0) += 1;
    }
}

fn unique_heading_id(base: String, seen: &mut HashMap<String, usize>) -> String {
    let count = seen.entry(base.clone()).or_insert(0);
    let id = if *count == 0 {
        base
    } else {
        format!("{base}-{count}")
    };
    *count += 1;
    id
}

// Embedded highlight.js + themes (offline)
const HLJS_JS: &str = include_str!("../assets/hljs/highlight.min.js");
const HLJS_LIGHT: &str = include_str!("../assets/hljs/github.min.css");
const HLJS_DARK: &str = include_str!("../assets/hljs/github-dark.min.css");
// Extra language pack(s) not in the `common` bundle. Each file
// ends with `hljs.registerLanguage(...)` and only works if evaluated
// in the same scope as the main bundle — we concat them into hljs-src.
const HLJS_EXTRA_LANGS: &str = concat!(
    // Delphi / Pascal (aliases: dpr, dfm, pas, pascal) — user requested
    include_str!("../assets/hljs/delphi.min.js"),
);
const PREVIEW_ENHANCE_JS: &str = include_str!("../assets/enhance/preview-enhance.js");
const KATEX_JS: &str = include_str!("../assets/katex/katex.min.js");
const KATEX_CSS: &str = include_str!("../assets/katex/katex.inline.css");
const MERMAID_JS: &str = include_str!("../assets/mermaid/mermaid.min.js");
const MAX_RECENT_FILES: usize = 8;

fn html_escape_ta(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn html_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 章节序号里可能出现的数字，阿拉伯数字、全角数字和中文数字都算
const CHAPTER_DIGITS: &str =
    "0123456789０１２３４５６７８９一二三四五六七八九十百千万零两壹贰叁肆伍陆柒捌玖拾佰仟";
/// 跟在序号后面的量词
const CHAPTER_UNITS: &str = "章节回卷篇幕集话";
/// 序号和正题之间可能的分隔符
const CHAPTER_SEPARATORS: [char; 12] = [
    ' ', '\t', '　', ':', '：', '.', '。', '、', ',', '，', '-', '—',
];

/// 作者模式里两个标题按钮要复制的内容。
/// 正文不在这里：它必须和屏幕上渲染出来的一致，由页面直接从 DOM 取
struct AuthorDoc {
    /// 整行标题的纯文本，例如「第二章 一只行李箱」
    title_line: String,
    /// 去掉序号后的标题，例如「一只行李箱」
    title: String,
}

/// 把标题里的行内 Markdown 还原成纯文本，跟页面上看到的一致。
/// 否则「## 第二章 **一只**行李箱」会把星号一起复制走
fn inline_plain_text(markdown: &str) -> String {
    let mut text = String::new();
    for event in Parser::new(markdown) {
        match event {
            MdEvent::Text(value) | MdEvent::Code(value) => text.push_str(&value),
            MdEvent::SoftBreak | MdEvent::HardBreak => text.push(' '),
            _ => {}
        }
    }
    text.trim().to_string()
}

/// 吃掉开头的「第X章」或「12、」这类序号，返回剩下的部分；不是序号则返回 None
fn chapter_number_prefix(title: &str) -> Option<&str> {
    if let Some(rest) = title.strip_prefix('第') {
        let split = rest.find(|c| !CHAPTER_DIGITS.contains(c))?;
        if split == 0 {
            return None;
        }
        let tail = &rest[split..];
        let unit = tail.chars().next()?;
        if !CHAPTER_UNITS.contains(unit) {
            return None;
        }
        return Some(&tail[unit.len_utf8()..]);
    }
    let split = title.find(|c: char| !c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let tail = &title[split..];
    // 纯数字开头必须紧跟分隔符才算序号，否则「2023年的夏天」会被误伤
    tail.starts_with(CHAPTER_SEPARATORS).then_some(tail)
}

/// 去掉章节标题里的序号前缀。整行只有序号时保持原样，免得复制出空串
fn strip_chapter_prefix(title: &str) -> &str {
    let title = title.trim();
    let stripped = chapter_number_prefix(title)
        .map(|rest| rest.trim_start_matches(CHAPTER_SEPARATORS))
        .unwrap_or("");
    if stripped.is_empty() {
        title
    } else {
        stripped
    }
}

/// 取出一章的标题行和去掉序号的标题，认第一个 ATX 标题作章节标题
fn author_doc(raw_md: &str) -> AuthorDoc {
    let heading = raw_md.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes == 0 || hashes > 6 {
            return None;
        }
        let rest = &trimmed[hashes..];
        // CommonMark 要求 # 后面跟空白，跟渲染器保持一致
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        let text = inline_plain_text(rest);
        (!text.is_empty()).then_some(text)
    });
    let Some(title_line) = heading else {
        return AuthorDoc {
            title_line: String::new(),
            title: String::new(),
        };
    };
    AuthorDoc {
        title: strip_chapter_prefix(&title_line).to_string(),
        title_line,
    }
}

/// 单个文件夹里列出的上限。目录是外部输入，遇到几千个文件的目录不该把界面拖死
const MAX_FOLDER_FILES: usize = 200;

/// 当前文档同目录下可预览的文档，按文件名排序。只列这一层不递归：
/// 侧栏的用途是在同一篇文档的邻居之间快速切换
fn folder_documents(active: Option<&Path>) -> Vec<PathBuf> {
    let Some(dir) = active.and_then(Path::parent) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_supported_document(path))
        .collect::<Vec<_>>();
    files.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    files.truncate(MAX_FOLDER_FILES);
    files
}

fn sidebar_entry_json(path: &Path, active: Option<&Path>) -> serde_json::Value {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    // 只显示所在目录名，完整路径放在条目的 title 里。
    // 用整条路径 + CSS 省略号会触发 bidi 重排，把路径分隔符甩到错误的位置
    let dir = path
        .parent()
        .and_then(Path::file_name)
        .map(|dir| dir.to_string_lossy().to_string())
        .unwrap_or_default();
    serde_json::json!({
        "path": path.to_string_lossy(),
        "name": name,
        "dir": dir,
        "active": Some(path) == active,
    })
}

fn sidebar_json(session: &DocumentSession, recent: &[PathBuf]) -> String {
    let active = session.active().map(|tab| tab.path.clone());
    let active = active.as_deref();
    let folder = folder_documents(active)
        .iter()
        .map(|path| sidebar_entry_json(path, active))
        .collect::<Vec<_>>();
    let recent = recent
        .iter()
        .map(|path| sidebar_entry_json(path, active))
        .collect::<Vec<_>>();
    serde_json::json!({ "folder": folder, "recent": recent }).to_string()
}

fn recent_files_path() -> PathBuf {
    config_dir().join("recent-files.txt")
}

fn session_path() -> PathBuf {
    config_dir().join("session.json")
}

fn tabs_json(session: &DocumentSession) -> String {
    let tabs = session
        .tabs
        .iter()
        .map(|tab| {
            let name = tab
                .path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| tab.path.to_string_lossy().to_string());
            serde_json::json!({
                "id": tab.id,
                "name": name,
                "path": tab.path.to_string_lossy(),
                "active": session.active_id == Some(tab.id),
                "missing": tab.missing,
                "dirty": tab.dirty,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&tabs).expect("tab state is serializable")
}

fn missing_preview_html(tab_id: u64, path: &Path, s: &Strings) -> String {
    format!(
        r#"<div class="missing-file"><div class="missing-mark">!</div><h2>{}</h2><p>{}</p><code>{}</code><div class="missing-actions"><button type="button" data-locate-tab="{tab_id}">{}</button><button type="button" data-close-tab="{tab_id}">{}</button></div></div>"#,
        html_escape_text(s.missing_title),
        html_escape_text(s.missing_body),
        html_escape_text(&path.to_string_lossy()),
        html_escape_text(s.locate_file),
        html_escape_text(s.close_tab),
    )
}

fn load_recent_files() -> Vec<PathBuf> {
    let Ok(txt) = fs::read_to_string(recent_files_path()) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for line in txt.lines() {
        let path = PathBuf::from(line);
        if line.is_empty() || !path.exists() || files.iter().any(|p| p == &path) {
            continue;
        }
        files.push(path);
        if files.len() == MAX_RECENT_FILES {
            break;
        }
    }
    files
}

fn save_recent_files(files: &[PathBuf]) {
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    let body = files
        .iter()
        .take(MAX_RECENT_FILES)
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::write(dir.join("recent-files.txt"), body);
}

fn remember_recent_file(files: &Arc<Mutex<Vec<PathBuf>>>, path: &Path) {
    let mut recent = files.lock().unwrap();
    recent.retain(|p| p != path);
    recent.insert(0, path.to_path_buf());
    recent.truncate(MAX_RECENT_FILES);
    save_recent_files(&recent);
}

fn forget_recent_file(files: &Arc<Mutex<Vec<PathBuf>>>, path: &Path) -> bool {
    let mut recent = files.lock().unwrap();
    let original_len = recent.len();
    recent.retain(|p| p != path);
    if recent.len() == original_len {
        return false;
    }
    save_recent_files(&recent);
    true
}

fn percent_encode_file_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn base_href_for_file(path: &Path) -> Option<String> {
    let dir = path.parent()?;
    Some(file_url_for_path_dir(dir))
}

fn file_url_for_path_dir(dir: &Path) -> String {
    let mut path = dir.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && !path.starts_with('/') {
        path.insert(0, '/');
    }
    if !path.ends_with('/') {
        path.push('/');
    }
    format!("file://{}", percent_encode_file_path(&path))
}

fn is_txt_document(path: &Path) -> bool {
    path.extension()
        .map(|extension| extension.to_string_lossy().eq_ignore_ascii_case("txt"))
        .unwrap_or(false)
}

/// 读取文档文本内容，支持 UTF-8、带 BOM 的 UTF-8/UTF-16 以及 Windows ANSI (GBK/CP936 等) 编码
/// 返回解析后的文本和实际采用的编码格式名称
fn read_document_with_encoding(
    path: &Path,
    encoding_override: Option<&str>,
) -> std::io::Result<(String, &'static str)> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() {
        let enc = match encoding_override {
            Some("GBK") => "GBK",
            Some("UTF-16 LE") => "UTF-16 LE",
            Some("UTF-16 BE") => "UTF-16 BE",
            _ => "UTF-8",
        };
        return Ok((String::new(), enc));
    }

    // 若用户明确指定了编码格式，优先按指定格式解码
    if let Some(enc) = encoding_override {
        match enc {
            "UTF-8" => {
                let slice = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    &bytes[3..]
                } else {
                    &bytes[..]
                };
                if let Ok(s) = std::str::from_utf8(slice) {
                    return Ok((s.to_string(), "UTF-8"));
                }
                return Ok((String::from_utf8_lossy(slice).into_owned(), "UTF-8"));
            }
            "GBK" => {
                #[cfg(target_os = "windows")]
                {
                    if let Some(s) = decode_windows_codepage(&bytes, 936) {
                        return Ok((s, "GBK"));
                    }
                }
                return Ok((String::from_utf8_lossy(&bytes).into_owned(), "GBK"));
            }
            "UTF-16 LE" => {
                let slice = if bytes.starts_with(&[0xFF, 0xFE]) {
                    &bytes[2..]
                } else {
                    &bytes[..]
                };
                let u16s: Vec<u16> = slice
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();
                if let Ok(s) = String::from_utf16(&u16s) {
                    return Ok((s, "UTF-16 LE"));
                }
                return Ok((String::from_utf16_lossy(&u16s), "UTF-16 LE"));
            }
            "UTF-16 BE" => {
                let slice = if bytes.starts_with(&[0xFE, 0xFF]) {
                    &bytes[2..]
                } else {
                    &bytes[..]
                };
                let u16s: Vec<u16> = slice
                    .chunks_exact(2)
                    .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                    .collect();
                if let Ok(s) = String::from_utf16(&u16s) {
                    return Ok((s, "UTF-16 BE"));
                }
                return Ok((String::from_utf16_lossy(&u16s), "UTF-16 BE"));
            }
            _ => {}
        }
    }

    // 自动探测编码：
    // 1. UTF-8 BOM: EF BB BF
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        if let Ok(s) = std::str::from_utf8(&bytes[3..]) {
            return Ok((s.to_string(), "UTF-8"));
        }
    }

    // 2. UTF-16 LE BOM: FF FE
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        if let Ok(s) = String::from_utf16(&u16s) {
            return Ok((s, "UTF-16 LE"));
        }
    }

    // 3. UTF-16 BE BOM: FE FF
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        if let Ok(s) = String::from_utf16(&u16s) {
            return Ok((s, "UTF-16 BE"));
        }
    }

    // 4. 标准 UTF-8
    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Ok((s.to_string(), "UTF-8"));
    }

    // 5. Windows ANSI (GBK/CP936 等系统代码页回退)
    #[cfg(target_os = "windows")]
    {
        if let Some(decoded) = decode_windows_codepage(&bytes, 936) {
            return Ok((decoded, "GBK"));
        }
    }

    // 兜底：容错转 UTF-8
    Ok((String::from_utf8_lossy(&bytes).into_owned(), "UTF-8"))
}

/// 读取文档文本内容快捷入口
#[allow(dead_code)]
fn read_document_to_string(path: &Path) -> std::io::Result<String> {
    read_document_with_encoding(path, None).map(|(content, _)| content)
}

/// 按照指定或文档已有编码保存文本
fn write_document_with_encoding(
    path: &Path,
    content: &str,
    encoding: Option<&str>,
) -> std::io::Result<()> {
    match encoding {
        Some("GBK") => {
            #[cfg(target_os = "windows")]
            {
                if let Some(bytes) = encode_windows_codepage(content, 936) {
                    return fs::write(path, bytes);
                }
            }
            fs::write(path, content)
        }
        Some("UTF-16 LE") => {
            let mut bytes = vec![0xFF, 0xFE];
            for u in content.encode_utf16() {
                bytes.extend_from_slice(&u.to_le_bytes());
            }
            fs::write(path, bytes)
        }
        Some("UTF-16 BE") => {
            let mut bytes = vec![0xFE, 0xFF];
            for u in content.encode_utf16() {
                bytes.extend_from_slice(&u.to_be_bytes());
            }
            fs::write(path, bytes)
        }
        _ => fs::write(path, content),
    }
}

#[cfg(target_os = "windows")]
fn decode_windows_codepage(bytes: &[u8], code_page: u32) -> Option<String> {
    if bytes.is_empty() {
        return Some(String::new());
    }
    extern "system" {
        fn MultiByteToWideChar(
            code_page: u32,
            flags: u32,
            multi_byte_str: *const u8,
            multi_byte_len: i32,
            wide_char_str: *mut u16,
            wide_char_len: i32,
        ) -> i32;
    }
    let len = unsafe {
        MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        )
    };
    if len <= 0 {
        let len_acp = unsafe {
            MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0)
        };
        if len_acp <= 0 {
            return None;
        }
        let mut wide = vec![0u16; len_acp as usize];
        let res = unsafe {
            MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, wide.as_mut_ptr(), len_acp)
        };
        return if res > 0 { String::from_utf16(&wide).ok() } else { None };
    }
    let mut wide = vec![0u16; len as usize];
    let res = unsafe {
        MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            len,
        )
    };
    if res > 0 {
        String::from_utf16(&wide).ok()
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn encode_windows_codepage(text: &str, code_page: u32) -> Option<Vec<u8>> {
    extern "system" {
        fn WideCharToMultiByte(
            code_page: u32,
            flags: u32,
            wide_char_str: *const u16,
            wide_char_len: i32,
            multi_byte_str: *mut u8,
            multi_byte_len: i32,
            default_char: *const u8,
            used_default_char: *mut i32,
        ) -> i32;
    }
    let wide: Vec<u16> = text.encode_utf16().collect();
    if wide.is_empty() {
        return Some(Vec::new());
    }
    let len = unsafe {
        WideCharToMultiByte(
            code_page,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            std::ptr::null_mut(),
        )
    };
    if len <= 0 {
        return None;
    }
    let mut bytes = vec![0u8; len as usize];
    let res = unsafe {
        WideCharToMultiByte(
            code_page,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            bytes.as_mut_ptr(),
            len,
            std::ptr::null(),
            std::ptr::null_mut(),
        )
    };
    if res > 0 {
        Some(bytes)
    } else {
        None
    }
}

/// 在系统文件管理器中定位并选中文件
fn reveal_in_file_manager(path: &Path) {
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

/// 纯文本渲染为保留换行与空格的 HTML 容器，特殊符号统一做 HTML 转义
fn txt_to_html(raw: &str) -> String {
    format!(
        r#"<div class="mdp-plain-text">{}</div>"#,
        html_escape_text(raw)
    )
}

/// 根据文档扩展名分发渲染：txt 走纯文本保留换行，md 走标准 Markdown 解析与增强
fn document_to_html(path: &Path, raw: &str) -> (String, EnhanceFlags, Option<String>) {
    if is_txt_document(path) {
        (txt_to_html(raw), EnhanceFlags::default(), None)
    } else {
        (
            md_to_html_with_base(raw, path.parent()),
            enhance_flags_for(raw),
            base_href_for_file(path),
        )
    }
}

fn starts_mermaid_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("```")
        .or_else(|| trimmed.strip_prefix("~~~"));
    let Some(info) = rest else {
        return false;
    };
    let info = info.trim_start();
    info == "mermaid"
        || info
            .strip_prefix("mermaid")
            .and_then(|s| s.chars().next())
            .map(|c| c.is_whitespace() || c == '{')
            .unwrap_or(false)
}

fn has_unescaped_at(s: &str, index: usize, needle: &str) -> bool {
    if !s[index..].starts_with(needle) {
        return false;
    }
    let mut backslashes = 0;
    for b in s[..index].bytes().rev() {
        if b == b'\\' {
            backslashes += 1;
        } else {
            break;
        }
    }
    backslashes % 2 == 0
}

fn has_unescaped_pair(s: &str, open: &str, close: &str) -> bool {
    let mut pos = 0;
    while let Some(rel) = s[pos..].find(open) {
        let start = pos + rel;
        if !has_unescaped_at(s, start, open) {
            pos = start + open.len();
            continue;
        }
        let body_start = start + open.len();
        let mut search = body_start;
        while let Some(close_rel) = s[search..].find(close) {
            let close_at = search + close_rel;
            if has_unescaped_at(s, close_at, close) {
                return true;
            }
            search = close_at + close.len();
        }
        pos = body_start;
    }
    false
}

fn has_inline_dollar_math(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' || !has_unescaped_at(s, i, "$") {
            i += 1;
            continue;
        }
        if bytes.get(i + 1).copied() == Some(b'$')
            || bytes
                .get(i + 1)
                .map(|b| b.is_ascii_whitespace())
                .unwrap_or(true)
        {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() {
            if bytes[j] == b'$'
                && has_unescaped_at(s, j, "$")
                && bytes
                    .get(j.wrapping_sub(1))
                    .map(|b| !b.is_ascii_whitespace())
                    .unwrap_or(false)
            {
                return true;
            }
            j += 1;
        }
        i += 1;
    }
    false
}

fn enhance_flags_for(md: &str) -> EnhanceFlags {
    EnhanceFlags {
        math: has_unescaped_pair(md, "$$", "$$")
            || has_unescaped_pair(md, "\\[", "\\]")
            || has_unescaped_pair(md, "\\(", "\\)")
            || has_inline_dollar_math(md),
        mermaid: md.lines().any(starts_mermaid_fence),
    }
}

fn build_enhancer_bootstrap(flags: EnhanceFlags, loaded: EnhanceFlags) -> Vec<String> {
    if !flags.any() {
        return Vec::new();
    }

    let mut scripts = Vec::new();
    if flags.math && !loaded.math {
        let mut js = String::from("(function(){\nif(!window.katex){\n");
        js.push_str(KATEX_JS);
        js.push_str("\n;try{window.katex=katex;}catch(e){}\n}\n");
        js.push_str("if(window.__setKatexCss)window.__setKatexCss('");
        js.push_str(&escape_js(KATEX_CSS));
        js.push_str("');\n})();");
        scripts.push(js);
    }
    if flags.mermaid && !loaded.mermaid {
        // Mermaid's standalone bundle expects global script scope. Keep it
        // out of the function wrapper that is safe for KaTeX/highlight.js.
        let mut js = String::with_capacity(MERMAID_JS.len() + 80);
        js.push_str(MERMAID_JS);
        js.push_str("\n;try{window.mermaid=mermaid;}catch(e){}\n");
        scripts.push(js);
    }
    scripts.push("if(window.__enhancePreview)window.__enhancePreview();".to_string());
    scripts
}

fn empty_preview_html(s: &Strings, recent_files: &[PathBuf]) -> String {
    let empty_class = if recent_files.is_empty() {
        "empty"
    } else {
        "empty has-recent"
    };
    let mut html = format!(
        r#"<div class="{empty_class}"><div class="icon">#</div><div>{}</div><button class="empty-open" type="button" data-open-file>{}</button>"#,
        html_escape_text(s.drop_hint),
        html_escape_text(s.open_file)
    );

    if !recent_files.is_empty() {
        html.push_str(&format!(
            r#"<div class="recent"><div class="recent-title">{}</div><div class="recent-list">"#,
            html_escape_text(s.recent_title)
        ));
        for (index, path) in recent_files.iter().take(MAX_RECENT_FILES).enumerate() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_else(|| path.to_string_lossy());
            let parent = path
                .parent()
                .map(|p| p.to_string_lossy())
                .unwrap_or_default();
            html.push_str(&format!(
                r#"<button class="recent-item" type="button" data-recent-index="{index}"><span class="recent-name">{}</span><span class="recent-path">{}</span></button>"#,
                html_escape_text(&name),
                html_escape_text(&parent)
            ));
        }
        html.push_str("</div></div>");
    }

    html.push_str("</div>");
    html
}

fn build_page_with_encoding(
    preview_html: &str,
    raw_md: &str,
    base_href: Option<&str>,
    flags: EnhanceFlags,
    s: &Strings,
    empty: bool,
    initial_encoding: &str,
) -> String {
    let body_class = if empty { "empty" } else { "" };
    let initial_encoding = if initial_encoding.is_empty() { "UTF-8" } else { initial_encoding };
    let base_tag = base_href
        .map(|href| format!(r#"<base id="base-href" href="{}">"#, html_escape_attr(href)))
        .unwrap_or_else(|| r#"<base id="base-href">"#.to_string());
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8">
{base_tag}
<style id="hljs-light">{css_light}</style>
<style id="hljs-dark" media="not all">{css_dark}</style>
<script>
(function(){{
  var mq = window.matchMedia('(prefers-color-scheme: dark)');
  function apply(e) {{
    document.getElementById('hljs-light').media = e.matches ? 'not all' : '';
    document.getElementById('hljs-dark').media = e.matches ? '' : 'not all';
  }}
  apply(mq); mq.addEventListener('change', apply);
}})();
</script>
<style>
:root {{ color-scheme: light dark; --chrome-top: 10px; --bar-top: 0px; --content-scale: 1; }}
/* Reserve scrollbar space permanently so the fixed toolbar doesn't shift
   between modes (one with scrollbar, one without). */
html {{ overflow-y: scroll; scrollbar-gutter: stable; }}
body {{
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
  margin: 0; padding: 0;
  line-height: 1.6; font-size: 15px;
  color: #1a1a1a; background: #fff;
}}
body.has-tabs {{ --chrome-top: 50px; --bar-top: 40px; }}
#app {{ max-width: 820px; margin: 0 auto; padding: 24px; }}
#preview {{ font-size: calc(15px * var(--content-scale)); }}
#preview .front-matter {{
  margin: 0 0 1.5em;
  padding: .85em 0;
  border-top: 1px solid #e1e4e8;
  border-bottom: 1px solid #e1e4e8;
  color: #59636e;
}}
#preview .front-matter pre {{
  margin: 0;
  padding: 0;
  border-radius: 0;
  overflow: visible;
  background: transparent;
  color: inherit;
  font: inherit;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}}
#preview h1,#preview h2,#preview h3,#preview h4 {{ margin-top: 1.4em; }}
#preview h1 {{ border-bottom: 1px solid #e1e4e8; padding-bottom: .3em; }}
#preview h2 {{ border-bottom: 1px solid #e1e4e8; padding-bottom: .2em; }}
#preview code {{ background: #f0f0f0; padding: 2px 6px; border-radius: 4px; font-size: 90%; }}
#preview pre {{ background: #f6f8fa; padding: 16px; border-radius: 8px; overflow-x: auto; position: relative; }}
#preview pre code {{ background: none; padding: 0; font-size: 14px; }}
#preview img {{ cursor: zoom-in; max-width: 100%; }}
#preview pre .code-copy-btn {{
  position: absolute;
  top: 8px;
  right: 8px;
  padding: 4px 8px;
  font-size: 11px;
  line-height: 1.2;
  color: #57606a;
  background: rgba(255,255,255,0.85);
  border: 1px solid #d0d7de;
  border-radius: 4px;
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s ease, background 0.15s ease, color 0.15s ease;
  user-select: none;
  z-index: 5;
}}
#preview pre:hover .code-copy-btn,
#preview pre .code-copy-btn:focus {{
  opacity: 1;
}}
#preview pre .code-copy-btn:hover {{
  background: #ffffff;
  color: #24292f;
  border-color: #8c959f;
}}
#preview pre .code-copy-btn.copied {{
  color: #1a7f37;
  border-color: #1a7f37;
  opacity: 1;
}}
#preview blockquote {{ border-left: 4px solid #ddd; margin: 0; padding: 0 1em; color: #666; }}
#preview .markdown-alert-note,
#preview .markdown-alert-tip,
#preview .markdown-alert-important,
#preview .markdown-alert-warning,
#preview .markdown-alert-caution {{
  margin: 1em 0;
  padding: 0.75em 1em;
  border-radius: 6px;
  color: inherit;
}}
#preview .markdown-alert-title {{
  display: flex;
  align-items: center;
  gap: .35em;
  margin: 0 0 .45em;
  font-weight: 600;
  line-height: 1.25;
}}
#preview .markdown-alert-title + p {{ margin-top: 0; }}
#preview .markdown-alert-note {{ border-color: #0969da; background: #ddf4ff; }}
#preview .markdown-alert-tip {{ border-color: #1a7f37; background: #dafbe1; }}
#preview .markdown-alert-important {{ border-color: #8250df; background: #fbefff; }}
#preview .markdown-alert-warning {{ border-color: #9a6700; background: #fff8c5; }}
#preview .markdown-alert-caution {{ border-color: #cf222e; background: #ffebe9; }}
#preview .markdown-alert-note .markdown-alert-title {{ color: #0969da; }}
#preview .markdown-alert-tip .markdown-alert-title {{ color: #1a7f37; }}
#preview .markdown-alert-important .markdown-alert-title {{ color: #8250df; }}
#preview .markdown-alert-warning .markdown-alert-title {{ color: #9a6700; }}
#preview .markdown-alert-caution .markdown-alert-title {{ color: #cf222e; }}
#preview .mdp-mark {{ border-radius: 3px; padding: 0 0.12em; background: #fff2a8; color: inherit; }}
#preview mark.search-hit {{ border-radius: 3px; padding: 0 0.12em; background: #fff2a8; color: inherit; }}
#preview mark.search-hit.current {{ background: #ffcc4d; color: #1a1a1a; }}
#preview table {{ border-collapse: collapse; width: 100%; }}
#preview .mdp-table-wrap {{
  width: min(calc(100vw - 64px), 1280px);
  margin: 1em 0 1em 50%;
  transform: translateX(-50%);
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
}}
#preview .mdp-table-wrap table {{ width: max-content; min-width: 100%; }}
#preview table th, #preview table td {{ border: 1px solid #ddd; padding: 8px 12px; text-align: left; }}
#preview table th {{ background: #f6f8fa; font-weight: 600; color: #1a1a1a; white-space: nowrap; }}
#preview table td {{ min-width: 64px; max-width: 360px; vertical-align: top; overflow-wrap: break-word; }}
#preview img {{ max-width: 100%; cursor: zoom-in; }}
#preview .katex-display {{ overflow-x: auto; overflow-y: hidden; padding: 0.15em 0; }}
#preview .mdp-mermaid {{ margin: 1.2em 0; overflow-x: auto; text-align: center; }}
#preview .mdp-mermaid svg {{ max-width: 100%; height: auto; }}
#preview .mdp-mermaid-error, #preview .mdp-math-error {{ color: #b42318; }}
#preview hr {{ border: none; border-top: 1px solid #e1e4e8; margin: 2em 0; }}
#preview a {{ color: #0969da; text-decoration: none; }}
#preview a:hover {{ text-decoration: underline; }}
#preview ul, #preview ol {{ padding-left: 2em; }}
#preview input[type="checkbox"] {{ margin-right: 6px; }}
#preview .mdp-plain-text {{
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
  font-family: "SF Mono", "Menlo", "Consolas", "Courier New", monospace;
  font-size: calc(14px * var(--content-scale));
  line-height: 1.65;
  tab-size: 4;
}}
	.empty {{ display: flex; flex-direction: column; align-items: center; justify-content: center;
	  min-height: 60vh; color: #999; font-size: 18px; gap: 12px; text-align: center; }}
	.empty.has-recent {{
	  justify-content: flex-start;
	  min-height: calc(100vh - 48px);
	  padding: clamp(56px, 10vh, 96px) 0 40px;
	  box-sizing: border-box;
	}}
	.empty .icon {{ font-size: 48px; opacity: 0.4; }}
	.empty-open {{
	  margin-top: 6px; min-height: 40px; padding: 0 16px;
	  border: 1px solid #ddd; border-radius: 8px; background: #fff;
	  color: #1a1a1a; font: inherit; font-size: 15px; cursor: pointer;
	}}
	.empty-open:hover {{ background: #f5f5f5; color: #000; }}
	.recent {{ width: min(480px, 100%); margin-top: 16px; text-align: left; }}
	.recent-title {{ margin: 0 0 8px; padding: 0; border: 0; font-size: 11px; font-weight: 600; letter-spacing: 0; color: #b6b6b6; text-transform: uppercase; }}
	.recent-list {{ display: grid; gap: 6px; }}
	.recent-item {{
	  width: 100%; min-height: 44px; padding: 7px 10px; border: 1px solid #eee;
	  border-radius: 8px; background: #fff; color: inherit; text-align: left; cursor: pointer;
	  display: grid; gap: 1px;
	}}
	.recent-item:hover {{ background: #f7f7f7; }}
	.recent-name {{ color: #555; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
	.recent-path {{ color: #aaa; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
	.tabbar {{
	  display: none; position: sticky; top: 0; z-index: 110; height: 40px;
	  box-sizing: border-box; align-items: stretch; gap: 4px; padding: 4px 8px;
	  border-bottom: 1px solid #e6e6e6; background: rgba(248,248,248,0.96);
	  backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
	}}
	body.has-tabs .tabbar {{ display: flex; }}
	.tabs {{ display: flex; flex: 1; min-width: 0; gap: 4px; overflow-x: auto; scrollbar-width: none; }}
	.tabs::-webkit-scrollbar {{ display: none; }}
	.tab {{
	  flex: 0 1 180px; min-width: 96px; max-width: 200px; height: 31px;
	  display: flex; align-items: center; gap: 7px; padding: 0 8px 0 10px;
	  box-sizing: border-box; border: 1px solid transparent; border-radius: 7px;
	  color: #6b6b6b; background: transparent; cursor: default; user-select: none;
	  font-size: 13px;
	}}
	.tab:hover {{ background: rgba(0,0,0,0.045); }}
	.tab.active {{ color: #202020; background: #fff; border-color: #ddd; box-shadow: 0 1px 2px rgba(0,0,0,.04); }}
	.tab.missing {{ color: #a15c00; }}
	.tab-status {{ width: 7px; height: 7px; flex: 0 0 auto; border-radius: 50%; background: transparent; }}
	.tab.dirty .tab-status {{ background: #2979c9; }}
	.tab.missing .tab-status {{ width: auto; height: auto; background: none; border-radius: 0; font-weight: 700; }}
	.tab-name {{ min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
	.tab-close {{
	  width: 20px; height: 20px; flex: 0 0 auto; padding: 0; border: 0; border-radius: 5px;
	  display: grid; place-items: center; color: inherit; background: transparent; cursor: pointer;
	  font: 16px/1 -apple-system, BlinkMacSystemFont, sans-serif; opacity: .58;
	}}
	.tab-close:hover {{ opacity: 1; background: rgba(0,0,0,.08); }}
	.tab-open {{
	  width: 31px; height: 31px; flex: 0 0 auto; padding: 0; border: 0; border-radius: 7px;
	  color: #666; background: transparent; cursor: pointer; font: 20px/1 -apple-system, sans-serif;
	}}
	.tab-open:hover {{ color: #111; background: rgba(0,0,0,.06); }}
	.doc-stats {{
	  flex: 0 0 auto; align-self: center; padding: 0 6px;
	  color: #8b8b8b; font-size: 11px; white-space: nowrap; user-select: none;
	  font-variant-numeric: tabular-nums;
	}}
	.encoding-control {{ position: relative; flex: 0 0 auto; align-self: center; }}
	body.empty .encoding-control {{ display: none !important; }}
	.encoding-btn {{
	  height: 23px; padding: 0 7px;
	  border: 1px solid rgba(0,0,0,0.08); border-radius: 6px;
	  background: rgba(0,0,0,0.03); color: #666;
	  font-size: 11px; font-family: inherit; font-weight: 500;
	  cursor: pointer; display: inline-flex; align-items: center; gap: 3px;
	  user-select: none; transition: all 0.15s ease;
	}}
	.encoding-btn:hover {{
	  color: #111; background: rgba(0,0,0,0.07); border-color: rgba(0,0,0,0.15);
	}}
	.encoding-popover {{
	  position: absolute; top: calc(100% + 6px); right: 0; min-width: 130px; padding: 4px;
	  background: rgba(255,255,255,0.98); border: 1px solid #d0d7de; border-radius: 8px;
	  box-shadow: 0 6px 20px rgba(0,0,0,0.12); backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
	  z-index: 120;
	}}
	.encoding-popover-title {{
	  font-size: 11px; font-weight: 600; color: #8c959f; padding: 4px 8px 3px;
	  border-bottom: 1px solid rgba(0,0,0,0.06); margin-bottom: 3px;
	}}
	.encoding-option {{
	  display: block; width: 100%; text-align: left; padding: 5px 8px; font-size: 12px;
	  border: none; background: transparent; color: #24292f; border-radius: 5px; cursor: pointer;
	  transition: background 0.12s;
	}}
	.encoding-option:hover {{ background: #f3f4f6; color: #0969da; }}
	.encoding-option.active {{ font-weight: 600; color: #0969da; background: rgba(9,105,218,0.08); }}
	.missing-file {{ min-height: 55vh; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; text-align: center; }}
	.missing-file h2, .missing-file p {{ margin: 0; }}
	.missing-file p {{ color: #777; }}
	.missing-file code {{ max-width: min(620px, 90vw); overflow-wrap: anywhere; color: #8a5a12; }}
	.missing-mark {{ width: 38px; height: 38px; border: 2px solid #c98322; border-radius: 50%; display: grid; place-items: center; color: #a15c00; font-weight: 700; font-size: 22px; }}
	.missing-actions {{ display: flex; gap: 8px; margin-top: 8px; }}
	.missing-actions button {{ min-height: 36px; padding: 0 13px; border: 1px solid #d8d8d8; border-radius: 7px; background: #fff; color: #333; cursor: pointer; font: inherit; }}
	.missing-actions button:hover {{ background: #f4f4f4; }}

/* Floating toolbar (top-right) — hover-reveal, hidden in empty state */
.toolbar {{
  position: fixed; top: var(--chrome-top); right: 12px;
  display: flex; gap: 6px; z-index: 100;
  opacity: 0; pointer-events: none;
  transition: opacity 0.18s ease;
}}
html:hover .toolbar {{ opacity: 1; pointer-events: auto; }}
body.empty .toolbar {{ display: none !important; }}
.toolbar button {{
  width: 34px; height: 34px; padding: 0;
  background: rgba(255,255,255,0.8);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  border: 1px solid rgba(0,0,0,0.08);
  border-radius: 8px;
  display: grid; place-items: center;
  cursor: pointer; color: #555;
  transition: color 0.15s, background 0.15s;
}}
.toolbar button:hover {{ color: #000; background: rgba(255,255,255,1); }}
.toolbar button[hidden] {{ display: none !important; }}
	.zoom-control {{ position: relative; }}
	.zoom-popover {{
	  position: absolute; top: 40px; right: 0;
	  display: none; align-items: center; gap: 2px; padding: 4px;
	  border: 1px solid rgba(0,0,0,.1); border-radius: 8px;
	  background: rgba(255,255,255,.96); box-shadow: 0 6px 20px rgba(0,0,0,.12);
	  backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);
	}}
	.zoom-control.open .zoom-popover {{ display: flex; }}
	.toolbar .zoom-popover button {{ width: 30px; height: 30px; border: 0; background: transparent; }}
	.toolbar .zoom-popover button:hover {{ background: rgba(0,0,0,.06); }}
	.toolbar .zoom-popover .zoom-reset {{
	  width: 52px; font-size: 11px; font-variant-numeric: tabular-nums;
	}}
	.settings-control {{ position: relative; }}
	.settings-popover {{
	  position: absolute; top: 40px; right: 0;
	  display: none; flex-direction: column; gap: 10px; padding: 10px;
	  border: 1px solid rgba(0,0,0,.1); border-radius: 8px;
	  background: rgba(255,255,255,.96); box-shadow: 0 6px 20px rgba(0,0,0,.12);
	  backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);
	  white-space: nowrap; text-align: left;
	}}
	.settings-control.open .settings-popover {{ display: flex; }}
	.settings-row {{ display: flex; flex-direction: column; gap: 5px; align-items: stretch; }}
	.settings-label {{ display: flex; align-items: center; gap: 5px; font-size: 11px; color: #666; }}
	.settings-help {{
	  display: inline-flex; align-items: center; justify-content: center; cursor: help;
	  width: 14px; height: 14px; border: 1px solid #bbb; border-radius: 50%;
	  color: #888; font-size: 10px; line-height: 1;
	}}
	.settings-help:hover {{ border-color: #1a73e8; color: #1a73e8; }}
	.settings-seg {{
	  display: flex; border: 1px solid rgba(0,0,0,.14); border-radius: 6px; overflow: hidden;
	}}
	.toolbar .settings-seg button {{
	  flex: 1; width: auto; height: 26px; padding: 0 12px;
	  border: 0; border-radius: 0; background: transparent;
	  font: inherit; font-size: 12px; color: #444;
	}}
	.toolbar .settings-seg button + button {{ border-left: 1px solid rgba(0,0,0,.14); }}
	.toolbar .settings-seg button:hover {{ background: rgba(0,0,0,.06); }}
	.toolbar .settings-seg button[aria-pressed="true"] {{
	  background: #1a73e8; color: #fff;
	}}
	.toolbar .settings-seg button[aria-pressed="true"]:hover {{ background: #1765cc; }}
	.sidebar-toggle {{ right: auto; left: 12px; }}
	body.sidebar-open .sidebar-toggle {{ left: {sidebar_toggle_left}px; opacity: 1; pointer-events: auto; }}
	.sidebar {{
	  position: fixed; top: 0; left: 0; bottom: 0; width: {sidebar_width}px; z-index: 105;
	  display: none; flex-direction: column; box-sizing: border-box;
	  border-right: 1px solid #e6e6e6; background: rgba(248,248,248,.98);
	  backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
	}}
	body.sidebar-open .sidebar {{ display: flex; }}
	body.sidebar-open:not(.empty) {{ padding-left: {sidebar_width}px; }}
	body.empty .sidebar {{ display: none !important; }}
	.sidebar-sections {{ display: flex; gap: 4px; padding: 48px 8px 8px; }}
	.sidebar-sections button {{
	  flex: 1; height: 28px; padding: 0 6px; cursor: pointer;
	  border: 1px solid transparent; border-radius: 6px;
	  background: transparent; font: inherit; font-size: 12px; color: #666;
	}}
	.sidebar-sections button:hover {{ background: rgba(0,0,0,.05); }}
	.sidebar-sections button[aria-pressed="true"] {{ background: #fff; color: #111; border-color: #dcdcdc; }}
	.sidebar-list {{ flex: 1; overflow-y: auto; padding: 0 8px 12px; }}
	.sidebar-item {{
	  display: block; width: 100%; box-sizing: border-box; cursor: pointer;
	  border: 0; border-radius: 6px; background: transparent;
	  padding: 6px 8px; margin-bottom: 2px; text-align: left; font: inherit;
	}}
	.sidebar-item:hover {{ background: rgba(0,0,0,.06); }}
	.sidebar-item.active {{ background: #e8effc; }}
	.sidebar-name {{
	  display: block; font-size: 12px; color: #333;
	  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
	}}
	.sidebar-item.active .sidebar-name {{ color: #1a3f7a; font-weight: 600; }}
	.sidebar-dir {{
	  display: block; font-size: 10px; color: #999;
	  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
	}}
	.sidebar-empty {{ padding: 10px 8px; font-size: 12px; color: #999; }}
	.author-actions {{ display: inline-flex; gap: 6px; margin-left: 10px; vertical-align: middle; }}
	.author-body-actions {{ display: flex; justify-content: flex-end; margin: -2px 0 12px; }}
	.author-copy {{
	  border: 1px solid #d8d8d8; border-radius: 6px; background: #fff;
	  padding: 3px 10px; cursor: pointer; white-space: nowrap; vertical-align: middle;
	  font-family: inherit; font-size: 12px; font-weight: 400; line-height: 1.6; color: #555;
	}}
	.author-copy:hover {{ border-color: #1a73e8; color: #1a73e8; }}
	.author-copy.done {{ border-color: #1a9e5c; color: #1a9e5c; }}
	.findbar {{
	  position: fixed; top: var(--chrome-top); left: 50%; transform: translateX(-50%);
	  display: none; align-items: center; gap: 6px; z-index: 101;
	  padding: 6px; border: 1px solid rgba(0,0,0,0.08); border-radius: 10px;
	  background: rgba(255,255,255,0.96); box-shadow: 0 8px 24px rgba(0,0,0,0.12);
	  backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);
	}}
	body.finding .findbar {{ display: flex; }}
	.findbar input {{
	  width: min(42vw, 320px); height: 30px; box-sizing: border-box; border: 0;
	  outline: none; background: transparent; color: inherit; font: inherit;
	}}
	.findbar span {{ min-width: 14px; color: #8c959f; text-align: center; }}
	.findbar button {{
	  width: 30px; height: 30px; padding: 0; border: 0; border-radius: 7px;
	  display: grid; place-items: center; color: #555; background: transparent; cursor: pointer;
	}}
	.findbar button:hover {{ background: #f0f0f0; color: #111; }}
	.sidebar-item.outline-item {{
	  display: block; width: 100%; text-align: left;
	  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
	  padding: 5px 8px; font-size: 13px; line-height: 1.4;
	  border-left: 2px solid transparent;
	}}
	.sidebar-item.outline-item.outline-level-1 {{ padding-left: 10px; font-weight: 600; }}
	.sidebar-item.outline-item.outline-level-2 {{ padding-left: 20px; }}
	.sidebar-item.outline-item.outline-level-3 {{ padding-left: 30px; font-size: 12px; }}
	.sidebar-item.outline-item.outline-level-4 {{ padding-left: 40px; font-size: 12px; }}
	.sidebar-item.outline-item.outline-level-5 {{ padding-left: 50px; font-size: 11px; }}
	.sidebar-item.outline-item.outline-level-6 {{ padding-left: 60px; font-size: 11px; }}
	.sidebar-item.outline-item.active {{
	  background: #eef3fc; border-left-color: #2f6fd0; color: #2f6fd0;
	}}
	.context-menu {{
	  position: fixed; z-index: 1000; min-width: 150px;
	  background: #ffffff; border: 1px solid #d0d7de; border-radius: 6px;
	  box-shadow: 0 8px 24px rgba(140,149,159,0.2); padding: 4px 0; font-size: 13px;
	}}
	.context-menu-item {{
	  display: block; width: 100%; padding: 6px 12px; text-align: left;
	  border: none; background: transparent; color: #24292f; cursor: pointer; font: inherit;
	}}
	.context-menu-item:hover {{ background: #f0f3f6; color: #0969da; }}
	.context-menu-sep {{ height: 1px; background: #e1e4e8; margin: 4px 0; }}
	.lightbox {{
	  position: fixed; top: 0; left: 0; right: 0; bottom: 0;
	  z-index: 500; display: flex; align-items: center; justify-content: center; user-select: none;
	}}
	.lightbox-backdrop {{
	  position: absolute; top: 0; left: 0; right: 0; bottom: 0;
	  background: rgba(0, 0, 0, 0.85);
	  backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);
	}}
	.lightbox-toolbar {{
	  position: absolute; top: 16px; right: 16px; display: flex; gap: 8px; z-index: 510;
	}}
	.lightbox-btn {{
	  background: rgba(30, 30, 30, 0.75); color: #ffffff;
	  border: 1px solid rgba(255, 255, 255, 0.2); border-radius: 6px;
	  padding: 6px 10px; font-size: 13px; cursor: pointer;
	  transition: background 0.15s ease;
	}}
	.lightbox-btn:hover {{ background: rgba(60, 60, 60, 0.95); }}
	.lightbox-stage {{
	  position: relative; z-index: 505; max-width: 90vw; max-height: 85vh;
	  display: flex; align-items: center; justify-content: center; overflow: hidden; cursor: grab;
	}}
	.lightbox-stage.dragging {{ cursor: grabbing; }}
	.lightbox-img {{
	  max-width: 90vw; max-height: 85vh; object-fit: contain;
	  transition: transform 0.1s ease-out; transform-origin: center center;
	}}
	.lightbox-caption {{
	  position: absolute; bottom: 16px; left: 50%; transform: translateX(-50%);
	  color: #cccccc; background: rgba(0, 0, 0, 0.6); padding: 4px 12px;
	  border-radius: 4px; font-size: 13px; max-width: 80%;
	  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
	  z-index: 510; pointer-events: none;
	}}
	@media (prefers-color-scheme: dark) {{
	  body {{ color: #d4d4d4; background: #1e1e1e; }}
	  #preview a {{ color: #6cb6ff; }}
	  #preview h1, #preview h2 {{ border-color: #333; }}
	  #preview .front-matter {{ border-color: #333; color: #a9b1ba; }}
	  #preview .front-matter pre {{ background: transparent !important; }}
	  #preview pre {{ background: #2d2d2d !important; }}
	  #preview code:not(pre code) {{ background: #2d2d2d; }}
	  #preview blockquote {{ border-color: #444; color: #aaa; }}
	  #preview .markdown-alert-note,
	  #preview .markdown-alert-tip,
	  #preview .markdown-alert-important,
	  #preview .markdown-alert-warning,
	  #preview .markdown-alert-caution {{ background: #161b22; color: #d4d4d4; }}
	  #preview .markdown-alert-note {{ border-color: #2f81f7; }}
	  #preview .markdown-alert-tip {{ border-color: #3fb950; }}
	  #preview .markdown-alert-important {{ border-color: #a371f7; }}
	  #preview .markdown-alert-warning {{ border-color: #d29922; }}
	  #preview .markdown-alert-caution {{ border-color: #f85149; }}
	  #preview .markdown-alert-note .markdown-alert-title {{ color: #2f81f7; }}
	  #preview .markdown-alert-tip .markdown-alert-title {{ color: #3fb950; }}
	  #preview .markdown-alert-important .markdown-alert-title {{ color: #a371f7; }}
	  #preview .markdown-alert-warning .markdown-alert-title {{ color: #d29922; }}
	  #preview .markdown-alert-caution .markdown-alert-title {{ color: #f85149; }}
	  #preview table th {{ background: #2d2d2d; color: #f0f0f0; }}
	  #preview table td, #preview table th {{ border-color: #444; }}
	  #preview hr {{ border-color: #333; }}
	  .toolbar button {{
	    background: rgba(40,40,40,0.8);
    border-color: rgba(255,255,255,0.1);
    color: #bbb;
	  }}
	  .toolbar button:hover {{ color: #fff; background: rgba(55,55,55,1); }}
	  .zoom-popover {{ background: rgba(34,34,34,.96); border-color: rgba(255,255,255,.12); }}
	  .toolbar .zoom-popover button:hover {{ background: rgba(255,255,255,.1); }}
	  .settings-popover {{ background: rgba(34,34,34,.96); border-color: rgba(255,255,255,.12); }}
	  .settings-label {{ color: #999; }}
	  .settings-seg {{ border-color: rgba(255,255,255,.16); }}
	  .toolbar .settings-seg button {{ color: #bbb; }}
	  .toolbar .settings-seg button + button {{ border-left-color: rgba(255,255,255,.16); }}
	  .toolbar .settings-seg button:hover {{ background: rgba(255,255,255,.1); color: #fff; }}
	  .toolbar .settings-seg button[aria-pressed="true"] {{ background: #2f6fd0; color: #fff; }}
	  .toolbar .settings-seg button[aria-pressed="true"]:hover {{ background: #3a7de0; }}
	  .sidebar {{ background: rgba(24,24,24,.98); border-right-color: #333; }}
	  .sidebar-sections button {{ color: #999; }}
	  .sidebar-sections button:hover {{ background: rgba(255,255,255,.07); }}
	  .sidebar-sections button[aria-pressed="true"] {{ background: #2c2c2c; color: #eee; border-color: #444; }}
	  .sidebar-item:hover {{ background: rgba(255,255,255,.07); }}
	  .sidebar-item.active {{ background: #23324a; }}
	  .sidebar-name {{ color: #ccc; }}
	  .sidebar-item.active .sidebar-name {{ color: #9dc0ff; }}
	  .sidebar-dir {{ color: #777; }}
	  .sidebar-empty {{ color: #777; }}
	  .settings-help {{ border-color: #555; color: #999; }}
	  .settings-help:hover {{ border-color: #6cb6ff; color: #6cb6ff; }}
	  .author-copy {{ background: #262626; border-color: #444; color: #bbb; }}
	  .author-copy:hover {{ border-color: #6cb6ff; color: #6cb6ff; }}
	  .author-copy.done {{ border-color: #4ec27f; color: #4ec27f; }}
		  .empty-open {{ background: #242424; border-color: #444; color: #ddd; }}
		  .empty-open:hover {{ background: #2d2d2d; color: #fff; }}
		  .recent-name {{ color: #ddd; }}
		  .recent-item {{ background: #242424; border-color: #333; }}
		  .recent-item:hover {{ background: #2d2d2d; }}
	  .findbar {{ background: rgba(34,34,34,0.96); border-color: rgba(255,255,255,0.1); }}
	  .findbar button:hover {{ background: #333; color: #fff; }}
	  .tabbar {{ background: rgba(28,28,28,.96); border-color: #363636; }}
	  body.editing #topbar {{ background: rgba(28,28,28,.96); border-color: #363636; }}
	  .tab {{ color: #aaa; }}
	  .tab:hover {{ background: rgba(255,255,255,.07); }}
	  .tab.active {{ color: #eee; background: #2c2c2c; border-color: #444; }}
	  .tab.missing {{ color: #e3a04b; }}
	  .tab-close:hover, .tab-open:hover {{ background: rgba(255,255,255,.1); color: #fff; }}
	  .missing-file p {{ color: #aaa; }}
	  .missing-actions button {{ background: #292929; border-color: #444; color: #ddd; }}
	  .missing-actions button:hover {{ background: #333; }}
	  #preview pre .code-copy-btn {{
	    color: #8b949e; background: rgba(30,30,30,0.85); border-color: rgba(240,246,252,0.15);
	  }}
	  #preview pre .code-copy-btn:hover {{
	    background: #2d333b; color: #c9d1d9; border-color: rgba(240,246,252,0.25);
	  }}
	  #preview pre .code-copy-btn.copied {{ color: #3fb950; border-color: #3fb950; }}
	  .sidebar-item.outline-item.active {{ background: #23324a; border-left-color: #58a6ff; color: #9dc0ff; }}
	  .context-menu {{
	    background: #1c2128; border-color: #30363d; box-shadow: 0 8px 24px rgba(0,0,0,0.5);
	  }}
	  .context-menu-item {{ color: #c9d1d9; }}
	  .context-menu-item:hover {{ background: #282e33; color: #58a6ff; }}
	  .context-menu-sep {{ background: #30363d; }}
	  body.editing.split-view #editor {{ border-right-color: #383e47; background: rgba(255,255,255,0.015); }}
	  .encoding-btn {{ background: rgba(255,255,255,0.06); border-color: rgba(255,255,255,0.12); color: #9da7b3; }}
	  .encoding-btn:hover {{ background: rgba(255,255,255,0.12); color: #f0f6fc; border-color: rgba(255,255,255,0.22); }}
	  .encoding-popover {{ background: #1c2128; border-color: #30363d; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }}
	  .encoding-popover-title {{ color: #768390; border-bottom-color: rgba(255,255,255,0.06); }}
	  .encoding-option {{ color: #adbac7; }}
	  .encoding-option:hover {{ background: #2d333b; color: #58a6ff; }}
	  .encoding-option.active {{ color: #58a6ff; background: rgba(56,139,253,0.15); }}
	  body.editing #btn-split[aria-pressed="true"] {{ background: rgba(56,139,253,0.18); color: #58a6ff; border-color: rgba(56,139,253,0.4); }}
	}}

/* Source editor textarea — height is auto-grown by JS to match content,
   so the page (html) owns the only vertical scrollbar. */
#editor {{
  display: none;
  width: 100%;
  box-sizing: border-box;
  border: none; outline: none; resize: none;
  overflow: hidden;
  font: calc(14px * var(--content-scale))/1.6 "SF Mono","Menlo","Consolas",monospace;
  background: transparent; color: inherit;
  padding: 0;
}}
#btn-split {{ display: none; }}
body.editing #btn-split {{ display: grid; place-items: center; }}
body.editing #btn-split[aria-pressed="true"] {{
  background: rgba(9, 105, 218, 0.12);
  color: #0969da;
  border-color: rgba(9, 105, 218, 0.3);
}}
body.editing #preview {{ display: none; }}
body.editing #editor {{ display: block; padding: 16px 24px; }}
body.editing.split-view #app {{
  display: flex; flex-direction: row; align-items: stretch;
  max-width: none; padding: 0; box-sizing: border-box;
}}
body.editing.split-view #editor {{
  order: 1;
  display: block; flex: 1 1 50%; width: 50%; min-width: 0;
  box-sizing: border-box; padding: 18px 24px;
  border-right: 2px solid #d0d7de;
  background: rgba(0, 0, 0, 0.015);
}}
body.editing.split-view #preview {{
  order: 2;
  display: block !important; flex: 1 1 50%; width: 50%; min-width: 0;
  box-sizing: border-box; padding: 18px 24px;
  border-left: none;
}}
body.no-wrap #editor {{
  white-space: pre; overflow-x: auto; word-break: normal;
}}
body.no-wrap #preview pre {{
  white-space: pre; overflow-x: auto; word-break: normal;
}}
body.editing #app {{ max-width: none; padding: 0; }}
body.editing #btn-open,
body.editing #btn-search,
body.editing #btn-print {{ display: none; }}
/* 编辑模式：悬浮控件改为独占顶栏，不再遮挡正文源码 */
body.editing #topbar {{
  display: flex; justify-content: space-between; align-items: center;
  position: sticky; top: var(--bar-top); z-index: 110;
  padding: 6px 12px; border-bottom: 1px solid #e6e6e6;
  background: rgba(248,248,248,0.96);
  backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
}}
body.editing .toolbar {{ position: static; opacity: 1; pointer-events: auto; }}
body.editing .findbar {{ display: none !important; }}

@page {{
  margin: 12mm;
}}

@media print {{
  .toolbar, #topbar, .tabbar, #editor, .sidebar,
  .author-actions, .author-body-actions {{ display: none !important; }}
  body {{ padding-left: 0 !important; }}
  #preview {{ display: block !important; }}
  #app {{ max-width: none; padding: 0; }}
  #preview .mdp-table-wrap {{ width: auto; margin: 1em 0; transform: none; overflow: visible; }}
}}
	</style></head><body class="{body_class}">
	<div class="tabbar" id="tabbar"><div class="tabs" id="tabs"></div><div class="doc-stats" id="doc-stats" aria-live="polite"></div><div class="encoding-control" id="encoding-control"><button class="encoding-btn" id="btn-encoding" type="button" title="{encoding_title}">{initial_encoding}</button><div class="encoding-popover" id="encoding-popover" style="display:none;" role="menu"><div class="encoding-popover-title">{encoding_title}</div><button type="button" class="encoding-option{opt_utf8}" data-encoding="UTF-8">UTF-8</button><button type="button" class="encoding-option{opt_gbk}" data-encoding="GBK">GBK / ANSI</button><button type="button" class="encoding-option{opt_u16le}" data-encoding="UTF-16 LE">UTF-16 LE</button><button type="button" class="encoding-option{opt_u16be}" data-encoding="UTF-16 BE">UTF-16 BE</button></div></div><button class="tab-open" id="tab-open" type="button" title="{btn_new}" aria-label="{btn_new}">+</button></div>
	<aside class="sidebar" id="sidebar" aria-label="{btn_sidebar}">
	  <div class="sidebar-sections">
	    <button type="button" data-sidebar-section="folder" aria-pressed="true">{sidebar_folder}</button>
	    <button type="button" data-sidebar-section="recent" aria-pressed="false">{sidebar_recent}</button>
	    <button type="button" data-sidebar-section="outline" aria-pressed="false">{sidebar_outline}</button>
	  </div>
	  <div class="sidebar-list" id="sidebar-list"></div>
	</aside>
	<div id="topbar">
	<div class="toolbar sidebar-toggle">
	  <button id="btn-sidebar" title="{btn_sidebar}" aria-label="{btn_sidebar}"></button>
	</div>
	<div class="toolbar">
	  <button id="btn-open" title="{btn_open}" aria-label="{btn_open}"></button>
	  <button id="btn-search" title="{btn_search}" aria-label="{btn_search}"></button>
	  <button id="btn-toggle" title="{btn_edit}" aria-label="{btn_edit}"></button>
	  <button id="btn-split" title="{btn_split}" aria-label="{btn_split}"></button>
	  <button id="btn-print" title="{btn_print}" aria-label="{btn_print}"></button>
	  <div class="zoom-control" id="zoom-control">
	    <button id="btn-zoom" title="{btn_zoom}" aria-label="{btn_zoom}"></button>
	    <div class="zoom-popover">
	      <button id="btn-zoom-out" title="{btn_zoom_out}" aria-label="{btn_zoom_out}">−</button>
	      <button id="btn-zoom-reset" class="zoom-reset" title="{btn_zoom_reset}" aria-label="{btn_zoom_reset}">100%</button>
	      <button id="btn-zoom-in" title="{btn_zoom_in}" aria-label="{btn_zoom_in}">+</button>
	    </div>
	  </div>
	  <div class="settings-control" id="settings-control">
	    <button id="btn-settings" title="{btn_settings}" aria-label="{btn_settings}"></button>
	    <div class="settings-popover" role="group" aria-label="{btn_settings}">
	      <div class="settings-row">
	        <span class="settings-label">{set_open_mode}</span>
	        <div class="settings-seg">
	          <button type="button" data-setting="open-mode" data-value="new-tab" aria-pressed="false">{set_open_tab}</button>
	          <button type="button" data-setting="open-mode" data-value="new-window" aria-pressed="false">{set_open_window}</button>
	        </div>
	      </div>
	      <div class="settings-row">
	        <span class="settings-label">{set_author_mode}<span class="settings-help" title="{author_help}" aria-label="{author_help}" role="img">?</span></span>
	        <div class="settings-seg">
	          <button type="button" data-setting="author-mode" data-value="off" aria-pressed="false">{set_author_off}</button>
	          <button type="button" data-setting="author-mode" data-value="on" aria-pressed="false">{set_author_on}</button>
	        </div>
	      </div>
	      <div class="settings-row">
	        <span class="settings-label">{set_tab_mode}</span>
	        <div class="settings-seg">
	          <button type="button" data-setting="tab-mode" data-value="accumulate" aria-pressed="false">{set_tab_keep}</button>
	          <button type="button" data-setting="tab-mode" data-value="single" aria-pressed="false">{set_tab_single}</button>
	        </div>
	      </div>
	      <div class="settings-row">
	        <span class="settings-label">{set_word_wrap}</span>
	        <div class="settings-seg">
	          <button type="button" data-setting="word-wrap" data-value="on" aria-pressed="true">{set_wrap_on}</button>
	          <button type="button" data-setting="word-wrap" data-value="off" aria-pressed="false">{set_wrap_off}</button>
	        </div>
	      </div>
	    </div>
	  </div>
	</div>
	</div>
	<div class="findbar" role="search">
	  <input id="find-input" type="search" placeholder="{search_placeholder}" aria-label="{search_placeholder}">
	  <span id="find-state"></span>
	  <button id="find-prev" title="Previous" aria-label="Previous"></button>
	  <button id="find-next" title="Next" aria-label="Next"></button>
	  <button id="find-close" title="Close" aria-label="Close"></button>
	</div>
	<div id="tab-context-menu" class="context-menu" style="display:none;" role="menu">
	  <button type="button" class="context-menu-item" data-tab-action="close">{tab_menu_close}</button>
	  <button type="button" class="context-menu-item" data-tab-action="close-others">{tab_menu_close_others}</button>
	  <div class="context-menu-sep"></div>
	  <button type="button" class="context-menu-item" data-tab-action="copy-path">{tab_menu_copy_path}</button>
	  <button type="button" class="context-menu-item" data-tab-action="reveal">{tab_menu_reveal}</button>
	</div>
	<div id="lightbox" class="lightbox" style="display:none;" role="dialog" aria-modal="true">
	  <div class="lightbox-backdrop"></div>
	  <div class="lightbox-toolbar">
	    <button type="button" id="lb-zoom-out" class="lightbox-btn" title="Zoom Out">−</button>
	    <button type="button" id="lb-zoom-reset" class="lightbox-btn" title="Reset">100%</button>
	    <button type="button" id="lb-zoom-in" class="lightbox-btn" title="Zoom In">+</button>
	    <button type="button" id="lb-close" class="lightbox-btn lb-close" title="Close">×</button>
	  </div>
	  <div class="lightbox-stage">
	    <img id="lb-img" class="lightbox-img" alt="">
	  </div>
	  <div id="lb-caption" class="lightbox-caption"></div>
	</div>
	<div id="app">
  <div id="preview">{preview_html}</div>
  <textarea id="editor" spellcheck="false">{raw_md_escaped}</textarea>
</div>
<script>
(function(){{
	  var ICON_EDIT = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>';
	  var ICON_VIEW = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>';
	  var ICON_OPEN = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 14 1.45-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.55 6A2 2 0 0 1 18.45 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/></svg>';
	  var ICON_SEARCH = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>';
	  var ICON_PRINT = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 6 2 18 2 18 9"/><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><rect x="6" y="14" width="12" height="8"/></svg>';
	  var ICON_ZOOM = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><path d="M8 11h6"/><path d="M11 8v6"/></svg>';
	  var ICON_SIDEBAR = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/></svg>';
	  var ICON_SETTINGS = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>';
	  var ICON_UP = '<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m18 15-6-6-6 6"/></svg>';
	  var ICON_DOWN = '<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>';
	  var ICON_CLOSE = '<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>';
	  var ICON_SPLIT = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="12" y1="3" x2="12" y2="21"/></svg>';
	  var L_EDIT = '{btn_edit}', L_VIEW = '{btn_preview}';
	  var L_COPY = '{code_copy_js}', L_COPIED = '{code_copied_js}';
	  var L_SPLIT = '{btn_split}';
	  var SIDEBAR_OUTLINE_EMPTY = '{sidebar_outline_empty_js}';

	  var btnOpen = document.getElementById('btn-open');
	  var btnSearch = document.getElementById('btn-search');
	  var btnToggle = document.getElementById('btn-toggle');
	  var btnSplit = document.getElementById('btn-split');
	  var btnPrint = document.getElementById('btn-print');
	  var btnZoom = document.getElementById('btn-zoom');
	  var btnZoomOut = document.getElementById('btn-zoom-out');
	  var btnZoomReset = document.getElementById('btn-zoom-reset');
	  var btnZoomIn = document.getElementById('btn-zoom-in');
	  var zoomControl = document.getElementById('zoom-control');
	  var btnSettings = document.getElementById('btn-settings');
	  var settingsControl = document.getElementById('settings-control');
	  var btnSidebar = document.getElementById('btn-sidebar');
	  var sidebarEl = document.getElementById('sidebar');
	  var sidebarList = document.getElementById('sidebar-list');
	  var sidebarData = {{ folder: [], recent: [] }};
	  var sidebarSection = 'folder';
	  var SIDEBAR_EMPTY = '{sidebar_empty_js}';
	  var findInput = document.getElementById('find-input');
	  var findState = document.getElementById('find-state');
	  var findPrev = document.getElementById('find-prev');
	  var findNext = document.getElementById('find-next');
	  var findClose = document.getElementById('find-close');
	  var tabsEl = document.getElementById('tabs');
	  var docStats = document.getElementById('doc-stats');
	  var btnEncoding = document.getElementById('btn-encoding');
	  var encodingPopover = document.getElementById('encoding-popover');
	  var currentEncoding = (btnEncoding && btnEncoding.textContent.trim()) || 'UTF-8';
	  var tabOpen = document.getElementById('tab-open');
	  var ta = document.getElementById('editor');
	  var previewEl = document.getElementById('preview');
	  var tabContextMenu = document.getElementById('tab-context-menu');
	  var contextMenuTabId = null;
	  var contextMenuTabPath = '';
	  var lightbox = document.getElementById('lightbox');
	  var lbImg = document.getElementById('lb-img');
	  var lbCaption = document.getElementById('lb-caption');
	  var lbClose = document.getElementById('lb-close');
	  var lbZoomIn = document.getElementById('lb-zoom-in');
	  var lbZoomOut = document.getElementById('lb-zoom-out');
	  var lbZoomReset = document.getElementById('lb-zoom-reset');
	  var lbScale = 1.0;
	  var lbTranslateX = 0;
	  var lbTranslateY = 0;
	  var lbIsDragging = false;
	  var lbStartX = 0;
	  var lbStartY = 0;
	  var pendingLiveRenderTimer = 0;
	  var dirty = false;
	  var activeTabId = 0;
	  var pendingAutosaveTimer = 0;
	  var autosavePaused = false;
	  var AUTOSAVE_DEBOUNCE_MS = 700;
	  var composingFind = false;
	  var pendingFindTimer = 0;
	  var FIND_DEBOUNCE_MS = 300;
	  var findHits = [];
	  var currentFindHit = -1;
	  var lastFindQuery = '';
	  var STAT_WORDS = '{stat_words_js}';
	  var STAT_CHARS = '{stat_chars_js}';
	  var ZOOM_STORAGE_KEY = 'md-previewer-content-zoom-v1';
	  var ZOOM_MIN = 70;
	  var ZOOM_MAX = 200;
	  var ZOOM_STEP = 10;
	  var zoomPercent = 100;

	  btnOpen.innerHTML = ICON_OPEN;
	  btnSearch.innerHTML = ICON_SEARCH;
	  btnToggle.innerHTML = ICON_EDIT;
	  if (btnSplit) btnSplit.innerHTML = ICON_SPLIT;
	  btnPrint.innerHTML = ICON_PRINT;
	  btnZoom.innerHTML = ICON_ZOOM;
	  btnSettings.innerHTML = ICON_SETTINGS;
	  btnSidebar.innerHTML = ICON_SIDEBAR;
	  findPrev.innerHTML = ICON_UP;
	  findNext.innerHTML = ICON_DOWN;
	  findClose.innerHTML = ICON_CLOSE;

  function inEdit() {{ return document.body.classList.contains('editing'); }}
  function textSegments(text) {{
    if (typeof Intl !== 'undefined' && Intl.Segmenter) {{
      var segmenter = new Intl.Segmenter(undefined, {{ granularity: 'grapheme' }});
      return Array.from(segmenter.segment(text), function(item) {{ return item.segment; }});
    }}
    return Array.from(text);
  }}
  function updateDocumentStats(raw) {{
    var segments = textSegments(String(raw || ''));
    var words = segments.reduce(function(total, segment) {{
      return total + (/^\s+$/u.test(segment) ? 0 : 1);
    }}, 0);
    var number = new Intl.NumberFormat().format;
    docStats.textContent =
      number(words) + ' ' + STAT_WORDS + ' · ' + number(segments.length) + ' ' + STAT_CHARS;
  }}
  function loadZoomPercent() {{
    try {{
      var stored = Number(localStorage.getItem(ZOOM_STORAGE_KEY));
      if (Number.isFinite(stored) && stored >= ZOOM_MIN && stored <= ZOOM_MAX) return stored;
    }} catch (_) {{}}
    return 100;
  }}
  function applyZoom(percent, persist) {{
    zoomPercent = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, percent));
    document.documentElement.style.setProperty('--content-scale', String(zoomPercent / 100));
    btnZoomReset.textContent = zoomPercent + '%';
    if (persist) {{
      try {{ localStorage.setItem(ZOOM_STORAGE_KEY, String(zoomPercent)); }} catch (_) {{}}
    }}
    if (inEdit()) autoResize();
  }}
  function changeZoom(delta) {{
    applyZoom(zoomPercent + delta, true);
  }}
  function currentScrollProgress() {{
    var root = document.documentElement;
    var max = Math.max(root.scrollHeight - window.innerHeight, 0);
    var y = window.scrollY || root.scrollTop || 0;
    return max > 0 ? Math.min(1, Math.max(0, y / max)) : 0;
  }}
  function restoreScrollProgress(progress) {{
    function restore() {{
      var max = Math.max(document.documentElement.scrollHeight - window.innerHeight, 0);
      window.scrollTo(window.scrollX || 0, max * progress);
    }}
    restore();
    requestAnimationFrame(function() {{
      restore();
      requestAnimationFrame(restore);
    }});
  }}
  applyZoom(loadZoomPercent(), false);
  updateDocumentStats(ta.value);
  function setEncodingUi(enc) {{
    if (!enc) return;
    currentEncoding = enc;
    if (btnEncoding) btnEncoding.textContent = enc;
    if (encodingPopover) {{
      var options = encodingPopover.querySelectorAll('.encoding-option');
      for (var i = 0; i < options.length; i++) {{
        if (options[i].getAttribute('data-encoding') === enc) {{
          options[i].classList.add('active');
        }} else {{
          options[i].classList.remove('active');
        }}
      }}
    }}
  }}
  window.__setEncoding = setEncodingUi;
  if (btnEncoding && encodingPopover) {{
    btnEncoding.addEventListener('click', function(e) {{
      e.stopPropagation();
      var isOpen = encodingPopover.style.display !== 'none';
      encodingPopover.style.display = isOpen ? 'none' : 'block';
    }});
    encodingPopover.addEventListener('click', function(e) {{
      var opt = e.target.closest('.encoding-option');
      if (opt) {{
        var enc = opt.getAttribute('data-encoding');
        encodingPopover.style.display = 'none';
        if (enc && enc !== currentEncoding && window.ipc) {{
          window.ipc.postMessage('set-encoding:' + enc);
        }}
      }}
    }});
  }}
  document.addEventListener('contextmenu', function(e) {{
    if (!inEdit() || e.target !== ta) e.preventDefault();
  }});
  function setDirty(d) {{
    if (dirty === d) return;
    dirty = d;
    window.ipc.postMessage(d ? 'dirty:1' : 'dirty:0');
  }}
	  function cancelPendingAutosave() {{
	    if (!pendingAutosaveTimer) return;
	    clearTimeout(pendingAutosaveTimer);
	    pendingAutosaveTimer = 0;
	  }}
	  function save() {{
	    cancelPendingAutosave();
	    if (!dirty) return;
	    window.ipc.postMessage('save:' + ta.value);
	  }}
	  function scheduleAutosave() {{
	    cancelPendingAutosave();
	    if (autosavePaused) return;
	    pendingAutosaveTimer = setTimeout(function() {{
	      pendingAutosaveTimer = 0;
	      if (dirty) save();
	    }}, AUTOSAVE_DEBOUNCE_MS);
	  }}
	  window.__mdPreviewerSave = save;
	  function requestTabAction(action, id) {{
	    cancelPendingAutosave();
	    var message = 'tab-action:' + action + ':' + id;
	    if (dirty) message += '\n' + ta.value;
	    window.ipc.postMessage(message);
	  }}
	  function openFile() {{
	    if (inEdit()) leaveEdit();
	    window.ipc.postMessage('open');
	  }}
	  window.__mdPreviewerOpenFile = openFile;
	  function newFile() {{
	    if (inEdit()) leaveEdit();
	    window.ipc.postMessage('new-file');
	  }}
	  window.__mdPreviewerNewFile = newFile;
	  function showFind() {{
	    if (document.body.classList.contains('empty')) return;
	    if (inEdit()) return;
	    document.body.classList.add('finding');
	    setTimeout(function(){{ findInput.focus(); findInput.select(); }}, 0);
	  }}
	  window.__mdPreviewerShowFind = showFind;
	  function hideFind() {{
	    document.body.classList.remove('finding');
	    findInput.value = '';
	    clearFindHits();
	    if (pendingFindTimer) {{ clearTimeout(pendingFindTimer); pendingFindTimer = 0; }}
	    var sel = window.getSelection && window.getSelection();
	    if (sel && sel.removeAllRanges) sel.removeAllRanges();
	  }}
	  function updateFindState() {{
	    findState.textContent = findHits.length ? (currentFindHit + 1) + '/' + findHits.length : '';
	  }}
	  function clearFindHits() {{
	    findHits.forEach(function(mark) {{
	      var parent = mark.parentNode;
	      if (!parent) return;
	      parent.replaceChild(document.createTextNode(mark.textContent), mark);
	      parent.normalize();
	    }});
	    findHits = [];
	    currentFindHit = -1;
	    lastFindQuery = '';
	    updateFindState();
	  }}
	  function focusFindInput(selectionStart, selectionEnd) {{
	    if (!document.body.classList.contains('finding')) return;
	    try {{
	      findInput.focus({{ preventScroll: true }});
	    }} catch (_) {{
	      findInput.focus();
	    }}
	    if (typeof selectionStart === 'number' && typeof selectionEnd === 'number') {{
	      try {{ findInput.setSelectionRange(selectionStart, selectionEnd); }} catch (_) {{}}
	    }}
	  }}
	  function restoreFindInput(selectionStart, selectionEnd) {{
	    setTimeout(function() {{ focusFindInput(selectionStart, selectionEnd); }}, 0);
	    requestAnimationFrame(function() {{
	      focusFindInput(selectionStart, selectionEnd);
	      setTimeout(function() {{ focusFindInput(selectionStart, selectionEnd); }}, 80);
	    }});
	  }}
	  function selectFindHit(index) {{
	    if (!findHits.length) {{
	      currentFindHit = -1;
	      updateFindState();
	      return;
	    }}
	    if (currentFindHit >= 0 && findHits[currentFindHit]) {{
	      findHits[currentFindHit].classList.remove('current');
	    }}
	    currentFindHit = (index + findHits.length) % findHits.length;
	    var hit = findHits[currentFindHit];
	    hit.classList.add('current');
	    hit.scrollIntoView({{ block: 'center', inline: 'nearest' }});
	    updateFindState();
	  }}
	  function runFindQuery(query) {{
	    clearFindHits();
	    query = String(query || '').trim();
	    if (!query) return;
	    lastFindQuery = query;
	    var needle = query.toLowerCase();
	    var previewEl = document.getElementById('preview');
	    var walker = document.createTreeWalker(previewEl, NodeFilter.SHOW_TEXT, {{
	      acceptNode: function(node) {{
	        if (!node.nodeValue || node.nodeValue.toLowerCase().indexOf(needle) < 0) {{
	          return NodeFilter.FILTER_REJECT;
	        }}
	        var parent = node.parentElement;
	        if (!parent || parent.closest('script,style,svg,mark.search-hit,.katex,.mdp-mermaid')) {{
	          return NodeFilter.FILTER_REJECT;
	        }}
	        return NodeFilter.FILTER_ACCEPT;
	      }}
	    }});
	    var nodes = [];
	    while (walker.nextNode()) nodes.push(walker.currentNode);
	    nodes.forEach(function(node) {{
	      var text = node.nodeValue;
	      var lower = text.toLowerCase();
	      var fragment = document.createDocumentFragment();
	      var start = 0;
	      var index;
	      while ((index = lower.indexOf(needle, start)) >= 0) {{
	        if (index > start) fragment.appendChild(document.createTextNode(text.slice(start, index)));
	        var mark = document.createElement('mark');
	        mark.className = 'search-hit';
	        mark.textContent = text.slice(index, index + query.length);
	        findHits.push(mark);
	        fragment.appendChild(mark);
	        start = index + query.length;
	      }}
	      if (start < text.length) fragment.appendChild(document.createTextNode(text.slice(start)));
	      node.parentNode.replaceChild(fragment, node);
	    }});
	    selectFindHit(0);
	  }}
	  function runFind(backward) {{
	    var q = findInput.value;
	    if (!q) {{ clearFindHits(); return; }}
	    var hadFocus = document.activeElement === findInput;
	    var selectionStart = findInput.selectionStart;
	    var selectionEnd = findInput.selectionEnd;
	    var normalized = String(q || '').trim();
	    if (!normalized) {{
	      clearFindHits();
	    }} else if (normalized !== lastFindQuery) {{
	      runFindQuery(normalized);
	    }} else {{
	      selectFindHit(currentFindHit + (backward ? -1 : 1));
	    }}
	    if (hadFocus) restoreFindInput(selectionStart, selectionEnd);
	  }}
	  function scheduleFind() {{
	    if (pendingFindTimer) clearTimeout(pendingFindTimer);
	    pendingFindTimer = setTimeout(function() {{
	      pendingFindTimer = 0;
	      if (!composingFind) runFindQuery(findInput.value);
	    }}, FIND_DEBOUNCE_MS);
	  }}
  // Grow textarea height to its content so the page (html) owns the sole
  // scrollbar; avoids the double-scrollbar you see if textarea keeps its
  // own internal scroll.
  function autoResize() {{
    var x = window.scrollX || document.documentElement.scrollLeft || 0;
    var y = window.scrollY || document.documentElement.scrollTop || 0;
    ta.style.height = 'auto';
    var h = Math.max(ta.scrollHeight, window.innerHeight);
    ta.style.height = h + 'px';
    window.scrollTo(x, y);
  }}
	  function enterEdit() {{
	    var progress = currentScrollProgress();
	    document.body.classList.add('editing');
	    btnToggle.innerHTML = ICON_VIEW;
	    btnToggle.title = L_VIEW;
	    btnToggle.setAttribute('aria-label', L_VIEW);
	    if (document.body.classList.contains('split-view')) scheduleLiveRender(0);
	    autoResize();
	    try {{
	      ta.focus({{ preventScroll: true }});
	    }} catch (_) {{
	      ta.focus();
	    }}
	    restoreScrollProgress(progress);
	  }}
  function leaveEdit() {{
    var progress = currentScrollProgress();
    if (dirty) save();
    document.body.classList.remove('editing');
    btnToggle.innerHTML = ICON_EDIT;
    btnToggle.title = L_EDIT;
    btnToggle.setAttribute('aria-label', L_EDIT);
    if (pendingLiveRenderTimer) {{ clearTimeout(pendingLiveRenderTimer); pendingLiveRenderTimer = 0; }}
    restoreScrollProgress(progress);
  }}
  function toggleSplitView() {{
    var split = !document.body.classList.contains('split-view');
    document.body.classList.toggle('split-view', split);
    if (btnSplit) btnSplit.setAttribute('aria-pressed', split ? 'true' : 'false');
    if (split) {{
      scheduleLiveRender(0);
    }}
  }}
  function scheduleLiveRender(delay) {{
    if (!document.body.classList.contains('split-view')) return;
    if (pendingLiveRenderTimer) clearTimeout(pendingLiveRenderTimer);
    pendingLiveRenderTimer = setTimeout(function() {{
      pendingLiveRenderTimer = 0;
      if (window.ipc) window.ipc.postMessage('render-preview:' + ta.value);
    }}, typeof delay === 'number' ? delay : 150);
  }}
  window.__setLivePreview = function(html, math, mermaid) {{
    if (!document.body.classList.contains('split-view')) return;
    document.getElementById('preview').innerHTML = html;
    if (typeof hljs !== 'undefined') hljs.highlightAll();
    setupCodeBlockCopyButtons();
    if (sidebarSection === 'outline') renderSidebar();
  }};
  window.__mdPreviewerToggleEdit = function() {{
    if (inEdit()) leaveEdit(); else enterEdit();
  }};
	window.__mdPreviewerEnterEdit = function() {{
	  if (!inEdit()) enterEdit();
	}};
	window.__mdPreviewerCloseActiveTab = function() {{
	  if (activeTabId) requestTabAction('close', activeTabId);
	}};

	  btnOpen.addEventListener('click', openFile);
	  tabOpen.addEventListener('click', newFile);
	  if (btnSplit) {{
	    btnSplit.addEventListener('click', function() {{
	      toggleSplitView();
	    }});
	  }}
	  btnSearch.addEventListener('click', showFind);
	  document.addEventListener('click', function(e) {{
	    if (tabContextMenu && tabContextMenu.style.display !== 'none' && !tabContextMenu.contains(e.target)) {{
	      hideTabContextMenu();
	    }}
	    var closeTab = e.target && e.target.closest ? e.target.closest('[data-close-tab]') : null;
	    if (closeTab) {{
	      e.preventDefault();
	      e.stopPropagation();
	      requestTabAction('close', closeTab.getAttribute('data-close-tab'));
	      return;
	    }}
	    var locateTab = e.target && e.target.closest ? e.target.closest('[data-locate-tab]') : null;
	    if (locateTab) {{
	      e.preventDefault();
	      window.ipc.postMessage('locate-tab:' + locateTab.getAttribute('data-locate-tab'));
	      return;
	    }}
	    var tab = e.target && e.target.closest ? e.target.closest('[data-tab-id]') : null;
	    if (tab) {{
	      e.preventDefault();
	      requestTabAction('activate', tab.getAttribute('data-tab-id'));
	      return;
	    }}
	    var copyBtn = e.target && e.target.closest ? e.target.closest('.code-copy-btn') : null;
	    if (copyBtn) {{
	      e.preventDefault();
	      e.stopPropagation();
	      var pre = copyBtn.closest('pre');
	      if (!pre) return;
	      var code = pre.querySelector('code');
	      var text = code ? code.innerText : pre.innerText;
	      if (!code && text.endsWith(copyBtn.innerText)) {{
	        text = text.slice(0, text.length - copyBtn.innerText.length).trimEnd();
	      }}
	      copyText(text, function() {{
	        var orig = copyBtn.textContent;
	        copyBtn.textContent = L_COPIED;
	        copyBtn.classList.add('copied');
	        setTimeout(function() {{
	          copyBtn.textContent = orig;
	          copyBtn.classList.remove('copied');
	        }}, 1500);
	      }});
	      return;
	    }}
	    var img = e.target && e.target.closest ? e.target.closest('#preview img') : null;
	    if (img && !inEdit()) {{
	      e.preventDefault();
	      openLightbox(img.src, img.alt || img.title || '');
	      return;
	    }}
	    var openBtn = e.target && e.target.closest ? e.target.closest('[data-open-file]') : null;
	    if (openBtn) {{
	      e.preventDefault();
	      openFile();
	      return;
	    }}
	    var recentBtn = e.target && e.target.closest ? e.target.closest('[data-recent-index]') : null;
	    if (recentBtn) {{
	      e.preventDefault();
	      window.ipc.postMessage('open-recent:' + recentBtn.getAttribute('data-recent-index'));
	    }}
	  }});
	  findInput.addEventListener('compositionstart', function() {{ composingFind = true; }});
	  findInput.addEventListener('compositionend', function() {{ composingFind = false; scheduleFind(); }});
	  findInput.addEventListener('input', function(e) {{
	    if (composingFind || e.isComposing) return;
	    scheduleFind();
	  }});
	  findInput.addEventListener('keydown', function(e) {{
	    if (composingFind || e.isComposing) return;
	    if (e.key === 'Enter') {{ e.preventDefault(); runFind(e.shiftKey); }}
	    if (e.key === 'Escape') {{ e.preventDefault(); hideFind(); }}
	  }});
	  findPrev.addEventListener('click', function() {{ runFind(true); }});
	  findNext.addEventListener('click', function() {{ runFind(false); }});
	  findClose.addEventListener('click', hideFind);

	  btnToggle.addEventListener('click', function() {{
	    window.__mdPreviewerToggleEdit();
	  }});
	  btnZoom.addEventListener('click', function(e) {{
	    e.stopPropagation();
	    settingsControl.classList.remove('open');
	    zoomControl.classList.toggle('open');
	  }});
	  btnSettings.addEventListener('click', function(e) {{
	    e.stopPropagation();
	    zoomControl.classList.remove('open');
	    settingsControl.classList.toggle('open');
	  }});
	  // 只上报点击，选中态一律等 Rust 存盘后通过 __setSettings 回显，避免界面和实际配置不一致
	  settingsControl.addEventListener('click', function(e) {{
	    var btn = e.target && e.target.closest ? e.target.closest('[data-setting]') : null;
	    if (!btn) return;
	    e.preventDefault();
	    window.ipc.postMessage('set-setting:' + btn.getAttribute('data-setting') + '=' + btn.getAttribute('data-value'));
	  }});
	  function renderSidebar() {{
	    var sections = sidebarEl.querySelectorAll('[data-sidebar-section]');
	    for (var s = 0; s < sections.length; s++) {{
	      sections[s].setAttribute('aria-pressed', sections[s].getAttribute('data-sidebar-section') === sidebarSection ? 'true' : 'false');
	    }}
	    sidebarList.textContent = '';
	    if (sidebarSection === 'outline') {{
	      var preview = document.getElementById('preview');
	      var headings = preview ? preview.querySelectorAll('h1, h2, h3, h4, h5, h6') : [];
	      if (!headings || !headings.length) {{
	        var empty = document.createElement('div');
	        empty.className = 'sidebar-empty';
	        empty.textContent = SIDEBAR_OUTLINE_EMPTY;
	        sidebarList.appendChild(empty);
	        return;
	      }}
	      for (var i = 0; i < headings.length; i++) {{
	        var h = headings[i];
	        if (!h.id) h.id = 'heading-' + i;
	        var level = parseInt(h.tagName.substring(1), 10) || 1;
	        var text = (h.innerText || h.textContent || '').replace(/\s+$/, '');
	        var authorActions = h.querySelector('.author-actions');
	        if (authorActions) {{
	          text = text.replace(authorActions.innerText, '').trim();
	        }}
	        var btn = document.createElement('button');
	        btn.type = 'button';
	        btn.className = 'sidebar-item outline-item outline-level-' + level;
	        btn.setAttribute('data-outline-id', h.id);
	        btn.title = text;
	        var name = document.createElement('span');
	        name.className = 'sidebar-name';
	        name.textContent = text;
	        btn.appendChild(name);
	        sidebarList.appendChild(btn);
	      }}
	      updateOutlineActive();
	      return;
	    }}
	    var items = sidebarData[sidebarSection] || [];
	    if (!items.length) {{
	      var empty = document.createElement('div');
	      empty.className = 'sidebar-empty';
	      empty.textContent = SIDEBAR_EMPTY;
	      sidebarList.appendChild(empty);
	      return;
	    }}
	    items.forEach(function(item) {{
	      var btn = document.createElement('button');
	      btn.type = 'button';
	      btn.className = 'sidebar-item' + (item.active ? ' active' : '');
	      btn.setAttribute('data-sidebar-path', item.path);
	      btn.title = item.path;
	      var name = document.createElement('span');
	      name.className = 'sidebar-name';
	      name.textContent = item.name;
	      btn.appendChild(name);
	      // 最近打开会跨目录，补一行所在目录才分得清同名文件
	      if (sidebarSection === 'recent') {{
	        var dir = document.createElement('span');
	        dir.className = 'sidebar-dir';
	        dir.textContent = item.dir;
	        btn.appendChild(dir);
	      }}
	      sidebarList.appendChild(btn);
	    }});
	  }}
	  function updateOutlineActive() {{
	    if (sidebarSection !== 'outline') return;
	    var preview = document.getElementById('preview');
	    if (!preview) return;
	    var headings = preview.querySelectorAll('h1, h2, h3, h4, h5, h6');
	    if (!headings.length) return;
	    var activeId = null;
	    for (var i = 0; i < headings.length; i++) {{
	      var rect = headings[i].getBoundingClientRect();
	      if (rect.top <= 120) {{
	        activeId = headings[i].id;
	      }} else {{
	        break;
	      }}
	    }}
	    if (!activeId && headings.length > 0) activeId = headings[0].id;
	    var items = sidebarList.querySelectorAll('.outline-item');
	    for (var j = 0; j < items.length; j++) {{
	      var match = items[j].getAttribute('data-outline-id') === activeId;
	      items[j].classList.toggle('active', match);
	    }}
	  }}
	  window.addEventListener('scroll', function() {{
	    if (sidebarSection === 'outline') updateOutlineActive();
	  }}, {{ passive: true }});
	  window.__setSidebar = function(data) {{
	    sidebarData = data || {{ folder: [], recent: [] }};
	    renderSidebar();
	  }};
	  btnSidebar.addEventListener('click', function(e) {{
	    e.stopPropagation();
	    // 立刻切换视觉状态，落盘交给 Rust；回显时状态一致，不会来回跳
	    var open = !document.body.classList.contains('sidebar-open');
	    document.body.classList.toggle('sidebar-open', open);
	    window.ipc.postMessage('set-setting:sidebar=' + (open ? '1' : '0'));
	  }});
	  sidebarEl.addEventListener('click', function(e) {{
	    var section = e.target && e.target.closest ? e.target.closest('[data-sidebar-section]') : null;
	    if (section) {{
	      sidebarSection = section.getAttribute('data-sidebar-section');
	      renderSidebar();
	      return;
	    }}
	    var outlineItem = e.target && e.target.closest ? e.target.closest('[data-outline-id]') : null;
	    if (outlineItem) {{
	      var targetHeading = document.getElementById(outlineItem.getAttribute('data-outline-id'));
	      if (targetHeading) {{
	        targetHeading.scrollIntoView({{ behavior: 'smooth', block: 'start' }});
	      }}
	      return;
	    }}
	    var item = e.target && e.target.closest ? e.target.closest('[data-sidebar-path]') : null;
	    if (item) window.ipc.postMessage('open-doc:' + item.getAttribute('data-sidebar-path'));
	  }});
	  renderSidebar();
	  var authorDoc = {{ titleLine: '', title: '' }};
	  var authorMode = false;
	  var AUTHOR_LABELS = {{
	    'title-line': '{copy_title_line_js}',
	    'title': '{copy_title_js}',
	    'body': '{copy_body_js}'
	  }};
	  var AUTHOR_COPIED = '{copied_js}';

	  function authorButton(kind) {{
	    var btn = document.createElement('button');
	    btn.type = 'button';
	    btn.className = 'author-copy';
	    btn.setAttribute('data-author-copy', kind);
	    btn.textContent = AUTHOR_LABELS[kind];
	    return btn;
	  }}
	  // 正文按屏幕上看到的取：逐个顶层块读 innerText，段落之间留空行。
	  // 用渲染结果而不是 Markdown 原文，复制出来才不会带 # * ` 这些语法
	  function authorBodyText() {{
	    var preview = document.getElementById('preview');
	    if (!preview) return '';
	    var heading = preview.querySelector('h1, h2, h3, h4, h5, h6');
	    var reached = !heading;
	    var blocks = [];
	    for (var i = 0; i < preview.children.length; i++) {{
	      var node = preview.children[i];
	      if (!reached) {{
	        if (node === heading) reached = true;
	        continue;
	      }}
	      if (node.classList && node.classList.contains('author-body-actions')) continue;
	      var text = (node.innerText || node.textContent || '').replace(/\s+$/, '');
	      if (text) blocks.push(text);
	    }}
	    return blocks.join('\n\n');
	  }}
	  function applyAuthorMode() {{
	    var preview = document.getElementById('preview');
	    if (!preview) return;
	    var stale = preview.querySelectorAll('.author-actions, .author-body-actions');
	    for (var i = 0; i < stale.length; i++) {{
	      stale[i].parentNode.removeChild(stale[i]);
	    }}
	    if (!authorMode) return;
	    var heading = preview.querySelector('h1, h2, h3, h4, h5, h6');
	    if (heading && authorDoc.titleLine) {{
	      var actions = document.createElement('span');
	      actions.className = 'author-actions';
	      actions.appendChild(authorButton('title-line'));
	      actions.appendChild(authorButton('title'));
	      heading.appendChild(actions);
	    }}
	    if (!authorBodyText()) return;
	    var bodyActions = document.createElement('div');
	    bodyActions.className = 'author-body-actions';
	    bodyActions.appendChild(authorButton('body'));
	    if (heading) preview.insertBefore(bodyActions, heading.nextSibling);
	    else preview.insertBefore(bodyActions, preview.firstChild);
	  }}
	  window.__applyAuthorMode = applyAuthorMode;
	  window.__setAuthorDoc = function(doc) {{
	    authorDoc = doc || {{ titleLine: '', title: '' }};
	    applyAuthorMode();
	  }};
	  // 页面由 with_html 载入，不是安全上下文，navigator.clipboard 未必可用，所以留一条 execCommand 退路
	  function authorCopyFallback(text) {{
	    var holder = document.createElement('textarea');
	    holder.value = text;
	    holder.setAttribute('readonly', '');
	    holder.style.position = 'fixed';
	    holder.style.top = '-1000px';
	    document.body.appendChild(holder);
	    holder.select();
	    var ok = false;
	    try {{ ok = document.execCommand('copy'); }} catch (err) {{ ok = false; }}
	    document.body.removeChild(holder);
	    return ok;
	  }}
	  function copyText(text, callback) {{
	    if (!text) return;
	    if (navigator.clipboard && navigator.clipboard.writeText) {{
	      navigator.clipboard.writeText(text).then(function() {{
	        if (callback) callback();
	      }}, function() {{
	        if (authorCopyFallback(text) && callback) callback();
	      }});
	    }} else {{
	      if (authorCopyFallback(text) && callback) callback();
	    }}
	  }}
	  function setupCodeBlockCopyButtons() {{
	    var preview = document.getElementById('preview');
	    if (!preview) return;
	    var pres = preview.querySelectorAll('pre');
	    for (var i = 0; i < pres.length; i++) {{
	      var pre = pres[i];
	      if (pre.classList && (pre.classList.contains('front-matter') || pre.classList.contains('mdp-mermaid'))) continue;
	      if (pre.querySelector('.code-copy-btn')) continue;
	      var btn = document.createElement('button');
	      btn.type = 'button';
	      btn.className = 'code-copy-btn';
	      btn.textContent = L_COPY;
	      btn.setAttribute('aria-label', L_COPY);
	      pre.appendChild(btn);
	    }}
	  }}
	  function showTabContextMenu(id, path, x, y) {{
	    if (!tabContextMenu) return;
	    contextMenuTabId = id;
	    contextMenuTabPath = path || '';
	    tabContextMenu.style.display = 'block';
	    var menuWidth = tabContextMenu.offsetWidth || 160;
	    var menuHeight = tabContextMenu.offsetHeight || 130;
	    var posX = Math.min(x, window.innerWidth - menuWidth - 8);
	    var posY = Math.min(y, window.innerHeight - menuHeight - 8);
	    tabContextMenu.style.left = Math.max(8, posX) + 'px';
	    tabContextMenu.style.top = Math.max(8, posY) + 'px';
	  }}
	  function hideTabContextMenu() {{
	    if (tabContextMenu) tabContextMenu.style.display = 'none';
	    contextMenuTabId = null;
	    contextMenuTabPath = '';
	  }}
	  if (tabsEl) {{
	    tabsEl.addEventListener('contextmenu', function(e) {{
	      var tab = e.target && e.target.closest ? e.target.closest('[data-tab-id]') : null;
	      if (!tab) return;
	      e.preventDefault();
	      e.stopPropagation();
	      showTabContextMenu(tab.getAttribute('data-tab-id'), tab.title, e.clientX, e.clientY);
	    }});
	    tabsEl.addEventListener('auxclick', function(e) {{
	      if (e.button === 1) {{
	        var tab = e.target && e.target.closest ? e.target.closest('[data-tab-id]') : null;
	        if (tab) {{
	          e.preventDefault();
	          e.stopPropagation();
	          requestTabAction('close', tab.getAttribute('data-tab-id'));
	        }}
	      }}
	    }});
	  }}
	  if (tabContextMenu) {{
	    tabContextMenu.addEventListener('click', function(e) {{
	      var item = e.target && e.target.closest ? e.target.closest('[data-tab-action]') : null;
	      if (!item) return;
	      var action = item.getAttribute('data-tab-action');
	      var id = contextMenuTabId;
	      var path = contextMenuTabPath;
	      hideTabContextMenu();
	      if (!id) return;
	      if (action === 'close') {{
	        requestTabAction('close', id);
	      }} else if (action === 'close-others') {{
	        requestTabAction('close-others', id);
	      }} else if (action === 'copy-path') {{
	        if (path) copyText(path);
	      }} else if (action === 'reveal') {{
	        window.ipc.postMessage('reveal-tab:' + id);
	      }}
	    }});
	  }}
	  function updateLightboxTransform() {{
	    if (!lbImg) return;
	    lbImg.style.transform = 'translate(' + lbTranslateX + 'px, ' + lbTranslateY + 'px) scale(' + lbScale + ')';
	    if (lbZoomReset) lbZoomReset.textContent = Math.round(lbScale * 100) + '%';
	  }}
	  function openLightbox(src, caption) {{
	    if (!lightbox || !lbImg) return;
	    lbImg.src = src;
	    if (lbCaption) lbCaption.textContent = caption || '';
	    lbScale = 1.0;
	    lbTranslateX = 0;
	    lbTranslateY = 0;
	    updateLightboxTransform();
	    lightbox.style.display = 'flex';
	    document.body.classList.add('lightbox-open');
	  }}
	  function closeLightbox() {{
	    if (!lightbox) return;
	    lightbox.style.display = 'none';
	    document.body.classList.remove('lightbox-open');
	    if (lbImg) lbImg.src = '';
	  }}
	  if (lbClose) lbClose.addEventListener('click', function(e) {{ e.stopPropagation(); closeLightbox(); }});
	  if (lbZoomIn) lbZoomIn.addEventListener('click', function(e) {{
	    e.stopPropagation();
	    lbScale = Math.min(5.0, lbScale + 0.25);
	    updateLightboxTransform();
	  }});
	  if (lbZoomOut) lbZoomOut.addEventListener('click', function(e) {{
	    e.stopPropagation();
	    lbScale = Math.max(0.2, lbScale - 0.25);
	    updateLightboxTransform();
	  }});
	  if (lbZoomReset) lbZoomReset.addEventListener('click', function(e) {{
	    e.stopPropagation();
	    lbScale = 1.0;
	    lbTranslateX = 0;
	    lbTranslateY = 0;
	    updateLightboxTransform();
	  }});
	  if (lightbox) {{
	    lightbox.addEventListener('click', function(e) {{
	      if (e.target === lightbox || (e.target && e.target.classList && (e.target.classList.contains('lightbox-backdrop') || e.target.classList.contains('lightbox-stage')))) {{
	        closeLightbox();
	      }}
	    }});
	    lightbox.addEventListener('wheel', function(e) {{
	      e.preventDefault();
	      var delta = e.deltaY < 0 ? 0.2 : -0.2;
	      lbScale = Math.min(5.0, Math.max(0.2, lbScale + delta));
	      updateLightboxTransform();
	    }}, {{ passive: false }});
	  }}
	  if (lbImg) {{
	    lbImg.addEventListener('mousedown', function(e) {{
	      if (e.button !== 0) return;
	      e.preventDefault();
	      lbIsDragging = true;
	      lbStartX = e.clientX - lbTranslateX;
	      lbStartY = e.clientY - lbTranslateY;
	      var stage = lbImg.closest('.lightbox-stage');
	      if (stage) stage.classList.add('dragging');
	    }});
	  }}
	  window.addEventListener('mousemove', function(e) {{
	    if (!lbIsDragging) return;
	    lbTranslateX = e.clientX - lbStartX;
	    lbTranslateY = e.clientY - lbStartY;
	    updateLightboxTransform();
	  }});
	  window.addEventListener('mouseup', function() {{
	    if (!lbIsDragging) return;
	    lbIsDragging = false;
	    if (lbImg) {{
	      var stage = lbImg.closest('.lightbox-stage');
	      if (stage) stage.classList.remove('dragging');
	    }}
	  }});
	  function flashCopied(btn) {{
	    var original = btn.textContent;
	    btn.textContent = AUTHOR_COPIED;
	    btn.classList.add('done');
	    setTimeout(function() {{
	      btn.textContent = original;
	      btn.classList.remove('done');
	    }}, 1200);
	  }}
	  document.addEventListener('click', function(e) {{
	    var btn = e.target && e.target.closest ? e.target.closest('[data-author-copy]') : null;
	    if (!btn) return;
	    e.preventDefault();
	    var kind = btn.getAttribute('data-author-copy');
	    var text = kind === 'title-line' ? authorDoc.titleLine : (kind === 'title' ? authorDoc.title : authorBodyText());
	    if (!text) return;
	    copyText(text, function() {{ flashCopied(btn); }});
	  }});
	  window.__setSettings = function(settings) {{
	    settings = settings || {{}};
	    authorMode = !!settings.authorMode;
	    applyAuthorMode();
	    document.body.classList.toggle('sidebar-open', !!settings.sidebarOpen);
	    document.body.classList.toggle('no-wrap', settings.wordWrap === false);
	    var current = {{
	      'open-mode': settings.openMode,
	      'tab-mode': settings.tabMode,
	      'word-wrap': settings.wordWrap === false ? 'off' : 'on',
	      'author-mode': settings.authorMode ? 'on' : 'off'
	    }};
	    var buttons = settingsControl.querySelectorAll('[data-setting]');
	    for (var i = 0; i < buttons.length; i++) {{
	      var btn = buttons[i];
	      var selected = current[btn.getAttribute('data-setting')] === btn.getAttribute('data-value');
	      btn.setAttribute('aria-pressed', selected ? 'true' : 'false');
	    }}
	  }};
	  btnZoomOut.addEventListener('click', function() {{ changeZoom(-ZOOM_STEP); }});
	  btnZoomReset.addEventListener('click', function() {{ applyZoom(100, true); }});
	  btnZoomIn.addEventListener('click', function() {{ changeZoom(ZOOM_STEP); }});
  btnPrint.addEventListener('click', function() {{
    if (inEdit()) leaveEdit();
    // Route through Rust: WKWebView ignores window.print(); wry's
    // WebView::print() calls the right native API on each platform.
    setTimeout(function(){{ window.ipc.postMessage('print'); }}, 0);
  }});
	  ta.addEventListener('input', function() {{
	    setDirty(true);
	    scheduleAutosave();
	    if (document.body.classList.contains('split-view')) scheduleLiveRender();
	    autoResize();
	    updateDocumentStats(ta.value);
	  }});
  window.addEventListener('resize', function() {{ if (inEdit()) autoResize(); }});
  document.addEventListener('click', function(e) {{
    if (!zoomControl.contains(e.target)) zoomControl.classList.remove('open');
    if (!settingsControl.contains(e.target)) settingsControl.classList.remove('open');
    if (encodingPopover && !encodingPopover.contains(e.target) && (!btnEncoding || !btnEncoding.contains(e.target))) {{
      encodingPopover.style.display = 'none';
    }}
  }});

  document.addEventListener('keydown', function(e) {{
	if ((e.metaKey || e.ctrlKey) && (e.key === 'w' || e.key === 'W')) {{
	  if (activeTabId) {{
	    e.preventDefault();
	    requestTabAction('close', activeTabId);
	  }}
	  return;
	}}
	if ((e.metaKey || e.ctrlKey) && (e.key === 'n' || e.key === 'N')) {{
	  e.preventDefault();
	  newFile();
	  return;
	}}
    if ((e.metaKey || e.ctrlKey) && (e.key === '+' || e.key === '=' || e.code === 'NumpadAdd')) {{
      e.preventDefault();
      changeZoom(ZOOM_STEP);
      return;
    }}
    if ((e.metaKey || e.ctrlKey) && (e.key === '-' || e.code === 'NumpadSubtract')) {{
      e.preventDefault();
      changeZoom(-ZOOM_STEP);
      return;
    }}
    if ((e.metaKey || e.ctrlKey) && (e.key === '0' || e.code === 'Numpad0')) {{
      e.preventDefault();
      applyZoom(100, true);
      return;
    }}
    if ((e.metaKey || e.ctrlKey) && (e.key === 'r' || e.key === 'R')) {{
      e.preventDefault();
      if (!inEdit()) window.ipc.postMessage('refresh');
      return;
    }}
	    if ((e.metaKey || e.ctrlKey) && (e.key === 'o' || e.key === 'O')) {{
	      e.preventDefault();
	      openFile();
	      return;
	    }}
	    if ((e.metaKey || e.ctrlKey) && (e.key === 'f' || e.key === 'F')) {{
	      if (inEdit()) return;
	      e.preventDefault();
	      showFind();
	      return;
	    }}
    if ((e.metaKey || e.ctrlKey) && (e.key === 'e' || e.key === 'E')) {{
      e.preventDefault();
      if (inEdit()) leaveEdit(); else enterEdit();
      return;
    }}
    if ((e.metaKey || e.ctrlKey) && (e.key === 's' || e.key === 'S')) {{
      if (inEdit()) {{ e.preventDefault(); save(); }}
      return;
    }}
    if ((e.metaKey || e.ctrlKey) && (e.key === 'p' || e.key === 'P')) {{
      e.preventDefault();
      if (inEdit()) leaveEdit();
      setTimeout(function(){{ window.ipc.postMessage('print'); }}, 0);
      return;
    }}
	    if ((e.metaKey || e.ctrlKey) && (e.key === '\\' || e.code === 'Backslash')) {{
	      e.preventDefault();
	      if (!inEdit()) enterEdit();
	      toggleSplitView();
	      return;
	    }}
	    if (e.key === 'Escape' && encodingPopover && encodingPopover.style.display !== 'none') {{ encodingPopover.style.display = 'none'; return; }}
	    if (e.key === 'Escape' && lightbox && lightbox.style.display !== 'none') {{ closeLightbox(); return; }}
	    if (e.key === 'Escape' && tabContextMenu && tabContextMenu.style.display !== 'none') {{ hideTabContextMenu(); return; }}
	    if (e.key === 'Escape' && document.body.classList.contains('finding')) {{ hideFind(); return; }}
	    if (e.key === 'Escape' && inEdit()) {{ leaveEdit(); }}
  }});

  // Called by Rust after a save (only preview is refreshed) or after an
  // external file change (both preview + textarea are refreshed).
  window.__setPreview = function(previewHtml, needsMath, needsMermaid) {{
    if (arguments.length > 1 && window.__setFeatureFlags) {{
      window.__setFeatureFlags(needsMath, needsMermaid);
    }}
    document.getElementById('preview').innerHTML = previewHtml;
    if (window.__applyAuthorMode) window.__applyAuthorMode();
    setupCodeBlockCopyButtons();
    if (sidebarSection === 'outline') renderSidebar();
    (window.requestIdleCallback || function(fn){{ return setTimeout(fn, 0); }})(function() {{
      if (typeof hljs !== 'undefined') hljs.highlightAll();
      if (window.__enhancePreview) window.__enhancePreview();
    }});
  }};
  window.__setBaseHref = function(baseHref) {{
    var base = document.getElementById('base-href');
    if (!base) {{
      base = document.createElement('base');
      base.id = 'base-href';
      document.head.insertBefore(base, document.head.firstChild);
    }}
    if (baseHref) base.setAttribute('href', baseHref);
    else base.removeAttribute('href');
  }};
	window.__markSaved = function(savedRaw) {{
	  if (typeof savedRaw === 'string' && ta.value !== savedRaw) {{
	    window.ipc.postMessage('dirty:1');
	    return;
	  }}
	  autosavePaused = false;
	  setDirty(false);
	}};
	window.__mdPreviewerPauseAutosave = function() {{
	  cancelPendingAutosave();
	  autosavePaused = true;
	}};
	window.__mdPreviewerResolveExternalChange = function() {{
	  cancelPendingAutosave();
	  window.ipc.postMessage('external-change:' + (dirty ? 'dirty' : 'clean'));
	}};
	window.__setTabs = function(tabs) {{
	  tabs = Array.isArray(tabs) ? tabs : [];
	  tabsEl.textContent = '';
	  activeTabId = 0;
	  document.body.classList.toggle('has-tabs', tabs.length > 0);
	  tabs.forEach(function(tab) {{
	    var item = document.createElement('div');
	    item.className = 'tab' + (tab.active ? ' active' : '') + (tab.missing ? ' missing' : '') + (tab.dirty ? ' dirty' : '');
	    item.setAttribute('data-tab-id', tab.id);
	    item.setAttribute('role', 'button');
	    item.setAttribute('tabindex', '0');
	    item.title = tab.path;
	    if (tab.active) activeTabId = tab.id;
	    var status = document.createElement('span');
	    status.className = 'tab-status';
	    status.textContent = tab.missing ? '!' : '';
	    var name = document.createElement('span');
	    name.className = 'tab-name';
	    name.textContent = tab.name;
	    var close = document.createElement('button');
	    close.className = 'tab-close';
	    close.type = 'button';
	    close.setAttribute('data-close-tab', tab.id);
	    close.setAttribute('aria-label', 'Close ' + tab.name);
	    close.textContent = '×';
	    item.appendChild(status);
	    item.appendChild(name);
	    item.appendChild(close);
	    tabsEl.appendChild(item);
	    if (tab.active) requestAnimationFrame(function() {{ item.scrollIntoView({{ block: 'nearest', inline: 'nearest' }}); }});
	  }});
	}};
	tabsEl.addEventListener('keydown', function(e) {{
	  if (e.key !== 'Enter' && e.key !== ' ') return;
	  var tab = e.target && e.target.closest ? e.target.closest('[data-tab-id]') : null;
	  if (!tab) return;
	  e.preventDefault();
	  requestTabAction('activate', tab.getAttribute('data-tab-id'));
	}});
	  window.__setContent = function(previewHtml, rawMd, baseHref, needsMath, needsMermaid) {{
	    document.body.classList.remove('empty');
	    document.body.classList.remove('missing');
	    hideFind();
	    window.__setBaseHref(baseHref);
	    window.__setPreview(previewHtml, needsMath, needsMermaid);
    if (!inEdit() || !dirty) {{
	      autosavePaused = false;
	      ta.value = rawMd;
	      updateDocumentStats(rawMd);
      setDirty(false);
      if (inEdit()) autoResize();
    }}
  }};
	  window.__setEmptyPreview = function(previewHtml) {{
	    document.body.classList.add('empty');
	    document.body.classList.remove('missing');
	    document.body.classList.remove('editing');
	    btnToggle.innerHTML = ICON_EDIT;
	    btnToggle.title = L_EDIT;
	    btnToggle.setAttribute('aria-label', L_EDIT);
	    cancelPendingAutosave();
	    autosavePaused = false;
	    hideFind();
	    window.__setBaseHref('');
	    document.getElementById('preview').innerHTML = previewHtml;
	    ta.value = '';
	    updateDocumentStats('');
	    setDirty(false);
	    window.scrollTo(0, 0);
	  }};
	  window.__setMissing = function(previewHtml) {{
	    document.body.classList.remove('empty');
	    document.body.classList.add('missing');
	    document.body.classList.remove('editing');
	    btnToggle.innerHTML = ICON_EDIT;
	    btnToggle.title = L_EDIT;
	    btnToggle.setAttribute('aria-label', L_EDIT);
	    cancelPendingAutosave();
	    autosavePaused = false;
	    hideFind();
	    window.__setBaseHref('');
	    document.getElementById('preview').innerHTML = previewHtml;
	    ta.value = '';
	    updateDocumentStats('');
	    setDirty(false);
	    window.scrollTo(0, 0);
	  }};

  // Defer hljs parse + initial highlight to idle time.
  // hljs itself is NOT inlined in this page — Rust injects it via
  // evaluate_script once we tell it we're painted. Until that injection
  // runs, typeof hljs === 'undefined' and highlightAll is skipped; once
  // it lands, hljs.highlightAll() gets called by the injected bootstrap
  // and __setPreview.

  setupCodeBlockCopyButtons();

  // Signal Rust after first paint (triggers hljs inject; bench mode exits).
  requestAnimationFrame(function() {{
    requestAnimationFrame(function() {{
      if (window.ipc) window.ipc.postMessage('ready');
    }});
  }});
}})();
window.__mdPreviewerFeatureFlags = {{ math: {needs_math}, mermaid: {needs_mermaid} }};
{preview_enhance_js}
if(window.__enhancePreview)window.__enhancePreview();
</script>
</body></html>"#,
        css_light = HLJS_LIGHT,
        css_dark = HLJS_DARK,
        base_tag = base_tag,
        preview_html = preview_html,
        raw_md_escaped = html_escape_ta(raw_md),
        btn_open = s.btn_open,
        btn_new = s.btn_new,
        btn_search = s.btn_search,
        btn_edit = s.btn_edit,
        btn_preview = s.btn_preview,
        btn_print = s.btn_print,
        btn_zoom = s.btn_zoom,
        btn_zoom_out = s.btn_zoom_out,
        btn_zoom_reset = s.btn_zoom_reset,
        btn_zoom_in = s.btn_zoom_in,
        search_placeholder = s.search_placeholder,
        btn_sidebar = s.btn_sidebar,
        sidebar_width = SIDEBAR_WIDTH,
        sidebar_toggle_left = SIDEBAR_WIDTH + 12.0,
        sidebar_folder = s.sidebar_folder,
        sidebar_recent = s.sidebar_recent,
        sidebar_empty_js = escape_js(s.sidebar_empty),
        sidebar_outline = s.sidebar_outline,
        sidebar_outline_empty_js = escape_js(s.sidebar_outline_empty),
        btn_split = s.btn_split,
        set_word_wrap = s.set_word_wrap,
        set_wrap_on = s.set_wrap_on,
        set_wrap_off = s.set_wrap_off,
        code_copy_js = escape_js(s.code_copy),
        code_copied_js = escape_js(s.code_copied),
        tab_menu_close = s.tab_menu_close,
        tab_menu_close_others = s.tab_menu_close_others,
        tab_menu_copy_path = s.tab_menu_copy_path,
        tab_menu_reveal = s.tab_menu_reveal,
        set_author_mode = s.set_author_mode,
        set_author_off = s.set_author_off,
        set_author_on = s.set_author_on,
        author_help = s.author_help,
        copy_title_line_js = escape_js(s.copy_title_line),
        copy_title_js = escape_js(s.copy_title),
        copy_body_js = escape_js(s.copy_body),
        copied_js = escape_js(s.copied),
        btn_settings = s.btn_settings,
        set_open_mode = s.set_open_mode,
        set_open_tab = s.set_open_tab,
        set_open_window = s.set_open_window,
        set_tab_mode = s.set_tab_mode,
        set_tab_keep = s.set_tab_keep,
        set_tab_single = s.set_tab_single,
        stat_words_js = escape_js(s.stat_words),
        stat_chars_js = escape_js(s.stat_chars),
        encoding_title = s.encoding_title,
        initial_encoding = initial_encoding,
        opt_utf8 = if initial_encoding == "UTF-8" { " active" } else { "" },
        opt_gbk = if initial_encoding == "GBK" { " active" } else { "" },
        opt_u16le = if initial_encoding == "UTF-16 LE" { " active" } else { "" },
        opt_u16be = if initial_encoding == "UTF-16 BE" { " active" } else { "" },
        body_class = body_class,
        needs_math = flags.math,
        needs_mermaid = flags.mermaid,
        preview_enhance_js = PREVIEW_ENHANCE_JS,
    )
}

fn build_page(
    preview_html: &str,
    raw_md: &str,
    base_href: Option<&str>,
    flags: EnhanceFlags,
    s: &Strings,
    empty: bool,
) -> String {
    build_page_with_encoding(preview_html, raw_md, base_href, flags, s, empty, "UTF-8")
}

fn escape_js(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn watch_scope_for_file(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(path)
}

fn event_path_matches_file(event_path: &Path, target: &Path) -> bool {
    event_path == target
        || (target.file_name().is_some()
            && event_path.file_name() == target.file_name()
            && event_path.parent() == target.parent())
}

fn event_should_reload_file(ev: &Event, target: &Path) -> bool {
    if ev.need_rescan() {
        return true;
    }

    if !(ev.kind.is_modify() || ev.kind.is_create() || ev.kind.is_remove()) {
        return false;
    }

    ev.paths
        .iter()
        .any(|event_path| event_path_matches_file(event_path, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::EventKind;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "md-previewer-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn chapter_numbering_is_stripped_without_hurting_ordinary_titles() {
        assert_eq!(strip_chapter_prefix("第二章 一只行李箱"), "一只行李箱");
        assert_eq!(strip_chapter_prefix("第32章 什么什么"), "什么什么");
        assert_eq!(strip_chapter_prefix("第一百二十三章：归途"), "归途");
        assert_eq!(strip_chapter_prefix("32、什么什么"), "什么什么");
        // 整行只有序号时保持原样，否则复制出来是空串
        assert_eq!(strip_chapter_prefix("第二章"), "第二章");
        // 这两个开头像序号但不是，不能误伤
        assert_eq!(strip_chapter_prefix("2023年的夏天"), "2023年的夏天");
        assert_eq!(strip_chapter_prefix("第一人称叙事"), "第一人称叙事");
    }

    #[test]
    fn author_doc_reads_the_chapter_heading_as_plain_text() {
        let raw = "## 第二章 一只行李箱\r\n\r\n雨没有停的意思。\r\n";

        let doc = author_doc(raw);

        assert_eq!(doc.title_line, "第二章 一只行李箱");
        assert_eq!(doc.title, "一只行李箱");
    }

    #[test]
    fn author_doc_strips_inline_markdown_from_the_heading() {
        // 标题里写了行内语法时，复制出来的要和屏幕上一致，不带星号和反引号
        let doc = author_doc("## 第二章 **一只**`行李`箱\n\n正文");

        assert_eq!(doc.title_line, "第二章 一只行李箱");
        assert_eq!(doc.title, "一只行李箱");
    }

    #[test]
    fn author_doc_without_a_heading_reports_no_title() {
        let doc = author_doc("\n只有正文，没有标题。\n");

        assert!(doc.title_line.is_empty());
        assert!(doc.title.is_empty());
    }

    #[test]
    fn is_txt_document_matches_txt_extension_case_insensitively() {
        assert!(is_txt_document(Path::new("note.txt")));
        assert!(is_txt_document(Path::new("NOTE.TXT")));
        assert!(is_txt_document(Path::new("/path/to/archive.Txt")));
        assert!(!is_txt_document(Path::new("readme.md")));
        assert!(!is_txt_document(Path::new("spec.markdown")));
        assert!(!is_txt_document(Path::new("no_extension")));
    }

    #[test]
    fn txt_document_preserves_newlines_and_escapes_html() {
        let raw = "# Title\n\nLine 1 <tag> & more\nLine 2 *not bold*\n| table | col |\n";
        let path = Path::new("test.txt");
        let (html, flags, base_href) = document_to_html(path, raw);

        assert!(html.starts_with(r#"<div class="mdp-plain-text">"#));
        assert!(html.ends_with("</div>"));
        assert!(html.contains("&lt;tag&gt; &amp; more"));
        assert!(html.contains("# Title\n\nLine 1"));
        assert!(html.contains("Line 2 *not bold*\n| table | col |"));
        assert!(!html.contains("<h1>"));
        assert!(!html.contains("<em>"));
        assert!(!html.contains("<table>"));
        assert!(!flags.math);
        assert!(!flags.mermaid);
        assert!(base_href.is_none());
    }

    #[test]
    fn document_to_html_disables_enhancers_for_txt() {
        let raw = "Math: $$E=mc^2$$\nMermaid:\n```mermaid\ngraph TD\nA-->B\n```\n";
        let (html, flags, _) = document_to_html(Path::new("notes.txt"), raw);

        assert!(!flags.math);
        assert!(!flags.mermaid);
        assert!(!html.contains("<svg"));
        assert!(html.contains("$$E=mc^2$$"));
        assert!(html.contains("```mermaid"));
    }

    #[test]
    fn author_doc_skips_hashes_that_are_not_headings() {
        let doc = author_doc("#不是标题，井号后面没空格\n\n## 第一章 开始\n\n正文");

        assert_eq!(doc.title_line, "第一章 开始");
        assert_eq!(doc.title, "开始");
    }

    #[test]
    fn folder_documents_lists_sorted_siblings_and_skips_other_entries() {
        let dir = temp_test_dir("folder-list");
        for name in [
            "beta.md",
            "Alpha.markdown",
            "gamma.txt",
            "notes.pdf",
            "readme",
        ] {
            fs::write(dir.join(name), "x").unwrap();
        }
        fs::create_dir_all(dir.join("nested.md")).unwrap();

        let files = folder_documents(Some(&dir.join("beta.md")));

        let names = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["Alpha.markdown", "beta.md", "gamma.txt"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn folder_documents_is_empty_without_an_active_document() {
        assert!(folder_documents(None).is_empty());
    }

    #[test]
    fn sidebar_json_marks_the_active_document_and_carries_recent_directories() {
        let dir = temp_test_dir("sidebar-json");
        let active = dir.join("active.md");
        let other = dir.join("other.md");
        fs::write(&active, "# active").unwrap();
        fs::write(&other, "# other").unwrap();
        let mut session = DocumentSession::default();
        session.open(active.clone(), false);

        let json: serde_json::Value =
            serde_json::from_str(&sidebar_json(&session, &[other.clone()])).unwrap();

        let folder = json["folder"].as_array().unwrap();
        assert_eq!(folder.len(), 2);
        let active_entries = folder
            .iter()
            .filter(|entry| entry["active"] == serde_json::json!(true))
            .collect::<Vec<_>>();
        assert_eq!(active_entries.len(), 1);
        assert_eq!(active_entries[0]["name"], serde_json::json!("active.md"));
        let recent = json["recent"].as_array().unwrap();
        assert_eq!(recent[0]["name"], serde_json::json!("other.md"));
        assert_eq!(recent[0]["active"], serde_json::json!(false));
        // 目录列只保留目录名，不是整条路径
        let shown_dir = recent[0]["dir"].as_str().unwrap();
        assert_eq!(shown_dir, dir.file_name().unwrap().to_string_lossy());
        assert!(!shown_dir.contains(std::path::MAIN_SEPARATOR));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dollar_math_is_protected_from_markdown_escapes() {
        let html = md_to_html(r"$\{x\}$");

        assert!(html.contains(r#"<span class="math math-inline">\{x\}</span>"#));
    }

    #[test]
    fn dollar_math_is_protected_from_markdown_emphasis() {
        let html = md_to_html(r"$\bar{\mu}_{n}$ and $x_{n}$");

        assert!(html.contains(r#"<span class="math math-inline">\bar{\mu}_{n}</span>"#));
        assert!(html.contains(r#"<span class="math math-inline">x_{n}</span>"#));
        assert!(!html.contains("<em>"));
    }

    #[test]
    fn github_alerts_render_as_markdown_alert_blockquotes() {
        let html = md_to_html("> [!IMPORTANT]\n> This is an alert");

        assert!(html.contains(r#"<blockquote class="markdown-alert-important">"#));
        assert!(html.contains("<p>This is an alert</p>"));
        assert!(!html.contains("[!IMPORTANT]"));
    }

    #[test]
    fn double_equals_highlight_renders_mark_without_touching_code() {
        let html = md_to_html("Use ==highlight & tag== here and `==literal==` there.");

        assert!(html.contains(r#"Use <mark class="mdp-mark">highlight &amp; tag</mark> here"#));
        assert!(html.contains("<code>==literal==</code>"));
    }

    #[test]
    fn local_relative_images_are_embedded_from_markdown_directory() {
        let dir = temp_test_dir("local-image");
        let assets = dir.join("assets");
        fs::create_dir_all(&assets).unwrap();
        fs::write(assets.join("pixel.png"), b"abc").unwrap();

        let html = md_to_html_with_base("![pixel](assets/pixel.png)", Some(&dir));

        assert!(html.contains(r#"<img src="data:image/png;base64,YWJj" alt="pixel" />"#));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn local_relative_images_keep_original_src_when_unreadable() {
        let dir = temp_test_dir("missing-image");

        let html = md_to_html_with_base("![missing](assets/missing.png)", Some(&dir));

        assert!(html.contains(r#"<img src="assets/missing.png" alt="missing" />"#));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn local_relative_images_do_not_embed_parent_traversal() {
        let dir = temp_test_dir("traversal-image");

        let html = md_to_html_with_base("![secret](../secret.png)", Some(&dir));

        assert!(html.contains(r#"<img src="../secret.png" alt="secret" />"#));
        assert!(!html.contains("data:image/png"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn yaml_front_matter_keeps_metadata_readable_without_blank_lines() {
        let html = md_to_html(
            "---\nname: skill-creator-lite\ndescription: Skill 创建封装工具。\n---\n# Body",
        );

        assert!(html.contains(r#"<aside class="front-matter">"#));
        assert!(html.contains("name: skill-creator-lite"));
        assert!(html.contains("description: Skill 创建封装工具。"));
        assert!(html.contains("<h1 id=\"body\">Body</h1>"));
        assert!(!html.contains("<h2"));
    }

    #[test]
    fn yaml_front_matter_accepts_dots_and_escapes_html() {
        let html = md_to_html("---\ntitle: <script>alert(1)</script>\n...\nParagraph");

        assert!(html.contains("title: &lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("<p>Paragraph</p>"));
    }

    #[test]
    fn horizontal_rules_and_setext_headings_are_not_front_matter() {
        let html = md_to_html("Title\n---\n\nParagraph\n\n---");

        assert!(!html.contains(r#"<aside class="front-matter">"#));
        assert!(html.contains(r#"<h2 id="title">Title</h2>"#));
        assert!(html.contains("<hr />"));
    }

    #[test]
    fn file_urls_only_open_existing_supported_documents() {
        let dir = temp_test_dir("document-links");
        let linked = dir.join("含 空格.md");
        let unsupported = dir.join("page.html");
        fs::write(&linked, "# Linked").unwrap();
        fs::write(&unsupported, "<h1>Page</h1>").unwrap();

        let linked_url = url::Url::from_file_path(&linked).unwrap().to_string();
        let unsupported_url = url::Url::from_file_path(&unsupported).unwrap().to_string();
        let missing_url = url::Url::from_file_path(dir.join("missing.md"))
            .unwrap()
            .to_string();

        assert_eq!(
            local_document_path_from_url(&format!("{linked_url}#section")),
            Some(fs::canonicalize(&linked).unwrap())
        );
        assert_eq!(local_document_path_from_url(&unsupported_url), None);
        assert_eq!(local_document_path_from_url(&missing_url), None);
        assert_eq!(
            local_document_path_from_url("https://example.com/readme.md"),
            None
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn linux_nvidia_compat_env_only_sets_dmabuf_when_unconfigured() {
        assert_eq!(
            linux_webkit_compat_env(None, None, true),
            Some(("WEBKIT_DISABLE_DMABUF_RENDERER", "1"))
        );
        assert_eq!(linux_webkit_compat_env(Some("0"), None, true), None);
        assert_eq!(linux_webkit_compat_env(None, Some("1"), true), None);
        assert_eq!(linux_webkit_compat_env(None, None, false), None);
    }

    #[test]
    fn generated_heading_ids_support_cjk_anchor_links() {
        let html = md_to_html("1. [需求概述](#需求概述)\n\n## 需求概述");

        assert!(html.contains(r##"<a href="#%E9%9C%80%E6%B1%82%E6%A6%82%E8%BF%B0">需求概述</a>"##));
        assert!(html.contains(r#"<h2 id="需求概述">需求概述</h2>"#));
    }

    #[test]
    fn generated_heading_ids_are_unique_and_keep_explicit_ids() {
        let html = md_to_html("## Intro\n## Intro\n## Custom {#fixed}\n## Fixed");

        assert!(html.contains(r#"<h2 id="intro">Intro</h2>"#));
        assert!(html.contains(r#"<h2 id="intro-1">Intro</h2>"#));
        assert!(html.contains(r#"<h2 id="fixed">Custom</h2>"#));
        assert!(html.contains(r#"<h2 id="fixed-1">Fixed</h2>"#));
    }

    #[test]
    fn help_flags_are_recognized() {
        assert!(is_help_arg("-h"));
        assert!(is_help_arg("--help"));
        assert!(!is_help_arg("--edit"));
    }

    #[test]
    fn theme_choice_parses_menu_values() {
        assert_eq!(ThemeChoice::from_str("system"), ThemeChoice::System);
        assert_eq!(ThemeChoice::from_str("light"), ThemeChoice::Light);
        assert_eq!(ThemeChoice::from_str("dark"), ThemeChoice::Dark);
        assert_eq!(ThemeChoice::from_str("unexpected"), ThemeChoice::System);
        assert_eq!(ThemeChoice::Dark.as_str(), "dark");
    }

    #[test]
    fn page_blocks_native_preview_reload_paths() {
        let strings = Strings::for_lang(Lang::En);
        let page = build_page(
            &md_to_html("# Hello"),
            "# Hello",
            None,
            EnhanceFlags::default(),
            &strings,
            false,
        );

        assert!(page.contains("document.addEventListener('contextmenu'"));
        assert!(page.contains("window.ipc.postMessage('refresh')"));
        assert!(page.contains("id=\"btn-open\""));
        assert!(page.contains("id=\"btn-search\""));
        assert!(page.contains("window.ipc.postMessage('open')"));
        assert!(page.contains("window.__mdPreviewerOpenFile = openFile"));
        assert!(page.contains("window.__mdPreviewerShowFind = showFind"));
        assert!(page.contains("window.__mdPreviewerToggleEdit"));
        assert!(!page.contains("id=\"btn-update\""));
        assert!(!page.contains("api.github.com"));
        assert!(!page.contains("check-updates"));
        assert!(page.contains("Cmd/Ctrl+F"));
        assert!(page
            .contains("if (inEdit()) return;\n\t      e.preventDefault();\n\t      showFind();"));
        assert!(page.contains("body.editing #btn-open"));
        assert!(page.contains("id=\"topbar\""));
        assert!(page.contains("top: var(--bar-top); z-index: 110;"));
        assert!(page.contains(
            "body.editing .toolbar { position: static; opacity: 1; pointer-events: auto; }"
        ));
        assert!(page.contains("body.editing .findbar { display: none !important; }"));
        assert!(page.contains("ta.focus({ preventScroll: true })"));
        assert!(page.contains("window.__setEmptyPreview"));
        assert!(page.contains("id=\"tabbar\""));
        assert!(page.contains("window.__setTabs"));
        assert!(page.contains("tab-action:'));") || page.contains("'tab-action:' + action"));
        assert!(page.contains("window.__markSaved"));
        assert!(
            !page.contains("window.ipc.postMessage('save:' + ta.value);\n\t    setDirty(false);")
        );
        assert!(page.contains("compositionstart"));
        assert!(page.contains("e.isComposing"));
        assert!(page.contains("focusFindInput"));
        assert!(page.contains("FIND_DEBOUNCE_MS = 300"));
        assert!(page.contains("document.createTreeWalker(previewEl, NodeFilter.SHOW_TEXT"));
        assert!(page.contains("mark.className = 'search-hit'"));
        assert!(page.contains("selectFindHit(currentFindHit + (backward ? -1 : 1))"));
        assert!(page.contains("#preview mark.search-hit.current"));
        assert!(page.contains("restoreFindInput(selectionStart, selectionEnd)"));
        assert!(page.contains("findInput.setSelectionRange(selectionStart, selectionEnd)"));
        assert!(page.contains("body.empty .toolbar"));
        assert!(page.contains("bindAnchorNavigation"));
        assert!(page.contains("event.target.closest('#preview a[href]')"));
        assert!(page.contains("window.ipc.postMessage('open-local-link:' + resolved)"));
        assert!(page.contains(".markdown-alert-important"));
        assert!(page.contains(".markdown-alert-title"));
        let light_alert = page.find("background: #dafbe1").unwrap();
        let dark_alert = page.rfind("background: #161b22; color: #d4d4d4").unwrap();
        assert!(dark_alert > light_alert);
        assert!(page.contains(".mdp-mark"));
        assert!(!page.contains("Local-first Markdown preview for AI-generated docs"));
    }

    #[test]
    fn page_separates_new_file_from_open_and_debounces_autosave() {
        let strings = Strings::for_lang(Lang::En);
        let page = build_page(
            &md_to_html("# Hello"),
            "# Hello",
            None,
            EnhanceFlags::default(),
            &strings,
            false,
        );

        assert!(page.contains("window.ipc.postMessage('new-file')"));
        assert!(page.contains("tabOpen.addEventListener('click', newFile)"));
        assert!(!page.contains("tabOpen.addEventListener('click', openFile)"));
        assert!(page.contains("AUTOSAVE_DEBOUNCE_MS = 700"));
        assert!(page.contains("pendingAutosaveTimer = setTimeout"));
        assert!(page.contains("window.__markSaved = function(savedRaw)"));
        assert!(page.contains("window.__mdPreviewerResolveExternalChange"));
        assert!(page.contains("'external-change:' + (dirty ? 'dirty' : 'clean')"));
        assert!(page.contains("(e.key === 'n' || e.key === 'N')"));
        assert!(page.contains("id=\"doc-stats\""));
        assert!(page.contains("updateDocumentStats"));
        assert!(page.contains("restoreScrollProgress"));
        assert!(page.contains("md-previewer-content-zoom-v1"));
        assert!(page.contains("data-setting=\"author-mode\" data-value=\"on\""));
        assert!(page.contains("class=\"settings-help\""));
        assert!(page.contains("window.__setAuthorDoc"));
        assert!(page.contains("data-author-copy"));
        assert!(page.contains("id=\"btn-sidebar\""));
        assert!(page.contains("id=\"sidebar-list\""));
        assert!(page.contains("data-sidebar-section=\"folder\""));
        assert!(page.contains("data-sidebar-section=\"recent\""));
        assert!(page.contains("window.__setSidebar"));
        assert!(page.contains("id=\"btn-settings\""));
        assert!(page.contains("data-setting=\"open-mode\" data-value=\"new-window\""));
        assert!(page.contains("data-setting=\"tab-mode\" data-value=\"single\""));
        assert!(page.contains("window.__setSettings"));
        assert!(page.contains("id=\"btn-zoom-in\""));
        assert!(page.contains("id=\"btn-zoom-out\""));
        assert!(page.contains("id=\"btn-zoom-reset\""));
    }

    #[test]
    fn closing_the_last_editing_tab_restores_the_empty_preview() {
        let strings = Strings::for_lang(Lang::En);
        let page = build_page(
            &md_to_html("# Hello"),
            "# Hello",
            None,
            EnhanceFlags::default(),
            &strings,
            false,
        );
        let empty_start = page.find("window.__setEmptyPreview = function").unwrap();
        let missing_start = page.find("window.__setMissing = function").unwrap();
        let empty_handler = &page[empty_start..missing_start];

        assert!(empty_handler.contains("document.body.classList.remove('editing')"));
        assert!(empty_handler.contains("btnToggle.innerHTML = ICON_EDIT"));
    }

    #[test]
    fn new_markdown_path_keeps_markdown_extensions_and_replaces_other_extensions() {
        assert_eq!(
            normalize_new_markdown_path(PathBuf::from("/tmp/plan")),
            PathBuf::from("/tmp/plan.md")
        );
        assert_eq!(
            normalize_new_markdown_path(PathBuf::from("/tmp/plan.markdown")),
            PathBuf::from("/tmp/plan.markdown")
        );
        assert_eq!(
            normalize_new_markdown_path(PathBuf::from("/tmp/plan.txt")),
            PathBuf::from("/tmp/plan.md")
        );
    }

    #[test]
    fn page_expands_multi_column_tables() {
        let strings = Strings::for_lang(Lang::En);
        let page = build_page(
            &md_to_html("| A | B | C | D |\n|---|---|---|---|\n| 1 | 2 | 3 | 4 |"),
            "",
            None,
            EnhanceFlags::default(),
            &strings,
            false,
        );

        assert!(page.contains("mdp-table-wrap"));
        assert!(page.contains("width: min(calc(100vw - 64px), 1280px)"));
        assert!(page.contains("if(window.__enhancePreview)window.__enhancePreview();"));
    }

    #[test]
    fn empty_state_exposes_open_and_recent_files() {
        let strings = Strings::for_lang(Lang::En);
        let recent = vec![PathBuf::from("/tmp/example.md")];
        let html = empty_preview_html(&strings, &recent);

        assert!(html.contains("Open File"));
        assert!(html.contains("Recent"));
        assert!(html.contains("example.md"));
        assert!(html.contains("data-recent-index=\"0\""));
        assert!(html.contains(r#"<div class="empty has-recent">"#));
        assert!(html.contains(r#"<div class="icon">#</div>"#));
        assert!(!html.contains("empty-mark"));

        let page = build_page(&html, "", None, EnhanceFlags::default(), &strings, true);
        assert!(page.contains(".empty.has-recent"));
        assert!(!page.contains(".empty.has-recent .recent { max-height"));
    }

    #[test]
    fn vim_style_target_rewrite_events_reload_current_file() {
        let target = PathBuf::from("/tmp/note.md");
        let ev = Event::new(EventKind::Create(notify::event::CreateKind::File))
            .add_path(PathBuf::from("/tmp/note.md"));

        assert!(event_should_reload_file(&ev, &target));
    }

    #[test]
    fn sibling_file_events_do_not_reload_current_file() {
        let target = PathBuf::from("/tmp/note.md");
        let ev = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Any,
        )))
        .add_path(PathBuf::from("/tmp/other.md"));

        assert!(!event_should_reload_file(&ev, &target));
    }

    #[test]
    fn self_write_suppression_checks_disk_content_not_only_time() {
        let dir = temp_test_dir("self-write-suppression");
        let path = dir.join("note.md");
        fs::write(&path, "saved by app").unwrap();
        let record = SelfWriteRecord {
            at: Instant::now(),
            path: path.clone(),
            content: "saved by app".to_string(),
        };

        assert!(self_write_still_matches_disk(Some(&record), &path));

        fs::write(&path, "external edit").unwrap();
        assert!(!self_write_still_matches_disk(Some(&record), &path));
    }

    #[test]
    fn external_change_protection_uses_dirty_state_from_either_side() {
        assert!(!should_protect_external_change(false, false));
        assert!(should_protect_external_change(true, false));
        assert!(should_protect_external_change(false, true));
        assert!(should_protect_external_change(true, true));
    }

    #[test]
    fn file_watch_scope_is_parent_directory() {
        let target = PathBuf::from("/tmp/note.md");

        assert_eq!(watch_scope_for_file(&target), Path::new("/tmp"));
    }

    #[test]
    fn finder_action_parses_encoded_folder_and_kind() {
        assert_eq!(
            parse_finder_action(
                "mdpreviewer://finder?action=create&path=%2Ftmp%2FMy%20Notes&kind=md"
            ),
            Some(FinderAction::Create {
                folder: PathBuf::from("/tmp/My Notes"),
                kind: "md".to_string(),
            })
        );
        assert!(parse_finder_action("https://example.com/").is_none());
    }

    #[test]
    fn finder_create_uses_non_conflicting_markdown_name() {
        let dir = temp_test_dir("finder-create");
        fs::write(dir.join("新建.md"), "existing").unwrap();

        let created = create_finder_file(&dir, "md").unwrap();

        assert_eq!(created.file_name().unwrap(), "新建 2.md");
        assert_eq!(fs::read_to_string(created).unwrap(), "");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn read_document_to_string_handles_utf8_bom_and_utf16() {
        let dir = temp_test_dir("encoding-detection");

        // 1. 标准 UTF-8
        let p_utf8 = dir.join("utf8.txt");
        fs::write(&p_utf8, "hello 编码".as_bytes()).unwrap();
        assert_eq!(read_document_to_string(&p_utf8).unwrap(), "hello 编码");

        // 2. 带 BOM 的 UTF-8
        let p_utf8_bom = dir.join("utf8_bom.txt");
        let mut bytes_bom = vec![0xEF, 0xBB, 0xBF];
        bytes_bom.extend_from_slice("with bom 文本".as_bytes());
        fs::write(&p_utf8_bom, &bytes_bom).unwrap();
        assert_eq!(
            read_document_to_string(&p_utf8_bom).unwrap(),
            "with bom 文本"
        );

        // 3. 带 BOM 的 UTF-16 LE
        let p_utf16_le = dir.join("utf16_le.txt");
        let u16s: Vec<u16> = "utf16 文本".encode_utf16().collect();
        let mut bytes_le = vec![0xFF, 0xFE];
        for u in u16s {
            bytes_le.extend_from_slice(&u.to_le_bytes());
        }
        fs::write(&p_utf16_le, &bytes_le).unwrap();
        assert_eq!(read_document_to_string(&p_utf16_le).unwrap(), "utf16 文本");

        // 4. 带 BOM 的 UTF-16 BE
        let p_utf16_be = dir.join("utf16_be.txt");
        let u16s: Vec<u16> = "utf16 be 文本".encode_utf16().collect();
        let mut bytes_be = vec![0xFE, 0xFF];
        for u in u16s {
            bytes_be.extend_from_slice(&u.to_be_bytes());
        }
        fs::write(&p_utf16_be, &bytes_be).unwrap();
        assert_eq!(
            read_document_to_string(&p_utf16_be).unwrap(),
            "utf16 be 文本"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn build_page_includes_all_ux_enhancement_components() {
        let strings = Strings::for_lang(Lang::En);
        let page = build_page(
            "# Heading",
            "# Heading",
            None,
            EnhanceFlags::default(),
            &strings,
            false,
        );
        assert!(page.contains("code-copy-btn"));
        assert!(page.contains("data-sidebar-section=\"outline\""));
        assert!(page.contains("id=\"btn-split\""));
        assert!(page.contains("id=\"tab-context-menu\""));
        assert!(page.contains("data-tab-action=\"close-others\""));
        assert!(page.contains("data-tab-action=\"copy-path\""));
        assert!(page.contains("data-tab-action=\"reveal\""));
        assert!(page.contains("id=\"lightbox\""));
        assert!(page.contains("data-setting=\"word-wrap\""));
        assert!(page.contains("id=\"encoding-control\""));
        assert!(page.contains("id=\"btn-encoding\""));
        assert!(page.contains("id=\"encoding-popover\""));
        assert!(page.contains("order: 1;"));
        assert!(page.contains("order: 2;"));
        assert!(page.contains("border-right: 2px solid"));
    }

    #[test]
    fn read_and_write_document_with_encoding_roundtrips() {
        let dir = temp_test_dir("encoding-roundtrip");
        let p = dir.join("test.txt");

        // UTF-8
        write_document_with_encoding(&p, "测试 UTF-8 编码", Some("UTF-8")).unwrap();
        let (content, enc) = read_document_with_encoding(&p, None).unwrap();
        assert_eq!(content, "测试 UTF-8 编码");
        assert_eq!(enc, "UTF-8");

        // UTF-16 LE
        write_document_with_encoding(&p, "测试 UTF-16 LE", Some("UTF-16 LE")).unwrap();
        let (content, enc) = read_document_with_encoding(&p, None).unwrap();
        assert_eq!(content, "测试 UTF-16 LE");
        assert_eq!(enc, "UTF-16 LE");

        // UTF-16 BE
        write_document_with_encoding(&p, "测试 UTF-16 BE", Some("UTF-16 BE")).unwrap();
        let (content, enc) = read_document_with_encoding(&p, None).unwrap();
        assert_eq!(content, "测试 UTF-16 BE");
        assert_eq!(enc, "UTF-16 BE");

        #[cfg(target_os = "windows")]
        {
            // GBK
            write_document_with_encoding(&p, "测试 GBK 编码", Some("GBK")).unwrap();
            let (content, enc) = read_document_with_encoding(&p, Some("GBK")).unwrap();
            assert_eq!(content, "测试 GBK 编码");
            assert_eq!(enc, "GBK");
        }

        let _ = fs::remove_dir_all(dir);
    }
}

/// Decode embedded icon.ico to an RGBA tao Icon for the window chrome.
fn load_window_icon() -> Option<tao::window::Icon> {
    let img = image::load_from_memory_with_format(ICON_BYTES, image::ImageFormat::Ico).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    tao::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
}

#[cfg(target_os = "macos")]
fn register_as_default(_lang: Lang) {
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
fn register_as_default(_lang: Lang) {
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
fn register_as_default(_lang: Lang) {}

#[cfg(target_os = "macos")]
const GITHUB_URL: &str = "https://github.com/ArnoldRedman/md-preview";

#[cfg(target_os = "macos")]
thread_local! {
    static MACOS_MENU_PROXY: std::cell::RefCell<Option<EventLoopProxy<UserEvent>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "macos")]
fn send_macos_menu_event(event: UserEvent) {
    MACOS_MENU_PROXY.with(|cell| {
        if let Some(proxy) = cell.borrow().as_ref() {
            let _ = proxy.send_event(event);
        }
    });
}

#[cfg(target_os = "macos")]
fn macos_menu_controller_class() -> &'static objc2::runtime::AnyClass {
    use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, NSObject, Sel};
    use objc2::{sel, ClassType, MainThreadOnly};
    use objc2_app_kit::{
        NSAlert, NSAlertStyle, NSButton, NSControlStateValueOff, NSControlStateValueOn, NSImage,
        NSMenuItem, NSView,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
    use std::sync::Once;

    extern "C" fn open_file(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::OpenFile);
    }

    extern "C" fn new_file(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::NewFile);
    }

    extern "C" fn close_tab(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::CloseActiveTab);
    }

    extern "C" fn show_find(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::ShowFind);
    }

    extern "C" fn toggle_edit(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::ToggleEdit);
    }

    extern "C" fn print(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::Print);
    }

    extern "C" fn quit(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::Quit);
    }

    extern "C" fn open_github(_: &AnyObject, _: Sel, _: &AnyObject) {
        send_macos_menu_event(UserEvent::OpenUrl(GITHUB_URL));
    }

    extern "C" fn set_theme(_: &AnyObject, _: Sel, sender: &NSMenuItem) {
        let choice = match sender.tag() {
            102 => ThemeChoice::Light,
            103 => ThemeChoice::Dark,
            _ => ThemeChoice::System,
        };
        let selected = sender.tag();
        if let Some(menu) = unsafe { sender.menu() } {
            for index in 0..menu.numberOfItems() {
                if let Some(item) = menu.itemAtIndex(index) {
                    let tag = item.tag();
                    if (101..=103).contains(&tag) {
                        item.setState(if tag == selected {
                            NSControlStateValueOn
                        } else {
                            NSControlStateValueOff
                        });
                    }
                }
            }
        }
        send_macos_menu_event(UserEvent::SetTheme(choice));
    }

    fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    fn symbol_button(
        symbol: &str,
        fallback_title: &str,
        tooltip: &str,
        action: Sel,
        target: &AnyObject,
        mtm: MainThreadMarker,
    ) -> objc2::rc::Retained<NSButton> {
        let accessibility = NSString::from_str(tooltip);
        let symbol_name = NSString::from_str(symbol);
        let button = if let Some(image) =
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &symbol_name,
                Some(&accessibility),
            ) {
            unsafe {
                NSButton::buttonWithImage_target_action(&image, Some(target), Some(action), mtm)
            }
        } else {
            unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(fallback_title),
                    Some(target),
                    Some(action),
                    mtm,
                )
            }
        };
        button.setBordered(false);
        button.setToolTip(Some(&accessibility));
        button
    }

    extern "C" fn show_about(controller: &AnyObject, _: Sel, _: &AnyObject) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };

        let alert = NSAlert::new(mtm);
        alert.setAlertStyle(NSAlertStyle::Informational);
        alert.setMessageText(&NSString::from_str("MD Previewer"));
        alert.setInformativeText(&NSString::from_str(&format!(
            "Version {}\n\nFollow local document links between lightweight tabs, keep your reading position between preview and source, and inspect character counts or zoom the content without changing the app chrome.",
            env!("CARGO_PKG_VERSION")
        )));

        let accessory = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 34.0, 28.0));
        let github = symbol_button(
            "chevron.left.forwardslash.chevron.right",
            "GitHub",
            "GitHub",
            sel!(mdPreviewerOpenGitHub:),
            controller,
            mtm,
        );
        github.setFrame(rect(4.0, 1.0, 26.0, 26.0));
        accessory.addSubview(&github);
        alert.setAccessoryView(Some(&accessory));
        alert.addButtonWithTitle(&NSString::from_str("OK"));

        alert.runModal();
    }

    static REGISTER_CLASS: Once = Once::new();
    REGISTER_CLASS.call_once(|| {
        let mut builder =
            ClassBuilder::new(c"MDPreviewerMenuController", NSObject::class()).unwrap();
        unsafe {
            builder.add_method(
                sel!(mdPreviewerOpenFile:),
                open_file as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerNewFile:),
                new_file as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerCloseTab:),
                close_tab as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerShowFind:),
                show_find as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerToggleEdit:),
                toggle_edit as extern "C" fn(_, _, _),
            );
            builder.add_method(sel!(mdPreviewerPrint:), print as extern "C" fn(_, _, _));
            builder.add_method(sel!(mdPreviewerQuit:), quit as extern "C" fn(_, _, _));
            builder.add_method(
                sel!(mdPreviewerOpenGitHub:),
                open_github as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerSetTheme:),
                set_theme as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(mdPreviewerShowAbout:),
                show_about as extern "C" fn(_, _, _),
            );
        }
        let _ = builder.register();
    });

    AnyClass::get(c"MDPreviewerMenuController").unwrap()
}

#[cfg(target_os = "macos")]
fn install_macos_menu(proxy: EventLoopProxy<UserEvent>, theme: ThemeChoice) {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{msg_send, sel, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSEventModifierFlags, NSMenu,
        NSMenuItem,
    };
    use objc2_foundation::{MainThreadMarker, NSString};

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    MACOS_MENU_PROXY.with(|cell| {
        *cell.borrow_mut() = Some(proxy);
    });

    fn menu(title: &str, mtm: MainThreadMarker) -> objc2::rc::Retained<NSMenu> {
        NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title))
    }

    fn item(
        title: &str,
        action: Option<Sel>,
        key: &str,
        modifiers: NSEventModifierFlags,
        mtm: MainThreadMarker,
    ) -> objc2::rc::Retained<NSMenuItem> {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                action,
                &NSString::from_str(key),
            )
        };
        item.setKeyEquivalentModifierMask(modifiers);
        item
    }

    fn command_item(
        title: &str,
        action: Sel,
        key: &str,
        modifiers: NSEventModifierFlags,
        target: &AnyObject,
        mtm: MainThreadMarker,
    ) -> objc2::rc::Retained<NSMenuItem> {
        let item = item(title, Some(action), key, modifiers, mtm);
        unsafe {
            item.setTarget(Some(target));
        }
        item
    }

    let app = NSApplication::sharedApplication(mtm);
    let main_menu = menu("", mtm);
    let controller: Retained<AnyObject> = unsafe { msg_send![macos_menu_controller_class(), new] };
    let controller_ptr = Retained::into_raw(controller);
    let controller = unsafe { &*controller_ptr };

    let app_menu = menu("MD Previewer", mtm);
    app_menu.setAutoenablesItems(false);
    app_menu.addItem(&command_item(
        "About MD Previewer",
        sel!(mdPreviewerShowAbout:),
        "",
        NSEventModifierFlags::empty(),
        controller,
        mtm,
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    app_menu.addItem(&command_item(
        "GitHub Repository",
        sel!(mdPreviewerOpenGitHub:),
        "",
        NSEventModifierFlags::empty(),
        controller,
        mtm,
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    app_menu.addItem(&command_item(
        "Quit MD Previewer",
        sel!(mdPreviewerQuit:),
        "q",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    let app_menu_item = item("MD Previewer", None, "", NSEventModifierFlags::empty(), mtm);
    app_menu_item.setSubmenu(Some(&app_menu));
    main_menu.addItem(&app_menu_item);

    let file_menu = menu("File", mtm);
    file_menu.setAutoenablesItems(false);
    file_menu.addItem(&command_item(
        "New Markdown...",
        sel!(mdPreviewerNewFile:),
        "n",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    file_menu.addItem(&command_item(
        "Open...",
        sel!(mdPreviewerOpenFile:),
        "o",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    file_menu.addItem(&command_item(
        "Close Tab",
        sel!(mdPreviewerCloseTab:),
        "w",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    file_menu.addItem(&NSMenuItem::separatorItem(mtm));
    file_menu.addItem(&command_item(
        "Print...",
        sel!(mdPreviewerPrint:),
        "p",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    let file_menu_item = item("File", None, "", NSEventModifierFlags::empty(), mtm);
    file_menu_item.setSubmenu(Some(&file_menu));
    main_menu.addItem(&file_menu_item);

    let edit_menu = menu("Edit", mtm);
    edit_menu.addItem(&item(
        "Undo",
        Some(sel!(undo:)),
        "z",
        NSEventModifierFlags::Command,
        mtm,
    ));
    edit_menu.addItem(&item(
        "Redo",
        Some(sel!(redo:)),
        "z",
        NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
        mtm,
    ));
    edit_menu.addItem(&NSMenuItem::separatorItem(mtm));
    edit_menu.addItem(&item(
        "Cut",
        Some(sel!(cut:)),
        "x",
        NSEventModifierFlags::Command,
        mtm,
    ));
    edit_menu.addItem(&item(
        "Copy",
        Some(sel!(copy:)),
        "c",
        NSEventModifierFlags::Command,
        mtm,
    ));
    edit_menu.addItem(&item(
        "Paste",
        Some(sel!(paste:)),
        "v",
        NSEventModifierFlags::Command,
        mtm,
    ));
    edit_menu.addItem(&item(
        "Select All",
        Some(sel!(selectAll:)),
        "a",
        NSEventModifierFlags::Command,
        mtm,
    ));
    let edit_menu_item = item("Edit", None, "", NSEventModifierFlags::empty(), mtm);
    edit_menu_item.setSubmenu(Some(&edit_menu));
    main_menu.addItem(&edit_menu_item);

    let view_menu = menu("View", mtm);
    view_menu.setAutoenablesItems(false);
    view_menu.addItem(&command_item(
        "Find",
        sel!(mdPreviewerShowFind:),
        "",
        NSEventModifierFlags::empty(),
        controller,
        mtm,
    ));
    view_menu.addItem(&command_item(
        "Toggle Edit Mode",
        sel!(mdPreviewerToggleEdit:),
        "e",
        NSEventModifierFlags::Command,
        controller,
        mtm,
    ));
    view_menu.addItem(&NSMenuItem::separatorItem(mtm));
    let theme_menu = menu("Theme", mtm);
    theme_menu.setAutoenablesItems(false);
    for (label, choice, tag) in [
        ("System", ThemeChoice::System, 101),
        ("Light", ThemeChoice::Light, 102),
        ("Dark", ThemeChoice::Dark, 103),
    ] {
        let theme_item = command_item(
            label,
            sel!(mdPreviewerSetTheme:),
            "",
            NSEventModifierFlags::empty(),
            controller,
            mtm,
        );
        theme_item.setTag(tag);
        theme_item.setState(if choice == theme {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        theme_menu.addItem(&theme_item);
    }
    let theme_menu_item = item("Theme", None, "", NSEventModifierFlags::empty(), mtm);
    theme_menu_item.setSubmenu(Some(&theme_menu));
    view_menu.addItem(&theme_menu_item);
    let view_menu_item = item("View", None, "", NSEventModifierFlags::empty(), mtm);
    view_menu_item.setSubmenu(Some(&view_menu));
    main_menu.addItem(&view_menu_item);

    app.setMainMenu(Some(&main_menu));
}

#[cfg(not(target_os = "macos"))]
fn install_macos_menu(_proxy: EventLoopProxy<UserEvent>, _theme: ThemeChoice) {}

#[cfg(any(target_os = "linux", test))]
fn linux_webkit_compat_env(
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
fn apply_linux_webkit_compat_env() {
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
fn apply_linux_webkit_compat_env() {}

fn is_supported_document(path: &Path) -> bool {
    path.extension()
        .map(|extension| {
            matches!(
                extension.to_string_lossy().to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkd" | "txt"
            )
        })
        .unwrap_or(false)
}

fn local_document_path_from_url(value: &str) -> Option<PathBuf> {
    let url = url::Url::parse(value).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    let path = url.to_file_path().ok()?;
    if !path.is_file() || !is_supported_document(&path) {
        return None;
    }
    fs::canonicalize(path).ok()
}

fn install_file_watcher(
    holder: &Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    proxy: &EventLoopProxy<UserEvent>,
    last_self_write: &Arc<Mutex<Option<SelfWriteRecord>>>,
    path: Option<PathBuf>,
) {
    let mut current = holder.lock().unwrap();
    *current = None;
    let Some(path) = path else {
        return;
    };
    let target_path = path.clone();
    let callback_path = path.clone();
    let proxy = proxy.clone();
    let last_self_write = Arc::clone(last_self_write);
    if let Ok(mut watcher) = notify::recommended_watcher(move |result: Result<Event, _>| {
        if let Ok(event) = result {
            if event_should_reload_file(&event, &callback_path) {
                let last_self_write = last_self_write.lock().unwrap();
                if !self_write_still_matches_disk(last_self_write.as_ref(), &callback_path) {
                    let _ = proxy.send_event(UserEvent::FileChanged(callback_path.clone()));
                }
            }
        }
    }) {
        let scope = watch_scope_for_file(&target_path);
        if scope.exists() && watcher.watch(scope, RecursiveMode::NonRecursive).is_ok() {
            *current = Some(watcher);
        }
    }
}

fn self_write_still_matches_disk(record: Option<&SelfWriteRecord>, path: &Path) -> bool {
    let Some(record) = record else {
        return false;
    };
    record.path == path
        && record.at.elapsed() < Duration::from_millis(500)
        && fs::read(path)
            .map(|content| content == record.content.as_bytes())
            .unwrap_or(false)
}

fn should_protect_external_change(webview_dirty: bool, session_dirty: bool) -> bool {
    webview_dirty || session_dirty
}

#[derive(Debug, PartialEq, Eq)]
enum FinderAction {
    Create { folder: PathBuf, kind: String },
    Terminal { folder: PathBuf },
}

fn parse_finder_action(value: &str) -> Option<FinderAction> {
    let url = url::Url::parse(value).ok()?;
    if url.scheme() != "mdpreviewer" || url.host_str() != Some("finder") {
        return None;
    }
    let query = url.query_pairs().collect::<HashMap<_, _>>();
    let folder = PathBuf::from(query.get("path")?.as_ref());
    match query.get("action")?.as_ref() {
        "create" => Some(FinderAction::Create {
            folder,
            kind: query
                .get("kind")
                .map(|value| value.to_string())
                .unwrap_or_else(|| "md".to_string()),
        }),
        "terminal" => Some(FinderAction::Terminal { folder }),
        _ => None,
    }
}

fn create_finder_file(folder: &Path, kind: &str) -> std::io::Result<PathBuf> {
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

fn normalize_new_markdown_path(mut path: PathBuf) -> PathBuf {
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

fn open_terminal(folder: &Path) -> bool {
    std::process::Command::new("open")
        .args(["-a", "Terminal"])
        .arg(folder)
        .spawn()
        .is_ok()
}

#[cfg(target_os = "macos")]
fn register_finder_extension() {
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
fn register_finder_extension() {}

fn persist_session(session: &DocumentSession) {
    // 新窗口模式是多进程、单标签模式按设计不恢复，这两种情况下落盘只会互相覆盖或把老标签带回来
    if !SESSION_ENABLED.load(Ordering::SeqCst) {
        return;
    }
    if let Err(error) = session.save(&session_path()) {
        eprintln!("Could not save tab session: {error}");
    }
}

fn update_author_doc(webview: &WebView, raw_md: &str) {
    let doc = author_doc(raw_md);
    let state = serde_json::json!({
        "titleLine": doc.title_line,
        "title": doc.title,
    });
    let _ = webview.evaluate_script(&format!(
        "if(window.__setAuthorDoc)window.__setAuthorDoc({state});"
    ));
}

fn update_sidebar(webview: &WebView, session: &DocumentSession, recent: &[PathBuf]) {
    let state = sidebar_json(session, recent);
    let _ = webview.evaluate_script(&format!(
        "if(window.__setSidebar)window.__setSidebar({state});"
    ));
}

fn update_settings_ui(webview: &WebView, settings: &Settings) {
    let state = settings.to_json();
    let _ = webview.evaluate_script(&format!(
        "if(window.__setSettings)window.__setSettings({state});"
    ));
}

fn update_tabs(webview: &WebView, session: &DocumentSession) {
    let state = tabs_json(session);
    let _ = webview.evaluate_script(&format!("if(window.__setTabs)window.__setTabs({state});"));
}

fn update_window_title(window: &Window, session: &DocumentSession) {
    let title = session
        .active()
        .map(|tab| {
            let name = tab
                .path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| tab.path.to_string_lossy().to_string());
            format!(
                "{}{} — MD Previewer",
                if tab.dirty { "• " } else { "" },
                name
            )
        })
        .unwrap_or_else(|| "MD Previewer".to_string());
    window.set_title(&title);
}

fn render_active_document(
    webview: &WebView,
    window: &Window,
    session: &mut DocumentSession,
    recent_files: &Arc<Mutex<Vec<PathBuf>>>,
    enhance_flags: &Arc<Mutex<EnhanceFlags>>,
    loaded_enhancers: &mut EnhanceFlags,
    strings: &Strings,
) {
    let Some(active) = session.active().cloned() else {
        APP_DIRTY.store(false, Ordering::SeqCst);
        let html = empty_preview_html(strings, &recent_files.lock().unwrap());
        let _ = webview.evaluate_script(&format!(
            "if(window.__setEmptyPreview)window.__setEmptyPreview('{}');",
            escape_js(&html)
        ));
        update_author_doc(webview, "");
        update_tabs(webview, session);
        update_sidebar(webview, session, &recent_files.lock().unwrap());
        update_window_title(window, session);
        return;
    };

    match read_document_with_encoding(&active.path, active.encoding.as_deref()) {
        Ok((raw, resolved_encoding)) => {
            if let Some(tab) = session.get_mut(active.id) {
                tab.missing = false;
                if tab.encoding.is_none() {
                    tab.encoding = Some(resolved_encoding.to_string());
                }
            }
            remember_recent_file(recent_files, &active.path);
            let is_txt = is_txt_document(&active.path);
            let (html, flags, base_href) = document_to_html(&active.path, &raw);
            let base_href = base_href.unwrap_or_default();
            *enhance_flags.lock().unwrap() = flags;
            let _ = webview.evaluate_script(&format!(
                "if(window.__setContent)window.__setContent('{}', '{}', '{}', {}, {});if(window.__setEncoding)window.__setEncoding('{}');",
                escape_js(&html),
                escape_js(&raw),
                escape_js(&base_href),
                flags.math,
                flags.mermaid,
                escape_js(resolved_encoding)
            ));
            if is_txt {
                update_author_doc(webview, "");
            } else {
                update_author_doc(webview, &raw);
            }
            for script in build_enhancer_bootstrap(flags, *loaded_enhancers) {
                let _ = webview.evaluate_script(&script);
            }
            loaded_enhancers.math |= flags.math;
            loaded_enhancers.mermaid |= flags.mermaid;
            if active.edit_on_open {
                if let Some(tab) = session.get_mut(active.id) {
                    tab.edit_on_open = false;
                }
                let _ = webview.evaluate_script(
                    "if(window.__mdPreviewerEnterEdit)window.__mdPreviewerEnterEdit();",
                );
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(tab) = session.get_mut(active.id) {
                tab.missing = true;
            }
            if active.dirty {
                show_warning_dialog(
                    strings.missing_title,
                    "The file disappeared while it still has unsaved edits. The editor content has been kept.",
                );
            } else {
                let html = missing_preview_html(active.id, &active.path, strings);
                let _ = webview.evaluate_script(&format!(
                    "if(window.__setMissing)window.__setMissing('{}');",
                    escape_js(&html)
                ));
            }
        }
        Err(error) => {
            show_warning_dialog(strings.cannot_read, &error.to_string());
        }
    }

    APP_DIRTY.store(
        session.active().map(|tab| tab.dirty).unwrap_or(false),
        Ordering::SeqCst,
    );
    update_tabs(webview, session);
    update_sidebar(webview, session, &recent_files.lock().unwrap());
    update_window_title(window, session);
}

fn main() {
    apply_linux_webkit_compat_env();

    // Bench instrumentation: MD_PREVIEWER_BENCH=1 makes the app print
    // cold-start timings to stderr and exit as soon as the first paint
    // lands. Costs nothing outside bench mode.
    let bench = std::env::var("MD_PREVIEWER_BENCH").is_ok();
    let t0 = Instant::now();
    let bench_log = |label: &str| {
        if bench {
            eprintln!("[bench] +{}ms {}", t0.elapsed().as_millis(), label);
        }
    };
    bench_log("main_start");

    // CLI: md-previewer [--edit] [file.md ...]
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| is_help_arg(arg)) {
        print_help();
        return;
    }
    let edit_from_cli = args.iter().any(|arg| arg == "--edit");
    let cli_paths = args
        .into_iter()
        .filter(|arg| arg != "--edit")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_relative() {
                std::env::current_dir().unwrap_or_default().join(path)
            } else {
                path
            }
        })
        .filter(|path| {
            if path.exists() && is_supported_document(path) {
                true
            } else {
                eprintln!("File not found or unsupported: {}", path.display());
                false
            }
        })
        .collect::<Vec<_>>();

    let lang = detect_lang();
    let strings = Strings::for_lang(lang);
    let settings = Settings::load(&settings_path());
    SESSION_ENABLED.store(settings.keeps_session(), Ordering::SeqCst);
    let instance = match single_instance::prepare(
        &config_dir(),
        &cli_paths,
        edit_from_cli,
        settings.open_mode == OpenMode::NewTab,
    ) {
        single_instance::Startup::Primary(server) => server,
        single_instance::Startup::Forwarded => return,
    };
    register_as_default(lang);
    register_finder_extension();
    bench_log("after_register");

    let mut initial_session = if settings.keeps_session() {
        DocumentSession::load(&session_path())
    } else {
        DocumentSession::default()
    };
    initial_session.single_tab = settings.tab_mode == TabMode::Single;
    for path in cli_paths {
        initial_session.open(path, edit_from_cli);
    }

    let event_loop: EventLoop<UserEvent> = EventLoopBuilder::with_user_event().build();
    let proxy = event_loop.create_proxy();
    instance.start(proxy.clone());
    let initial_theme = load_theme_choice();
    install_macos_menu(proxy.clone(), initial_theme);

    let title = initial_session
        .active()
        .and_then(|tab| tab.path.file_name())
        .map(|name| format!("{} — MD Previewer", name.to_string_lossy()))
        .unwrap_or_else(|| "MD Previewer".to_string());

    let geom = load_window_geom()
        .filter(|g| geom_visible(g, &event_loop))
        .unwrap_or_else(|| centered_geom(&event_loop));

    let mut window_builder = WindowBuilder::new()
        .with_title(&title)
        .with_inner_size(LogicalSize::new(geom.w, geom.h))
        .with_position(LogicalPosition::new(geom.x, geom.y))
        .with_theme(initial_theme.tao_theme());
    if let Some(icon) = load_window_icon() {
        window_builder = window_builder.with_window_icon(Some(icon));
    }
    let window = window_builder
        .build(&event_loop)
        .expect("failed to build window");
    bench_log("window_built");
    let recent_files: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(load_recent_files()));

    let mut initial_flags = EnhanceFlags::default();
    // 首屏文档的原文：页面是用 with_html 一次成型的，不经过 __setContent，
    // 作者模式要的标题/正文只能在 webview 建好后单独推一次
    let mut initial_raw = String::new();
    let initial_page = match initial_session.active().cloned() {
        Some(tab) => match read_document_with_encoding(&tab.path, tab.encoding.as_deref()) {
            Ok((raw, resolved_encoding)) => {
                if let Some(active) = initial_session.active_mut() {
                    if active.encoding.is_none() {
                        active.encoding = Some(resolved_encoding.to_string());
                    }
                }
                remember_recent_file(&recent_files, &tab.path);
                initial_raw = raw.clone();
                let (html_body, doc_flags, base_href) = document_to_html(&tab.path, &raw);
                initial_flags = doc_flags;
                build_page_with_encoding(
                    &html_body,
                    &raw,
                    base_href.as_deref(),
                    initial_flags,
                    &strings,
                    false,
                    resolved_encoding,
                )
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(active) = initial_session.active_mut() {
                    active.missing = true;
                }
                build_page(
                    &missing_preview_html(tab.id, &tab.path, &strings),
                    "",
                    None,
                    EnhanceFlags::default(),
                    &strings,
                    false,
                )
            }
            Err(error) => build_page(
                &format!(
                    r#"<div class="empty"><div class="icon">#</div><div>{}: {}</div><button class="empty-open" type="button" data-open-file>{}</button></div>"#,
                    html_escape_text(strings.cannot_read),
                    html_escape_text(&error.to_string()),
                    html_escape_text(strings.open_file)
                ),
                "",
                None,
                EnhanceFlags::default(),
                &strings,
                true,
            ),
        },
        None => build_page(
            &empty_preview_html(&strings, &recent_files.lock().unwrap()),
            "",
            None,
            EnhanceFlags::default(),
            &strings,
            true,
        ),
    };

    persist_session(&initial_session);
    let document_session = Arc::new(Mutex::new(initial_session));
    let settings = Arc::new(Mutex::new(settings));
    let enhance_flags: Arc<Mutex<EnhanceFlags>> = Arc::new(Mutex::new(initial_flags));
    let last_self_write: Arc<Mutex<Option<SelfWriteRecord>>> = Arc::new(Mutex::new(None));
    let session_for_ipc = Arc::clone(&document_session);
    let settings_for_ipc = Arc::clone(&settings);
    let recent_files_for_ipc = Arc::clone(&recent_files);
    let last_self_write_for_ipc = Arc::clone(&last_self_write);
    let proxy_for_ipc = proxy.clone();

    // Windows: steer WebView2's cache/cookie tree into %LOCALAPPDATA% instead of
    // letting it drop next to the exe. Other platforms: use default (None).
    let data_dir: Option<PathBuf> = {
        #[cfg(target_os = "windows")]
        {
            let d = config_dir().join("WebView2");
            let _ = fs::create_dir_all(&d);
            Some(d)
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    };
    let mut web_context = wry::WebContext::new(data_dir);

    let proxy_for_navigation = proxy.clone();
    let builder = WebViewBuilder::with_web_context(&mut web_context)
        .with_html(&initial_page)
        .with_navigation_handler(move |url: String| {
            // Let wry load the initial in-memory document; route any real URL click
            // to the app's tab model or the system default handler.
            if url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with("mailto:")
            {
                let _ = open::that(&url);
                false
            } else if let Some(path) = local_document_path_from_url(&url) {
                let _ = proxy_for_navigation.send_event(UserEvent::OpenPaths(vec![path], false));
                false
            } else if url.starts_with("file:") {
                false
            } else {
                true
            }
        })
        .with_ipc_handler(move |msg| {
            let body = msg.body();
            if body == "new-file" {
                let _ = proxy_for_ipc.send_event(UserEvent::NewFile);
            } else if body == "open" {
                let _ = proxy_for_ipc.send_event(UserEvent::OpenFile);
            } else if let Some(index) = body.strip_prefix("open-recent:") {
                if let Ok(index) = index.parse::<usize>() {
                    let path = recent_files_for_ipc.lock().unwrap().get(index).cloned();
                    if let Some(path) = path {
                        if path.exists() {
                            let _ =
                                proxy_for_ipc.send_event(UserEvent::OpenPaths(vec![path], false));
                        } else if forget_recent_file(&recent_files_for_ipc, &path) {
                            let _ = proxy_for_ipc.send_event(UserEvent::RecentChanged);
                        }
                    }
                }
            } else if let Some(raw) = body.strip_prefix("open-doc:") {
                let path = PathBuf::from(raw);
                if path.is_file() && is_supported_document(&path) {
                    let _ = proxy_for_ipc.send_event(UserEvent::OpenPaths(vec![path], false));
                } else if forget_recent_file(&recent_files_for_ipc, &path) {
                    // 侧栏里点到已经不存在的历史条目，顺手把它从最近列表剔掉
                    let _ = proxy_for_ipc.send_event(UserEvent::RecentChanged);
                }
            } else if let Some(url) = body.strip_prefix("open-local-link:") {
                if let Some(path) = local_document_path_from_url(url) {
                    let _ = proxy_for_ipc.send_event(UserEvent::OpenPaths(vec![path], false));
                }
            } else if let Some(rest) = body.strip_prefix("tab-action:") {
                let (header, pending_content) = rest
                    .split_once('\n')
                    .map(|(header, content)| (header, Some(content)))
                    .unwrap_or((rest, None));
                let mut parts = header.splitn(2, ':');
                let action = parts.next().unwrap_or("");
                let id = parts.next().and_then(|value| value.parse::<u64>().ok());
                let Some(id) = id else {
                    return;
                };
                if let Some(content) = pending_content {
                    let active_info = session_for_ipc
                        .lock()
                        .unwrap()
                        .active()
                        .map(|tab| (tab.path.clone(), tab.encoding.clone()));
                    let Some((path, encoding)) = active_info else {
                        return;
                    };
                    *last_self_write_for_ipc.lock().unwrap() = Some(SelfWriteRecord {
                        at: Instant::now(),
                        path: path.clone(),
                        content: content.to_string(),
                    });
                    match write_document_with_encoding(&path, content, encoding.as_deref()) {
                        Ok(()) => {
                            let _ = proxy_for_ipc.send_event(UserEvent::FileSaved(path));
                        }
                        Err(error) => {
                            let _ = proxy_for_ipc.send_event(UserEvent::SaveFailed(format!(
                                "{}: {error}",
                                path.display()
                            )));
                            return;
                        }
                    }
                }
                match action {
                    "activate" => {
                        let _ = proxy_for_ipc.send_event(UserEvent::ActivateTab(id));
                    }
                    "close" => {
                        let _ = proxy_for_ipc.send_event(UserEvent::CloseTab(id));
                    }
                    "close-others" => {
                        let _ = proxy_for_ipc.send_event(UserEvent::CloseOthers(id));
                    }
                    _ => {}
                }
            } else if let Some(change) = body.strip_prefix("set-setting:") {
                let Some((key, value)) = change.split_once('=') else {
                    return;
                };
                // 只有取值真的变了才唤醒事件循环，避免重复点同一项也走一遍落盘和重绘
                if settings_for_ipc.lock().unwrap().apply(key, value) {
                    let _ = proxy_for_ipc.send_event(UserEvent::SettingsChanged);
                }
            } else if let Some(id) = body.strip_prefix("locate-tab:") {
                if let Ok(id) = id.parse::<u64>() {
                    let _ = proxy_for_ipc.send_event(UserEvent::LocateTab(id));
                }
            } else if let Some(id) = body.strip_prefix("reveal-tab:") {
                if let Ok(id) = id.parse::<u64>() {
                    let _ = proxy_for_ipc.send_event(UserEvent::RevealTab(id));
                }
            } else if let Some(content) = body.strip_prefix("render-preview:") {
                let _ = proxy_for_ipc.send_event(UserEvent::RenderPreview(content.to_string()));
            } else if body == "dirty:1" {
                let _ = proxy_for_ipc.send_event(UserEvent::DirtyChanged(true));
            } else if body == "dirty:0" {
                let _ = proxy_for_ipc.send_event(UserEvent::DirtyChanged(false));
            } else if body == "external-change:dirty" {
                let _ = proxy_for_ipc.send_event(UserEvent::ExternalChangeResolved(true));
            } else if body == "external-change:clean" {
                let _ = proxy_for_ipc.send_event(UserEvent::ExternalChangeResolved(false));
            } else if body == "print" {
                let _ = proxy_for_ipc.send_event(UserEvent::Print);
            } else if body == "ready" {
                let _ = proxy_for_ipc.send_event(UserEvent::Ready);
            } else if body == "refresh" {
                if let Some(path) = session_for_ipc
                    .lock()
                    .unwrap()
                    .active()
                    .map(|tab| tab.path.clone())
                {
                    let _ = proxy_for_ipc.send_event(UserEvent::FileChanged(path));
                }
            } else if let Some(content) = body.strip_prefix("save:") {
                let active_info = session_for_ipc
                    .lock()
                    .unwrap()
                    .active()
                    .map(|tab| (tab.path.clone(), tab.encoding.clone()));
                if let Some((path, encoding)) = active_info {
                    *last_self_write_for_ipc.lock().unwrap() = Some(SelfWriteRecord {
                        at: Instant::now(),
                        path: path.clone(),
                        content: content.to_string(),
                    });
                    match write_document_with_encoding(&path, content, encoding.as_deref()) {
                        Ok(()) => {
                            let _ = proxy_for_ipc.send_event(UserEvent::FileSaved(path));
                        }
                        Err(error) => {
                            let _ = proxy_for_ipc.send_event(UserEvent::SaveFailed(format!(
                                "{}: {error}",
                                path.display()
                            )));
                        }
                    }
                }
            } else if let Some(enc) = body.strip_prefix("set-encoding:") {
                let _ = proxy_for_ipc.send_event(UserEvent::SetEncoding(enc.to_string()));
            }
        })
        .with_drag_drop_handler({
            let proxy = proxy.clone();
            move |event| {
                if let wry::DragDropEvent::Drop { paths, .. } = event {
                    let paths = paths
                        .into_iter()
                        .filter(|path| is_supported_document(path))
                        .collect::<Vec<_>>();
                    if !paths.is_empty() {
                        let _ = proxy.send_event(UserEvent::OpenPaths(paths, false));
                    }
                }
                true
            }
        });

    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;

        let vbox = window
            .default_vbox()
            .expect("failed to get default GTK container");
        builder.build_gtk(vbox).expect("failed to build webview")
    };
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window).expect("failed to build webview");
    bench_log("webview_built");
    let session_for_event = Arc::clone(&document_session);
    let settings_for_event = Arc::clone(&settings);
    update_tabs(&webview, &session_for_event.lock().unwrap());
    update_settings_ui(&webview, &settings_for_event.lock().unwrap());
    let initial_is_txt = session_for_event
        .lock()
        .unwrap()
        .active()
        .map(|tab| is_txt_document(&tab.path))
        .unwrap_or(false);
    if initial_is_txt {
        update_author_doc(&webview, "");
    } else {
        update_author_doc(&webview, &initial_raw);
    }
    update_sidebar(
        &webview,
        &session_for_event.lock().unwrap(),
        &recent_files.lock().unwrap(),
    );
    if session_for_event
        .lock()
        .unwrap()
        .active()
        .map(|tab| tab.edit_on_open && !tab.missing)
        .unwrap_or(false)
    {
        let _ = webview
            .evaluate_script("if(window.__mdPreviewerEnterEdit)window.__mdPreviewerEnterEdit();");
        if let Some(tab) = session_for_event.lock().unwrap().active_mut() {
            tab.edit_on_open = false;
        }
    }

    // hljs + extra language packs aren't part of first-paint HTML anymore.
    // We push them in via evaluate_script the moment the webview tells us
    // it's painted (IPC 'ready'). Keeps ~125KB out of the HTML-parse critical
    // path so the app window shows content faster on cold start.
    let hljs_bootstrap = format!(
        "(function(){{{hljs_js};{hljs_extra};try{{window.hljs=hljs;}}catch(e){{}}if(typeof hljs!=='undefined'&&hljs.highlightAll){{hljs.highlightAll();}}}})();",
        hljs_js = HLJS_JS,
        hljs_extra = HLJS_EXTRA_LANGS,
    );

    // File watcher state
    let watcher_holder: Arc<Mutex<Option<notify::RecommendedWatcher>>> = Arc::new(Mutex::new(None));
    let watcher_for_event = Arc::clone(&watcher_holder);
    let initial_watch_path = session_for_event
        .lock()
        .unwrap()
        .active()
        .map(|tab| tab.path.clone());
    install_file_watcher(
        &watcher_holder,
        &proxy,
        &last_self_write,
        initial_watch_path,
    );

    let mut loaded_enhancers = EnhanceFlags::default();
    let mut pending_window_close = false;
    let mut warned_external_change: Option<PathBuf> = None;
    let mut pending_external_change: Option<PathBuf> = None;
    let mut sidebar_open_applied = settings_for_event.lock().unwrap().sidebar_open;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            TaoEvent::UserEvent(UserEvent::NewFile) => {
                if session_for_event
                    .lock()
                    .unwrap()
                    .active()
                    .map(|tab| tab.dirty)
                    .unwrap_or(false)
                {
                    let _ = webview.evaluate_script(
                        "if(window.__mdPreviewerNewFile)window.__mdPreviewerNewFile();",
                    );
                    return;
                }
                let current_dir = session_for_event
                    .lock()
                    .unwrap()
                    .active()
                    .and_then(|tab| tab.path.parent().map(Path::to_path_buf));
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("Markdown", &["md", "markdown", "mdown", "mkd"])
                    .set_file_name(strings.new_filename);
                if let Some(current_dir) = current_dir {
                    dialog = dialog.set_directory(current_dir);
                }
                if let Some(path) = dialog.save_file() {
                    let path = normalize_new_markdown_path(path);
                    match fs::write(&path, "") {
                        Ok(()) => {
                            let _ = proxy.send_event(UserEvent::OpenPaths(vec![path], true));
                        }
                        Err(error) => {
                            show_warning_dialog("Could Not Create File", &error.to_string());
                        }
                    }
                }
            }
            TaoEvent::UserEvent(UserEvent::OpenFile) => {
                if session_for_event
                    .lock()
                    .unwrap()
                    .active()
                    .map(|tab| tab.dirty)
                    .unwrap_or(false)
                {
                    let _ = webview.evaluate_script(
                        "if(window.__mdPreviewerOpenFile)window.__mdPreviewerOpenFile();",
                    );
                    return;
                }
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("Supported Documents", &["md", "markdown", "mdown", "mkd", "txt"])
                    .add_filter("Markdown", &["md", "markdown", "mdown", "mkd"])
                    .add_filter("Text", &["txt"])
                    .pick_files()
                {
                    let _ = proxy.send_event(UserEvent::OpenPaths(paths, false));
                }
            }
            TaoEvent::UserEvent(UserEvent::OpenPaths(paths, edit_on_open)) => {
                window.set_minimized(false);
                window.set_visible(true);
                window.set_focus();
                if paths.is_empty() {
                    return;
                }
                let mut session = session_for_event.lock().unwrap();
                let previous_active = session.active_id;
                let preserve_active = session.active().map(|tab| tab.dirty).unwrap_or(false);
                for path in paths.into_iter().filter(|path| is_supported_document(path)) {
                    session.open(path, edit_on_open);
                }
                if preserve_active {
                    if let Some(id) = previous_active {
                        session.activate(id);
                    }
                }
                persist_session(&session);
                if preserve_active {
                    update_tabs(&webview, &session);
                } else {
                    render_active_document(
                        &webview,
                        &window,
                        &mut session,
                        &recent_files,
                        &enhance_flags,
                        &mut loaded_enhancers,
                        &strings,
                    );
                }
                let path = session.active().map(|tab| tab.path.clone());
                drop(session);
                install_file_watcher(&watcher_for_event, &proxy, &last_self_write, path);
            }
            TaoEvent::UserEvent(UserEvent::ActivateTab(id)) => {
                let mut session = session_for_event.lock().unwrap();
                if session.activate(id) {
                    persist_session(&session);
                    render_active_document(
                        &webview,
                        &window,
                        &mut session,
                        &recent_files,
                        &enhance_flags,
                        &mut loaded_enhancers,
                        &strings,
                    );
                    let path = session.active().map(|tab| tab.path.clone());
                    drop(session);
                    install_file_watcher(&watcher_for_event, &proxy, &last_self_write, path);
                }
            }
            TaoEvent::UserEvent(UserEvent::CloseTab(id)) => {
                let mut session = session_for_event.lock().unwrap();
                let was_active = session.active_id == Some(id);
                if session.close(id) {
                    persist_session(&session);
                    if was_active {
                        render_active_document(
                            &webview,
                            &window,
                            &mut session,
                            &recent_files,
                            &enhance_flags,
                            &mut loaded_enhancers,
                            &strings,
                        );
                        let path = session.active().map(|tab| tab.path.clone());
                        drop(session);
                        install_file_watcher(&watcher_for_event, &proxy, &last_self_write, path);
                    } else {
                        update_tabs(&webview, &session);
                    }
                }
            }
            TaoEvent::UserEvent(UserEvent::CloseOthers(id)) => {
                let mut session = session_for_event.lock().unwrap();
                let was_active = session.active_id == Some(id);
                if session.close_others(id) {
                    persist_session(&session);
                    if !was_active {
                        render_active_document(
                            &webview,
                            &window,
                            &mut session,
                            &recent_files,
                            &enhance_flags,
                            &mut loaded_enhancers,
                            &strings,
                        );
                        let path = session.active().map(|tab| tab.path.clone());
                        drop(session);
                        install_file_watcher(&watcher_for_event, &proxy, &last_self_write, path);
                    } else {
                        update_tabs(&webview, &session);
                    }
                }
            }
            TaoEvent::UserEvent(UserEvent::RevealTab(id)) => {
                let path = session_for_event
                    .lock()
                    .unwrap()
                    .tabs
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.path.clone());
                if let Some(path) = path {
                    reveal_in_file_manager(&path);
                }
            }
            TaoEvent::UserEvent(UserEvent::RenderPreview(content)) => {
                let path = session_for_event
                    .lock()
                    .unwrap()
                    .active()
                    .map(|t| t.path.clone());
                if let Some(path) = path {
                    let (html, flags, _) = document_to_html(&path, &content);
                    let _ = webview.evaluate_script(&format!(
                        "if(window.__setLivePreview)window.__setLivePreview('{}', {}, {});",
                        escape_js(&html),
                        flags.math,
                        flags.mermaid
                    ));
                }
            }
            TaoEvent::UserEvent(UserEvent::CloseActiveTab) => {
                if session_for_event.lock().unwrap().active_id.is_some() {
                    let _ = webview.evaluate_script(
                        "if(window.__mdPreviewerCloseActiveTab)window.__mdPreviewerCloseActiveTab();",
                    );
                } else {
                    save_window_geom(&window);
                    *control_flow = ControlFlow::Exit;
                }
            }
            TaoEvent::UserEvent(UserEvent::LocateTab(id)) => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Supported Documents", &["md", "markdown", "mdown", "mkd", "txt"])
                    .add_filter("Markdown", &["md", "markdown", "mdown", "mkd"])
                    .add_filter("Text", &["txt"])
                    .pick_file()
                {
                    let mut session = session_for_event.lock().unwrap();
                    if session.relocate(id, path) && session.activate(id) {
                        persist_session(&session);
                        render_active_document(
                            &webview,
                            &window,
                            &mut session,
                            &recent_files,
                            &enhance_flags,
                            &mut loaded_enhancers,
                            &strings,
                        );
                        let path = session.active().map(|tab| tab.path.clone());
                        drop(session);
                        install_file_watcher(&watcher_for_event, &proxy, &last_self_write, path);
                    } else {
                        show_warning_dialog("Already Open", "That file is already open in another tab.");
                    }
                }
            }
            TaoEvent::UserEvent(UserEvent::FileChanged(path)) => {
                if session_for_event
                    .lock()
                    .unwrap()
                    .active()
                    .map(|tab| tab.path.as_path())
                    == Some(path.as_path())
                {
                    pending_external_change = Some(path);
                    let _ = webview.evaluate_script(
                        "if(window.__mdPreviewerResolveExternalChange)window.__mdPreviewerResolveExternalChange();",
                    );
                }
            }
            TaoEvent::UserEvent(UserEvent::ExternalChangeResolved(webview_dirty)) => {
                let Some(path) = pending_external_change.take() else {
                    return;
                };
                let mut session = session_for_event.lock().unwrap();
                if session.active().map(|tab| tab.path.as_path()) == Some(path.as_path()) {
                    let session_dirty = session.active().map(|tab| tab.dirty).unwrap_or(false);
                    if should_protect_external_change(webview_dirty, session_dirty) {
                        if let Some(tab) = session.active_mut() {
                            tab.dirty = true;
                        }
                        APP_DIRTY.store(true, Ordering::SeqCst);
                        let _ = webview.evaluate_script(
                            "if(window.__mdPreviewerPauseAutosave)window.__mdPreviewerPauseAutosave();",
                        );
                        if warned_external_change.as_ref() != Some(&path) {
                            show_warning_dialog(
                                "File Changed on Disk",
                                "Automatic saving is paused and your edits are still in the editor. Press Cmd/Ctrl+S to replace the disk version, or reopen the file to keep the external version.",
                            );
                            warned_external_change = Some(path);
                        }
                        return;
                    }
                    render_active_document(
                        &webview,
                        &window,
                        &mut session,
                        &recent_files,
                        &enhance_flags,
                        &mut loaded_enhancers,
                        &strings,
                    );
                    persist_session(&session);
                }
            }
            TaoEvent::UserEvent(UserEvent::SetEncoding(enc)) => {
                let mut session = session_for_event.lock().unwrap();
                if let Some(tab) = session.active_mut() {
                    tab.encoding = Some(enc);
                    tab.dirty = false;
                }
                render_active_document(
                    &webview,
                    &window,
                    &mut session,
                    &recent_files,
                    &enhance_flags,
                    &mut loaded_enhancers,
                    &strings,
                );
                persist_session(&session);
            }
            TaoEvent::UserEvent(UserEvent::FileSaved(path)) => {
                if warned_external_change.as_ref() == Some(&path) {
                    warned_external_change = None;
                }
                let mut session = session_for_event.lock().unwrap();
                let active_matches = session.active().map(|tab| tab.path.as_path()) == Some(path.as_path());
                if let Some(tab) = session.tabs.iter_mut().find(|tab| tab.path == path) {
                    tab.dirty = false;
                    tab.missing = false;
                }
                APP_DIRTY.store(false, Ordering::SeqCst);
                if active_matches {
                    let enc = session.active().and_then(|t| t.encoding.clone());
                    if let Ok((raw, _)) = read_document_with_encoding(&path, enc.as_deref()) {
                        let is_txt = is_txt_document(&path);
                        let (html, flags, _) = document_to_html(&path, &raw);
                        *enhance_flags.lock().unwrap() = flags;
                        let _ = webview.evaluate_script(&format!(
                            "if(window.__setPreview)window.__setPreview('{}', {}, {});if(window.__markSaved)window.__markSaved('{}');",
                            escape_js(&html),
                            flags.math,
                            flags.mermaid,
                            escape_js(&raw)
                        ));
                        if is_txt {
                            update_author_doc(&webview, "");
                        } else {
                            update_author_doc(&webview, &raw);
                        }
                        for script in build_enhancer_bootstrap(flags, loaded_enhancers) {
                            let _ = webview.evaluate_script(&script);
                        }
                        loaded_enhancers.math |= flags.math;
                        loaded_enhancers.mermaid |= flags.mermaid;
                    }
                }
                persist_session(&session);
                update_tabs(&webview, &session);
                update_window_title(&window, &session);
                if pending_window_close {
                    save_window_geom(&window);
                    *control_flow = ControlFlow::Exit;
                }
            }
            TaoEvent::UserEvent(UserEvent::SaveFailed(error)) => {
                pending_window_close = false;
                show_warning_dialog("Could Not Save", &error);
            }
            TaoEvent::UserEvent(UserEvent::DirtyChanged(dirty)) => {
                APP_DIRTY.store(dirty, Ordering::SeqCst);
                let mut session = session_for_event.lock().unwrap();
                if let Some(tab) = session.active_mut() {
                    tab.dirty = dirty;
                }
                update_tabs(&webview, &session);
                update_window_title(&window, &session);
            }
            TaoEvent::UserEvent(UserEvent::ToggleEdit) => {
                let _ = webview.evaluate_script(
                    "if(window.__mdPreviewerToggleEdit)window.__mdPreviewerToggleEdit();",
                );
            }
            TaoEvent::UserEvent(UserEvent::ShowFind) => {
                let _ = webview
                    .evaluate_script("if(window.__mdPreviewerShowFind)window.__mdPreviewerShowFind();");
            }
            TaoEvent::UserEvent(UserEvent::RecentChanged) => {
                let html = empty_preview_html(&strings, &recent_files.lock().unwrap());
                let js = format!(
                    "if(window.__setEmptyPreview)window.__setEmptyPreview('{}');",
                    escape_js(&html)
                );
                let _ = webview.evaluate_script(&js);
                update_sidebar(
                    &webview,
                    &session_for_event.lock().unwrap(),
                    &recent_files.lock().unwrap(),
                );
            }
            TaoEvent::UserEvent(UserEvent::Print) => {
                let _ = webview.print();
            }
            TaoEvent::UserEvent(UserEvent::Quit) => {
                if APP_DIRTY.load(Ordering::SeqCst) && !pending_window_close {
                    pending_window_close = true;
                    let _ = webview.evaluate_script(
                        "if(window.__mdPreviewerSave)window.__mdPreviewerSave();",
                    );
                } else if !pending_window_close {
                    save_window_geom(&window);
                    persist_session(&session_for_event.lock().unwrap());
                    *control_flow = ControlFlow::Exit;
                }
            }
            TaoEvent::UserEvent(UserEvent::SettingsChanged) => {
                let current = *settings_for_event.lock().unwrap();
                if current.sidebar_open != sidebar_open_applied {
                    resize_for_sidebar(&window, current.sidebar_open);
                    sidebar_open_applied = current.sidebar_open;
                }
                SESSION_ENABLED.store(current.keeps_session(), Ordering::SeqCst);
                if let Err(error) = current.save(&settings_path()) {
                    eprintln!("Could not save settings: {error}");
                }
                let mut session = session_for_event.lock().unwrap();
                session.single_tab = current.tab_mode == TabMode::Single;
                if session.single_tab {
                    session.collapse_to_active();
                }
                if current.keeps_session() {
                    persist_session(&session);
                } else {
                    // 留着旧会话文件会在下次启动又把老标签拉回来，正是这个设置要避免的
                    let _ = fs::remove_file(session_path());
                }
                update_tabs(&webview, &session);
                drop(session);
                update_settings_ui(&webview, &current);
            }
            TaoEvent::UserEvent(UserEvent::SetTheme(choice)) => {
                save_theme_choice(choice);
                window.set_theme(choice.tao_theme());
            }
            TaoEvent::UserEvent(UserEvent::OpenUrl(url)) => {
                let _ = open::that(url);
            }
            TaoEvent::UserEvent(UserEvent::Ready) => {
                // First paint is on the screen; now push hljs into the page
                // (kept out of first-paint HTML to keep it slim). Always do
                // this, even in bench mode, so subsequent panes would still
                // highlight — bench just exits right after measuring.
                let _ = webview.evaluate_script(&hljs_bootstrap);
                let flags = *enhance_flags.lock().unwrap();
                for js in build_enhancer_bootstrap(flags, loaded_enhancers) {
                    let _ = webview.evaluate_script(&js);
                }
                loaded_enhancers.math |= flags.math;
                loaded_enhancers.mermaid |= flags.mermaid;
                update_tabs(&webview, &session_for_event.lock().unwrap());
                update_sidebar(
                    &webview,
                    &session_for_event.lock().unwrap(),
                    &recent_files.lock().unwrap(),
                );
                if bench {
                    eprintln!("[bench] +{}ms ready", t0.elapsed().as_millis());
                    *control_flow = ControlFlow::Exit;
                }
            }
            // macOS: Finder file opens and embedded Finder Sync actions arrive here.
            TaoEvent::Opened { urls } => {
                let mut paths = Vec::new();
                for url in urls {
                    if let Ok(path) = url.to_file_path() {
                        if is_supported_document(&path) {
                            paths.push(path);
                        }
                        continue;
                    }
                    if let Some(action) = parse_finder_action(url.as_str()) {
                        match action {
                            FinderAction::Create { folder, kind } => match create_finder_file(&folder, &kind) {
                                Ok(path) if kind == "md" => {
                                    let _ = proxy.send_event(UserEvent::OpenPaths(vec![path], true));
                                }
                                Ok(_) => {}
                                Err(error) => show_warning_dialog("Could Not Create File", &error.to_string()),
                            },
                            FinderAction::Terminal { folder } => {
                                if !open_terminal(&folder) {
                                    show_warning_dialog("Could Not Open Terminal", &folder.to_string_lossy());
                                }
                            }
                        }
                    }
                }
                if !paths.is_empty() {
                    let _ = proxy.send_event(UserEvent::OpenPaths(paths, false));
                }
            }
            TaoEvent::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if APP_DIRTY.load(Ordering::SeqCst) && !pending_window_close {
                    pending_window_close = true;
                    let _ = webview.evaluate_script(
                        "if(window.__mdPreviewerSave)window.__mdPreviewerSave();",
                    );
                } else if !pending_window_close {
                    save_window_geom(&window);
                    persist_session(&session_for_event.lock().unwrap());
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}
