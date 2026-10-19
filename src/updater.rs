// 应用内更新：下载当前平台的发布包，交给接手脚本替换后重启。
// Windows 覆盖 exe 或跑 Setup 安装器；Linux 便携版覆盖单个二进制，deb 版交给 dpkg

use std::path::{Path, PathBuf};

/// 更新包在 temp 里的统一前缀，下次下载前据此清理上次残留
#[cfg(any(target_os = "windows", target_os = "linux"))]
const STAGING_PREFIX: &str = "md-preview-update-";
/// 进度回报的最小间隔，避免把事件循环刷满
#[cfg(any(target_os = "windows", target_os = "linux"))]
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
/// 更新包上限，防止异常响应把磁盘写爆
#[cfg(any(target_os = "windows", target_os = "linux"))]
const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
/// .deb 把二进制固定装到 /usr/bin，包安装成功后按这个路径重启新版本
#[cfg(any(target_os = "linux", test))]
const LINUX_PACKAGED_BINARY: &str = "/usr/bin/md-previewer";

/// 更新包下载地址只认本仓库的 release 资产
#[cfg_attr(not(any(target_os = "windows", target_os = "linux")), allow(dead_code))]
pub(crate) fn is_allowed_update_url(url: &str) -> bool {
    url.starts_with("https://github.com/ArnoldRedman/MD-Previewer/releases/")
        || url.starts_with("https://github.com/ArnoldRedman/md-preview/releases/")
}

/// 发布资产名后缀（按优先级）。页面据此挑包，避免在 Linux 上误下 Windows 安装器；
/// 空列表表示本平台没有可自动安装的包，只能打开发布页手动处理
pub(crate) fn asset_suffixes() -> Vec<&'static str> {
    #[cfg(target_os = "windows")]
    {
        vec!["-windows-x64.exe", "-Setup.exe"]
    }
    #[cfg(target_os = "linux")]
    {
        // 未知架构不给后缀：宁可让用户去发布页，也别下错架构的包
        match (std::env::consts::ARCH, exe_dir_writable()) {
            ("x86_64", true) => vec!["-linux-x64.tar.gz"],
            ("x86_64", false) => vec!["-linux-amd64.deb"],
            ("aarch64", true) => vec!["-linux-arm64.tar.gz"],
            ("aarch64", false) => vec!["-linux-arm64.deb"],
            _ => Vec::new(),
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Vec::new()
    }
}

/// 便携版的二进制放哪都能写，直接覆盖；deb 装在 /usr/bin 只有 root 能写，
/// 这种只能下 .deb 交给 dpkg。用户自己装进 /usr/local/bin 也走这条
#[cfg(target_os = "linux")]
fn exe_dir_writable() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(dir) = exe.parent() else {
        return false;
    };
    let probe = dir.join(format!("{STAGING_PREFIX}probe"));
    match std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 在调用线程里同步下载更新包到 temp，边写边通过 on_progress 回报字节数。
