// 每个窗口沿用同一套 App，窗口之间共享最近文件和阅读记录
use crate::app::App;
use crate::assets::{hljs_js, HLJS_EXTRA_LANGS};
use crate::i18n::{detect_lang, Strings};
use crate::markdown::EnhanceFlags;
use crate::page::startup_page;
use crate::paths::config_dir;
use crate::reading::ReadingPositions;
use crate::recent::RecentFiles;
use crate::session::DocumentSession;
use crate::settings::Settings;
use crate::webview::{navigation_decision, Navigation};
use crate::window::{load_window_icon, WindowGeom};
use crate::UserEvent;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

pub(crate) struct Desktop {
    pub(crate) context: wry::WebContext,
    pub(crate) proxy: EventLoopProxy<UserEvent>,
    pub(crate) recent: Rc<RefCell<RecentFiles>>,
    pub(crate) reading: Rc<RefCell<ReadingPositions>>,
}

impl Desktop {
    pub(crate) fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let data_dir: Option<PathBuf> = if cfg!(target_os = "windows") {
            let dir = config_dir().join("WebView2");
            if let Err(error) = std::fs::create_dir_all(&dir) {
                eprintln!("无法创建 WebView2 数据目录: {error}");
            }
            Some(dir)
        } else {
            None
        };
        Self {
            context: wry::WebContext::new(data_dir),
            proxy,
            recent: Rc::new(RefCell::new(RecentFiles::load(
                crate::paths::recent_files_path(),
            ))),
            reading: Rc::new(RefCell::new(ReadingPositions::load(
                &crate::paths::reading_positions_path(),
            ))),
        }
    }

    pub(crate) fn create_window(
        &mut self,
        target: &EventLoopWindowTarget<UserEvent>,
        settings: Settings,
        session: DocumentSession,
        geom: &WindowGeom,
    ) -> Result<App, String> {
        let strings = Strings::for_lang(detect_lang());
        let theme = settings.theme;
        let mut builder = WindowBuilder::new()
            .with_title("MD Previewer")
            .with_inner_size(LogicalSize::new(geom.w, geom.h))
            .with_position(LogicalPosition::new(geom.x, geom.y))
            .with_theme(theme.tao_theme());
        if let Some(icon) = load_window_icon() {
            builder = builder.with_window_icon(Some(icon));
        }
        let window = builder.build(target).map_err(|e| e.to_string())?;
        let id = window.id();
        let initial_page = startup_page(&strings, self.recent.borrow().paths());
        let navigation_proxy = self.proxy.clone();
        let ipc_proxy = self.proxy.clone();
        let drop_proxy = self.proxy.clone();
        let initial_page_loaded = std::cell::Cell::new(false);
        let builder = WebViewBuilder::with_web_context(&mut self.context)
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
                            eprintln!("无法打开链接 {url}: {error}");
                        }
                        false
                    }
                    Navigation::OpenDocument(path) => {
                        let _ = navigation_proxy.send_event(UserEvent::ForWindow(
                            id,
                            Box::new(UserEvent::OpenPaths(vec![path], false)),
                        ));
                        false
                    }
                }
            })
            .with_ipc_handler(move |msg| {
                let _ = ipc_proxy.send_event(UserEvent::ForWindow(
                    id,
                    Box::new(UserEvent::Ipc(msg.body().to_string())),
                ));
            })
            .with_drag_drop_handler(move |event| {
                if let wry::DragDropEvent::Drop { paths, .. } = event {
                    let _ = drop_proxy.send_event(UserEvent::ForWindow(
                        id,
                        Box::new(UserEvent::OpenPaths(paths, false)),
                    ));
                }
                true
            });
        #[cfg(target_os = "windows")]
        let builder = {
            use wry::WebViewBuilderExtWindows;
            builder.with_theme(theme.wry_theme())
        };
        #[cfg(target_os = "linux")]
        let webview = {
            use tao::platform::unix::WindowExtUnix;
            use wry::WebViewBuilderExtUnix;
            builder.build_gtk(window.default_vbox().ok_or("无法获取 GTK 窗口容器")?)
        };
        #[cfg(not(target_os = "linux"))]
        let webview = builder.build(&window);
        let webview = webview.map_err(|e| e.to_string())?;
        let sidebar_open_applied = settings.sidebar_open;
        Ok(App {
            webview,
            window,
            proxy: self.proxy.clone(),
            strings,
            settings,
            session,
            recent: Rc::clone(&self.recent),
            reading: Rc::clone(&self.reading),
            enhance_flags: EnhanceFlags::default(),
            loaded_enhancers: EnhanceFlags::default(),
            watcher: None,
            last_self_write: Arc::new(Mutex::new(None)),
            hljs_bootstrap: format!(
                "(function(){{{hljs};{extra};try{{window.hljs=hljs;}}catch(e){{}}if(typeof hljs!=='undefined'&&hljs.highlightAll){{hljs.highlightAll();}}}})();",
                hljs = hljs_js(), extra = HLJS_EXTRA_LANGS,
            ),
            pending_window_close: false,
            warned_external_change: None,
            pending_external_change: None,
            sidebar_open_applied,
            theme_applied: theme,
            update_downloading: false,
            bench_started: None,
            ready: false,
        })
    }
}
