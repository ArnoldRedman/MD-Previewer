// 单元测试：覆盖编码、Markdown 渲染、侧栏与 IPC 解析
use super::*;
use crate::document::{
    decode_utf16, document_to_html, encode_document, read_document_to_string,
    read_document_with_encoding, save_as_target_path, save_document_text,
    self_write_still_matches_disk, should_protect_external_change, write_document_bytes,
    SelfWriteRecord,
};
// Windows 专用：代码页转换只在 Windows 上有实现
#[cfg(target_os = "windows")]
use crate::document::decode_windows_codepage;
use crate::finder::{create_finder_file, normalize_new_markdown_path};
use crate::i18n::{Lang, Strings};
use crate::ipc::{parse_finder_action, parse_ipc_message, FinderAction, IpcMessage, TabAction};
use crate::markdown::{md_to_html, md_to_html_with_base, EnhanceFlags};
use crate::page::{build_page, build_page_with_encoding, empty_preview_html};
use crate::paths::{
    is_listed_document, is_markdown_document, is_supported_document, local_document_path_from_url,
    supported_dialog_extensions,
};
use crate::platform::linux_webkit_compat_env;
use crate::sanitize::is_safe_url;
use crate::session::strip_verbatim_prefix;
use crate::sidebar::{author_doc, folder_documents, sidebar_json, strip_chapter_prefix};
use crate::updater::windows_updater;
use crate::watch::{event_should_reload_file, watch_scope_for_file};
use crate::webview::{is_script_bearing_url, navigation_decision, Navigation};
use notify::{Event, EventKind};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn temp_test_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "md-previewer-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
pub(crate) fn chapter_numbering_is_stripped_without_hurting_ordinary_titles() {
    assert_eq!(strip_chapter_prefix("第二章 一只行李箱"), "一只行李箱");
    assert_eq!(strip_chapter_prefix("第32章 什么什么"), "什么什么");
    assert_eq!(strip_chapter_prefix("第一百二十三章：归途"), "归途");
    assert_eq!(strip_chapter_prefix("32、什么什么"), "什么什么");
    // 整行只有序号时保持原样，否则复制出来是空串
    assert_eq!(strip_chapter_prefix("第二章"), "第二章");
    // 这两个开头像序号但不是，不能误伤
    assert_eq!(strip_chapter_prefix("2023年的夏天"), "2023年的夏天");
    assert_eq!(strip_chapter_prefix("第一人称叙事"), "第一人称叙事");
}

#[test]
pub(crate) fn author_doc_reads_the_chapter_heading_as_plain_text() {
    let raw = "## 第二章 一只行李箱\r\n\r\n雨没有停的意思。\r\n";

    let doc = author_doc(raw);

    assert_eq!(doc.title_line, "第二章 一只行李箱");
    assert_eq!(doc.title, "一只行李箱");
}

#[test]
pub(crate) fn author_doc_strips_inline_markdown_from_the_heading() {
    // 标题里写了行内语法时，复制出来的要和屏幕上一致，不带星号和反引号
    let doc = author_doc("## 第二章 **一只**`行李`箱\n\n正文");

    assert_eq!(doc.title_line, "第二章 一只行李箱");
    assert_eq!(doc.title, "一只行李箱");
}

#[test]
pub(crate) fn author_doc_without_a_heading_reports_no_title() {
    let doc = author_doc("\n只有正文，没有标题。\n");

    assert!(doc.title_line.is_empty());
    assert!(doc.title.is_empty());
}

#[test]
pub(crate) fn is_markdown_document_matches_markdown_extensions_case_insensitively() {
    assert!(is_markdown_document(Path::new("readme.md")));
    assert!(is_markdown_document(Path::new("SPEC.MARKDOWN")));
    assert!(is_markdown_document(Path::new("/path/to/notes.Mkd")));
    assert!(!is_markdown_document(Path::new("note.txt")));
    assert!(!is_markdown_document(Path::new("config.toml")));
    assert!(!is_markdown_document(Path::new("no_extension")));
}