/// 返回落盘路径；任何失败都会把半截文件删掉
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn download_update(
    download_url: &str,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf, String> {
    if !is_allowed_update_url(download_url) {
        return Err("Disallowed update URL".to_string());
    }
    remove_stale_files();
    let dest = staging_path(download_url);
    let result = download_to(download_url, &dest, &mut on_progress);
    if result.is_err() {
        let _ = std::fs::remove_file(&dest);
    }
    result.map(|()| dest)
}

/// 清理 temp 里历史更新的残留：上次失败或被杀的接手脚本不会自己收尾
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn remove_stale_files() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // setup 是 Windows 安装包的独立前缀，一起清以免留下历史残留
        if !name.starts_with(STAGING_PREFIX) && !name.starts_with("md-preview-setup-") {
            continue;
        }
        let path = entry.path();
        // tar 的解压目录要先删内容，remove_file 对目录无效
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// 交给系统安装器/包管理器的更新包：Windows 是 Setup.exe，Linux 是 .deb
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn is_package(name: &str) -> bool {
    let name = name.to_lowercase();
    #[cfg(target_os = "windows")]
    {
        name.contains("setup")
    }
    #[cfg(target_os = "linux")]
    {
        name.ends_with(".deb")
    }
}

#[cfg(target_os = "windows")]
fn staging_path(download_url: &str) -> PathBuf {
    let pid = std::process::id();
    let name = if is_package(download_url) {
        format!("md-preview-setup-{pid}.exe")
    } else {
        format!("{STAGING_PREFIX}{pid}.exe")
    };
    std::env::temp_dir().join(name)
}

#[cfg(target_os = "linux")]
fn staging_path(download_url: &str) -> PathBuf {
    let ext = if is_package(download_url) {
        "deb"
    } else {
        "tar.gz"
    };
    std::env::temp_dir().join(format!("{STAGING_PREFIX}{}.{ext}", std::process::id()))
}

#[cfg(target_os = "windows")]
fn tls_provider() -> ureq::tls::TlsProvider {
    ureq::tls::TlsProvider::NativeTls
}

#[cfg(target_os = "linux")]
fn tls_provider() -> ureq::tls::TlsProvider {
    ureq::tls::TlsProvider::Rustls
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn download_to(
    download_url: &str,
    dest: &Path,
    on_progress: &mut impl FnMut(u64, Option<u64>),
) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::time::Instant;

    // 用系统证书库校验，公司代理注入的根证书才认得出来
    let tls = ureq::tls::TlsConfig::builder()
        .provider(tls_provider())
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(std::time::Duration::from_secs(15)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(30)))
        .user_agent(concat!("md-previewer/", env!("CARGO_PKG_VERSION")))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut response = agent
        .get(download_url)
        .call()
        .map_err(|error| error.to_string())?;
    let total = response.body().content_length();
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .reader();
    let mut file = std::fs::File::create(dest).map_err(|error| error.to_string())?;
    let mut buffer = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    let mut last_report = Instant::now();
    on_progress(0, total);
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| error.to_string())?;
        downloaded += read as u64;
        if last_report.elapsed() >= PROGRESS_INTERVAL {
            on_progress(downloaded, total);
            last_report = Instant::now();
        }
    }
    file.flush().map_err(|error| error.to_string())?;
    drop(file);
    if let Some(total) = total {
        if downloaded != total {
            return Err(format!(
                "Incomplete download: {downloaded} of {total} bytes"
            ));
        }
    }
    let min_size = if is_package(download_url) {
        100_000
    } else {
        1_000_000
    };
    if downloaded < min_size {
        return Err(format!("Downloaded file too small: {downloaded} bytes"));
    }
    on_progress(downloaded, Some(downloaded));
    Ok(())
}

/// 单引号包裹，路径里的单引号按 shell 规则转义
#[cfg(any(target_os = "linux", test))]
fn sh_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

/// 接手脚本要处理的路径。写成纯数据，方便在非 Linux 平台上单测脚本文本
#[cfg(any(target_os = "linux", test))]
struct Handoff<'a> {
    /// 要等的旧进程
    pid: u32,
    /// 便携版要被替换的二进制；包安装时是失败回退拉起的旧版本
    target: &'a Path,
    /// tar 解压出来的新二进制；包安装时为 None
    new_binary: Option<&'a Path>,
    /// 下载好的 .deb；便携版为 None
    package: Option<&'a Path>,
    /// tar 解压目录，脚本收尾时删掉
    stage: Option<&'a Path>,
}

