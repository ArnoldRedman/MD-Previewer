use crate::UserEvent;
use std::path::{Path, PathBuf};
use tao::event_loop::EventLoopProxy;

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use crate::is_supported_document;
    use notify::{RecursiveMode, Watcher};
    use serde::{Deserialize, Serialize};
    use std::fs::{self, File, OpenOptions};
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const LOCK_FILE: &str = "instance.lock";
    const REQUEST_DIR: &str = "instance-requests";
    const FILE_SHARE_READ: u32 = 0x00000001;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const FALLBACK_POLL_INTERVAL: Duration = Duration::from_millis(250);
    const MAX_REQUEST_BYTES: u64 = 1024 * 1024;
    const MAX_REQUEST_AGE: Duration = Duration::from_secs(30);

    #[link(name = "user32")]
    unsafe extern "system" {
        fn AllowSetForegroundWindow(process_id: u32) -> i32;
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct OpenRequest {
        paths: Vec<PathBuf>,
        edit: bool,
        created_at_ms: u128,
    }

    pub(crate) enum Startup {
        Primary(Server),
        Forwarded,
    }

    pub(crate) struct Server {
        lock: Option<File>,
        lock_path: PathBuf,
        request_dir: PathBuf,
    }

    impl Server {
        fn disabled(config_dir: &Path) -> Self {
            Self {
                lock: None,
                lock_path: config_dir.join(LOCK_FILE),
                request_dir: config_dir.join(REQUEST_DIR),
            }
        }

        pub(crate) fn start(&self, proxy: EventLoopProxy<UserEvent>) {
            if self.lock.is_none() {
                return;
            }
            let request_dir = self.request_dir.clone();
            thread::spawn(move || watch_requests(request_dir, proxy));
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.lock.take();
            let _ = fs::remove_file(&self.lock_path);
        }
    }

    pub(crate) fn prepare(config_dir: &Path, paths: &[PathBuf], edit: bool) -> Startup {
        if fs::create_dir_all(config_dir).is_err() {
            return Startup::Primary(Server::disabled(config_dir));
        }
        let lock_path = config_dir.join(LOCK_FILE);
        let request_dir = config_dir.join(REQUEST_DIR);

        match acquire_lock(&lock_path) {
            Ok(lock) => Startup::Primary(Server {
                lock: Some(lock),
                lock_path,
                request_dir,
            }),
            Err(error) if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION) => {
                allow_primary_to_focus(&lock_path);
                if write_request(&request_dir, paths, edit) {
                    Startup::Forwarded
                } else {
                    Startup::Primary(Server::disabled(config_dir))
                }
            }
            Err(_) => Startup::Primary(Server::disabled(config_dir)),
        }
    }

    fn acquire_lock(lock_path: &Path) -> std::io::Result<File> {
        let mut lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .share_mode(FILE_SHARE_READ)
            .open(lock_path)?;
        lock.set_len(0)?;
        write!(lock, "{}", std::process::id())?;
        lock.flush()?;
        Ok(lock)
    }

    fn allow_primary_to_focus(lock_path: &Path) {
        let Some(process_id) = fs::read_to_string(lock_path)
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
        else {
            return;
        };
        unsafe {
            let _ = AllowSetForegroundWindow(process_id);
        }
    }

    fn write_request(request_dir: &Path, paths: &[PathBuf], edit: bool) -> bool {
        if fs::create_dir_all(request_dir).is_err() {
            return false;
        }
        let id = format!("{}-{}", now_nanos(), std::process::id());
        let temporary = request_dir.join(format!("{id}.tmp"));
        let request_path = request_dir.join(format!("{id}.json"));
        let request = OpenRequest {
            paths: paths.to_vec(),
            edit,
            created_at_ms: now_millis(),
        };
        let result = File::create(&temporary)
            .and_then(|mut file| {
                serde_json::to_writer(&mut file, &request).map_err(std::io::Error::other)?;
                file.flush()
            })
            .and_then(|_| fs::rename(&temporary, &request_path));
        if result.is_err() {
            let _ = fs::remove_file(temporary);
            return false;
        }
        true
    }

    fn watch_requests(request_dir: PathBuf, proxy: EventLoopProxy<UserEvent>) {
        if fs::create_dir_all(&request_dir).is_err() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        });
        if let Ok(mut watcher) = watcher {
            if watcher
                .watch(&request_dir, RecursiveMode::NonRecursive)
                .is_ok()
            {
                // 先处理监听建立前已经入队的请求，再等待后续文件事件
                process_requests(&request_dir, &proxy);
                while receiver.recv().is_ok() {
                    process_requests(&request_dir, &proxy);
                }
                return;
            }
        }

        // ponytail: 仅在系统文件监听不可用时轮询；若出现真实空闲开销再加平台专用 IPC
        loop {
            process_requests(&request_dir, &proxy);
            thread::sleep(FALLBACK_POLL_INTERVAL);
        }
    }

    fn process_requests(request_dir: &Path, proxy: &EventLoopProxy<UserEvent>) {
        let Ok(entries) = fs::read_dir(request_dir) else {
            return;
        };
        let mut paths = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();
        for request_path in paths {
            process_request(&request_path, proxy);
        }
    }

    fn process_request(request_path: &Path, proxy: &EventLoopProxy<UserEvent>) {
        let request = fs::metadata(request_path)
            .ok()
            .filter(|metadata| metadata.len() <= MAX_REQUEST_BYTES)
            .and_then(|_| fs::read(request_path).ok())
            .and_then(|raw| serde_json::from_slice::<OpenRequest>(&raw).ok());
        let Some(request) = request else {
            let _ = fs::remove_file(request_path);
            return;
        };
        if now_millis().saturating_sub(request.created_at_ms) > MAX_REQUEST_AGE.as_millis() {
            let _ = fs::remove_file(request_path);
            return;
        }
        let paths = request
            .paths
            .into_iter()
            .filter(|path| path.is_file() && is_supported_document(path))
            .collect();
        if proxy
            .send_event(UserEvent::OpenPaths(paths, request.edit))
            .is_ok()
        {
            let _ = fs::remove_file(request_path);
        }
    }

    fn now_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    }

    fn now_nanos() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn only_one_process_can_hold_the_lock() {
            let dir = temp_dir("lock");
            let lock_path = dir.join(LOCK_FILE);
            let first = acquire_lock(&lock_path).unwrap();
            assert_eq!(
                fs::read_to_string(&lock_path).unwrap(),
                std::process::id().to_string()
            );

            let second = acquire_lock(&lock_path).unwrap_err();

            assert_eq!(second.raw_os_error(), Some(ERROR_SHARING_VIOLATION));
            drop(first);
            assert!(acquire_lock(&lock_path).is_ok());
            let _ = fs::remove_dir_all(dir);
        }

        #[test]
        fn second_launch_writes_request_for_the_primary() {
            let dir = temp_dir("forward");
            let document = dir.join("计划.md");
            fs::write(&document, "# 计划").unwrap();
            let Startup::Primary(primary) = prepare(&dir, &[], false) else {
                panic!("first launch must become primary");
            };

            let second = prepare(&dir, std::slice::from_ref(&document), true);

            assert!(matches!(second, Startup::Forwarded));
            let request_path = fs::read_dir(dir.join(REQUEST_DIR))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let request: OpenRequest =
                serde_json::from_slice(&fs::read(request_path).unwrap()).unwrap();
            assert_eq!(request.paths, vec![document]);
            assert!(request.edit);
            drop(primary);
            assert!(!dir.join(LOCK_FILE).exists());
            let _ = fs::remove_dir_all(dir);
        }

        fn temp_dir(name: &str) -> PathBuf {
            let unique = now_millis();
            let dir = std::env::temp_dir().join(format!(
                "md-previewer-instance-{name}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&dir).unwrap();
            dir
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;

    #[allow(dead_code)]
    pub(crate) enum Startup {
        Primary(Server),
        Forwarded,
    }

    pub(crate) struct Server;

    impl Server {
        pub(crate) fn start(&self, _proxy: EventLoopProxy<UserEvent>) {}
    }

    pub(crate) fn prepare(_config_dir: &Path, _paths: &[PathBuf], _edit: bool) -> Startup {
        Startup::Primary(Server)
    }
}

pub(crate) use platform::{prepare, Startup};
