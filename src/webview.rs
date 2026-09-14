// WebView 导航策略：哪些地址可以留在页面内
use crate::paths::local_document_path_from_url;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Navigation {
    Allow,
    Block,
    OpenExternally,
    OpenDocument(PathBuf),
}

/// 页面内导航的处理决定。首屏由 with_html 载入，在 WebView2 里表现为一次 data:text/html 导航，
/// 只放行这第一次；之后再出现的 data:/blob:/javascript: 导航都不是应用自己发起的，一律拦下。
/// 外部链接交给系统浏览器，本地文档进标签页，file: 导航会离开当前页面所以也拦下
pub(crate) fn navigation_decision(url: &str, initial_page_loaded: &mut bool) -> Navigation {
    if is_script_bearing_url(url) {
        if *initial_page_loaded {
            return Navigation::Block;
        }
        *initial_page_loaded = true;
        return Navigation::Allow;
    }
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:") {
        return Navigation::OpenExternally;
    }
    if let Some(path) = local_document_path_from_url(url) {
        return Navigation::OpenDocument(path);
    }
    if url.starts_with("file:") {
        return Navigation::Block;
    }
    Navigation::Allow
}

pub(crate) fn is_script_bearing_url(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    lower.starts_with("data:")
        || lower.starts_with("blob:")
        || lower.starts_with("javascript:")
        || lower.starts_with("vbscript:")
}
