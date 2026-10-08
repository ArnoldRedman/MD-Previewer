use std::io::Write;

/// 构建期压成 gzip 的前端资源。
/// Mermaid / KaTeX / highlight.js 原文合计约 3.7MB，直接 include_str! 会让 exe 膨胀到 6MB 以上；
/// 压缩后由 `assets.rs` 在真正用到时解压一次。仓库里只留未压缩的原文，压缩产物落 OUT_DIR，
/// 不提交进版本库，也就不会和原文不同步
const GZIP_ASSETS: &[(&str, &str)] = &[
    ("assets/mermaid/mermaid.min.js", "mermaid.min.js.gz"),
    ("assets/katex/katex.min.js", "katex.min.js.gz"),
    ("assets/hljs/highlight.min.js", "highlight.min.js.gz"),
];

fn stage_gzipped_assets() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    for (src, name) in GZIP_ASSETS {
        println!("cargo:rerun-if-changed={src}");
        let raw = std::fs::read(src)
            .unwrap_or_else(|error| panic!("failed to read embedded asset {src}: {error}"));
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        encoder
            .write_all(&raw)
            .unwrap_or_else(|error| panic!("failed to gzip {src}: {error}"));
        let packed = encoder
            .finish()
            .unwrap_or_else(|error| panic!("failed to gzip {src}: {error}"));
        std::fs::write(out.join(name), packed)
            .unwrap_or_else(|error| panic!("failed to write packed asset {name}: {error}"));
    }
}

#[cfg(target_os = "windows")]
fn stage_embedded(env_var: &str, file_name: &str) {
    println!("cargo:rerun-if-env-changed={env_var}");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(file_name);
    if let Ok(src) = std::env::var(env_var) {
        if !src.is_empty() {
            println!("cargo:rerun-if-changed={src}");
            std::fs::copy(&src, &out).unwrap_or_else(|error| {
                panic!("failed to embed {src}: {error}");
            });
            return;
        }
    }
    if !out.exists() {
        std::fs::write(&out, []).unwrap();
    }
}

#[cfg(target_os = "windows")]
fn windows_resources() {
    stage_embedded("MD_PREVIEWER_SHELL_DLL", "md-previewer-shell.dll");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    let version = env!("CARGO_PKG_VERSION");
    let file_version = format!("{version}.0");
    res.set("CompanyName", "ArnoldRedman");
    res.set("FileDescription", "MD Previewer");
    res.set("FileVersion", &file_version);
    res.set("LegalCopyright", "Copyright (c) 2026 ArnoldRedman");
    res.set("ProductName", "MD Previewer");
    res.set("ProductVersion", version);
    res.compile().expect("failed to compile Windows resources");
}

fn main() {
    stage_gzipped_assets();
    #[cfg(target_os = "windows")]
    windows_resources();
}