/// 生成接手脚本：等旧进程退出（它还要写回未保存的编辑和会话），再替换或安装并重启。
/// 脚本留在 temp 里由下次更新的 remove_stale_files 清理，避免删正在被 sh 读取的文件
#[cfg(any(target_os = "linux", test))]
fn linux_handoff_script(handoff: &Handoff) -> String {
    let target = sh_quote(handoff.target);
    let mut script = String::from("#!/bin/sh\n");
    // 最多等 30 秒，超时就照常往下走，免得一个卡住的旧进程把更新拖死
    script.push_str(&format!(
        "i=0\nwhile kill -0 {} 2>/dev/null; do\n  sleep 0.2\n  i=$((i + 1))\n  [ \"$i\" -ge 150 ] && break\ndone\n",
        handoff.pid
    ));
    if let Some(new_binary) = handoff.new_binary {
        // 先写同目录的 .new 再改名，半截写入不会毁掉正在用的二进制
        script.push_str(&format!(
            "if cp -f {} {target}.new 2>/dev/null; then\n  chmod 0755 {target}.new && mv -f {target}.new {target}\nfi\n",
            sh_quote(new_binary)
        ));
    }
    if let Some(package) = handoff.package {
        let package_s = sh_quote(package);
        // pkexec 弹系统授权框；没有 polkit 的桌面再用免交互 sudo，都不行就保留旧版本
        script.push_str(&format!(
            "if command -v pkexec >/dev/null 2>&1; then\n  pkexec dpkg -i {package_s}\nelse\n  sudo -n dpkg -i {package_s}\nfi\ninstalled=$?\nif [ \"$installed\" -eq 0 ]; then\n  rm -f {package_s}\nfi\n"
        ));
    }
    if let Some(stage) = handoff.stage {
        script.push_str(&format!("rm -rf {}\n", sh_quote(stage)));
    }
    if handoff.package.is_some() {
        // 装成功重启包里的新路径；失败（用户取消授权等）重启旧版本，别让人没界面可用
        script.push_str(&format!(
            "if [ \"$installed\" -eq 0 ]; then\n  nohup {} >/dev/null 2>&1 &\nelse\n  nohup {target} >/dev/null 2>&1 &\nfi\n",
            sh_quote(Path::new(LINUX_PACKAGED_BINARY))
        ));
    } else {
        script.push_str(&format!("nohup {target} >/dev/null 2>&1 &\n"));
    }
    script
}

/// 解出 tar 包里的二进制。包结构由 scripts/build-linux.sh 固定为
/// md-previewer-<版本>-linux-<arch>/md-previewer
#[cfg(target_os = "linux")]
fn extract_linux_binary(package: &Path, stage: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(stage);
    std::fs::create_dir_all(stage).map_err(|error| format!("无法创建解压目录：{error}"))?;
    let status = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(package)
        .arg("-C")
        .arg(stage)
        .status()
        .map_err(|error| format!("无法运行 tar：{error}"))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(stage);
        return Err(format!("解压更新包失败（tar 退出码 {:?}）", status.code()));
    }
    let extracted = std::fs::read_dir(stage).ok().and_then(|entries| {
        entries
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.is_dir())
    });
    let binary = extracted
        .map(|dir| dir.join("md-previewer"))
        .filter(|path| path.is_file());
    match binary {
        Some(path) => Ok(path),
        None => {
            let _ = std::fs::remove_dir_all(stage);
            Err("更新包结构异常，里面没有找到 md-previewer".to_string())
        }
    }
}

