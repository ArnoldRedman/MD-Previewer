// 主题切换：系统深浅色跟随与手动覆盖
use crate::settings::ThemeChoice;
use tao::window::{Theme, Window};
use wry::WebView;

impl ThemeChoice {
    pub(crate) fn tao_theme(self) -> Option<Theme> {
        match self {
            ThemeChoice::System => None,
            ThemeChoice::Light => Some(Theme::Light),
            ThemeChoice::Dark => Some(Theme::Dark),
        }
    }

    /// WebView2 的 prefers-color-scheme 只跟系统走，不跟窗口标题栏走，
    /// 手动指定主题必须单独压到 WebView 上，页面里的深色样式才会切换
    #[cfg(target_os = "windows")]
    pub(crate) fn wry_theme(self) -> wry::Theme {
        match self {
            ThemeChoice::System => wry::Theme::Auto,
            ThemeChoice::Light => wry::Theme::Light,
            ThemeChoice::Dark => wry::Theme::Dark,
        }
    }
}

/// 同时把主题压到窗口和 WebView 上。macOS 与 Linux 的 WebView 跟随窗口外观，
/// 只有 Windows 需要额外设置 WebView2 的首选配色
pub(crate) fn apply_theme(window: &Window, webview: &WebView, theme: ThemeChoice) {
    window.set_theme(theme.tao_theme());
    #[cfg(target_os = "windows")]
    {
        use wry::WebViewExtWindows;
        if let Err(error) = webview.set_theme(theme.wry_theme()) {
            eprintln!("Could not apply webview theme: {error}");
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = webview;
}
