// 运行与崩溃日志：大小限制为 5KB，超出自动滚动清理旧日志以控制资源开销
use crate::paths::config_dir;
use crate::platform::reveal_in_file_manager;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const MAX_LOG_SIZE: usize = 5 * 1024; // 5KB

static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn log_file_path() -> PathBuf {
    config_dir().join("app.log")
}

pub fn current_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = (secs / 86400) as i64;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

pub fn write_log(level: &str, message: &str) {
    let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = log_file_path();
    write_log_to_path(&path, level, message, MAX_LOG_SIZE);
}

pub(crate) fn write_log_to_path(path: &Path, level: &str, message: &str, max_bytes: usize) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let timestamp = current_timestamp();
    let entry = format!("[{timestamp}] [{level}] {message}\n");
    let entry_bytes = entry.as_bytes();

    let existing = fs::read(path).unwrap_or_default();
    let total_len = existing.len() + entry_bytes.len();

    if total_len <= max_bytes {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(entry_bytes);
        }
    } else {
        // 超出限制时滚动清理：截取末尾保留较新的内容
        let mut combined = existing;
        combined.extend_from_slice(entry_bytes);

        let start_idx = combined.len().saturating_sub(max_bytes);
        let mut slice = &combined[start_idx..];

        // 尽量对齐到换行符，避免截断单行日志
        if let Some(pos) = slice.iter().position(|&b| b == b'\n') {
            if pos + 1 < slice.len() {
                slice = &slice[pos + 1..];
            }
        } else {
            // 确保从有效 UTF-8 字符边界开始
            while !slice.is_empty() && std::str::from_utf8(slice).is_err() {
                slice = &slice[1..];
            }
        }

        let prefix = b"[... older logs cleared to stay under 5KB ...]\n";
        let mut final_buf = Vec::new();
        if slice.len() + prefix.len() <= max_bytes {
            final_buf.extend_from_slice(prefix);
            final_buf.extend_from_slice(slice);
        } else {
            final_buf.extend_from_slice(slice);
        }

        if final_buf.len() > max_bytes {
            let start = final_buf.len() - max_bytes;
            final_buf = final_buf[start..].to_vec();
        }

        let _ = fs::write(path, final_buf);
    }
}

pub fn clear_log() {
    let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = log_file_path();
    let _ = fs::write(path, b"");
}

pub fn reveal_log() {
    let path = log_file_path();
    if !path.exists() {
        write_log("INFO", "Log initialized");
    }
    reveal_in_file_manager(&path);
}

pub fn init_panic_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };
        let message = format!("PANIC at {location}: {payload}");
        write_log("CRASH", &message);
        prev_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_writes_and_cleans_when_exceeding_max_bytes() {
        let dir = std::env::temp_dir().join(format!(
            "md-previewer-logger-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let log_file = dir.join("test.log");

        // 1. 写入小数据
        write_log_to_path(&log_file, "INFO", "Hello world", 200);
        let content = fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("[INFO] Hello world"));
        assert!(content.len() <= 200);

        // 2. 连续写入大量数据，验证严格限制在 max_bytes 内
        for i in 0..50 {
            write_log_to_path(
                &log_file,
                "WARN",
                &format!("Warning entry number {i:03} with some extra padding text"),
                200,
            );
            let len = fs::metadata(&log_file).unwrap().len() as usize;
            assert!(
                len <= 200,
                "Log file size {len} exceeded max_bytes 200 at iteration {i}"
            );
        }

        // 3. 最新的一条日志一定保留在文件中
        let final_content = fs::read_to_string(&log_file).unwrap();
        assert!(final_content.contains("number 049"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn log_stays_strictly_under_5kb() {
        let dir = std::env::temp_dir().join(format!(
            "md-previewer-5kb-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let log_file = dir.join("app.log");

        // 写入 200 条单条约 100 字节的日志（总计约 20KB）
        for i in 0..200 {
            write_log_to_path(
                &log_file,
                "ERROR",
                &format!("Crash dump test iteration {i}: Something failed in edit mode"),
                MAX_LOG_SIZE,
            );
            let len = fs::metadata(&log_file).unwrap().len() as usize;
            assert!(
                len <= MAX_LOG_SIZE,
                "Log file size {len} exceeded 5KB limit {MAX_LOG_SIZE}"
            );
        }

        let content = fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("iteration 199"));
        assert!(content.len() <= MAX_LOG_SIZE);

        let _ = fs::remove_dir_all(dir);
    }
}
