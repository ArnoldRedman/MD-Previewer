// 文档读写：编码探测与转换、保存、外部改动保护
use crate::i18n::Strings;
use crate::markdown::{enhance_flags_for, md_to_html_with_base, txt_to_html, EnhanceFlags};
use crate::paths::{base_href_for_file, is_markdown_document};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub(crate) struct SelfWriteRecord {
    pub(crate) at: Instant,
    pub(crate) path: PathBuf,
    /// 实际写入磁盘的字节；非 UTF-8 编码下与文本不同，必须按字节比较
    pub(crate) content: Vec<u8>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
/// 返回解析后的文本和实际采用的编码格式名称
pub(crate) fn read_document_with_encoding(
    path: &Path,
    encoding_override: Option<&str>,
) -> std::io::Result<(String, &'static str)> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() {
        let enc = match encoding_override {
            Some("UTF-8 BOM") => "UTF-8 BOM",
            Some("GBK") => "GBK",
            Some("UTF-16 LE") => "UTF-16 LE",
            Some("UTF-16 BE") => "UTF-16 BE",
            _ => "UTF-8",
        };
        return Ok((String::new(), enc));
    }

    // 若用户明确指定了编码格式，优先按指定格式解码
    if let Some(enc) = encoding_override {
        match enc {
            "UTF-8" | "UTF-8 BOM" => {
                // 两者解码方式相同，只是回报的编码名不同，决定保存时是否写 BOM
                let label = if enc == "UTF-8 BOM" {
                    "UTF-8 BOM"
                } else {
                    "UTF-8"
                };
                let slice = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    &bytes[3..]
                } else {
                    &bytes[..]
                };
                if let Ok(s) = std::str::from_utf8(slice) {
                    return Ok((s.to_string(), label));
                }
                return Ok((String::from_utf8_lossy(slice).into_owned(), label));
            }
            "GBK" => {
                // 用户明确选了 GBK，非法序列按替换字符处理也要给出结果
                #[cfg(target_os = "windows")]
                {
                    if let Some(s) = decode_windows_codepage(&bytes, 936, false) {
                        return Ok((s, "GBK"));
                    }
                }
                return Ok((String::from_utf8_lossy(&bytes).into_owned(), "GBK"));
            }
            "UTF-16 LE" => {
                return Ok((
                    decode_utf16(strip_bom(&bytes, &[0xFF, 0xFE]), false),
                    "UTF-16 LE",
                ));
            }
            "UTF-16 BE" => {
                return Ok((
                    decode_utf16(strip_bom(&bytes, &[0xFE, 0xFF]), true),
                    "UTF-16 BE",
                ));
            }
            _ => {}
        }
    }

    // 自动探测编码。带 BOM 的文件已经声明了编码，内容不合法也按该编码容错解码，
    // 不再退回后面的探测分支，否则 BOM 字节会被当成 GBK 正文
    if let Some(body) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return Ok((String::from_utf8_lossy(body).into_owned(), "UTF-8 BOM"));
    }
    if let Some(body) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return Ok((decode_utf16(body, false), "UTF-16 LE"));
    }
    if let Some(body) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return Ok((decode_utf16(body, true), "UTF-16 BE"));
    }
    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Ok((s.to_string(), "UTF-8"));
    }

    // Windows 上按 GBK 严格探测：非法序列直接失败，Latin-1、Big5 这类文件不会被误标成 GBK 后再按 GBK 写坏
    #[cfg(target_os = "windows")]
    {
        if let Some(decoded) = decode_windows_codepage(&bytes, 936, true) {
            return Ok((decoded, "GBK"));
        }
    }

    // 兜底：容错转 UTF-8。此时文件编码未知，编辑保存会把替换字符写回去
    Ok((String::from_utf8_lossy(&bytes).into_owned(), "UTF-8"))
}

fn strip_bom<'a>(bytes: &'a [u8], bom: &[u8]) -> &'a [u8] {
    bytes.strip_prefix(bom).unwrap_or(bytes)
}

