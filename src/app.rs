// 应用主体：事件循环状态机与 IPC 分发
use crate::document::{
    confirm_lossy_conversion, confirm_overwrite, document_to_html, encode_document,
    read_document_with_encoding, save_as_target_path, save_document_text,
    self_write_still_matches_disk, should_protect_external_change, write_document_bytes,
    SelfWriteRecord,
};
use crate::escape::escape_js;
use crate::finder::{create_finder_file, normalize_new_markdown_path, open_terminal};
use crate::i18n::Strings;
use crate::ipc::{parse_finder_action, parse_ipc_message, FinderAction, IpcMessage, TabAction};
use crate::markdown::{build_enhancer_bootstrap, md_to_html, EnhanceFlags};
use crate::page::empty_preview_html;
use crate::paths::{
    is_markdown_document, is_supported_document, local_document_path_from_url, session_path,
    supported_dialog_extensions, MARKDOWN_EXTENSIONS, TEXT_EXTENSIONS,
};
use crate::platform::reveal_in_file_manager;
use crate::recent::RecentFiles;
use crate::session::DocumentSession;
use crate::settings::{Settings, TabMode, ThemeChoice};
use crate::sidebar::{author_doc, missing_preview_html, sidebar_json, tabs_json};
use crate::theme::apply_theme;
// 只有 Windows 分支会调用更新下载
#[cfg(target_os = "windows")]
use crate::updater::windows_updater;
use crate::watch::{event_should_reload_file, watch_scope_for_file};
use crate::window::{resize_for_sidebar, save_window_geom, settings_path, show_warning_dialog};
use crate::UserEvent;
use notify::{Event, RecursiveMode, Watcher};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tao::event::Event as TaoEvent;
use tao::event::WindowEvent;
use tao::event_loop::{ControlFlow, EventLoopProxy};
use tao::window::Window;
use wry::WebView;

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

/// 页面发来的 IPC 消息。字符串协议只在这里解析一次，事件循环按类型分发
fn document_picker() -> rfd::FileDialog {
    let supported = supported_dialog_extensions();
    rfd::FileDialog::new()
        .add_filter("Supported Documents", supported.as_slice())
        .add_filter("Markdown", MARKDOWN_EXTENSIONS)
        .add_filter("Text", TEXT_EXTENSIONS)
        .add_filter("All Files", &["*"])
}

/// 事件循环持有的全部状态。页面回调只把消息转成 `UserEvent::Ipc`，
/// 所有状态改动都在事件循环里串行发生，不需要跨线程共享；
/// 只有文件监听线程要读的自写记录仍用 Arc<Mutex>
pub(crate) struct App {
    pub(crate) webview: WebView,
    pub(crate) window: Window,
    pub(crate) proxy: EventLoopProxy<UserEvent>,
    pub(crate) strings: Strings,
    pub(crate) settings: Settings,
    pub(crate) session: DocumentSession,
    pub(crate) recent: RecentFiles,
    pub(crate) enhance_flags: EnhanceFlags,
    pub(crate) loaded_enhancers: EnhanceFlags,
    pub(crate) watcher: Option<notify::RecommendedWatcher>,
    pub(crate) last_self_write: Arc<Mutex<Option<SelfWriteRecord>>>,
    /// hljs 不进首屏 HTML，页面报告就绪后再注入，缩短冷启动首屏解析路径
    pub(crate) hljs_bootstrap: String,
    pub(crate) pending_window_close: bool,
    pub(crate) warned_external_change: Option<PathBuf>,
    pub(crate) pending_external_change: Option<PathBuf>,
    pub(crate) sidebar_open_applied: bool,
    /// 已压到窗口和 WebView 上的主题，设置变化时只在真的不同才重新应用
    pub(crate) theme_applied: ThemeChoice,
    /// 更新包下载线程是否在跑；重复点「立即更新」不再起第二个下载
    pub(crate) update_downloading: bool,
    /// MD_PREVIEWER_BENCH=1 时记录启动时刻，首屏就绪即打印耗时并退出
    pub(crate) bench_started: Option<Instant>,
}

impl App {
    pub(crate) fn eval(&self, script: &str) {
        if let Err(error) = self.webview.evaluate_script(script) {
            crate::logger::write_log("WARN", &format!("Failed to evaluate script: {error}"));
        }
    }

