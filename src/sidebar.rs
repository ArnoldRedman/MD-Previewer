// 侧栏数据：同目录文件、最近打开、大纲与作者模式文档解析
/// 章节序号里可能出现的数字，阿拉伯数字、全角数字和中文数字都算
use crate::escape::html_escape_text;
use crate::i18n::Strings;
use crate::paths::is_listed_document;
use crate::session::DocumentSession;
use pulldown_cmark::Event as MdEvent;
use pulldown_cmark::Parser;
use std::fs;
use std::path::{Path, PathBuf};

const CHAPTER_DIGITS: &str =
    "0123456789０１２３４５６７８９一二三四五六七八九十百千万零两壹贰叁肆伍陆柒捌玖拾佰仟";
/// 跟在序号后面的量词
const CHAPTER_UNITS: &str = "章节回卷篇幕集话";
/// 序号和正题之间可能的分隔符
const CHAPTER_SEPARATORS: [char; 12] = [
    ' ', '\t', '　', ':', '：', '.', '。', '、', ',', '，', '-', '—',
];

/// 作者模式里两个标题按钮要复制的内容。
/// 正文不在这里：它必须和屏幕上渲染出来的一致，由页面直接从 DOM 取
pub(crate) struct AuthorDoc {
    /// 整行标题的纯文本，例如「第二章 一只行李箱」
    pub(crate) title_line: String,
    /// 去掉序号后的标题，例如「一只行李箱」
    pub(crate) title: String,
}

/// 把标题里的行内 Markdown 还原成纯文本，跟页面上看到的一致。
/// 否则「## 第二章 **一只**行李箱」会把星号一起复制走
fn inline_plain_text(markdown: &str) -> String {
    let mut text = String::new();
    for event in Parser::new(markdown) {
        match event {
            MdEvent::Text(value) | MdEvent::Code(value) => text.push_str(&value),
            MdEvent::SoftBreak | MdEvent::HardBreak => text.push(' '),
            _ => {}
        }
    }
    text.trim().to_string()
}

/// 吃掉开头的「第X章」或「12、」这类序号，返回剩下的部分；不是序号则返回 None
fn chapter_number_prefix(title: &str) -> Option<&str> {
    if let Some(rest) = title.strip_prefix('第') {
        let split = rest.find(|c| !CHAPTER_DIGITS.contains(c))?;
        if split == 0 {
            return None;
        }
        let tail = &rest[split..];
        let unit = tail.chars().next()?;
        if !CHAPTER_UNITS.contains(unit) {
            return None;
        }
        return Some(&tail[unit.len_utf8()..]);
    }
    let split = title.find(|c: char| !c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let tail = &title[split..];
    // 纯数字开头必须紧跟分隔符才算序号，否则「2023年的夏天」会被误伤
    tail.starts_with(CHAPTER_SEPARATORS).then_some(tail)
}

/// 去掉章节标题里的序号前缀。整行只有序号时保持原样，免得复制出空串
pub(crate) fn strip_chapter_prefix(title: &str) -> &str {
    let title = title.trim();
    let stripped = chapter_number_prefix(title)
        .map(|rest| rest.trim_start_matches(CHAPTER_SEPARATORS))
        .unwrap_or("");
    if stripped.is_empty() {
        title
    } else {
        stripped
    }
}

/// 取出一章的标题行和去掉序号的标题，认第一个 ATX 标题作章节标题
pub(crate) fn author_doc(raw_md: &str) -> AuthorDoc {
    let heading = raw_md.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes == 0 || hashes > 6 {
            return None;
        }
        let rest = &trimmed[hashes..];
        // CommonMark 要求 # 后面跟空白，跟渲染器保持一致
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        let text = inline_plain_text(rest);
        (!text.is_empty()).then_some(text)
    });
    let Some(title_line) = heading else {
        return AuthorDoc {
            title_line: String::new(),
            title: String::new(),
        };
    };
    AuthorDoc {
        title: strip_chapter_prefix(&title_line).to_string(),
        title_line,
    }
}

/// 单个文件夹里列出的上限。目录是外部输入，遇到几千个文件的目录不该把界面拖死
const MAX_FOLDER_FILES: usize = 200;

/// 当前文档同目录下可预览的文档，按文件名排序。只列这一层不递归：
/// 侧栏的用途是在同一篇文档的邻居之间快速切换
pub(crate) fn folder_documents(active: Option<&Path>) -> Vec<PathBuf> {
    let Some(dir) = active.and_then(Path::parent) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_listed_document(path))
        .collect::<Vec<_>>();
    files.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    files.truncate(MAX_FOLDER_FILES);
    files
}

fn sidebar_entry_json(path: &Path, active: Option<&Path>) -> serde_json::Value {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    // 只显示所在目录名，完整路径放在条目的 title 里。
    // 用整条路径 + CSS 省略号会触发 bidi 重排，把路径分隔符甩到错误的位置
    let dir = path
        .parent()
        .and_then(Path::file_name)
        .map(|dir| dir.to_string_lossy().to_string())
        .unwrap_or_default();
    serde_json::json!({
        "path": path.to_string_lossy(),
        "name": name,
        "dir": dir,
        "active": Some(path) == active,
    })
}

pub(crate) fn sidebar_json(session: &DocumentSession, recent: &[PathBuf]) -> String {
    let active = session.active().map(|tab| tab.path.clone());
    let active = active.as_deref();
    let folder = folder_documents(active)
        .iter()
        .map(|path| sidebar_entry_json(path, active))
        .collect::<Vec<_>>();
    let recent = recent
        .iter()
        .map(|path| sidebar_entry_json(path, active))
        .collect::<Vec<_>>();
    serde_json::json!({ "folder": folder, "recent": recent }).to_string()
}

pub(crate) fn tabs_json(session: &DocumentSession) -> String {
    let tabs = session
        .tabs
        .iter()
        .map(|tab| {
            let name = tab
                .path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| tab.path.to_string_lossy().to_string());
            serde_json::json!({
                "id": tab.id,
                "name": name,
                "path": tab.path.to_string_lossy(),
                "active": session.active_id == Some(tab.id),
                "missing": tab.missing,
                "dirty": tab.dirty,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&tabs).expect("tab state is serializable")
}

pub(crate) fn missing_preview_html(tab_id: u64, path: &Path, s: &Strings) -> String {
    format!(
        r#"<div class="missing-file"><div class="missing-mark">!</div><h2>{}</h2><p>{}</p><code>{}</code><div class="missing-actions"><button type="button" data-locate-tab="{tab_id}">{}</button><button type="button" data-close-tab="{tab_id}">{}</button></div></div>"#,
        html_escape_text(s.missing_title),
        html_escape_text(s.missing_body),
        html_escape_text(&path.to_string_lossy()),
        html_escape_text(s.locate_file),
        html_escape_text(s.close_tab),
    )
}