/// 写出并启动接手脚本。便携版替换二进制，包安装版调 dpkg 装新包；调用方随后负责退出本进程
#[cfg(target_os = "linux")]
pub(crate) fn launch_installer(downloaded: &Path) -> Result<(), String> {
    use std::process::Stdio;

    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let pid = std::process::id();
    let package = is_package(&downloaded.to_string_lossy()).then(|| downloaded.to_path_buf());
    let (new_binary, stage) = if package.is_some() {
        (None, None)
    } else {
        let stage = std::env::temp_dir().join(format!("{STAGING_PREFIX}{pid}-stage"));
        let binary = extract_linux_binary(downloaded, &stage)?;
        (Some(binary), Some(stage))
    };
    let handoff = Handoff {
        pid,
        target: &current_exe,
        new_binary: new_binary.as_deref(),
        package: package.as_deref(),
        stage: stage.as_deref(),
    };
    let script = std::env::temp_dir().join(format!("{STAGING_PREFIX}{pid}.sh"));
    std::fs::write(&script, linux_handoff_script(&handoff)).map_err(|error| error.to_string())?;
    // 三个流都断开：旧进程退到后台后不该再往终端写
    std::process::Command::new("sh")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// 安装包走 Start-Process 交给安装器；便携版等本进程退出后覆盖当前 exe 并重启。
/// 脚本完全无窗口运行；调用方随后负责退出本进程
#[cfg(target_os = "windows")]
pub(crate) fn launch_installer(downloaded: &Path) -> Result<(), String> {
    use std::process::Command;

    /// 不给 powershell.exe 分配控制台，否则即使 -WindowStyle Hidden 也会闪一下黑窗
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let pid = std::process::id();
    let update_script = std::env::temp_dir().join(format!("{STAGING_PREFIX}{pid}.ps1"));
    let downloaded_name = downloaded
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let installer = downloaded_name.starts_with("md-preview-setup-");

    let target_s = current_exe.to_string_lossy().replace('\'', "''");
    let script_s = update_script.to_string_lossy().replace('\'', "''");
    let download_s = downloaded.to_string_lossy().replace('\'', "''");

    let ps_content = if installer {
        format!(
            r#"$ErrorActionPreference = 'Stop'
$downloaded = '{download_s}'
$script = '{script_s}'
$pidToWait = {pid}

try {{
    Wait-Process -Id $pidToWait -Timeout 30 -ErrorAction SilentlyContinue
    Start-Process -FilePath $downloaded
}} catch {{
    Start-Process -FilePath '{target_s}'
}} finally {{
    Remove-Item -LiteralPath $script -Force -ErrorAction SilentlyContinue
}}
"#
        )
    } else {
        format!(
            r#"$ErrorActionPreference = 'Stop'
$target = '{target_s}'
$downloaded = '{download_s}'
$script = '{script_s}'
$pidToWait = {pid}

try {{
    Wait-Process -Id $pidToWait -Timeout 30 -ErrorAction SilentlyContinue
    $copied = $false
    $started = $false
    for ($i = 0; $i -lt 80; $i++) {{
        try {{
            Copy-Item -LiteralPath $downloaded -Destination $target -Force -ErrorAction Stop
            $copied = $true
            break
        }} catch {{
            Start-Sleep -Milliseconds 250
        }}
    }}
    if (-not $copied) {{
        try {{
            Start-Process -FilePath powershell.exe -WindowStyle Hidden -ArgumentList "-NoProfile -WindowStyle Hidden -Command Copy-Item -LiteralPath '$downloaded' -Destination '$target' -Force; Start-Process -FilePath '$target'" -Verb RunAs -Wait
            $copied = $true
            $started = $true
        }} catch {{}}
    }}
    if ($copied -and -not $started) {{
        Start-Process -FilePath $target
    }}
}} catch {{
    Start-Process -FilePath $target
}} finally {{
    Remove-Item -LiteralPath $downloaded -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $script -Force -ErrorAction SilentlyContinue
}}
"#
        )
    };

    std::fs::write(&update_script, ps_content).map_err(|e| e.to_string())?;

    use std::os::windows::process::CommandExt;
    Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-WindowStyle")
        .arg("Hidden")
        .arg("-File")
        .arg(&update_script)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn rejects_urls_outside_the_release_pages() {
        let mut calls = 0;
        let error = download_update("https://example.com/x.exe", |_, _| calls += 1).unwrap_err();
        assert_eq!(error, "Disallowed update URL");
        assert_eq!(calls, 0);
    }

    /// 真实走一遍 GitHub 下载：需要网络，默认忽略，手动 `cargo test -- --ignored` 验证
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore]
    fn downloads_the_installer_with_monotonic_progress() {
        let url = "https://github.com/ArnoldRedman/MD-Previewer/releases/download/v1.4.1/MD-Previewer-Setup.exe";
        let mut reports: Vec<(u64, Option<u64>)> = Vec::new();
        let path =
            download_update(url, |downloaded, total| reports.push((downloaded, total))).unwrap();
        let size = std::fs::metadata(&path).unwrap().len();
        let _ = std::fs::remove_file(&path);
        assert_eq!(size, 1_986_560);
        assert!(reports.len() >= 2, "{reports:?}");
        assert_eq!(reports[0], (0, Some(size)));
        assert_eq!(*reports.last().unwrap(), (size, Some(size)));
        assert!(reports.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_assets_are_the_two_exe_builds() {
        assert_eq!(asset_suffixes(), vec!["-windows-x64.exe", "-Setup.exe"]);
    }

    /// Linux 上用 sh -n 验一遍生成脚本的语法：字符串拼错了只有真机才会发现
    #[cfg(target_os = "linux")]
    #[test]
    fn handoff_scripts_parse_with_sh() {
        let cases = [
            linux_handoff_script(&Handoff {
                pid: 1,
                target: Path::new("/opt/it's here/md-previewer"),
                new_binary: Some(Path::new("/tmp/md-preview-update-1-stage/x/md-previewer")),
                package: None,
                stage: Some(Path::new("/tmp/md-preview-update-1-stage")),
            }),
            linux_handoff_script(&Handoff {
                pid: 1,
                target: Path::new("/usr/bin/md-previewer"),
                new_binary: None,
                package: Some(Path::new("/tmp/md-preview-update-1.deb")),
                stage: None,
            }),
        ];
        for (index, script) in cases.iter().enumerate() {
            let path = std::env::temp_dir().join(format!("md-preview-sh-check-{index}.sh"));
            std::fs::write(&path, script).unwrap();
            let status = std::process::Command::new("sh")
                .arg("-n")
                .arg(&path)
                .status()
                .unwrap();
            let _ = std::fs::remove_file(&path);
            assert!(status.success(), "handoff script {index} has syntax errors");
        }
    }

    #[test]
    fn sh_quote_escapes_single_quotes() {
        let path = Path::new("/tmp/it's md-previewer");
        assert_eq!(sh_quote(path), r"'/tmp/it'\''s md-previewer'");
    }

    /// 便携版：覆盖二进制、清理解压目录、最后把原路径重新拉起
    #[test]
    fn portable_handoff_replaces_the_binary_then_relaunches() {
        let script = linux_handoff_script(&Handoff {
            pid: 4242,
            target: Path::new("/home/u/bin/md-previewer"),
            new_binary: Some(Path::new(
                "/tmp/md-preview-update-4242-stage/x/md-previewer",
            )),
            package: None,
            stage: Some(Path::new("/tmp/md-preview-update-4242-stage")),
        });
        assert!(script.starts_with("#!/bin/sh\n"), "{script}");
        // 必须先等旧进程写完 session.json 再动手
        assert!(script.contains("kill -0 4242"), "{script}");
        assert!(
            script.contains("cp -f '/tmp/md-preview-update-4242-stage/x/md-previewer' '/home/u/bin/md-previewer'.new"),
            "{script}"
        );
        assert!(script.contains("chmod 0755"), "{script}");
        assert!(
            script.contains("rm -rf '/tmp/md-preview-update-4242-stage'"),
            "{script}"
        );
        assert!(
            script
                .trim_end()
                .ends_with("nohup '/home/u/bin/md-previewer' >/dev/null 2>&1 &"),
            "{script}"
        );
        assert!(!script.contains("dpkg"), "{script}");
    }

    /// 包安装：pkexec 提权装 deb，成功重启 /usr/bin 下的新版本，失败保留旧版本
    #[test]
    fn package_handoff_installs_through_pkexec_and_falls_back() {
        let script = linux_handoff_script(&Handoff {
            pid: 7,
            target: Path::new("/usr/bin/md-previewer"),
            new_binary: None,
            package: Some(Path::new("/tmp/md-preview-update-7.deb")),
            stage: None,
        });
        assert!(script.contains("command -v pkexec"), "{script}");
        assert!(
            script.contains("pkexec dpkg -i '/tmp/md-preview-update-7.deb'"),
            "{script}"
        );
        assert!(
            script.contains("sudo -n dpkg -i '/tmp/md-preview-update-7.deb'"),
            "{script}"
        );
        assert!(
            script.contains("rm -f '/tmp/md-preview-update-7.deb'"),
            "{script}"
        );
        assert!(
            script.contains("nohup '/usr/bin/md-previewer' >/dev/null 2>&1 &"),
            "{script}"
        );
        // 失败分支必须还有一次拉起旧版本，不能把人晾在关窗状态
        assert_eq!(
            script.matches("nohup").count(),
            2,
            "expected both success and fallback relaunch: {script}"
        );
    }
}
