#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod assets;
mod desktop;
mod document;
mod escape;
mod finder;
mod i18n;
mod ipc;
mod logger;
mod macos_menu;
mod markdown;
mod page;
mod paths;
mod platform;
mod reading;
mod recent;
mod sanitize;
mod session;
mod settings;
mod sidebar;
mod single_instance;
mod tab_drag_preview;
mod tab_transfer;
mod theme;
mod updater;
mod watch;
mod webview;
mod window;
mod workspace;

#[cfg(test)]
mod tests;

use crate::finder::register_finder_extension;
use crate::i18n::{detect_lang, Strings};
use crate::macos_menu::install_macos_menu;
use crate::paths::{config_dir, is_supported_document, session_path};
use crate::platform::{apply_linux_webkit_compat_env, register_as_default};
use crate::session::DocumentSession;
use crate::settings::{OpenMode, Settings, TabMode, ThemeChoice};
use crate::window::{
    centered_geom, geom_visible, load_window_geom, migrate_theme_file, settings_path,
    show_warning_dialog,
};
use std::env;
use std::path::PathBuf;
use std::time::Instant;
use tao::event_loop::{EventLoop, EventLoopBuilder};
use tao::window::WindowId;

/// 原生菜单发给当前窗口；页面、文件监听与下载回调用 ForWindow 指明所属窗口
#[derive(Debug)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum UserEvent {
    ForWindow(WindowId, Box<UserEvent>),
    PersistSessions,
    OpenWindow(Vec<PathBuf>, bool),
    Ipc(String),
    NewFile,
    OpenFile,
    OpenPaths(Vec<PathBuf>, bool),
    CloseActiveTab,
    FileChanged(PathBuf),
    ToggleEdit,
    ShowFind,
    Print,
    SetTheme(ThemeChoice),
    OpenUrl(&'static str),
    Quit,
    UpdateProgress { downloaded: u64, total: Option<u64> },
    UpdateDownloaded(PathBuf),
    UpdateFailed(String),
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

fn main() {
    logger::init_panic_hook();
    apply_linux_webkit_compat_env();
    let bench = std::env::var("MD_PREVIEWER_BENCH").is_ok();
    let t0 = Instant::now();
    let bench_log = |label: &str| {
        if bench {
            eprintln!("[bench] +{}ms {}", t0.elapsed().as_millis(), label);
        }
    };
    bench_log("main_start");
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
    let mut settings = Settings::load(&settings_path());
    migrate_theme_file(&mut settings);
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
    let mut session = if settings.keeps_session() {
        DocumentSession::load(&session_path())
    } else {
        DocumentSession::default()
    };
    session.single_tab = settings.tab_mode == TabMode::Single;
    for path in cli_paths {
        session.open(path, edit_from_cli);
    }
    let event_loop: EventLoop<UserEvent> = EventLoopBuilder::with_user_event().build();
    let proxy = event_loop.create_proxy();
    instance.start(proxy.clone());
    install_macos_menu(proxy.clone(), settings.theme);
    let geom = load_window_geom()
        .filter(|g| geom_visible(g, &event_loop))
        .unwrap_or_else(|| centered_geom(&event_loop));
    let mut desktop = desktop::Desktop::new(proxy);
    let app = match desktop.create_window(&event_loop, settings, session, &geom) {
        Ok(mut app) => {
            app.bench_started = bench.then_some(t0);
            app
        }
        Err(error) => {
            show_warning_dialog(strings.webview_failed, &error);
            return;
        }
    };
    bench_log("webview_built");
    let mut workspace = workspace::Workspace::new(desktop, app);
    event_loop.run(move |event, target, control_flow| {
        workspace.handle(event, target, control_flow);
    });
}