#[test]
pub(crate) fn any_text_file_opens_as_plain_text_but_binaries_are_rejected() {
    let dir = temp_test_dir("open-any-text");
    let toml = dir.join("Cargo.toml");
    let no_extension = dir.join("Dockerfile");
    let utf16 = dir.join("notes.dat");
    let binary = dir.join("image.png");
    let empty = dir.join("empty.log");
    fs::write(&toml, "[package]\nname = \"x\"\n").unwrap();
    fs::write(&no_extension, "FROM scratch\n").unwrap();
    fs::write(
        &utf16,
        encode_document("宽字符", "UTF-16 LE").unwrap().bytes,
    )
    .unwrap();
    fs::write(&binary, [0x89, b'P', b'N', b'G', 0, 0, 0, 13]).unwrap();
    fs::write(&empty, b"").unwrap();

    assert!(is_supported_document(&toml));
    assert!(is_supported_document(&no_extension));
    assert!(is_supported_document(&utf16));
    assert!(is_supported_document(&empty));
    assert!(!is_supported_document(&binary));
    assert!(!is_supported_document(&dir));

    let (html, flags, base_href) = document_to_html(&toml, "[package]\nname = \"x\"\n");
    assert!(html.starts_with(r#"<div class="mdp-plain-text">"#));
    assert!(!flags.math && !flags.mermaid);
    assert!(base_href.is_none());
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn listed_documents_and_dialog_filters_use_the_text_extension_list() {
    assert!(is_listed_document(Path::new("a.json")));
    assert!(is_listed_document(Path::new("b.TOML")));
    assert!(is_listed_document(Path::new("c.md")));
    assert!(!is_listed_document(Path::new("d.png")));
    assert!(!is_listed_document(Path::new("Dockerfile")));

    let supported = supported_dialog_extensions();
    assert_eq!(supported[0], "md");
    assert!(supported.contains(&"txt"));
    assert!(supported.contains(&"json"));
    assert!(supported.contains(&"toml"));
}

#[test]
pub(crate) fn raw_html_scripts_and_event_handlers_are_stripped() {
    let html = md_to_html(
        "<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n<a href=\"javascript:alert(1)\">x</a>",
    );
    assert!(!html.contains("<script"));
    assert!(!html.contains("alert(1)"));
    assert!(html.contains("<img src=\"x\">"));
    assert!(!html.contains("onerror"));
    assert!(html.contains("<a>x</a>"));
    assert!(!html.contains("javascript:"));
}

#[test]
pub(crate) fn raw_html_split_across_lines_is_joined_before_filtering() {
    let html = md_to_html("<img\nsrc=x\nonerror=alert(1)>");
    assert!(html.contains("<img src=\"x\">"));
    assert!(!html.contains("onerror"));
}

#[test]
pub(crate) fn unterminated_raw_tags_are_escaped() {
    let html = md_to_html("<div class=\"a\n\nonclick=alert(1)>text");
    assert!(!html.contains("<div"));
    assert!(html.contains("&lt;div"));
    assert!(!html.contains("onclick=alert(1)>"));
}

#[test]
pub(crate) fn harmless_raw_html_survives_filtering() {
    let html = md_to_html(
        "<details>\n<summary>More</summary>\n\nBody <kbd>Ctrl</kbd> <a href=\"https://x.y/?a=1&amp;b=2\" title=\"t\">link</a>\n\n</details>",
    );
    assert!(html.contains("<details>"));
    assert!(html.contains("<summary>More</summary>"));
    assert!(html.contains("<kbd>Ctrl</kbd>"));
    assert!(html.contains(r#"<a href="https://x.y/?a=1&amp;b=2" title="t">link</a>"#));
    assert!(html.contains("</details>"));
}

#[test]
pub(crate) fn raw_html_rejects_entity_schemes_data_attributes_style_and_svg() {
    let html = md_to_html(
        "<a href=\"&#106;avascript:alert(1)\">a</a> <a href=\"java&Tab;script:alert(1)\">b</a>\n\n<div data-tab-id=\"1\" class=\"x\">c</div>\n\n<style>body{display:none}</style>\n\n<!-- hidden -->\n\n<svg onload=alert(1)><circle r=1/></svg>",
    );
    assert!(!html.contains("href"));
    assert!(html.contains(r#"<div class="x">c</div>"#));
    assert!(!html.contains("data-tab-id"));
    assert!(!html.contains("<style"));
    assert!(!html.contains("display:none"));
    assert!(!html.contains("hidden"));
    assert!(!html.contains("<svg"));
    assert!(!html.contains("onload"));
}

#[test]
pub(crate) fn safe_url_check_covers_schemes_and_obfuscation() {
    assert!(is_safe_url("https://example.com/a?b=1&c=2"));
    assert!(is_safe_url("./docs/readme.md"));
    assert!(is_safe_url("#section"));
    assert!(is_safe_url("data:image/png;base64,AAAA"));
    assert!(!is_safe_url("data:image/svg+xml,<svg/>"));
    assert!(!is_safe_url("javascript:alert(1)"));
    assert!(!is_safe_url("JaVaScRiPt:alert(1)"));
    assert!(!is_safe_url("java\tscript:alert(1)"));
    assert!(!is_safe_url("java&Tab;script:alert(1)"));
    assert!(!is_safe_url("&#106;avascript:alert(1)"));
    assert!(!is_safe_url("data:text/html,<script>"));
    assert!(is_script_bearing_url(" DATA:text/html,x"));
    assert!(is_script_bearing_url("blob:null/abc"));
    assert!(!is_script_bearing_url("https://example.com"));
}

#[test]
pub(crate) fn navigation_allows_only_the_first_data_url_and_routes_the_rest() {
    let mut loaded = false;
    // 首屏就是一次 data: 导航，拦掉它就是白屏
    assert_eq!(
        navigation_decision("data:text/html;charset=utf-8;base64,PGh0bWw+", &mut loaded),
        Navigation::Allow
    );
    assert!(loaded);
    assert_eq!(
        navigation_decision("data:text/html,<script>", &mut loaded),
        Navigation::Block
    );
    assert_eq!(
        navigation_decision("javascript:alert(1)", &mut loaded),
        Navigation::Block
    );
    assert_eq!(
        navigation_decision("blob:null/abc", &mut loaded),
        Navigation::Block
    );
    assert_eq!(
        navigation_decision("https://example.com", &mut loaded),
        Navigation::OpenExternally
    );
    assert_eq!(
        navigation_decision("mailto:a@b.c", &mut loaded),
        Navigation::OpenExternally
    );
    assert_eq!(
        navigation_decision("file:///nowhere/missing.md", &mut loaded),
        Navigation::Block
    );
    assert_eq!(
        navigation_decision("about:blank", &mut loaded),
        Navigation::Allow
    );

    let dir = temp_test_dir("navigation");
    let doc = dir.join("doc.md");
    fs::write(&doc, "# doc").unwrap();
    let url = url::Url::from_file_path(&doc).unwrap().to_string();
    assert_eq!(
        navigation_decision(&url, &mut loaded),
        Navigation::OpenDocument(strip_verbatim_prefix(fs::canonicalize(&doc).unwrap()))
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn utf8_bom_with_invalid_body_still_reports_bom_encoding() {
    let dir = temp_test_dir("bom-invalid");
    let path = dir.join("bad.md");
    fs::write(&path, [0xEF, 0xBB, 0xBF, b'a', 0xFF, b'b']).unwrap();
    let (text, encoding) = read_document_with_encoding(&path, None).unwrap();
    assert_eq!(encoding, "UTF-8 BOM");
    assert_eq!(text, "a\u{FFFD}b");
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn utf16_with_trailing_stray_byte_decodes_the_complete_units() {
    let dir = temp_test_dir("utf16-odd");
    let path = dir.join("odd.txt");
    fs::write(&path, [0xFF, 0xFE, b'h', 0, b'i', 0, 0x41]).unwrap();
    let (text, encoding) = read_document_with_encoding(&path, None).unwrap();
    assert_eq!(encoding, "UTF-16 LE");
    assert_eq!(text, "hi");
    assert_eq!(decode_utf16(&[0x00, 0x41, 0xD8, 0x00], true), "A\u{FFFD}");
    let _ = fs::remove_dir_all(dir);
}

#[cfg(target_os = "windows")]
#[test]
pub(crate) fn strict_codepage_detection_rejects_invalid_gbk() {
    // UTF-8 的“中”是 E4 B8 AD，末尾的 AD 是没有尾字节的 GBK 首字节，严格模式必须拒绝
    assert!(decode_windows_codepage("中".as_bytes(), 936, true).is_none());
    assert!(decode_windows_codepage("中".as_bytes(), 936, false).is_some());
    assert_eq!(
        decode_windows_codepage(&[0xD6, 0xD0], 936, true).as_deref(),
        Some("中")
    );
}

#[test]
pub(crate) fn txt_document_preserves_newlines_and_escapes_html() {
    let raw = "# Title\n\nLine 1 <tag> & more\nLine 2 *not bold*\n| table | col |\n";
    let path = Path::new("test.txt");
    let (html, flags, base_href) = document_to_html(path, raw);

    assert!(html.starts_with(r#"<div class="mdp-plain-text">"#));
    assert!(html.ends_with("</div>"));
    assert!(html.contains("&lt;tag&gt; &amp; more"));
    assert!(html.contains("# Title\n\nLine 1"));
    assert!(html.contains("Line 2 *not bold*\n| table | col |"));
    assert!(!html.contains("<h1>"));
    assert!(!html.contains("<em>"));
    assert!(!html.contains("<table>"));
    assert!(!flags.math);
    assert!(!flags.mermaid);
    assert!(base_href.is_none());
}

#[test]
pub(crate) fn document_to_html_disables_enhancers_for_txt() {
    let raw = "Math: $$E=mc^2$$\nMermaid:\n```mermaid\ngraph TD\nA-->B\n```\n";
    let (html, flags, _) = document_to_html(Path::new("notes.txt"), raw);

    assert!(!flags.math);
    assert!(!flags.mermaid);
    assert!(!html.contains("<svg"));
    assert!(html.contains("$$E=mc^2$$"));
    assert!(html.contains("```mermaid"));
}

#[test]
pub(crate) fn release_notes_render_with_document_renderer() {
    // 更新弹窗的发布说明走 md_to_html：加粗、行内代码要变成标签；
    // 标题会带 id，前端负责去掉，避免与正文锚点重名
    let html = md_to_html("## 1.3.2 Notes\n- Fixed **something** in `main.rs`\n");

    assert!(html.contains("<h2 id=\""));
    assert!(html.contains("1.3.2 Notes</h2>"));
    assert!(html.contains("<li>Fixed <strong>something</strong> in <code>main.rs</code></li>"));
    assert!(!html.contains("**"));
}

#[test]
pub(crate) fn author_doc_skips_hashes_that_are_not_headings() {
    let doc = author_doc("#不是标题，井号后面没空格\n\n## 第一章 开始\n\n正文");

    assert_eq!(doc.title_line, "第一章 开始");
    assert_eq!(doc.title, "开始");
}

#[test]
pub(crate) fn folder_documents_lists_sorted_siblings_and_skips_other_entries() {
    let dir = temp_test_dir("folder-list");
    for name in [
        "beta.md",
        "Alpha.markdown",
        "gamma.txt",
        "config.json",
        "settings.toml",
        "notes.pdf",
        "readme",
    ] {
        fs::write(dir.join(name), "x").unwrap();
    }
    fs::create_dir_all(dir.join("nested.md")).unwrap();

    let files = folder_documents(Some(&dir.join("beta.md")));

    let names = files
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "Alpha.markdown",
            "beta.md",
            "config.json",
            "gamma.txt",
            "settings.toml"
        ]
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn folder_documents_is_empty_without_an_active_document() {
    assert!(folder_documents(None).is_empty());
}

#[test]
pub(crate) fn sidebar_json_marks_the_active_document_and_carries_recent_directories() {
    let dir = temp_test_dir("sidebar-json");
    let active = dir.join("active.md");
    let other = dir.join("other.md");
    fs::write(&active, "# active").unwrap();
    fs::write(&other, "# other").unwrap();
    let mut session = DocumentSession::default();
    session.open(active.clone(), false);

    let json: serde_json::Value =
        serde_json::from_str(&sidebar_json(&session, &[other.clone()])).unwrap();

    let folder = json["folder"].as_array().unwrap();
    assert_eq!(folder.len(), 2);
    let active_entries = folder
        .iter()
        .filter(|entry| entry["active"] == serde_json::json!(true))
        .collect::<Vec<_>>();
    assert_eq!(active_entries.len(), 1);
    assert_eq!(active_entries[0]["name"], serde_json::json!("active.md"));
    let recent = json["recent"].as_array().unwrap();
    assert_eq!(recent[0]["name"], serde_json::json!("other.md"));
    assert_eq!(recent[0]["active"], serde_json::json!(false));
    // 目录列只保留目录名，不是整条路径
    let shown_dir = recent[0]["dir"].as_str().unwrap();
    assert_eq!(shown_dir, dir.file_name().unwrap().to_string_lossy());
    assert!(!shown_dir.contains(std::path::MAIN_SEPARATOR));
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn dollar_math_is_protected_from_markdown_escapes() {
    let html = md_to_html(r"$\{x\}$");

    assert!(html.contains(r#"<span class="math math-inline">\{x\}</span>"#));
}

#[test]
pub(crate) fn dollar_math_is_protected_from_markdown_emphasis() {
    let html = md_to_html(r"$\bar{\mu}_{n}$ and $x_{n}$");

    assert!(html.contains(r#"<span class="math math-inline">\bar{\mu}_{n}</span>"#));
    assert!(html.contains(r#"<span class="math math-inline">x_{n}</span>"#));
    assert!(!html.contains("<em>"));
}

#[test]
pub(crate) fn github_alerts_render_as_markdown_alert_blockquotes() {
    let html = md_to_html("> [!IMPORTANT]\n> This is an alert");

    assert!(html.contains(r#"<blockquote class="markdown-alert-important">"#));
    assert!(html.contains("<p>This is an alert</p>"));
    assert!(!html.contains("[!IMPORTANT]"));
}

#[test]
pub(crate) fn double_equals_highlight_renders_mark_without_touching_code() {
    let html = md_to_html("Use ==highlight & tag== here and `==literal==` there.");

    assert!(html.contains(r#"Use <mark class="mdp-mark">highlight &amp; tag</mark> here"#));
    assert!(html.contains("<code>==literal==</code>"));
}

#[test]
pub(crate) fn local_relative_images_are_embedded_from_markdown_directory() {
    let dir = temp_test_dir("local-image");
    let assets = dir.join("assets");
    fs::create_dir_all(&assets).unwrap();
    fs::write(assets.join("pixel.png"), b"abc").unwrap();

    let html = md_to_html_with_base("![pixel](assets/pixel.png)", Some(&dir));

    assert!(html.contains(r#"<img src="data:image/png;base64,YWJj" alt="pixel" />"#));
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn local_relative_images_keep_original_src_when_unreadable() {
    let dir = temp_test_dir("missing-image");

    let html = md_to_html_with_base("![missing](assets/missing.png)", Some(&dir));

    assert!(html.contains(r#"<img src="assets/missing.png" alt="missing" />"#));
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn local_relative_images_do_not_embed_parent_traversal() {
    let dir = temp_test_dir("traversal-image");

    let html = md_to_html_with_base("![secret](../secret.png)", Some(&dir));

    assert!(html.contains(r#"<img src="../secret.png" alt="secret" />"#));
    assert!(!html.contains("data:image/png"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn yaml_front_matter_keeps_metadata_readable_without_blank_lines() {
    let html =
        md_to_html("---\nname: skill-creator-lite\ndescription: Skill 创建封装工具。\n---\n# Body");

    assert!(html.contains(r#"<aside class="front-matter">"#));
    assert!(html.contains("name: skill-creator-lite"));
    assert!(html.contains("description: Skill 创建封装工具。"));
    assert!(html.contains("<h1 id=\"body\">Body</h1>"));
    assert!(!html.contains("<h2"));
}

#[test]
pub(crate) fn yaml_front_matter_accepts_dots_and_escapes_html() {
    let html = md_to_html("---\ntitle: <script>alert(1)</script>\n...\nParagraph");

    assert!(html.contains("title: &lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html.contains("<script>alert(1)</script>"));
    assert!(html.contains("<p>Paragraph</p>"));
}

#[test]
pub(crate) fn horizontal_rules_and_setext_headings_are_not_front_matter() {
    let html = md_to_html("Title\n---\n\nParagraph\n\n---");

    assert!(!html.contains(r#"<aside class="front-matter">"#));
    assert!(html.contains(r#"<h2 id="title">Title</h2>"#));
    assert!(html.contains("<hr />"));
}

#[test]
pub(crate) fn file_urls_only_open_existing_supported_documents() {
    let dir = temp_test_dir("document-links");
    let linked = dir.join("含 空格.md");
    let binary = dir.join("page.bin");
    fs::write(&linked, "# Linked").unwrap();
    fs::write(&binary, [0u8, 1, 2]).unwrap();

    let linked_url = url::Url::from_file_path(&linked).unwrap().to_string();
    let binary_url = url::Url::from_file_path(&binary).unwrap().to_string();
    let missing_url = url::Url::from_file_path(dir.join("missing.md"))
        .unwrap()
        .to_string();

    assert_eq!(
        local_document_path_from_url(&format!("{linked_url}#section")),
        Some(strip_verbatim_prefix(fs::canonicalize(&linked).unwrap()))
    );
    assert_eq!(local_document_path_from_url(&binary_url), None);
    assert_eq!(local_document_path_from_url(&missing_url), None);
    assert_eq!(
        local_document_path_from_url("https://example.com/readme.md"),
        None
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn linux_nvidia_compat_env_only_sets_dmabuf_when_unconfigured() {
    assert_eq!(
        linux_webkit_compat_env(None, None, true),
        Some(("WEBKIT_DISABLE_DMABUF_RENDERER", "1"))
    );
    assert_eq!(linux_webkit_compat_env(Some("0"), None, true), None);
    assert_eq!(linux_webkit_compat_env(None, Some("1"), true), None);
    assert_eq!(linux_webkit_compat_env(None, None, false), None);
}

#[test]
pub(crate) fn generated_heading_ids_support_cjk_anchor_links() {
    let html = md_to_html("1. [需求概述](#需求概述)\n\n## 需求概述");

    assert!(html.contains(r##"<a href="#%E9%9C%80%E6%B1%82%E6%A6%82%E8%BF%B0">需求概述</a>"##));
    assert!(html.contains(r#"<h2 id="需求概述">需求概述</h2>"#));
}

#[test]
pub(crate) fn generated_heading_ids_are_unique_and_keep_explicit_ids() {
    let html = md_to_html("## Intro\n## Intro\n## Custom {#fixed}\n## Fixed");

    assert!(html.contains(r#"<h2 id="intro">Intro</h2>"#));
    assert!(html.contains(r#"<h2 id="intro-1">Intro</h2>"#));
    assert!(html.contains(r#"<h2 id="fixed">Custom</h2>"#));
    assert!(html.contains(r#"<h2 id="fixed-1">Fixed</h2>"#));
}

#[test]
pub(crate) fn help_flags_are_recognized() {
    assert!(is_help_arg("-h"));
    assert!(is_help_arg("--help"));
    assert!(!is_help_arg("--edit"));
}

#[test]
pub(crate) fn page_blocks_native_preview_reload_paths() {
    let strings = Strings::for_lang(Lang::En);
    let page = build_page(
        &md_to_html("# Hello"),
        "# Hello",
        None,
        EnhanceFlags::default(),
        &strings,
        false,
    );

    assert!(page.contains("document.addEventListener('contextmenu'"));
    assert!(page.contains("window.ipc.postMessage('refresh')"));
    assert!(page.contains("id=\"btn-open\""));
    assert!(page.contains("id=\"btn-search\""));
    assert!(page.contains("window.ipc.postMessage('open')"));
    assert!(page.contains("window.__mdPreviewerOpenFile = openFile"));
    assert!(page.contains("window.__mdPreviewerShowFind = showFind"));
    assert!(page.contains("window.__mdPreviewerToggleEdit"));
    assert!(!page.contains("id=\"btn-update\""));
    assert!(!page.contains("check-updates"));
    assert!(page.contains("id=\"btn-update-available\""));
    assert!(page.contains("id=\"btn-check-update\""));
    assert!(page.contains("id=\"update-modal\""));
    assert!(page.contains("Cmd/Ctrl+F"));
    assert!(
        page.contains("if (inEdit()) return;\n\t      e.preventDefault();\n\t      showFind();")
    );
    assert!(page.contains("body.editing #btn-open"));
    assert!(page.contains("id=\"topbar\""));
    assert!(page.contains("top: var(--bar-top); z-index: 110;"));
    assert!(page
        .contains("body.editing .toolbar { position: static; opacity: 1; pointer-events: auto; }"));
    assert!(page.contains("body.editing .findbar { display: none !important; }"));
    assert!(page.contains("ta.focus({ preventScroll: true })"));
    assert!(page.contains("window.__setEmptyPreview"));
    assert!(page.contains("id=\"tabbar\""));
    assert!(page.contains("window.__setTabs"));
    assert!(page.contains("tab-action:'));") || page.contains("'tab-action:' + action"));
    assert!(page.contains("window.__markSaved"));
    assert!(!page.contains("window.ipc.postMessage('save:' + ta.value);\n\t    setDirty(false);"));
    assert!(page.contains("compositionstart"));
    assert!(page.contains("e.isComposing"));
    assert!(page.contains("focusFindInput"));
    assert!(page.contains("FIND_DEBOUNCE_MS = 300"));
    assert!(page.contains("document.createTreeWalker(previewEl, NodeFilter.SHOW_TEXT"));
    assert!(page.contains("mark.className = 'search-hit'"));
    assert!(page.contains("selectFindHit(currentFindHit + (backward ? -1 : 1))"));
    assert!(page.contains("#preview mark.search-hit.current"));
    assert!(page.contains("restoreFindInput(selectionStart, selectionEnd)"));
    assert!(page.contains("findInput.setSelectionRange(selectionStart, selectionEnd)"));
    assert!(page.contains("body.empty .toolbar"));
    assert!(page.contains("bindAnchorNavigation"));
    assert!(page.contains("event.target.closest('#preview a[href]')"));
    assert!(page.contains("window.ipc.postMessage('open-local-link:' + resolved)"));
    assert!(page.contains(".markdown-alert-important"));
    assert!(page.contains(".markdown-alert-title"));
    let light_alert = page.find("background: #dafbe1").unwrap();
    let dark_alert = page.rfind("background: #161b22; color: #d4d4d4").unwrap();
    assert!(dark_alert > light_alert);
    assert!(page.contains(".mdp-mark"));
    assert!(!page.contains("Local-first Markdown preview for AI-generated docs"));
}

#[test]
pub(crate) fn page_separates_new_file_from_open_and_debounces_autosave() {
    let strings = Strings::for_lang(Lang::En);
    let page = build_page(
        &md_to_html("# Hello"),
        "# Hello",
        None,
        EnhanceFlags::default(),
        &strings,
        false,
    );

    assert!(page.contains("window.ipc.postMessage('new-file')"));
    assert!(page.contains("tabOpen.addEventListener('click', newFile)"));
    assert!(!page.contains("tabOpen.addEventListener('click', openFile)"));
    assert!(page.contains("AUTOSAVE_DEBOUNCE_MS = 700"));
    assert!(page.contains("pendingAutosaveTimer = setTimeout"));
    assert!(page.contains("window.__markSaved = function(savedRaw)"));
    assert!(page.contains("window.__mdPreviewerResolveExternalChange"));
    assert!(page.contains("'external-change:' + (dirty ? 'dirty' : 'clean')"));
    assert!(page.contains("(e.key === 'n' || e.key === 'N')"));
    assert!(page.contains("id=\"doc-stats\""));
    assert!(page.contains("updateDocumentStats"));
    assert!(page.contains("restoreScrollProgress"));
    assert!(page.contains("md-previewer-content-zoom-v1"));
    assert!(page.contains("data-setting=\"author-mode\" data-value=\"on\""));
    assert!(page.contains("class=\"settings-help\""));
    assert!(page.contains("window.__setAuthorDoc"));
    assert!(page.contains("data-author-copy"));
    assert!(page.contains("id=\"btn-sidebar\""));
    assert!(page.contains("id=\"sidebar-list\""));
    assert!(page.contains("data-sidebar-section=\"folder\""));
    assert!(page.contains("data-sidebar-section=\"recent\""));
    assert!(page.contains("window.__setSidebar"));
    assert!(page.contains("id=\"btn-settings\""));
    assert!(page.contains("data-setting=\"open-mode\" data-value=\"new-window\""));
    assert!(page.contains("data-setting=\"tab-mode\" data-value=\"single\""));
    assert!(page.contains("window.__setSettings"));
    assert!(page.contains(r#"data-setting="theme" data-value="system""#));
    assert!(page.contains(r#"data-setting="theme" data-value="dark""#));
    assert!(page.contains("'theme': settings.theme"));
    assert!(page.contains("id=\"btn-zoom-in\""));
    assert!(page.contains("id=\"btn-zoom-out\""));
    assert!(page.contains("id=\"btn-zoom-reset\""));
}

#[test]
pub(crate) fn closing_the_last_editing_tab_restores_the_empty_preview() {
    let strings = Strings::for_lang(Lang::En);
    let page = build_page(
        &md_to_html("# Hello"),
        "# Hello",
        None,
        EnhanceFlags::default(),
        &strings,
        false,
    );
    // 空白页和缺失页共用 showPlaceholder：退出编辑、复位工具栏按钮、收起所有浮层
    let start = page
        .find("function showPlaceholder(state, previewHtml)")
        .unwrap();
    let end = page[start..]
        .find("window.__setMissing = function")
        .unwrap()
        + start;
    let handler = &page[start..end];
    assert!(handler.contains("document.body.classList.remove('editing')"));
    assert!(handler.contains("btnToggle.innerHTML = ICON_EDIT"));
    assert!(handler.contains("resetTransientUi();"));
    assert!(handler.contains("showPlaceholder('empty', previewHtml)"));
    assert!(page.contains("window.__setMissing = function(previewHtml) {{ showPlaceholder('missing', previewHtml); }};".replace("{{", "{").replace("}}", "}").as_str()));
}

#[test]
pub(crate) fn new_markdown_path_keeps_markdown_extensions_and_replaces_other_extensions() {
    assert_eq!(
        normalize_new_markdown_path(PathBuf::from("/tmp/plan")),
        PathBuf::from("/tmp/plan.md")
    );
    assert_eq!(
        normalize_new_markdown_path(PathBuf::from("/tmp/plan.markdown")),
        PathBuf::from("/tmp/plan.markdown")
    );
    assert_eq!(
        normalize_new_markdown_path(PathBuf::from("/tmp/plan.txt")),
        PathBuf::from("/tmp/plan.md")
    );
}

#[test]
pub(crate) fn page_expands_multi_column_tables() {
    let strings = Strings::for_lang(Lang::En);
    let page = build_page(
        &md_to_html("| A | B | C | D |\n|---|---|---|---|\n| 1 | 2 | 3 | 4 |"),
        "",
        None,
        EnhanceFlags::default(),
        &strings,
        false,
    );

    assert!(page.contains("mdp-table-wrap"));
    assert!(page.contains("width: min(calc(100vw - 64px), 1280px)"));
    assert!(page.contains("if(window.__enhancePreview)window.__enhancePreview();"));
}

#[test]
pub(crate) fn empty_state_exposes_open_and_recent_files() {
    let strings = Strings::for_lang(Lang::En);
    let recent = vec![PathBuf::from("/tmp/example.md")];
    let html = empty_preview_html(&strings, &recent);

    assert!(html.contains("Open File"));
    assert!(html.contains("Recent"));
    assert!(html.contains("example.md"));
    assert!(html.contains("data-recent-index=\"0\""));
    assert!(html.contains(r#"<div class="empty has-recent">"#));
    assert!(html.contains(r#"<div class="icon">#</div>"#));
    assert!(!html.contains("empty-mark"));

    let page = build_page(&html, "", None, EnhanceFlags::default(), &strings, true);
    assert!(page.contains(".empty.has-recent"));
    assert!(!page.contains(".empty.has-recent .recent { max-height"));
}

#[test]
pub(crate) fn vim_style_target_rewrite_events_reload_current_file() {
    let target = PathBuf::from("/tmp/note.md");
    let ev = Event::new(EventKind::Create(notify::event::CreateKind::File))
        .add_path(PathBuf::from("/tmp/note.md"));

    assert!(event_should_reload_file(&ev, &target));
}

#[test]
pub(crate) fn sibling_file_events_do_not_reload_current_file() {
    let target = PathBuf::from("/tmp/note.md");
    let ev = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
        notify::event::DataChange::Any,
    )))
    .add_path(PathBuf::from("/tmp/other.md"));

    assert!(!event_should_reload_file(&ev, &target));
}

#[test]
pub(crate) fn self_write_suppression_checks_disk_content_not_only_time() {
    let dir = temp_test_dir("self-write-suppression");
    let path = dir.join("note.md");
    fs::write(&path, "saved by app").unwrap();
    let record = SelfWriteRecord {
        at: Instant::now(),
        path: path.clone(),
        content: b"saved by app".to_vec(),
    };

    assert!(self_write_still_matches_disk(Some(&record), &path));

    fs::write(&path, "external edit").unwrap();
    assert!(!self_write_still_matches_disk(Some(&record), &path));
}

#[test]
pub(crate) fn write_document_bytes_records_encoded_bytes_for_watcher() {
    let dir = temp_test_dir("self-write-bytes");
    let path = dir.join("utf16.md");
    let holder = Mutex::new(None);
    let bytes = encode_document("字节比较", "UTF-16 LE").unwrap().bytes;

    write_document_bytes(&holder, &path, bytes.clone()).unwrap();

    assert_eq!(fs::read(&path).unwrap(), bytes);
    let record = holder.lock().unwrap();
    assert!(self_write_still_matches_disk(record.as_ref(), &path));
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn save_document_text_reports_unknown_encoding() {
    let dir = temp_test_dir("save-text");
    let path = dir.join("note.md");
    let holder = Mutex::new(None);

    assert!(save_document_text(&holder, &path, "x", Some("Latin-1")).is_err());
    assert!(!path.exists());
    save_document_text(&holder, &path, "正文", None).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "正文");
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn external_change_protection_uses_dirty_state_from_either_side() {
    assert!(!should_protect_external_change(false, false));
    assert!(should_protect_external_change(true, false));
    assert!(should_protect_external_change(false, true));
    assert!(should_protect_external_change(true, true));
}

#[test]
pub(crate) fn file_watch_scope_is_parent_directory() {
    let target = PathBuf::from("/tmp/note.md");

    assert_eq!(watch_scope_for_file(&target), Path::new("/tmp"));
}

#[test]
pub(crate) fn finder_action_parses_encoded_folder_and_kind() {
    assert_eq!(
        parse_finder_action("mdpreviewer://finder?action=create&path=%2Ftmp%2FMy%20Notes&kind=md"),
        Some(FinderAction::Create {
            folder: PathBuf::from("/tmp/My Notes"),
            kind: "md".to_string(),
        })
    );
    assert!(parse_finder_action("https://example.com/").is_none());
}

#[test]
pub(crate) fn finder_create_uses_non_conflicting_markdown_name() {
    let dir = temp_test_dir("finder-create");
    fs::write(dir.join("新建.md"), "existing").unwrap();

    let created = create_finder_file(&dir, "md").unwrap();

    assert_eq!(created.file_name().unwrap(), "新建 2.md");
    assert_eq!(fs::read_to_string(created).unwrap(), "");
    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn read_document_to_string_handles_utf8_bom_and_utf16() {
    let dir = temp_test_dir("encoding-detection");

    // 1. 标准 UTF-8
    let p_utf8 = dir.join("utf8.txt");
    fs::write(&p_utf8, "hello 编码".as_bytes()).unwrap();
    assert_eq!(read_document_to_string(&p_utf8).unwrap(), "hello 编码");

    // 2. 带 BOM 的 UTF-8
    let p_utf8_bom = dir.join("utf8_bom.txt");
    let mut bytes_bom = vec![0xEF, 0xBB, 0xBF];
    bytes_bom.extend_from_slice("with bom 文本".as_bytes());
    fs::write(&p_utf8_bom, &bytes_bom).unwrap();
    assert_eq!(
        read_document_to_string(&p_utf8_bom).unwrap(),
        "with bom 文本"
    );

    // 3. 带 BOM 的 UTF-16 LE
    let p_utf16_le = dir.join("utf16_le.txt");
    let u16s: Vec<u16> = "utf16 文本".encode_utf16().collect();
    let mut bytes_le = vec![0xFF, 0xFE];
    for u in u16s {
        bytes_le.extend_from_slice(&u.to_le_bytes());
    }
    fs::write(&p_utf16_le, &bytes_le).unwrap();
    assert_eq!(read_document_to_string(&p_utf16_le).unwrap(), "utf16 文本");

    // 4. 带 BOM 的 UTF-16 BE
    let p_utf16_be = dir.join("utf16_be.txt");
    let u16s: Vec<u16> = "utf16 be 文本".encode_utf16().collect();
    let mut bytes_be = vec![0xFE, 0xFF];
    for u in u16s {
        bytes_be.extend_from_slice(&u.to_be_bytes());
    }
    fs::write(&p_utf16_be, &bytes_be).unwrap();
    assert_eq!(
        read_document_to_string(&p_utf16_be).unwrap(),
        "utf16 be 文本"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn build_page_includes_all_ux_enhancement_components() {
    let strings = Strings::for_lang(Lang::En);
    let page = build_page(
        "# Heading",
        "# Heading",
        None,
        EnhanceFlags::default(),
        &strings,
        false,
    );
    assert!(page.contains("code-copy-btn"));
    assert!(page.contains("data-sidebar-section=\"outline\""));
    assert!(page.contains("id=\"btn-split\""));
    assert!(page.contains("id=\"tab-context-menu\""));
    assert!(page.contains("data-tab-action=\"close-others\""));
    assert!(page.contains("data-tab-action=\"copy-path\""));
    assert!(page.contains("data-tab-action=\"reveal\""));
    assert!(page.contains("id=\"recent-context-menu\""));
    assert!(page.contains("data-recent-action=\"reveal\""));
    assert!(page.contains("data-recent-action=\"copy-path\""));
    assert!(page.contains("data-recent-action=\"remove\""));
    assert!(page.contains("id=\"sidebar-clear-recent\""));
    assert!(page.contains("id=\"sidebar-tooltip\""));
    assert!(page.contains("'forget-recent:' + path"));
    assert!(page.contains("'reveal-path:' + path"));
    assert!(page.contains("postMessage('clear-recent')"));
    assert!(page.contains("id=\"lightbox\""));
    assert!(page.contains("data-setting=\"word-wrap\""));
    assert!(page.contains("id=\"encoding-control\""));
    assert!(page.contains("id=\"btn-encoding\""));
    assert!(page.contains("id=\"encoding-popover\""));
    assert!(page.contains("order: 1;"));
    assert!(page.contains("order: 2;"));
    assert!(page.contains("border-right: 2px solid"));
}

#[test]
pub(crate) fn build_page_includes_encoding_convert_and_save_as() {
    let strings = Strings::for_lang(Lang::Zh);
    let page = build_page_with_encoding(
        "<p>x</p>",
        "x",
        None,
        EnhanceFlags::default(),
        &strings,
        false,
        "UTF-8 BOM",
    );
    assert!(page.contains("data-encoding=\"UTF-8 BOM\">UTF-8 BOM</button>"));
    assert!(page.contains("class=\"encoding-option active\" data-encoding=\"UTF-8 BOM\""));
    assert!(page.contains("class=\"encoding-option\" data-encoding=\"UTF-8\">"));
    for encoding in ["UTF-8", "UTF-8 BOM", "GBK", "UTF-16 LE", "UTF-16 BE"] {
        assert!(page.contains(&format!("data-convert-encoding=\"{encoding}\"")));
    }
    assert!(page.contains("id=\"btn-save-as\""));
    assert!(page.contains(">以此编码重新打开<"));
    assert!(page.contains(">转换为<"));
    assert!(page.contains("'convert-encoding:' + enc + '\\n' + ta.value"));
    assert!(page.contains("'save-as\\n' + ta.value"));
    assert!(page.contains("e.shiftKey && (e.key === 's' || e.key === 'S')"));
}

#[test]
pub(crate) fn save_as_target_path_fills_missing_extension_from_current_file() {
    let current = Path::new("D:/docs/readme.md");
    assert_eq!(
        save_as_target_path(PathBuf::from("D:/docs/copy"), current),
        PathBuf::from("D:/docs/copy.md")
    );
    assert_eq!(
        save_as_target_path(PathBuf::from("D:/docs/copy.txt"), current),
        PathBuf::from("D:/docs/copy.txt")
    );
    assert_eq!(
        save_as_target_path(PathBuf::from("D:/docs/copy.html"), current),
        PathBuf::from("D:/docs/copy.html")
    );
}

#[test]
pub(crate) fn ipc_messages_parse_into_typed_commands() {
    assert_eq!(parse_ipc_message("open"), Some(IpcMessage::OpenFile));
    assert_eq!(
        parse_ipc_message("open-doc:D:\\docs\\a.md"),
        Some(IpcMessage::OpenDoc(PathBuf::from("D:\\docs\\a.md")))
    );
    assert_eq!(
        parse_ipc_message("open-recent:2"),
        Some(IpcMessage::OpenRecent(2))
    );
    assert_eq!(parse_ipc_message("open-recent:x"), None);
    assert_eq!(
        parse_ipc_message("tab-action:close-others:7"),
        Some(IpcMessage::TabAction {
            action: TabAction::CloseOthers,
            id: 7,
            content: None,
        })
    );
    assert_eq!(
        parse_ipc_message("tab-action:activate:3\nline one\nline: two"),
        Some(IpcMessage::TabAction {
            action: TabAction::Activate,
            id: 3,
            content: Some("line one\nline: two".to_string()),
        })
    );
    assert_eq!(parse_ipc_message("tab-action:activate:abc"), None);
    assert_eq!(parse_ipc_message("tab-action:rename:3"), None);
    assert_eq!(
        parse_ipc_message("convert-encoding:GBK\n正文"),
        Some(IpcMessage::ConvertEncoding {
            encoding: "GBK".to_string(),
            content: "正文".to_string(),
        })
    );
    assert_eq!(
        parse_ipc_message("save-as\nbody"),
        Some(IpcMessage::SaveAs("body".to_string()))
    );
    assert_eq!(
        parse_ipc_message("save:a:b"),
        Some(IpcMessage::Save("a:b".to_string()))
    );
    assert_eq!(
        parse_ipc_message("save-skipped"),
        Some(IpcMessage::SaveSkipped)
    );
    assert_eq!(
        parse_ipc_message("set-setting:tab-mode=single"),
        Some(IpcMessage::SetSetting {
            key: "tab-mode".to_string(),
            value: "single".to_string(),
        })
    );
    assert_eq!(parse_ipc_message("set-setting:broken"), None);
    assert_eq!(
        parse_ipc_message("dirty:1"),
        Some(IpcMessage::DirtyChanged(true))
    );
    assert_eq!(
        parse_ipc_message("external-change:clean"),
        Some(IpcMessage::ExternalChangeResolved { dirty: false })
    );
    assert_eq!(
        parse_ipc_message("clear-recent"),
        Some(IpcMessage::ClearRecent)
    );
    assert_eq!(parse_ipc_message("refresh"), Some(IpcMessage::Refresh));
    assert_eq!(parse_ipc_message("open-log"), Some(IpcMessage::OpenLog));
    assert_eq!(parse_ipc_message("clear-log"), Some(IpcMessage::ClearLog));
    assert_eq!(
        parse_ipc_message("log-error:something broke"),
        Some(IpcMessage::LogError("something broke".to_string()))
    );
    assert_eq!(
        parse_ipc_message("set-setting:disable-all-shortcuts=on"),
        Some(IpcMessage::SetSetting {
            key: "disable-all-shortcuts".to_string(),
            value: "on".to_string(),
        })
    );
    assert_eq!(
        parse_ipc_message("set-setting:disable-shortcut=close-tab"),
        Some(IpcMessage::SetSetting {
            key: "disable-shortcut".to_string(),
            value: "close-tab".to_string(),
        })
    );
    assert_eq!(parse_ipc_message("open:"), None);
    assert_eq!(parse_ipc_message("bogus"), None);
}

#[test]
pub(crate) fn encode_document_prefixes_bom_for_unicode_encodings() {
    let utf8 = encode_document("中a", "UTF-8").unwrap();
    assert_eq!(utf8.bytes, "中a".as_bytes());
    assert_eq!(utf8.lossy_chars, 0);

    let utf8_bom = encode_document("中a", "UTF-8 BOM").unwrap();
    assert_eq!(&utf8_bom.bytes[..3], &[0xEF, 0xBB, 0xBF]);
    assert_eq!(&utf8_bom.bytes[3..], "中a".as_bytes());

    let le = encode_document("中a", "UTF-16 LE").unwrap();
    assert_eq!(le.bytes, vec![0xFF, 0xFE, 0x2D, 0x4E, 0x61, 0x00]);

    let be = encode_document("中a", "UTF-16 BE").unwrap();
    assert_eq!(be.bytes, vec![0xFE, 0xFF, 0x4E, 0x2D, 0x00, 0x61]);

    assert!(encode_document("x", "Latin-1").is_err());
}

#[cfg(target_os = "windows")]
#[test]
pub(crate) fn encode_document_counts_characters_gbk_cannot_represent() {
    let clean = encode_document("测试 GBK 编码", "GBK").unwrap();
    assert_eq!(clean.lossy_chars, 0);
    assert_eq!(
        decode_windows_codepage(&clean.bytes, 936, true).unwrap(),
        "测试 GBK 编码"
    );

    let lossy = encode_document("前😀中🚀后", "GBK").unwrap();
    assert_eq!(lossy.lossy_chars, 2);
    let decoded = decode_windows_codepage(&lossy.bytes, 936, true).unwrap();
    assert_eq!(decoded.replace('?', ""), "前中后");
    assert!(decoded.contains('?'));
}

#[test]
pub(crate) fn utf8_bom_is_detected_and_preserved_on_write() {
    let dir = temp_test_dir("utf8-bom");
    let path = dir.join("bom.md");
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice("带 BOM".as_bytes());
    fs::write(&path, &bytes).unwrap();

    let (content, encoding) = read_document_with_encoding(&path, None).unwrap();
    assert_eq!(content, "带 BOM");
    assert_eq!(encoding, "UTF-8 BOM");

    let (content, encoding) = read_document_with_encoding(&path, Some("UTF-8 BOM")).unwrap();
    assert_eq!((content.as_str(), encoding), ("带 BOM", "UTF-8 BOM"));

    let (_, encoding) = read_document_with_encoding(&path, Some("UTF-8")).unwrap();
    assert_eq!(encoding, "UTF-8");

    fs::write(&path, encode_document("改写", "UTF-8 BOM").unwrap().bytes).unwrap();
    assert!(fs::read(&path).unwrap().starts_with(&[0xEF, 0xBB, 0xBF]));
    assert_eq!(
        read_document_with_encoding(&path, None).unwrap(),
        ("改写".to_string(), "UTF-8 BOM")
    );

    fs::write(&path, b"").unwrap();
    assert_eq!(
        read_document_with_encoding(&path, Some("UTF-8 BOM"))
            .unwrap()
            .1,
        "UTF-8 BOM"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn read_and_write_document_with_encoding_roundtrips() {
    let dir = temp_test_dir("encoding-roundtrip");
    let p = dir.join("test.txt");

    // UTF-8
    fs::write(
        &p,
        encode_document("测试 UTF-8 编码", "UTF-8").unwrap().bytes,
    )
    .unwrap();
    let (content, enc) = read_document_with_encoding(&p, None).unwrap();
    assert_eq!(content, "测试 UTF-8 编码");
    assert_eq!(enc, "UTF-8");

    // UTF-16 LE
    fs::write(
        &p,
        encode_document("测试 UTF-16 LE", "UTF-16 LE")
            .unwrap()
            .bytes,
    )
    .unwrap();
    let (content, enc) = read_document_with_encoding(&p, None).unwrap();
    assert_eq!(content, "测试 UTF-16 LE");
    assert_eq!(enc, "UTF-16 LE");

    // UTF-16 BE
    fs::write(
        &p,
        encode_document("测试 UTF-16 BE", "UTF-16 BE")
            .unwrap()
            .bytes,
    )
    .unwrap();
    let (content, enc) = read_document_with_encoding(&p, None).unwrap();
    assert_eq!(content, "测试 UTF-16 BE");
    assert_eq!(enc, "UTF-16 BE");

    #[cfg(target_os = "windows")]
    {
        // GBK
        fs::write(&p, encode_document("测试 GBK 编码", "GBK").unwrap().bytes).unwrap();
        let (content, enc) = read_document_with_encoding(&p, Some("GBK")).unwrap();
        assert_eq!(content, "测试 GBK 编码");
        assert_eq!(enc, "GBK");
    }

    let _ = fs::remove_dir_all(dir);
}

#[test]
pub(crate) fn build_page_includes_auto_update_components() {
    let strings = Strings::for_lang(Lang::En);
    let page = build_page(
        "# Heading",
        "# Heading",
        None,
        EnhanceFlags::default(),
        &strings,
        false,
    );
    assert!(page.contains("id=\"btn-update-available\""));
    assert!(page.contains("id=\"btn-check-update\""));
    assert!(page.contains("id=\"update-modal\""));
    assert!(page.contains("UPDATE_CHECK_INTERVAL_MS"));
    assert!(page.contains("self-update:"));
    assert!(page.contains("isNewerVersion"));
    assert!(page.contains("id=\"update-progress-fill\""));
    assert!(page.contains("window.__setUpdateProgress = setUpdateProgress"));
    assert!(page.contains("window.__setUpdateFailed = setUpdateFailed"));
}

#[test]
pub(crate) fn update_url_whitelist_safety_checks() {
    assert!(windows_updater::is_allowed_update_url(
        "https://github.com/ArnoldRedman/MD-Previewer/releases/download/v1.2.2/MD-Previewer-Setup.exe"
    ));
    assert!(windows_updater::is_allowed_update_url(
        "https://github.com/ArnoldRedman/MD-Previewer/releases/download/v1.2.2/MD-Previewer-windows-x64.exe"
    ));
    assert!(windows_updater::is_allowed_update_url(
        "https://github.com/ArnoldRedman/md-preview/releases/tag/v1.2.2"
    ));
    assert!(!windows_updater::is_allowed_update_url(
        "https://github.com/malicious/repo/releases/download/v1/bad.exe"
    ));
    assert!(!windows_updater::is_allowed_update_url(
        "http://example.com/bad.exe"
    ));
    assert!(!windows_updater::is_allowed_update_url(
        "https://evil-phishing.com/setup.exe"
    ));
}

#[test]
pub(crate) fn page_template_fills_every_placeholder() {
    let strings = Strings::for_lang(Lang::Zh);
    let page = build_page(
        &empty_preview_html(&strings, &[PathBuf::from("/tmp/a.md")]),
        "# 标题",
        Some("file:///tmp/"),
        EnhanceFlags::default(),
        &strings,
        false,
    );
    // 模板里残留 {{...}} 说明占位符名和参数列表对不上
    assert!(!page.contains("{{"), "页面模板存在未替换的占位符");
    // 前端文案通过 __mdPreviewerConfig 注入，页面里必须带上它
    assert!(page.contains(r#"window.__mdPreviewerConfig = {"#));
    assert!(page.contains(&format!(r#""statWordsJs":"{}""#, strings.stat_words)));
}

#[test]
pub(crate) fn page_config_covers_every_frontend_key() {
    let strings = Strings::for_lang(Lang::En);
    let page = build_page("", "", None, EnhanceFlags::default(), &strings, true);
    // page.js 里读 CFG.xxx 的字段必须都由 Rust 注入，键名写错前端只会静默拿到 undefined
    let page_js = include_str!("../frontend/page.js");
    let mut checked = 0;
    for (index, _) in page_js.match_indices("CFG.") {
        let rest = &page_js[index + "CFG.".len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        let key = &rest[..end];
        if key.is_empty() {
            continue;
        }
        assert!(
            page.contains(&format!(r#""{key}""#)),
            "页面配置缺少前端要读的 {key}"
        );
        checked += 1;
    }
    assert!(
        checked > 20,
        "只检查到 {checked} 个前端配置项，page.js 大概没被读到"
    );
}
