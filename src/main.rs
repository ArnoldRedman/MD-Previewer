#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod assets;
mod document;
mod escape;
mod finder;
mod i18n;
mod ipc;
mod macos_menu;
mod markdown;
mod page;
mod paths;
mod platform;
mod recent;
mod sanitize;
mod session;
mod settings;
mod sidebar;
mod single_instance;
mod theme;
mod updater;
mod watch;
mod webview;
mod window;

#[cfg(test)]
mod tests;

use crate::app::App;
use crate::assets::{HLJS_EXTRA_LANGS, HLJS_JS};
use crate::document::{document_to_html, read_document_with_encoding};
use crate::escape::html_escape_text;
use crate::finder::register_finder_extension;
use crate::i18n::{detect_lang, Strings};
use crate::macos_menu::install_macos_menu;
use crate::markdown::EnhanceFlags;
use crate::page::{build_page, build_page_with_encoding, empty_preview_html};
use crate::paths::{config_dir, is_supported_document, recent_files_path, session_path};
use crate::platform::{apply_linux_webkit_compat_env, register_as_default};
use crate::recent::RecentFiles;
use crate::session::DocumentSession;
use crate::settings::{OpenMode, Settings, TabMode, ThemeChoice};
use crate::sidebar::missing_preview_html;
use crate::webview::{navigation_decision, Navigation};
use crate::window::{
    centered_geom, geom_visible, load_window_geom, load_window_icon, migrate_theme_file,
    settings_path,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{env, fs};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

/// 事件循环消息：页面回调、文件监听、更新线程都只把消息投进队列，状态全部在主线程改
/// macOS 原生菜单直接构造其中一部分动作，其他平台的同名动作由页面 IPC 触发，
/// 所以非 macOS 构建下这些变体没有构造点，属于设计如此
#[derive(Debug)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum UserEvent {
    /// 页面发来的原始 IPC 消息，在事件循环里解析和处理，页面回调线程不碰任何状态
    Ipc(String),
    NewFile,
    OpenFile,
    OpenPaths(Vec<PathBuf>, bool),
    CloseActiveTab,
    /// 文件监听线程发现磁盘上的活动文档变了
    FileChanged(PathBuf),
    ToggleEdit,
    ShowFind,
    Print, // route print through wry's native API (WKWebView ignores window.print())
    SetTheme(ThemeChoice),
    OpenUrl(&'static str),
    Quit,
    /// 更新下载线程的进度回报；total 在服务端不给 Content-Length 时为 None
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    UpdateProgress {
        downloaded: u64,
        total: Option<u64>,
    },
    /// 更新包已完整落盘，事件循环里接着写安装脚本并退出
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    UpdateDownloaded(PathBuf),
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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
    let initial_theme = settings.theme;
    install_macos_menu(proxy.clone(), initial_theme);

    let title = session
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
    let mut recent = RecentFiles::load(recent_files_path());

    let mut initial_flags = EnhanceFlags::default();
    let mut initial_author = None;
    let initial_page = match session.active().cloned() {
        Some(tab) => match read_document_with_encoding(&tab.path, tab.encoding.as_deref()) {
            Ok((raw, resolved_encoding)) => {
                if let Some(active) = session.active_mut() {
                    if active.encoding.is_none() {
                        active.encoding = Some(resolved_encoding.to_string());
                    }
                }
                recent.remember(&tab.path);
                let (html_body, doc_flags, base_href) = document_to_html(&tab.path, &raw);
                initial_flags = doc_flags;
                let page = build_page_with_encoding(
                    &html_body,
                    &raw,
                    base_href.as_deref(),
                    initial_flags,
                    &strings,
                    false,
                    resolved_encoding,
                );
                initial_author = Some((tab.id, raw));
                page
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(active) = session.active_mut() {
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
            &empty_preview_html(&strings, recent.paths()),
            "",
            None,
            EnhanceFlags::default(),
            &strings,
            true,
        ),
    };

    // Windows: steer WebView2's cache/cookie tree into %LOCALAPPDATA% instead of
    // letting it drop next to the exe. Other platforms: use default (None).
    let data_dir: Option<PathBuf> = {
        #[cfg(target_os = "windows")]
        {
            let d = config_dir().join("WebView2");
            if let Err(error) = fs::create_dir_all(&d) {
                eprintln!(
                    "Could not create WebView2 data dir {}: {error}",
                    d.display()
                );
            }
            Some(d)
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    };
    let mut web_context = wry::WebContext::new(data_dir);

    let proxy_for_navigation = proxy.clone();
    let proxy_for_ipc = proxy.clone();
    let proxy_for_drop = proxy.clone();
    // with_html 的首屏在 WebView2 里表现为一次 data:text/html 导航，必须放行；见 navigation_decision
    let initial_page_loaded = std::cell::Cell::new(false);
    let builder = WebViewBuilder::with_web_context(&mut web_context)
        .with_html(&initial_page)
        .with_navigation_handler(move |url: String| {
            let mut loaded = initial_page_loaded.get();
            let decision = navigation_decision(&url, &mut loaded);
            initial_page_loaded.set(loaded);
            match decision {
                Navigation::Allow => true,
                Navigation::Block => false,
                Navigation::OpenExternally => {
                    if let Err(error) = open::that(&url) {
                        eprintln!("Could not open {url}: {error}");
                    }
                    false
                }
                Navigation::OpenDocument(path) => {
                    let _ =
                        proxy_for_navigation.send_event(UserEvent::OpenPaths(vec![path], false));
                    false
                }
            }
        })
        .with_ipc_handler(move |msg| {
            let _ = proxy_for_ipc.send_event(UserEvent::Ipc(msg.body().to_string()));
        })
        .with_drag_drop_handler(move |event| {
            if let wry::DragDropEvent::Drop { paths, .. } = event {
                let _ = proxy_for_drop.send_event(UserEvent::OpenPaths(paths, false));
            }
            true
        });
    #[cfg(target_os = "windows")]
    let builder = {
        use wry::WebViewBuilderExtWindows;
        builder.with_theme(initial_theme.wry_theme())
    };

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

    // hljs + extra language packs aren't part of first-paint HTML anymore.
    // We push them in via evaluate_script the moment the webview tells us
    // it's painted (IPC 'ready'). Keeps ~125KB out of the HTML-parse critical
    // path so the app window shows content faster on cold start.
    let hljs_bootstrap = format!(
        "(function(){{{hljs_js};{hljs_extra};try{{window.hljs=hljs;}}catch(e){{}}if(typeof hljs!=='undefined'&&hljs.highlightAll){{hljs.highlightAll();}}}})();",
        hljs_js = HLJS_JS,
        hljs_extra = HLJS_EXTRA_LANGS,
    );

    let sidebar_open_applied = settings.sidebar_open;
    let theme_applied = settings.theme;
    let mut app = App {
        webview,
        window,
        proxy,
        strings,
        settings,
        session,
        recent,
        enhance_flags: initial_flags,
        loaded_enhancers: EnhanceFlags::default(),
        watcher: None,
        last_self_write: Arc::new(Mutex::new(None)),
        hljs_bootstrap,
        initial_author,
        pending_window_close: false,
        warned_external_change: None,
        pending_external_change: None,
        sidebar_open_applied,
        theme_applied,
        update_downloading: false,
        bench_started: bench.then_some(t0),
    };
    app.persist_session();
    app.push_ui_state();
    app.install_watcher();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        app.handle(event, control_flow);
    });
}
