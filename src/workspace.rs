// 窗口生命周期和标签交接集中在主线程，App 继续只处理一个窗口的文档
use crate::app::App;
use crate::desktop::Desktop;
use crate::document::read_document_with_encoding;
use crate::paths::session_path;
use crate::session::{normalize_path, DocumentSession};
use crate::settings::TabMode;
use crate::tab_transfer::{self, DragRequest, ReceiveReady, Snapshot, TabBarRect};
use crate::window::{show_warning_dialog, WindowGeom};
use crate::UserEvent;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tao::dpi::PhysicalPosition;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopWindowTarget};
use tao::window::WindowId;

#[derive(PartialEq)]
enum Phase {
    Dragging,
    Preparing,
    Acknowledging,
}

struct Transfer {
    token: u64,
    source: WindowId,
    tab: u64,
    snapshot: Option<Snapshot>,
    target: Option<WindowId>,
    created: bool,
    received_tab: Option<u64>,
    previous_active: Option<u64>,
    phase: Phase,
    preview: Option<crate::tab_drag_preview::DragPreview>,
    started: Instant,
}

pub(crate) struct Workspace {
    // App 在 Desktop 的 WebContext 之前销毁
    apps: HashMap<WindowId, App>,
    desktop: Desktop,
    focused: WindowId,
    bars: HashMap<WindowId, TabBarRect>,
    transfer: Option<Transfer>,
    highlighted: Option<WindowId>,
    serial: u64,
    deferred: Vec<(WindowId, UserEvent)>,
}

impl Workspace {
    pub(crate) fn new(desktop: Desktop, mut app: App) -> Self {
        let focused = app.window.id();
        app.install_watcher();
        let workspace = Self {
            apps: HashMap::from([(focused, app)]),
            desktop,
            focused,
            bars: HashMap::new(),
            transfer: None,
            highlighted: None,
            serial: 0,
            deferred: Vec::new(),
        };
        workspace.persist();
        workspace
    }

    fn persist(&self) {
        if !self.apps.values().any(|app| app.settings.keeps_session()) {
            return;
        }
        // 保持原有恢复格式：所有窗口的文件合并恢复到一个窗口，不引入新的布局配置
        let mut session = DocumentSession::default();
        for app in self
            .apps
            .values()
            .filter(|app| app.settings.keeps_session())
        {
            for tab in &app.session.tabs {
                session.open(tab.path.clone(), false);
            }
        }
        if let Some(active) = self
            .apps
            .get(&self.focused)
            .and_then(|app| app.session.active())
        {
            session.open(active.path.clone(), false);
        }
        if let Err(error) = session.save(&session_path()) {
            eprintln!("无法保存窗口会话: {error}");
        }
    }

    fn locked(&self, id: WindowId) -> bool {
        self.transfer
            .as_ref()
            .is_some_and(|t| t.source == id || t.target == Some(id))
    }

    fn dispatch(
        &mut self,
        id: WindowId,
        event: UserEvent,
        target: &EventLoopWindowTarget<UserEvent>,
    ) {
        if matches!(&event, UserEvent::UpdateDownloaded(_)) && self.apps.len() > 1 {
            if let Some(app) = self.apps.get_mut(&id) {
                app.update_downloading = false;
                app.eval("if(window.__setUpdateFailed)window.__setUpdateFailed('请先关闭其他窗口，再重试更新');");
            }
            return;
        }
        if self.transfer.is_some()
            && (matches!(&event, UserEvent::SetTheme(_))
                || matches!(&event, UserEvent::Ipc(body) if body.starts_with("set-setting:")))
        {
            self.cancel(None);
        }
        if let UserEvent::Ipc(body) = &event {
            if self.transfer_ipc(id, body, target) {
                return;
            }
        }
        if self.locked(id) {
            // 页面快照接管这段期间的正文；旧 save/tab-action 不能在切换后回放到另一个文件
            if matches!(event, UserEvent::Ipc(_)) {
                return;
            }
            self.deferred.push((id, event));
            return;
        }
        let Some(app) = self.apps.get_mut(&id) else {
            return;
        };
        let old_settings = app.settings.clone();
        let mut flow = ControlFlow::Wait;
        app.handle_user_event(event, &mut flow);
        let settings = app.settings.clone();
        if settings != old_settings {
            for (other_id, other) in &mut self.apps {
                if *other_id != id {
                    other.settings = settings.clone();
                    other.on_settings_changed();
                }
            }
        }
        if flow == ControlFlow::Exit {
            self.close(id);
        }
    }

