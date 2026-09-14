// 应用内更新：下载安装包并交给替换脚本

#[cfg(target_os = "windows")]
pub(crate) mod windows_updater {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, Instant};

    /// 不给 powershell.exe 分配控制台，否则即使 -WindowStyle Hidden 也会闪一下黑窗
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    /// 进度回报的最小间隔，避免把事件循环刷满
    const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
    /// 更新包上限，防止异常响应把磁盘写爆
    const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

    pub fn is_allowed_update_url(url: &str) -> bool {
        url.starts_with("https://github.com/ArnoldRedman/MD-Previewer/releases/")
            || url.starts_with("https://github.com/ArnoldRedman/md-preview/releases/")
    }

    /// 安装包走 Start-Process 交给安装器；便携版直接覆盖当前 exe
    pub(crate) fn is_installer(download_url: &str) -> bool {
        download_url.to_lowercase().contains("setup")
    }

    pub(crate) fn staging_path(download_url: &str) -> PathBuf {
        let pid = std::process::id();
        let name = if is_installer(download_url) {
            format!("md-preview-setup-{pid}.exe")
        } else {
            format!("md-preview-update-{pid}.exe")
        };
        std::env::temp_dir().join(name)
    }

    /// 清理 temp 目录中历史更新残留：上次失败或被杀的脚本不会自己收尾
    pub(crate) fn remove_stale_files() {
        let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if s.starts_with("md-preview-update-") || s.starts_with("md-preview-setup-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// 在调用线程里同步下载更新包到 temp，边写边通过 on_progress 回报字节数。
    /// 返回落盘路径；任何失败都会把半截文件删掉
    pub fn download_update(
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
        result.map(|_| dest)
    }

    pub(crate) fn download_to(
        download_url: &str,
        dest: &Path,
        on_progress: &mut impl FnMut(u64, Option<u64>),
    ) -> Result<(), String> {
        // 用系统证书库校验，公司代理注入的根证书才认得出来
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let config = ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
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
        let mut file = File::create(dest).map_err(|error| error.to_string())?;
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
        let min_size = if is_installer(download_url) {
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

    /// 写出并启动接手脚本：等本进程退出后用下载好的文件覆盖当前 exe 并重启，
    /// 或直接拉起安装器。脚本完全无窗口运行；调用方随后负责退出本进程
    pub fn launch_installer(downloaded: &Path) -> Result<(), String> {
        let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let pid = std::process::id();
        let update_script = std::env::temp_dir().join(format!("md-preview-update-{pid}.ps1"));
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

        #[test]
        fn rejects_urls_outside_the_release_pages() {
            let mut calls = 0;
            let error =
                download_update("https://example.com/x.exe", |_, _| calls += 1).unwrap_err();
            assert_eq!(error, "Disallowed update URL");
            assert_eq!(calls, 0);
        }

        /// 真实走一遍 GitHub 下载：需要网络，默认忽略，手动 `cargo test -- --ignored` 验证
        #[test]
        #[ignore]
        fn downloads_the_installer_with_monotonic_progress() {
            let url = "https://github.com/ArnoldRedman/MD-Previewer/releases/download/v1.4.1/MD-Previewer-Setup.exe";
            let mut reports: Vec<(u64, Option<u64>)> = Vec::new();
            let path = download_update(url, |downloaded, total| reports.push((downloaded, total)))
                .unwrap();
            let size = std::fs::metadata(&path).unwrap().len();
            let _ = std::fs::remove_file(&path);
            assert_eq!(size, 1_986_560);
            assert!(reports.len() >= 2, "{reports:?}");
            assert_eq!(reports[0], (0, Some(size)));
            assert_eq!(*reports.last().unwrap(), (size, Some(size)));
            assert!(reports.windows(2).all(|pair| pair[0].0 <= pair[1].0));
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) mod windows_updater {
    pub fn is_allowed_update_url(url: &str) -> bool {
        url.starts_with("https://github.com/ArnoldRedman/MD-Previewer/releases/")
            || url.starts_with("https://github.com/ArnoldRedman/md-preview/releases/")
    }
}
