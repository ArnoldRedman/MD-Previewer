#[cfg(target_os = "windows")]
fn main() {
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
