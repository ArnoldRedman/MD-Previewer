// HTML 消毒与转义：只放行白名单标签和属性
/// pulldown-cmark 按行切分 HTML 块，跨行的标签只有拼起来才看得出真实结构
use pulldown_cmark::CowStr;
use pulldown_cmark::Event as MdEvent;

pub(crate) fn sanitize_raw_html<'a>(events: Vec<MdEvent<'a>>) -> Vec<MdEvent<'a>> {
    pub(crate) fn flush<'a>(pending: &mut Option<(bool, String)>, out: &mut Vec<MdEvent<'a>>) {
        let Some((inline, raw)) = pending.take() else {
            return;
        };
        let clean = sanitize_html_fragment(&raw);
        if clean.is_empty() {
            return;
        }
        let text = CowStr::from(clean);
        out.push(if inline {
            MdEvent::InlineHtml(text)
        } else {
            MdEvent::Html(text)
        });
    }

    let mut out = Vec::with_capacity(events.len());
    let mut pending: Option<(bool, String)> = None;
    for event in events {
        let (inline, raw) = match event {
            MdEvent::Html(raw) => (false, raw),
            MdEvent::InlineHtml(raw) => (true, raw),
            other => {
                flush(&mut pending, &mut out);
                out.push(other);
                continue;
            }
        };
        match pending.as_mut() {
            Some((kind, buffer)) if *kind == inline => buffer.push_str(&raw),
            _ => {
                flush(&mut pending, &mut out);
                pending = Some((inline, raw.into_string()));
            }
        }
    }
    flush(&mut pending, &mut out);
    out
}

/// 连同内容一起丢弃的标签：能执行脚本、嵌入外部文档，或者会改掉整个页面的样式
const HTML_DROP_WITH_CONTENT: &[&str] = &[
    "script",
    "style",
    "iframe",
    "object",
    "svg",
    "math",
    "xmp",
    "plaintext",
    "noembed",
    "noframes",
];
/// 只丢标签本身、保留后续内容的标签
const HTML_DROP_TAG: &[&str] = &[
    "embed", "link", "meta", "base", "applet", "frame", "frameset", "param",
];
/// 取值是 URL 的属性，需要检查协议
const HTML_URL_ATTRS: &[&str] = &[
    "href",
    "src",
    "action",
    "cite",
    "poster",
    "background",
    "data",
    "ping",
    "longdesc",
    "codebase",
    "srcset",
];

/// 最小 HTML 词法过滤：逐个标签重建，去掉事件属性、危险协议和 data-* 属性；
/// 没有闭合的 `<` 一律转义，避免和后面的文本拼成新标签
fn sanitize_html_fragment(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            let ch = raw[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        if raw[i..].starts_with("<!--") {
            match raw[i + 4..].find("-->") {
                Some(end) => i += 4 + end + 3,
                None => {
                    out.push_str("&lt;");
                    i += 1;
                }
            }
            continue;
        }
        let Some(tag) = parse_html_tag(raw, i) else {
            out.push_str("&lt;");
            i += 1;
            continue;
        };
        let name = tag.name.to_ascii_lowercase();
        i = tag.end;
        if tag.closing {
            if !HTML_DROP_WITH_CONTENT.contains(&name.as_str())
                && !HTML_DROP_TAG.contains(&name.as_str())
            {
                out.push_str("</");
                out.push_str(&name);
                out.push('>');
            }
            continue;
        }
        if HTML_DROP_WITH_CONTENT.contains(&name.as_str()) {
            i = skip_past_closing_tag(raw, i, &name);
            continue;
        }
        if HTML_DROP_TAG.contains(&name.as_str()) {
            continue;
        }
        out.push('<');
        out.push_str(&name);
        for (attr_name, value) in &tag.attrs {
            let attr = attr_name.to_ascii_lowercase();
            if attr.starts_with("on")
                || attr.starts_with("data-")
                || attr == "srcdoc"
                || attr == "formaction"
            {
                continue;
            }
            if HTML_URL_ATTRS.contains(&attr.as_str()) {
                let unsafe_url = match value {
                    Some(value) if attr == "srcset" => value
                        .split(',')
                        .filter_map(|candidate| candidate.split_whitespace().next())
                        .any(|url| !is_safe_url(url)),
                    Some(value) => !is_safe_url(value),
                    None => false,
                };
                if unsafe_url {
                    continue;
                }
            }
            out.push(' ');
            out.push_str(&attr);
            if let Some(value) = value {
                out.push_str("=\"");
                out.push_str(&value.replace('"', "&quot;").replace('<', "&lt;"));
                out.push('"');
            }
        }
        if tag.self_closing {
            out.push_str(" /");
        }
        out.push('>');
    }
    out
}