    fn close(&mut self, id: WindowId) {
        if self.locked(id) {
            self.cancel(None);
        }
        // 最后一个窗口关闭时保留恢复列表；关闭其他窗口则保存剩余窗口
        if self.apps.len() == 1 {
            self.persist();
        }
        self.apps.remove(&id);
        self.bars.remove(&id);
        if self.focused == id {
            if let Some(next) = self.apps.keys().next() {
                self.focused = *next;
            }
        }
        if !self.apps.is_empty() {
            self.persist();
        }
    }

    fn window_geom(&self, source: WindowId) -> WindowGeom {
        let app = &self.apps[&source];
        let scale = app.window.scale_factor();
        let size = app.window.inner_size().to_logical::<f64>(scale);
        let pos = app
            .window
            .outer_position()
            .unwrap_or_default()
            .to_logical::<f64>(scale);
        WindowGeom {
            x: pos.x + 32.0,
            y: pos.y + 32.0,
            w: size.width,
            h: size.height,
        }
    }

    fn open_window(
        &mut self,
        paths: Vec<PathBuf>,
        edit: bool,
        target: &EventLoopWindowTarget<UserEvent>,
    ) {
        let Some(source) = self.apps.get(&self.focused) else {
            return;
        };
        let settings = source.settings.clone();
        let mut session = DocumentSession::default();
        session.single_tab = settings.tab_mode == TabMode::Single;
        for path in paths {
            session.open(path, edit);
        }
        let geom = self.window_geom(self.focused);
        match self.desktop.create_window(target, settings, session, &geom) {
            Ok(mut app) => {
                self.focused = app.window.id();
                app.install_watcher();
                self.apps.insert(self.focused, app);
                self.persist();
            }
            Err(error) => show_warning_dialog("无法创建窗口", &error),
        }
    }

    fn transfer_ipc(
        &mut self,
        id: WindowId,
        body: &str,
        target: &EventLoopWindowTarget<UserEvent>,
    ) -> bool {
        if let Some(json) = body.strip_prefix("tabbar-rect:") {
            if let Ok(rect) = serde_json::from_str::<TabBarRect>(json) {
                self.bars.insert(id, rect);
            }
            return true;
        }
        if let Some(json) = body.strip_prefix("tab-drag:") {
            let request = serde_json::from_str::<DragRequest>(json);
            if let Ok(request) = request {
                let app = &self.apps[&id];
                let valid = cfg!(target_os = "windows")
                    && self.transfer.is_none()
                    && app.ready
                    && app.session.tabs.iter().any(|tab| tab.id == request.id)
                    && request.snapshot.as_ref().is_none_or(Snapshot::valid)
                    && (request.snapshot.is_some() == (app.session.active_id == Some(request.id)));
                if valid {
                    let Some(pointer) = tab_transfer::pointer() else {
                        app.eval("window.__endTabTransfer();");
                        return true;
                    };
                    let tab = app
                        .session
                        .tabs
                        .iter()
                        .find(|tab| tab.id == request.id)
                        .unwrap();
                    let name = tab
                        .path
                        .file_name()
                        .unwrap_or(tab.path.as_os_str())
                        .to_string_lossy();
                    let dirty = request
                        .snapshot
                        .as_ref()
                        .map_or(tab.dirty, |state| state.dirty);
                    let preview = match crate::tab_drag_preview::DragPreview::new(
                        &name,
                        dirty,
                        app.window.scale_factor(),
                        pointer.x,
                        pointer.y,
                    ) {
                        Ok(preview) => preview,
                        Err(error) => {
                            app.eval("window.__endTabTransfer();");
                            show_warning_dialog(
                                "无法拖动标签",
                                &format!("无法创建标签预览：{error}"),
                            );
                            return true;
                        }
                    };
                    self.serial += 1;
                    self.transfer = Some(Transfer {
                        token: self.serial,
                        source: id,
                        tab: request.id,
                        snapshot: request.snapshot,
                        target: None,
                        created: false,
                        received_tab: None,
                        previous_active: None,
                        phase: Phase::Dragging,
                        preview: Some(preview),
                        started: Instant::now(),
                    });
                    return true;
                }
            }
            if let Some(app) = self.apps.get(&id) {
                app.eval("window.__endTabTransfer();");
            }
            return true;
        }
        if body == "tab-drag-cancel" {
            if self.locked(id) {
                self.cancel(None);
            }
            return true;
        }
        if let Some(json) = body.strip_prefix("tab-receive-ready:") {
            if let Ok(ready) = serde_json::from_str::<ReceiveReady>(json) {
                self.receive(id, ready);
            }
            return true;
        }
        if let Some(token) = body
            .strip_prefix("tab-received:")
            .and_then(|value| value.parse::<u64>().ok())
        {
            if self.transfer.as_ref().is_some_and(|t| {
                t.token == token && t.target == Some(id) && t.phase == Phase::Acknowledging
            }) {
                self.commit();
            }
            return true;
        }
        // 新窗口先正常加载空页面，收到 ready 后才开始交接
        if body == "ready"
            && self
                .transfer
                .as_ref()
                .is_some_and(|t| t.target == Some(id) && t.created)
        {
            if let Some(app) = self.apps.get_mut(&id) {
                app.handle_user_event(UserEvent::Ipc(body.into()), &mut ControlFlow::Wait);
            }
            self.prepare_receive();
            return true;
        }
        let _ = target;
        false
    }

