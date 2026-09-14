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
#[derive(Debug, PartialEq, Eq)]
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
    ExternalChangeResolved {
        dirty: bool,
    },
    Print,
    Ready,
    Refresh,
    SetEncoding(String),
    SelfUpdate(String),
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
        "external-change:dirty" => Some(IpcMessage::ExternalChangeResolved { dirty: true }),
        "external-change:clean" => Some(IpcMessage::ExternalChangeResolved { dirty: false }),
        "print" => Some(IpcMessage::Print),
        "ready" => Some(IpcMessage::Ready),
        "refresh" => Some(IpcMessage::Refresh),
        "save-skipped" => Some(IpcMessage::SaveSkipped),
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
        "self-update" => IpcMessage::SelfUpdate(rest.to_string()),
        _ => return None,
    };
    Some(message)
}
