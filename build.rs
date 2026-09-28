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
fn main() {
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

#[cfg(not(target_os = "windows"))]
fn main() {}