    fn highlight(&mut self, id: Option<WindowId>) {
        if self.highlighted == id {
            return;
        }
        if let Some(previous) = self.highlighted.and_then(|id| self.apps.get(&id)) {
            previous.eval("document.body.classList.remove('tab-drop-target');");
        }
        if let Some(app) = id.and_then(|id| self.apps.get(&id)) {
            app.eval("document.body.classList.add('tab-drop-target');");
        }
        self.highlighted = id;
    }

    fn tick(&mut self, target: &EventLoopWindowTarget<UserEvent>) {
        let Some(transfer) = &self.transfer else {
            return;
        };
        if transfer.phase != Phase::Dragging {
            if transfer.started.elapsed() > Duration::from_secs(15) {
                self.cancel(Some("目标窗口未完成接收，原标签已保留"));
            }
            return;
        }
        let source = transfer.source;
        let Some(pointer) = tab_transfer::pointer() else {
            self.cancel(None);
            return;
        };
        if pointer.cancel {
            self.cancel(None);
            return;
        }
        if let Some(preview) = &transfer.preview {
            preview.move_to(pointer.x, pointer.y);
        }
        let hovered = self
            .apps
            .iter()
            .find_map(|(id, app)| (tab_transfer::root(&app.window) == pointer.root).then_some(*id));
        let hovered_bar = hovered.filter(|id| {
            let app = &self.apps[id];
            let Ok(origin) = app.window.inner_position() else {
                return false;
            };
            let scale = app.window.scale_factor();
            app.ready
                && self.bars.get(id).is_some_and(|rect| {
                    rect.contains(
                        (pointer.x - origin.x) as f64 / scale,
                        (pointer.y - origin.y) as f64 / scale,
                    )
                })
        });
        let destination = hovered_bar.filter(|id| *id != source);
        self.highlight(destination);
        if pointer.down {
            return;
        }
        self.highlight(None);
        self.transfer.as_mut().unwrap().preview = None;
        if let Some(id) = destination {
            let t = self.transfer.as_mut().unwrap();
            t.target = Some(id);
            t.phase = Phase::Preparing;
            t.started = Instant::now();
            self.prepare_receive();
        } else if hovered_bar == Some(source) {
            // 只松回源标签栏才取消；正文、工具栏和窗口外都属于拆窗区域
            self.cancel(None);
        } else if self.apps[&source].session.tabs.len() == 1 {
            // 单标签拖到空白处只移动原窗口，正文与 WebView 都不用重建
            self.apps[&source]
                .window
                .set_outer_position(PhysicalPosition::new(pointer.x - 80, pointer.y - 20));
            self.cancel(None);
        } else {
            let settings = self.apps[&source].settings.clone();
            let geom = self.window_geom(source);
            match self
                .desktop
                .create_window(target, settings, DocumentSession::default(), &geom)
            {
                Ok(app) => {
                    let id = app.window.id();
                    app.window
                        .set_outer_position(PhysicalPosition::new(pointer.x - 80, pointer.y - 20));
                    self.apps.insert(id, app);
                    let t = self.transfer.as_mut().unwrap();
                    t.target = Some(id);
                    t.created = true;
                    t.phase = Phase::Preparing;
                    t.started = Instant::now();
                }
                Err(error) => self.cancel(Some(&format!("无法创建窗口，原标签已保留：{error}"))),
            }
        }
    }