struct HtmlTag<'a> {
    name: &'a str,
    closing: bool,
    self_closing: bool,
    attrs: Vec<(&'a str, Option<&'a str>)>,
    /// 紧随 `>` 之后的字节下标
    end: usize,
}

// 从 `<` 开始解析一个标签；不是合法标签或没遇到 `>` 就返回 None
fn parse_html_tag(raw: &str, start: usize) -> Option<HtmlTag<'_>> {
    let bytes = raw.as_bytes();
    let mut i = start + 1;
    let closing = bytes.get(i) == Some(&b'/');
    if closing {
        i += 1;
    }
    let name_start = i;
    if !bytes.get(i).map(u8::is_ascii_alphabetic).unwrap_or(false) {
        return None;
    }
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b':')
    {
        i += 1;
    }
    let name = &raw[name_start..i];
    let mut attrs = Vec::new();
    let mut self_closing = false;
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let current = *bytes.get(i)?;
        if current == b'>' {
            i += 1;
            break;
        }
        if current == b'/' {
            if bytes.get(i + 1) == Some(&b'>') {
                self_closing = true;
                i += 2;
                break;
            }
            i += 1;
            continue;
        }
        let attr_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'=' | b'>' | b'/')
        {
            i += 1;
        }
        if i == attr_start {
            i += 1;
            continue;
        }
        let attr_name = &raw[attr_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            attrs.push((attr_name, None));
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let quote = *bytes.get(i)?;
        let value = if quote == b'"' || quote == b'\'' {
            let value_start = i + 1;
            let len = raw[value_start..].find(quote as char)?;
            i = value_start + len + 1;
            &raw[value_start..value_start + len]
        } else {
            let value_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                i += 1;
            }
            &raw[value_start..i]
        };
        attrs.push((attr_name, Some(value)));
    }
    Some(HtmlTag {
        name,
        closing,
        self_closing,
        attrs,
        end: i,
    })
}

// 跳到 `</name>` 之后；找不到闭合标签就把剩余内容全部丢掉
fn skip_past_closing_tag(raw: &str, from: usize, name: &str) -> usize {
    let lower = raw[from..].to_ascii_lowercase();
    let marker = format!("</{name}");
    let mut search = 0;
    while let Some(found) = lower[search..].find(&marker) {
        let after = search + found + marker.len();
        let boundary = lower.as_bytes().get(after).copied();
        if boundary
            .map(|b| b.is_ascii_whitespace() || b == b'>')
            .unwrap_or(false)
        {
            return match lower[after..].find('>') {
                Some(close) => from + after + close + 1,
                None => raw.len(),
            };
        }
        search = after;
    }
    raw.len()
}

/// 浏览器会忽略 URL 里的控制字符和空白，并解码实体，所以先压缩再看协议；
/// 协议部分出现实体一律拒绝，`java&Tab;script:` 这类写法才拦得住
pub(crate) fn is_safe_url(value: &str) -> bool {
    let compact: String = value
        .chars()
        .filter(|ch| !ch.is_control() && !ch.is_whitespace())
        .collect();
    let head_end = compact.find(['/', '?', '#']).unwrap_or(compact.len());
    let head = &compact[..head_end];
    if head.contains('&') {
        return false;
    }
    let Some(colon) = head.find(':') else {
        return true;
    };
    let scheme = head[..colon].to_ascii_lowercase();
    match scheme.as_str() {
        "http" | "https" | "mailto" | "tel" | "file" | "ftp" => true,
        "data" => {
            let media = compact[colon + 1..].to_ascii_lowercase();
            // SVG 图片可以带脚本，作为文档导航目标时会执行
            media.starts_with("image/") && !media.starts_with("image/svg")
        }
        _ => false,
    }
}