    // 新窗口模式是多进程、单标签模式按设计不恢复，这两种情况下落盘只会互相覆盖或把老标签带回来
    pub(crate) fn persist_session(&self) {
        if !self.settings.keeps_session() {
            return;
        }
        if let Err(error) = self.session.save(&session_path()) {
            eprintln!("Could not save tab session: {error}");
        }
    }

    pub(crate) fn refresh_tabs(&self) {
        update_tabs(&self.webview, &self.session);
        update_window_title(&self.window, &self.session);
    }

    pub(crate) fn refresh_sidebar(&self) {
        update_sidebar(&self.webview, &self.session, self.recent.paths());
    }

    pub(crate) fn push_empty_preview(&self) {
        let html = empty_preview_html(&self.strings, self.recent.paths());
        self.eval(&format!(
            "if(window.__setEmptyPreview)window.__setEmptyPreview('{}');",
            escape_js(&html)
        ));
    }

    // 空白启动页上也列着最近文件，只有没有活动文档时才重画它；有文档打开时重画会把正文整个换成空白页
    pub(crate) fn on_recent_changed(&self) {
        if self.session.active().is_none() {
            self.push_empty_preview();
        }
        self.refresh_sidebar();
    }

    // 纯文本文档没有标题结构，作者模式的复制按钮不该出现
    pub(crate) fn push_author_doc(&self, path: &Path, raw: &str) {
        let raw = if is_markdown_document(path) { raw } else { "" };
        update_author_doc(&self.webview, raw);
    }

    pub(crate) fn bootstrap_enhancers(&mut self, flags: EnhanceFlags) {
        for script in build_enhancer_bootstrap(flags, self.loaded_enhancers) {
            self.eval(&script);
        }
        self.loaded_enhancers.math |= flags.math;
        self.loaded_enhancers.mermaid |= flags.mermaid;
    }

    /// 首屏不含文档内容，页面就绪后由 render_active 统一推入
    pub(crate) fn push_ui_state(&self) {
        self.refresh_tabs();
        self.refresh_sidebar();
        update_settings_ui(&self.webview, &self.settings);
    }

    /// 切换活动文档后的固定流程：落盘会话、重绘、把文件监听挪到新文档上
    pub(crate) fn show_active(&mut self) {
        self.persist_session();
        self.render_active();
        self.install_watcher();
    }