    fn prepare_receive(&mut self) {
        let Some(t) = &self.transfer else {
            return;
        };
        let Some(id) = t.target else {
            return;
        };
        let source_tab = self.apps[&t.source]
            .session
            .tabs
            .iter()
            .find(|tab| tab.id == t.tab)
            .unwrap();
        let app = &self.apps[&id];
        if app
            .session
            .tabs
            .iter()
            .any(|tab| tab.path == source_tab.path)
        {
            self.cancel(Some("目标窗口已经打开此文件，请先关闭其中一个标签再合并"));
            return;
        }
        if app.session.single_tab && !app.session.tabs.is_empty() {
            self.cancel(Some("目标窗口处于单标签模式，请先在设置中启用累积标签"));
            return;
        }
        app.eval(&format!("window.__prepareTabReceive({});", t.token));
    }

    fn receive(&mut self, id: WindowId, ready: ReceiveReady) {
        let Some(t) = &self.transfer else {
            return;
        };
        if t.target != Some(id) || t.token != ready.token || t.phase != Phase::Preparing {
            return;
        }
        let app = self.apps.get_mut(&id).unwrap();
        if app.session.active_id.unwrap_or(0) != ready.active_id {
            self.cancel(Some("目标标签已变化，转移已取消"));
            return;
        }
        if let Some(content) = ready.content {
            if !app.save_active_content(&content) {
                self.cancel(None);
                return;
            }
        } else if app.session.active_is_dirty() {
            self.cancel(Some("目标窗口还有未保存内容，转移已取消"));
            return;
        }
        let t = self.transfer.as_mut().unwrap();
        let mut tab = self.apps[&t.source]
            .session
            .tabs
            .iter()
            .find(|tab| tab.id == t.tab)
            .unwrap()
            .clone();
        let raw = if let Some(snapshot) = &t.snapshot {
            tab.dirty = snapshot.dirty;
            tab.edit_mode = snapshot.edit_mode;
            snapshot.raw.clone()
        } else {
            match read_document_with_encoding(&tab.path, tab.encoding.as_deref()) {
                Ok((raw, encoding)) => {
                    tab.encoding = Some(encoding.into());
                    raw
                }
                Err(error) => {
                    self.cancel(Some(&format!("无法读取待转移文件，原标签已保留：{error}")));
                    return;
                }
            }
        };
        let app = self.apps.get_mut(&id).unwrap();
        t.previous_active = app.session.active_id;
        let tab_id = app.session.insert_transferred(tab);
        t.received_tab = Some(tab_id);
        t.phase = Phase::Acknowledging;
        t.started = Instant::now();
        app.render_active_content(Some(raw));
        let snapshot = t.snapshot.as_ref().map(|s| serde_json::json!({
            "dirty": s.dirty, "editMode": s.edit_mode, "progress": s.progress,
            "autosavePaused": s.autosave_paused,
            "zoom": s.zoom, "selectionStart": s.selection_start, "selectionEnd": s.selection_end,
        }));
        app.eval(&format!(
            "window.__restoreTransferredTab({}, {}, {});",
            serde_json::to_string(&snapshot).unwrap(),
            t.token,
            tab_id
        ));
    }

