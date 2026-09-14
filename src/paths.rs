// 路径与文件类型判断：配置目录、file:// 地址、支持的文档类型
use crate::session::strip_verbatim_prefix;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn config_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("MD_PREVIEWER_CONFIG_DIR") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("md-previewer")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".config/md-previewer")
    }
}

pub(crate) fn recent_files_path() -> PathBuf {
    config_dir().join("recent-files.txt")
}

pub(crate) fn session_path() -> PathBuf {
    config_dir().join("session.json")
}

fn percent_encode_file_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub(crate) fn base_href_for_file(path: &Path) -> Option<String> {
    let dir = path.parent()?;
    Some(file_url_for_path_dir(dir))
}

fn file_url_for_path_dir(dir: &Path) -> String {
    let mut path = dir.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && !path.starts_with('/') {
        path.insert(0, '/');
    }
    if !path.ends_with('/') {
        path.push('/');
    }
    format!("file://{}", percent_encode_file_path(&path))
}

pub(crate) const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mkd"];

/// 侧栏同目录列表与文件选择器过滤用的常见文本扩展名；打开文件不看这份列表，只看内容是否为文本
#[rustfmt::skip]
pub(crate) const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "text", "log", "json", "jsonc", "toml", "yaml", "yml", "xml", "ini", "cfg", "conf",
    "properties", "env", "csv", "tsv", "sql", "sh", "bash", "bat", "cmd", "ps1", "py", "rb", "php",
    "lua", "pl", "js", "mjs", "cjs", "jsx", "ts", "tsx", "rs", "go", "java", "kt", "c", "h", "cc",
    "cpp", "hpp", "cs", "swift", "dart", "css", "scss", "less", "html", "htm", "vue", "gradle",
    "cmake", "diff", "patch", "rst", "adoc", "tex", "srt", "vtt", "lock",
];

fn has_extension_in(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .map(|extension| {
            let extension = extension.to_string_lossy().to_ascii_lowercase();
            extensions.contains(&extension.as_str())
        })
        .unwrap_or(false)
}

pub(crate) fn is_markdown_document(path: &Path) -> bool {
    has_extension_in(path, MARKDOWN_EXTENSIONS)
}

/// 列目录时不读文件内容，只按扩展名判断
pub(crate) fn is_listed_document(path: &Path) -> bool {
    is_markdown_document(path) || has_extension_in(path, TEXT_EXTENSIONS)
}

/// 文件选择器默认过滤项：Markdown 加常见文本扩展名
pub(crate) fn supported_dialog_extensions() -> Vec<&'static str> {
    MARKDOWN_EXTENSIONS
        .iter()
        .chain(TEXT_EXTENSIONS.iter())
        .copied()
        .collect()
}

/// 读取文档文本内容，支持 UTF-8、带 BOM 的 UTF-8（回报为 UTF-8 BOM）、带 BOM 的 UTF-16 以及 Windows ANSI (GBK/CP936 等) 编码
/// 显式打开的文件只做一次二进制嗅探：前 8 KB 含 NUL 且不是 UTF-16 BOM 开头就当作二进制拒绝
fn looks_like_text_file(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 8192];
    let Ok(read) = file.read(&mut head) else {
        return false;
    };
    let head = &head[..read];
    head.starts_with(&[0xFF, 0xFE]) || head.starts_with(&[0xFE, 0xFF]) || !head.contains(&0)
}

/// 能作为标签页打开的文件：Markdown 按扩展名识别，其余任何文本文件都按纯文本打开
pub(crate) fn is_supported_document(path: &Path) -> bool {
    is_markdown_document(path) || looks_like_text_file(path)
}

pub(crate) fn local_document_path_from_url(value: &str) -> Option<PathBuf> {
    let url = url::Url::parse(value).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    let path = url.to_file_path().ok()?;
    if !path.is_file() || !is_supported_document(&path) {
        return None;
    }
    fs::canonicalize(path).ok().map(strip_verbatim_prefix)
}
