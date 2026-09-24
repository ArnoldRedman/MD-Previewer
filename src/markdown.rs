// Markdown 渲染：正文转 HTML、标题锚点、增强特性探测
use crate::assets::{KATEX_CSS, KATEX_JS, MERMAID_JS};
use crate::escape::{escape_js, html_escape_text};
use crate::sanitize::sanitize_raw_html;
use pulldown_cmark::Event as MdEvent;
use pulldown_cmark::{html, CowStr, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EnhanceFlags {
    pub(crate) math: bool,
    pub(crate) mermaid: bool,
}

impl EnhanceFlags {
    pub(crate) fn any(self) -> bool {
        self.math || self.mermaid
    }
}
pub(crate) fn md_to_html(md: &str) -> String {
    md_to_html_with_base(md, None)
}

pub(crate) fn md_to_html_with_base(md: &str, base_dir: Option<&Path>) -> String {
    let (front_matter, markdown) = split_yaml_front_matter(md)
        .map(|(metadata, body)| (Some(metadata), body))
        .unwrap_or((None, md));
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM;
    let parser = Parser::new_ext(markdown, opts);
    // 过滤必须在 add_mark_highlights 之前，它自己注入的 <mark> 不能被当成用户 HTML 处理
    let events = embed_local_images(
        add_mark_highlights(add_heading_ids(sanitize_raw_html(parser.collect()))),
        base_dir,
    );
    let mut html_out = String::new();
    if let Some(metadata) = front_matter {
        html_out.push_str(r#"<aside class="front-matter"><pre>"#);
        html_out.push_str(&html_escape_text(metadata.trim_end_matches(['\r', '\n'])));
        html_out.push_str("</pre></aside>\n");
    }
    html::push_html(&mut html_out, events.into_iter());
    html_out
}

fn split_yaml_front_matter(md: &str) -> Option<(&str, &str)> {
    let mut lines = md.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return None;
    }

    let metadata_start = first.len();
    let mut offset = metadata_start;
    for line in lines {
        let value = line.trim_end_matches(['\r', '\n']);
        if value == "---" || value == "..." {
            let metadata = &md[metadata_start..offset];
            let body = &md[offset + line.len()..];
            return Some((metadata, body));
        }
        offset += line.len();
    }
    None
}

fn embed_local_images<'a>(events: Vec<MdEvent<'a>>, base_dir: Option<&Path>) -> Vec<MdEvent<'a>> {
    let Some(base_dir) = base_dir else {
        return events;
    };

    events
        .into_iter()
        .map(|event| match event {
            MdEvent::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let embedded = local_image_data_url(base_dir, dest_url.as_ref());
                MdEvent::Start(Tag::Image {
                    link_type,
                    dest_url: embedded.map(CowStr::from).unwrap_or(dest_url),
                    title,
                    id,
                })
            }
            _ => event,
        })
        .collect()
}

fn local_image_data_url(base_dir: &Path, url: &str) -> Option<String> {
    let image_path = resolve_local_relative_image_path(base_dir, url)?;
    let mime = image_mime_type(&image_path)?;
    let bytes = fs::read(image_path).ok()?;
    Some(format!("data:{mime};base64,{}", base64_encode(&bytes)))
}

fn resolve_local_relative_image_path(base_dir: &Path, url: &str) -> Option<PathBuf> {
    let path_part = url.split(['#', '?']).next()?.trim();
    if !is_local_relative_url(path_part) {
        return None;
    }

    let mut candidate = base_dir.to_path_buf();
    for segment in path_part.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        let decoded = percent_decode_path_segment(segment)?;
        let segment_path = Path::new(&decoded);
        if segment_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return None;
        }
        candidate.push(segment_path);
    }

    if candidate == base_dir {
        return None;
    }
    Some(candidate)
}

fn is_local_relative_url(url: &str) -> bool {
    !url.is_empty()
        && !url.starts_with('#')
        && !url.starts_with('/')
        && !url.starts_with('\\')
        && !url.starts_with("//")
        && !url.contains(':')
}

fn percent_decode_path_segment(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = bytes.get(i + 1).and_then(|b| hex_value(*b))?;
            let lo = bytes.get(i + 2).and_then(|b| hex_value(*b))?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn image_mime_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()?
        .to_string_lossy()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(((bytes.len() + 2) / 3) * 4);
    let mut chunks = bytes.chunks_exact(3);
    for chunk in &mut chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | chunk[2] as u32;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        out.push(TABLE[(n & 0x3f) as usize] as char);
    }

    match chunks.remainder() {
        [a] => {
            let n = (*a as u32) << 16;
            out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
            out.push('=');
            out.push('=');
        }
        [a, b] => {
            let n = ((*a as u32) << 16) | ((*b as u32) << 8);
            out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
            out.push('=');
        }
        _ => {}
    }

    out
}