/// UTF-16 码元解码；尾部多出的单字节和无效代理对都属于损坏数据，按替换字符处理
pub(crate) fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if big_endian {
                u16::from_be_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_le_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

/// 读取文档文本内容快捷入口
#[allow(dead_code)]
pub(crate) fn read_document_to_string(path: &Path) -> std::io::Result<String> {
    read_document_with_encoding(path, None).map(|(content, _)| content)
}

/// 文本按目标编码转成写盘字节的结果
pub(crate) struct EncodedDocument {
    pub(crate) bytes: Vec<u8>,
    /// 目标编码无法表示、已替换成 ? 的字符数，目前只有 GBK 会出现
    pub(crate) lossy_chars: usize,
}

/// 把文本编码成指定编码的字节；编码名未知或当前系统不支持时返回错误信息
pub(crate) fn encode_document(content: &str, encoding: &str) -> Result<EncodedDocument, String> {
    let bytes = match encoding {
        "UTF-8" => content.as_bytes().to_vec(),
        "UTF-8 BOM" => {
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(content.as_bytes());
            bytes
        }
        "UTF-16 LE" => {
            let mut bytes = vec![0xFF, 0xFE];
            for unit in content.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            bytes
        }
        "UTF-16 BE" => {
            let mut bytes = vec![0xFE, 0xFF];
            for unit in content.encode_utf16() {
                bytes.extend_from_slice(&unit.to_be_bytes());
            }
            bytes
        }
        "GBK" => return encode_gbk(content),
        other => return Err(format!("不支持的编码：{other}")),
    };
    Ok(EncodedDocument {
        bytes,
        lossy_chars: 0,
    })
}

/// GBK 走系统代码页 936；先整体转换，只有确认出现替换字符时才逐字符统计数量
#[cfg(target_os = "windows")]
fn encode_gbk(content: &str) -> Result<EncodedDocument, String> {
    let wide: Vec<u16> = content.encode_utf16().collect();
    let (bytes, replaced) =
        encode_wide_to_codepage(&wide, 936).ok_or_else(|| "系统代码页 936 转换失败".to_string())?;
    if !replaced {
        return Ok(EncodedDocument {
            bytes,
            lossy_chars: 0,
        });
    }
    let lossy_chars = content
        .chars()
        .filter(|ch| !ch.is_ascii())
        .filter(|ch| {
            let mut units = [0u16; 2];
            matches!(
                encode_wide_to_codepage(ch.encode_utf16(&mut units), 936),
                Some((_, true))
            )
        })
        .count();
    Ok(EncodedDocument { bytes, lossy_chars })
}

#[cfg(not(target_os = "windows"))]
fn encode_gbk(_content: &str) -> Result<EncodedDocument, String> {
    Err("当前系统不支持写入 GBK 编码".to_string())
}

/// `strict` 为真时非法字节序列直接返回 None，用于编码探测；
/// 为假时按系统默认替换字符解码，用于用户明确指定编码的场景
#[cfg(target_os = "windows")]
pub(crate) fn decode_windows_codepage(
    bytes: &[u8],
    code_page: u32,
    strict: bool,
) -> Option<String> {
    if bytes.is_empty() {
        return Some(String::new());
    }
    const MB_ERR_INVALID_CHARS: u32 = 0x0000_0008;
    let flags = if strict { MB_ERR_INVALID_CHARS } else { 0 };
    extern "system" {
        fn MultiByteToWideChar(
            code_page: u32,
            flags: u32,
            multi_byte_str: *const u8,
            multi_byte_len: i32,
            wide_char_str: *mut u16,
            wide_char_len: i32,
        ) -> i32;
    }
    let len = unsafe {
        MultiByteToWideChar(
            code_page,
            flags,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        )
    };
    if len <= 0 {
        return None;
    }
    let mut wide = vec![0u16; len as usize];
    let res = unsafe {
        MultiByteToWideChar(
            code_page,
            flags,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            len,
        )
    };
    if res > 0 {
        String::from_utf16(&wide).ok()
    } else {
        None
    }
}

/// 用 Windows 代码页编码 UTF-16 码元；返回字节和“是否有字符被替换成 ?”
#[cfg(target_os = "windows")]
fn encode_wide_to_codepage(wide: &[u16], code_page: u32) -> Option<(Vec<u8>, bool)> {
    extern "system" {
        fn WideCharToMultiByte(
            code_page: u32,
            flags: u32,
            wide_char_str: *const u16,
            wide_char_len: i32,
            multi_byte_str: *mut u8,
            multi_byte_len: i32,
            default_char: *const u8,
            used_default_char: *mut i32,
        ) -> i32;
    }
    // 关闭“近似字符”映射，无法表示的字符一律记为替换，转码提示才准确
    const WC_NO_BEST_FIT_CHARS: u32 = 0x0000_0400;
    if wide.is_empty() {
        return Some((Vec::new(), false));
    }
    let default_char = b"?";
    let len = unsafe {
        WideCharToMultiByte(
            code_page,
            WC_NO_BEST_FIT_CHARS,
            wide.as_ptr(),
            wide.len() as i32,
            std::ptr::null_mut(),
            0,
            default_char.as_ptr(),
            std::ptr::null_mut(),
        )
    };
    if len <= 0 {
        return None;
    }
    let mut bytes = vec![0u8; len as usize];
    let mut used_default = 0i32;
    let written = unsafe {
        WideCharToMultiByte(
            code_page,
            WC_NO_BEST_FIT_CHARS,
            wide.as_ptr(),
            wide.len() as i32,
            bytes.as_mut_ptr(),
            len,
            default_char.as_ptr(),
            &mut used_default,
        )
    };
    if written <= 0 {
        return None;
    }
    Some((bytes, used_default != 0))
}

/// Markdown 按扩展名识别；其余任何文本文件都按纯文本渲染，不做 Markdown 解析
pub(crate) fn document_to_html(path: &Path, raw: &str) -> (String, EnhanceFlags, Option<String>) {
    if is_markdown_document(path) {
        (
            md_to_html_with_base(raw, path.parent()),
            enhance_flags_for(raw),
            base_href_for_file(path),
        )
    } else {
        (txt_to_html(raw), EnhanceFlags::default(), None)
    }
}
/// 先登记自写记录再落盘，文件监听才能把这次写入识别为应用自己的保存
pub(crate) fn write_document_bytes(
    last_self_write: &Mutex<Option<SelfWriteRecord>>,
    path: &Path,
    bytes: Vec<u8>,
) -> std::io::Result<()> {
    *last_self_write.lock().unwrap() = Some(SelfWriteRecord {
        at: Instant::now(),
        path: path.to_path_buf(),
        content: bytes.clone(),
    });
    fs::write(path, bytes)
}

/// 按标签编码把编辑器内容写回磁盘；标签尚无编码时按 UTF-8
pub(crate) fn save_document_text(
    last_self_write: &Mutex<Option<SelfWriteRecord>>,
    path: &Path,
    content: &str,
    encoding: Option<&str>,
) -> Result<(), String> {
    let encoded = encode_document(content, encoding.unwrap_or("UTF-8"))?;
    write_document_bytes(last_self_write, path, encoded.bytes).map_err(|error| error.to_string())
}

pub(crate) fn self_write_still_matches_disk(record: Option<&SelfWriteRecord>, path: &Path) -> bool {
    let Some(record) = record else {
        return false;
    };
    record.path == path
        && record.at.elapsed() < Duration::from_millis(500)
        && fs::read(path)
            .map(|content| content == record.content)
            .unwrap_or(false)
}

pub(crate) fn should_protect_external_change(webview_dirty: bool, session_dirty: bool) -> bool {
    webview_dirty || session_dirty
}

/// 另存为对话框返回的路径没有扩展名时补上当前文件的扩展名，新文件才仍能被本应用打开
pub(crate) fn save_as_target_path(mut target: PathBuf, current: &Path) -> PathBuf {
    if target.extension().is_none() {
        if let Some(extension) = current.extension() {
            target.set_extension(extension);
        }
    }
    target
}

/// 目标编码无法表示部分字符时弹系统确认框；用户拒绝返回 false
pub(crate) fn confirm_lossy_conversion(
    strings: &Strings,
    encoding: &str,
    lossy_chars: usize,
) -> bool {
    if lossy_chars == 0 {
        return true;
    }
    let body = strings
        .convert_lossy_body
        .replace("{n}", &lossy_chars.to_string())
        .replace("{enc}", encoding);
    let result = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(strings.convert_lossy_title)
        .set_description(body)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();
    matches!(result, rfd::MessageDialogResult::Yes)
}

pub(crate) fn confirm_overwrite(strings: &Strings, path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    let result = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(strings.overwrite_title)
        .set_description(strings.overwrite_body.replace("{name}", &name))
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();
    matches!(result, rfd::MessageDialogResult::Yes)
}
