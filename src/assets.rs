// 内嵌前端资源：离线打包的语法高亮、KaTeX、Mermaid 与预览页本身
// 更新流程：更新这些文件后跑一次 ./scripts/verify.sh

pub(crate) const HLJS_JS: &str = include_str!("../assets/hljs/highlight.min.js");
pub(crate) const HLJS_LIGHT: &str = include_str!("../assets/hljs/github.min.css");
pub(crate) const HLJS_DARK: &str = include_str!("../assets/hljs/github-dark.min.css");
// Extra language pack(s) not in the `common` bundle. Each file
// ends with `hljs.registerLanguage(...)` and only works if evaluated
// in the same scope as the main bundle — we concat them into hljs-src.
pub(crate) const HLJS_EXTRA_LANGS: &str = concat!(
    // Delphi / Pascal (aliases: dpr, dfm, pas, pascal) — user requested
    include_str!("../assets/hljs/delphi.min.js"),
);
pub(crate) const PREVIEW_ENHANCE_JS: &str = include_str!("../frontend/preview-enhance.js");
pub(crate) const KATEX_JS: &str = include_str!("../assets/katex/katex.min.js");
pub(crate) const KATEX_CSS: &str = include_str!("../assets/katex/katex.inline.css");
pub(crate) const MERMAID_JS: &str = include_str!("../assets/mermaid/mermaid.min.js");

pub(crate) const PAGE_TEMPLATE: &str = include_str!("../frontend/page.html");
pub(crate) const PAGE_CSS: &str = include_str!("../frontend/page.css");
pub(crate) const PAGE_JS: &str = include_str!("../frontend/page.js");