/// 用户 Markdown 里的原始 HTML 会直接进页面，而页面握着 window.ipc，
/// 一段 `<script>` 或 `onerror` 就能替用户改写当前文件。这里把相邻的 HTML 事件合并后统一过滤：
fn add_mark_highlights<'a>(events: Vec<MdEvent<'a>>) -> Vec<MdEvent<'a>> {
    let mut out = Vec::with_capacity(events.len());

    for event in events {
        match event {
            MdEvent::Text(text) => {
                if text.contains("==") {
                    push_mark_highlight_events(text.as_ref(), &mut out);
                } else {
                    out.push(MdEvent::Text(text));
                }
            }
            _ => out.push(event),
        }
    }

    out
}

fn push_mark_highlight_events<'a>(text: &str, out: &mut Vec<MdEvent<'a>>) {
    let mut rest = text;

    while let Some(open) = rest.find("==") {
        let after_open = open + 2;
        let Some(close_rel) = rest[after_open..].find("==") else {
            break;
        };
        let close = after_open + close_rel;
        let body = &rest[after_open..close];
        if body.trim().is_empty() {
            break;
        }

        if open > 0 {
            out.push(MdEvent::Text(CowStr::Boxed(
                rest[..open].to_string().into_boxed_str(),
            )));
        }
        out.push(MdEvent::Html(CowStr::Borrowed(
            r#"<mark class="mdp-mark">"#,
        )));
        out.push(MdEvent::Text(CowStr::Boxed(
            body.to_string().into_boxed_str(),
        )));
        out.push(MdEvent::Html(CowStr::Borrowed("</mark>")));
        rest = &rest[close + 2..];
    }

    if !rest.is_empty() {
        out.push(MdEvent::Text(CowStr::Boxed(
            rest.to_string().into_boxed_str(),
        )));
    }
}

fn add_heading_ids<'a>(mut events: Vec<MdEvent<'a>>) -> Vec<MdEvent<'a>> {
    let mut seen: HashMap<String, usize> = HashMap::new();

    for i in 0..events.len() {
        let generate_id = match &events[i] {
            MdEvent::Start(Tag::Heading { id: Some(id), .. }) => {
                register_heading_id(id.as_ref(), &mut seen);
                false
            }
            MdEvent::Start(Tag::Heading { id: None, .. }) => true,
            _ => false,
        };

        if !generate_id {
            continue;
        }

        let text = collect_heading_text(&events, i);
        let base = heading_slug(&text);
        let id_value = unique_heading_id(base, &mut seen);
        if let MdEvent::Start(Tag::Heading { id, .. }) = &mut events[i] {
            *id = Some(CowStr::Boxed(id_value.into_boxed_str()));
        }
    }

    events
}

fn collect_heading_text(events: &[MdEvent<'_>], start: usize) -> String {
    let mut text = String::new();

    for event in events.iter().skip(start + 1) {
        match event {
            MdEvent::End(TagEnd::Heading(_)) => break,
            MdEvent::Text(value)
            | MdEvent::Code(value)
            | MdEvent::InlineMath(value)
            | MdEvent::DisplayMath(value) => text.push_str(value.as_ref()),
            MdEvent::SoftBreak | MdEvent::HardBreak => text.push(' '),
            _ => {}
        }
    }

    text
}

fn heading_slug(text: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;

    for c in text.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            slug.push(c);
            last_dash = false;
        } else if c.is_whitespace() && !slug.is_empty() && !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

fn register_heading_id(id: &str, seen: &mut HashMap<String, usize>) {
    if !id.is_empty() {
        *seen.entry(id.to_string()).or_insert(0) += 1;
    }
}

fn unique_heading_id(base: String, seen: &mut HashMap<String, usize>) -> String {
    let count = seen.entry(base.clone()).or_insert(0);
    let id = if *count == 0 {
        base
    } else {
        format!("{base}-{count}")
    };
    *count += 1;
    id
}

/// 纯文本渲染为保留换行与空格的 HTML 容器，特殊符号统一做 HTML 转义
pub(crate) fn txt_to_html(raw: &str) -> String {
    format!(
        r#"<div class="mdp-plain-text">{}</div>"#,
        html_escape_text(raw)
    )
}

/// 根据文档扩展名分发渲染：txt 走纯文本保留换行，md 走标准 Markdown 解析与增强
fn starts_mermaid_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("```")
        .or_else(|| trimmed.strip_prefix("~~~"));
    let Some(info) = rest else {
        return false;
    };
    let info = info.trim_start();
    info == "mermaid"
        || info
            .strip_prefix("mermaid")
            .and_then(|s| s.chars().next())
            .map(|c| c.is_whitespace() || c == '{')
            .unwrap_or(false)
}