    pub(crate) fn install_watcher(&mut self) {
        self.watcher = None;
        let Some(path) = self.session.active().map(|tab| tab.path.clone()) else {
            return;
        };
        let scope = watch_scope_for_file(&path).to_path_buf();
        if !scope.exists() {
            return;
        }
        let callback_path = path.clone();
        let proxy = self.proxy.clone();
        let last_self_write = Arc::clone(&self.last_self_write);
        let watcher = notify::recommended_watcher(move |result: Result<Event, _>| {
            let Ok(event) = result else {
                return;
            };
            if !event_should_reload_file(&event, &callback_path) {
                return;
            }
            // 只在锁内拷贝记录；比对磁盘内容要读文件，不能让保存路径等在这把锁上
            let record = last_self_write.lock().unwrap().clone();
            if !self_write_still_matches_disk(record.as_ref(), &callback_path) {
                let _ = proxy.send_event(UserEvent::FileChanged(callback_path.clone()));
            }
        });
        let mut watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                eprintln!("无法创建文件监听，外部修改将不会自动刷新: {error}");
                return;
            }
        };
        match watcher.watch(&scope, RecursiveMode::NonRecursive) {
            Ok(()) => self.watcher = Some(watcher),
            Err(error) => eprintln!("无法监听目录 {}: {error}", scope.display()),
        }
    }

    pub(crate) fn render_active(&mut self) {
        let Some(active) = self.session.active().cloned() else {
            self.push_empty_preview();
            update_author_doc(&self.webview, "");
            self.refresh_tabs();
            self.refresh_sidebar();
            return;
        };

        match read_document_with_encoding(&active.path, active.encoding.as_deref()) {
            Ok((raw, resolved_encoding)) => {
                if let Some(tab) = self.session.get_mut(active.id) {
                    tab.missing = false;
                    if tab.encoding.is_none() {
                        tab.encoding = Some(resolved_encoding.to_string());
                    }
                }
                self.recent.remember(&active.path);
                let (html, flags, base_href) = document_to_html(&active.path, &raw);
                self.enhance_flags = flags;
                self.eval(&format!(
                    "if(window.__setContent)window.__setContent('{}', '{}', '{}', {}, {});if(window.__setEncoding)window.__setEncoding('{}');",
                    escape_js(&html),
                    escape_js(&raw),
                    escape_js(&base_href.unwrap_or_default()),
                    flags.math,
                    flags.mermaid,
                    escape_js(resolved_encoding)
                ));
                self.push_author_doc(&active.path, &raw);
                self.bootstrap_enhancers(flags);
                if active.edit_on_open {
                    if let Some(tab) = self.session.get_mut(active.id) {
                        tab.edit_on_open = false;
                    }
                    self.eval("if(window.__mdPreviewerEnterEdit)window.__mdPreviewerEnterEdit();");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(tab) = self.session.get_mut(active.id) {
                    tab.missing = true;
                }
                if active.dirty {
                    show_warning_dialog(
                        self.strings.missing_title,
                        "The file disappeared while it still has unsaved edits. The editor content has been kept.",
                    );
                } else {
                    let html = missing_preview_html(active.id, &active.path, &self.strings);
                    self.eval(&format!(
                        "if(window.__setMissing)window.__setMissing('{}');",
                        escape_js(&html)
                    ));
                }
            }
            Err(error) => {
                show_warning_dialog(self.strings.cannot_read, &error.to_string());
            }
        }

        self.refresh_tabs();
        self.refresh_sidebar();
    }

    /// 把页面正文按活动标签的编码写回磁盘；成功后刷新预览并清脏标记，失败弹窗并取消待关窗
    pub(crate) fn save_active_content(&mut self, content: &str) -> bool {
        let Some((path, encoding)) = self
            .session
            .active()
            .map(|tab| (tab.path.clone(), tab.encoding.clone()))
        else {
            return false;
        };
        match save_document_text(&self.last_self_write, &path, content, encoding.as_deref()) {
            Ok(()) => {
                self.on_file_saved(path);
                true
            }
            Err(error) => {
                crate::logger::write_log(
                    "ERROR",
                    &format!("Could not save {}: {error}", path.display()),
                );
                self.pending_window_close = false;
                show_warning_dialog("Could Not Save", &format!("{}: {error}", path.display()));
                false
            }
        }
    }

    // 自己的保存只刷新预览，编辑框和光标保持不动
    pub(crate) fn on_file_saved(&mut self, path: PathBuf) {
        if self.warned_external_change.as_ref() == Some(&path) {
            self.warned_external_change = None;
        }
        let active_matches =
            self.session.active().map(|tab| tab.path.as_path()) == Some(path.as_path());
        self.session.mark_saved(&path);
        if active_matches {
            let encoding = self.session.active().and_then(|tab| tab.encoding.clone());
            match read_document_with_encoding(&path, encoding.as_deref()) {
                Ok((raw, _)) => {
                    let (html, flags, _) = document_to_html(&path, &raw);
                    self.enhance_flags = flags;
                    self.eval(&format!(
                        "if(window.__setPreview)window.__setPreview('{}', {}, {});if(window.__markSaved)window.__markSaved('{}');",
                        escape_js(&html),
                        flags.math,
                        flags.mermaid,
                        escape_js(&raw)
                    ));
                    self.push_author_doc(&path, &raw);
                    self.bootstrap_enhancers(flags);
                }
                Err(error) => {
                    crate::logger::write_log(
                        "ERROR",
                        &format!("Cannot read {}: {error}", path.display()),
                    );
                    show_warning_dialog(
                        self.strings.cannot_read,
                        &format!("{}: {error}", path.display()),
                    );
                }
            }
        }
        self.persist_session();
        self.refresh_tabs();
    }

    pub(crate) fn exit(&self, control_flow: &mut ControlFlow) {
        save_window_geom(&self.window);
        self.persist_session();
        *control_flow = ControlFlow::Exit;
    }

    /// 关窗前先让页面把未保存正文回写；页面没有脏内容时会回 save-skipped，随后再真正退出
    pub(crate) fn request_close(&mut self, control_flow: &mut ControlFlow) {
        if self.pending_window_close {
            return;
        }
        if self.session.active_is_dirty() {
            self.pending_window_close = true;
            self.eval("if(window.__mdPreviewerSave)window.__mdPreviewerSave();");
            return;
        }
        self.exit(control_flow);
    }

    pub(crate) fn open_paths(&mut self, paths: Vec<PathBuf>, edit_on_open: bool) {
        let paths = paths
            .into_iter()
            .filter(|path| is_supported_document(path))
            .collect::<Vec<_>>();
        self.open_checked_paths(paths, edit_on_open);
    }

    /// 调用方已确认路径可打开时直接走这里，避免再嗅探一次文件内容
    pub(crate) fn open_checked_paths(&mut self, paths: Vec<PathBuf>, edit_on_open: bool) {
        self.window.set_minimized(false);
        self.window.set_visible(true);
        self.window.set_focus();
        if paths.is_empty() {
            return;
        }
        let previous_active = self.session.active_id;
        // 活动标签有未保存内容时，新文件在后台打开，不打断正在进行的编辑
        let keep_active = self.session.active_is_dirty();
        for path in paths {
            self.session.open(path, edit_on_open);
        }
        if keep_active {
            if let Some(id) = previous_active {
                self.session.activate(id);
            }
            self.persist_session();
            self.refresh_tabs();
            return;
        }
        self.show_active();
    }

    pub(crate) fn activate_tab(&mut self, id: u64) {
        if !self.session.activate(id) {
            return;
        }
        // 单标签模式下脏标签会被暂时保留，切换成功说明它已经回写磁盘，这时收敛回只剩当前一个
        if self.session.single_tab {
            self.session.collapse_to_active();
        }
        self.show_active();
    }

    pub(crate) fn close_tab(&mut self, id: u64) {
        let was_active = self.session.active_id == Some(id);
        if !self.session.close(id) {
            return;
        }
        if was_active {
            self.show_active();
        } else {
            self.persist_session();
            self.refresh_tabs();
        }
    }

    pub(crate) fn close_others(&mut self, id: u64) {
        let was_active = self.session.active_id == Some(id);
        if !self.session.close_others(id) {
            return;
        }
        if was_active {
            self.persist_session();
            self.refresh_tabs();
        } else {
            self.show_active();
        }
    }

    pub(crate) fn new_file(&mut self) {
        if self.session.active_is_dirty() {
            self.eval("if(window.__mdPreviewerNewFile)window.__mdPreviewerNewFile();");
            return;
        }
        let current_dir = self
            .session
            .active()
            .and_then(|tab| tab.path.parent().map(Path::to_path_buf));
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Markdown", MARKDOWN_EXTENSIONS)
            .set_file_name(self.strings.new_filename);
        if let Some(current_dir) = current_dir {
            dialog = dialog.set_directory(current_dir);
        }
        let Some(chosen) = dialog.save_file() else {
            return;
        };
        let path = normalize_new_markdown_path(chosen.clone());
        // 系统对话框只对用户输入的文件名做过覆盖确认；补上 .md 后撞到别的文件时必须再问一次
        if path != chosen && path.exists() && !confirm_overwrite(&self.strings, &path) {
            return;
        }
        match fs::write(&path, "") {
            Ok(()) => self.open_paths(vec![path], true),
            Err(error) => show_warning_dialog("Could Not Create File", &error.to_string()),
        }
    }

    pub(crate) fn open_file(&mut self) {
        if self.session.active_is_dirty() {
            self.eval("if(window.__mdPreviewerOpenFile)window.__mdPreviewerOpenFile();");
            return;
        }
        if let Some(paths) = document_picker().pick_files() {
            self.open_paths(paths, false);
        }
    }

    pub(crate) fn locate_tab(&mut self, id: u64) {
        let Some(path) = document_picker().pick_file() else {
            return;
        };
        if self.session.relocate(id, path) && self.session.activate(id) {
            self.show_active();
        } else {
            show_warning_dialog("Already Open", "That file is already open in another tab.");
        }
    }

    pub(crate) fn reveal_tab(&self, id: u64) {
        if let Some(tab) = self.session.tabs.iter().find(|tab| tab.id == id) {
            reveal_in_file_manager(&tab.path);
        }
    }

    pub(crate) fn render_preview(&self, content: &str) {
        let Some(path) = self.session.active().map(|tab| tab.path.clone()) else {
            return;
        };
        let (html, flags, _) = document_to_html(&path, content);
        self.eval(&format!(
            "if(window.__setLivePreview)window.__setLivePreview('{}', {}, {});",
            escape_js(&html),
            flags.math,
            flags.mermaid
        ));
    }

    pub(crate) fn on_file_changed(&mut self, path: PathBuf) {
        if self.session.active().map(|tab| tab.path.as_path()) != Some(path.as_path()) {
            return;
        }
        self.pending_external_change = Some(path);
        self.eval(
            "if(window.__mdPreviewerResolveExternalChange)window.__mdPreviewerResolveExternalChange();",
        );
    }

    pub(crate) fn on_external_change_resolved(&mut self, webview_dirty: bool) {
        let Some(path) = self.pending_external_change.take() else {
            return;
        };
        if self.session.active().map(|tab| tab.path.as_path()) != Some(path.as_path()) {
            return;
        }
        if should_protect_external_change(webview_dirty, self.session.active_is_dirty()) {
            self.session.set_active_dirty(true);
            self.eval("if(window.__mdPreviewerPauseAutosave)window.__mdPreviewerPauseAutosave();");
            self.refresh_tabs();
            if self.warned_external_change.as_ref() != Some(&path) {
                show_warning_dialog(
                    "File Changed on Disk",
                    "Automatic saving is paused and your edits are still in the editor. Press Cmd/Ctrl+S to replace the disk version, or reopen the file to keep the external version.",
                );
                self.warned_external_change = Some(path);
            }
            return;
        }
        self.render_active();
        self.persist_session();
    }

    // 页面切换编码前会先保存脏内容；若保存失败标签仍是脏的，此时重读磁盘会丢掉编辑，直接忽略
    pub(crate) fn set_encoding(&mut self, encoding: String) {
        if self.session.active_is_dirty() {
            return;
        }
        let Some(tab) = self.session.active_mut() else {
            return;
        };
        tab.encoding = Some(encoding);
        self.render_active();
        self.persist_session();
    }

    pub(crate) fn refresh_active(&mut self) {
        if self.session.active_is_dirty() {
            return;
        }
        self.render_active();
    }

    pub(crate) fn convert_encoding(&mut self, encoding: String, content: &str) {
        let Some(path) = self.session.active().map(|tab| tab.path.clone()) else {
            return;
        };
        let encoded = match encode_document(content, &encoding) {
            Ok(encoded) => encoded,
            Err(message) => {
                show_warning_dialog(self.strings.convert_failed_title, &message);
                return;
            }
        };
        if !confirm_lossy_conversion(&self.strings, &encoding, encoded.lossy_chars) {
            return;
        }
        if let Err(error) = write_document_bytes(&self.last_self_write, &path, encoded.bytes) {
            show_warning_dialog(
                self.strings.convert_failed_title,
                &format!("{}: {error}", path.display()),
            );
            return;
        }
        if let Some(tab) = self.session.active_mut() {
            tab.encoding = Some(encoding);
        }
        self.session.mark_saved(&path);
        self.persist_session();
        // 先清前端脏标记，再按新编码从磁盘重读，编辑器才会显示磁盘实际内容（有损时可见 ?）
        self.eval("if(window.__markSaved)window.__markSaved();");
        self.render_active();
    }

    pub(crate) fn save_as(&mut self, content: &str) {
        let Some((id, current_path, encoding)) = self
            .session
            .active()
            .map(|tab| (tab.id, tab.path.clone(), tab.encoding.clone()))
        else {
            return;
        };
        let mut dialog = rfd::FileDialog::new()
            .set_title(self.strings.save_as_dialog_title)
            .add_filter("Markdown", MARKDOWN_EXTENSIONS)
            .add_filter("Text", TEXT_EXTENSIONS)
            .add_filter("All Files", &["*"]);
        if let Some(dir) = current_path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
        {
            dialog = dialog.set_directory(dir);
        }
        if let Some(name) = current_path.file_name().and_then(|name| name.to_str()) {
            dialog = dialog.set_file_name(name);
        }
        let Some(target) = dialog.save_file() else {
            return;
        };
        let target = save_as_target_path(target, &current_path);
        if self.session.is_open_in_other_tab(id, &target) {
            show_warning_dialog(
                self.strings.save_as_failed_title,
                self.strings.save_as_already_open,
            );
            return;
        }
        let encoding = encoding.unwrap_or_else(|| "UTF-8".to_string());
        let encoded = match encode_document(content, &encoding) {
            Ok(encoded) => encoded,
            Err(message) => {
                show_warning_dialog(self.strings.save_as_failed_title, &message);
                return;
            }
        };
        if !confirm_lossy_conversion(&self.strings, &encoding, encoded.lossy_chars) {
            return;
        }
        if let Err(error) = write_document_bytes(&self.last_self_write, &target, encoded.bytes) {
            show_warning_dialog(
                self.strings.save_as_failed_title,
                &format!("{}: {error}", target.display()),
            );
            return;
        }
        // 目标已排除其他标签占用，relocate 只会因标签不存在而失败；
        // relocate 会清掉编码，另存为刚按这个编码写过盘，必须设回去
        if self.session.relocate(id, target.clone()) {
            if let Some(tab) = self.session.get_mut(id) {
                tab.encoding = Some(encoding);
                tab.dirty = false;
                tab.missing = false;
            }
        }
        self.eval("if(window.__markSaved)window.__markSaved();");
        self.show_active();
    }

    pub(crate) fn on_settings_changed(&mut self) {
        let current = self.settings.clone();
        if current.sidebar_open != self.sidebar_open_applied {
            resize_for_sidebar(&self.window, current.sidebar_open);
            self.sidebar_open_applied = current.sidebar_open;
        }
        if current.theme != self.theme_applied {
            apply_theme(&self.window, &self.webview, current.theme);
            self.theme_applied = current.theme;
        }
        if let Err(error) = current.save(&settings_path()) {
            eprintln!("Could not save settings: {error}");
        }
        self.session.single_tab = current.tab_mode == TabMode::Single;
        if self.session.single_tab {
            self.session.collapse_to_active();
        }
        if current.keeps_session() {
            self.persist_session();
        } else {
            // 留着旧会话文件会在下次启动又把老标签拉回来，正是这个设置要避免的
            if let Err(error) = fs::remove_file(session_path()) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("Could not remove tab session: {error}");
                }
            }
        }
        self.refresh_tabs();
        update_settings_ui(&self.webview, &current);
    }

    pub(crate) fn on_ready(&mut self, control_flow: &mut ControlFlow) {
        let hljs = std::mem::take(&mut self.hljs_bootstrap);
        if !hljs.is_empty() {
            self.eval(&hljs);
        }
        self.push_ui_state();
        // 首屏文档也走 __setContent：它不能进 with_html 那份 HTML，否则大文档会顶穿 WebView2 的 2MiB 上限
        // 作者模式原文、增强脚本和 --edit 的首屏进入编辑都在 render_active 里一起处理
        self.render_active();
        if let Some(started) = self.bench_started {
            eprintln!("[bench] +{}ms ready", started.elapsed().as_millis());
            *control_flow = ControlFlow::Exit;
        }
    }

    pub(crate) fn handle_ipc(&mut self, message: IpcMessage, control_flow: &mut ControlFlow) {
        match message {
            IpcMessage::NewFile => self.new_file(),
            IpcMessage::OpenFile => self.open_file(),
            IpcMessage::OpenRecent(index) => {
                let Some(path) = self.recent.get(index).cloned() else {
                    return;
                };
                if path.exists() {
                    self.open_paths(vec![path], false);
                } else if self.recent.forget(&path) {
                    self.on_recent_changed();
                }
            }
            IpcMessage::OpenDoc(path) => {
                if path.is_file() && is_supported_document(&path) {
                    self.open_checked_paths(vec![path], false);
                } else if self.recent.forget(&path) {
                    // 侧栏里点到已经不存在的历史条目，顺手把它从最近列表剔掉
                    self.on_recent_changed();
                }
            }
            IpcMessage::ForgetRecent(path) => {
                if self.recent.forget(&path) {
                    self.on_recent_changed();
                }
            }
            IpcMessage::ClearRecent => {
                if self.recent.clear() {
                    self.on_recent_changed();
                }
            }
            IpcMessage::RevealPath(path) => {
                if path.exists() {
                    reveal_in_file_manager(&path);
                } else if self.recent.forget(&path) {
                    // 历史条目对应的文件已经不在了，定位不到就直接从列表剔掉
                    self.on_recent_changed();
                }
            }
            IpcMessage::OpenLocalLink(url) => {
                if let Some(path) = local_document_path_from_url(&url) {
                    self.open_checked_paths(vec![path], false);
                }
            }
            IpcMessage::TabAction {
                action,
                id,
                content,
            } => {
                if let Some(content) = content {
                    if !self.save_active_content(&content) {
                        return;
                    }
                }
                match action {
                    TabAction::Activate => self.activate_tab(id),
                    TabAction::Close => self.close_tab(id),
                    TabAction::CloseOthers => self.close_others(id),
                }
            }
            IpcMessage::SetSetting { key, value } => {
                // 只有取值真的变了才落盘和重绘，重复点同一项不做事
                if self.settings.apply(&key, &value) {
                    self.on_settings_changed();
                }
            }
            IpcMessage::LocateTab(id) => self.locate_tab(id),
            IpcMessage::RevealTab(id) => self.reveal_tab(id),
            IpcMessage::RenderPreview(content) => self.render_preview(&content),
            IpcMessage::RenderReleaseNotes(markdown) => {
                // 发布说明不属于任何标签页，不走 document_to_html，也不解析本地图片路径
                self.eval(&format!(
                    "if(window.__setUpdateNotes)window.__setUpdateNotes('{}');",
                    escape_js(&md_to_html(&markdown))
                ));
            }
            IpcMessage::ConvertEncoding { encoding, content } => {
                self.convert_encoding(encoding, &content)
            }
            IpcMessage::SaveAs(content) => self.save_as(&content),
            IpcMessage::Save(content) => {
                if self.save_active_content(&content) && self.pending_window_close {
                    self.exit(control_flow);
                }
            }
            IpcMessage::SaveSkipped => {
                if self.pending_window_close {
                    self.exit(control_flow);
                }
            }
            IpcMessage::DirtyChanged(dirty) => {
                if self.session.set_active_dirty(dirty) {
                    self.refresh_tabs();
                }
            }
            IpcMessage::ExternalChangeResolved { dirty } => self.on_external_change_resolved(dirty),
            IpcMessage::Print => {
                if let Err(error) = self.webview.print() {
                    eprintln!("Could not print: {error}");
                }
            }
            IpcMessage::Ready => self.on_ready(control_flow),
            IpcMessage::Refresh => self.refresh_active(),
            IpcMessage::SetEncoding(encoding) => self.set_encoding(encoding),
            IpcMessage::SelfUpdate(download_url) => self.start_self_update(download_url),
            IpcMessage::LogError(msg) => {
                crate::logger::write_log("JS_ERROR", &msg);
            }
            IpcMessage::OpenLog => self.open_log(),
            IpcMessage::ClearLog => self.clear_log(),
        }
    }

    pub(crate) fn open_log(&self) {
        crate::logger::reveal_log();
    }

    pub(crate) fn clear_log(&self) {
        crate::logger::clear_log();
    }

    /// Windows 上在后台线程下载更新包并把进度推给页面；其他平台只打开下载页
    #[cfg(target_os = "windows")]
    pub(crate) fn start_self_update(&mut self, download_url: String) {
        if self.update_downloading {
            return;
        }
        self.update_downloading = true;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = windows_updater::download_update(&download_url, |downloaded, total| {
                let _ = proxy.send_event(UserEvent::UpdateProgress { downloaded, total });
            });
            let event = match result {
                Ok(path) => UserEvent::UpdateDownloaded(path),
                Err(error) => UserEvent::UpdateFailed(error),
            };
            let _ = proxy.send_event(event);
        });
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn start_self_update(&mut self, download_url: String) {
        if let Err(error) = open::that(&download_url) {
            eprintln!("Could not open {download_url}: {error}");
        }
    }

    pub(crate) fn handle_user_event(&mut self, event: UserEvent, control_flow: &mut ControlFlow) {
        match event {
            UserEvent::UpdateProgress { downloaded, total } => {
                let total = total.map_or("null".to_string(), |total| total.to_string());
                self.eval(&format!(
                    "if(window.__setUpdateProgress)window.__setUpdateProgress({downloaded},{total});"
                ));
            }
            UserEvent::UpdateDownloaded(path) => {
                self.update_downloading = false;
                #[cfg(target_os = "windows")]
                match windows_updater::launch_installer(&path) {
                    Ok(()) => {
                        // 接手脚本已在等本进程退出；走正常关窗流程，未保存的编辑会先写回
                        self.eval(
                            "if(window.__setUpdateInstalling)window.__setUpdateInstalling();",
                        );
                        self.request_close(control_flow);
                    }
                    Err(error) => {
                        let _ = fs::remove_file(&path);
                        self.eval(&format!(
                            "if(window.__setUpdateFailed)window.__setUpdateFailed('{}');",
                            escape_js(&error)
                        ));
                    }
                }
                #[cfg(not(target_os = "windows"))]
                let _ = path;
            }
            UserEvent::UpdateFailed(error) => {
                self.update_downloading = false;
                eprintln!("[update error] {error}");
                self.eval(&format!(
                    "if(window.__setUpdateFailed)window.__setUpdateFailed('{}');",
                    escape_js(&error)
                ));
            }
            UserEvent::Ipc(body) => {
                if let Some(message) = parse_ipc_message(&body) {
                    self.handle_ipc(message, control_flow);
                }
            }
            UserEvent::NewFile => self.new_file(),
            UserEvent::OpenFile => self.open_file(),
            UserEvent::OpenPaths(paths, edit_on_open) => self.open_paths(paths, edit_on_open),
            UserEvent::CloseActiveTab => {
                if self.session.active_id.is_some() {
                    self.eval(
                        "if(window.__mdPreviewerCloseActiveTab)window.__mdPreviewerCloseActiveTab();",
                    );
                } else {
                    self.exit(control_flow);
                }
            }
            UserEvent::FileChanged(path) => self.on_file_changed(path),
            UserEvent::ToggleEdit => {
                self.eval("if(window.__mdPreviewerToggleEdit)window.__mdPreviewerToggleEdit();");
            }
            UserEvent::ShowFind => {
                self.eval("if(window.__mdPreviewerShowFind)window.__mdPreviewerShowFind();");
            }
            UserEvent::Print => {
                if let Err(error) = self.webview.print() {
                    eprintln!("Could not print: {error}");
                }
            }
            UserEvent::SetTheme(choice) => {
                // macOS 菜单和设置面板走同一条落盘与应用路径
                if self.settings.apply("theme", choice.as_str()) {
                    self.on_settings_changed();
                }
            }
            UserEvent::OpenUrl(url) => {
                if let Err(error) = open::that(url) {
                    eprintln!("Could not open {url}: {error}");
                }
            }
            UserEvent::Quit => self.request_close(control_flow),
        }
    }

    // macOS: Finder file opens and embedded Finder Sync actions arrive here.
    pub(crate) fn handle_opened_urls(&mut self, urls: Vec<url::Url>) {
        let mut paths = Vec::new();
        for url in urls {
            if let Ok(path) = url.to_file_path() {
                if is_supported_document(&path) {
                    paths.push(path);
                }
                continue;
            }
            let Some(action) = parse_finder_action(url.as_str()) else {
                continue;
            };
            match action {
                FinderAction::Create { folder, kind } => match create_finder_file(&folder, &kind) {
                    Ok(path) if kind == "md" => self.open_paths(vec![path], true),
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
        if !paths.is_empty() {
            self.open_checked_paths(paths, false);
        }
    }

    pub(crate) fn handle(
        &mut self,
        event: TaoEvent<'_, UserEvent>,
        control_flow: &mut ControlFlow,
    ) {
        match event {
            TaoEvent::UserEvent(event) => self.handle_user_event(event, control_flow),
            TaoEvent::Opened { urls } => self.handle_opened_urls(urls),
            TaoEvent::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => self.request_close(control_flow),
            _ => {}
        }
    }
}
