// 前端消息解析：IpcMessage / TabAction / FinderAction
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FinderAction {
    Create { folder: PathBuf, kind: String },
    Terminal { folder: PathBuf },
}

pub(crate) fn parse_finder_action(value: &str) -> Option<FinderAction> {
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
// 带阅读位置的消息用 f64，所以这里只能派 PartialEq
#[derive(Debug, PartialEq)]
pub(crate) enum IpcMessage {
    NewFile,
    OpenFile,
    OpenRecent(usize),
    OpenDoc(PathBuf),
    ForgetRecent(PathBuf),
    ClearRecent,
    RevealPath(PathBuf),
    OpenLocalLink(String),
    /// 切换或关闭标签；活动标签有未保存正文时随消息带上，先落盘再执行动作
    TabAction {
        action: TabAction,
        id: u64,
        content: Option<String>,
    },
    SetSetting {
        key: String,
        value: String,
    },
    LocateTab(u64),
    RevealTab(u64),
    RenderPreview(String),
    RenderReleaseNotes(String),
    ConvertEncoding {
        encoding: String,
        content: String,
    },
    SaveAs(String),
    Save(String),
    /// 关窗前请求页面保存，页面发现没有脏内容时回这条，事件循环据此继续退出
    SaveSkipped,
    DirtyChanged(bool),
    /// 页面手动切换预览/编辑模式；按标签记住，切回来时恢复
    EditMode(bool),
    /// 当前文件的阅读位置（0~1）：只有已记住的文件会被落盘
    ReadingProgress(f64),
    /// 提示条上的"仍按 Markdown 渲染"：这次不要按纯文本降级
    RenderMarkdownAnyway,
    /// 点书签按钮：记住/不记住当前文件的阅读位置，带上点击时的进度
    RememberPosition {
        remember: bool,
        progress: f64,
    },
    ExternalChangeResolved {
        dirty: bool,
    },
    Print,
    Ready,
    Refresh,
    SetEncoding(String),
    SelfUpdate(String),
    LogError(String),
    OpenLog,
    ClearLog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabAction {
    Activate,
    Close,
    CloseOthers,
}

pub(crate) fn parse_ipc_message(body: &str) -> Option<IpcMessage> {
    let exact = match body {
        "new-file" => Some(IpcMessage::NewFile),
        "open" => Some(IpcMessage::OpenFile),
        "clear-recent" => Some(IpcMessage::ClearRecent),
        "dirty:1" => Some(IpcMessage::DirtyChanged(true)),
        "dirty:0" => Some(IpcMessage::DirtyChanged(false)),
        "edit-mode:1" => Some(IpcMessage::EditMode(true)),
        "edit-mode:0" => Some(IpcMessage::EditMode(false)),
        "external-change:dirty" => Some(IpcMessage::ExternalChangeResolved { dirty: true }),
        "external-change:clean" => Some(IpcMessage::ExternalChangeResolved { dirty: false }),
        "print" => Some(IpcMessage::Print),
        "ready" => Some(IpcMessage::Ready),
        "refresh" => Some(IpcMessage::Refresh),
        "save-skipped" => Some(IpcMessage::SaveSkipped),
        "render-markdown-anyway" => Some(IpcMessage::RenderMarkdownAnyway),
        "open-log" => Some(IpcMessage::OpenLog),
        "clear-log" => Some(IpcMessage::ClearLog),
        _ => None,
    };
    if exact.is_some() {
        return exact;
    }
    if let Some(content) = body.strip_prefix("save-as\n") {
        return Some(IpcMessage::SaveAs(content.to_string()));
    }
    let (prefix, rest) = body.split_once(':')?;
    let message = match prefix {
        "log-error" => IpcMessage::LogError(rest.to_string()),
        "open-recent" => IpcMessage::OpenRecent(rest.parse().ok()?),
        "open-doc" => IpcMessage::OpenDoc(PathBuf::from(rest)),
        "forget-recent" => IpcMessage::ForgetRecent(PathBuf::from(rest)),
        "reveal-path" => IpcMessage::RevealPath(PathBuf::from(rest)),
        "open-local-link" => IpcMessage::OpenLocalLink(rest.to_string()),
        "tab-action" => {
            let (header, content) = rest
                .split_once('\n')
                .map(|(header, content)| (header, Some(content.to_string())))
                .unwrap_or((rest, None));
            let (action, id) = header.split_once(':')?;
            let action = match action {
                "activate" => TabAction::Activate,
                "close" => TabAction::Close,
                "close-others" => TabAction::CloseOthers,
                _ => return None,
            };
            IpcMessage::TabAction {
                action,
                id: id.parse().ok()?,
                content,
            }
        }
        "set-setting" => {
            let (key, value) = rest.split_once('=')?;
            IpcMessage::SetSetting {
                key: key.to_string(),
                value: value.to_string(),
            }
        }
        "locate-tab" => IpcMessage::LocateTab(rest.parse().ok()?),
        "reveal-tab" => IpcMessage::RevealTab(rest.parse().ok()?),
        "render-preview" => IpcMessage::RenderPreview(rest.to_string()),
        "render-release-notes" => IpcMessage::RenderReleaseNotes(rest.to_string()),
        "convert-encoding" => {
            let (encoding, content) = rest.split_once('\n')?;
            IpcMessage::ConvertEncoding {
                encoding: encoding.to_string(),
                content: content.to_string(),
            }
        }
        "save" => IpcMessage::Save(rest.to_string()),
        "set-encoding" => IpcMessage::SetEncoding(rest.to_string()),
        "reading-progress" => IpcMessage::ReadingProgress(rest.parse().ok()?),
        // 页面点书签时把当前位置一起带上，避免等下一次滚动上报才拿到进度
        "remember-position" => {
            let (remember, progress) = rest.split_once('\n').unwrap_or((rest, "0"));
            IpcMessage::RememberPosition {
                remember: remember == "1",
                progress: progress.parse().unwrap_or(0.0),
            }
        }
        "self-update" => IpcMessage::SelfUpdate(rest.to_string()),
        _ => return None,
    };
    Some(message)
}
