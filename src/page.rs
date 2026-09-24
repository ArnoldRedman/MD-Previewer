// 预览页组装：把模板、样式和文案拼成一份完整 HTML
use crate::assets::{HLJS_DARK, HLJS_LIGHT, PAGE_CSS, PAGE_JS, PAGE_TEMPLATE, PREVIEW_ENHANCE_JS};
use crate::escape::{html_escape_attr, html_escape_ta, html_escape_text};
use crate::i18n::Strings;
use crate::markdown::EnhanceFlags;
use crate::recent::MAX_RECENT_FILES;
use crate::window::SIDEBAR_WIDTH;
use std::path::PathBuf;

pub(crate) fn empty_preview_html(s: &Strings, recent_files: &[PathBuf]) -> String {
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

/// 单次扫描替换模板里的 `{{name}}` 占位符
/// 未登记的占位符原样保留，模板和参数列表不一致时能在渲染结果里直接看出来
fn render_template(template: &str, vars: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len() + 8 * 1024);
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let key = &after[..end];
        match vars.iter().find(|(name, _)| *name == key) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str(&rest[start..start + 2 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// 首屏只送一个不含文档内容的空壳页
/// WebView2 的 NavigateToString 有 2MiB 上限，文档正文和原文一律等页面就绪后走 __setContent 推入，
/// 否则大文档会让 webview 创建失败，应用直接打不开
pub(crate) fn startup_page(s: &Strings, recent_files: &[PathBuf]) -> String {
    build_page(
        &empty_preview_html(s, recent_files),
        "",
        None,
        EnhanceFlags::default(),
        s,
        true,
    )
}

pub(crate) fn build_page_with_encoding(
    preview_html: &str,
    raw_md: &str,
    base_href: Option<&str>,
    flags: EnhanceFlags,
    s: &Strings,
    empty: bool,
    initial_encoding: &str,
) -> String {
    let body_class = if empty { "empty" } else { "" };
    let initial_encoding = if initial_encoding.is_empty() {
        "UTF-8"
    } else {
        initial_encoding
    };
    let base_tag = base_href
        .map(|href| format!(r#"<base id="base-href" href="{}">"#, html_escape_attr(href)))
        .unwrap_or_else(|| r#"<base id="base-href">"#.to_string());
    let opt = |encoding: &str| {
        if initial_encoding == encoding {
            " active"
        } else {
            ""
        }
    };

    // 前端从 window.__mdPreviewerConfig 读文案，转义交给 serde_json，不再手工拼 JS 字符串
    let config = serde_json::json!({
        "btnEditJs": s.btn_edit,
        "btnPreviewJs": s.btn_preview,
        "codeCopyJs": s.code_copy,
        "codeCopiedJs": s.code_copied,
        "sidebarOutlineEmptyJs": s.sidebar_outline_empty,
        "sidebarEmptyJs": s.sidebar_empty,
        "statWordsJs": s.stat_words,
        "statCharsJs": s.stat_chars,
        "copyTitleLineJs": s.copy_title_line,
        "copyTitleJs": s.copy_title,
        "copyBodyJs": s.copy_body,
        "copiedJs": s.copied,
        "cargoVersion": env!("CARGO_PKG_VERSION"),
        "updateStatusCheckingJs": s.update_status_checking,
        "updateStatusLatestJs": s.update_status_latest,
        "updateStatusFailedJs": s.update_status_failed,
        "updateDownloadingJs": s.update_downloading,
        "updateInstallingJs": s.update_installing,
        "updateFailedJs": s.update_failed,
        "btnUpdateTextJs": s.btn_update_text,
        "btnCheckUpdateJs": s.btn_check_update,
    })
    .to_string();

    let vars: Vec<(&str, String)> = vec![
        ("base_tag", base_tag.clone()),
        ("css_light", HLJS_LIGHT.to_string()),
        ("css_dark", HLJS_DARK.to_string()),
        ("sidebar_width", SIDEBAR_WIDTH.to_string()),
        ("sidebar_toggle_left", (SIDEBAR_WIDTH + 12.0).to_string()),
        ("css", PAGE_CSS.to_string()),
        ("body_class", body_class.to_string()),
        ("encoding_title", s.encoding_title.to_string()),
        ("initial_encoding", initial_encoding.to_string()),
        ("encoding_reopen_group", s.encoding_reopen_group.to_string()),
        ("opt_utf8", opt("UTF-8").to_string()),
        ("opt_utf8bom", opt("UTF-8 BOM").to_string()),
        ("opt_gbk", opt("GBK").to_string()),
        ("opt_u16le", opt("UTF-16 LE").to_string()),
        ("opt_u16be", opt("UTF-16 BE").to_string()),
        (
            "encoding_convert_group",
            s.encoding_convert_group.to_string(),
        ),
        ("btn_save_as", s.btn_save_as.to_string()),
        ("btn_new", s.btn_new.to_string()),
        ("btn_sidebar", s.btn_sidebar.to_string()),
        ("sidebar_folder", s.sidebar_folder.to_string()),
        ("sidebar_recent", s.sidebar_recent.to_string()),
        ("sidebar_outline", s.sidebar_outline.to_string()),
        ("recent_clear_all", s.recent_clear_all.to_string()),
        ("btn_open", s.btn_open.to_string()),
        ("btn_search", s.btn_search.to_string()),
        ("btn_edit", s.btn_edit.to_string()),
        ("btn_split", s.btn_split.to_string()),
        ("btn_print", s.btn_print.to_string()),
        ("btn_zoom", s.btn_zoom.to_string()),
        ("btn_zoom_out", s.btn_zoom_out.to_string()),
        ("btn_zoom_reset", s.btn_zoom_reset.to_string()),
        ("btn_zoom_in", s.btn_zoom_in.to_string()),
        ("btn_update_title", s.btn_update_title.to_string()),
        ("btn_update_text", s.btn_update_text.to_string()),
        ("btn_settings", s.btn_settings.to_string()),
        ("set_open_mode", s.set_open_mode.to_string()),
        ("set_open_tab", s.set_open_tab.to_string()),
        ("set_open_window", s.set_open_window.to_string()),
        ("set_author_mode", s.set_author_mode.to_string()),
        ("author_help", s.author_help.to_string()),
        ("set_author_off", s.set_author_off.to_string()),
        ("set_author_on", s.set_author_on.to_string()),
        ("set_tab_mode", s.set_tab_mode.to_string()),
        ("set_tab_keep", s.set_tab_keep.to_string()),
        ("set_tab_single", s.set_tab_single.to_string()),
        ("set_word_wrap", s.set_word_wrap.to_string()),
        ("set_wrap_on", s.set_wrap_on.to_string()),
        ("set_wrap_off", s.set_wrap_off.to_string()),
        ("set_theme", s.set_theme.to_string()),
        ("set_theme_system", s.set_theme_system.to_string()),
        ("set_theme_light", s.set_theme_light.to_string()),
        ("set_theme_dark", s.set_theme_dark.to_string()),
        ("set_version_label", s.set_version_label.to_string()),
        ("cargo_version", env!("CARGO_PKG_VERSION").to_string()),
        ("btn_check_update", s.btn_check_update.to_string()),
        ("set_title", s.set_title.to_string()),
        ("set_tab_general", s.set_tab_general.to_string()),
        ("set_tab_shortcuts", s.set_tab_shortcuts.to_string()),
        ("set_all_shortcuts", s.set_all_shortcuts.to_string()),
        ("set_shortcuts_enabled", s.set_shortcuts_enabled.to_string()),
        (
            "set_shortcuts_disabled",
            s.set_shortcuts_disabled.to_string(),
        ),
        ("set_log_label", s.set_log_label.to_string()),
        ("btn_open_log", s.btn_open_log.to_string()),
        ("btn_clear_log", s.btn_clear_log.to_string()),
        ("shortcut_close_tab", s.shortcut_close_tab.to_string()),
        ("shortcut_new_file", s.shortcut_new_file.to_string()),
        ("shortcut_open_file", s.shortcut_open_file.to_string()),
        ("shortcut_save", s.shortcut_save.to_string()),
        ("shortcut_save_as", s.shortcut_save_as.to_string()),
        ("shortcut_toggle_edit", s.shortcut_toggle_edit.to_string()),
        ("shortcut_split_view", s.shortcut_split_view.to_string()),
        ("shortcut_find", s.shortcut_find.to_string()),
        ("shortcut_refresh", s.shortcut_refresh.to_string()),
        ("shortcut_zoom_in", s.shortcut_zoom_in.to_string()),
        ("shortcut_zoom_out", s.shortcut_zoom_out.to_string()),
        ("shortcut_zoom_reset", s.shortcut_zoom_reset.to_string()),
        ("shortcut_print", s.shortcut_print.to_string()),
        ("shortcut_escape", s.shortcut_escape.to_string()),
        ("search_placeholder", s.search_placeholder.to_string()),
        ("tab_menu_close", s.tab_menu_close.to_string()),
        ("tab_menu_close_others", s.tab_menu_close_others.to_string()),
        ("tab_menu_copy_path", s.tab_menu_copy_path.to_string()),
        ("tab_menu_reveal", s.tab_menu_reveal.to_string()),
        ("recent_menu_remove", s.recent_menu_remove.to_string()),
        ("update_dialog_title", s.update_dialog_title.to_string()),
        ("update_close", s.update_close.to_string()),
        ("btn_do_update", s.btn_do_update.to_string()),
        ("btn_view_release", s.btn_view_release.to_string()),
        ("btn_dismiss", s.btn_dismiss.to_string()),
        ("preview_html", preview_html.to_string()),
        ("raw_md_escaped", html_escape_ta(raw_md)),
        ("config_json", config),
        ("js", PAGE_JS.to_string()),
        ("needs_math", flags.math.to_string()),
        ("needs_mermaid", flags.mermaid.to_string()),
        ("preview_enhance_js", PREVIEW_ENHANCE_JS.to_string()),
    ];
    render_template(PAGE_TEMPLATE, &vars)
}

pub(crate) fn build_page(
    preview_html: &str,
    raw_md: &str,
    base_href: Option<&str>,
    flags: EnhanceFlags,
    s: &Strings,
    empty: bool,
) -> String {
    build_page_with_encoding(preview_html, raw_md, base_href, flags, s, empty, "UTF-8")
}