    fn commit(&mut self) {
        let Some(t) = self.transfer.take() else {
            return;
        };
        let destination = t.target.unwrap();
        let moved_path = self.apps[&destination]
            .session
            .active()
            .unwrap()
            .path
            .clone();
        for (id, event) in &mut self.deferred {
            if *id == t.source
                && matches!(event, UserEvent::FileChanged(path) if *path == moved_path)
            {
                *id = destination;
            }
        }
        let source = self.apps.get_mut(&t.source).unwrap();
        if source.session.active_id == Some(t.tab) {
            source.eval("window.__discardTransferredTab();");
        }
        source.close_tab(t.tab);
        if source.session.tabs.is_empty() {
            self.apps.remove(&t.source);
            self.bars.remove(&t.source);
        } else {
            source.eval("window.__endTabTransfer();");
        }
        let app = self.apps.get_mut(&destination).unwrap();
        app.install_watcher();
        app.eval("window.__endTabTransfer();");
        app.window.set_focus();
        self.focused = destination;
        self.persist();
        self.replay();
    }

    fn cancel(&mut self, message: Option<&str>) {
        self.highlight(None);
        let Some(mut t) = self.transfer.take() else {
            return;
        };
        t.preview = None;
        if let Some(id) = t.target {
            if t.created {
                self.apps.remove(&id);
                self.bars.remove(&id);
                if self.focused == id {
                    self.focused = t.source;
                }
            } else if let Some(app) = self.apps.get_mut(&id) {
                if let Some(received) = t.received_tab {
                    app.session.close(received);
                    if let Some(previous) = t.previous_active {
                        app.session.activate(previous);
                    }
                    // 接收页仍被锁住，撤销接收不会丢掉用户新输入
                    app.eval("window.__discardTransferredTab();");
                    app.render_active();
                }
                app.eval("window.__endTabTransfer();");
            }
        }
        if let Some(source) = self.apps.get(&t.source) {
            source.eval("window.__endTabTransfer();");
        }
        self.replay();
        if let Some(message) = message {
            show_warning_dialog("标签未移动", message);
        }
    }

    fn replay(&mut self) {
        for (id, event) in self.deferred.drain(..) {
            let _ = self
                .desktop
                .proxy
                .send_event(UserEvent::ForWindow(id, Box::new(event)));
        }
    }

    pub(crate) fn handle(
        &mut self,
        event: Event<'_, UserEvent>,
        target: &EventLoopWindowTarget<UserEvent>,
        flow: &mut ControlFlow,
    ) {
        match event {
            Event::UserEvent(UserEvent::PersistSessions) => self.persist(),
            Event::UserEvent(UserEvent::OpenWindow(paths, edit)) => {
                self.open_window(paths, edit, target)
            }
            Event::UserEvent(UserEvent::ForWindow(id, event)) => {
                if self.apps.contains_key(&id) {
                    self.dispatch(id, *event, target);
                }
            }
            Event::UserEvent(UserEvent::OpenPaths(paths, edit)) => {
                let id = paths
                    .first()
                    .and_then(|path| {
                        let normalized = normalize_path(path.clone());
                        self.apps.iter().find_map(|(id, app)| {
                            app.session
                                .tabs
                                .iter()
                                .any(|tab| tab.path == normalized)
                                .then_some(*id)
                        })
                    })
                    .unwrap_or(self.focused);
                self.dispatch(id, UserEvent::OpenPaths(paths, edit), target);
            }
            Event::UserEvent(event) => self.dispatch(self.focused, event, target),
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if self.locked(window_id) {
                    self.cancel(None);
                }
                self.dispatch(window_id, UserEvent::Quit, target);
            }
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Focused(true),
                ..
            } => {
                if self.apps.contains_key(&window_id) {
                    self.focused = window_id;
                }
            }
            Event::Opened { urls } => {
                if self.locked(self.focused) {
                    self.cancel(None);
                }
                if let Some(app) = self.apps.get_mut(&self.focused) {
                    app.handle_opened_urls(urls);
                }
            }
            Event::MainEventsCleared => self.tick(target),
            _ => {}
        }
        *flow = if self.apps.is_empty() {
            ControlFlow::Exit
        } else if self.transfer.is_some() {
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16))
        } else {
            ControlFlow::Wait
        };
    }
}
