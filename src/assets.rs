// 内嵌前端资源：离线打包的语法高亮、KaTeX、Mermaid 与预览页本身
// 更新流程：更新这些文件后跑一次 ./scripts/verify.sh
//
// 体积大的三个（Mermaid 3.3MB、KaTeX 272KB、highlight.js 122KB）由 build.rs 压成 gzip 后再内嵌，
// 用到时才解压一次并缓存；未压缩内嵌会让 exe 多出约 2.7MB。小文件保持原样，不值得为此多一次解压
use std::io::Read;
use std::sync::OnceLock;

pub(crate) const HLJS_JS_GZ: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/highlight.min.js.gz"));
pub(crate) const KATEX_JS_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/katex.min.js.gz"));
pub(crate) const MERMAID_JS_GZ: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/mermaid.min.js.gz"));

pub(crate) const HLJS_LIGHT: &str = include_str!("../assets/hljs/github.min.css");
pub(crate) const HLJS_DARK: &str = include_str!("../assets/hljs/github-dark.min.css");
// Extra language pack(s) not in the `common` bundle. Each file
// ends with `hljs.registerLanguage(...)` and only works if evaluated
// in the same scope as the main bundle — we concat them into hljs-src.
pub(crate) const HLJS_EXTRA_LANGS: &str = concat!(
    // Delphi / Pascal (aliases: dpr, dfm, pas, pascal) — user requested
    include_str!("../assets/hljs/delphi.min.js"),
);
pub(crate) const KATEX_CSS: &str = include_str!("../assets/katex/katex.inline.css");

pub(crate) const PAGE_TEMPLATE: &str = include_str!("../frontend/page.html");
pub(crate) const PAGE_CSS: &str = include_str!("../frontend/page.css");
pub(crate) const PAGE_JS: &str = include_str!("../frontend/page.js");
pub(crate) const PREVIEW_ENHANCE_JS: &str = include_str!("../frontend/preview-enhance.js");

// 解压失败只可能是构建脚本产出的资源和代码对不上，属于构建期就该失败的情况
fn inflate(name: &str, packed: &[u8]) -> String {
    let mut out = String::with_capacity(packed.len() * 4);
    flate2::read::GzDecoder::new(packed)
        .read_to_string(&mut out)
        .unwrap_or_else(|error| panic!("failed to inflate embedded {name}: {error}"));
    out
}

pub(crate) fn hljs_js() -> &'static str {
    static SRC: OnceLock<String> = OnceLock::new();
    SRC.get_or_init(|| inflate("highlight.min.js", HLJS_JS_GZ))
}

pub(crate) fn katex_js() -> &'static str {
    static SRC: OnceLock<String> = OnceLock::new();
    SRC.get_or_init(|| inflate("katex.min.js", KATEX_JS_GZ))
}

pub(crate) fn mermaid_js() -> &'static str {
    static SRC: OnceLock<String> = OnceLock::new();
    SRC.get_or_init(|| inflate("mermaid.min.js", MERMAID_JS_GZ))
}