fn has_unescaped_at(s: &str, index: usize, needle: &str) -> bool {
    if !s[index..].starts_with(needle) {
        return false;
    }
    let mut backslashes = 0;
    for b in s[..index].bytes().rev() {
        if b == b'\\' {
            backslashes += 1;
        } else {
            break;
        }
    }
    backslashes % 2 == 0
}

/// 找 `from` 之后第一个没有被反斜杠转义的 needle
fn find_unescaped(s: &str, needle: &str, from: usize) -> Option<usize> {
    s[from..]
        .match_indices(needle)
        .find(|(index, _)| has_unescaped_at(s, from + index, needle))
        .map(|(index, _)| from + index)
}

fn has_unescaped_pair(s: &str, open: &str, close: &str) -> bool {
    // 命中条件只看「第一个没被转义的开符号之后有没有没被转义的闭符号」：
    // 更靠后的开符号不会带来新的闭符号位置。逐个开符号去扫尾巴在大文档上是 O(n²)，
    // 一千多行的 PowerShell/LaTeX 文档就能把打开文件卡住几十秒
    let Some(start) = find_unescaped(s, open, 0) else {
        return false;
    };
    find_unescaped(s, close, start + open.len()).is_some()
}

/// 行内 `$...$` 的开符号：`$` 本身没被转义，后面既不能是空白也不能是另一个 `$`
fn is_inline_dollar_open(s: &str, bytes: &[u8], i: usize) -> bool {
    if bytes[i] != b'$' || !has_unescaped_at(s, i, "$") {
        return false;
    }
    match bytes.get(i + 1) {
        Some(b'$') | None => false,
        Some(b) => !b.is_ascii_whitespace(),
    }
}

/// 行内 `$...$` 的闭符号：`$` 没被转义，且前一个字符不是空白
fn is_inline_dollar_close(s: &str, bytes: &[u8], i: usize) -> bool {
    bytes[i] == b'$'
        && has_unescaped_at(s, i, "$")
        && bytes
            .get(i.wrapping_sub(1))
            .map(|b| !b.is_ascii_whitespace())
            .unwrap_or(false)
}

fn has_inline_dollar_math(s: &str) -> bool {
    let bytes = s.as_bytes();
    // 闭符号合不合法只和它前一个字符有关，跟开符号无关：
    // 第一个合法开符号找不到闭符号，更靠后的开符号也一样找不到。
    // 原来的写法对每个 `$` 都往尾巴扫一遍，是 O(n²)。
    // 两轮都只在 `$` 位置上做事（match_indices 走 memchr）
    let Some(open) = s
        .match_indices('$')
        .map(|(index, _)| index)
        .find(|&index| is_inline_dollar_open(s, bytes, index))
    else {
        return false;
    };
    s[open + 1..]
        .match_indices('$')
        .map(|(index, _)| open + 1 + index)
        .any(|index| is_inline_dollar_close(s, bytes, index))
}

pub(crate) fn enhance_flags_for(md: &str) -> EnhanceFlags {
    EnhanceFlags {
        math: has_unescaped_pair(md, "$$", "$$")
            || has_unescaped_pair(md, "\\[", "\\]")
            || has_unescaped_pair(md, "\\(", "\\)")
            || has_inline_dollar_math(md),
        mermaid: md.lines().any(starts_mermaid_fence),
    }
}

pub(crate) fn build_enhancer_bootstrap(flags: EnhanceFlags, loaded: EnhanceFlags) -> Vec<String> {
    if !flags.any() {
        return Vec::new();
    }

    let mut scripts = Vec::new();
    if flags.math && !loaded.math {
        let mut js = String::from("(function(){\nif(!window.katex){\n");
        js.push_str(KATEX_JS);
        js.push_str("\n;try{window.katex=katex;}catch(e){}\n}\n");
        js.push_str("if(window.__setKatexCss)window.__setKatexCss('");
        js.push_str(&escape_js(KATEX_CSS));
        js.push_str("');\n})();");
        scripts.push(js);
    }
    if flags.mermaid && !loaded.mermaid {
        // Mermaid's standalone bundle expects global script scope. Keep it
        // out of the function wrapper that is safe for KaTeX/highlight.js.
        let mut js = String::with_capacity(MERMAID_JS.len() + 80);
        js.push_str(MERMAID_JS);
        js.push_str("\n;try{window.mermaid=mermaid;}catch(e){}\n");
        scripts.push(js);
    }
    scripts.push("if(window.__enhancePreview)window.__enhancePreview();".to_string());
    scripts
}
